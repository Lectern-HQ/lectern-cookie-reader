//! Tests of `read_cookies` on generated Firefox profiles: values are read without a key.

use std::path::PathBuf;

use lectern_cookie_reader::{Browser, CookieFile, Profile, read_cookies};

/// A generated Firefox profile folder, removed when dropped.
struct FirefoxProfile(PathBuf);

impl FirefoxProfile {
    /// A profile with one unexpired cookie per host, in a schema version 16 database.
    fn with_cookies(name: &str, hosts: &[&str]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "lectern-cookie-reader-firefox-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();
        let conn = rusqlite::Connection::open(path.join("cookies.sqlite")).unwrap();
        conn.execute_batch(include_str!("fixtures/firefox_cookies.sql"))
            .unwrap();
        conn.pragma_update(None, "user_version", 16).unwrap();
        for host in hosts {
            conn.execute(
                "INSERT INTO moz_cookies (name, value, host, path, expiry, lastAccessed,
                                          isSecure, isHttpOnly)
                 VALUES ('session', 'generated', ?1, '/', 32503680000000, 0, 1, 1)",
                [host],
            )
            .unwrap();
        }
        Self(path)
    }

    fn profile(&self) -> Profile {
        Profile {
            browser: Browser::by_id("firefox").unwrap(),
            name: "default-release".into(),
            path: self.0.clone(),
        }
    }
}

impl Drop for FirefoxProfile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn reads_the_sites_cookies_and_writes_the_cookie_file() {
    let profile = FirefoxProfile::with_cookies("site", &[".unige.ch", ".other.com"]);
    let cookies = read_cookies(&profile.profile(), "mediaserver.unige.ch").unwrap();
    assert_eq!(cookies.len(), 1);
    assert_eq!(cookies[0].domain, ".unige.ch");
    assert_eq!(cookies[0].value.expose(), "generated");
    assert!(CookieFile::create(&cookies).is_ok());
}

#[test]
fn no_login_for_the_site_gives_an_empty_list() {
    let profile = FirefoxProfile::with_cookies("empty", &[".other.com"]);
    assert!(
        read_cookies(&profile.profile(), "example.com")
            .unwrap()
            .is_empty()
    );
}
