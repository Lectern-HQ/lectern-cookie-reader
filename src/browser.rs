//! A browser this crate knows, as a handle to its entry in the catalog (`catalog.rs`).
//! Nothing here lists browsers: every browser's details are data in the catalog.

use crate::catalog::BROWSERS;
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

/// Where a browser keeps its profiles inside its data folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProfileLayout {
    /// One subfolder per profile: `Default`, `Profile 1`, `Profile 2`...
    Folders,
    /// The data folder itself is the only profile (Opera).
    Single,
}

/// Where a browser keeps its data on macOS, and the names macOS knows it by.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct MacOs {
    /// The folder that holds the profiles, relative to the home folder.
    pub(crate) data_dir: &'static str,
    /// The Keychain item has the service "<name> Safe Storage" and the account "<name>".
    pub(crate) keychain_name: &'static str,
    /// File name of the app in `/Applications`.
    pub(crate) app_name: &'static str,
}

/// One browser's entry in the catalog.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Spec {
    /// Short, stable identifier in lower case, for example `dia`.
    pub(crate) id: &'static str,
    /// Name shown to the user, for example `Microsoft Edge`.
    pub(crate) name: &'static str,
    /// How the browser stores its cookies.
    pub(crate) engine: Engine,
    /// Where its profiles are inside its data folder.
    pub(crate) layout: ProfileLayout,
    /// Its macOS details, or `None` when it does not exist on macOS.
    pub(crate) macos: Option<MacOs>,
}

/// A browser this crate knows how to read. Get one with [`Browser::by_id`], or go through
/// all of them with [`Browser::all`].
#[derive(Clone, Copy)]
pub struct Browser(&'static Spec);

impl Browser {
    /// Every browser this crate knows, in alphabetical order of id.
    pub fn all() -> impl Iterator<Item = Browser> {
        BROWSERS.iter().map(Browser)
    }

    /// The browser with this id (`arc`, `brave`, `chrome`, `chromium`, `dia`, `edge`,
    /// `opera`, `vivaldi`), or `None` for one this crate does not know.
    pub fn by_id(id: &str) -> Option<Browser> {
        Self::all().find(|b| b.id() == id)
    }

    /// Short, stable identifier, for example `dia`. Suitable for saving a user's choice.
    pub fn id(self) -> &'static str {
        self.spec().id
    }

    /// Name shown to the user, for example `Microsoft Edge`.
    pub fn name(self) -> &'static str {
        self.spec().name
    }

    /// How the browser stores its cookies.
    pub fn engine(self) -> Engine {
        self.spec().engine
    }

    /// The browser's catalog entry.
    pub(crate) fn spec(self) -> &'static Spec {
        self.0
    }

    /// Whether the browser keeps several profiles in subfolders (`Default`, `Profile 1`).
    pub(crate) fn has_profiles(self) -> bool {
        self.spec().layout == ProfileLayout::Folders
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

impl PartialEq for Browser {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Eq for Browser {}

impl std::hash::Hash for Browser {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id().hash(state);
    }
}

impl std::fmt::Debug for Browser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Browser({})", self.id())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A browser from the catalog, for tests.
    pub(crate) fn browser(id: &str) -> Browser {
        Browser::by_id(id).unwrap_or_else(|| panic!("{id} is not in the catalog"))
    }

    #[test]
    fn a_browser_is_found_by_its_id() {
        let dia = browser("dia");
        assert_eq!(dia.id(), "dia");
        assert_eq!(dia.name(), "Dia");
        assert_eq!(dia.engine(), Engine::Chromium);
        assert_eq!(Browser::by_id("netscape"), None);
    }

    #[test]
    fn every_catalog_entry_is_a_browser() {
        assert_eq!(Browser::all().count(), BROWSERS.len());
        assert!(Browser::all().all(|b| Browser::by_id(b.id()) == Some(b)));
    }

    #[test]
    fn browsers_compare_and_print_by_id() {
        assert_eq!(browser("dia"), browser("dia"));
        assert_ne!(browser("dia"), browser("arc"));
        assert_eq!(format!("{:?}", browser("edge")), "Browser(edge)");
        let set: std::collections::HashSet<Browser> = Browser::all().collect();
        assert_eq!(set.len(), BROWSERS.len());
    }

    #[test]
    fn the_profile_layout_comes_from_the_catalog() {
        assert!(browser("chrome").has_profiles());
        assert!(!browser("opera").has_profiles());
    }
}
