//! A temporary folder that only the current user can open. It is removed, with everything
//! in it, when it goes out of scope, also after an error or a panic.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{Error, Result};

const ATTEMPTS: u32 = 8;

pub(crate) struct PrivateDir {
    path: PathBuf,
}

impl PrivateDir {
    /// Creates a new folder in the system's temporary folder.
    pub(crate) fn new() -> Result<Self> {
        Self::new_in(&std::env::temp_dir())
    }

    /// The folder is created in one step with its final permissions (owner only on Unix),
    /// and creating fails if the name already exists, so a folder prepared by someone else
    /// is never reused.
    pub(crate) fn new_in(parent: &Path) -> Result<Self> {
        let mut last_error = None;
        for _ in 0..ATTEMPTS {
            let path = parent.join(unique_name());
            match builder().create(&path) {
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

pub(crate) fn work_folder_error(path: &Path, error: std::io::Error) -> Error {
    Error::WorkFolder {
        path: path.to_owned(),
        reason: error.to_string(),
    }
}

fn builder() -> fs::DirBuilder {
    #[allow(unused_mut)]
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
}

fn unique_name() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "lectern-cookie-reader-{}-{nanos}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
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
