//! Test data shared by the Chromium tests.
//!
//! Vectors made outside this crate with Python's hashlib and the OpenSSL command line:
//! key = PBKDF2-HMAC-SHA1("test-password", "saltysalt", 1003, 16 bytes),
//! value = "v10" + AES-128-CBC(key, IV of 16 spaces, PKCS7)(plaintext).
//!
//! Windows vectors made with Apple's CryptoKit (`AES.GCM.seal`) and checked with yt-dlp's
//! pure Python AES-GCM: key = bytes 00 to 1f, nonce = bytes 0a to 15,
//! value = "v10" + nonce + AES-256-GCM(key, nonce)(plaintext) + tag.

use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use crate::chromium::crypto::ChromiumKey;
use crate::chromium::unix_to_chromium;

pub(crate) const KEY: &str = "c0ffe4c25f07f62bfc6ab011d9efa54e";
/// plaintext = SHA-256(".example.com") + "hello-world"
pub(crate) const V24: &str = "763130c9f9d215dfe3fcea9f5f48b78c27252a61339f8511ba847e3060c0caa08c030cea6a3324ec0935447379bb81cf90ecc9";
/// plaintext = "hello-world"
pub(crate) const V23: &str = "763130e8b708e830066522ef13587430f60f95";

/// plaintext = SHA-256(".example.com") + "hello-world", AES-256-GCM.
pub(crate) const GCM_V24: &str = "7631300a0b0c0d0e0f1011121314157b2b139340351ff8c49eec1be0c460f8ba16ececf1e1a5e7282962638aeaacea41043cddcca9b538ea3d9262999f9af8847a27854c8f0e82ec5c1f";
/// plaintext = "hello-world", AES-256-GCM.
pub(crate) const GCM_V23: &str =
    "7631300a0b0c0d0e0f10111213141507d856a404f0a01ac468bc2add1f7f167adba6a14c443934516ded";

pub(crate) fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// The macOS test key, for the `V24` and `V23` vectors.
pub(crate) fn key() -> ChromiumKey {
    ChromiumKey::Cbc(Zeroizing::new(hex(KEY).try_into().unwrap()))
}

/// The Windows test key, for the `GCM_V24` and `GCM_V23` vectors.
pub(crate) fn gcm_key() -> ChromiumKey {
    ChromiumKey::Gcm(Zeroizing::new(std::array::from_fn(|i| i as u8)))
}

/// The schema of a Chromium cookie database, shared with the tests of the public API.
const SCHEMA: &str = include_str!("../../tests/fixtures/chromium_cookies.sql");

/// Inserts one cookie of a secure `https` page, sent with medium priority and no
/// `SameSite` attribute, as Chromium stores them.
const INSERT_COOKIE: &str = "INSERT INTO cookies (
        creation_utc, host_key, top_frame_site_key, name, value, encrypted_value, path,
        expires_utc, is_secure, is_httponly, last_access_utc, has_expires, is_persistent,
        priority, samesite, source_scheme, source_port, last_update_utc, source_type,
        has_cross_site_ancestor)
    VALUES (?8, ?1, ?7, ?2, '', ?6, ?10, ?4, 1, 1, ?5, ?3, ?3, 1, -1, 2, ?9, ?8, 0, 0)";

/// The port of an `https` page.
const HTTPS_PORT: u16 = 443;

pub(crate) struct Row<'a> {
    domain: &'a str,
    name: &'a str,
    path: &'a str,
    expires: Option<i64>,
    last_used: i64,
    value: Vec<u8>,
    top_frame_site: &'a str,
    created: i64,
    port: u16,
}

pub(crate) use crate::clock::tests::NOW;

/// A first-party cookie for the path `/`, whose value is the `V24` vector, encrypted for
/// `.example.com`. It was set by an `https` page 100 seconds before `NOW` and last sent to
/// the site 10 seconds before `NOW`.
pub(crate) fn row<'a>(domain: &'a str, name: &'a str, expires: Option<i64>) -> Row<'a> {
    Row {
        domain,
        name,
        path: "/",
        expires,
        last_used: NOW - 10,
        value: hex(V24),
        top_frame_site: "",
        created: NOW - 100,
        port: HTTPS_PORT,
    }
}

impl<'a> Row<'a> {
    /// Last sent to the site at `unix_secs`.
    pub(crate) fn last_used(mut self, unix_secs: i64) -> Self {
        self.last_used = unix_secs;
        self
    }

    /// Stored as `stored` in `encrypted_value`.
    pub(crate) fn value(mut self, stored: &[u8]) -> Self {
        self.value = stored.to_vec();
        self
    }

    /// Partitioned: set while the site was embedded in a page of `site` (stored as
    /// `top_frame_site_key`), for example `https://other.com`.
    pub(crate) fn partitioned(mut self, site: &'a str) -> Self {
        self.top_frame_site = site;
        self
    }

    /// Created at `unix_secs` (`creation_utc`), which decides which of two rows of one
    /// cookie is kept.
    pub(crate) fn created(mut self, unix_secs: i64) -> Self {
        self.created = unix_secs;
        self
    }

    /// For the path `path` on the site instead of `/`.
    pub(crate) fn path(mut self, path: &'a str) -> Self {
        self.path = path;
        self
    }

    /// Set by a page on `port` (`source_port`) instead of 443. Two rows of one cookie must
    /// differ in it, or the unique index refuses the second.
    pub(crate) fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }
}

/// Writes a generated cookie database named `Cookies` into `folder`, with `rows` in their
/// order.
pub(crate) fn create_db(folder: &Path, version: &str, rows: &[Row]) -> PathBuf {
    let path = folder.join("Cookies");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(SCHEMA).unwrap();
    conn.execute("INSERT INTO meta VALUES ('version', ?1)", [version])
        .unwrap();
    for r in rows {
        conn.execute(
            INSERT_COOKIE,
            rusqlite::params![
                r.domain,
                r.name,
                r.expires.is_some(),
                r.expires.map_or(0, unix_to_chromium),
                unix_to_chromium(r.last_used),
                r.value,
                r.top_frame_site,
                unix_to_chromium(r.created),
                r.port,
                r.path
            ],
        )
        .unwrap();
    }
    path
}
