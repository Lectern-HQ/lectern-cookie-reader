//! Browser profiles on this computer: the profile folders that hold a cookie database.

use std::path::{Path, PathBuf};

use crate::chromium::database;
use crate::{Browser, Error, Result};

/// Folder and display name of the first profile in Chromium browsers.
const DEFAULT_PROFILE: &str = "Default";
/// Chromium names further profile folders "Profile 1", "Profile 2", ...
const OTHER_PROFILE_PREFIX: &str = "Profile ";

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

/// The profile `browser` opens when the user never created another one, in its data
/// folder `data_dir`.
pub(crate) fn default_profile(browser: Browser, data_dir: &Path) -> Result<Profile> {
    Ok(Profile {
        browser,
        name: DEFAULT_PROFILE.to_owned(),
        path: if browser.has_profiles() {
            data_dir.join(DEFAULT_PROFILE)
        } else {
            data_dir.to_owned()
        },
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::tests::browser;

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
        let profiles = profiles_in(browser("dia"), data.path()).unwrap();
        assert_eq!(names(&profiles), ["Default", "Profile 1", "Profile 2"]);
        assert_eq!(profiles[1].path, data.path().join("Profile 1"));
        assert!(profiles.iter().all(|p| p.browser == browser("dia")));
    }

    #[test]
    fn a_browser_that_is_not_installed_has_no_profiles() {
        let data = PrivateDir::new().unwrap();
        let missing = data.path().join("not installed");
        assert!(profiles_in(browser("chrome"), &missing).unwrap().is_empty());
        assert!(profiles_in(browser("opera"), &missing).unwrap().is_empty());
    }

    #[test]
    fn opera_has_one_profile_in_its_data_folder() {
        let data = PrivateDir::new().unwrap();
        std::fs::write(data.path().join("Cookies"), "").unwrap();
        let profiles = profiles_in(browser("opera"), data.path()).unwrap();
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
            profiles_in(browser("dia"), &file),
            Err(Error::DatabaseUnreadable { .. })
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_default_profile_is_the_default_folder() {
        let profile = browser("dia").default_profile().unwrap();
        assert_eq!(profile.browser, browser("dia"));
        assert_eq!(profile.name, "Default");
        assert!(profile.path.ends_with("Dia/User Data/Default"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn opera_uses_its_data_folder_as_the_profile() {
        let profile = browser("opera").default_profile().unwrap();
        assert!(profile.path.ends_with("com.operasoftware.Opera"));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn other_systems_are_reported_as_unsupported() {
        assert!(matches!(
            browser("dia").default_profile(),
            Err(crate::Error::Unsupported(_))
        ));
    }
}
