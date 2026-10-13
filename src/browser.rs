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

/// A browser this crate knows how to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Browser {
    Dia,
}

impl Browser {
    /// Name shown to the user.
    pub fn name(self) -> &'static str {
        match self {
            Browser::Dia => "Dia",
        }
    }

    pub fn engine(self) -> Engine {
        match self {
            Browser::Dia => Engine::Chromium,
        }
    }

    /// The profile the browser opens when the user never created another one.
    pub fn default_profile(self) -> Result<Profile> {
        Ok(Profile {
            browser: self,
            name: DEFAULT_PROFILE.to_owned(),
            path: platform::user_data_dir(self)?.join(DEFAULT_PROFILE),
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

    #[cfg(target_os = "macos")]
    #[test]
    fn the_default_profile_is_the_default_folder() {
        let profile = Browser::Dia.default_profile().unwrap();
        assert_eq!(profile.browser, Browser::Dia);
        assert_eq!(profile.name, "Default");
        assert!(profile.path.ends_with("Dia/User Data/Default"));
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
