//! Steps that depend on the operating system. Each platform has one module with the same
//! functions:
//!
//! - `user_data_dir(browser)`: the folder that holds the browser's profiles.
//! - `chromium_key(browser)`: the key that decrypts the browser's cookie values.
//! - `default_browser()`: the browser that opens links, if the system says.
//! - `is_protected(browser)`: whether the system protects the browser's data folder.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::{chromium_key, default_browser, is_protected, user_data_dir};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::{chromium_key, default_browser, is_protected, user_data_dir};

#[cfg(not(any(target_os = "macos", windows)))]
mod unsupported;
#[cfg(not(any(target_os = "macos", windows)))]
pub(crate) use unsupported::{chromium_key, default_browser, is_protected, user_data_dir};
