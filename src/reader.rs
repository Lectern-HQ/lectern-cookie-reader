//! The public lookup: the cookies of one website in one browser profile.

use crate::clock::now_unix;
use crate::domain::domains_for;
use crate::engine_store::StoreCopy;
use crate::{Cookie, Profile, Result, platform};

/// Reads the unexpired cookies that a browser would send to `host`.
///
/// `host` is a plain host name such as `mediaserver.unige.ch`, not a link.
///
/// The steps, in order:
/// 1. The profile's cookie database is copied into a folder only the user can open,
///    removed again before this function returns.
/// 2. If no cookie for `host` is stored, an empty list is returned. The browser's key is
///    not requested, so the user sees no permission dialog for a site they are not
///    logged in to.
/// 3. For Chromium browsers, if a matching cookie uses app-bound encryption (Chrome and
///    Edge on Windows), the error is [`crate::Error::AppBoundEncryption`] and the key is
///    not requested. Otherwise the browser's key is requested from the operating system (on
///    macOS the Keychain, which may show a dialog), and only the matching cookies are
///    decrypted. The Firefox family and Safari store values without encryption, so there
///    is no key and no dialog.
///
/// Safari's cookie file is protected by macOS and read directly, not copied. For Safari,
/// `profile.path` is the folder of the `Cookies.binarycookies` file the user allowed this
/// app to read; without that permission the error is [`crate::Error::PermissionNeeded`].
pub fn read_cookies(profile: &Profile, host: &str) -> Result<Vec<Cookie>> {
    let domains = domains_for(host)?;
    StoreCopy::of(profile)?.read(
        profile.browser,
        &domains,
        now_unix(),
        platform::chromium_key,
    )
}
