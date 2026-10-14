//! macOS: where each browser keeps its data, its key in the Keychain, and the user's
//! default browser.

mod keychain;
mod launch_services;

use std::path::PathBuf;

use crate::{Browser, Error, Result};

pub(crate) use keychain::chromium_key;
pub(crate) use launch_services::default_browser;

/// Where a Chromium browser keeps its data on macOS, and the name of its Keychain item.
/// Folders and names match yt-dlp's table (`yt_dlp/cookies.py`); Arc and Dia were added
/// here, and only Dia has been checked on a real Mac so far.
struct ChromiumBrowser {
    /// Relative to the home folder.
    user_data_dir: &'static str,
    /// The Keychain item has the service "<name> Safe Storage" and the account "<name>".
    keychain_name: &'static str,
    /// File name of the app in `/Applications`.
    app_name: &'static str,
}

fn chromium_browser(browser: Browser) -> ChromiumBrowser {
    let (user_data_dir, keychain_name, app_name) = match browser {
        Browser::Arc => (
            "Library/Application Support/Arc/User Data",
            "Arc",
            "Arc.app",
        ),
        Browser::Brave => (
            "Library/Application Support/BraveSoftware/Brave-Browser",
            "Brave",
            "Brave Browser.app",
        ),
        Browser::Chrome => (
            "Library/Application Support/Google/Chrome",
            "Chrome",
            "Google Chrome.app",
        ),
        Browser::Chromium => (
            "Library/Application Support/Chromium",
            "Chromium",
            "Chromium.app",
        ),
        Browser::Dia => (
            "Library/Application Support/Dia/User Data",
            "Dia",
            "Dia.app",
        ),
        Browser::Edge => (
            "Library/Application Support/Microsoft Edge",
            "Microsoft Edge",
            "Microsoft Edge.app",
        ),
        Browser::Opera => (
            "Library/Application Support/com.operasoftware.Opera",
            "Opera",
            "Opera.app",
        ),
        Browser::Vivaldi => (
            "Library/Application Support/Vivaldi",
            "Vivaldi",
            "Vivaldi.app",
        ),
    };
    ChromiumBrowser {
        user_data_dir,
        keychain_name,
        app_name,
    }
}

pub(crate) fn user_data_dir(browser: Browser) -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .ok_or_else(|| Error::Unsupported("the HOME environment variable is not set".into()))?;
    Ok(PathBuf::from(home).join(chromium_browser(browser).user_data_dir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_browser_has_its_own_folder_keychain_item_and_app() {
        let table: Vec<_> = Browser::ALL.into_iter().map(chromium_browser).collect();
        let folders: HashSet<_> = table.iter().map(|b| b.user_data_dir).collect();
        let names: HashSet<_> = table.iter().map(|b| b.keychain_name).collect();
        let apps: HashSet<_> = table.iter().map(|b| b.app_name).collect();
        assert_eq!(folders.len(), table.len());
        assert_eq!(names.len(), table.len());
        assert_eq!(apps.len(), table.len());
        assert!(table.iter().all(
            |b| b.user_data_dir.starts_with("Library/Application Support/")
                && b.app_name.ends_with(".app")
        ));
    }

    #[test]
    fn dia_profiles_are_under_application_support() {
        let dir = user_data_dir(Browser::Dia).unwrap();
        assert!(dir.ends_with("Library/Application Support/Dia/User Data"));
    }
}
