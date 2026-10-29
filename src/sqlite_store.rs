//! The SQLite cookie databases of Chromium browsers and the Firefox family, behind one
//! store: it copies a profile's database, opens the copy, checks its version, and queries
//! the rows of one site. Each engine only describes its [`Schema`] and turns its rows into
//! cookies.

use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, Row, ToSql};

use crate::Result;
use crate::database_copy::DatabaseCopy;
use crate::files::unreadable;

/// What differs between the engines' SQLite cookie databases.
pub(crate) trait Schema {
    /// The table that holds the cookies.
    const TABLE: &'static str;
    /// The column with the cookie's domain as stored, for example `.example.com`.
    const HOST: &'static str;
    /// The column with the time the browser last sent the cookie.
    const LAST_ACCESS: &'static str;
    /// SQL condition for the rows the browser would send: in the normal browsing context,
    /// the one row of its cookie that counts where a cookie can be stored twice, and
    /// unexpired. `?1` is the current time from [`Schema::now`].
    const SENDABLE: &'static str;

    /// The cookie database in the profile folder `profile_dir`.
    fn database_path(profile_dir: &Path) -> PathBuf;
    /// The database's version. Fails for a version this crate has not been checked against.
    fn version(conn: &Connection) -> Result<i64>;
    /// `now_unix` in the unit [`Schema::SENDABLE`] compares with, for database `version`.
    fn now(version: i64, now_unix: i64) -> i64;
    /// A [`Schema::LAST_ACCESS`] value as Unix seconds.
    fn last_access_to_unix(stored: i64) -> i64;
}

/// One more condition on the rows of a [`Query`], with its one value. Its SQL uses a bare
/// `?` for the value: it comes last in the query text, so SQLite numbers it after the
/// domains.
pub(crate) struct Filter<'a> {
    /// The condition, for example `substr(encrypted_value, 1, 3) = ?`.
    pub(crate) condition: &'a str,
    /// The value for its `?`.
    pub(crate) value: &'a dyn ToSql,
}

/// Which columns to read from the sendable rows of the site's domains.
pub(crate) struct Query<'a> {
    /// The columns or aggregate to select, for example `MAX(last_access_utc)`.
    pub(crate) columns: &'a str,
    /// The stored domains that apply to the site.
    pub(crate) domains: &'a [String],
    /// The current time, as Unix seconds.
    pub(crate) now_unix: i64,
    /// One more condition, or `None`.
    pub(crate) filter: Option<Filter<'a>>,
}

/// A profile's cookie database, copied into a private folder and opened. Dropping it
/// closes the database, then deletes the copy with its folder.
pub(crate) struct Store<S> {
    /// The open connection to the copy. Declared before the copy, so it is closed first:
    /// Windows cannot delete a file that is still open.
    conn: Connection,
    /// The database version, checked when opened.
    version: i64,
    /// The copy in its private folder.
    _copy: DatabaseCopy,
    /// The schema the database follows.
    _schema: PhantomData<S>,
}

impl<S: Schema> Store<S> {
    /// Whether `profile_dir` holds a cookie database of this schema.
    pub(crate) fn has_database(profile_dir: &Path) -> bool {
        S::database_path(profile_dir).is_file()
    }

    /// Copies the cookie database of the profile in `profile_dir`, opens the copy and
    /// checks its version.
    pub(crate) fn of(profile_dir: &Path) -> Result<Self> {
        let copy = DatabaseCopy::of(&S::database_path(profile_dir))?;
        let conn = Connection::open(copy.path()).map_err(|e| unreadable(copy.path(), e))?;
        let version = S::version(&conn)?;
        Ok(Self {
            conn,
            version,
            _copy: copy,
            _schema: PhantomData,
        })
    }

    /// The database version, checked when opened.
    pub(crate) fn version(&self) -> i64 {
        self.version
    }

    /// When the browser last sent a sendable cookie stored under one of `domains`, as
    /// Unix seconds, or `None` if there is no such cookie. Nothing is decrypted.
    pub(crate) fn last_used_for(&self, domains: &[String], now_unix: i64) -> Result<Option<i64>> {
        let columns = format!("MAX({})", S::LAST_ACCESS);
        let query = Query {
            columns: &columns,
            domains,
            now_unix,
            filter: None,
        };
        let max = self.rows(&query, |row| row.get::<_, Option<i64>>(0))?;
        Ok(max.into_iter().flatten().next().map(S::last_access_to_unix))
    }

    /// When the browser last sent any cookie, for any site, as Unix seconds: a sign of how
    /// recently the profile was used. `None` for a profile without cookies.
    pub(crate) fn last_active(&self) -> Result<Option<i64>> {
        let sql = format!("SELECT MAX({}) FROM {}", S::LAST_ACCESS, S::TABLE);
        let max: Option<i64> = self
            .conn
            .query_row(&sql, [], |row| row.get(0))
            .map_err(query_error)?;
        Ok(max.map(S::last_access_to_unix))
    }

    /// The selected columns of every sendable row stored under one of the query's domains,
    /// each turned into a value by `map`. Every value is a parameter, never SQL text.
    pub(crate) fn rows<T>(
        &self,
        query: &Query<'_>,
        mut map: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Vec<T>> {
        let placeholders = (2..query.domains.len() + 2)
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let mut sql = format!(
            "SELECT {} FROM {} WHERE {} IN ({placeholders}) AND {}",
            query.columns,
            S::TABLE,
            S::HOST,
            S::SENDABLE
        );
        let now = S::now(self.version, query.now_unix);
        let mut params: Vec<&dyn ToSql> = vec![&now];
        params.extend(query.domains.iter().map(|d| d as &dyn ToSql));
        if let Some(filter) = &query.filter {
            sql.push_str(" AND ");
            sql.push_str(filter.condition);
            params.push(filter.value);
        }
        let mut statement = self.conn.prepare(&sql).map_err(query_error)?;
        let rows = statement
            .query_map(params.as_slice(), |row| map(row))
            .map_err(query_error)?;
        rows.collect::<rusqlite::Result<Vec<T>>>()
            .map_err(query_error)
    }
}

/// A database error as this crate's error, with SQLite's reason.
pub(crate) fn query_error(error: rusqlite::Error) -> crate::Error {
    crate::Error::DatabaseQuery(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;
    use crate::clock::tests::NOW;
    use crate::private_dir::PrivateDir;

    /// A minimal schema: seconds everywhere, version from `PRAGMA user_version`.
    struct Plain;

    impl Schema for Plain {
        const TABLE: &'static str = "c";
        const HOST: &'static str = "host";
        const LAST_ACCESS: &'static str = "used";
        const SENDABLE: &'static str = "expires > ?1";

        fn database_path(profile_dir: &Path) -> PathBuf {
            profile_dir.join("plain.sqlite")
        }

        fn version(conn: &Connection) -> Result<i64> {
            let version: i64 = conn
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .map_err(query_error)?;
            if version == 1 {
                Ok(version)
            } else {
                Err(Error::UnsupportedDatabaseVersion(version))
            }
        }

        fn now(_version: i64, now_unix: i64) -> i64 {
            now_unix
        }

        fn last_access_to_unix(stored: i64) -> i64 {
            stored
        }
    }

    /// A profile folder with a `Plain` database of `(host, expires, used, tag)` rows.
    fn profile(version: i64, rows: &[(&str, i64, i64, &str)]) -> PrivateDir {
        let dir = PrivateDir::new().unwrap();
        let conn = Connection::open(Plain::database_path(dir.path())).unwrap();
        conn.execute_batch("CREATE TABLE c(host TEXT, expires INTEGER, used INTEGER, tag TEXT)")
            .unwrap();
        conn.pragma_update(None, "user_version", version).unwrap();
        for (host, expires, used, tag) in rows {
            conn.execute(
                "INSERT INTO c VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![host, expires, used, tag],
            )
            .unwrap();
        }
        dir
    }

    fn domains() -> Vec<String> {
        vec!["example.com".into(), ".example.com".into()]
    }

    #[test]
    fn queries_only_the_sendable_rows_of_the_sites_domains() {
        let dir = profile(
            1,
            &[
                (".example.com", NOW + 60, NOW - 30, "a"),
                ("example.com", NOW + 60, NOW - 10, "b"),
                (".example.com", NOW - 1, NOW - 1, "expired"),
                ("other.com", NOW + 60, NOW - 5, "foreign"),
            ],
        );
        assert!(Store::<Plain>::has_database(dir.path()));
        let store = Store::<Plain>::of(dir.path()).unwrap();
        assert_eq!(store.version(), 1);
        assert_eq!(
            store.last_used_for(&domains(), NOW).unwrap(),
            Some(NOW - 10)
        );
        assert_eq!(store.last_active().unwrap(), Some(NOW - 1));
        let query = Query {
            columns: "tag",
            domains: &domains(),
            now_unix: NOW,
            filter: None,
        };
        let mut tags = store.rows(&query, |row| row.get::<_, String>(0)).unwrap();
        tags.sort();
        assert_eq!(tags, ["a", "b"]);
    }

    #[test]
    fn a_filter_narrows_the_rows_with_its_value() {
        let dir = profile(
            1,
            &[
                (".example.com", NOW + 60, NOW, "a"),
                (".example.com", NOW + 60, NOW, "b"),
            ],
        );
        let store = Store::<Plain>::of(dir.path()).unwrap();
        let query = Query {
            columns: "tag",
            domains: &domains(),
            now_unix: NOW,
            filter: Some(Filter {
                condition: "tag = ?",
                value: &"b",
            }),
        };
        let tags = store.rows(&query, |row| row.get::<_, String>(0)).unwrap();
        assert_eq!(tags, ["b"]);
    }

    #[test]
    fn a_site_or_profile_without_cookies_has_no_times() {
        let dir = profile(1, &[("other.com", NOW + 60, NOW, "x")]);
        let store = Store::<Plain>::of(dir.path()).unwrap();
        assert_eq!(store.last_used_for(&domains(), NOW).unwrap(), None);
        let empty = profile(1, &[]);
        let store = Store::<Plain>::of(empty.path()).unwrap();
        assert_eq!(store.last_active().unwrap(), None);
    }

    #[test]
    fn domains_are_parameters_not_sql() {
        let dir = profile(1, &[("example.com", NOW + 60, NOW, "x")]);
        let store = Store::<Plain>::of(dir.path()).unwrap();
        let hostile = vec!["x' OR '1'='1".to_owned()];
        assert_eq!(store.last_used_for(&hostile, NOW).unwrap(), None);
    }

    #[test]
    fn an_unknown_version_is_refused_and_a_missing_database_reported() {
        let dir = profile(2, &[]);
        assert!(matches!(
            Store::<Plain>::of(dir.path()),
            Err(Error::UnsupportedDatabaseVersion(2))
        ));
        let empty = PrivateDir::new().unwrap();
        assert!(!Store::<Plain>::has_database(empty.path()));
        assert!(matches!(
            Store::<Plain>::of(empty.path()),
            Err(Error::DatabaseNotFound(_))
        ));
    }

    #[test]
    fn the_copy_is_deleted_when_the_store_is_dropped() {
        let dir = profile(1, &[]);
        let store = Store::<Plain>::of(dir.path()).unwrap();
        let copy = store._copy.path().to_owned();
        assert!(copy.is_file());
        drop(store);
        assert!(!copy.exists());
        assert!(!copy.parent().unwrap().exists());
    }
}
