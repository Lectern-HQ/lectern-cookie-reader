//! The browsers this crate knows, their profiles on this computer, and the system's
//! default browser.

use std::path::{Path, PathBuf};

use crate::chromium::database;
use crate::{Error, Result, platform};

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
/// Chromium names further profile folders "Profile 1", "Profile 2", ...
const OTHER_PROFILE_PREFIX: &str = "Profile ";

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
        profiles_in(self, &platform::user_data_dir(self)?)
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

/// The profiles of `browser` in its data folder `data_dir` that have a cookie database, `Default` first. Empty if the folder does not exist.
pub(crate) fn profiles_in(browser: Browser, data_dir: &Path) -> Result<Vec<Profile>> {
    if !browser.has_profiles() {
        let profile = Profile {
            browser,
            name: DEFAULT_PROFILE.to_owned(),
            path: data_dir.to_owned(),
        };
        return Ok(database::has_database(data_dir)
            .then_some(profile)
            .into_iter()
            .collect());
    }
    let entries = match std::fs::read_dir(data_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(Error::DatabaseUnreadable {
                path: data_dir.to_owned(),
                reason: e.to_string(),
            });
        }
    };
    let mut profiles: Vec<Profile> = entries
        // An entry that cannot be read is not a profile this crate could use either.
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let is_profile = name == DEFAULT_PROFILE || name.starts_with(OTHER_PROFILE_PREFIX);
            (is_profile && database::has_database(&entry.path())).then(|| Profile {
                browser,
                name,
                path: entry.path(),
            })
        })
        .collect();
    profiles.sort_by_key(|p| (p.name != DEFAULT_PROFILE, p.name.clone()));
    Ok(profiles)
}

/// One profile of one browser on this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// The browser this profile belongs to.
    pub browser: Browser,
    /// Profile name as the browser shows it, for example `Default` or `Profile 1`.
    pub name: String,
    /// Folder of the profile.
    pub path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::private_dir::PrivateDir;

    fn make_profile(data_dir: &Path, name: &str, with_database: bool) {
        let dir = data_dir.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        if with_database {
            std::fs::write(dir.join("Cookies"), "").unwrap();
        }
    }

    fn names(profiles: &[Profile]) -> Vec<&str> {
        profiles.iter().map(|p| p.name.as_str()).collect()
    }

    #[test]
    fn lists_profiles_with_a_cookie_database_default_first() {
        let data = PrivateDir::new().unwrap();
        make_profile(data.path(), "Profile 2", true);
        make_profile(data.path(), "Default", true);
        make_profile(data.path(), "Profile 1", true);
        make_profile(data.path(), "Profile 3", false);
        make_profile(data.path(), "Guest Profile", true);
        make_profile(data.path(), "System Profile", true);
        let profiles = profiles_in(Browser::Dia, data.path()).unwrap();
        assert_eq!(names(&profiles), ["Default", "Profile 1", "Profile 2"]);
        assert_eq!(profiles[1].path, data.path().join("Profile 1"));
        assert!(profiles.iter().all(|p| p.browser == Browser::Dia));
    }

    #[test]
    fn a_browser_that_is_not_installed_has_no_profiles() {
        let data = PrivateDir::new().unwrap();
        let missing = data.path().join("not installed");
        assert!(profiles_in(Browser::Chrome, &missing).unwrap().is_empty());
        assert!(profiles_in(Browser::Opera, &missing).unwrap().is_empty());
    }

    #[test]
    fn opera_has_one_profile_in_its_data_folder() {
        let data = PrivateDir::new().unwrap();
        std::fs::write(data.path().join("Cookies"), "").unwrap();
        let profiles = profiles_in(Browser::Opera, data.path()).unwrap();
        assert_eq!(names(&profiles), ["Default"]);
        assert_eq!(profiles[0].path, data.path());
    }

    #[cfg(unix)]
    #[test]
    fn a_data_folder_that_cannot_be_read_is_an_error() {
        let data = PrivateDir::new().unwrap();
        let file = data.path().join("not a folder");
        std::fs::write(&file, "").unwrap();
        assert!(matches!(
            profiles_in(Browser::Dia, &file),
            Err(Error::DatabaseUnreadable { .. })
        ));
    }

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
