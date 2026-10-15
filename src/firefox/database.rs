//! Reads cookie rows from a copy of a Firefox family cookie database (`cookies.sqlite`).
//! Values are stored without encryption, so no key and no permission dialog are needed.
//!
//! Only cookies of the normal browsing context are read: `originAttributes` is empty.
//! Cookies of Multi-Account Containers (`^userContextId=2`) and partitioned third-party
//! cookies (`^partitionKey=...`) are not the ones the browser sends when the user opens
//! the site in a normal tab, so they are left out.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::database_copy::{DatabaseCopy, placeholders, query_error, time_and_domains};
use crate::files::unreadable;
use crate::{Cookie, Error, Result, SecretValue};

/// The cookie database's file name in a Firefox profile folder.
const DATABASE_FILE: &str = "cookies.sqlite";
/// Oldest schema version this crate has been checked against. Every version from here on
/// has the columns this crate reads.
pub(crate) const MIN_SCHEMA_VERSION: i64 = 12;
/// Newest schema version this crate has been checked against. Newer versions are refused
/// instead of risking wrong values. Version 17 only added the `updateTime` column.
pub(crate) const MAX_SCHEMA_VERSION: i64 = 17;
/// From this version on (Firefox 142), `expiry` is in milliseconds instead of seconds.
const MILLISECOND_EXPIRY_VERSION: i64 = 16;
/// `lastAccessed` is in microseconds since 1970.
const MICROS_PER_SEC: i64 = 1_000_000;
/// Milliseconds per second, for `expiry` from schema version 16.
const MILLIS_PER_SEC: i64 = 1_000;

/// The cookie database of the Firefox profile in `profile_dir`.
fn database_path(profile_dir: &Path) -> PathBuf {
    profile_dir.join(DATABASE_FILE)
}

/// Whether `profile_dir` holds a Firefox cookie database.
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

/// An opened database copy whose schema version this crate knows.
pub(crate) struct CookieDb {
    /// The open connection to the copy.
    conn: Connection,
    /// The schema version, checked when opened.
    version: i64,
}

impl CookieDb {
    /// Opens a database copy and refuses versions this crate has not been checked against.
    pub(crate) fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| unreadable(path, e))?;
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(query_error)?;
        if !(MIN_SCHEMA_VERSION..=MAX_SCHEMA_VERSION).contains(&version) {
            return Err(Error::UnsupportedDatabaseVersion(version));
        }
        Ok(Self { conn, version })
    }

    /// When the browser last sent an unexpired cookie stored under one of `domains`, as Unix
    /// seconds, or `None` if there is no such cookie.
    pub(crate) fn last_used_for(&self, domains: &[String], now_unix: i64) -> Result<Option<i64>> {
        let sql = format!(
            "SELECT MAX(lastAccessed) FROM moz_cookies WHERE {}",
            matching_rows(domains.len())
        );
        let now = self.expiry_from_unix(now_unix);
        let last_access: Option<i64> = self
            .conn
            .query_row(&sql, time_and_domains(&now, domains).as_slice(), |row| {
                row.get(0)
            })
            .map_err(query_error)?;
        Ok(last_access.map(|micros| micros / MICROS_PER_SEC))
    }

    /// When the browser last sent any cookie, for any site, as Unix seconds: a sign of how
    /// recently the profile was used. `None` for a profile without cookies.
    pub(crate) fn last_active(&self) -> Result<Option<i64>> {
        let last_access: Option<i64> = self
            .conn
            .query_row("SELECT MAX(lastAccessed) FROM moz_cookies", [], |row| {
                row.get(0)
            })
            .map_err(query_error)?;
        Ok(last_access.map(|micros| micros / MICROS_PER_SEC))
    }

    /// The unexpired cookies stored under one of `domains`.
    pub(crate) fn read(&self, domains: &[String], now_unix: i64) -> Result<Vec<Cookie>> {
        let sql = format!(
            "SELECT host, name, value, path, isSecure, isHttpOnly, expiry
             FROM moz_cookies WHERE {}",
            matching_rows(domains.len())
        );
        let now = self.expiry_from_unix(now_unix);
        let mut statement = self.conn.prepare(&sql).map_err(query_error)?;
        let mut rows = statement
            .query(time_and_domains(&now, domains).as_slice())
            .map_err(query_error)?;
        let mut cookies = Vec::new();
        while let Some(row) = rows.next().map_err(query_error)? {
            let expiry: i64 = row.get(6).map_err(query_error)?;
            cookies.push(Cookie {
                domain: row.get(0).map_err(query_error)?,
                name: row.get(1).map_err(query_error)?,
                value: SecretValue::new(row.get(2).map_err(query_error)?),
                path: row.get(3).map_err(query_error)?,
                secure: row.get(4).map_err(query_error)?,
                http_only: row.get(5).map_err(query_error)?,
                expires: Some(self.expiry_to_unix(expiry)),
            });
        }
        Ok(cookies)
    }

    /// Unix seconds in the unit of this version's `expiry` column.
    fn expiry_from_unix(&self, unix_secs: i64) -> i64 {
        if self.version >= MILLISECOND_EXPIRY_VERSION {
            unix_secs.saturating_mul(MILLIS_PER_SEC)
        } else {
            unix_secs
        }
    }

    /// This version's `expiry` value as Unix seconds.
    fn expiry_to_unix(&self, expiry: i64) -> i64 {
        if self.version >= MILLISECOND_EXPIRY_VERSION {
            expiry / MILLIS_PER_SEC
        } else {
            expiry
        }
    }
}

/// SQL condition for unexpired rows of the normal browsing context under one of
/// `domain_count` domains. Parameter `?1` is the current time in the `expiry` column's
/// unit, `?2` onwards are the domains.
fn matching_rows(domain_count: usize) -> String {
    format!(
        "host IN ({}) AND expiry > ?1 AND originAttributes = ''",
        placeholders(2, domain_count)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::candidate_domains;
    use crate::firefox::test_data::{NOW, create_db, row};
    use crate::private_dir::PrivateDir;

    #[test]
    fn reads_only_unexpired_cookies_of_the_requested_site() {
        let dir = PrivateDir::new().unwrap();
        let path = create_db(
            dir.path(),
            16,
            &[
                row(".example.com", "session", NOW + 60),
                row("www.example.com", "host-only", NOW + 120),
                row(".example.com", "expired", NOW - 60),
                row("other.com", "foreign", NOW + 60),
            ],
        );
        let db = CookieDb::open(&path).unwrap();
        let domains = candidate_domains("www.example.com");

        assert_eq!(db.last_used_for(&domains, NOW).unwrap(), Some(NOW - 10));
        let mut cookies = db.read(&domains, NOW).unwrap();
        cookies.sort_by(|a, b| a.name.cmp(&b.name));
        let names: Vec<_> = cookies.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["host-only", "session"]);
        assert_eq!(cookies[1].value.expose(), "value-of-session");
        assert_eq!(cookies[1].expires, Some(NOW + 60));
        assert_eq!(cookies[1].domain, ".example.com");
        assert!(cookies[1].secure && cookies[1].http_only);
    }

    #[test]
    fn schema_version_17_reads_like_16() {
        let dir = PrivateDir::new().unwrap();
        let path = create_db(dir.path(), 17, &[row(".example.com", "a", NOW + 60)]);
        let cookies = CookieDb::open(&path)
            .unwrap()
            .read(&candidate_domains("example.com"), NOW)
            .unwrap();
        assert_eq!(cookies[0].expires, Some(NOW + 60));
    }

    #[test]
    fn expiry_is_in_seconds_before_schema_version_16() {
        let dir = PrivateDir::new().unwrap();
        let path = create_db(
            dir.path(),
            15,
            &[
                row(".example.com", "valid", NOW + 60),
                row(".example.com", "expired", NOW - 60),
            ],
        );
        let cookies = CookieDb::open(&path)
            .unwrap()
            .read(&candidate_domains("example.com"), NOW)
            .unwrap();
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0].expires, Some(NOW + 60));
    }

    #[test]
    fn container_and_partitioned_cookies_are_left_out() {
        let dir = PrivateDir::new().unwrap();
        let path = create_db(
            dir.path(),
            16,
            &[
                row(".example.com", "container", NOW + 60).origin("^userContextId=2"),
                row(".example.com", "partitioned", NOW + 60)
                    .origin("^partitionKey=%28https%2Cother.com%29"),
            ],
        );
        let db = CookieDb::open(&path).unwrap();
        let domains = candidate_domains("example.com");
        assert!(db.last_used_for(&domains, NOW).unwrap().is_none());
        assert!(db.read(&domains, NOW).unwrap().is_empty());
    }

    #[test]
    fn last_activity_counts_every_site() {
        let dir = PrivateDir::new().unwrap();
        let path = create_db(
            dir.path(),
            16,
            &[
                row(".example.com", "a", NOW + 60).last_used(NOW - 3600),
                row("other.com", "b", NOW - 1).last_used(NOW - 5),
            ],
        );
        assert_eq!(
            CookieDb::open(&path).unwrap().last_active().unwrap(),
            Some(NOW - 5)
        );
        let empty = PrivateDir::new().unwrap();
        let path = create_db(empty.path(), 16, &[]);
        assert_eq!(CookieDb::open(&path).unwrap().last_active().unwrap(), None);
    }

    #[test]
    fn domains_are_parameters_not_sql() {
        let dir = PrivateDir::new().unwrap();
        let path = create_db(dir.path(), 16, &[row("example.com", "x", NOW + 60)]);
        let db = CookieDb::open(&path).unwrap();
        let hostile = vec!["x' OR '1'='1".to_owned()];
        assert!(db.last_used_for(&hostile, NOW).unwrap().is_none());
    }

    #[test]
    fn unknown_schema_versions_are_refused() {
        for version in [11, 18] {
            let dir = PrivateDir::new().unwrap();
            let path = create_db(dir.path(), version, &[]);
            assert!(matches!(
                CookieDb::open(&path),
                Err(Error::UnsupportedDatabaseVersion(v)) if v == version
            ));
        }
    }

    #[test]
    fn a_profile_copy_reads_the_profiles_database() {
        let dir = PrivateDir::new().unwrap();
        assert!(!has_database(dir.path()));
        create_db(dir.path(), 16, &[row(".example.com", "a", NOW + 60)]);
        assert!(has_database(dir.path()));
        let copy = ProfileCopy::of(dir.path()).unwrap();
        let cookies = copy
            .db()
            .read(&candidate_domains("example.com"), NOW)
            .unwrap();
        assert_eq!(cookies.len(), 1);
    }
}
