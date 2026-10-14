//! The best guess for which browser profile the user watches lectures in. Lectern shows
//! it already selected and asks the user to confirm; it is never used silently.

use crate::{DefaultBrowser, Profile, Scan};

/// Why a profile was suggested, strongest reason first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// It has a login for the site (the most recently used one if several do).
    LoggedIn,
    /// No profile is logged in, and this is the system's default browser.
    DefaultBrowser,
    /// No profile is logged in and the default browser cannot be read: this profile was
    /// used most recently.
    RecentlyUsed,
}

/// A profile to show already selected, and why it was chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// The suggested profile.
    pub profile: Profile,
    /// Why it was suggested.
    pub reason: Reason,
}

/// Picks one profile from a [`Scan`]: a profile logged in to the site, else the default
/// browser's first profile, else the most recently used profile. `None` when no profile
/// could be checked.
///
/// The default browser alone is a weak guess: a Mac may open links in Safari while the
/// user watches lectures in another browser. That is why it only counts when no profile
/// is logged in.
pub fn suggest(scan: &Scan, default: Option<&DefaultBrowser>) -> Option<Suggestion> {
    if let Some(login) = scan.logins.first() {
        return Some(suggestion(&login.profile, Reason::LoggedIn));
    }
    let default_browser = default.and_then(|d| d.browser);
    if let Some(checked) = scan
        .checked
        .iter()
        .find(|c| Some(c.profile.browser) == default_browser)
    {
        return Some(suggestion(&checked.profile, Reason::DefaultBrowser));
    }
    scan.checked
        .iter()
        .filter(|c| c.last_active.is_some())
        .max_by_key(|c| c.last_active)
        .map(|c| suggestion(&c.profile, Reason::RecentlyUsed))
}

/// A suggestion of `profile` for `reason`.
fn suggestion(profile: &Profile, reason: Reason) -> Suggestion {
    Suggestion {
        profile: profile.clone(),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Browser, Checked, Login};
    use std::path::PathBuf;

    fn profile(browser: Browser, name: &str) -> Profile {
        Profile {
            browser,
            name: name.into(),
            path: PathBuf::from(format!("/profiles/{}/{name}", browser.name())),
        }
    }

    fn checked(browser: Browser, name: &str, last_active: Option<i64>) -> Checked {
        Checked {
            profile: profile(browser, name),
            last_active,
        }
    }

    fn scan(logins: Vec<Login>, checked: Vec<Checked>) -> Scan {
        Scan {
            logins,
            checked,
            skipped: Vec::new(),
        }
    }

    fn safari() -> DefaultBrowser {
        DefaultBrowser {
            name: "Safari".into(),
            browser: None,
        }
    }

    fn chrome_default() -> DefaultBrowser {
        DefaultBrowser {
            name: "Google Chrome".into(),
            browser: Some(Browser::Chrome),
        }
    }

    #[test]
    fn a_login_for_the_site_wins_over_the_default_browser() {
        let result = suggest(
            &scan(
                vec![Login {
                    profile: profile(Browser::Dia, "Default"),
                    last_used: 100,
                }],
                vec![
                    checked(Browser::Dia, "Default", Some(100)),
                    checked(Browser::Chrome, "Default", Some(900)),
                ],
            ),
            Some(&chrome_default()),
        )
        .unwrap();
        assert_eq!(result.profile.browser, Browser::Dia);
        assert_eq!(result.reason, Reason::LoggedIn);
    }

    #[test]
    fn without_a_login_the_default_browser_is_next() {
        let result = suggest(
            &scan(
                Vec::new(),
                vec![
                    checked(Browser::Dia, "Default", Some(900)),
                    checked(Browser::Chrome, "Default", Some(100)),
                    checked(Browser::Chrome, "Profile 1", Some(500)),
                ],
            ),
            Some(&chrome_default()),
        )
        .unwrap();
        assert_eq!(result.profile, profile(Browser::Chrome, "Default"));
        assert_eq!(result.reason, Reason::DefaultBrowser);
    }

    #[test]
    fn an_unreadable_default_browser_falls_back_to_the_most_recently_used_profile() {
        let result = suggest(
            &scan(
                Vec::new(),
                vec![
                    checked(Browser::Chrome, "Default", Some(100)),
                    checked(Browser::Dia, "Default", Some(900)),
                    checked(Browser::Brave, "Default", None),
                ],
            ),
            Some(&safari()),
        )
        .unwrap();
        assert_eq!(result.profile.browser, Browser::Dia);
        assert_eq!(result.reason, Reason::RecentlyUsed);
    }

    #[test]
    fn a_default_browser_with_no_profile_here_is_skipped() {
        let result = suggest(
            &scan(Vec::new(), vec![checked(Browser::Dia, "Default", Some(1))]),
            Some(&chrome_default()),
        )
        .unwrap();
        assert_eq!(result.reason, Reason::RecentlyUsed);
    }

    #[test]
    fn nothing_to_suggest_without_any_used_profile() {
        assert_eq!(suggest(&scan(Vec::new(), Vec::new()), None), None);
        let unused = scan(Vec::new(), vec![checked(Browser::Dia, "Default", None)]);
        assert_eq!(suggest(&unused, Some(&safari())), None);
    }
}
