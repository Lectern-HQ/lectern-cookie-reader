//! Decryption of Chromium cookie values that use the `v10` format.
//!
//! - macOS: AES-128-CBC. The key comes from the browser's "Safe Storage" password in the
//!   Keychain, stretched with PBKDF2.
//! - Windows: AES-256-GCM with a 12-byte nonce in front of the value and a 16-byte tag
//!   after it. The key is stored in the browser's `Local State`, protected with DPAPI.
//!
//! From cookie database version 24, Chromium puts the SHA-256 hash of the cookie's domain
//! in front of the value, so a value copied from another site's row is detected and
//! refused. Values that start with `v20` use app-bound encryption and are never decrypted.

#[cfg(any(target_os = "macos", test))]
use aes::cipher::{BlockModeDecrypt, KeyIvInit, block_padding::Pkcs7};
#[cfg(any(windows, test))]
use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
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

/// Every value this crate decrypts starts with this version marker.
const PREFIX: &[u8] = b"v10";
/// Values with this marker use app-bound encryption (Chrome 127 and later on Windows):
/// only the browser itself can decrypt them, and this crate never tries.
pub(crate) const APP_BOUND_PREFIX: &[u8] = b"v20";
/// Chromium's fixed salt for turning the password into a key.
#[cfg(any(target_os = "macos", test))]
const SALT: &[u8] = b"saltysalt";
/// Chromium's number of PBKDF2 rounds on macOS.
#[cfg(any(target_os = "macos", test))]
const ITERATIONS: u32 = 1003;
/// Chromium's fixed initialisation vector on macOS: 16 spaces.
#[cfg(any(target_os = "macos", test))]
const IV: [u8; 16] = [b' '; 16];
/// Bytes of the AES-GCM nonce in front of a Windows value.
#[cfg(any(windows, test))]
const NONCE_LEN: usize = 12;

/// The key that decrypts a browser's cookie values, with the cipher it is for.
pub(crate) enum ChromiumKey {
    /// macOS: AES-128-CBC.
    #[cfg(any(target_os = "macos", test))]
    Cbc(Zeroizing<[u8; 16]>),
    /// Windows: AES-256-GCM.
    #[cfg(any(windows, test))]
    Gcm(Zeroizing<[u8; 32]>),
}

/// Turns the browser's "Safe Storage" password into the AES key.
#[cfg(any(target_os = "macos", test))]
pub(crate) fn derive_key(password: &[u8]) -> ChromiumKey {
    let mut key = Zeroizing::new([0u8; 16]);
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password, SALT, ITERATIONS, key.as_mut());
    ChromiumKey::Cbc(key)
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
    key: &ChromiumKey,
    domain: &str,
    cookie_name: &str,
    db_version: i64,
) -> Result<SecretValue> {
    let failed = || Error::DecryptionFailed {
        cookie_name: cookie_name.to_owned(),
    };
    let body = encrypted.strip_prefix(PREFIX).ok_or_else(failed)?;
    let plain = match key {
        #[cfg(any(target_os = "macos", test))]
        ChromiumKey::Cbc(key) => decrypt_cbc(body, key),
        #[cfg(any(windows, test))]
        ChromiumKey::Gcm(key) => decrypt_gcm(body, key),
    }
    .ok_or_else(failed)?;

    let value = if db_version >= HOST_HASH_DB_VERSION {
        let (hash, value) = plain.split_at_checked(32).ok_or_else(failed)?;
        if hash != Sha256::digest(domain.as_bytes()).as_slice() {
            return Err(Error::HostHashMismatch {
                cookie_name: cookie_name.to_owned(),
            });
        }
        value
    } else {
        &plain[..]
    };

    let text = std::str::from_utf8(value).map_err(|_| failed())?;
    Ok(SecretValue::new(text.to_owned()))
}

/// AES-128-CBC with Chromium's fixed IV, or `None` if the padding is wrong.
#[cfg(any(target_os = "macos", test))]
fn decrypt_cbc(body: &[u8], key: &[u8; 16]) -> Option<Zeroizing<Vec<u8>>> {
    let mut buffer = Zeroizing::new(body.to_vec());
    let len = cbc::Decryptor::<aes::Aes128>::new(key.into(), &IV.into())
        .decrypt_padded::<Pkcs7>(buffer.as_mut_slice())
        .ok()?
        .len();
    buffer.truncate(len);
    Some(buffer)
}

/// AES-256-GCM with the nonce in front, or `None` if the tag does not match.
#[cfg(any(windows, test))]
fn decrypt_gcm(body: &[u8], key: &[u8; 32]) -> Option<Zeroizing<Vec<u8>>> {
    let (nonce, sealed) = body.split_at_checked(NONCE_LEN)?;
    let nonce = <&[u8; NONCE_LEN]>::try_from(nonce).ok()?;
    Aes256Gcm::new(key.into())
        .decrypt(nonce.into(), sealed)
        .ok()
        .map(Zeroizing::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chromium::test_vectors::{GCM_V23, GCM_V24, KEY, V23, V24, gcm_key, hex, key};

    #[test]
    fn derives_the_same_key_as_python() {
        let ChromiumKey::Cbc(derived) = derive_key(b"test-password") else {
            panic!("macOS keys are for AES-CBC");
        };
        assert_eq!(derived.as_slice(), hex(KEY).as_slice());
    }

    #[test]
    fn decrypts_version_24_and_checks_the_host_hash() {
        for (value, key) in [(V24, key()), (GCM_V24, gcm_key())] {
            let value = decrypt_value(&hex(value), &key, ".example.com", "session", 24).unwrap();
            assert_eq!(value.expose(), "hello-world");
        }
    }

    #[test]
    fn refuses_a_value_that_belongs_to_another_domain() {
        for (value, key) in [(V24, key()), (GCM_V24, gcm_key())] {
            let err = decrypt_value(&hex(value), &key, ".other.com", "session", 24).unwrap_err();
            assert!(
                matches!(err, Error::HostHashMismatch { cookie_name } if cookie_name == "session")
            );
        }
    }

    #[test]
    fn decrypts_older_versions_without_a_host_hash() {
        for (value, key) in [(V23, key()), (GCM_V23, gcm_key())] {
            let value = decrypt_value(&hex(value), &key, ".example.com", "session", 23).unwrap();
            assert_eq!(value.expose(), "hello-world");
        }
    }

    #[test]
    fn wrong_key_is_a_decryption_error_without_the_value() {
        let wrong = [
            (V24, ChromiumKey::Cbc(Zeroizing::new([0u8; 16]))),
            (GCM_V24, ChromiumKey::Gcm(Zeroizing::new([0u8; 32]))),
        ];
        for (value, key) in wrong {
            let err = decrypt_value(&hex(value), &key, ".example.com", "session", 24).unwrap_err();
            assert!(matches!(err, Error::DecryptionFailed { .. }));
            assert!(!err.to_string().contains("hello"));
        }
    }

    #[test]
    fn a_value_for_the_other_cipher_is_refused() {
        let err = decrypt_value(&hex(V24), &gcm_key(), ".example.com", "session", 24).unwrap_err();
        assert!(matches!(err, Error::DecryptionFailed { .. }));
        let short = hex("7631300a0b0c");
        let err = decrypt_value(&short, &gcm_key(), ".example.com", "session", 24).unwrap_err();
        assert!(matches!(err, Error::DecryptionFailed { .. }));
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
