//! Whether another process is still running, asked without sending it anything.

/// `Some(true)` if process `pid` exists, `Some(false)` if it certainly does not, `None`
/// when that cannot be told. A process of another user still counts as running.
pub(crate) fn is_running(pid: u32) -> Option<bool> {
    let pid = libc::pid_t::try_from(pid).ok().filter(|pid| *pid > 0)?;
    // SAFETY: signal 0 sends nothing; the call only checks that the process exists.
    if unsafe { libc::kill(pid, 0) } == 0 {
        return Some(true);
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::ESRCH) => Some(false),
        Some(libc::EPERM) => Some(true),
        _ => None,
    }
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
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        assert_eq!(is_running(pid), Some(false));
    }

    #[test]
    fn an_impossible_id_cannot_be_told() {
        assert_eq!(is_running(0), None);
        assert_eq!(is_running(u32::MAX), None);
    }
}
