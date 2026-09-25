//! Summary-level derivations, ported from
//! `app/services/logservice/dataflash/{logservice.go summarize, px4airframe.go}`.
//! The wasm side collapses the Go parser/logservice layers, so these
//! projections run once at session finish. The future query layer owns them.

use crate::logfile::LogFile;

const PX4_DEFAULT_FRAME: &str = "QUADROTOR";
const PX4_DEFAULT_AIRFRAME: &str = "multirotor";
const PX4_SYS_AUTOSTART_KEY: &str = "SYS_AUTOSTART";

fn classify_px4_airframe(id: i64) -> (&'static str, &'static str) {
    match id {
        13000..=13999 => ("", "vtol"),
        2100..=2199 | 3000..=3099 => ("", "vtol"),
        4000..=5999 => ("QUADROTOR", "multirotor"),
        6000..=7999 | 11000..=11999 => ("HEXAROTOR", "multirotor"),
        8000..=9999 | 12000..=12999 => ("OCTOROTOR", "multirotor"),
        _ => (PX4_DEFAULT_FRAME, PX4_DEFAULT_AIRFRAME),
    }
}

fn classify_px4_from_log(lf: &LogFile) -> (&'static str, &'static str) {
    match lf.parameters.get(PX4_SYS_AUTOSTART_KEY) {
        Some(v) => classify_px4_airframe(*v as i64),
        None => (PX4_DEFAULT_FRAME, PX4_DEFAULT_AIRFRAME),
    }
}

fn classify_airframe(lf: &LogFile) -> &'static str {
    if lf.parameters.contains_key("Q_ENABLE") {
        "vtol"
    } else {
        "multirotor"
    }
}

/// Applies the format-specific frame/airframe classification in place.
pub fn apply_summary_projection(lf: &mut LogFile) {
    match lf.summary.format.as_str() {
        "ulog" => {
            let (frame, airframe) = classify_px4_from_log(lf);
            lf.summary.frame = frame.to_string();
            lf.summary.airframe = airframe.to_string();
        }
        "apm" | "" => {
            lf.summary.airframe = classify_airframe(lf).to_string();
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn px4_classification_ranges() {
        assert_eq!(classify_px4_airframe(0), ("QUADROTOR", "multirotor"));
        assert_eq!(classify_px4_airframe(4500), ("QUADROTOR", "multirotor"));
        assert_eq!(classify_px4_airframe(6500), ("HEXAROTOR", "multirotor"));
        assert_eq!(classify_px4_airframe(8500), ("OCTOROTOR", "multirotor"));
        assert_eq!(classify_px4_airframe(13100), ("", "vtol"));
    }
}
