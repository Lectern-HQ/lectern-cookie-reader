//! Every browser this crate knows, one entry each. **To add a browser, add one entry here**:
//! nothing else in the crate lists browsers, and the tests below check every entry.
//!
//! Chromium folders and Keychain names on macOS follow yt-dlp's table (`yt_dlp/cookies.py`);
//! Arc and Dia were added here. The Firefox family's folders are each browser's
//! `profiles.ini` folder. Safari's is the folder of its cookie file in its container,
//! which macOS protects. Only Dia has been tested on a real Mac so far (issues #54, #6).

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
            keychain_name: Some("Arc"),
            app_name: "Arc.app",
            protected: false,
        }),
    },
    Spec {
        id: "brave",
        name: "Brave",
        engine: Engine::Chromium,
        layout: ProfileLayout::Folders,
        macos: Some(MacOs {
            data_dir: "Library/Application Support/BraveSoftware/Brave-Browser",
            keychain_name: Some("Brave"),
            app_name: "Brave Browser.app",
            protected: false,
        }),
    },
    Spec {
        id: "chrome",
        name: "Google Chrome",
        engine: Engine::Chromium,
        layout: ProfileLayout::Folders,
        macos: Some(MacOs {
            data_dir: "Library/Application Support/Google/Chrome",
            keychain_name: Some("Chrome"),
            app_name: "Google Chrome.app",
            protected: false,
        }),
    },
    Spec {
        id: "chromium",
        name: "Chromium",
        engine: Engine::Chromium,
        layout: ProfileLayout::Folders,
        macos: Some(MacOs {
            data_dir: "Library/Application Support/Chromium",
            keychain_name: Some("Chromium"),
            app_name: "Chromium.app",
            protected: false,
        }),
    },
    Spec {
        id: "dia",
        name: "Dia",
        engine: Engine::Chromium,
        layout: ProfileLayout::Folders,
        macos: Some(MacOs {
            data_dir: "Library/Application Support/Dia/User Data",
            keychain_name: Some("Dia"),
            app_name: "Dia.app",
            protected: false,
        }),
    },
    Spec {
        id: "edge",
        name: "Microsoft Edge",
        engine: Engine::Chromium,
        layout: ProfileLayout::Folders,
        macos: Some(MacOs {
            data_dir: "Library/Application Support/Microsoft Edge",
            keychain_name: Some("Microsoft Edge"),
            app_name: "Microsoft Edge.app",
            protected: false,
        }),
    },
    Spec {
        id: "firefox",
        name: "Firefox",
        engine: Engine::Firefox,
        layout: ProfileLayout::ProfilesIni,
        macos: Some(MacOs {
            data_dir: "Library/Application Support/Firefox",
            keychain_name: None,
            app_name: "Firefox.app",
            protected: false,
        }),
    },
    Spec {
        id: "floorp",
        name: "Floorp",
        engine: Engine::Firefox,
        layout: ProfileLayout::ProfilesIni,
        macos: Some(MacOs {
            data_dir: "Library/Application Support/Floorp",
            keychain_name: None,
            app_name: "Floorp.app",
            protected: false,
        }),
    },
    Spec {
        id: "librewolf",
        name: "LibreWolf",
        engine: Engine::Firefox,
        layout: ProfileLayout::ProfilesIni,
        macos: Some(MacOs {
            data_dir: "Library/Application Support/librewolf",
            keychain_name: None,
            app_name: "LibreWolf.app",
            protected: false,
        }),
    },
    Spec {
        id: "opera",
        name: "Opera",
        engine: Engine::Chromium,
        layout: ProfileLayout::Single,
        macos: Some(MacOs {
            data_dir: "Library/Application Support/com.operasoftware.Opera",
            keychain_name: Some("Opera"),
            app_name: "Opera.app",
            protected: false,
        }),
    },
    Spec {
        id: "safari",
        name: "Safari",
        engine: Engine::Safari,
        layout: ProfileLayout::Single,
        macos: Some(MacOs {
            data_dir: "Library/Containers/com.apple.Safari/Data/Library/Cookies",
            keychain_name: None,
            app_name: "Safari.app",
            protected: true,
        }),
    },
    Spec {
        id: "vivaldi",
        name: "Vivaldi",
        engine: Engine::Chromium,
        layout: ProfileLayout::Folders,
        macos: Some(MacOs {
            data_dir: "Library/Application Support/Vivaldi",
            keychain_name: Some("Vivaldi"),
            app_name: "Vivaldi.app",
            protected: false,
        }),
    },
    Spec {
        id: "waterfox",
        name: "Waterfox",
        engine: Engine::Firefox,
        layout: ProfileLayout::ProfilesIni,
        macos: Some(MacOs {
            data_dir: "Library/Application Support/Waterfox",
            keychain_name: None,
            app_name: "Waterfox.app",
            protected: false,
        }),
    },
    Spec {
        id: "zen",
        name: "Zen",
        engine: Engine::Firefox,
        layout: ProfileLayout::ProfilesIni,
        macos: Some(MacOs {
            data_dir: "Library/Application Support/zen",
            keychain_name: None,
            app_name: "Zen.app",
            protected: false,
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
        assert!(unique(|m| m.app_name));
        let keychain: Vec<&str> = mac.iter().filter_map(|m| m.keychain_name).collect();
        assert_eq!(
            keychain.iter().collect::<HashSet<_>>().len(),
            keychain.len()
        );
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
            assert_eq!(
                spec.layout == ProfileLayout::ProfilesIni,
                spec.engine == Engine::Firefox,
                "{}: the Firefox family, and only it, lists profiles in profiles.ini",
                spec.id
            );
            if let Some(mac) = &spec.macos {
                assert!(mac.data_dir.starts_with("Library/"), "{}", spec.id);
                assert_eq!(
                    mac.protected,
                    spec.engine == Engine::Safari,
                    "{}: only Safari's folder is protected",
                    spec.id
                );
                assert!(mac.app_name.ends_with(".app"), "{}", spec.id);
                assert_eq!(
                    mac.keychain_name.is_some_and(|n| !n.is_empty()),
                    spec.engine == Engine::Chromium,
                    "{}: a Keychain name for Chromium browsers only",
                    spec.id
                );
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
        assert_eq!(mac.keychain_name, Some("Dia"));
        assert_eq!(mac.app_name, "Dia.app");
    }
}
