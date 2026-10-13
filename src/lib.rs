//! Reads the login cookies for **one website** from a browser on the user's own computer.
//!
//! The library is built so that every step that touches browser data is visible here:
//! copying the browser's cookie database, asking the operating system for the browser's
//! key, decrypting only the cookies of the requested site, and writing the short-lived
//! cookie file a downloader needs.
//!
//! What it never does: ask for or store a password, read cookies of other sites, send
//! anything over the network, or put a cookie value in a log or an error message.
//!
//! ```no_run
//! # fn main() -> lectern_cookie_reader::Result<()> {
//! use lectern_cookie_reader::{Browser, CookieFile, read_cookies};
//!
//! let profile = Browser::Dia.default_profile()?;
//! let cookies = read_cookies(&profile, "mediaserver.unige.ch")?;
//! let file = CookieFile::create(&cookies)?;
//! println!("yt-dlp --cookies {} <link>", file.path().display());
//! # Ok(())
//! # }
//! ```

mod browser;
mod chromium;
mod clock;
mod cookie;
mod cookie_file;
mod domain;
mod error;
mod logins;
mod platform;
mod private_dir;
mod reader;

pub use browser::{Browser, DefaultBrowser, Engine, Profile, default_browser};
pub use cookie::{Cookie, SecretValue};
pub use cookie_file::CookieFile;
pub use error::Error;
pub use logins::{Login, Scan, Skipped, find_logins};
pub use reader::read_cookies;

/// Result type used by this crate.
pub type Result<T> = std::result::Result<T, Error>;

// Compiles the examples in the README with the tests, so they cannot go out of date.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExamples;
