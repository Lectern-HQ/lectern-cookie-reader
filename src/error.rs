use std::fmt;
use std::path::PathBuf;

/// Everything that can go wrong. No variant ever contains a cookie value or a key.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The host is empty or is not a plain host name.
    InvalidHost(String),
    /// The browser's cookie database was not found at this path.
    DatabaseNotFound(PathBuf),
    /// The cookie database could not be copied or read.
    DatabaseUnreadable { path: PathBuf, reason: String },
    /// The copied cookie database could be opened but not queried (unexpected layout).
    DatabaseQuery(String),
    /// The private temporary folder could not be created or written.
    WorkFolder { path: PathBuf, reason: String },
    /// The database uses a format this version does not understand. Reading it anyway
    /// could return wrong values, so the crate refuses.
    UnsupportedDatabaseVersion(i64),
    /// The user said no to the operating system's permission dialog.
    KeyAccessDenied,
    /// The browser's key was not found (for example the browser was never opened).
    KeyNotFound,
    /// Reading the key failed for another reason (operating system error code).
    KeyUnavailable(i32),
    /// One cookie could not be decrypted. Only its name is given, never its value.
    DecryptionFailed { cookie_name: String },
    /// A decrypted value was stored for another site. The value is discarded.
    HostHashMismatch { cookie_name: String },
    /// This browser or operating system is not supported yet.
    Unsupported(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidHost(host) => write!(f, "not a valid host name: {host:?}"),
            Error::DatabaseNotFound(path) => {
                write!(f, "cookie database not found: {}", path.display())
            }
            Error::DatabaseUnreadable { path, reason } => {
                write!(
                    f,
                    "cannot read cookie database {}: {reason}",
                    path.display()
                )
            }
            Error::DatabaseQuery(reason) => write!(f, "cannot query cookie database: {reason}"),
            Error::WorkFolder { path, reason } => {
                write!(
                    f,
                    "cannot use the private work folder {}: {reason}",
                    path.display()
                )
            }
            Error::UnsupportedDatabaseVersion(v) => {
                write!(f, "cookie database version {v} is not supported yet")
            }
            Error::KeyAccessDenied => write!(f, "access to the browser's key was denied"),
            Error::KeyNotFound => write!(f, "the browser's key was not found"),
            Error::KeyUnavailable(code) => {
                write!(f, "the browser's key is not available (error {code})")
            }
            Error::DecryptionFailed { cookie_name } => {
                write!(f, "cookie {cookie_name:?} could not be decrypted")
            }
            Error::HostHashMismatch { cookie_name } => {
                write!(
                    f,
                    "cookie {cookie_name:?} belongs to another site and was discarded"
                )
            }
            Error::Unsupported(what) => write!(f, "not supported yet: {what}"),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_error_has_a_message_that_names_what_failed() {
        let path = || PathBuf::from("/tmp/Cookies");
        let reason = || "disk full".to_owned();
        let cases = [
            (Error::InvalidHost("a b".into()), "a b"),
            (Error::DatabaseNotFound(path()), "/tmp/Cookies"),
            (
                Error::DatabaseUnreadable {
                    path: path(),
                    reason: reason(),
                },
                "disk full",
            ),
            (
                Error::DatabaseQuery("no such table".into()),
                "no such table",
            ),
            (
                Error::WorkFolder {
                    path: path(),
                    reason: reason(),
                },
                "/tmp/Cookies",
            ),
            (Error::UnsupportedDatabaseVersion(25), "25"),
            (Error::KeyAccessDenied, "denied"),
            (Error::KeyNotFound, "not found"),
            (Error::KeyUnavailable(-1), "-1"),
            (
                Error::DecryptionFailed {
                    cookie_name: "session".into(),
                },
                "session",
            ),
            (
                Error::HostHashMismatch {
                    cookie_name: "session".into(),
                },
                "another site",
            ),
            (Error::Unsupported("Safari".into()), "Safari"),
        ];
        for (error, expected) in cases {
            let message = error.to_string();
            assert!(message.contains(expected), "{message}");
        }
    }
}
