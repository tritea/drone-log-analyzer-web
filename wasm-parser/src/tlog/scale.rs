//! Unit-scale rules for MAVLink wire fields, ported from
//! `app/modules/parser/tlog/layout.go`.

/// Field kinds as emitted by the dialect table generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    F32,
    Char,
}

impl FieldKind {
    pub fn parse(s: &str) -> Option<FieldKind> {
        Some(match s {
            "u8" => FieldKind::U8,
            "i8" => FieldKind::I8,
            "u16" => FieldKind::U16,
            "i16" => FieldKind::I16,
            "u32" => FieldKind::U32,
            "i32" => FieldKind::I32,
            "f32" => FieldKind::F32,
            "char" => FieldKind::Char,
            _ => return None,
        })
    }

    /// Wire width of one value. f64 collapses to 4 bytes to match MAVLink's
    /// float-only wire encoding (Go glTypeFor does the same).
    pub fn size(self) -> usize {
        match self {
            FieldKind::U8 | FieldKind::I8 => 1,
            FieldKind::U16 | FieldKind::I16 => 2,
            FieldKind::U32 | FieldKind::I32 | FieldKind::F32 => 4,
            FieldKind::Char => 1,
        }
    }

    fn is_int(self) -> bool {
        matches!(
            self,
            FieldKind::U16 | FieldKind::I16 | FieldKind::U32 | FieldKind::I32
        )
    }

    fn is_float(self) -> bool {
        matches!(self, FieldKind::F32)
    }
}

/// The multiplier that converts a raw field into display units: 1e-7 for
/// integer lat/lon degrees, 1e-3 for integer altitudes in millimeters, 0.01
/// for integer centi-units, and radian->degree for attitude angles.
pub fn unit_scale_for(name: &str, k: FieldKind) -> f64 {
    let low = name.to_ascii_lowercase();
    if k.is_int() {
        if (k == FieldKind::U32 || k == FieldKind::I32) && is_geographic_field(&low) {
            return 1e-7;
        }
        if k == FieldKind::I32 && is_altitude_field(&low) {
            return 1e-3;
        }
        if is_centi_field(&low) {
            return 0.01;
        }
    }
    if k.is_float() && is_angle_radians(&low) {
        return std::f64::consts::PI.recip() * 180.0;
    }
    1.0
}

/// Integer fields using MAVLink's ×100 wire encoding. Voltages are
/// deliberately excluded — BATTERY2 uses mV while ESC_STATUS uses cV, so the
/// name alone cannot disambiguate them.
fn is_centi_field(low: &str) -> bool {
    matches!(
        low,
        "vel"
            | "vx"
            | "vy"
            | "vz"
            | "airspeed"
            | "eph"
            | "epv"
            | "hacc"
            | "vacc"
            | "velacc"
            | "hdgacc"
            | "currentbattery"
            | "currentdistance"
            | "mindistance"
            | "maxdistance"
    )
}

fn is_geographic_field(low: &str) -> bool {
    matches!(low, "lat" | "lon" | "lng" | "latint" | "lonint")
        || low.contains("latitude")
        || low.contains("longitude")
}

fn is_angle_radians(low: &str) -> bool {
    matches!(
        low,
        "roll" | "pitch" | "yaw" | "rollspeed" | "pitchspeed" | "yawspeed"
    )
}

fn is_altitude_field(low: &str) -> bool {
    matches!(
        low,
        "alt"
            | "relative_alt"
            | "relativealt"
            | "altitude"
            | "alt_ellipsoid"
            | "altellipsoid"
            | "altitudemonotonic"
            | "altitudeamsl"
            | "altitudelocal"
            | "altituderelative"
            | "altitudeterrain"
            | "bottomclearance"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_match_go() {
        assert_eq!(unit_scale_for("Lat", FieldKind::I32), 1e-7);
        assert_eq!(unit_scale_for("Alt", FieldKind::I32), 1e-3);
        assert_eq!(unit_scale_for("Alt", FieldKind::F32), 1.0);
        assert_eq!(unit_scale_for("Vx", FieldKind::I16), 0.01);
        assert_eq!(
            unit_scale_for("Roll", FieldKind::F32),
            180.0 / std::f64::consts::PI
        );
        assert_eq!(unit_scale_for("Roll", FieldKind::I16), 1.0);
        assert_eq!(unit_scale_for("Voltages", FieldKind::F32), 1.0);
        // Voltages excluded from centi by name.
        assert_eq!(unit_scale_for("voltage_battery", FieldKind::U16), 1.0);
    }
}
