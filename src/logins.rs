//! Which browser profiles are logged in to a site. Only site names and the time each
//! cookie was last used are read: nothing is decrypted and the operating system shows no
//! permission dialog.

use std::cmp::Reverse;
use std::path::PathBuf;

use crate::browser::profiles_in;
use crate::chromium::database::{self, CookieDb};
use crate::clock::now_unix;
use crate::domain::{candidate_domains, normalize_host};
use crate::private_dir::PrivateDir;
use crate::{Browser, Error, Profile, Result, platform};

/// A profile with an unexpired cookie for the site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Login {
    /// The profile that has a login for the site.
    pub profile: Profile,
    /// When the browser last sent one of the site's cookies, as Unix seconds.
    pub last_used: i64,
}

/// A browser or profile that could not be checked. The others are still checked.
#[derive(Debug)]
pub struct Skipped {
    /// The browser that was being checked.
    pub browser: Browser,
    /// The browser's data folder, or the profile's folder.
    pub path: PathBuf,
    /// For example macOS refusing access to another app's folder.
    pub error: Error,
}

/// A profile that was checked, logged in to the site or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    /// The profile that was checked.
    pub profile: Profile,
    /// When the browser last sent any cookie, for any site, as Unix seconds.
    pub last_active: Option<i64>,
}

/// The result of [`find_logins`].
#[derive(Debug)]
pub struct Scan {
    /// Profiles logged in to the site, most recently used first.
    pub logins: Vec<Login>,
    /// Every profile that could be checked, in the order checked.
    pub checked: Vec<Checked>,
    /// Browsers and profiles that could not be checked, with the reason.
    pub skipped: Vec<Skipped>,
}

/// Looks in every profile of every known browser for a login to `host`.
///
/// `host` is a plain host name such as `mediaserver.unige.ch`, not a link.
///
/// Fails only for an invalid host or an operating system this crate does not support yet.
/// A browser or profile that cannot be read is listed in [`Scan::skipped`].
pub fn find_logins(host: &str) -> Result<Scan> {
    let domains = candidate_domains(&normalize_host(host)?);
    let data_dirs = Browser::ALL
        .into_iter()
        .map(|browser| Ok((browser, platform::user_data_dir(browser)?)))
        .collect::<Result<Vec<_>>>()?;
    Ok(find_in(&data_dirs, &domains, now_unix()))
}

/// Lists the profiles in each browser data folder, then checks them.
fn find_in(data_dirs: &[(Browser, PathBuf)], domains: &[String], now_unix: i64) -> Scan {
    let mut profiles = Vec::new();
    let mut skipped = Vec::new();
    for (browser, data_dir) in data_dirs {
        match profiles_in(*browser, data_dir) {
            Ok(found) => profiles.extend(found),
            Err(error) => skipped.push(Skipped {
                browser: *browser,
                path: data_dir.clone(),
                error,
            }),
        }
    }
    let mut result = scan(&profiles, domains, now_unix);
    skipped.append(&mut result.skipped);
    result.skipped = skipped;
    result
}

/// Checks each profile: whether it has a login for the site and when it was last used.
fn scan(profiles: &[Profile], domains: &[String], now_unix: i64) -> Scan {
    let mut logins = Vec::new();
    let mut checked = Vec::new();
    let mut skipped = Vec::new();
    for profile in profiles {
        match check(profile, domains, now_unix) {
            Ok((last_used, last_active)) => {
                if let Some(last_used) = last_used {
                    logins.push(Login {
                        profile: profile.clone(),
                        last_used,
                    });
                }
                checked.push(Checked {
                    profile: profile.clone(),
                    last_active,
                });
            }
            Err(error) => skipped.push(Skipped {
                browser: profile.browser,
                path: profile.path.clone(),
                error,
            }),
        }
    }
    logins.sort_by_key(|login| Reverse(login.last_used));
    Scan {
        logins,
        checked,
        skipped,
    }
}

/// When the profile last used the site, and when it last used any site.
fn check(
    profile: &Profile,
    domains: &[String],
    now_unix: i64,
) -> Result<(Option<i64>, Option<i64>)> {
    let folder = PrivateDir::new()?;
    let db = CookieDb::open(&database::copy_database(&profile.path, folder.path())?)?;
    Ok((db.last_used_for(domains, now_unix)?, db.last_active()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chromium::test_vectors::{NOW, create_db, row};

    fn profile(dir: &PrivateDir, browser: Browser) -> Profile {
        Profile {
            browser,
            name: "Default".into(),
            path: dir.path().to_owned(),
        }
    }

    #[test]
    fn logged_in_profiles_come_most_recently_used_first() {
        let dia = PrivateDir::new().unwrap();
        let chrome = PrivateDir::new().unwrap();
        let none = PrivateDir::new().unwrap();
        create_db(
            dia.path(),
            "24",
            &[row(".example.com", "a", None).last_used(NOW - 3600)],
        );
        create_db(
            chrome.path(),
            "24",
            &[row("example.com", "b", None).last_used(NOW - 60)],
        );
        create_db(none.path(), "24", &[row(".other.com", "c", None)]);
        let profiles = [
            profile(&dia, Browser::Dia),
            profile(&chrome, Browser::Chrome),
            profile(&none, Browser::Brave),
        ];

        let result = scan(&profiles, &candidate_domains("example.com"), NOW);

        let found: Vec<_> = result
            .logins
            .iter()
            .map(|l| (l.profile.browser, l.last_used))
            .collect();
        assert_eq!(
            found,
            [(Browser::Chrome, NOW - 60), (Browser::Dia, NOW - 3600)]
        );
        assert!(result.skipped.is_empty());
    }

    #[test]
    fn expired_cookies_are_not_a_login() {
        let dir = PrivateDir::new().unwrap();
        create_db(dir.path(), "24", &[row(".example.com", "a", Some(NOW - 1))]);
        let result = scan(
            &[profile(&dir, Browser::Dia)],
            &candidate_domains("example.com"),
            NOW,
        );
        assert!(result.logins.is_empty());
    }

    #[test]
    fn a_profile_that_cannot_be_read_is_reported_and_the_others_still_checked() {
        let newer = PrivateDir::new().unwrap();
        let good = PrivateDir::new().unwrap();
        create_db(newer.path(), "99", &[row(".example.com", "a", None)]);
        create_db(good.path(), "24", &[row(".example.com", "a", None)]);
        let profiles = [
            profile(&newer, Browser::Dia),
            profile(&good, Browser::Chrome),
        ];

        let result = scan(&profiles, &candidate_domains("example.com"), NOW);

        assert_eq!(result.logins.len(), 1);
        assert_eq!(result.skipped.len(), 1);
        assert_eq!(result.skipped[0].path, newer.path());
        assert!(matches!(
            result.skipped[0].error,
            Error::UnsupportedDatabaseVersion(99)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_browser_folder_is_skipped_and_the_others_still_checked() {
        let dia = PrivateDir::new().unwrap();
        std::fs::create_dir(dia.path().join("Default")).unwrap();
        create_db(
            &dia.path().join("Default"),
            "24",
            &[row(".example.com", "a", None)],
        );
        let not_a_folder = dia.path().join("Default").join("Cookies");
        let missing = dia.path().join("missing");
        let data_dirs = [
            (Browser::Chrome, not_a_folder.clone()),
            (Browser::Dia, dia.path().to_owned()),
            (Browser::Brave, missing),
        ];

        let result = find_in(&data_dirs, &candidate_domains("example.com"), NOW);

        assert_eq!(result.logins.len(), 1);
        assert_eq!(result.logins[0].profile.browser, Browser::Dia);
        assert_eq!(result.skipped.len(), 1);
        assert_eq!(result.skipped[0].browser, Browser::Chrome);
        assert_eq!(result.skipped[0].path, not_a_folder);
    }

    #[test]
    fn a_link_is_not_a_host() {
        assert!(matches!(
            find_logins("https://example.com/play/1"),
            Err(Error::InvalidHost(_))
        ));
    }
}
