//! Steps that depend on the operating system. Each platform has one module with the same
//! functions:
//!
//! - `user_data_dir(browser)`: the folder that holds the browser's profiles.
//! - `chromium_key(browser)`: the key that decrypts the browser's cookie values.
//! - `default_browser()`: the browser that opens links, if the system says.
//! - `is_protected(browser)`: whether the system protects the browser's data folder.
//! - `is_locked(error)`: whether a read failed because another program locked the file.
//! - `is_running(pid)`: whether a process is still running, if that can be told.
//! - `PERMISSION_HINT`: what the user does to let the app read a protected file.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::{
    PERMISSION_HINT, chromium_key, default_browser, is_locked, is_protected, is_running,
    user_data_dir,
};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::{
    PERMISSION_HINT, chromium_key, create_owner_only_dir, default_browser, is_locked, is_protected,
    is_running, user_data_dir,
};

#[cfg(not(any(target_os = "macos", windows)))]
mod unsupported;
#[cfg(not(any(target_os = "macos", windows)))]
pub(crate) use unsupported::{
    PERMISSION_HINT, chromium_key, default_browser, is_locked, is_protected, is_running,
    user_data_dir,
};

/// `browser`'s details for `system`, or [`crate::Error::Unsupported`] when the catalog has
/// none: the browser does not exist there, or is not read there.
#[cfg(any(target_os = "macos", windows))]
fn details<T>(
    browser: crate::Browser,
    details: Option<&'static T>,
    system: &str,
) -> crate::Result<&'static T> {
    details.ok_or_else(|| crate::Error::Unsupported(format!("{} on {system}", browser.name())))
}

/// The folder the environment variable `name` holds, such as `HOME` or `LOCALAPPDATA`.
#[cfg(any(target_os = "macos", windows))]
fn env_folder(name: &str) -> crate::Result<std::path::PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
        .ok_or_else(|| {
            crate::Error::Unsupported(format!("the {name} environment variable is not set"))
        })
}
