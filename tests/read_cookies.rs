//! Tests of `read_cookies` on generated profiles of each SQLite engine. None of them
//! reaches the step that asks the operating system for the browser's key.

use std::path::{Path, PathBuf};

use lectern_cookie_reader::{Browser, CookieFile, Error, Profile, read_cookies};

/// An expiry far in the future: 3000-01-01, in milliseconds, as Firefox stores it.
const FAR_FUTURE_MS: i64 = 32_503_680_000_000;

/// A generated profile folder of one browser, removed when dropped.
struct TestProfile {
    /// The browser the folder is a profile of.
    browser: &'static str,
    /// The folder.
    path: PathBuf,
}

impl TestProfile {
    /// An empty profile folder of `browser`, unique to this test (`name`).
    fn empty(browser: &'static str, name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "lectern-cookie-reader-test-{browser}-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();
        Self { browser, path }
    }

    /// A Dia profile with one `session` cookie per host, in a database of `version`.
    fn chromium(name: &str, version: &str, hosts: &[&str]) -> Self {
        let profile = Self::empty("dia", name);
        let conn = rusqlite::Connection::open(profile.path.join("Cookies")).unwrap();
        conn.execute_batch(include_str!("fixtures/chromium_cookies.sql"))
            .unwrap();
        conn.execute("INSERT INTO meta VALUES ('version', ?1)", [version])
            .unwrap();
        for host in hosts {
            conn.execute(
                "INSERT INTO cookies (
                    creation_utc, host_key, top_frame_site_key, name, value, encrypted_value,
                    path, expires_utc, is_secure, is_httponly, last_access_utc, has_expires,
                    is_persistent, priority, samesite, source_scheme, source_port,
                    last_update_utc, source_type, has_cross_site_ancestor)
                 VALUES (0, ?1, '', 'session', '', x'00', '/', 0, 1, 1, 0, 0, 0, 1, -1, 2,
                         443, 0, 0, 0)",
                [host],
            )
            .unwrap();
        }
        profile
    }

    /// A Firefox profile with one unexpired `session` cookie per host, schema version 17.
    fn firefox(name: &str, hosts: &[&str]) -> Self {
        let profile = Self::empty("firefox", name);
        let conn = rusqlite::Connection::open(profile.path.join("cookies.sqlite")).unwrap();
        conn.execute_batch(include_str!("fixtures/firefox_cookies.sql"))
            .unwrap();
        conn.pragma_update(None, "user_version", 17).unwrap();
        for host in hosts {
            conn.execute(
                "INSERT INTO moz_cookies (name, value, host, path, expiry, lastAccessed,
                                          isSecure, isHttpOnly)
                 VALUES ('session', 'generated', ?1, '/', ?2, 0, 1, 1)",
                rusqlite::params![host, FAR_FUTURE_MS],
            )
            .unwrap();
        }
        profile
    }

    /// The profile, as the library sees it.
    fn profile(&self) -> Profile {
        Profile {
            browser: Browser::by_id(self.browser).unwrap(),
            name: "Test".into(),
            path: self.path.clone(),
        }
    }

    /// The folder.
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestProfile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn no_login_for_the_site_gives_an_empty_list_without_asking_for_a_key() {
    for profile in [
        TestProfile::chromium("empty", "24", &[".other.com"]),
        TestProfile::firefox("empty", &[".other.com"]),
    ] {
        let cookies = read_cookies(&profile.profile(), "mediaserver.unige.ch").unwrap();
        assert!(cookies.is_empty(), "{}", profile.browser);
    }
}

#[test]
fn firefox_cookies_are_read_and_written_to_the_cookie_file() {
    let profile = TestProfile::firefox("site", &[".unige.ch", ".other.com"]);
    let cookies = read_cookies(&profile.profile(), "mediaserver.unige.ch").unwrap();
    assert_eq!(cookies.len(), 1);
    assert_eq!(cookies[0].domain, ".unige.ch");
    assert_eq!(cookies[0].value.expose(), "generated");
    assert!(CookieFile::create(&cookies).is_ok());
}

#[test]
fn the_browser_database_is_left_unchanged() {
    let profile = TestProfile::chromium("unchanged", "24", &[".other.com"]);
    let before = std::fs::read(profile.path().join("Cookies")).unwrap();
    read_cookies(&profile.profile(), "example.com").unwrap();
    assert_eq!(
        std::fs::read(profile.path().join("Cookies")).unwrap(),
        before
    );
    assert!(!profile.path().join("Cookies-wal").exists());
}

#[test]
fn a_link_instead_of_a_host_is_refused() {
    let profile = TestProfile::chromium("link", "24", &[]);
    let err = read_cookies(&profile.profile(), "https://mediaserver.unige.ch/play/1").unwrap_err();
    assert!(matches!(err, Error::InvalidHost(_)));
}

#[test]
fn a_profile_without_a_database_is_reported() {
    for browser in ["dia", "firefox"] {
        let profile = TestProfile::empty(browser, "missing");
        let err = read_cookies(&profile.profile(), "example.com").unwrap_err();
        assert!(matches!(err, Error::DatabaseNotFound(_)), "{browser}");
    }
}

#[test]
fn a_newer_database_version_is_refused_before_asking_for_the_key() {
    let profile = TestProfile::chromium("newer", "99", &[".example.com"]);
    let err = read_cookies(&profile.profile(), "example.com").unwrap_err();
    assert!(matches!(err, Error::UnsupportedDatabaseVersion(99)));
}
