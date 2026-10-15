//! The work folders this process still holds, so that they can be removed on the way out
//! even when the values that own them are never dropped: a quit through `process::exit`,
//! or a termination signal (see [`crate::clean_up_on_signals`]).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::private_dir::remove_folders;

/// The work folders of this process.
pub(crate) static LIVE: Registry = Registry::new();

/// Removes the work folders this process still holds, with the cookie files and database
/// copies in them, and returns how many it removed. Call it from the app's exit handler.
///
/// After it, a [`crate::CookieFile`] that is still held points to a missing file, so call it
/// only when the process ends. Dropping such a value later is harmless. A folder that cannot
/// be removed now (on Windows, while a downloader still has the file open) is not counted,
/// and is removed when its value is dropped or by [`crate::remove_leftovers`] at the next
/// start. Nothing else is reported, so no path ever appears.
///
/// It is safe to call more than once and from several threads: a folder already removed is
/// skipped.
pub fn remove_work_folders() -> usize {
    LIVE.remove_all()
}

/// A list of work folders, safe to use from several threads.
pub(crate) struct Registry {
    /// The folders, in the order they were made.
    paths: Mutex<Vec<PathBuf>>,
}

impl Registry {
    /// An empty list.
    const fn new() -> Self {
        Self {
            paths: Mutex::new(Vec::new()),
        }
    }

    /// The list. A thread that panicked while holding it cannot leave it half changed, so
    /// it is used anyway.
    fn paths(&self) -> MutexGuard<'_, Vec<PathBuf>> {
        self.paths.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Adds the folder `path`.
    pub(crate) fn add(&self, path: &Path) {
        self.paths().push(path.to_owned());
    }

    /// Takes the folder `path` off the list, once its owner removes it.
    pub(crate) fn remove(&self, path: &Path) {
        self.paths().retain(|p| p != path);
    }

    /// Removes every folder on the list from the disk and returns how many it removed.
    /// The folders stay on the list until their owners are dropped.
    fn remove_all(&self) -> usize {
        let paths = self.paths().clone();
        remove_folders(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::private_dir::PrivateDir;
    use std::fs;

    #[test]
    fn every_folder_on_the_list_is_removed_once() {
        let parent = PrivateDir::new().unwrap();
        let registry = Registry::new();
        let folders: Vec<PathBuf> = ["a", "b"].iter().map(|n| parent.path().join(n)).collect();
        for folder in &folders {
            fs::create_dir(folder).unwrap();
            fs::write(folder.join("cookies.txt"), "x").unwrap();
            registry.add(folder);
        }
        assert_eq!(registry.remove_all(), 2);
        assert!(folders.iter().all(|folder| !folder.exists()));
        assert_eq!(registry.remove_all(), 0);
    }

    #[test]
    fn a_folder_taken_off_the_list_is_kept() {
        let parent = PrivateDir::new().unwrap();
        let registry = Registry::new();
        let folder = parent.path().join("kept");
        fs::create_dir(&folder).unwrap();
        registry.add(&folder);
        registry.remove(&folder);
        assert_eq!(registry.remove_all(), 0);
        assert!(folder.exists());
    }

    #[test]
    fn a_work_folder_is_on_the_list_until_dropped() {
        let dir = PrivateDir::new().unwrap();
        let path = dir.path().to_owned();
        assert!(LIVE.paths().contains(&path));
        drop(dir);
        assert!(!LIVE.paths().contains(&path));
    }
}
