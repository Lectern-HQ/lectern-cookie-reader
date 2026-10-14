//! The system's default browser: the app that opens links. Reading it needs no permission.

use crate::{Browser, platform};

/// The app that opens links on this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultBrowser {
    /// The app's name, for example `Dia` or `Safari`.
    pub name: String,
    /// The matching browser, or `None` for one this crate cannot read (such as Safari).
    pub browser: Option<Browser>,
}

/// The user's default browser as set in the system settings, or `None` if the system does
/// not say (and on systems not supported yet). Needs no permission.
pub fn default_browser() -> Option<DefaultBrowser> {
    platform::default_browser()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_system_names_a_default_browser_on_macos_only() {
        let found = default_browser();
        if cfg!(target_os = "macos") {
            assert!(!found.unwrap().name.is_empty());
        } else {
            assert!(found.is_none());
        }
    }
}
