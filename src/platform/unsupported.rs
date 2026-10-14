//! Platforms that are not supported yet. Windows is planned (issue #8).

use std::path::PathBuf;

use zeroize::Zeroizing;

use crate::{Browser, DefaultBrowser, Error, Result};

/// Always fails: profile folders are not known on this system yet.
pub(crate) fn user_data_dir(browser: Browser) -> Result<PathBuf> {
    Err(unsupported(browser))
}

/// Always fails: the browser's key cannot be read on this system yet.
pub(crate) fn chromium_key(browser: Browser) -> Result<Zeroizing<[u8; 16]>> {
    Err(unsupported(browser))
}

/// Always `None`: the default browser is not read on this system yet.
pub(crate) fn default_browser() -> Option<DefaultBrowser> {
    None
}

/// The error that names the browser and says only macOS is supported for now.
fn unsupported(browser: Browser) -> Error {
    Error::Unsupported(format!(
        "reading {} on this operating system (only macOS for now)",
        browser.name()
    ))
}
