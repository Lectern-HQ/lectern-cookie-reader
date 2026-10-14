//! The user's default web browser, as set in System Settings, read from Launch Services.
//! Reading it needs no permission.

use std::path::Path;

use core_foundation::base::TCFType;
use core_foundation::error::CFError;
use core_foundation::string::CFString;
use core_foundation::url::CFURL;
use core_foundation_sys::base::kCFAllocatorDefault;
use core_foundation_sys::error::CFErrorRef;
use core_foundation_sys::url::{CFURLCreateWithString, CFURLRef};

use crate::{Browser, DefaultBrowser};

/// `kLSRolesViewer`: the app that opens a link for viewing.
const LS_ROLES_VIEWER: u32 = 0x0000_0002;

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    fn LSCopyDefaultApplicationURLForURL(
        url: CFURLRef,
        role: u32,
        error: *mut CFErrorRef,
    ) -> CFURLRef;
}

/// The app that opens `https` links, or `None` if macOS does not name one.
pub(crate) fn default_browser() -> Option<DefaultBrowser> {
    default_app_for_https().map(|app| from_app_path(&app))
}

/// The path of the app that opens `https` links, from Launch Services.
fn default_app_for_https() -> Option<std::path::PathBuf> {
    let link = CFString::from_static_string("https://example.com");
    // SAFETY: both functions follow Core Foundation's create rule: every non-null object
    // they return is owned by us and released once, by the wrapper it is placed in.
    unsafe {
        let url = CFURLCreateWithString(
            kCFAllocatorDefault,
            link.as_concrete_TypeRef(),
            std::ptr::null(),
        );
        if url.is_null() {
            return None;
        }
        let url = CFURL::wrap_under_create_rule(url);
        let mut error: CFErrorRef = std::ptr::null_mut();
        let app = LSCopyDefaultApplicationURLForURL(
            url.as_concrete_TypeRef(),
            LS_ROLES_VIEWER,
            &mut error,
        );
        if !error.is_null() {
            drop(CFError::wrap_under_create_rule(error));
        }
        if app.is_null() {
            return None;
        }
        CFURL::wrap_under_create_rule(app).to_path()
    }
}

/// `/Applications/Dia.app` gives Dia; an app this crate cannot read, such as Safari,
/// keeps its name with no [`Browser`].
fn from_app_path(app: &Path) -> DefaultBrowser {
    let file_name = app.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    DefaultBrowser {
        name: file_name.trim_end_matches(".app").to_owned(),
        browser: Browser::all().find(|b| {
            b.spec()
                .macos
                .as_ref()
                .is_some_and(|m| m.app_name == file_name)
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_known_app_is_matched_to_its_browser() {
        let found = from_app_path(Path::new("/Applications/Google Chrome.app"));
        assert_eq!(found.browser, Browser::by_id("chrome"));
        assert_eq!(found.name, "Google Chrome");
    }

    #[test]
    fn safari_keeps_its_name_without_a_browser() {
        let found = from_app_path(Path::new("/Applications/Safari.app"));
        assert_eq!(found.browser, None);
        assert_eq!(found.name, "Safari");
    }

    #[test]
    fn macos_names_an_app_for_https_links() {
        let app = default_app_for_https().expect("a default browser is always set on macOS");
        assert!(app.to_string_lossy().ends_with(".app"), "{app:?}");
        let found = default_browser().unwrap();
        assert!(!found.name.is_empty() && !found.name.ends_with(".app"));
    }
}
