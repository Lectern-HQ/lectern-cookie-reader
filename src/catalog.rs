//! Every browser this crate knows, one entry each. **To add a browser, add one entry here**:
//! nothing else in the crate lists browsers, and the tests below check every entry.
//!
//! macOS folders and Keychain names follow yt-dlp's table (`yt_dlp/cookies.py`); Arc and
//! Dia were added here. Only Dia has been tested on a real Mac so far.

use crate::browser::{Engine, MacOs, ProfileLayout, Spec};

/// The catalog, in alphabetical order of `id`.
pub(crate) const BROWSERS: &[Spec] = &[
    Spec {
        id: "arc",
        name: "Arc",
        engine: Engine::Chromium,
        layout: ProfileLayout::Folders,
        macos: Some(MacOs {
            data_dir: "Library/Application Support/Arc/User Data",
            keychain_name: "Arc",
            app_name: "Arc.app",
        }),
    },
    Spec {
        id: "brave",
        name: "Brave",
        engine: Engine::Chromium,
        layout: ProfileLayout::Folders,
        macos: Some(MacOs {
            data_dir: "Library/Application Support/BraveSoftware/Brave-Browser",
            keychain_name: "Brave",
            app_name: "Brave Browser.app",
        }),
    },
    Spec {
        id: "chrome",
        name: "Google Chrome",
        engine: Engine::Chromium,
        layout: ProfileLayout::Folders,
        macos: Some(MacOs {
            data_dir: "Library/Application Support/Google/Chrome",
            keychain_name: "Chrome",
            app_name: "Google Chrome.app",
        }),
    },
    Spec {
        id: "chromium",
        name: "Chromium",
        engine: Engine::Chromium,
        layout: ProfileLayout::Folders,
        macos: Some(MacOs {
            data_dir: "Library/Application Support/Chromium",
            keychain_name: "Chromium",
            app_name: "Chromium.app",
        }),
    },
    Spec {
        id: "dia",
        name: "Dia",
        engine: Engine::Chromium,
        layout: ProfileLayout::Folders,
        macos: Some(MacOs {
            data_dir: "Library/Application Support/Dia/User Data",
            keychain_name: "Dia",
            app_name: "Dia.app",
        }),
    },
    Spec {
        id: "edge",
        name: "Microsoft Edge",
        engine: Engine::Chromium,
        layout: ProfileLayout::Folders,
        macos: Some(MacOs {
            data_dir: "Library/Application Support/Microsoft Edge",
            keychain_name: "Microsoft Edge",
            app_name: "Microsoft Edge.app",
        }),
    },
    Spec {
        id: "opera",
        name: "Opera",
        engine: Engine::Chromium,
        layout: ProfileLayout::Single,
        macos: Some(MacOs {
            data_dir: "Library/Application Support/com.operasoftware.Opera",
            keychain_name: "Opera",
            app_name: "Opera.app",
        }),
    },
    Spec {
        id: "vivaldi",
        name: "Vivaldi",
        engine: Engine::Chromium,
        layout: ProfileLayout::Folders,
        macos: Some(MacOs {
            data_dir: "Library/Application Support/Vivaldi",
            keychain_name: "Vivaldi",
            app_name: "Vivaldi.app",
        }),
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Whether every value `of` gives for the catalog's entries is different.
    fn all_different<T: std::hash::Hash + Eq>(of: impl Fn(&Spec) -> T) -> bool {
        let values: HashSet<T> = BROWSERS.iter().map(of).collect();
        values.len() == BROWSERS.len()
    }

    #[test]
    fn ids_names_and_macos_details_are_unique() {
        assert!(all_different(|s| s.id));
        assert!(all_different(|s| s.name));
        let mac: Vec<&MacOs> = BROWSERS.iter().filter_map(|s| s.macos.as_ref()).collect();
        let unique = |of: fn(&MacOs) -> &str| {
            mac.iter().map(|m| of(m)).collect::<HashSet<_>>().len() == mac.len()
        };
        assert!(unique(|m| m.data_dir));
        assert!(unique(|m| m.keychain_name));
        assert!(unique(|m| m.app_name));
    }

    #[test]
    fn every_entry_is_well_formed() {
        for spec in BROWSERS {
            assert!(
                !spec.id.is_empty()
                    && spec
                        .id
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
                "id {:?}: lower case letters and digits only",
                spec.id
            );
            assert!(!spec.name.is_empty(), "{}: empty name", spec.id);
            if let Some(mac) = &spec.macos {
                assert!(
                    mac.data_dir.starts_with("Library/Application Support/"),
                    "{}",
                    spec.id
                );
                assert!(mac.app_name.ends_with(".app"), "{}", spec.id);
                assert!(!mac.keychain_name.is_empty(), "{}", spec.id);
            }
        }
    }

    #[test]
    fn entries_are_in_alphabetical_order_of_id() {
        let ids: Vec<&str> = BROWSERS.iter().map(|s| s.id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn dia_matches_what_was_checked_on_a_real_mac() {
        let dia = BROWSERS.iter().find(|s| s.id == "dia").unwrap();
        let mac = dia.macos.as_ref().unwrap();
        assert_eq!(mac.data_dir, "Library/Application Support/Dia/User Data");
        assert_eq!(mac.keychain_name, "Dia");
        assert_eq!(mac.app_name, "Dia.app");
    }
}
