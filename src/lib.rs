//! Reads the login cookies for **one website** from a browser on the user's own computer.
//!
//! The library is built so that every step that touches browser data is visible here:
//! finding the browser, asking the operating system for the browser's key, decrypting
//! only the cookies of the requested site, and writing the short-lived cookie file a
//! downloader needs.
//!
//! What it never does: ask for or store a password, read cookies of other sites, send
//! anything over the network, or put a cookie value in a log or an error message.

mod browser;
mod chromium;
mod cookie;
mod domain;
mod error;
mod platform;
mod private_dir;
mod reader;

pub use browser::{Browser, Engine, Profile};
pub use cookie::{Cookie, SecretValue};
pub use error::Error;
pub use reader::read_cookies;

/// Result type used by this crate.
pub type Result<T> = std::result::Result<T, Error>;
