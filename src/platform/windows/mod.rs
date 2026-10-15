//! Windows: where each browser keeps its data, and its cookie key from `Local State`.
//! Each browser's Windows details are data in the catalog (`catalog.rs`). Only x64 Windows
//! is supported.

mod dpapi;
mod owner_only;

use std::path::PathBuf;

use windows_sys::Win32::Foundation::ERROR_SHARING_VIOLATION;
use zeroize::Zeroizing;

use crate::browser::{AppData, Windows};
use crate::chromium::crypto::ChromiumKey;
use crate::chromium::local_state;
use crate::{Browser, DefaultBrowser, Error, Result};

pub(crate) use owner_only::create_owner_only_dir;

/// The browser's Windows details, or an error if it is not read on Windows.
fn windows(browser: Browser) -> Result<&'static Windows> {
    super::details(browser, browser.spec().windows.as_ref(), "Windows")
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
    Ok(super::env_folder(variable(details.base))?.join(details.data_dir))
}

/// The browser's AES-256-GCM key: read from `Local State` in its data folder and
/// unprotected with DPAPI for the current user. Windows shows no dialog.
pub(crate) fn chromium_key(browser: Browser) -> Result<ChromiumKey> {
    let protected = local_state::read_protected_key(&user_data_dir(browser)?)?;
    let key = dpapi::unprotect(&protected)?;
    let key: [u8; 32] = key.as_slice().try_into().map_err(|_| Error::InvalidKey)?;
    Ok(ChromiumKey::Gcm(Zeroizing::new(key)))
}

/// Always `None` for now: the default browser is not read on Windows yet.
pub(crate) fn default_browser() -> Option<DefaultBrowser> {
    None
}

/// Whether the read failed because another program, such as a running Chrome, opened the
/// file without sharing it.
pub(crate) fn is_locked(error: &std::io::Error) -> bool {
    u32::try_from(error.raw_os_error().unwrap_or_default()) == Ok(ERROR_SHARING_VIOLATION)
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
        let sharing = std::io::Error::from_raw_os_error(ERROR_SHARING_VIOLATION as i32);
        assert!(is_locked(&sharing));
        assert!(!is_locked(&std::io::Error::other("disk full")));
        assert!(default_browser().is_none());
        assert!(matches!(
            chromium_key(browser("dia")),
            Err(Error::Unsupported(_))
        ));
    }
}
