//! The current time in the unit cookie expiry is compared with, and the units browsers
//! store times in.

use std::time::{SystemTime, UNIX_EPOCH};

/// Milliseconds per second.
pub(crate) const MILLIS_PER_SEC: i64 = 1_000;
/// Microseconds per second.
pub(crate) const MICROS_PER_SEC: i64 = 1_000_000;

/// Seconds since 1970-01-01 UTC. A clock set before 1970 counts as 0.
pub(crate) fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

#[cfg(test)]
pub(crate) mod tests {
    /// A fixed "now" for tests that read generated databases and files (2027-01-15).
    pub(crate) const NOW: i64 = 1_800_000_000;

    #[test]
    fn the_current_time_is_after_2020() {
        assert!(super::now_unix() > 1_577_836_800);
    }
}
