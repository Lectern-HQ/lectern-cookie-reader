//! Tests of `CookieFile`: where it is written, who can read it, and that it is always
//! deleted. Unix only until Windows access lists are added (issue #2).
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use lectern_cookie_reader::{Cookie, CookieFile, SecretValue};

fn cookies() -> Vec<Cookie> {
    vec![Cookie {
        domain: ".example.com".into(),
        name: "session".into(),
        value: SecretValue::new("generated-test-value".into()),
        path: "/".into(),
        secure: true,
        http_only: true,
        expires: None,
    }]
}

/// Stands in for a downloader run that fails.
fn failing_download(file: &CookieFile) -> Result<(), String> {
    assert!(file.path().exists());
    Err("download failed".into())
}

#[test]
fn the_file_holds_the_cookies_and_only_the_user_can_read_it() {
    let file = CookieFile::create(&cookies()).unwrap();
    let text = std::fs::read_to_string(file.path()).unwrap();
    assert!(text.contains("\tsession\tgenerated-test-value\n"));
    let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(file.path()), 0o600);
    assert_eq!(mode(file.path().parent().unwrap()), 0o700);
}

#[test]
fn the_file_and_its_folder_are_gone_after_a_failed_download() {
    let path: PathBuf;
    {
        let file = CookieFile::create(&cookies()).unwrap();
        path = file.path().to_owned();
        assert!(failing_download(&file).is_err());
    }
    assert!(!path.exists());
    assert!(!path.parent().unwrap().exists());
}

#[test]
fn the_file_is_gone_after_a_panic() {
    let mut path = None;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let file = CookieFile::create(&cookies()).unwrap();
        path = Some(file.path().to_owned());
        panic!("simulated cancel");
    }));
    assert!(result.is_err());
    assert!(!path.unwrap().exists());
}

#[test]
fn two_files_never_share_a_folder() {
    let a = CookieFile::create(&cookies()).unwrap();
    let b = CookieFile::create(&cookies()).unwrap();
    assert_ne!(a.path().parent(), b.path().parent());
}
