//! macOS: where each browser keeps its data, its key in the Keychain, and the user's
//! default browser. Each browser's macOS details are data in the catalog (`catalog.rs`).

mod keychain;
mod launch_services;

use std::path::PathBuf;

use crate::browser::MacOs;
use crate::{Browser, Error, Result};

pub(crate) use keychain::chromium_key;
pub(crate) use launch_services::default_browser;

/// The browser's macOS details, or an error if it does not exist on macOS.
fn macos(browser: Browser) -> Result<&'static MacOs> {
    browser
        .spec()
        .macos
        .as_ref()
        .ok_or_else(|| Error::Unsupported(format!("{} on macOS", browser.name())))
}

/// The folder that holds `browser`'s profiles, under the user's home folder.
pub(crate) fn user_data_dir(browser: Browser) -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .ok_or_else(|| Error::Unsupported("the HOME environment variable is not set".into()))?;
    Ok(PathBuf::from(home).join(macos(browser)?.data_dir))
}

/// Whether macOS protects `browser`'s data folder (Safari's container), so that reading
/// it needs the user's permission.
pub(crate) fn is_protected(browser: Browser) -> bool {
    macos(browser).is_ok_and(|m| m.protected)
}

/// Always `false`: macOS does not lock files against reading.
pub(crate) fn is_locked(_error: &std::io::Error) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::tests::browser;

    #[test]
    fn dia_profiles_are_under_application_support() {
        let dir = user_data_dir(browser("dia")).unwrap();
        assert!(dir.ends_with("Library/Application Support/Dia/User Data"));
    }

    #[test]
    fn only_safaris_folder_is_protected() {
        assert!(is_protected(browser("safari")));
        assert!(!is_protected(browser("dia")));
        assert!(!is_protected(browser("firefox")));
        assert!(!is_locked(&std::io::Error::from_raw_os_error(32)));
    }

    #[test]
    fn every_catalog_browser_has_macos_details() {
        assert!(Browser::all().all(|b| macos(b).is_ok()));
    }
}
