//! Browser profiles on this computer: the profile folders that hold a cookie database.

use std::path::{Path, PathBuf};

use crate::browser::ProfileLayout;
use crate::engine_store::has_database;
use crate::firefox::profiles_ini::{self, IniProfile};
use crate::{Browser, Error, Result, platform};

/// Folder and display name of the first profile in Chromium browsers.
const DEFAULT_PROFILE: &str = "Default";
/// Chromium names further profile folders "Profile 1", "Profile 2", ...
const OTHER_PROFILE_PREFIX: &str = "Profile ";

/// One profile of one browser on this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// The browser this profile belongs to.
    pub browser: Browser,
    /// Profile name as the browser shows it, for example `Default`, `Profile 1` or
    /// `default-release`.
    pub name: String,
    /// Folder of the profile.
    pub path: PathBuf,
}

/// The profile `browser` opens when the user never created another one, in its data
/// folder `data_dir`.
pub(crate) fn default_profile(browser: Browser, data_dir: &Path) -> Result<Profile> {
    let (name, path) = match browser.layout() {
        ProfileLayout::Folders => (DEFAULT_PROFILE.to_owned(), data_dir.join(DEFAULT_PROFILE)),
        ProfileLayout::Single => (DEFAULT_PROFILE.to_owned(), data_dir.to_owned()),
        ProfileLayout::ProfilesIni => {
            let ini = data_dir.join(profiles_ini::FILE_NAME);
            let profile = read_ini(&ini, data_dir)?
                .into_iter()
                .find(|p| p.is_default)
                .ok_or(Error::DatabaseNotFound(ini))?;
            (profile.name, profile.path)
        }
    };
    Ok(Profile {
        browser,
        name,
        path,
    })
}

/// The profiles of `browser` in its data folder `data_dir` that have a cookie database,
/// the default profile first. Empty if the folder does not exist. A folder the system
/// protects is not read: its default profile is returned unchecked.
pub(crate) fn profiles_in(browser: Browser, data_dir: &Path) -> Result<Vec<Profile>> {
    if platform::is_protected(browser) {
        return Ok(vec![default_profile(browser, data_dir)?]);
    }
    let found: Vec<(String, PathBuf)> = match browser.layout() {
        ProfileLayout::Single => vec![(DEFAULT_PROFILE.to_owned(), data_dir.to_owned())],
        ProfileLayout::Folders => profile_folders(data_dir)?,
        ProfileLayout::ProfilesIni => read_ini(&data_dir.join(profiles_ini::FILE_NAME), data_dir)?
            .into_iter()
            .map(|p| (p.name, p.path))
            .collect(),
    };
    Ok(found
        .into_iter()
        .filter(|(_, path)| has_database(browser.engine(), path))
        .map(|(name, path)| Profile {
            browser,
            name,
            path,
        })
        .collect())
}

/// Chromium profile folders in `data_dir`, `Default` first: `Default`, `Profile 1`, ...
fn profile_folders(data_dir: &Path) -> Result<Vec<(String, PathBuf)>> {
    let Some(entries) = existing(std::fs::read_dir(data_dir), data_dir)? else {
        return Ok(Vec::new());
    };
    let mut folders: Vec<(String, PathBuf)> = entries
        // An entry that cannot be read is not a profile this crate could use either.
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            (name == DEFAULT_PROFILE || name.starts_with(OTHER_PROFILE_PREFIX))
                .then(|| (name, entry.path()))
        })
        .collect();
    folders.sort_by_key(|(name, _)| (name != DEFAULT_PROFILE, name.clone()));
    Ok(folders)
}

/// The profiles listed in the `profiles.ini` at `ini`, the default first. Empty if the
/// file does not exist (the browser is not installed).
fn read_ini(ini: &Path, data_dir: &Path) -> Result<Vec<IniProfile>> {
    Ok(existing(std::fs::read_to_string(ini), ini)?
        .map(|text| profiles_ini::parse(&text, data_dir))
        .unwrap_or_default())
}

/// `None` for a file or folder that does not exist, the value if it was read, and an
/// error naming `path` otherwise.
fn existing<T>(read: std::io::Result<T>, path: &Path) -> Result<Option<T>> {
    match read {
        Ok(value) => Ok(Some(value)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::DatabaseUnreadable {
            path: path.to_owned(),
            reason: e.to_string(),
        }),
    }
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

    /// A Firefox data folder with two profiles; only `default-release` has cookies.
    fn firefox_data() -> PrivateDir {
        let data = PrivateDir::new().unwrap();
        for folder in ["Profiles/a.default", "Profiles/b.default-release"] {
            std::fs::create_dir_all(data.path().join(folder)).unwrap();
        }
        std::fs::write(
            data.path()
                .join("Profiles/b.default-release/cookies.sqlite"),
            "",
        )
        .unwrap();
        std::fs::write(
            data.path().join("profiles.ini"),
            "[Profile0]\nName=default\nPath=Profiles/a.default\n\
             [Profile1]\nName=default-release\nPath=Profiles/b.default-release\n\
             [Install1]\nDefault=Profiles/b.default-release\n",
        )
        .unwrap();
        data
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
        for id in ["chrome", "opera", "firefox"] {
            assert!(
                profiles_in(browser(id), &missing).unwrap().is_empty(),
                "{id}"
            );
        }
    }

    #[test]
    fn opera_has_one_profile_in_its_data_folder() {
        let data = PrivateDir::new().unwrap();
        std::fs::write(data.path().join("Cookies"), "").unwrap();
        let profiles = profiles_in(browser("opera"), data.path()).unwrap();
        assert_eq!(names(&profiles), ["Default"]);
        assert_eq!(profiles[0].path, data.path());
        let profile = default_profile(browser("opera"), data.path()).unwrap();
        assert_eq!(profile.path, data.path());
    }

    #[test]
    fn firefox_profiles_come_from_profiles_ini() {
        let data = firefox_data();
        let profiles = profiles_in(browser("firefox"), data.path()).unwrap();
        assert_eq!(names(&profiles), ["default-release"]);
        assert_eq!(
            profiles[0].path,
            data.path().join("Profiles").join("b.default-release")
        );
        let profile = default_profile(browser("firefox"), data.path()).unwrap();
        assert_eq!(profile, profiles[0]);
    }

    #[test]
    fn firefox_without_profiles_ini_has_no_default_profile() {
        let data = PrivateDir::new().unwrap();
        assert!(matches!(
            default_profile(browser("firefox"), data.path()),
            Err(Error::DatabaseNotFound(path)) if path.ends_with("profiles.ini")
        ));
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
        assert!(matches!(
            profiles_in(browser("firefox"), &file),
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
    fn safaris_protected_folder_is_never_read() {
        let missing = PrivateDir::new().unwrap().path().join("not readable");
        let profiles = profiles_in(browser("safari"), &missing).unwrap();
        assert_eq!(names(&profiles), ["Default"]);
        assert_eq!(profiles[0].path, missing);
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
