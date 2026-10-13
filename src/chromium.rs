//! Decryption of Chromium cookie values that use the `v10` format with AES-128-CBC.
//!
//! This is the format on macOS (and Linux). The key comes from the browser's "Safe Storage"
//! password, stretched with PBKDF2. From cookie database version 24, Chromium puts the
//! SHA-256 hash of the cookie's domain in front of the value, so a value copied from
//! another site's row is detected and refused.

use aes::cipher::{BlockModeDecrypt, KeyIvInit, block_padding::Pkcs7};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::{Error, Result, SecretValue};

/// Oldest database version this crate has been checked against.
pub(crate) const MIN_DB_VERSION: i64 = 18;
/// Newest database version this crate has been checked against. Newer versions are
/// refused instead of risking wrong values.
pub(crate) const MAX_DB_VERSION: i64 = 24;
/// From this version on, each value starts with the SHA-256 hash of its domain.
const HOST_HASH_DB_VERSION: i64 = 24;

const PREFIX: &[u8] = b"v10";
const SALT: &[u8] = b"saltysalt";
const ITERATIONS: u32 = 1003;
const IV: [u8; 16] = [b' '; 16];

/// Turns the browser's "Safe Storage" password into the AES key.
pub(crate) fn derive_key(password: &[u8]) -> Zeroizing<[u8; 16]> {
    let mut key = Zeroizing::new([0u8; 16]);
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password, SALT, ITERATIONS, key.as_mut());
    key
}

/// Checks that the database version is one this crate knows.
pub(crate) fn check_db_version(version: i64) -> Result<()> {
    if (MIN_DB_VERSION..=MAX_DB_VERSION).contains(&version) {
        Ok(())
    } else {
        Err(Error::UnsupportedDatabaseVersion(version))
    }
}

/// Decrypts one stored value.
///
/// `domain` is the row's `host_key` column, exactly as stored (for example `.example.com`).
/// The cookie name is only used in error messages.
pub(crate) fn decrypt_value(
    encrypted: &[u8],
    key: &[u8; 16],
    domain: &str,
    cookie_name: &str,
    db_version: i64,
) -> Result<SecretValue> {
    let failed = || Error::DecryptionFailed {
        cookie_name: cookie_name.to_owned(),
    };
    let body = encrypted.strip_prefix(PREFIX).ok_or_else(failed)?;

    let mut buffer = Zeroizing::new(body.to_vec());
    let plain = cbc::Decryptor::<aes::Aes128>::new(key.into(), &IV.into())
        .decrypt_padded::<Pkcs7>(buffer.as_mut_slice())
        .map_err(|_| failed())?;

    let value = if db_version >= HOST_HASH_DB_VERSION {
        let (hash, value) = plain.split_at_checked(32).ok_or_else(failed)?;
        if hash != Sha256::digest(domain.as_bytes()).as_slice() {
            return Err(Error::HostHashMismatch {
                cookie_name: cookie_name.to_owned(),
            });
        }
        value
    } else {
        plain
    };

    let text = std::str::from_utf8(value).map_err(|_| failed())?;
    Ok(SecretValue::new(text.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Vectors made outside this crate with Python's hashlib and the OpenSSL command line:
    // key = PBKDF2-HMAC-SHA1("test-password", "saltysalt", 1003, 16 bytes),
    // value = "v10" + AES-128-CBC(key, IV of 16 spaces, PKCS7)(plaintext).
    const KEY: &str = "c0ffe4c25f07f62bfc6ab011d9efa54e";
    // plaintext = SHA-256(".example.com") + "hello-world"
    const V24: &str = "763130c9f9d215dfe3fcea9f5f48b78c27252a61339f8511ba847e3060c0caa08c030cea6a3324ec0935447379bb81cf90ecc9";
    // plaintext = "hello-world"
    const V23: &str = "763130e8b708e830066522ef13587430f60f95";

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    fn key() -> [u8; 16] {
        hex(KEY).try_into().unwrap()
    }

    #[test]
    fn derives_the_same_key_as_python() {
        assert_eq!(derive_key(b"test-password").as_slice(), hex(KEY).as_slice());
    }

    #[test]
    fn decrypts_version_24_and_checks_the_host_hash() {
        let value = decrypt_value(&hex(V24), &key(), ".example.com", "session", 24).unwrap();
        assert_eq!(value.expose(), "hello-world");
    }

    #[test]
    fn refuses_a_value_that_belongs_to_another_domain() {
        let err = decrypt_value(&hex(V24), &key(), ".other.com", "session", 24).unwrap_err();
        assert!(matches!(err, Error::HostHashMismatch { cookie_name } if cookie_name == "session"));
    }

    #[test]
    fn decrypts_older_versions_without_a_host_hash() {
        let value = decrypt_value(&hex(V23), &key(), ".example.com", "session", 23).unwrap();
        assert_eq!(value.expose(), "hello-world");
    }

    #[test]
    fn wrong_key_is_a_decryption_error_without_the_value() {
        let wrong = [0u8; 16];
        let err = decrypt_value(&hex(V24), &wrong, ".example.com", "session", 24).unwrap_err();
        assert!(matches!(err, Error::DecryptionFailed { .. }));
        assert!(!err.to_string().contains("hello"));
    }

    #[test]
    fn value_without_v10_prefix_is_refused() {
        let err = decrypt_value(b"plain", &key(), ".example.com", "session", 24).unwrap_err();
        assert!(matches!(err, Error::DecryptionFailed { .. }));
    }

    #[test]
    fn unknown_database_versions_are_refused() {
        assert!(check_db_version(24).is_ok());
        assert!(check_db_version(18).is_ok());
        assert!(matches!(
            check_db_version(25),
            Err(Error::UnsupportedDatabaseVersion(25))
        ));
        assert!(matches!(
            check_db_version(17),
            Err(Error::UnsupportedDatabaseVersion(17))
        ));
    }
}
