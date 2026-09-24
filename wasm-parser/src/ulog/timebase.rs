//! UTC time-base helpers, ported from
//! `app/modules/parser/ulog/timebase.go`.

use crate::format::FormatDef;

use super::fields::read_scalar_uint64;

/// Whether a topic is known to carry a wall-clock UTC field.
pub fn carries_utc(name: &str) -> bool {
    matches!(name, "system_time" | "sensor_gps" | "vehicle_gps_position")
}

/// Reads the time_utc_usec field and converts it to millis.
pub fn utc_millis_from_topic(fd: &FormatDef, data: &[u8]) -> Option<f64> {
    let utc_usec = read_scalar_uint64(fd, data, "time_utc_usec")?;
    if utc_usec == 0 {
        return None;
    }
    Some(utc_usec as f64 / 1000.0)
}

/// Bounds-checks a UTC millis value to reject garbage epochs.
pub fn plausible_utc(utc_ms: f64) -> bool {
    const MIN_REASONABLE_UTC_MS: f64 = 946_684_800_000.0; // 2000-01-01
    const MAX_REASONABLE_UTC_MS: f64 = 2_524_608_000_000.0; // 2050-01-01
    (MIN_REASONABLE_UTC_MS..MAX_REASONABLE_UTC_MS).contains(&utc_ms)
}
