//! A temporary folder that only the current user can open. It is removed, with everything
//! in it, when it goes out of scope, also after an error or a panic.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{Error, Result};

/// How many unused names to try before giving up.
const ATTEMPTS: u32 = 8;

/// The start of every work folder's name, followed by `{pid}-{nanos}-{counter}`.
const PREFIX: &str = concat!(env!("CARGO_PKG_NAME"), "-");

/// A temporary folder that only the current user can open, removed when dropped.
pub(crate) struct PrivateDir {
    /// Where the folder is.
    path: PathBuf,
}

impl PrivateDir {
    /// Creates a new folder in the system's temporary folder. The first time in a process,
    /// it first removes the folders that ended processes left there.
    pub(crate) fn new() -> Result<Self> {
        crate::leftovers::remove_once();
        Self::new_in(&std::env::temp_dir())
    }

    /// The folder is created in one step with its final permissions (owner only),
    /// and creating fails if the name already exists, so a folder prepared by someone else
    /// is never reused.
    pub(crate) fn new_in(parent: &Path) -> Result<Self> {
        let mut last_error = None;
        for _ in 0..ATTEMPTS {
            let path = parent.join(unique_name());
            match create_owner_only(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last_error = Some(e),
                Err(e) => return Err(work_folder_error(&path, e)),
            }
        }
        Err(Error::WorkFolder {
            path: parent.to_owned(),
            reason: last_error.map_or_else(String::new, |e| e.to_string()),
        })
    }

    /// Where the folder is.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PrivateDir {
    fn drop(&mut self) {
        // Nothing useful can be done with an error while dropping. The folder is in the
        // system's temporary folder and holds only files of this crate.
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// An I/O error in the work folder as this crate's error.
pub(crate) fn work_folder_error(path: &Path, error: std::io::Error) -> Error {
    Error::WorkFolder {
        path: path.to_owned(),
        reason: error.to_string(),
    }
}

/// Creates the folder `path` that only the user can open: mode 0700 on Unix.
#[cfg(unix)]
fn create_owner_only(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new().mode(0o700).create(path)
}

/// Creates the folder `path` that only the user can open: an access list on Windows.
#[cfg(windows)]
fn create_owner_only(path: &Path) -> std::io::Result<()> {
    crate::platform::create_owner_only_dir(path)
}

/// A name no other folder of this crate uses: process id, time and a counter.
fn unique_name() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    folder_name(
        std::process::id(),
        nanos,
        COUNTER.fetch_add(1, Ordering::Relaxed),
    )
}

/// The work folder name for process `pid`, made at `nanos` with `counter`. The only place
/// that writes the name format; [`owner_pid`] reads it back.
pub(crate) fn folder_name(pid: u32, nanos: u128, counter: u64) -> String {
    format!("{PREFIX}{pid}-{nanos}-{counter}")
}

/// Removes each folder in `paths` with everything in it, and returns how many it removed.
/// A folder that cannot be removed, or is already gone, is not counted.
pub(crate) fn remove_folders(paths: impl IntoIterator<Item = PathBuf>) -> usize {
    paths
        .into_iter()
        .filter(|path| fs::remove_dir_all(path).is_ok())
        .count()
}

/// The id of the process that made the work folder `name`, or `None` if `name` is not
/// exactly a name [`unique_name`] makes.
pub(crate) fn owner_pid(name: &str) -> Option<u32> {
    let mut parts = name.strip_prefix(PREFIX)?.split('-');
    let pid = digits(parts.next()?)?.parse().ok()?;
    digits(parts.next()?)?.parse::<u128>().ok()?;
    digits(parts.next()?)?.parse::<u64>().ok()?;
    parts.next().is_none().then_some(pid)
}

/// `part` if it is one or more ASCII digits and nothing else.
fn digits(part: &str) -> Option<&str> {
    (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())).then_some(part)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_and_its_files_are_removed_when_dropped() {
        let dir = PrivateDir::new().unwrap();
        let path = dir.path().to_owned();
        fs::write(path.join("file"), "x").unwrap();
        drop(dir);
        assert!(!path.exists());
    }

    #[test]
    fn folder_is_removed_after_a_panic() {
        let mut kept = None;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let dir = PrivateDir::new().unwrap();
            kept = Some(dir.path().to_owned());
            panic!("simulated failure");
        }));
        assert!(result.is_err());
        assert!(!kept.unwrap().exists());
    }

    #[test]
    fn the_owner_is_read_back_from_a_made_name() {
        assert_eq!(owner_pid(&unique_name()), Some(std::process::id()));
        assert_eq!(owner_pid(&folder_name(42, 1, 5)), Some(42));
    }

    #[test]
    fn any_other_name_has_no_owner() {
        let malformed = [
            "42-1-",
            "42-1",
            "42-1-5-6",
            "+42-1-5",
            "x-1-5",
            "99999999999-1-5",
        ];
        for name in malformed.map(|rest| format!("{PREFIX}{rest}")) {
            assert_eq!(owner_pid(&name), None, "{name}");
        }
        assert_eq!(owner_pid("other-42-1-5"), None);
        assert_eq!(owner_pid(""), None);
    }

    #[test]
    fn removed_folders_are_counted_once() {
        let parent = PrivateDir::new().unwrap();
        let folder = parent.path().join("a");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("file"), "x").unwrap();
        let missing = parent.path().join("missing");
        assert_eq!(remove_folders([folder.clone(), missing]), 1);
        assert!(!folder.exists());
    }

    #[test]
    fn two_folders_never_share_a_path() {
        let a = PrivateDir::new().unwrap();
        let b = PrivateDir::new().unwrap();
        assert_ne!(a.path(), b.path());
    }

    #[test]
    fn a_missing_parent_folder_is_a_work_folder_error() {
        let parent = PrivateDir::new().unwrap();
        let missing = parent.path().join("missing");
        assert!(matches!(
            PrivateDir::new_in(&missing),
            Err(Error::WorkFolder { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn only_the_owner_can_open_the_folder() {
        use std::os::unix::fs::PermissionsExt;
        let dir = PrivateDir::new().unwrap();
        let mode = fs::metadata(dir.path()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
    }
}
