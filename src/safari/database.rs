//! Reads the cookies of one site from Safari's cookie file. Values are stored without
//! encryption, so no key is needed, but macOS protects the file: reading it needs the
//! user's permission, given by picking the file in a file dialog or by Full Disk Access.
//!
//! The file is read into memory that is cleared when dropped, never copied to disk.

use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use crate::domain::is_sent;
use crate::files::read_error;
use crate::safari::binary_cookies::{self, Record};
use crate::{Cookie, Error, Result, SecretValue};

/// The cookie file's name in Safari's cookie folder.
pub(crate) const DATABASE_FILE: &str = "Cookies.binarycookies";

/// The cookie file in Safari's cookie folder `profile_dir`.
fn database_path(profile_dir: &Path) -> PathBuf {
    profile_dir.join(DATABASE_FILE)
}

/// The content of a Safari cookie file, cleared from memory when dropped.
pub(crate) struct CookieFileData {
    /// The file's bytes.
    data: Zeroizing<Vec<u8>>,
}

impl CookieFileData {
    /// Whether `profile_dir` holds a Safari cookie file.
    pub(crate) fn has_database(profile_dir: &Path) -> bool {
        database_path(profile_dir).is_file()
    }

    /// Reads the cookie file in `profile_dir`. A file macOS does not let this app read is
    /// [`Error::PermissionNeeded`].
    pub(crate) fn of(profile_dir: &Path) -> Result<Self> {
        let path = database_path(profile_dir);
        let data = std::fs::read(&path).map_err(|e| read_error(&path, e))?;
        Ok(Self {
            data: Zeroizing::new(data),
        })
    }

    /// When an unexpired cookie stored under one of `domains` was created, the latest, as
    /// Unix seconds. Safari does not store when a cookie was last sent, so the creation
    /// time, renewed at each login, stands in for it.
    pub(crate) fn last_used_for(&self, domains: &[String], now_unix: i64) -> Result<Option<i64>> {
        Ok(self.matching(domains, now_unix)?.map(|r| r.created).max())
    }

    /// When the latest cookie, for any site, was created, as Unix seconds.
    pub(crate) fn last_active(&self) -> Result<Option<i64>> {
        Ok(binary_cookies::parse(&self.data)?
            .iter()
            .map(|r| r.created)
            .max())
    }

    /// The unexpired cookies stored under one of `domains`.
    pub(crate) fn read(&self, domains: &[String], now_unix: i64) -> Result<Vec<Cookie>> {
        self.matching(domains, now_unix)?
            .map(|r| {
                let value = std::str::from_utf8(r.value).map_err(|_| {
                    Error::DatabaseQuery(format!(
                        "cookie {:?} has a value that is not text",
                        r.name
                    ))
                })?;
                Ok(Cookie {
                    domain: r.domain.to_owned(),
                    name: r.name.to_owned(),
                    value: SecretValue::new(value.to_owned()),
                    path: r.path.to_owned(),
                    secure: r.secure,
                    http_only: r.http_only,
                    expires: Some(r.expires),
                })
            })
            .collect()
    }

    /// The unexpired cookies stored under one of `domains`, still as stored.
    fn matching<'a>(
        &'a self,
        domains: &'a [String],
        now_unix: i64,
    ) -> Result<impl Iterator<Item = Record<'a>>> {
        Ok(binary_cookies::parse(&self.data)?
            .into_iter()
            .filter(move |r| is_sent(r.domain, Some(r.expires), domains, now_unix)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::candidate_domains;
    use crate::private_dir::PrivateDir;
    use crate::safari::test_data::{Generated, NOW, file, page};

    /// A Safari cookie folder holding a generated file with `cookies`.
    fn folder(cookies: Vec<Generated>) -> PrivateDir {
        let dir = PrivateDir::new().unwrap();
        std::fs::write(dir.path().join(DATABASE_FILE), file(&[page(&cookies)])).unwrap();
        dir
    }

    #[test]
    fn reads_only_unexpired_cookies_of_the_requested_site() {
        let dir = folder(vec![
            Generated::new(".example.com", "session", NOW + 60).flags(5),
            Generated::new("www.example.com", "host-only", NOW + 60).created(NOW - 5),
            Generated::new(".example.com", "expired", NOW - 60),
            Generated::new("other.com", "foreign", NOW + 60).created(NOW - 1),
        ]);
        assert!(CookieFileData::has_database(dir.path()));
        let data = CookieFileData::of(dir.path()).unwrap();
        let domains = candidate_domains("www.example.com");

        assert_eq!(data.last_used_for(&domains, NOW).unwrap(), Some(NOW - 5));
        assert_eq!(data.last_active().unwrap(), Some(NOW - 1));
        let mut cookies = data.read(&domains, NOW).unwrap();
        cookies.sort_by(|a, b| a.name.cmp(&b.name));
        let names: Vec<_> = cookies.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["host-only", "session"]);
        assert_eq!(cookies[1].value.expose(), "value-of-session");
        assert_eq!(cookies[1].expires, Some(NOW + 60));
        assert!(cookies[1].secure && cookies[1].http_only);
    }

    #[test]
    fn a_site_without_cookies_has_no_login() {
        let dir = folder(vec![Generated::new(".other.com", "a", NOW + 60)]);
        let data = CookieFileData::of(dir.path()).unwrap();
        let domains = candidate_domains("example.com");
        assert_eq!(data.last_used_for(&domains, NOW).unwrap(), None);
        assert!(data.read(&domains, NOW).unwrap().is_empty());
    }

    #[test]
    fn a_missing_file_is_its_own_error() {
        let dir = PrivateDir::new().unwrap();
        assert!(!CookieFileData::has_database(dir.path()));
        assert!(matches!(
            CookieFileData::of(dir.path()),
            Err(Error::DatabaseNotFound(_))
        ));
    }

    #[test]
    fn a_folder_in_place_of_the_file_is_unreadable() {
        let dir = PrivateDir::new().unwrap();
        std::fs::create_dir(dir.path().join(DATABASE_FILE)).unwrap();
        assert!(CookieFileData::of(dir.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_file_this_app_may_not_read_needs_permission() {
        use std::os::unix::fs::PermissionsExt;
        let dir = folder(vec![]);
        let path = dir.path().join(DATABASE_FILE);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        let result = CookieFileData::of(dir.path());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(matches!(result, Err(Error::PermissionNeeded(p)) if p == path));
    }
}
