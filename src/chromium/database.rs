//! The Chromium cookie database: its schema for the shared SQLite store, and what only
//! Chromium does, finding app-bound values and decrypting values with the browser's key.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::chromium::crypto::{self, ChromiumKey};
use crate::chromium::{chromium_to_unix, unix_to_chromium};
use crate::sqlite_store::{Filter, Query, Schema, Store, query_error};
use crate::{Cookie, Error, Result};

/// The cookie database's file name in a Chromium profile folder.
const DATABASE_FILE: &str = "Cookies";
/// Since Chrome 96 on Windows the database is in this subfolder of the profile.
const NETWORK_FOLDER: &str = "Network";
/// The columns [`read`] builds a cookie from, in the order it reads them.
const COOKIE_COLUMNS: &str =
    "host_key, name, path, is_secure, is_httponly, has_expires, expires_utc, encrypted_value";
/// The condition for a value with app-bound encryption; `?` is its version marker.
const APP_BOUND_CONDITION: &str = "substr(encrypted_value, 1, 3) = ?";

/// The schema of a Chromium cookie database.
pub(crate) struct ChromiumSchema;

/// A copied and opened Chromium cookie database.
pub(crate) type ChromiumStore = Store<ChromiumSchema>;

impl Schema for ChromiumSchema {
    const TABLE: &'static str = "cookies";
    const HOST: &'static str = "host_key";
    const LAST_ACCESS: &'static str = "last_access_utc";
    const SENDABLE: &'static str = "(has_expires = 0 OR expires_utc > ?1)";

    /// `Network\Cookies` where it exists (Windows), else `Cookies`.
    fn database_path(profile_dir: &Path) -> PathBuf {
        let network = profile_dir.join(NETWORK_FOLDER).join(DATABASE_FILE);
        if network.is_file() {
            network
        } else {
            profile_dir.join(DATABASE_FILE)
        }
    }

    /// The version in the `meta` table, stored as text.
    fn version(conn: &Connection) -> Result<i64> {
        let version: String = conn
            .query_row("SELECT value FROM meta WHERE key = 'version'", [], |row| {
                row.get(0)
            })
            .map_err(query_error)?;
        let version = version
            .parse()
            .map_err(|_| Error::DatabaseQuery(format!("version {version:?} is not a number")))?;
        crypto::check_db_version(version)?;
        Ok(version)
    }

    fn now(_version: i64, now_unix: i64) -> i64 {
        unix_to_chromium(now_unix)
    }

    fn last_access_to_unix(stored: i64) -> i64 {
        chromium_to_unix(stored)
    }
}

/// One row as stored, before its value is decrypted.
struct Encrypted {
    /// `host_key`.
    domain: String,
    /// `name`.
    name: String,
    /// `path`.
    path: String,
    /// `is_secure`.
    secure: bool,
    /// `is_httponly`.
    http_only: bool,
    /// `expires_utc` as Unix seconds, or `None` without `has_expires`.
    expires: Option<i64>,
    /// `encrypted_value`.
    value: Vec<u8>,
}

impl Encrypted {
    /// The cookie, with its value decrypted with `key` for a database of `db_version`.
    fn decrypt(self, key: &ChromiumKey, db_version: i64) -> Result<Cookie> {
        let value = crypto::decrypt_value(&self.value, key, &self.domain, &self.name, db_version)?;
        Ok(Cookie {
            domain: self.domain,
            name: self.name,
            value,
            path: self.path,
            secure: self.secure,
            http_only: self.http_only,
            expires: self.expires,
        })
    }
}

/// Whether a sendable cookie stored under one of `domains` uses app-bound encryption.
/// Nothing is decrypted, so this can run before asking for the browser's key.
pub(crate) fn uses_app_bound_encryption(
    store: &ChromiumStore,
    domains: &[String],
    now_unix: i64,
) -> Result<bool> {
    let query = Query {
        columns: "1",
        domains,
        now_unix,
        filter: Some(Filter {
            condition: APP_BOUND_CONDITION,
            value: &crypto::APP_BOUND_PREFIX,
        }),
    };
    Ok(!store.rows(&query, |_| Ok(()))?.is_empty())
}

/// Decrypts the sendable cookies stored under one of `domains`.
///
/// If one value cannot be decrypted, the whole read fails with that cookie's error. A
/// login with a cookie missing fails later in ways that are hard to explain, and a value
/// stored for another site must never be used.
pub(crate) fn read(
    store: &ChromiumStore,
    domains: &[String],
    key: &ChromiumKey,
    now_unix: i64,
) -> Result<Vec<Cookie>> {
    let query = Query {
        columns: COOKIE_COLUMNS,
        domains,
        now_unix,
        filter: None,
    };
    let rows = store.rows(&query, |row| {
        let has_expiry: bool = row.get(5)?;
        let expiry: i64 = row.get(6)?;
        Ok(Encrypted {
            domain: row.get(0)?,
            name: row.get(1)?,
            path: row.get(2)?,
            secure: row.get(3)?,
            http_only: row.get(4)?,
            expires: has_expiry.then(|| chromium_to_unix(expiry)),
            value: row.get(7)?,
        })
    })?;
    rows.into_iter()
        .map(|row| row.decrypt(key, store.version()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chromium::test_vectors::{NOW, create_db, key, row};
    use crate::domain::candidate_domains;
    use crate::private_dir::PrivateDir;

    #[test]
    fn reads_only_sendable_cookies_of_the_requested_site() {
        let dir = PrivateDir::new().unwrap();
        create_db(
            dir.path(),
            "24",
            &[
                row(".example.com", "session", Some(NOW + 60)),
                row(".example.com", "no-expiry", None),
                row(".example.com", "expired", Some(NOW - 60)),
                row("other.com", "foreign", Some(NOW + 60)),
            ],
        );
        let store = ChromiumStore::of(dir.path()).unwrap();
        let domains = candidate_domains("www.example.com");

        assert_eq!(store.last_used_for(&domains, NOW).unwrap(), Some(NOW - 10));
        let mut cookies = read(&store, &domains, &key(), NOW).unwrap();
        cookies.sort_by(|a, b| a.name.cmp(&b.name));
        let names: Vec<_> = cookies.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["no-expiry", "session"]);
        assert_eq!(cookies[1].value.expose(), "hello-world");
        assert_eq!(cookies[1].expires, Some(NOW + 60));
        assert!(cookies[1].secure && cookies[1].http_only);
        assert_eq!(cookies[0].expires, None);
    }

    #[test]
    fn last_activity_counts_every_site() {
        let dir = PrivateDir::new().unwrap();
        create_db(
            dir.path(),
            "24",
            &[
                row(".example.com", "a", None).last_used(NOW - 3600),
                row("other.com", "b", Some(NOW - 1)).last_used(NOW - 5),
            ],
        );
        let store = ChromiumStore::of(dir.path()).unwrap();
        assert_eq!(store.last_active().unwrap(), Some(NOW - 5));
    }

    #[test]
    fn a_value_copied_from_another_site_fails_the_whole_read() {
        let dir = PrivateDir::new().unwrap();
        // The value was encrypted for ".example.com" but is stored under "www.example.com".
        create_db(dir.path(), "24", &[row("www.example.com", "session", None)]);
        let store = ChromiumStore::of(dir.path()).unwrap();
        let domains = candidate_domains("www.example.com");
        let err = read(&store, &domains, &key(), NOW).unwrap_err();
        assert!(matches!(err, Error::HostHashMismatch { cookie_name } if cookie_name == "session"));
    }

    #[test]
    fn app_bound_values_of_the_site_are_found_without_decrypting() {
        let dir = PrivateDir::new().unwrap();
        create_db(
            dir.path(),
            "24",
            &[
                row(".example.com", "plain", None),
                row(".other.com", "bound", None).value(b"v20sealed"),
                row(".example.com", "old", Some(NOW - 1)).value(b"v20sealed"),
            ],
        );
        let store = ChromiumStore::of(dir.path()).unwrap();
        let bound = |host| uses_app_bound_encryption(&store, &candidate_domains(host), NOW);
        assert!(!bound("example.com").unwrap());
        assert!(bound("other.com").unwrap());
    }

    #[test]
    fn the_network_folder_is_preferred() {
        let profile = PrivateDir::new().unwrap();
        assert!(!ChromiumStore::has_database(profile.path()));
        assert_eq!(
            ChromiumSchema::database_path(profile.path()),
            profile.path().join("Cookies")
        );
        let network = profile.path().join("Network");
        std::fs::create_dir(&network).unwrap();
        create_db(&network, "24", &[row(".example.com", "a", None)]);
        assert!(ChromiumStore::has_database(profile.path()));
        assert_eq!(
            ChromiumSchema::database_path(profile.path()),
            network.join("Cookies")
        );
        let store = ChromiumStore::of(profile.path()).unwrap();
        let domains = candidate_domains("example.com");
        assert!(store.last_used_for(&domains, NOW).unwrap().is_some());
    }

    #[test]
    fn unknown_database_versions_are_refused() {
        let dir = PrivateDir::new().unwrap();
        create_db(dir.path(), "25", &[]);
        assert!(matches!(
            ChromiumStore::of(dir.path()),
            Err(Error::UnsupportedDatabaseVersion(25))
        ));
        let dir = PrivateDir::new().unwrap();
        create_db(dir.path(), "next", &[]);
        assert!(matches!(
            ChromiumStore::of(dir.path()),
            Err(Error::DatabaseQuery(reason)) if reason.contains("next")
        ));
    }
}
