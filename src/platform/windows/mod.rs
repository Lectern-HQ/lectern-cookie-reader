//! Windows: where each browser keeps its data. Each browser's Windows details are data in
//! the catalog (`catalog.rs`). Only x64 Windows is supported.

use std::path::PathBuf;

use zeroize::Zeroizing;

use crate::browser::{AppData, Windows};
use crate::{Browser, DefaultBrowser, Error, Result};

/// The browser's Windows details, or an error if it is not read on Windows.
fn windows(browser: Browser) -> Result<&'static Windows> {
    browser
        .spec()
        .windows
        .as_ref()
        .ok_or_else(|| Error::Unsupported(format!("{} on Windows", browser.name())))
}

/// The environment variable that names `base`.
fn variable(base: AppData) -> &'static str {
    match base {
        AppData::Local => "LOCALAPPDATA",
        AppData::Roaming => "APPDATA",
    }
}

/// The folder that holds `browser`'s profiles, under the user's app data folder.
pub(crate) fn user_data_dir(browser: Browser) -> Result<PathBuf> {
    let details = windows(browser)?;
    let name = variable(details.base);
    let base = std::env::var_os(name)
        .filter(|base| !base.is_empty())
        .ok_or_else(|| Error::Unsupported(format!("the {name} environment variable is not set")))?;
    Ok(PathBuf::from(base).join(details.data_dir))
}

/// Not yet: the key in `Local State` is read with DPAPI in issue #18.
pub(crate) fn chromium_key(browser: Browser) -> Result<Zeroizing<[u8; 16]>> {
    Err(Error::Unsupported(format!(
        "decrypting {} cookies on Windows",
        browser.name()
    )))
}

/// Always `None` for now: the default browser is not read on Windows yet.
pub(crate) fn default_browser() -> Option<DefaultBrowser> {
    None
}

/// Always `false`: no browser folder this crate reads on Windows is protected.
pub(crate) fn is_protected(_browser: Browser) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::tests::browser;

    #[test]
    fn chrome_profiles_are_under_local_app_data() {
        let dir = user_data_dir(browser("chrome")).unwrap();
        let local = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap());
        assert_eq!(dir, local.join("Google").join("Chrome").join("User Data"));
    }

    #[test]
    fn firefox_profiles_are_under_roaming_app_data() {
        let dir = user_data_dir(browser("firefox")).unwrap();
        let roaming = PathBuf::from(std::env::var_os("APPDATA").unwrap());
        assert_eq!(dir, roaming.join("Mozilla").join("Firefox"));
    }

    #[test]
    fn browsers_that_are_not_read_on_windows_are_unsupported() {
        for id in ["arc", "dia", "safari"] {
            assert!(
                matches!(user_data_dir(browser(id)), Err(Error::Unsupported(_))),
                "{id}"
            );
        }
        assert!(!is_protected(browser("chrome")));
        assert!(default_browser().is_none());
        assert!(matches!(
            chromium_key(browser("chrome")),
            Err(Error::Unsupported(_))
        ));
    }
}
