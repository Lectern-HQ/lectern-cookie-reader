//! The crate's error type. No error ever contains a cookie value or a key.

use std::fmt;
use std::path::PathBuf;

/// Everything that can go wrong. No variant ever contains a cookie value or a key.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The host is empty or is not a plain host name.
    InvalidHost(String),
    /// Not a usable `https` link; the reason only, never the link itself.
    InvalidLink(String),
    /// The browser's cookie database was not found at this path.
    DatabaseNotFound(PathBuf),
    /// The cookie database could not be copied or read.
    DatabaseUnreadable {
        /// The file that could not be copied or opened.
        path: PathBuf,
        /// The reason from the operating system or SQLite.
        reason: String,
    },
    /// The running browser keeps its cookie database locked, so it cannot be copied
    /// (Chrome and Edge on Windows). Closing the browser releases it.
    DatabaseLocked(PathBuf),
    /// The copied cookie database could be opened but not queried (unexpected layout).
    DatabaseQuery(String),
    /// The private temporary folder could not be created or written.
    WorkFolder {
        /// The folder or file that could not be created or written.
        path: PathBuf,
        /// The reason from the operating system.
        reason: String,
    },
    /// The database uses a format this version does not understand. Reading it anyway
    /// could return wrong values, so the crate refuses.
    UnsupportedDatabaseVersion(i64),
    /// The user said no to the operating system's permission dialog.
    KeyAccessDenied,
    /// The operating system protects this file or folder, and the user has not allowed this
    /// app to read it yet. On macOS 27 that is Safari's, Google Chrome's and Firefox's data
    /// without Full Disk Access; the message says where to allow it.
    PermissionNeeded(PathBuf),
    /// The browser's key was not found (for example the browser was never opened).
    KeyNotFound,
    /// The browser's stored key does not have the expected size or format.
    InvalidKey,
    /// Reading the key failed for another reason (operating system error code).
    KeyUnavailable(i32),
    /// One cookie could not be decrypted. Only its name is given, never its value.
    DecryptionFailed {
        /// The cookie's name, never its value.
        cookie_name: String,
    },
    /// A line of a `cookies.txt` is not in Netscape format. Only its number is given, never
    /// its content, which may be another site's cookie.
    InvalidCookieFile {
        /// The line's number, from 1.
        line: usize,
    },
    /// A cookie field contains a tab or a line break, which would break the cookie file.
    InvalidCookie {
        /// The cookie's name, with tabs and line breaks escaped.
        cookie_name: String,
    },
    /// The browser protects the site's cookies with app-bound encryption (Chrome and Edge
    /// on Windows since version 127). Only the browser itself can read them, and this
    /// crate never tries to get around that: use a `cookies.txt` export instead.
    AppBoundEncryption {
        /// The browser's name, for example `Google Chrome`.
        browser: String,
    },
    /// A decrypted value was stored for another site. The value is discarded.
    HostHashMismatch {
        /// The cookie's name, never its value.
        cookie_name: String,
    },
    /// This browser or operating system is not supported yet.
    Unsupported(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidHost(host) => write!(f, "not a valid host name: {host:?}"),
            Error::InvalidLink(reason) => write!(f, "not a usable lecture link: {reason}"),
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
            Error::DatabaseLocked(path) => write!(
                f,
                "the browser keeps its cookie database {} locked; close the browser and try again",
                path.display()
            ),
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
            Error::PermissionNeeded(path) => write!(
                f,
                "the system protects {}; {}",
                path.display(),
                crate::platform::PERMISSION_HINT
            ),
            Error::KeyNotFound => write!(f, "the browser's key was not found"),
            Error::InvalidKey => write!(f, "the browser's key does not have the expected format"),
            Error::KeyUnavailable(code) => {
                write!(f, "the browser's key is not available (error {code})")
            }
            Error::DecryptionFailed { cookie_name } => {
                write!(f, "cookie {cookie_name:?} could not be decrypted")
            }
            Error::InvalidCookieFile { line } => {
                write!(
                    f,
                    "line {line} of the cookies.txt is not in Netscape format"
                )
            }
            Error::InvalidCookie { cookie_name } => {
                write!(
                    f,
                    "cookie {cookie_name:?} contains a tab or a line break and cannot be written"
                )
            }
            Error::AppBoundEncryption { browser } => write!(
                f,
                "{browser} protects this site's cookies with app-bound encryption; export them to a cookies.txt file instead"
            ),
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
            (Error::InvalidLink("only https".into()), "only https"),
            (Error::DatabaseNotFound(path()), "/tmp/Cookies"),
            (Error::DatabaseLocked(path()), "close the browser"),
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
            (Error::PermissionNeeded(path()), "/tmp/Cookies"),
            (
                Error::PermissionNeeded(path()),
                crate::platform::PERMISSION_HINT,
            ),
            (Error::KeyNotFound, "not found"),
            (Error::InvalidKey, "format"),
            (Error::KeyUnavailable(-25308), "-25308"),
            (
                Error::DecryptionFailed {
                    cookie_name: "session".into(),
                },
                "session",
            ),
            (
                Error::InvalidCookie {
                    cookie_name: "session".into(),
                },
                "line break",
            ),
            (Error::InvalidCookieFile { line: 3 }, "line 3"),
            (
                Error::HostHashMismatch {
                    cookie_name: "session".into(),
                },
                "another site",
            ),
            (
                Error::AppBoundEncryption {
                    browser: "Google Chrome".into(),
                },
                "cookies.txt",
            ),
            (Error::Unsupported("Safari".into()), "Safari"),
        ];
        for (error, expected) in cases {
            let message = error.to_string();
            assert!(message.contains(expected), "{message}");
        }
    }
}
