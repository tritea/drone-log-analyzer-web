//! Field types and format definitions, ported from `app/modules/parser/format.go`.

/// Compact numeric type tag used for curve samples in the binary body layout.
/// Mirrors the WebGL attribute types the frontend reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FieldGlType {
    Int8 = 0,
    Uint8 = 1,
    Int16 = 2,
    Uint16 = 3,
    Int32 = 4,
    Uint32 = 5,
    Float32 = 6,
}

impl FieldGlType {
    /// Byte width of a single value of this type.
    pub fn size(self) -> usize {
        match self {
            FieldGlType::Int8 | FieldGlType::Uint8 => 1,
            FieldGlType::Int16 | FieldGlType::Uint16 => 2,
            _ => 4,
        }
    }

    pub fn from_u8(v: u8) -> Option<FieldGlType> {
        Some(match v {
            0 => FieldGlType::Int8,
            1 => FieldGlType::Uint8,
            2 => FieldGlType::Int16,
            3 => FieldGlType::Uint16,
            4 => FieldGlType::Int32,
            5 => FieldGlType::Uint32,
            6 => FieldGlType::Float32,
            _ => return None,
        })
    }
}

/// Rounds `off` up to the next multiple of `a` (a must be a power of two) so
/// each column in the row buffer starts on a naturally aligned boundary.
pub fn align_offset(off: usize, a: usize) -> usize {
    (off + a - 1) & !(a - 1)
}

/// One parsed column of a message format: its decoded numeric shape plus the
/// metadata needed to map raw bytes into a curve.
#[derive(Debug, Clone)]
pub struct FieldLayout {
    pub name: String,
    pub gl_type: FieldGlType,
    pub size: usize,
    pub scale: f64,
    pub offset: usize,
    pub in_body: bool,
    pub orig_type: String,
}

/// The decoded FMT record for a message type: how to split its payload into
/// named, typed fields.
#[derive(Debug, Clone, Default)]
pub struct FormatDef {
    pub msg_type: u8,
    pub msg_len: u8,
    pub name: String,
    pub format_str: String,
    pub field_names: Vec<String>,
    pub layout: Vec<FieldLayout>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gl_sizes() {
        assert_eq!(FieldGlType::Int8.size(), 1);
        assert_eq!(FieldGlType::Uint16.size(), 2);
        assert_eq!(FieldGlType::Int32.size(), 4);
        assert_eq!(FieldGlType::Float32.size(), 4);
    }

    #[test]
    fn alignment() {
        assert_eq!(align_offset(0, 4), 0);
        assert_eq!(align_offset(1, 4), 4);
        assert_eq!(align_offset(5, 2), 6);
        assert_eq!(align_offset(8, 4), 8);
    }
}
