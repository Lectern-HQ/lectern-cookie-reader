use std::path::PathBuf;

use crate::{Result, platform};

/// How a browser stores its cookies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    /// Chrome, Arc, Dia, Brave, Edge, ...: SQLite database with encrypted values.
    Chromium,
    /// Firefox, Zen, LibreWolf, ...: SQLite database with plain values.
    Firefox,
}

/// Folder and display name of the first profile in Chromium browsers.
const DEFAULT_PROFILE: &str = "Default";

/// A browser this crate knows how to read. New browsers are added over time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Browser {
    Arc,
    Brave,
    Chrome,
    Chromium,
    Dia,
    Edge,
    Opera,
    Vivaldi,
}

impl Browser {
    /// Every browser this crate knows, in alphabetical order.
    pub const ALL: [Browser; 8] = [
        Browser::Arc,
        Browser::Brave,
        Browser::Chrome,
        Browser::Chromium,
        Browser::Dia,
        Browser::Edge,
        Browser::Opera,
        Browser::Vivaldi,
    ];

    /// Name shown to the user.
    pub fn name(self) -> &'static str {
        match self {
            Browser::Arc => "Arc",
            Browser::Brave => "Brave",
            Browser::Chrome => "Google Chrome",
            Browser::Chromium => "Chromium",
            Browser::Dia => "Dia",
            Browser::Edge => "Microsoft Edge",
            Browser::Opera => "Opera",
            Browser::Vivaldi => "Vivaldi",
        }
    }

    pub fn engine(self) -> Engine {
        Engine::Chromium
    }

    /// Whether the browser keeps several profiles in subfolders (`Default`, `Profile 1`).
    /// Opera keeps its only profile directly in its data folder.
    pub(crate) fn has_profiles(self) -> bool {
        self != Browser::Opera
    }

    /// The profile the browser opens when the user never created another one.
    pub fn default_profile(self) -> Result<Profile> {
        let data_dir = platform::user_data_dir(self)?;
        Ok(Profile {
            browser: self,
            name: DEFAULT_PROFILE.to_owned(),
            path: if self.has_profiles() {
                data_dir.join(DEFAULT_PROFILE)
            } else {
                data_dir
            },
        })
    }
}

/// One profile of one browser on this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub browser: Browser,
    /// Profile name as the browser shows it, for example `Default` or `Profile 1`.
    pub name: String,
    /// Folder of the profile.
    pub path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dia_is_a_chromium_browser() {
        assert_eq!(Browser::Dia.name(), "Dia");
        assert_eq!(Browser::Dia.engine(), Engine::Chromium);
    }

    #[test]
    fn every_browser_is_listed_once_with_its_own_name() {
        let names: std::collections::HashSet<_> = Browser::ALL.iter().map(|b| b.name()).collect();
        assert_eq!(names.len(), Browser::ALL.len());
    }

    #[test]
    fn only_opera_has_no_profile_folders() {
        let without: Vec<_> = Browser::ALL
            .into_iter()
            .filter(|b| !b.has_profiles())
            .collect();
        assert_eq!(without, [Browser::Opera]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_default_profile_is_the_default_folder() {
        let profile = Browser::Dia.default_profile().unwrap();
        assert_eq!(profile.browser, Browser::Dia);
        assert_eq!(profile.name, "Default");
        assert!(profile.path.ends_with("Dia/User Data/Default"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn opera_uses_its_data_folder_as_the_profile() {
        let profile = Browser::Opera.default_profile().unwrap();
        assert!(profile.path.ends_with("com.operasoftware.Opera"));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn other_systems_are_reported_as_unsupported() {
        assert!(matches!(
            Browser::Dia.default_profile(),
            Err(crate::Error::Unsupported(_))
        ));
    }
}
