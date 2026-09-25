//! UTC time-base helpers, ported from
//! `app/modules/parser/dataflash/timebase.go`.

use crate::format::FormatDef;

use super::fielddecode::{read_field_by_name, FieldValue};

/// Millisecond Unix timestamp plausibility window: 2000..2050.
pub fn is_plausible_utc_ms(utc_ms: f64) -> bool {
    const MIN_REASONABLE_UTC_MS: f64 = 946_684_800_000.0;
    const MAX_REASONABLE_UTC_MS: f64 = 2_524_608_000_000.0;
    (MIN_REASONABLE_UTC_MS..MAX_REASONABLE_UTC_MS).contains(&utc_ms)
}

/// Second-precision Unix timestamp plausibility window: 2000..2050.
pub fn is_plausible_utc_sec(utc_secs: u32) -> bool {
    const MIN_REASONABLE_UTC: u32 = 946_684_800;
    const MAX_REASONABLE_UTC: u32 = 2_524_608_000;
    (MIN_REASONABLE_UTC..MAX_REASONABLE_UTC).contains(&utc_secs)
}

/// Converts a GPS week + GPS milliseconds value to Unix milliseconds,
/// applying the GPS-UTC leap offset and a plausibility window.
pub fn week_ms_to_unix_millis(week: f64, gms: f64) -> Option<f64> {
    if week <= 0.0 || gms < 0.0 {
        return None;
    }
    const GPS_EPOCH_UNIX_MS: f64 = 315_964_800_000.0;
    const WEEK_MS: f64 = 7.0 * 24.0 * 60.0 * 60.0 * 1000.0;
    const GPS_UTC_LEAP_MS: f64 = 18.0 * 1000.0;
    let utc_ms = GPS_EPOCH_UNIX_MS + week * WEEK_MS + gms - GPS_UTC_LEAP_MS;
    if !is_plausible_utc_ms(utc_ms) {
        return None;
    }
    Some(utc_ms)
}

/// Finds the first numeric field matching any candidate name and returns its
/// finite value (case-insensitive name match, Go `lookupNamedNumber`).
pub fn lookup_named_number(fd: &FormatDef, values: &[FieldValue], names: &[&str]) -> Option<f64> {
    for (i, fn_) in fd.field_names.iter().enumerate() {
        let Some(v) = values.get(i) else { break };
        for name in names {
            if fn_.trim().eq_ignore_ascii_case(name) {
                let n = v.to_f64();
                return if n.is_finite() { Some(n) } else { None };
            }
        }
    }
    None
}

const WEEK_ALIASES: &[&str] = &["GWk", "GPSWeek", "Week", "Wk"];
const GMS_ALIASES: &[&str] = &[
    "GMS",
    "GTimeMS",
    "GPSTimeMS",
    "TOW",
    "Tow",
    "TowMS",
    "TimeOfWeekMS",
    "MS",
];

/// Decodes GPS week/GMS fields from text-line values into Unix milliseconds.
pub fn gps_field_unix_millis(fd: &FormatDef, values: &[FieldValue]) -> Option<f64> {
    let week = lookup_named_number(fd, values, WEEK_ALIASES)?;
    let gms = lookup_named_number(fd, values, GMS_ALIASES)?;
    week_ms_to_unix_millis(week, gms)
}

/// Decodes GPS week/GMS fields from a binary record and returns the
/// corresponding Unix milliseconds.
pub fn gps_bytes_unix_millis(fd: &FormatDef, data: &[u8]) -> Option<f64> {
    let wk = read_field_by_name(fd, data, WEEK_ALIASES)?;
    let gms = read_field_by_name(fd, data, GMS_ALIASES)?;
    week_ms_to_unix_millis(wk.to_f64(), gms.to_f64())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gps_week_conversion_window() {
        // 2200 weeks + 129600000 ms — the fixture's GPS base.
        let ms = week_ms_to_unix_millis(2200.0, 129_600_000.0).unwrap();
        assert_eq!(ms, 1_646_654_382_000.0);
        assert!(week_ms_to_unix_millis(0.0, 1.0).is_none());
        assert!(week_ms_to_unix_millis(65535.0, 0.0).is_none()); // outside window
    }

    #[test]
    fn plausible_windows() {
        assert!(is_plausible_utc_sec(1_780_000_000));
        assert!(!is_plausible_utc_sec(0));
        assert!(!is_plausible_utc_sec(u32::MAX));
        assert!(is_plausible_utc_ms(1_700_000_000_000.0));
        assert!(!is_plausible_utc_ms(0.0));
    }
}
