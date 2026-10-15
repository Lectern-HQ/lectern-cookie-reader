//! The one place that picks the cookie database code for a browser's engine. Everything
//! that only needs site names and times goes through here, for every engine.

use std::path::Path;

use crate::{Engine, Profile, Result, chromium, firefox, safari};

/// Whether `profile_dir` holds a cookie database of `engine`.
pub(crate) fn has_database(engine: Engine, profile_dir: &Path) -> bool {
    match engine {
        Engine::Chromium => chromium::database::has_database(profile_dir),
        Engine::Firefox => firefox::database::has_database(profile_dir),
        Engine::Safari => safari::database::has_database(profile_dir),
    }
}

/// A profile's cookie database, copied into a private folder and opened.
pub(crate) enum StoreCopy {
    /// A Chromium browser's database.
    Chromium(chromium::database::ProfileCopy),
    /// A Firefox family database.
    Firefox(firefox::database::ProfileCopy),
    /// Safari's cookie file, read into memory.
    Safari(safari::database::CookieFileData),
}

impl StoreCopy {
    /// Copies and opens the cookie database of `profile`.
    pub(crate) fn of(profile: &Profile) -> Result<Self> {
        Ok(match profile.browser.engine() {
            Engine::Chromium => Self::Chromium(chromium::database::ProfileCopy::of(&profile.path)?),
            Engine::Firefox => Self::Firefox(firefox::database::ProfileCopy::of(&profile.path)?),
            Engine::Safari => Self::Safari(safari::database::CookieFileData::of(&profile.path)?),
        })
    }

    /// When the browser last sent an unexpired cookie stored under one of `domains`, as
    /// Unix seconds, or `None` if there is no such cookie. Nothing is decrypted.
    pub(crate) fn last_used_for(&self, domains: &[String], now_unix: i64) -> Result<Option<i64>> {
        match self {
            Self::Chromium(copy) => copy.db().last_used_for(domains, now_unix),
            Self::Firefox(copy) => copy.db().last_used_for(domains, now_unix),
            Self::Safari(data) => data.last_used_for(domains, now_unix),
        }
    }

    /// When the browser last sent any cookie, for any site, as Unix seconds.
    pub(crate) fn last_active(&self) -> Result<Option<i64>> {
        match self {
            Self::Chromium(copy) => copy.db().last_active(),
            Self::Firefox(copy) => copy.db().last_active(),
            Self::Safari(data) => data.last_active(),
        }
    }
}
