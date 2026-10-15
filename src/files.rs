//! What a failure to read one of the browser's files means for the user. Every read of a
//! browser file or folder maps its error here, so the same failure is always reported the
//! same way.

use std::io;
use std::path::Path;

use crate::{Error, Result, platform};

/// The error for `path` that could not be read or copied: missing, protected by the
/// system, locked by the running browser, or unreadable for another reason.
pub(crate) fn read_error(path: &Path, error: io::Error) -> Error {
    if platform::is_locked(&error) {
        return Error::DatabaseLocked(path.to_owned());
    }
    match error.kind() {
        io::ErrorKind::NotFound => Error::DatabaseNotFound(path.to_owned()),
        io::ErrorKind::PermissionDenied => Error::PermissionNeeded(path.to_owned()),
        _ => unreadable(path, error),
    }
}

/// `None` for a file or folder at `path` that does not exist, the value if it was read,
/// and the [`read_error`] otherwise.
pub(crate) fn existing<T>(read: io::Result<T>, path: &Path) -> Result<Option<T>> {
    match read {
        Ok(value) => Ok(Some(value)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(read_error(path, e)),
    }
}

/// The error for the file at `path` that was read but could not be used, with the reason.
pub(crate) fn unreadable(path: &Path, reason: impl std::fmt::Display) -> Error {
    Error::DatabaseUnreadable {
        path: path.to_owned(),
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn path() -> PathBuf {
        PathBuf::from("Cookies")
    }

    #[test]
    fn each_kind_of_failure_has_its_own_error() {
        let missing = read_error(&path(), io::ErrorKind::NotFound.into());
        assert!(matches!(missing, Error::DatabaseNotFound(p) if p == path()));
        let denied = read_error(&path(), io::ErrorKind::PermissionDenied.into());
        assert!(matches!(denied, Error::PermissionNeeded(p) if p == path()));
        let other = read_error(&path(), io::Error::other("disk full"));
        assert!(
            matches!(other, Error::DatabaseUnreadable { path: p, reason } if p == path() && reason == "disk full")
        );
    }

    #[test]
    fn a_missing_file_is_none_and_other_failures_are_errors() {
        assert_eq!(existing(Ok(7), &path()).unwrap(), Some(7));
        let missing: io::Result<u8> = Err(io::ErrorKind::NotFound.into());
        assert_eq!(existing(missing, &path()).unwrap(), None);
        let denied: io::Result<u8> = Err(io::ErrorKind::PermissionDenied.into());
        assert!(matches!(
            existing(denied, &path()),
            Err(Error::PermissionNeeded(_))
        ));
    }
}
