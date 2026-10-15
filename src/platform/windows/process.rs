//! Whether another process is still running, asked without changing it.

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER, GetLastError, STILL_ACTIVE,
};
use windows_sys::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};

/// `Some(true)` if process `pid` is running, `Some(false)` if it certainly is not, `None`
/// when that cannot be told. A process of another user still counts as running.
pub(crate) fn is_running(pid: u32) -> Option<bool> {
    // SAFETY: opens the process for reading its exit code only; closed once below.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        // SAFETY: reads the calling thread's last error code, set by the failed call.
        return match unsafe { GetLastError() } {
            ERROR_INVALID_PARAMETER => Some(false),
            ERROR_ACCESS_DENIED => Some(true),
            _ => None,
        };
    }
    let mut code = 0;
    // SAFETY: `handle` was opened above and `code` outlives the call.
    let ok = unsafe { GetExitCodeProcess(handle, &mut code) };
    // SAFETY: `handle` was opened above and is closed exactly once.
    unsafe { CloseHandle(handle) };
    // A process that ended with the code 259 looks running; it is then kept, never removed.
    (ok != 0).then(|| u32::try_from(STILL_ACTIVE) == Ok(code))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_process_is_running() {
        assert_eq!(is_running(std::process::id()), Some(true));
    }

    #[test]
    fn a_finished_child_is_not_running() {
        let mut child = std::process::Command::new("cmd")
            .args(["/C", "exit"])
            .spawn()
            .unwrap();
        let pid = child.id();
        child.wait().unwrap();
        drop(child);
        assert_eq!(is_running(pid), Some(false));
    }
}
