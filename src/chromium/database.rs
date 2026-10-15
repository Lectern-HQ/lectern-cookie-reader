//! Reads cookie rows from a copy of a Chromium cookie database. The browser's own file is
//! only ever copied, never opened, so a running browser is not disturbed.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::chromium::crypto::{self, ChromiumKey};
use crate::chromium::{chromium_to_unix, unix_to_chromium};
use crate::database_copy::{DatabaseCopy, placeholders, query_error, time_and_domains};
use crate::files::unreadable;
use crate::{Cookie, Error, Result};

/// The cookie database's file name in a Chromium profile folder.
const DATABASE_FILE: &str = "Cookies";
/// Since Chrome 96 on Windows the database is in this subfolder of the profile.
const NETWORK_FOLDER: &str = "Network";

/// The cookie database of the Chromium profile in `profile_dir`: `Network\Cookies` where
/// it exists (Windows), else `Cookies`.
fn database_path(profile_dir: &Path) -> PathBuf {
    let network = profile_dir.join(NETWORK_FOLDER).join(DATABASE_FILE);
    if network.is_file() {
        network
    } else {
        profile_dir.join(DATABASE_FILE)
    }
}

/// Whether `profile_dir` holds a Chromium cookie database.
pub(crate) fn has_database(profile_dir: &Path) -> bool {
    database_path(profile_dir).is_file()
}

/// A profile's cookie database, copied into a private folder and opened. Dropping it
/// closes the database, then deletes the copy with its folder.
pub(crate) struct ProfileCopy {
    /// The opened copy. Declared first, so it is closed before the copy is deleted.
    db: CookieDb,
    /// The copy in its private folder.
    _copy: DatabaseCopy,
}

impl ProfileCopy {
    /// Copies the cookie database of the profile in `profile_dir` and opens the copy.
    pub(crate) fn of(profile_dir: &Path) -> Result<Self> {
        let copy = DatabaseCopy::of(&database_path(profile_dir))?;
        let db = CookieDb::open(copy.path())?;
        Ok(Self { db, _copy: copy })
    }

    /// The opened copy.
    pub(crate) fn db(&self) -> &CookieDb {
        &self.db
    }
}

/// An opened database copy whose version this crate knows.
pub(crate) struct CookieDb {
    /// The open connection to the copy.
    conn: Connection,
    /// The database version, checked when opened.
    version: i64,
}

impl CookieDb {
    /// Opens a database copy and refuses versions this crate has not been checked against.
    pub(crate) fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| unreadable(path, e))?;
        let version: String = conn
            .query_row("SELECT value FROM meta WHERE key = 'version'", [], |row| {
                row.get(0)
            })
            .map_err(query_error)?;
        let version = version
            .parse()
            .map_err(|_| Error::DatabaseQuery(format!("version {version:?} is not a number")))?;
        crypto::check_db_version(version)?;
        Ok(Self { conn, version })
    }

    /// When the browser last sent an unexpired cookie stored under one of `domains`, as Unix
    /// seconds, or `None` if there is no such cookie. Nothing is decrypted, so this can run
    /// before asking the operating system for the browser's key.
    pub(crate) fn last_used_for(&self, domains: &[String], now_unix: i64) -> Result<Option<i64>> {
        let sql = format!(
            "SELECT MAX(last_access_utc) FROM cookies WHERE {}",
            matching_rows(domains.len())
        );
        let now = unix_to_chromium(now_unix);
        let last_access: Option<i64> = self
            .conn
            .query_row(&sql, time_and_domains(&now, domains).as_slice(), |row| {
                row.get(0)
            })
            .map_err(query_error)?;
        Ok(last_access.map(chromium_to_unix))
    }

    /// Whether an unexpired cookie stored under one of `domains` uses app-bound encryption.
    /// Nothing is decrypted, so this can run before asking for the browser's key.
    pub(crate) fn uses_app_bound_encryption(
        &self,
        domains: &[String],
        now_unix: i64,
    ) -> Result<bool> {
        let sql = format!(
            "SELECT EXISTS(SELECT 1 FROM cookies WHERE {} AND substr(encrypted_value, 1, {}) = ?{})",
            matching_rows(domains.len()),
            crypto::APP_BOUND_PREFIX.len(),
            domains.len() + 2
        );
        let now = unix_to_chromium(now_unix);
        let mut params = time_and_domains(&now, domains);
        params.push(&crypto::APP_BOUND_PREFIX);
        self.conn
            .query_row(&sql, params.as_slice(), |row| row.get(0))
            .map_err(query_error)
    }

    /// When the browser last sent any cookie, for any site, as Unix seconds: a sign of how
    /// recently the profile was used. `None` for a profile without cookies.
    pub(crate) fn last_active(&self) -> Result<Option<i64>> {
        let last_access: Option<i64> = self
            .conn
            .query_row("SELECT MAX(last_access_utc) FROM cookies", [], |row| {
                row.get(0)
            })
            .map_err(query_error)?;
        Ok(last_access.map(chromium_to_unix))
    }

    /// Decrypts the unexpired cookies stored under one of `domains`.
    ///
    /// If one value cannot be decrypted, the whole read fails with that cookie's error.
    /// A login with a cookie missing fails later in ways that are hard to explain, and a
    /// value stored for another site must never be used.
    pub(crate) fn read(
        &self,
        domains: &[String],
        key: &ChromiumKey,
        now_unix: i64,
    ) -> Result<Vec<Cookie>> {
        let sql = format!(
            "SELECT host_key, name, path, is_secure, is_httponly, has_expires, expires_utc,
                    encrypted_value
             FROM cookies WHERE {}",
            matching_rows(domains.len())
        );
        let now = unix_to_chromium(now_unix);
        let mut statement = self.conn.prepare(&sql).map_err(query_error)?;
        let mut rows = statement
            .query(time_and_domains(&now, domains).as_slice())
            .map_err(query_error)?;
        let mut cookies = Vec::new();
        while let Some(row) = rows.next().map_err(query_error)? {
            let domain: String = row.get(0).map_err(query_error)?;
            let name: String = row.get(1).map_err(query_error)?;
            let has_expiry: bool = row.get(5).map_err(query_error)?;
            let expiry: i64 = row.get(6).map_err(query_error)?;
            let encrypted: Vec<u8> = row.get(7).map_err(query_error)?;
            let value = crypto::decrypt_value(&encrypted, key, &domain, &name, self.version)?;
            cookies.push(Cookie {
                path: row.get(2).map_err(query_error)?,
                secure: row.get(3).map_err(query_error)?,
                http_only: row.get(4).map_err(query_error)?,
                expires: has_expiry.then(|| chromium_to_unix(expiry)),
                domain,
                name,
                value,
            });
        }
        Ok(cookies)
    }
}

/// SQL condition for unexpired rows under one of `domain_count` domains. Parameter `?1` is
/// the current time, `?2` onwards are the domains. Values are always passed as parameters,
/// never written into the SQL text.
fn matching_rows(domain_count: usize) -> String {
    format!(
        "host_key IN ({}) AND (has_expires = 0 OR expires_utc > ?1)",
        placeholders(2, domain_count)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chromium::test_vectors::{NOW, create_db, key, row};
    use crate::domain::candidate_domains;
    use crate::private_dir::PrivateDir;

    #[test]
    fn reads_only_unexpired_cookies_of_the_requested_site() {
        let dir = PrivateDir::new().unwrap();
        let path = create_db(
            dir.path(),
            "24",
            &[
                row(".example.com", "session", Some(NOW + 60)),
                row(".example.com", "no-expiry", None),
                row(".example.com", "expired", Some(NOW - 60)),
                row("other.com", "foreign", Some(NOW + 60)),
            ],
        );
        let db = CookieDb::open(&path).unwrap();
        let domains = candidate_domains("www.example.com");

        assert_eq!(db.last_used_for(&domains, NOW).unwrap(), Some(NOW - 10));
        let mut cookies = db.read(&domains, &key(), NOW).unwrap();
        cookies.sort_by(|a, b| a.name.cmp(&b.name));
        let names: Vec<_> = cookies.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["no-expiry", "session"]);
        assert_eq!(cookies[1].value.expose(), "hello-world");
        assert_eq!(cookies[1].expires, Some(NOW + 60));
        assert_eq!(cookies[0].expires, None);
    }

    #[test]
    fn last_activity_counts_every_site() {
        let dir = PrivateDir::new().unwrap();
        let path = create_db(
            dir.path(),
            "24",
            &[
                row(".example.com", "a", None).last_used(NOW - 3600),
                row("other.com", "b", Some(NOW - 1)).last_used(NOW - 5),
            ],
        );
        assert_eq!(
            CookieDb::open(&path).unwrap().last_active().unwrap(),
            Some(NOW - 5)
        );
        let empty = PrivateDir::new().unwrap();
        let path = create_db(empty.path(), "24", &[]);
        assert_eq!(CookieDb::open(&path).unwrap().last_active().unwrap(), None);
    }

    #[test]
    fn a_site_without_cookies_is_reported_before_any_decryption() {
        let dir = PrivateDir::new().unwrap();
        let path = create_db(dir.path(), "24", &[row("other.com", "x", None)]);
        let db = CookieDb::open(&path).unwrap();
        assert!(
            db.last_used_for(&candidate_domains("example.com"), NOW)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn a_value_copied_from_another_site_fails_the_whole_read() {
        let dir = PrivateDir::new().unwrap();
        // The value was encrypted for ".example.com" but is stored under "www.example.com".
        let path = create_db(dir.path(), "24", &[row("www.example.com", "session", None)]);
        let db = CookieDb::open(&path).unwrap();
        let err = db
            .read(&candidate_domains("www.example.com"), &key(), NOW)
            .unwrap_err();
        assert!(matches!(err, Error::HostHashMismatch { cookie_name } if cookie_name == "session"));
    }

    #[test]
    fn app_bound_values_of_the_site_are_found_without_decrypting() {
        let dir = PrivateDir::new().unwrap();
        let path = create_db(
            dir.path(),
            "24",
            &[
                row(".example.com", "plain", None),
                row(".other.com", "bound", None).value(b"v20sealed"),
                row(".example.com", "old", Some(NOW - 1)).value(b"v20sealed"),
            ],
        );
        let db = CookieDb::open(&path).unwrap();
        assert!(
            !db.uses_app_bound_encryption(&candidate_domains("example.com"), NOW)
                .unwrap()
        );
        assert!(
            db.uses_app_bound_encryption(&candidate_domains("other.com"), NOW)
                .unwrap()
        );
    }

    #[test]
    fn domains_are_parameters_not_sql() {
        let dir = PrivateDir::new().unwrap();
        let path = create_db(dir.path(), "24", &[row("example.com", "x", None)]);
        let db = CookieDb::open(&path).unwrap();
        let hostile = vec!["x' OR '1'='1".to_owned()];
        assert!(db.last_used_for(&hostile, NOW).unwrap().is_none());
    }

    #[test]
    fn the_network_folder_is_preferred() {
        let profile = PrivateDir::new().unwrap();
        assert!(!has_database(profile.path()));
        assert_eq!(
            database_path(profile.path()),
            profile.path().join("Cookies")
        );
        let network = profile.path().join("Network");
        std::fs::create_dir(&network).unwrap();
        create_db(&network, "24", &[row(".example.com", "a", None)]);
        assert!(has_database(profile.path()));
        assert_eq!(database_path(profile.path()), network.join("Cookies"));
        let copy = ProfileCopy::of(profile.path()).unwrap();
        assert!(
            copy.db()
                .last_used_for(&candidate_domains("example.com"), NOW)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn unknown_database_versions_are_refused() {
        let dir = PrivateDir::new().unwrap();
        let path = create_db(dir.path(), "25", &[]);
        assert!(matches!(
            CookieDb::open(&path),
            Err(Error::UnsupportedDatabaseVersion(25))
        ));
    }
}
