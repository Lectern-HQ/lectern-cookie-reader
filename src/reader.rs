//! The public lookup: the cookies of one website in one browser profile.

use zeroize::Zeroizing;

use crate::chromium::database::{self, CookieDb};
use crate::clock::now_unix;
use crate::domain::{candidate_domains, normalize_host};
use crate::private_dir::PrivateDir;
use crate::{Browser, Cookie, Engine, Error, Profile, Result, platform};

/// Reads the unexpired cookies that a browser would send to `host`.
///
/// `host` is a plain host name such as `mediaserver.unige.ch`, not a link.
///
/// The steps, in order:
/// 1. The profile's cookie database is copied into a folder only the user can open,
///    removed again before this function returns.
/// 2. If no cookie for `host` is stored, an empty list is returned. The browser's key is
///    not requested, so the user sees no permission dialog for a site they are not
///    logged in to.
/// 3. The browser's key is requested from the operating system (on macOS the Keychain,
///    which may show a dialog), and only the matching cookies are decrypted.
pub fn read_cookies(profile: &Profile, host: &str) -> Result<Vec<Cookie>> {
    let domains = candidate_domains(&normalize_host(host)?);
    match profile.browser.engine() {
        Engine::Chromium => read_chromium(profile, &domains, now_unix(), platform::chromium_key),
        Engine::Firefox => Err(Error::Unsupported(format!(
            "reading {} profiles",
            profile.browser.name()
        ))),
    }
}

/// `chromium_key` is a parameter so tests can run every step with a test key.
fn read_chromium(
    profile: &Profile,
    domains: &[String],
    now_unix: i64,
    chromium_key: impl FnOnce(Browser) -> Result<Zeroizing<[u8; 16]>>,
) -> Result<Vec<Cookie>> {
    let folder = PrivateDir::new()?;
    let db = CookieDb::open(&database::copy_database(&profile.path, folder.path())?)?;
    if db.last_used_for(domains, now_unix)?.is_none() {
        return Ok(Vec::new());
    }
    let key = chromium_key(profile.browser)?;
    db.read(domains, &key, now_unix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::tests::browser;
    use crate::chromium::test_vectors::{NOW, create_db, key, row};

    fn dia_profile(dir: &PrivateDir) -> Profile {
        Profile {
            browser: browser("dia"),
            name: "Test".into(),
            path: dir.path().to_owned(),
        }
    }

    #[test]
    fn decrypts_the_matching_cookies_with_the_browser_key() {
        let dir = PrivateDir::new().unwrap();
        create_db(dir.path(), "24", &[row(".example.com", "session", None)]);
        let mut asked_for = None;
        let cookies = read_chromium(
            &dia_profile(&dir),
            &candidate_domains("example.com"),
            NOW,
            |browser| {
                asked_for = Some(browser);
                Ok(Zeroizing::new(key()))
            },
        )
        .unwrap();
        assert_eq!(asked_for, Some(browser("dia")));
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0].value.expose(), "hello-world");
    }

    #[test]
    fn the_key_is_not_requested_without_a_matching_cookie() {
        let dir = PrivateDir::new().unwrap();
        create_db(dir.path(), "24", &[row(".other.com", "session", None)]);
        let cookies = read_chromium(
            &dia_profile(&dir),
            &candidate_domains("example.com"),
            NOW,
            |_| panic!("the key must not be requested"),
        )
        .unwrap();
        assert!(cookies.is_empty());
    }

    #[test]
    fn a_denied_key_is_passed_on() {
        let dir = PrivateDir::new().unwrap();
        create_db(dir.path(), "24", &[row(".example.com", "session", None)]);
        let err = read_chromium(
            &dia_profile(&dir),
            &candidate_domains("example.com"),
            NOW,
            |_| Err(Error::KeyAccessDenied),
        )
        .unwrap_err();
        assert!(matches!(err, Error::KeyAccessDenied));
    }
}
