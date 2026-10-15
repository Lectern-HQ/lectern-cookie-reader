//! The Firefox family cookie database (`cookies.sqlite`): its schema for the shared SQLite
//! store, and reading cookies from its rows. Values are stored without encryption, so no
//! key and no permission dialog are needed.
//!
//! Only cookies of the normal browsing context are read: `originAttributes` is empty.
//! Cookies of Multi-Account Containers (`^userContextId=2`) and partitioned third-party
//! cookies (`^partitionKey=...`) are not the ones the browser sends when the user opens
//! the site in a normal tab, so they are left out.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::clock::{MICROS_PER_SEC, MILLIS_PER_SEC};
use crate::sqlite_store::{Query, Schema, Store, query_error};
use crate::{Cookie, Error, Result, SecretValue};

/// The cookie database's file name in a Firefox profile folder.
const DATABASE_FILE: &str = "cookies.sqlite";
/// Oldest schema version this crate has been checked against. Every version from here on
/// has the columns this crate reads.
const MIN_SCHEMA_VERSION: i64 = 12;
/// Newest schema version this crate has been checked against. Newer versions are refused
/// instead of risking wrong values. Version 17 only added the `updateTime` column.
const MAX_SCHEMA_VERSION: i64 = 17;
/// From this version on (Firefox 142), `expiry` is in milliseconds instead of seconds.
const MILLISECOND_EXPIRY_VERSION: i64 = 16;
/// The columns [`read`] builds a cookie from, in the order it reads them.
const COOKIE_COLUMNS: &str = "host, name, value, path, isSecure, isHttpOnly, expiry";

/// The schema of a Firefox family cookie database.
pub(crate) struct FirefoxSchema;

/// A copied and opened Firefox family cookie database.
pub(crate) type FirefoxStore = Store<FirefoxSchema>;

impl Schema for FirefoxSchema {
    const TABLE: &'static str = "moz_cookies";
    const HOST: &'static str = "host";
    const LAST_ACCESS: &'static str = "lastAccessed";
    const SENDABLE: &'static str = "expiry > ?1 AND originAttributes = ''";

    fn database_path(profile_dir: &Path) -> PathBuf {
        profile_dir.join(DATABASE_FILE)
    }

    /// The version in `PRAGMA user_version`.
    fn version(conn: &Connection) -> Result<i64> {
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(query_error)?;
        if (MIN_SCHEMA_VERSION..=MAX_SCHEMA_VERSION).contains(&version) {
            Ok(version)
        } else {
            Err(Error::UnsupportedDatabaseVersion(version))
        }
    }

    /// In the unit of this version's `expiry` column.
    fn now(version: i64, now_unix: i64) -> i64 {
        now_unix.saturating_mul(expiry_units_per_sec(version))
    }

    /// `lastAccessed` is in microseconds since 1970.
    fn last_access_to_unix(stored: i64) -> i64 {
        stored / MICROS_PER_SEC
    }
}

/// How many `expiry` units make a second in schema `version`.
fn expiry_units_per_sec(version: i64) -> i64 {
    if version >= MILLISECOND_EXPIRY_VERSION {
        MILLIS_PER_SEC
    } else {
        1
    }
}

/// The sendable cookies stored under one of `domains`.
pub(crate) fn read(store: &FirefoxStore, domains: &[String], now_unix: i64) -> Result<Vec<Cookie>> {
    let per_sec = expiry_units_per_sec(store.version());
    let query = Query {
        columns: COOKIE_COLUMNS,
        domains,
        now_unix,
        filter: None,
    };
    store.rows(&query, |row| {
        Ok(Cookie {
            domain: row.get(0)?,
            name: row.get(1)?,
            value: SecretValue::new(row.get(2)?),
            path: row.get(3)?,
            secure: row.get(4)?,
            http_only: row.get(5)?,
            expires: Some(row.get::<_, i64>(6)? / per_sec),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::candidate_domains;
    use crate::firefox::test_data::{NOW, create_db, row};
    use crate::private_dir::PrivateDir;

    /// Reads `host`'s cookies from a database of `version` with `rows`, sorted by name.
    fn read_from(version: i64, rows: &[crate::firefox::test_data::Row], host: &str) -> Vec<Cookie> {
        let dir = PrivateDir::new().unwrap();
        create_db(dir.path(), version, rows);
        let store = FirefoxStore::of(dir.path()).unwrap();
        let mut cookies = read(&store, &candidate_domains(host), NOW).unwrap();
        cookies.sort_by(|a, b| a.name.cmp(&b.name));
        cookies
    }

    #[test]
    fn reads_only_sendable_cookies_of_the_requested_site() {
        let cookies = read_from(
            16,
            &[
                row(".example.com", "session", NOW + 60),
                row("www.example.com", "host-only", NOW + 120),
                row(".example.com", "expired", NOW - 60),
                row("other.com", "foreign", NOW + 60),
            ],
            "www.example.com",
        );
        let names: Vec<_> = cookies.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["host-only", "session"]);
        assert_eq!(cookies[1].value.expose(), "value-of-session");
        assert_eq!(cookies[1].expires, Some(NOW + 60));
        assert_eq!(cookies[1].domain, ".example.com");
        assert!(cookies[1].secure && cookies[1].http_only);
    }

    #[test]
    fn expiry_is_in_seconds_before_schema_version_16_and_milliseconds_after() {
        for version in [15, 16, 17] {
            let cookies = read_from(
                version,
                &[
                    row(".example.com", "valid", NOW + 60),
                    row(".example.com", "expired", NOW - 60),
                ],
                "example.com",
            );
            assert_eq!(cookies.len(), 1, "version {version}");
            assert_eq!(cookies[0].expires, Some(NOW + 60), "version {version}");
        }
    }

    #[test]
    fn container_and_partitioned_cookies_are_left_out() {
        let dir = PrivateDir::new().unwrap();
        create_db(
            dir.path(),
            16,
            &[
                row(".example.com", "container", NOW + 60).origin("^userContextId=2"),
                row(".example.com", "partitioned", NOW + 60)
                    .origin("^partitionKey=%28https%2Cother.com%29"),
            ],
        );
        let store = FirefoxStore::of(dir.path()).unwrap();
        let domains = candidate_domains("example.com");
        assert!(store.last_used_for(&domains, NOW).unwrap().is_none());
        assert!(read(&store, &domains, NOW).unwrap().is_empty());
    }

    #[test]
    fn times_are_read_in_seconds() {
        let dir = PrivateDir::new().unwrap();
        create_db(
            dir.path(),
            16,
            &[
                row(".example.com", "a", NOW + 60).last_used(NOW - 3600),
                row("other.com", "b", NOW - 1).last_used(NOW - 5),
            ],
        );
        let store = FirefoxStore::of(dir.path()).unwrap();
        let domains = candidate_domains("example.com");
        assert_eq!(
            store.last_used_for(&domains, NOW).unwrap(),
            Some(NOW - 3600)
        );
        assert_eq!(store.last_active().unwrap(), Some(NOW - 5));
    }

    #[test]
    fn unknown_schema_versions_are_refused() {
        for version in [11, 18] {
            let dir = PrivateDir::new().unwrap();
            create_db(dir.path(), version, &[]);
            assert!(matches!(
                FirefoxStore::of(dir.path()),
                Err(Error::UnsupportedDatabaseVersion(v)) if v == version
            ));
        }
    }
}
