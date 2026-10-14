//! Tests of `read_cookies` on generated profiles. None of them reaches the step that asks
//! the operating system for the browser's key.

use std::path::{Path, PathBuf};

use lectern_cookie_reader::{Browser, Error, Profile, read_cookies};

/// A generated profile folder, removed when dropped.
struct TestProfile(PathBuf);

impl TestProfile {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "lectern-cookie-reader-test-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn with_database(name: &str, version: &str, host_keys: &[&str]) -> Self {
        let profile = Self::new(name);
        let conn = rusqlite::Connection::open(profile.0.join("Cookies")).unwrap();
        conn.execute_batch(include_str!("fixtures/chromium_cookies.sql"))
            .unwrap();
        conn.execute("INSERT INTO meta VALUES ('version', ?1)", [version])
            .unwrap();
        for host_key in host_keys {
            conn.execute(
                "INSERT INTO cookies VALUES (?1, 'session', '/', 1, 1, 0, 0, 0, x'00')",
                [host_key],
            )
            .unwrap();
        }
        profile
    }

    fn profile(&self) -> Profile {
        Profile {
            browser: Browser::Dia,
            name: "Test".into(),
            path: self.0.clone(),
        }
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestProfile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn no_login_for_the_site_gives_an_empty_list_without_asking_for_the_key() {
    let profile = TestProfile::with_database("empty", "24", &[".other.com"]);
    let cookies = read_cookies(&profile.profile(), "mediaserver.unige.ch").unwrap();
    assert!(cookies.is_empty());
}

#[test]
fn the_browser_database_is_left_unchanged() {
    let profile = TestProfile::with_database("unchanged", "24", &[".other.com"]);
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
    let profile = TestProfile::with_database("link", "24", &[]);
    let err = read_cookies(&profile.profile(), "https://mediaserver.unige.ch/play/1").unwrap_err();
    assert!(matches!(err, Error::InvalidHost(_)));
}

#[test]
fn a_profile_without_a_database_is_reported() {
    let profile = TestProfile::new("missing");
    let err = read_cookies(&profile.profile(), "example.com").unwrap_err();
    assert!(matches!(err, Error::DatabaseNotFound(_)));
}

#[test]
fn a_newer_database_version_is_refused_before_asking_for_the_key() {
    let profile = TestProfile::with_database("newer", "99", &[".example.com"]);
    let err = read_cookies(&profile.profile(), "example.com").unwrap_err();
    assert!(matches!(err, Error::UnsupportedDatabaseVersion(99)));
}
