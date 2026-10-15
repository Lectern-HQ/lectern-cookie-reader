//! A private copy of a browser's SQLite cookie database, and the helpers every engine uses
//! to query it. The browser's own file is never opened, so a running browser is not
//! disturbed.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::ToSql;

use crate::private_dir::PrivateDir;
use crate::{Error, Result};

/// SQLite keeps recent changes in these files next to the database until it merges them.
/// Without them the copy could miss a login that just happened.
const SIDECAR_SUFFIXES: &[&str] = &["-wal", "-journal"];

/// A copy of a cookie database in its own private folder. Dropping it deletes the copy
/// with its folder; anything that opened the copy must be dropped first.
pub(crate) struct DatabaseCopy {
    /// Where the copy is.
    path: PathBuf,
    /// The private folder holding the copy.
    _folder: PrivateDir,
}

impl DatabaseCopy {
    /// Copies the database at `source`, with its SQLite sidecar files, into a new private
    /// folder.
    pub(crate) fn of(source: &Path) -> Result<Self> {
        let folder = PrivateDir::new()?;
        let path = copy_database(source, folder.path())?;
        Ok(Self {
            path,
            _folder: folder,
        })
    }

    /// Where the copy is.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

/// Copies the database at `source`, with its SQLite sidecar files, into `folder` and
/// returns the path of the copy.
pub(crate) fn copy_database(source: &Path, folder: &Path) -> Result<PathBuf> {
    if !source.is_file() {
        return Err(Error::DatabaseNotFound(source.to_owned()));
    }
    let file_name = source
        .file_name()
        .ok_or_else(|| Error::DatabaseNotFound(source.to_owned()))?;
    let copy = folder.join(file_name);
    copy_file(source, &copy)?;
    for suffix in SIDECAR_SUFFIXES {
        let mut name = file_name.to_owned();
        name.push(suffix);
        let sidecar = source.with_file_name(&name);
        if sidecar.is_file() {
            copy_file(&sidecar, &folder.join(name))?;
        }
    }
    Ok(copy)
}

/// Windows' `ERROR_SHARING_VIOLATION`: another program opened the file without sharing it.
const ERROR_SHARING_VIOLATION: i32 = 32;

/// Copies one file, reporting the source path if it fails.
fn copy_file(from: &Path, to: &Path) -> Result<()> {
    fs::copy(from, to).map(|_| ()).map_err(|e| {
        if is_locked(&e) {
            Error::DatabaseLocked(from.to_owned())
        } else {
            unreadable(from, e)
        }
    })
}

/// Whether the copy failed because the running browser keeps the file locked, as Chrome
/// does on Windows.
fn is_locked(error: &std::io::Error) -> bool {
    cfg!(windows) && error.raw_os_error() == Some(ERROR_SHARING_VIOLATION)
}

/// The error for a database file that could not be copied or opened.
pub(crate) fn unreadable(path: &Path, reason: impl std::fmt::Display) -> Error {
    Error::DatabaseUnreadable {
        path: path.to_owned(),
        reason: reason.to_string(),
    }
}

/// A database error as this crate's error, with SQLite's reason.
pub(crate) fn query_error(error: rusqlite::Error) -> Error {
    Error::DatabaseQuery(error.to_string())
}

/// SQL placeholders `?first`, `?first+1`, ... for `count` values, separated by commas.
/// Values are always passed as parameters, never written into the SQL text.
pub(crate) fn placeholders(first: usize, count: usize) -> String {
    (first..first + count)
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A cookie query's parameters in order: the current time as `?1`, then each domain.
pub(crate) fn time_and_domains<'a>(now: &'a i64, domains: &'a [String]) -> Vec<&'a dyn ToSql> {
    let mut params: Vec<&dyn ToSql> = vec![now];
    params.extend(domains.iter().map(|d| d as &dyn ToSql));
    params
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_includes_the_sidecar_files() {
        let profile = PrivateDir::new().unwrap();
        let copy = PrivateDir::new().unwrap();
        for name in ["Cookies", "Cookies-wal", "Cookies-journal"] {
            fs::write(profile.path().join(name), name).unwrap();
        }
        let copied = copy_database(&profile.path().join("Cookies"), copy.path()).unwrap();
        assert_eq!(copied, copy.path().join("Cookies"));
        for name in ["Cookies", "Cookies-wal", "Cookies-journal"] {
            assert_eq!(fs::read_to_string(copy.path().join(name)).unwrap(), name);
        }
    }

    #[test]
    fn missing_database_is_its_own_error() {
        let profile = PrivateDir::new().unwrap();
        let copy = PrivateDir::new().unwrap();
        assert!(matches!(
            copy_database(&profile.path().join("Cookies"), copy.path()),
            Err(Error::DatabaseNotFound(_))
        ));
    }

    #[test]
    fn the_copy_is_deleted_when_dropped() {
        let profile = PrivateDir::new().unwrap();
        fs::write(profile.path().join("cookies.sqlite"), "x").unwrap();
        let copy = DatabaseCopy::of(&profile.path().join("cookies.sqlite")).unwrap();
        let path = copy.path().to_owned();
        assert_eq!(fs::read_to_string(&path).unwrap(), "x");
        drop(copy);
        assert!(!path.exists());
    }

    #[test]
    fn only_a_sharing_violation_on_windows_is_a_lock() {
        let sharing = std::io::Error::from_raw_os_error(ERROR_SHARING_VIOLATION);
        assert_eq!(is_locked(&sharing), cfg!(windows));
        assert!(!is_locked(&std::io::Error::other("disk full")));
    }

    #[test]
    fn placeholders_are_numbered_from_the_first() {
        assert_eq!(placeholders(2, 3), "?2, ?3, ?4");
        assert_eq!(placeholders(1, 0), "");
    }
}
