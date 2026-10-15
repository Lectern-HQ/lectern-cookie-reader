//! Work folders that ended processes left behind. A folder is removed when it goes out of
//! scope, but not when its process is killed, crashes or quits with `process::exit`: its
//! cookie file or database copy would then stay in the temporary folder.

use std::fs::{self, DirEntry};
use std::path::Path;
use std::sync::Once;

use crate::platform;
use crate::private_dir::owner_pid;

/// Removes the work folders that processes which are no longer running left in the system's
/// temporary folder, and returns how many it removed.
///
/// A folder is removed only when its name is exactly one this crate makes, it is a real
/// folder (not a link), and its process has certainly ended. When that cannot be told, the
/// folder is kept. The folders of this process are never touched. Folders that cannot be
/// removed are not counted; nothing else is reported, so no path ever appears.
///
/// It runs once by itself before the first work folder of a process is made. Call it at
/// startup too, so leftovers go before any cookie is read. It is safe to call more than
/// once and from several threads.
pub fn remove_leftovers() -> usize {
    remove_leftovers_in(&std::env::temp_dir(), platform::is_running)
}

/// Runs [`remove_leftovers`] the first time it is called in this process.
pub(crate) fn remove_once() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        remove_leftovers();
    });
}

/// Removes the leftovers in `parent`, asking `is_running` whether each owner still runs.
fn remove_leftovers_in(parent: &Path, is_running: impl Fn(u32) -> Option<bool>) -> usize {
    let Ok(entries) = fs::read_dir(parent) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| is_leftover(entry, &is_running))
        .filter(|entry| fs::remove_dir_all(entry.path()).is_ok())
        .count()
}

/// Whether `entry` is a work folder of another process that has certainly ended.
/// `DirEntry::file_type` does not follow links, so a link is never taken for a folder.
fn is_leftover(entry: &DirEntry, is_running: impl Fn(u32) -> Option<bool>) -> bool {
    let Some(pid) = entry.file_name().to_str().and_then(owner_pid) else {
        return false;
    };
    pid != std::process::id()
        && entry.file_type().is_ok_and(|kind| kind.is_dir())
        && is_running(pid) == Some(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::private_dir::PrivateDir;

    /// A work folder name for process `pid`, with a file inside.
    fn folder(parent: &Path, pid: u32) -> std::path::PathBuf {
        let path = parent.join(format!("lectern-cookie-reader-{pid}-1792103148685754000-0"));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("cookies.txt"), "x").unwrap();
        path
    }

    #[test]
    fn only_folders_of_ended_processes_are_removed() {
        let parent = PrivateDir::new().unwrap();
        let ended = folder(parent.path(), 101);
        let running = folder(parent.path(), 102);
        let unknown = folder(parent.path(), 103);
        let state = |pid| match pid {
            101 => Some(false),
            102 => Some(true),
            _ => None,
        };
        assert_eq!(remove_leftovers_in(parent.path(), state), 1);
        assert!(!ended.exists());
        assert!(running.exists() && unknown.exists());
    }

    #[test]
    fn this_processs_folders_are_never_removed() {
        let parent = PrivateDir::new().unwrap();
        let own = folder(parent.path(), std::process::id());
        assert_eq!(remove_leftovers_in(parent.path(), |_| Some(false)), 0);
        assert!(own.exists());
    }

    #[test]
    fn other_names_and_files_are_never_removed() {
        let parent = PrivateDir::new().unwrap();
        let other = parent.path().join("other-101-1-0");
        fs::create_dir(&other).unwrap();
        let file = parent.path().join("lectern-cookie-reader-101-1-1");
        fs::write(&file, "x").unwrap();
        assert_eq!(remove_leftovers_in(parent.path(), |_| Some(false)), 0);
        assert!(other.exists() && file.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_is_never_followed() {
        let parent = PrivateDir::new().unwrap();
        let target = parent.path().join("target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("kept"), "x").unwrap();
        let link = parent.path().join("lectern-cookie-reader-101-1-2");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert_eq!(remove_leftovers_in(parent.path(), |_| Some(false)), 0);
        assert!(target.join("kept").exists() && link.exists());
    }

    #[test]
    fn a_missing_parent_removes_nothing() {
        let parent = PrivateDir::new().unwrap();
        assert_eq!(
            remove_leftovers_in(&parent.path().join("missing"), |_| None),
            0
        );
    }

    #[test]
    fn the_real_sweep_keeps_this_processs_folders() {
        let own = PrivateDir::new().unwrap();
        remove_leftovers();
        remove_once();
        assert!(own.path().exists());
    }
}
