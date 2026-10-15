//! Test data shared by the Firefox tests: generated databases, never real cookies.

use std::path::{Path, PathBuf};

pub(crate) use crate::chromium::test_vectors::NOW;

/// The schema of a Firefox cookie database, shared with the tests of the public API.
const SCHEMA: &str = include_str!("../../tests/fixtures/firefox_cookies.sql");

pub(crate) struct Row<'a> {
    host: &'a str,
    name: &'a str,
    expires: i64,
    last_used: i64,
    origin: &'a str,
}

/// A cookie of the normal browsing context with the value `value-of-<name>`, last sent
/// to the site 10 seconds before `NOW`.
pub(crate) fn row<'a>(host: &'a str, name: &'a str, expires: i64) -> Row<'a> {
    Row {
        host,
        name,
        expires,
        last_used: NOW - 10,
        origin: "",
    }
}

impl<'a> Row<'a> {
    pub(crate) fn last_used(mut self, unix_secs: i64) -> Self {
        self.last_used = unix_secs;
        self
    }

    pub(crate) fn origin(mut self, origin_attributes: &'a str) -> Self {
        self.origin = origin_attributes;
        self
    }
}

/// Writes a generated cookie database named `cookies.sqlite` into `folder`, with `expiry`
/// in the unit of schema `version`.
pub(crate) fn create_db(folder: &Path, version: i64, rows: &[Row]) -> PathBuf {
    let path = folder.join("cookies.sqlite");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(SCHEMA).unwrap();
    conn.pragma_update(None, "user_version", version).unwrap();
    let per_second = if version >= 16 { 1000 } else { 1 };
    for r in rows {
        conn.execute(
            "INSERT INTO moz_cookies (originAttributes, name, value, host, path, expiry,
                                      lastAccessed, isSecure, isHttpOnly)
             VALUES (?1, ?2, ?3, ?4, '/', ?5, ?6, 1, 1)",
            rusqlite::params![
                r.origin,
                r.name,
                format!("value-of-{}", r.name),
                r.host,
                r.expires * per_second,
                r.last_used * 1_000_000,
            ],
        )
        .unwrap();
    }
    path
}
