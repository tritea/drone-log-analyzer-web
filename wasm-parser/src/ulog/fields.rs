//! Topic field readers, ported from `app/modules/parser/ulog/fields.go`.

use crate::format::FormatDef;

/// Reads an unsigned 1/2/4-byte little-endian field by name.
pub fn read_scalar_uint(fd: &FormatDef, data: &[u8], name: &str) -> Option<u64> {
    for fl in &fd.layout {
        if fl.name != name || !fl.in_body {
            continue;
        }
        let off = fl.offset;
        match fl.size {
            1 if off < data.len() => return Some(data[off] as u64),
            2 if off + 2 <= data.len() => {
                return Some(u16::from_le_bytes([data[off], data[off + 1]]) as u64)
            }
            4 if off + 4 <= data.len() => {
                return Some(u32::from_le_bytes([
                    data[off],
                    data[off + 1],
                    data[off + 2],
                    data[off + 3],
                ]) as u64)
            }
            _ => {}
        }
    }
    None
}

/// Reads a 4- or 8-byte float field by name, sanitizing NaN/Inf values to 0
/// so downstream consumers never see non-finite floats.
pub fn read_scalar_float(fd: &FormatDef, data: &[u8], name: &str) -> Option<f64> {
    for fl in &fd.layout {
        if fl.name != name || !fl.in_body {
            continue;
        }
        let off = fl.offset;
        match fl.size {
            4 if off + 4 <= data.len() => {
                let bits =
                    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
                return Some(clean_float(f32::from_bits(bits) as f64));
            }
            8 if off + 8 <= data.len() => {
                let mut b = [0u8; 8];
                b.copy_from_slice(&data[off..off + 8]);
                return Some(clean_float(f64::from_le_bytes(b)));
            }
            _ => {}
        }
    }
    None
}

/// Returns 0 for NaN/Inf, otherwise v unchanged.
fn clean_float(v: f64) -> f64 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

/// Reads an 8-byte little-endian u64 field by name.
pub fn read_scalar_uint64(fd: &FormatDef, data: &[u8], name: &str) -> Option<u64> {
    for fl in &fd.layout {
        if fl.name != name || fl.size != 8 {
            continue;
        }
        if fl.offset + 8 <= data.len() {
            let mut b = [0u8; 8];
            b.copy_from_slice(&data[fl.offset..fl.offset + 8]);
            return Some(u64::from_le_bytes(b));
        }
    }
    None
}

/// Returns the topic sample's timestamp in microseconds, or 0.
pub fn read_topic_time(fd: &FormatDef, data: &[u8]) -> u64 {
    for fl in &fd.layout {
        if fl.size == 8 && looks_like_timestamp(&fl.name) && fl.offset + 8 <= data.len() {
            let mut b = [0u8; 8];
            b.copy_from_slice(&data[fl.offset..fl.offset + 8]);
            return u64::from_le_bytes(b);
        }
    }
    0
}

/// Whether a field name is a ULog timestamp field.
fn looks_like_timestamp(name: &str) -> bool {
    let low = name.to_ascii_lowercase();
    low == "timestamp" || low == "timestamp_sample" || low.contains("timestamp")
}

/// Returns the NUL-terminated string at the start of b (the Go original also
/// returns the remainder; its only caller ignores it).
pub fn cut_cstring(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).to_string()
}
