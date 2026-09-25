//! Curve/type-body wire types and decoding, ported from
//! `app/modules/parser/curvebin.go`.

use crate::format::FieldGlType;

/// On-disk wire constants for the curve and type-body blobs handed to the
/// frontend. The magic lets the renderer sanity-check the buffer.
pub const CURVE_BIN_MAGIC: u32 = 0x4E55_4342;
pub const CURVE_BIN_VERSION: u32 = 1;
pub const CURVE_BIN_HEADER: usize = 28;

/// Type-body blob constants.
pub const TYPE_BIN_MAGIC: u32 = 0x4E55_5442;
pub const TYPE_BIN_VERSION: u32 = 1;
pub const TYPE_BIN_HEADER: usize = 32;
pub const TIME_COL_BYTES: usize = 4;

/// One numeric series for a (message, field) pair. While a log is being
/// parsed it holds growing times/samples vectors; after finalize the data is
/// packed into a [`TypeBody`] and the vectors are released.
#[derive(Debug, Clone, Default)]
pub struct CurveData {
    pub name: String,
    pub unit: String,
    pub instance: String,
    pub min: f64,
    pub max: f64,
    pub times: Vec<f64>,
    pub samples: Vec<f32>,
}

impl CurveData {
    /// How many samples the curve holds.
    pub fn count(&self) -> usize {
        self.samples.len()
    }
}

/// Describes one column within a [`TypeBody`].
#[derive(Debug, Clone)]
pub struct TypeField {
    pub name: String,
    pub gl_type: FieldGlType,
    pub scale: f64,
    pub offset: usize,
    pub min: f64,
    pub max: f64,
    pub count: usize,
}

/// The packed, fixed-stride row table for one message type, prefixed by
/// [`TYPE_BIN_HEADER`] bytes (magic/version/rowCount/fieldCount/stride/pad/base).
#[derive(Debug, Clone, Default)]
pub struct TypeBody {
    pub fields: Vec<TypeField>,
    pub row_count: usize,
    pub stride: usize,
    pub base_time_ms: f64,
    pub bin: Vec<u8>,
}

impl TypeBody {
    /// Finds a column by field name, returning its index.
    pub fn field_index(&self, name: &str) -> Option<usize> {
        self.fields.iter().position(|f| f.name == name)
    }
}

/// JSON-friendly projections used by the schema query (serde field order and
/// names mirror the Go `TypeSchemaField`/`TypeSchemaEntry` JSON tags exactly).
#[derive(Debug, Clone, serde::Serialize)]
pub struct TypeSchemaField {
    pub name: String,
    #[serde(rename = "glType")]
    pub gl_type: u8,
    pub scale: f64,
    pub offset: usize,
    pub min: f64,
    pub max: f64,
    pub count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TypeSchemaEntry {
    pub fields: Vec<TypeSchemaField>,
    #[serde(rename = "rowCount")]
    pub row_count: usize,
    pub stride: usize,
    #[serde(rename = "baseTimeMs")]
    pub base_time_ms: f64,
}

/// Decodes one little-endian numeric value of the given wire type from `body`
/// at byte offset `off`. Returns 0.0 for out-of-range reads (the Go original
/// would panic; wasm must never).
pub fn read_field_gl(body: &[u8], off: usize, gl: FieldGlType) -> f64 {
    match gl {
        FieldGlType::Int8 => body.get(off).map(|&b| b as i8 as f64).unwrap_or(0.0),
        FieldGlType::Uint8 => body.get(off).map(|&b| b as f64).unwrap_or(0.0),
        FieldGlType::Int16 => body
            .get(off..off + 2)
            .map(|s| i16::from_le_bytes([s[0], s[1]]) as f64)
            .unwrap_or(0.0),
        FieldGlType::Uint16 => body
            .get(off..off + 2)
            .map(|s| u16::from_le_bytes([s[0], s[1]]) as f64)
            .unwrap_or(0.0),
        FieldGlType::Int32 => body
            .get(off..off + 4)
            .map(|s| i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64)
            .unwrap_or(0.0),
        FieldGlType::Uint32 => body
            .get(off..off + 4)
            .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64)
            .unwrap_or(0.0),
        FieldGlType::Float32 => body
            .get(off..off + 4)
            .map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64)
            .unwrap_or(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_little_endian_values() {
        let mut b = [0u8; 8];
        b[0] = 0xFF; // int8 -1
        assert_eq!(read_field_gl(&b, 0, FieldGlType::Int8), -1.0);
        assert_eq!(read_field_gl(&b, 0, FieldGlType::Uint8), 255.0);

        b[..2].copy_from_slice(&0xFFFDu16.to_le_bytes()); // int16 -3
        assert_eq!(read_field_gl(&b, 0, FieldGlType::Int16), -3.0);
        assert_eq!(read_field_gl(&b, 0, FieldGlType::Uint16), 0xFFFD as f64);

        b[..4].copy_from_slice(&1.5f32.to_le_bytes());
        assert_eq!(read_field_gl(&b, 0, FieldGlType::Float32), 1.5);

        b[..4].copy_from_slice(&(-7i32).to_le_bytes());
        assert_eq!(read_field_gl(&b, 0, FieldGlType::Int32), -7.0);
    }

    #[test]
    fn out_of_range_reads_zero() {
        let b = [1u8, 2];
        assert_eq!(read_field_gl(&b, 0, FieldGlType::Int32), 0.0);
        assert_eq!(read_field_gl(&b, 9, FieldGlType::Uint8), 0.0);
    }
}
