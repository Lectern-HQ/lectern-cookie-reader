//! Platforms that are not supported yet. Windows is planned (issue #8).

use std::path::PathBuf;

use zeroize::Zeroizing;

use crate::{Browser, DefaultBrowser, Error, Result};

pub(crate) fn user_data_dir(browser: Browser) -> Result<PathBuf> {
    Err(unsupported(browser))
}

pub(crate) fn chromium_key(browser: Browser) -> Result<Zeroizing<[u8; 16]>> {
    Err(unsupported(browser))
}

pub(crate) fn default_browser() -> Option<DefaultBrowser> {
    None
}

fn unsupported(browser: Browser) -> Error {
    Error::Unsupported(format!(
        "reading {} on this operating system (only macOS for now)",
        browser.name()
    ))
}
