//! Opt-in removal of the work folders when the process is asked to end by a signal.

/// Removes this process's work folders (see [`crate::remove_work_folders`]) when it
/// receives `SIGINT`, `SIGTERM` or `SIGHUP`, then lets the signal end the process as it
/// would have, with the same exit status.
///
/// Nothing is installed unless this is called. Do not call it if the app handles these
/// signals itself: after removing the folders, it ends the process. Call
/// [`crate::remove_work_folders`] from the app's own handler instead.
///
/// `SIGKILL` and crashes cannot be caught; [`crate::remove_leftovers`] removes their
/// folders at the next start. On Windows it does nothing, since a forced end cannot be
/// caught there either. Calling it again does nothing.
pub fn clean_up_on_signals() -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        macos::install()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

/// Signals on macOS, waited for on a normal thread: removing folders is not safe inside a
/// signal handler.
#[cfg(target_os = "macos")]
mod macos {
    use std::io;
    use std::sync::{Mutex, PoisonError};

    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    use signal_hook::iterator::Signals;
    use signal_hook::low_level::emulate_default_handler;

    use crate::remove_work_folders;

    /// Starts the thread that waits for the signals, once per process.
    pub(super) fn install() -> io::Result<()> {
        static INSTALLED: Mutex<bool> = Mutex::new(false);
        let mut installed = INSTALLED.lock().unwrap_or_else(PoisonError::into_inner);
        if *installed {
            return Ok(());
        }
        let mut signals = Signals::new([SIGINT, SIGTERM, SIGHUP])?;
        std::thread::Builder::new()
            .name(concat!(env!("CARGO_PKG_NAME"), "-signals").to_owned())
            .spawn(move || {
                if let Some(signal) = signals.forever().next() {
                    remove_work_folders();
                    // Only fails for a signal that cannot be raised, which these are not.
                    let _ = emulate_default_handler(signal);
                }
            })?;
        *installed = true;
        Ok(())
    }
}
