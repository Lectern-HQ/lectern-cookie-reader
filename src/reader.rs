//! The public lookup: the cookies of one website in one browser profile.

use zeroize::Zeroizing;

use crate::chromium;
use crate::clock::now_unix;
use crate::domain::domains_for;
use crate::firefox;
use crate::{Browser, Cookie, Engine, Profile, Result, platform};

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
/// 3. For Chromium browsers, the browser's key is requested from the operating system (on
///    macOS the Keychain, which may show a dialog), and only the matching cookies are
///    decrypted. The Firefox family stores values without encryption, so there is no key
///    and no dialog.
pub fn read_cookies(profile: &Profile, host: &str) -> Result<Vec<Cookie>> {
    let domains = domains_for(host)?;
    match profile.browser.engine() {
        Engine::Chromium => read_chromium(profile, &domains, now_unix(), platform::chromium_key),
        Engine::Firefox => read_firefox(profile, &domains, now_unix()),
    }
}

/// `chromium_key` is a parameter so tests can run every step with a test key.
fn read_chromium(
    profile: &Profile,
    domains: &[String],
    now_unix: i64,
    chromium_key: impl FnOnce(Browser) -> Result<Zeroizing<[u8; 16]>>,
) -> Result<Vec<Cookie>> {
    let copy = chromium::database::ProfileCopy::of(&profile.path)?;
    if copy.db().last_used_for(domains, now_unix)?.is_none() {
        return Ok(Vec::new());
    }
    let key = chromium_key(profile.browser)?;
    copy.db().read(domains, &key, now_unix)
}

/// Firefox family values are stored without encryption: no key, no permission dialog.
fn read_firefox(profile: &Profile, domains: &[String], now_unix: i64) -> Result<Vec<Cookie>> {
    firefox::database::ProfileCopy::of(&profile.path)?
        .db()
        .read(domains, now_unix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;
    use crate::browser::tests::browser;
    use crate::chromium::test_vectors::{NOW, create_db, key, row};
    use crate::domain::candidate_domains;
    use crate::private_dir::PrivateDir;

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
    fn firefox_values_are_read_without_a_key() {
        use crate::firefox::test_data;
        let dir = PrivateDir::new().unwrap();
        test_data::create_db(
            dir.path(),
            16,
            &[test_data::row(".example.com", "session", NOW + 60)],
        );
        let profile = Profile {
            browser: browser("firefox"),
            name: "default-release".into(),
            path: dir.path().to_owned(),
        };
        let cookies = read_firefox(&profile, &candidate_domains("example.com"), NOW).unwrap();
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0].value.expose(), "value-of-session");
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
