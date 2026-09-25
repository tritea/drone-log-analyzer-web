//! Vehicle/mode classification, ported from
//! `app/modules/parser/tlog/vehicles.go`.

/// Maps a MAV_TYPE (numeric) to the ArduPilot vehicle family, frame label,
/// and airframe hint used in the Summary. Unknown types return empty strings.
pub fn decode_vehicle(t: u8) -> (&'static str, &'static str, &'static str) {
    match t {
        2 => ("Copter", "QUADROTOR", "multirotor"),
        13 => ("Copter", "HEXAROTOR", "multirotor"),
        14 => ("Copter", "OCTOROTOR", "multirotor"),
        15 => ("Copter", "TRICOPTER", "multirotor"),
        29 => ("Copter", "DODECAROTOR", "multirotor"),
        35 => ("Copter", "DECAROTOR", "multirotor"),
        4 => ("Copter", "HELICOPTER", "multirotor"),
        3 => ("Copter", "COAXIAL", "multirotor"),
        1 => ("Plane", "FIXED_WING", ""),
        10 => ("Rover", "ROVER", ""),
        11 => ("Boat", "BOAT", ""),
        12 => ("Sub", "SUBMARINE", ""),
        19 | 20 | 21 | 22 | 23 | 24 | 47 => ("Plane", "VTOL", "vtol"),
        _ => ("", "", ""),
    }
}

/// Maps a MAV_AUTOPILOT (numeric) to its display label.
pub fn autopilot_name(a: u8) -> &'static str {
    match a {
        3 => "ArduPilot",
        12 => "PX4",
        _ => "",
    }
}

/// Resolves a custom mode number to its ArduPilot flight-mode name,
/// dispatching by vehicle family and defaulting to the Copter table.
pub fn flight_mode_label(vehicle: &str, custom_mode: u32) -> String {
    match vehicle {
        "Plane" => plane_mode(custom_mode as i64),
        "Rover" => rover_mode(custom_mode as i64),
        "Sub" => sub_mode(custom_mode as i64),
        _ => copter_mode(custom_mode as i64),
    }
}

fn plane_mode(mode: i64) -> String {
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

fn copter_mode(mode: i64) -> String {
    let name = match mode {
        0 => "STABILIZE",
        1 => "ACRO",
        2 => "ALT_HOLD",
        3 => "AUTO",
        4 => "GUIDED",
        5 => "LOITER",
        6 => "RTL",
        7 => "CIRCLE",
        8 => "POSITION",
        9 => "LAND",
        10 => "OF_LOITER",
        11 => "DRIFT",
        13 => "SPORT",
        14 => "FLIP",
        15 => "AUTOTUNE",
        16 => "POSHOLD",
        17 => "BRAKE",
        18 => "THROW",
        19 => "AVOID_ADSB",
        20 => "GUIDED_NOGPS",
        21 => "SMART_RTL",
        22 => "FLOWHOLD",
        23 => "FOLLOW",
        24 => "ZIGZAG",
        25 => "SYSTEMID",
        26 => "AUTOROTATE",
        27 => "AUTO_RTL",
        _ => return format!("MODE_{}", mode),
    };
    name.to_string()
}

fn rover_mode(mode: i64) -> String {
    let name = match mode {
        0 => "MANUAL",
        1 => "ACRO",
        3 => "STEERING",
        4 => "HOLD",
        5 => "LOITER",
        6 => "FOLLOW",
        7 => "SIMPLE",
        10 => "AUTO",
        11 => "RTL",
        12 => "SMART_RTL",
        15 => "GUIDED",
        16 => "INITIALISING",
        _ => return format!("MODE_{}", mode),
    };
    name.to_string()
}

fn sub_mode(mode: i64) -> String {
    let name = match mode {
        0 => "STABILIZE",
        1 => "ACRO",
        2 => "ALT_HOLD",
        3 => "AUTO",
        4 => "GUIDED",
        7 => "CIRCLE",
        9 => "SURFACE",
        10 => "OF_LOITER",
        11 => "DRIFT",
        16 => "POSHOLD",
        19 => "MANUAL",
        _ => return format!("MODE_{}", mode),
    };
    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vehicles_map() {
        assert_eq!(decode_vehicle(2), ("Copter", "QUADROTOR", "multirotor"));
        assert_eq!(decode_vehicle(1), ("Plane", "FIXED_WING", ""));
        assert_eq!(decode_vehicle(21), ("Plane", "VTOL", "vtol"));
        assert_eq!(decode_vehicle(99), ("", "", ""));
    }

    #[test]
    fn modes_match_go_tables() {
        assert_eq!(flight_mode_label("Copter", 4), "GUIDED");
        assert_eq!(flight_mode_label("Copter", 5), "LOITER");
        assert_eq!(flight_mode_label("Plane", 11), "RTL");
        assert_eq!(flight_mode_label("Rover", 3), "STEERING");
        assert_eq!(flight_mode_label("Sub", 9), "SURFACE");
        assert_eq!(flight_mode_label("Copter", 99), "MODE_99");
    }
}
