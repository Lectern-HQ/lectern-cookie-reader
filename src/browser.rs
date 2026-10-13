use std::path::PathBuf;

/// How a browser stores its cookies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    /// Chrome, Arc, Dia, Brave, Edge, ...: SQLite database with encrypted values.
    Chromium,
    /// Firefox, Zen, LibreWolf, ...: SQLite database with plain values.
    Firefox,
}

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
