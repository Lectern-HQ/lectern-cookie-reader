//! The browser's "Safe Storage" password from the Keychain, through Apple's Security
//! framework.
//!
//! The `security` command line tool is never used: with the framework, macOS asks the
//! user about the app that uses this library, not about a shared system tool.

use security_framework::passwords::get_generic_password;
use zeroize::Zeroizing;

use super::chromium_browser;
use crate::chromium::crypto;
use crate::{Browser, Error, Result};

/// `errSecUserCanceled`: the user clicked "Deny" in the Keychain dialog.
const ERR_SEC_USER_CANCELED: i32 = -128;
/// `errSecAuthFailed`: the user typed a wrong password or closed the dialog.
const ERR_SEC_AUTH_FAILED: i32 = -25293;
/// `errSecItemNotFound`: the browser never stored its key (for example never opened).
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

fn keychain_service(name: &str) -> String {
    format!("{name} Safe Storage")
}

/// Asks the Keychain for the browser's password. macOS may show its permission dialog.
pub(crate) fn chromium_key(browser: Browser) -> Result<Zeroizing<[u8; 16]>> {
    let name = chromium_browser(browser).keychain_name;
    let password =
        get_generic_password(&keychain_service(name), name).map_err(|e| key_error(e.code()))?;
    let password = Zeroizing::new(password);
    Ok(crypto::derive_key(&password))
}

fn key_error(code: i32) -> Error {
    match code {
        ERR_SEC_USER_CANCELED | ERR_SEC_AUTH_FAILED => Error::KeyAccessDenied,
        ERR_SEC_ITEM_NOT_FOUND => Error::KeyNotFound,
        other => Error::KeyUnavailable(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_denied_dialog_is_its_own_error() {
        assert!(matches!(key_error(-128), Error::KeyAccessDenied));
        assert!(matches!(key_error(-25293), Error::KeyAccessDenied));
    }

    #[test]
    fn a_missing_key_is_its_own_error() {
        assert!(matches!(key_error(-25300), Error::KeyNotFound));
    }

    #[test]
    fn other_codes_are_kept() {
        assert!(matches!(key_error(-25308), Error::KeyUnavailable(-25308)));
    }

    #[test]
    fn dia_matches_the_keychain_item_seen_on_a_real_mac() {
        let name = chromium_browser(Browser::Dia).keychain_name;
        assert_eq!(keychain_service(name), "Dia Safe Storage");
        assert_eq!(name, "Dia");
    }
}
