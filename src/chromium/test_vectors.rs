//! Test data shared by the Chromium tests.
//!
//! Vectors made outside this crate with Python's hashlib and the OpenSSL command line:
//! key = PBKDF2-HMAC-SHA1("test-password", "saltysalt", 1003, 16 bytes),
//! value = "v10" + AES-128-CBC(key, IV of 16 spaces, PKCS7)(plaintext).

use std::path::{Path, PathBuf};

use crate::chromium::unix_to_chromium;

pub(crate) const KEY: &str = "c0ffe4c25f07f62bfc6ab011d9efa54e";
/// plaintext = SHA-256(".example.com") + "hello-world"
pub(crate) const V24: &str = "763130c9f9d215dfe3fcea9f5f48b78c27252a61339f8511ba847e3060c0caa08c030cea6a3324ec0935447379bb81cf90ecc9";
/// plaintext = "hello-world"
pub(crate) const V23: &str = "763130e8b708e830066522ef13587430f60f95";

pub(crate) fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

pub(crate) fn key() -> [u8; 16] {
    hex(KEY).try_into().unwrap()
}

/// The schema of a Chromium cookie database, shared with the tests of the public API.
const SCHEMA: &str = include_str!("../../tests/fixtures/chromium_cookies.sql");

pub(crate) struct Row<'a> {
    domain: &'a str,
    name: &'a str,
    expires: Option<i64>,
    last_used: i64,
    value: Vec<u8>,
}

/// Fixed "now" for tests that read generated databases.
pub(crate) const NOW: i64 = 1_800_000_000;

/// A cookie whose value is the `V24` vector, encrypted for `.example.com`, last sent to
/// the site 10 seconds before `NOW`.
pub(crate) fn row<'a>(domain: &'a str, name: &'a str, expires: Option<i64>) -> Row<'a> {
    Row {
        domain,
        name,
        expires,
        last_used: NOW - 10,
        value: hex(V24),
    }
}

impl Row<'_> {
    pub(crate) fn last_used(mut self, unix_secs: i64) -> Self {
        self.last_used = unix_secs;
        self
    }
}

/// Writes a generated cookie database named `Cookies` into `folder`.
pub(crate) fn create_db(folder: &Path, version: &str, rows: &[Row]) -> PathBuf {
    let path = folder.join("Cookies");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(SCHEMA).unwrap();
    conn.execute("INSERT INTO meta VALUES ('version', ?1)", [version])
        .unwrap();
    for r in rows {
        conn.execute(
            "INSERT INTO cookies VALUES (?1, ?2, '/', 1, 1, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                r.domain,
                r.name,
                r.expires.is_some(),
                r.expires.map_or(0, unix_to_chromium),
                unix_to_chromium(r.last_used),
                r.value
            ],
        )
        .unwrap();
    }
    path
}
