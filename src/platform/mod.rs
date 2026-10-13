//! Steps that depend on the operating system. Each platform has one module with the same
//! functions:
//!
//! - `user_data_dir(browser)`: the folder that holds the browser's profiles.
//! - `chromium_key(browser)`: the key that decrypts the browser's cookie values.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::{chromium_key, user_data_dir};

#[cfg(not(target_os = "macos"))]
mod unsupported;
#[cfg(not(target_os = "macos"))]
pub(crate) use unsupported::{chromium_key, user_data_dir};
