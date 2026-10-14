//! Reads cookie rows from a copy of a Chromium cookie database. The browser's own file is
//! only ever copied, never opened, so a running browser is not disturbed.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, ToSql};

use crate::chromium::{chromium_to_unix, crypto, unix_to_chromium};
use crate::private_dir::PrivateDir;
use crate::{Cookie, Error, Result};

/// The cookie database's file name in a Chromium profile folder.
const DATABASE_FILE: &str = "Cookies";
/// SQLite keeps recent changes in these files next to the database until it merges them.
/// Without them the copy could miss a login that just happened.
const SIDECAR_SUFFIXES: &[&str] = &["-wal", "-journal"];

/// Whether `profile_dir` holds a Chromium cookie database.
pub(crate) fn has_database(profile_dir: &Path) -> bool {
    profile_dir.join(DATABASE_FILE).is_file()
}

/// Copies the cookie database of a profile, with its SQLite sidecar files, into `folder`
/// and returns the path of the copy.
pub(crate) fn copy_database(profile_dir: &Path, folder: &Path) -> Result<PathBuf> {
    let source = profile_dir.join(DATABASE_FILE);
    if !has_database(profile_dir) {
        return Err(Error::DatabaseNotFound(source));
    }
    copy_file(&source, &folder.join(DATABASE_FILE))?;
    for suffix in SIDECAR_SUFFIXES {
        let name = format!("{DATABASE_FILE}{suffix}");
        let sidecar = profile_dir.join(&name);
        if sidecar.is_file() {
            copy_file(&sidecar, &folder.join(name))?;
        }
    }
    Ok(folder.join(DATABASE_FILE))
}

/// Copies one file, reporting the source path if it fails.
fn copy_file(from: &Path, to: &Path) -> Result<()> {
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| unreadable(from, e))
}

/// The error for a database file that could not be copied or opened.
fn unreadable(path: &Path, reason: impl std::fmt::Display) -> Error {
    Error::DatabaseUnreadable {
        path: path.to_owned(),
        reason: reason.to_string(),
    }
}

/// A profile's cookie database, copied into a private folder and opened. Dropping it
/// closes the database, then deletes the copy with its folder.
pub(crate) struct ProfileCopy {
    /// The opened copy. Declared first, so it is closed before the folder is deleted.
    db: CookieDb,
    /// The private folder holding the copy.
    _folder: PrivateDir,
}

impl ProfileCopy {
    /// Copies the cookie database of the profile in `profile_dir` and opens the copy.
    pub(crate) fn of(profile_dir: &Path) -> Result<Self> {
        let folder = PrivateDir::new()?;
        let db = CookieDb::open(&copy_database(profile_dir, folder.path())?)?;
        Ok(Self {
            db,
            _folder: folder,
        })
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
            .query_row(&sql, query_params(&now, domains).as_slice(), |row| {
                row.get(0)
            })
            .map_err(query_error)?;
        Ok(last_access.map(chromium_to_unix))
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
        key: &[u8; 16],
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
            .query(query_params(&now, domains).as_slice())
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
    let placeholders = (2..domain_count + 2)
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("host_key IN ({placeholders}) AND (has_expires = 0 OR expires_utc > ?1)")
}

/// The query's parameters in order: the current time as `?1`, then each domain.
fn query_params<'a>(now: &'a i64, domains: &'a [String]) -> Vec<&'a dyn ToSql> {
    let mut params: Vec<&dyn ToSql> = vec![now];
    params.extend(domains.iter().map(|d| d as &dyn ToSql));
    params
}

/// A database error as this crate's error, with SQLite's reason.
fn query_error(error: rusqlite::Error) -> Error {
    Error::DatabaseQuery(error.to_string())
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
    fn domains_are_parameters_not_sql() {
        let dir = PrivateDir::new().unwrap();
        let path = create_db(dir.path(), "24", &[row("example.com", "x", None)]);
        let db = CookieDb::open(&path).unwrap();
        let hostile = vec!["x' OR '1'='1".to_owned()];
        assert!(db.last_used_for(&hostile, NOW).unwrap().is_none());
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

    #[test]
    fn copy_includes_the_sidecar_files() {
        let profile = PrivateDir::new().unwrap();
        let copy = PrivateDir::new().unwrap();
        for name in ["Cookies", "Cookies-wal", "Cookies-journal"] {
            fs::write(profile.path().join(name), name).unwrap();
        }
        let copied = copy_database(profile.path(), copy.path()).unwrap();
        assert_eq!(copied, copy.path().join("Cookies"));
        for name in ["Cookies", "Cookies-wal", "Cookies-journal"] {
            assert_eq!(fs::read_to_string(copy.path().join(name)).unwrap(), name);
        }
    }

    #[test]
    fn missing_database_is_its_own_error() {
        let profile = PrivateDir::new().unwrap();
        let copy = PrivateDir::new().unwrap();
        assert!(matches!(
            copy_database(profile.path(), copy.path()),
            Err(Error::DatabaseNotFound(_))
        ));
    }
}
