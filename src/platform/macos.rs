//! macOS: profile folders under `~/Library/Application Support`, and the browser's
//! "Safe Storage" password from the Keychain through Apple's Security framework.
//!
//! The `security` command line tool is never used: with the framework, macOS asks the
//! user about the app that uses this library, not about a shared system tool.

use std::path::PathBuf;

use security_framework::passwords::get_generic_password;
use zeroize::Zeroizing;

use crate::chromium::crypto;
use crate::{Browser, Error, Result};

/// `errSecUserCanceled`: the user clicked "Deny" in the Keychain dialog.
const ERR_SEC_USER_CANCELED: i32 = -128;
/// `errSecAuthFailed`: the user typed a wrong password or closed the dialog.
const ERR_SEC_AUTH_FAILED: i32 = -25293;
/// `errSecItemNotFound`: the browser never stored its key (for example never opened).
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

struct ChromiumBrowser {
    /// Relative to the home folder.
    user_data_dir: &'static str,
    keychain_service: &'static str,
    keychain_account: &'static str,
}

fn chromium_browser(browser: Browser) -> ChromiumBrowser {
    match browser {
        Browser::Dia => ChromiumBrowser {
            user_data_dir: "Library/Application Support/Dia/User Data",
            keychain_service: "Dia Safe Storage",
            keychain_account: "Dia",
        },
    }
}

pub(crate) fn user_data_dir(browser: Browser) -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .ok_or_else(|| Error::Unsupported("the HOME environment variable is not set".into()))?;
    Ok(PathBuf::from(home).join(chromium_browser(browser).user_data_dir))
}

/// Asks the Keychain for the browser's password. macOS may show its permission dialog.
pub(crate) fn chromium_key(browser: Browser) -> Result<Zeroizing<[u8; 16]>> {
    let names = chromium_browser(browser);
    let password = get_generic_password(names.keychain_service, names.keychain_account)
        .map_err(|e| key_error(e.code()))?;
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
    fn dia_profiles_are_under_application_support() {
        let dir = user_data_dir(Browser::Dia).unwrap();
        assert!(dir.ends_with("Library/Application Support/Dia/User Data"));
    }
}
