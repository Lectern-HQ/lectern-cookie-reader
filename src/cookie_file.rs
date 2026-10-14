//! The cookie file a downloader such as yt-dlp reads (`--cookies`), in Netscape format.
//!
//! It is the most sensitive file Lectern creates, so it is written into a folder only the
//! user can open, readable only by the user, and deleted with its folder when the
//! [`CookieFile`] goes out of scope: after success, after an error and after a panic.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::netscape::to_netscape;
use crate::private_dir::{PrivateDir, work_folder_error};
use crate::{Cookie, Result};

/// The file's name in its private folder.
const FILE_NAME: &str = "cookies.txt";

/// A temporary `cookies.txt` that is deleted when dropped.
///
/// ```no_run
/// # fn main() -> lectern_cookie_reader::Result<()> {
/// use lectern_cookie_reader::{Browser, CookieFile, read_cookies};
///
/// let profile = Browser::by_id("dia").expect("Dia is in the catalog").default_profile()?;
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
    Err(crate::Error::Unsupported(
        "a cookie file only the current user can read, on this operating system".into(),
    ))
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
