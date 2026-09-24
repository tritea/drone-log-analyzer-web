//! Binary field decoding, ported from
//! `app/modules/parser/dataflash/fielddecode.go`. Decoded values are either
//! numbers (already f64, matching Go's numeric `any` cases) or strings.

use crate::format::FormatDef;

/// One decoded record value. Mirrors the Go `any` returns of `readPrimitive`:
/// numbers collapse to f64 (ToFloat64-exact for i8..i32/u32/f32; u64/i64
/// beyond 2^53 lose precision identically to Go), strings keep their bytes.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    Num(f64),
    Str(String),
}

impl FieldValue {
    /// Go `parser.ToFloat64`: strings parse defensively, NaN on failure.
    pub fn to_f64(&self) -> f64 {
        match self {
            FieldValue::Num(v) => *v,
            FieldValue::Str(s) => s.parse::<f64>().unwrap_or(f64::NAN),
        }
    }
}

fn le_i16(d: &[u8], o: usize) -> Option<i16> {
    d.get(o..o + 2).map(|s| i16::from_le_bytes([s[0], s[1]]))
}
fn le_u16(d: &[u8], o: usize) -> Option<u16> {
    d.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}
fn le_i32(d: &[u8], o: usize) -> Option<i32> {
    d.get(o..o + 4)
        .map(|s| i32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}
fn le_u32(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}
fn le_u64(d: &[u8], o: usize) -> Option<u64> {
    d.get(o..o + 8)
        .map(|s| u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
}
fn le_f32(d: &[u8], o: usize) -> Option<f32> {
    le_u32(d, o).map(f32::from_bits)
}
fn le_f64(d: &[u8], o: usize) -> Option<f64> {
    le_u64(d, o).map(f64::from_bits)
}

fn trimmed_str(d: &[u8], o: usize, n: usize) -> Option<FieldValue> {
    d.get(o..o + n).map(|s| {
        FieldValue::Str(
            String::from_utf8_lossy(s)
                .trim_end_matches('\0')
                .to_string(),
        )
    })
}

/// Decodes a single little-endian value of the given format code from
/// `data[offset:]`. Returns the decoded value and the number of bytes
/// consumed. Out-of-range reads yield `None` (the Go original would panic;
/// wasm must never).
pub fn read_primitive(ch: u8, data: &[u8], offset: usize) -> Option<(FieldValue, usize)> {
    let v = match ch {
        b'b' => FieldValue::Num(*data.get(offset)? as i8 as f64),
        b'B' | b'M' => FieldValue::Num(*data.get(offset)? as f64),
        b'h' => FieldValue::Num(le_i16(data, offset)? as f64),
        b'H' => FieldValue::Num(le_u16(data, offset)? as f64),
        b'i' => FieldValue::Num(le_i32(data, offset)? as f64),
        b'I' => FieldValue::Num(le_u32(data, offset)? as f64),
        b'q' => FieldValue::Num(le_u64(data, offset)? as i64 as f64),
        b'Q' => FieldValue::Num(le_u64(data, offset)? as f64),
        b'f' => FieldValue::Num(le_f32(data, offset)? as f64),
        b'd' => FieldValue::Num(le_f64(data, offset)?),
        b'c' => FieldValue::Num(le_i16(data, offset)? as f64 / 100.0),
        b'C' => FieldValue::Num(le_u16(data, offset)? as f64 / 100.0),
        b'e' => FieldValue::Num(le_i32(data, offset)? as f64 / 100.0),
        b'E' => FieldValue::Num(le_u32(data, offset)? as f64 / 100.0),
        b'L' => FieldValue::Num(le_i32(data, offset)? as f64 / 1e7),
        b'n' => trimmed_str(data, offset, 4)?,
        b'N' => trimmed_str(data, offset, 16)?,
        b'Z' => trimmed_str(data, offset, 64)?,
        _ => return None,
    };
    Some((v, byte_width_of_pub(ch)))
}

fn byte_width_of_pub(ch: u8) -> usize {
    super::schema::byte_width_of(ch)
}

/// Position of the first field whose name matches any of the candidates
/// (case-insensitive, trimmed), or None.
pub fn index_of_named_field(fd: &FormatDef, names: &[&str]) -> Option<usize> {
    for (i, fn_) in fd.field_names.iter().enumerate() {
        let f = fn_.trim().to_ascii_lowercase();
        for n in names {
            if f == n.trim().to_ascii_lowercase() {
                return Some(i);
            }
        }
    }
    None
}

/// Byte offset of field idx within a record, summing the byte widths of the
/// preceding format characters.
pub fn byte_offset_of_field(fd: &FormatDef, idx: usize) -> usize {
    let mut off = 0;
    for &ch in fd.format_str.as_bytes().iter().take(idx) {
        off += byte_width_of_pub(ch);
    }
    off
}

/// Decodes the first field matching any candidate name.
pub fn read_field_by_name(fd: &FormatDef, data: &[u8], names: &[&str]) -> Option<FieldValue> {
    let idx = index_of_named_field(fd, names)?;
    let ch = *fd.format_str.as_bytes().get(idx)?;
    read_primitive(ch, data, byte_offset_of_field(fd, idx)).map(|(v, _)| v)
}

/// Decodes every field of a binary record, stopping at the end of data.
pub fn read_all_fields(fd: &FormatDef, data: &[u8]) -> Vec<FieldValue> {
    let mut values = Vec::with_capacity(fd.format_str.len());
    let mut off = 0;
    for &ch in fd.format_str.as_bytes() {
        if off >= data.len() {
            break;
        }
        match read_primitive(ch, data, off) {
            Some((v, size)) => {
                values.push(v);
                off += size;
            }
            None => break, // unknown char or truncated tail: Go would misbehave; we stop
        }
    }
    values
}

/// True for the timestamp field names TimeMS/Time/TimeUS.
pub fn is_timestamp_field(fn_: &str) -> bool {
    fn_ == "TimeMS" || fn_ == "Time" || fn_ == "TimeUS"
}

/// True for the instance field names I/Instance/Inst.
pub fn is_instance_field(fn_: &str) -> bool {
    fn_ == "I" || fn_ == "Instance" || fn_ == "Inst"
}

/// TimeUS is microseconds; everything else is already milliseconds.
pub fn normalize_time_micros(fn_: &str, raw: f64) -> f64 {
    if fn_ == "TimeUS" {
        raw / 1000.0
    } else {
        raw
    }
}

/// Instance ids must be finite and non-negative.
pub fn to_instance_id(v: f64) -> Option<usize> {
    if v.is_finite() && v >= 0.0 {
        Some(v as usize)
    } else {
        None
    }
}

/// Extracts the TimeMS/Time (millis) or TimeUS (micros /1000) timestamp from
/// a binary record.
pub fn read_time_millis(fd: &FormatDef, data: &[u8]) -> f64 {
    for (i, fn_) in fd.field_names.iter().enumerate() {
        let Some(&ch) = fd.format_str.as_bytes().get(i) else {
            break;
        };
        if is_timestamp_field(fn_) {
            if let Some((v, _)) = read_primitive(ch, data, byte_offset_of_field(fd, i)) {
                return normalize_time_micros(fn_, v.to_f64());
            }
        }
    }
    0.0
}

/// Extracts the Instance/I/Inst field of a binary record.
pub fn read_instance_id(fd: &FormatDef, data: &[u8]) -> Option<usize> {
    for (i, fn_) in fd.field_names.iter().enumerate() {
        let Some(&ch) = fd.format_str.as_bytes().get(i) else {
            break;
        };
        if is_instance_field(fn_) {
            if let Some((v, _)) = read_primitive(ch, data, byte_offset_of_field(fd, i)) {
                return to_instance_id(v.to_f64());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_primitives() {
        let mut b = vec![0u8; 16];
        b[..2].copy_from_slice(&0xFFFDu16.to_le_bytes());
        let (v, n) = read_primitive(b'h', &b, 0).unwrap();
        assert_eq!((v, n), (FieldValue::Num(-3.0), 2));

        b[..4].copy_from_slice(&1.5f32.to_le_bytes());
        let (v, _) = read_primitive(b'f', &b, 0).unwrap();
        assert_eq!(v, FieldValue::Num(1.5));

        // centi-scaled
        b[..2].copy_from_slice(&1234u16.to_le_bytes());
        let (v, _) = read_primitive(b'C', &b, 0).unwrap();
        assert_eq!(v, FieldValue::Num(12.34));
        // lat/lon scaled
        b[..4].copy_from_slice(&345678901i32.to_le_bytes());
        let (v, _) = read_primitive(b'L', &b, 0).unwrap();
        assert!((v.to_f64() - 34.5678901).abs() < 1e-12);
        // nul-terminated string
        let s = [b'a', b'b', 0, 0];
        let (v, n) = read_primitive(b'n', &s, 0).unwrap();
        assert_eq!((v, n), (FieldValue::Str("ab".into()), 4));
    }

    #[test]
    fn truncated_read_is_none() {
        let b = [1u8, 2];
        assert!(read_primitive(b'i', &b, 0).is_none());
    }
}
