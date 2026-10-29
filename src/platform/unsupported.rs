//! Platforms that are not supported: anything but macOS and Windows.

use std::path::PathBuf;

use crate::chromium::crypto::ChromiumKey;
use crate::{Browser, DefaultBrowser, Error, Result};

/// Always fails: profile folders are not known on this system yet.
pub(crate) fn user_data_dir(browser: Browser) -> Result<PathBuf> {
    Err(unsupported(browser))
}

/// Always fails: the browser's key cannot be read on this system yet.
pub(crate) fn chromium_key(browser: Browser) -> Result<ChromiumKey> {
    Err(unsupported(browser))
}

/// Always `None`: the default browser is not read on this system yet.
pub(crate) fn default_browser() -> Option<DefaultBrowser> {
    None
}

/// What the user does when the system refuses a file.
pub(crate) const PERMISSION_HINT: &str = super::ALLOW_READING_HINT;

/// Always `false`: no folder is known on this system, so none is protected.
pub(crate) fn is_protected(_browser: Browser) -> bool {
    false
}

/// Always `false`: no lock is recognised on this system.
pub(crate) fn is_locked(_error: &std::io::Error) -> bool {
    false
}

/// Always `None`: whether a process runs is not asked on this system.
pub(crate) fn is_running(_pid: u32) -> Option<bool> {
    None
}

/// The error that names the browser and says only macOS is supported for now.
fn unsupported(browser: Browser) -> Error {
    Error::Unsupported(format!(
        "reading {} on this operating system (only macOS and Windows)",
        browser.name()
    ))
}
