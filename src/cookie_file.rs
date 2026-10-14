//! The cookie file a downloader such as yt-dlp reads (`--cookies`), in Netscape format.
//!
//! It is the most sensitive file Lectern creates, so it is written into a folder only the
//! user can open, readable only by the user, and deleted with its folder when the
//! [`CookieFile`] goes out of scope: after success, after an error and after a panic.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use crate::private_dir::{PrivateDir, work_folder_error};
use crate::{Cookie, Error, Result};

/// The file's name in its private folder.
const FILE_NAME: &str = "cookies.txt";
/// The first line of a Netscape cookie file, which readers check for.
const HEADER: &str = "# Netscape HTTP Cookie File\n";

/// A temporary `cookies.txt` that is deleted when dropped.
///
/// ```no_run
/// # fn main() -> lectern_cookie_reader::Result<()> {
/// use lectern_cookie_reader::{Browser, CookieFile, read_cookies};
///
/// let profile = Browser::Dia.default_profile()?;
/// let cookies = read_cookies(&profile, "mediaserver.unige.ch")?;
/// let file = CookieFile::create(&cookies)?;
/// // Run the downloader with `--cookies <file.path()>` here.
/// drop(file); // The file is deleted here, or when `file` goes out of scope.
/// # Ok(())
/// # }
/// ```
pub struct CookieFile {
    /// Where the file is.
    path: PathBuf,
    // Dropping the folder deletes the file.
    /// The private folder holding the file; dropping it deletes both.
    _folder: PrivateDir,
}

impl CookieFile {
    /// Writes `cookies` to a new file in a new private temporary folder.
    pub fn create(cookies: &[Cookie]) -> Result<Self> {
        let content = to_netscape(cookies)?;
        ensure_private_files_supported()?;
        let folder = PrivateDir::new()?;
        let path = folder.path().join(FILE_NAME);
        let mut file = create_owner_only(&path).map_err(|e| work_folder_error(&path, e))?;
        file.write_all(content.as_bytes())
            .map_err(|e| work_folder_error(&path, e))?;
        Ok(Self {
            path,
            _folder: folder,
        })
    }

    /// Where the file is, for the downloader's `--cookies` option.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// `create_new` refuses an existing file, so nothing prepared in advance is written to.
fn create_owner_only(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// Refuses to write a cookie file where it cannot be made readable only by the user.
#[cfg(unix)]
fn ensure_private_files_supported() -> Result<()> {
    Ok(())
}

/// On Windows the file must be limited to the current user with an access list first.
#[cfg(not(unix))]
fn ensure_private_files_supported() -> Result<()> {
    Err(Error::Unsupported(
        "a cookie file only the current user can read, on this operating system".into(),
    ))
}

/// Netscape format: one line per cookie with seven tab-separated fields.
///
/// The `#HttpOnly_` prefix that some tools write is left out: Python's standard cookie
/// reader treats such lines as comments, and the cookies work without it.
fn to_netscape(cookies: &[Cookie]) -> Result<Zeroizing<String>> {
    let mut out = Zeroizing::new(String::from(HEADER));
    for cookie in cookies {
        check_fields(cookie)?;
        let line = Zeroizing::new(format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            cookie.domain,
            flag(cookie.domain.starts_with('.')),
            cookie.path,
            flag(cookie.secure),
            cookie.expires.unwrap_or(0),
            cookie.name,
            cookie.value.expose(),
        ));
        out.push_str(&line);
    }
    Ok(out)
}

/// A tab or line break inside a field would shift the columns or start a new line, so a
/// cookie for another site could be slipped into the file.
fn check_fields(cookie: &Cookie) -> Result<()> {
    let fields = [
        cookie.domain.as_str(),
        &cookie.path,
        &cookie.name,
        cookie.value.expose(),
    ];
    if fields
        .iter()
        .any(|field| field.contains(['\t', '\n', '\r']))
    {
        return Err(Error::InvalidCookie {
            cookie_name: cookie.name.escape_debug().to_string(),
        });
    }
    Ok(())
}

/// `TRUE` or `FALSE`, as the Netscape format writes them.
fn flag(value: bool) -> &'static str {
    if value { "TRUE" } else { "FALSE" }
}

impl std::fmt::Debug for CookieFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CookieFile")
            .field("path", &self.path)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SecretValue;

    fn cookie(domain: &str, name: &str, value: &str) -> Cookie {
        Cookie {
            domain: domain.into(),
            name: name.into(),
            value: SecretValue::new(value.into()),
            path: "/".into(),
            secure: true,
            http_only: true,
            expires: Some(1_900_000_000),
        }
    }

    #[test]
    fn writes_one_line_per_cookie_in_netscape_format() {
        let mut session = cookie("mediaserver.unige.ch", "csrftoken", "b");
        session.secure = false;
        session.expires = None;
        let text = to_netscape(&[cookie(".unige.ch", "sessionid", "a"), session]).unwrap();
        assert_eq!(
            text.as_str(),
            "# Netscape HTTP Cookie File\n\
             .unige.ch\tTRUE\t/\tTRUE\t1900000000\tsessionid\ta\n\
             mediaserver.unige.ch\tFALSE\t/\tFALSE\t0\tcsrftoken\tb\n"
        );
    }

    #[test]
    fn no_cookies_gives_only_the_header() {
        assert_eq!(to_netscape(&[]).unwrap().as_str(), HEADER);
    }

    #[test]
    fn a_line_break_in_a_value_is_refused_without_showing_the_value() {
        let err = to_netscape(&[cookie(
            ".unige.ch",
            "sessionid",
            "x\n.evil.com\tTRUE\t/\tTRUE\t0\tsteal\tme",
        )])
        .unwrap_err();
        assert!(matches!(&err, Error::InvalidCookie { cookie_name } if cookie_name == "sessionid"));
        assert!(!err.to_string().contains("evil"));
    }

    #[test]
    fn a_tab_in_any_field_is_refused() {
        for bad in [
            cookie("a\tb", "n", "v"),
            cookie("a", "n\t", "v"),
            cookie("a", "n", "v\r"),
        ] {
            assert!(matches!(
                to_netscape(&[bad]),
                Err(Error::InvalidCookie { .. })
            ));
        }
        let mut bad_path = cookie("a", "n", "v");
        bad_path.path = "/\t".into();
        assert!(to_netscape(&[bad_path]).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn an_existing_file_is_never_reused() {
        use std::fs;
        let folder = PrivateDir::new().unwrap();
        let path = folder.path().join(FILE_NAME);
        fs::write(&path, "prepared").unwrap();
        assert!(create_owner_only(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "prepared");
    }

    #[test]
    fn debug_shows_only_the_path() {
        let folder = PrivateDir::new().unwrap();
        let file = CookieFile {
            path: folder.path().join(FILE_NAME),
            _folder: folder,
        };
        assert!(format!("{file:?}").starts_with("CookieFile { path: "));
    }
}
