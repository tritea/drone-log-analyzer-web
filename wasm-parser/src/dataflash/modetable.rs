//! Flight-mode label tables, ported from
//! `app/modules/parser/dataflash/modetable.go`.

/// Firmware/vehicle identification from a banner line like
/// "ArduCopter V4.5.6 (hash)".
pub fn detect_firmware(lf: &mut crate::logfile::LogFile, line: &str) {
    let parts: Vec<&str> = line.split(' ').collect();
    if parts.len() < 2 || parts[1].is_empty() || !parts[1].starts_with('V') {
        return;
    }
    lf.summary.firmware_version = parts[1].to_string();
    match parts[0] {
        "ArduCopter" | "APM:Copter" => lf.summary.vehicle_type = "Copter".to_string(),
        "ArduPlane" => lf.summary.vehicle_type = "Plane".to_string(),
        "ArduRover" => lf.summary.vehicle_type = "Rover".to_string(),
        "ArduSub" => lf.summary.vehicle_type = "Sub".to_string(),
        _ => {}
    }
    if let Some(hash) = parts.get(2) {
        lf.summary.firmware_hash = hash.trim_matches(|c| c == '(' || c == ')').to_string();
    }
}

/// Maps a numeric flight mode to its label for the current vehicle type.
pub fn vehicle_mode_label(lf: &crate::logfile::LogFile, mode: i64) -> String {
    if lf.summary.vehicle_type == "Plane" {
        plane_mode_label(mode)
    } else {
        copter_mode_label(mode)
    }
}

fn plane_mode_label(mode: i64) -> String {
    let name = match mode {
        0 => "MANUAL",
        1 => "CIRCLE",
        2 => "STABILIZE",
        3 => "TRAINING",
        4 => "ACRO",
        5 => "FBWA",
        6 => "FBWB",
        7 => "CRUISE",
        8 => "AUTOTUNE",
        10 => "AUTO",
        11 => "RTL",
        12 => "LOITER",
        13 => "TAKEOFF",
        14 => "AVOID_ADSB",
        15 => "GUIDED",
        16 => "INITIALISING",
        17 => "QSTABILIZE",
        18 => "QHOVER",
        19 => "QLOITER",
        20 => "QLAND",
        21 => "QRTL",
        22 => "QAUTOTUNE",
        23 => "QACRO",
        24 => "THERMAL",
        _ => return format!("MODE_{}", mode),
    };
    name.to_string()
}

fn copter_mode_label(mode: i64) -> String {
    let name = match mode {
        0 => "STABILIZE",
        1 => "ACRO",
        2 => "ALT_HOLD",
        3 => "AUTO",
        4 => "GUIDED",
        5 => "LOITER",
        6 => "RTL",
        7 => "CIRCLE",
        9 => "LAND",
        10 => "OF_LOITER",
        11 => "DRIFT",
        13 => "SPORT",
        14 => "FLIP",
        15 => "AUTOTUNE",
        16 => "HYBRID",
        17 => "POSHOLD",
        18 => "BRAKE",
        19 => "THROW",
        20 => "AVOID_ADSB",
        21 => "GUIDED_NOGPS",
        22 => "SMART_RTL",
        23 => "FLOWHOLD",
        24 => "FOLLOW",
        _ => return format!("MODE_{}", mode),
    };
    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logfile::LogFile;

    #[test]
    fn firmware_detection() {
        let mut lf = LogFile::new("t", "apm");
        detect_firmware(&mut lf, "ArduCopter V4.5.6 (1931a2b1)");
        assert_eq!(lf.summary.vehicle_type, "Copter");
        assert_eq!(lf.summary.firmware_version, "V4.5.6");
        assert_eq!(lf.summary.firmware_hash, "1931a2b1");

        let mut lf2 = LogFile::new("t", "apm");
        detect_firmware(&mut lf2, "no version here");
        assert_eq!(lf2.summary.firmware_version, "");
    }

    #[test]
    fn mode_labels() {
        let mut lf = LogFile::new("t", "apm");
        lf.summary.vehicle_type = "Copter".into();
        assert_eq!(vehicle_mode_label(&lf, 4), "GUIDED");
        assert_eq!(vehicle_mode_label(&lf, 5), "LOITER");
        assert_eq!(vehicle_mode_label(&lf, 99), "MODE_99");
        lf.summary.vehicle_type = "Plane".into();
        assert_eq!(vehicle_mode_label(&lf, 11), "RTL");
    }
}
