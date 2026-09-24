//! Info/param scalar coercion, ported from
//! `app/modules/parser/ulog/values.go`.

/// A coerced info/param scalar: the narrowest numeric shape or a trimmed
/// string (mirrors the Go `any` returns of `coerceScalar`).
#[derive(Debug, Clone)]
pub enum InfoValue {
    I64(i64),
    U32(u32),
    I32(i32),
    U16(u16),
    I16(i16),
    F64(f64),
    F32(f32),
    Bool(bool),
    Str(String),
}

impl InfoValue {
    /// Go `asFloat64`: numeric types convert; strings parse or fail.
    pub fn as_float64(&self) -> Option<f64> {
        match self {
            InfoValue::I64(v) => Some(*v as f64),
            InfoValue::U32(v) => Some(*v as f64),
            InfoValue::I32(v) => Some(*v as f64),
            InfoValue::U16(v) => Some(*v as f64),
            InfoValue::I16(v) => Some(*v as f64),
            InfoValue::F64(v) => Some(*v),
            InfoValue::F32(v) => Some(*v as f64),
            // Go: int8/uint8/bool never come out of coerceScalar's match.
            InfoValue::Bool(_) => None,
            InfoValue::Str(s) => s.trim().parse::<f64>().ok(),
        }
    }

    /// Go `asText`: strings trimmed (hex when unprintable); floats integral
    /// ones render without decimals.
    pub fn as_text(&self) -> String {
        match self {
            InfoValue::Str(s) => {
                let s = s.trim();
                if has_unprintable(s) {
                    hex_encode(s.as_bytes())
                } else {
                    s.to_string()
                }
            }
            InfoValue::F64(x) => {
                if *x == *x as i64 as f64 {
                    format!("{}", *x as i64)
                } else {
                    format!("{}", x)
                }
            }
            other => format!("{:?}", other),
        }
    }
}

fn hex_encode(b: &[u8]) -> String {
    b.iter().map(|byte| format!("{:02x}", byte)).collect()
}

/// Interprets b according to a ULog info/param type string.
pub fn coerce_scalar(b: &[u8], type_str: &str) -> InfoValue {
    if type_str.starts_with("char") {
        return InfoValue::Str(
            String::from_utf8_lossy(b)
                .trim_end_matches('\0')
                .to_string(),
        );
    }
    if type_str.contains("uint64") || type_str.contains("int64") {
        if b.len() >= 8 {
            let mut w = [0u8; 8];
            w.copy_from_slice(&b[..8]);
            return InfoValue::I64(u64::from_le_bytes(w) as i64);
        }
    } else if type_str.contains("uint32") {
        if b.len() >= 4 {
            return InfoValue::U32(u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        }
    } else if type_str.contains("int32") {
        if b.len() >= 4 {
            return InfoValue::I32(i32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        }
    } else if type_str.contains("uint16") {
        if b.len() >= 2 {
            return InfoValue::U16(u16::from_le_bytes([b[0], b[1]]));
        }
    } else if type_str.contains("int16") {
        if b.len() >= 2 {
            return InfoValue::I16(i16::from_le_bytes([b[0], b[1]]));
        }
    } else if type_str.contains("double") {
        if b.len() >= 8 {
            let mut w = [0u8; 8];
            w.copy_from_slice(&b[..8]);
            return InfoValue::F64(f64::from_le_bytes(w));
        }
    } else if type_str.contains("float") {
        if b.len() >= 4 {
            return InfoValue::F32(f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        }
    } else if type_str == "bool" && !b.is_empty() {
        return InfoValue::Bool(b[0] != 0);
    }
    InfoValue::Str(
        String::from_utf8_lossy(b)
            .trim_end_matches('\0')
            .to_string(),
    )
}

/// Whether s contains non-printable bytes (Go `hasUnprintable`).
pub fn has_unprintable(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.bytes()
        .any(|b| (b < 0x20 && b != b'\t' && b != b'\n' && b != b'\r') || b >= 0x7f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coerces_typed_scalars() {
        let f = coerce_scalar(&42.0f32.to_le_bytes(), "float");
        assert_eq!(f.as_float64(), Some(42.0));

        let u = coerce_scalar(&0x12345678u32.to_le_bytes(), "uint32_t");
        assert_eq!(u.as_float64(), Some(0x12345678 as f64));

        let d = coerce_scalar(&1.5f64.to_le_bytes(), "double");
        assert_eq!(d.as_float64(), Some(1.5));

        let s = coerce_scalar(b"hello\0\0", "char[16]");
        assert_eq!(s.as_text(), "hello");
    }

    #[test]
    fn text_hex_encodes_unprintables() {
        let v = InfoValue::Str("\u{1}\u{2}x".to_string());
        assert_eq!(v.as_text(), "010278");
    }

    #[test]
    fn integral_float_text() {
        assert_eq!(InfoValue::F64(1700.0).as_text(), "1700");
    }
}
