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
//! Every item has a doc comment, private ones too: `cargo clippy` fails otherwise.
//!
//! ```no_run
//! # fn main() -> lectern_cookie_reader::Result<()> {
//! use lectern_cookie_reader::{Browser, CookieFile, read_cookies};
//!
//! let profile = Browser::by_id("dia").expect("Dia is in the catalog").default_profile()?;
//! let cookies = read_cookies(&profile, "mediaserver.unige.ch")?;
//! let file = CookieFile::create(&cookies)?;
//! println!("yt-dlp --cookies {} <link>", file.path().display());
//! # Ok(())
//! # }
//! ```
#![deny(missing_docs)]
#![cfg_attr(not(test), deny(clippy::missing_docs_in_private_items))]

mod browser;
mod catalog;
mod chromium;
mod clock;
mod cookie;
mod cookie_file;
mod database_copy;
mod default_browser;
mod domain;
mod engine_store;
mod error;
mod firefox;
mod logins;
mod netscape;
mod platform;
mod private_dir;
mod profile;
mod reader;
mod suggest;

pub use browser::{Browser, Engine};
pub use cookie::{Cookie, SecretValue};
pub use cookie_file::CookieFile;
pub use default_browser::{DefaultBrowser, default_browser};
pub use domain::host_of_link;
pub use error::Error;
pub use logins::{Checked, Login, Scan, Skipped, find_logins};
pub use profile::Profile;
pub use reader::read_cookies;
pub use suggest::{Reason, Suggestion, suggest};

/// Result type used by this crate.
pub type Result<T> = std::result::Result<T, Error>;

// Compiles the examples in the README with the tests, so they cannot go out of date.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExamples;
