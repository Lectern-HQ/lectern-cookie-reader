//! The one place that picks the cookie store for a browser's engine, for scanning and for
//! reading. Everything else works the same for every engine.

use std::path::Path;

use crate::chromium::crypto::ChromiumKey;
use crate::chromium::database::{self as chromium_db, ChromiumStore};
use crate::firefox::database::{self as firefox_db, FirefoxStore};
use crate::safari::database::CookieFileData;
use crate::{Browser, Cookie, Engine, Error, Profile, Result};

/// Whether `profile_dir` holds a cookie database of `engine`.
pub(crate) fn has_database(engine: Engine, profile_dir: &Path) -> bool {
    match engine {
        Engine::Chromium => ChromiumStore::has_database(profile_dir),
        Engine::Firefox => FirefoxStore::has_database(profile_dir),
        Engine::Safari => CookieFileData::has_database(profile_dir),
    }
}

/// A profile's cookies, ready to be queried: a private copy of a SQLite database, or
/// Safari's file in memory.
pub(crate) enum StoreCopy {
    /// A Chromium browser's database.
    Chromium(ChromiumStore),
    /// A Firefox family database.
    Firefox(FirefoxStore),
    /// Safari's cookie file.
    Safari(CookieFileData),
}

impl StoreCopy {
    /// Copies and opens the cookie database of `profile`.
    pub(crate) fn of(profile: &Profile) -> Result<Self> {
        let dir = &profile.path;
        Ok(match profile.browser.engine() {
            Engine::Chromium => Self::Chromium(ChromiumStore::of(dir)?),
            Engine::Firefox => Self::Firefox(FirefoxStore::of(dir)?),
            Engine::Safari => Self::Safari(CookieFileData::of(dir)?),
        })
    }

    /// When the browser last sent a sendable cookie stored under one of `domains`, as
    /// Unix seconds, or `None` if there is no such cookie. Nothing is decrypted.
    pub(crate) fn last_used_for(&self, domains: &[String], now_unix: i64) -> Result<Option<i64>> {
        match self {
            Self::Chromium(store) => store.last_used_for(domains, now_unix),
            Self::Firefox(store) => store.last_used_for(domains, now_unix),
            Self::Safari(data) => data.last_used_for(domains, now_unix),
        }
    }

    /// When the browser last sent any cookie, for any site, as Unix seconds.
    pub(crate) fn last_active(&self) -> Result<Option<i64>> {
        match self {
            Self::Chromium(store) => store.last_active(),
            Self::Firefox(store) => store.last_active(),
            Self::Safari(data) => data.last_active(),
        }
    }

    /// The sendable cookies stored under one of `domains`, for `browser`.
    ///
    /// Chromium values are decrypted with the key from `chromium_key`, which is only asked
    /// for when the site has cookies and none of them uses app-bound encryption.
    pub(crate) fn read(
        &self,
        browser: Browser,
        domains: &[String],
        now_unix: i64,
        chromium_key: impl FnOnce(Browser) -> Result<ChromiumKey>,
    ) -> Result<Vec<Cookie>> {
        match self {
            Self::Chromium(store) => {
                if store.last_used_for(domains, now_unix)?.is_none() {
                    return Ok(Vec::new());
                }
                if chromium_db::uses_app_bound_encryption(store, domains, now_unix)? {
                    return Err(Error::AppBoundEncryption {
                        browser: browser.name().to_owned(),
                    });
                }
                chromium_db::read(store, domains, &chromium_key(browser)?, now_unix)
            }
            Self::Firefox(store) => firefox_db::read(store, domains, now_unix),
            Self::Safari(data) => data.read(domains, now_unix),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::tests::browser;
    use crate::chromium::test_vectors::{NOW, create_db, key, row};
    use crate::domain::candidate_domains;
    use crate::private_dir::PrivateDir;

    /// A profile of `id` in the generated folder `dir`.
    fn profile(id: &str, dir: &PrivateDir) -> Profile {
        Profile {
            browser: browser(id),
            name: "Test".into(),
            path: dir.path().to_owned(),
        }
    }

    /// Reads `example.com`'s cookies from `profile` at `NOW`, with `chromium_key`.
    fn read_example(
        profile: &Profile,
        chromium_key: impl FnOnce(Browser) -> Result<ChromiumKey>,
    ) -> Result<Vec<Cookie>> {
        StoreCopy::of(profile)?.read(
            profile.browser,
            &candidate_domains("example.com"),
            NOW,
            chromium_key,
        )
    }

    fn no_key(_: Browser) -> Result<ChromiumKey> {
        panic!("the key must not be requested")
    }

    #[test]
    fn decrypts_the_matching_cookies_with_the_browser_key() {
        let dir = PrivateDir::new().unwrap();
        create_db(dir.path(), "24", &[row(".example.com", "session", None)]);
        let mut asked_for = None;
        let cookies = read_example(&profile("dia", &dir), |browser| {
            asked_for = Some(browser);
            Ok(key())
        })
        .unwrap();
        assert_eq!(asked_for, Some(browser("dia")));
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0].value.expose(), "hello-world");
    }

    #[test]
    fn the_key_is_not_requested_without_a_matching_cookie() {
        let dir = PrivateDir::new().unwrap();
        create_db(dir.path(), "24", &[row(".other.com", "session", None)]);
        let cookies = read_example(&profile("dia", &dir), no_key).unwrap();
        assert!(cookies.is_empty());
    }

    #[test]
    fn firefox_values_are_read_without_a_key() {
        use crate::firefox::test_data;
        let dir = PrivateDir::new().unwrap();
        test_data::create_db(
            dir.path(),
            16,
            &[test_data::row(".example.com", "session", NOW + 60)],
        );
        let cookies = read_example(&profile("firefox", &dir), no_key).unwrap();
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0].value.expose(), "value-of-session");
    }

    #[test]
    fn safari_values_are_read_without_a_key() {
        use crate::safari::test_data::{Generated, file, page};
        let dir = PrivateDir::new().unwrap();
        let data = file(&[page(&[Generated::new(".example.com", "session", NOW + 60)])]);
        std::fs::write(
            dir.path().join(crate::safari::database::DATABASE_FILE),
            data,
        )
        .unwrap();
        let cookies = read_example(&profile("safari", &dir), no_key).unwrap();
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0].value.expose(), "value-of-session");
    }

    #[test]
    fn app_bound_values_are_reported_before_asking_for_the_key() {
        let dir = PrivateDir::new().unwrap();
        create_db(
            dir.path(),
            "24",
            &[row(".example.com", "session", None).value(b"v20sealed")],
        );
        let err = read_example(&profile("dia", &dir), no_key).unwrap_err();
        assert!(matches!(err, Error::AppBoundEncryption { browser } if browser == "Dia"));
    }

    #[test]
    fn scanning_reads_times_for_every_engine() {
        use crate::firefox::test_data;
        let chromium = PrivateDir::new().unwrap();
        create_db(chromium.path(), "24", &[row(".example.com", "a", None)]);
        let firefox = PrivateDir::new().unwrap();
        test_data::create_db(
            firefox.path(),
            16,
            &[test_data::row(".example.com", "a", NOW + 60)],
        );
        for (id, dir) in [("dia", &chromium), ("firefox", &firefox)] {
            assert!(has_database(browser(id).engine(), dir.path()), "{id}");
            let store = StoreCopy::of(&profile(id, dir)).unwrap();
            let domains = candidate_domains("example.com");
            assert_eq!(store.last_used_for(&domains, NOW).unwrap(), Some(NOW - 10));
            assert_eq!(store.last_active().unwrap(), Some(NOW - 10));
        }
    }

    #[test]
    fn a_denied_key_is_passed_on() {
        let dir = PrivateDir::new().unwrap();
        create_db(dir.path(), "24", &[row(".example.com", "session", None)]);
        let err = read_example(&profile("dia", &dir), |_| Err(Error::KeyAccessDenied)).unwrap_err();
        assert!(matches!(err, Error::KeyAccessDenied));
    }
}
