//! The current time in the unit cookie expiry is compared with.

use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds since 1970-01-01 UTC. A clock set before 1970 counts as 0.
pub(crate) fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_current_time_is_after_2020() {
        assert!(super::now_unix() > 1_577_836_800);
    }
}
