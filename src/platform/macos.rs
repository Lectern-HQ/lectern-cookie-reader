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

/// Where a Chromium browser keeps its data on macOS, and the name of its Keychain item.
/// Folders and names match yt-dlp's table (`yt_dlp/cookies.py`); Arc and Dia were added
/// here, and only Dia has been checked on a real Mac so far.
struct ChromiumBrowser {
    /// Relative to the home folder.
    user_data_dir: &'static str,
    /// The Keychain item has the service "<name> Safe Storage" and the account "<name>".
    keychain_name: &'static str,
}

fn chromium_browser(browser: Browser) -> ChromiumBrowser {
    let (user_data_dir, keychain_name) = match browser {
        Browser::Arc => ("Library/Application Support/Arc/User Data", "Arc"),
        Browser::Brave => (
            "Library/Application Support/BraveSoftware/Brave-Browser",
            "Brave",
        ),
        Browser::Chrome => ("Library/Application Support/Google/Chrome", "Chrome"),
        Browser::Chromium => ("Library/Application Support/Chromium", "Chromium"),
        Browser::Dia => ("Library/Application Support/Dia/User Data", "Dia"),
        Browser::Edge => (
            "Library/Application Support/Microsoft Edge",
            "Microsoft Edge",
        ),
        Browser::Opera => (
            "Library/Application Support/com.operasoftware.Opera",
            "Opera",
        ),
        Browser::Vivaldi => ("Library/Application Support/Vivaldi", "Vivaldi"),
    };
    ChromiumBrowser {
        user_data_dir,
        keychain_name,
    }
}

fn keychain_service(name: &str) -> String {
    format!("{name} Safe Storage")
}

pub(crate) fn user_data_dir(browser: Browser) -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .ok_or_else(|| Error::Unsupported("the HOME environment variable is not set".into()))?;
    Ok(PathBuf::from(home).join(chromium_browser(browser).user_data_dir))
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
        let dia = chromium_browser(Browser::Dia);
        assert_eq!(keychain_service(dia.keychain_name), "Dia Safe Storage");
        assert_eq!(dia.keychain_name, "Dia");
    }

    #[test]
    fn every_browser_has_its_own_folder_and_keychain_item() {
        let table: Vec<_> = Browser::ALL.into_iter().map(chromium_browser).collect();
        let folders: std::collections::HashSet<_> = table.iter().map(|b| b.user_data_dir).collect();
        let names: std::collections::HashSet<_> = table.iter().map(|b| b.keychain_name).collect();
        assert_eq!(folders.len(), table.len());
        assert_eq!(names.len(), table.len());
        assert!(
            table
                .iter()
                .all(|b| b.user_data_dir.starts_with("Library/Application Support/"))
        );
    }

    #[test]
    fn dia_profiles_are_under_application_support() {
        let dir = user_data_dir(Browser::Dia).unwrap();
        assert!(dir.ends_with("Library/Application Support/Dia/User Data"));
    }
}
