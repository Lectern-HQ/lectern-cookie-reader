//! The browsers this crate knows and how each stores its cookies.

use crate::profile::{self, Profile};
use crate::{Result, platform};

/// How a browser stores its cookies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    /// Chrome, Arc, Dia, Brave, Edge, ...: SQLite database with encrypted values.
    Chromium,
    /// Firefox, Zen, LibreWolf, ...: SQLite database with plain values.
    Firefox,
}

/// A browser this crate knows how to read. New browsers are added over time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Browser {
    /// Arc, from The Browser Company.
    Arc,
    /// Brave.
    Brave,
    /// Google Chrome.
    Chrome,
    /// Chromium, the open source base of Chrome.
    Chromium,
    /// Dia, from The Browser Company.
    Dia,
    /// Microsoft Edge.
    Edge,
    /// Opera.
    Opera,
    /// Vivaldi.
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

    /// How the browser stores its cookies.
    pub fn engine(self) -> Engine {
        Engine::Chromium
    }

    /// Whether the browser keeps several profiles in subfolders (`Default`, `Profile 1`).
    /// Opera keeps its only profile directly in its data folder.
    pub(crate) fn has_profiles(self) -> bool {
        self != Browser::Opera
    }

    /// The profiles of this browser on this computer that have a cookie database,
    /// `Default` first. Empty if the browser is not installed.
    pub fn profiles(self) -> Result<Vec<Profile>> {
        profile::profiles_in(self, &platform::user_data_dir(self)?)
    }

    /// The profile the browser opens when the user never created another one.
    pub fn default_profile(self) -> Result<Profile> {
        profile::default_profile(self, &platform::user_data_dir(self)?)
    }
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
}
