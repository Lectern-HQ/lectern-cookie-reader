//! Chromium-specific code: decrypting values and reading the cookie database.

pub(crate) mod crypto;
pub(crate) mod database;
#[cfg(any(windows, test))]
pub(crate) mod local_state;
#[cfg(test)]
pub(crate) mod test_vectors;

use crate::clock::MICROS_PER_SEC;

/// Seconds from 1601-01-01, where Chromium counts time from, to 1970-01-01 (Unix time).
const EPOCH_OFFSET_SECS: i64 = 11_644_473_600;

/// Chromium stores times as microseconds since 1601-01-01 UTC.
pub(crate) fn unix_to_chromium(unix_secs: i64) -> i64 {
    (unix_secs + EPOCH_OFFSET_SECS) * MICROS_PER_SEC
}

/// A Chromium time (microseconds since 1601) as Unix seconds.
pub(crate) fn chromium_to_unix(chromium_micros: i64) -> i64 {
    chromium_micros / MICROS_PER_SEC - EPOCH_OFFSET_SECS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_epoch_in_chromium_time() {
        assert_eq!(unix_to_chromium(0), 11_644_473_600_000_000);
        assert_eq!(chromium_to_unix(11_644_473_600_000_000), 0);
    }

    #[test]
    fn conversion_round_trips() {
        assert_eq!(
            chromium_to_unix(unix_to_chromium(1_900_000_000)),
            1_900_000_000
        );
    }
}
