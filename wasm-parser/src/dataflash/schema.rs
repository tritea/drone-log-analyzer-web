//! Format-character schema, ported from `app/modules/parser/dataflash/schema.go`.

use crate::format::{FieldGlType, FieldLayout, FormatDef};

/// Maps a DataFlash format character to its WebGL field type and the scale
/// factor applied when rendering raw integer values as floats.
/// Returns (glType, scale, inBody).
pub fn gl_type_for_code(ch: u8) -> Option<(FieldGlType, f64, bool)> {
    Some(match ch {
        b'b' => (FieldGlType::Int8, 1.0, true),
        b'B' | b'M' => (FieldGlType::Uint8, 1.0, true),
        b'h' => (FieldGlType::Int16, 1.0, true),
        b'H' => (FieldGlType::Uint16, 1.0, true),
        b'i' => (FieldGlType::Int32, 1.0, true),
        b'I' => (FieldGlType::Uint32, 1.0, true),
        b'f' => (FieldGlType::Float32, 1.0, true),
        b'c' => (FieldGlType::Int16, 0.01, true),
        b'C' => (FieldGlType::Uint16, 0.01, true),
        b'e' => (FieldGlType::Int32, 0.01, true),
        b'E' => (FieldGlType::Uint32, 0.01, true),
        b'L' => (FieldGlType::Int32, 1e-7, true),
        _ => return None,
    })
}

/// On-disk size in bytes of a single format character.
pub fn byte_width_of(ch: u8) -> usize {
    match ch {
        b'b' | b'B' | b'M' => 1,
        b'h' | b'H' | b'c' | b'C' => 2,
        b'i' | b'I' | b'e' | b'E' | b'L' | b'f' => 4,
        b'q' | b'Q' | b'd' => 8,
        b'n' => 4,
        b'N' => 16,
        b'Z' => 64,
        _ => 0,
    }
}

/// Derives the per-field byte layout of a FormatDef from its
/// format_str/field_names. Idempotent: a format that already has a layout or
/// no format string is left untouched.
pub fn assemble_layout(fd: &mut FormatDef) {
    if !fd.layout.is_empty() || fd.format_str.is_empty() {
        return;
    }
    let mut off = 0;
    let fmt_bytes = fd.format_str.as_bytes().to_vec();
    for (i, ch) in fmt_bytes.iter().enumerate() {
        let size = byte_width_of(*ch);
        // Go's glTypeForCode returns the zero GLType (Int8) with scale 0 for
        // unknown characters; layout entries are still appended.
        let (gl_type, scale, in_body) =
            gl_type_for_code(*ch).unwrap_or((FieldGlType::Int8, 0.0, false));
        if i < fd.field_names.len() {
            fd.layout.push(FieldLayout {
                name: fd.field_names[i].clone(),
                gl_type,
                size,
                scale,
                offset: off,
                in_body,
                orig_type: (*ch as char).to_string(),
            });
        }
        off += size;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_go_offsets() {
        // Mirrors ulog/dataflash expectations: Q(8) then B(1) then f(4).
        let mut fd = FormatDef {
            format_str: "QBf".into(),
            field_names: vec!["TimeUS".into(), "I".into(), "Val".into()],
            ..Default::default()
        };
        assemble_layout(&mut fd);
        assert_eq!(fd.layout.len(), 3);
        assert_eq!((fd.layout[0].offset, fd.layout[0].in_body), (0, false));
        assert_eq!((fd.layout[1].offset, fd.layout[1].size), (8, 1));
        assert_eq!(
            (fd.layout[2].offset, fd.layout[2].gl_type),
            (9, FieldGlType::Float32)
        );
    }

    #[test]
    fn scaled_codes() {
        assert_eq!(
            gl_type_for_code(b'L'),
            Some((FieldGlType::Int32, 1e-7, true))
        );
        assert_eq!(
            gl_type_for_code(b'c'),
            Some((FieldGlType::Int16, 0.01, true))
        );
        assert_eq!(gl_type_for_code(b'Z'), None);
        assert_eq!(byte_width_of(b'Q'), 8);
        assert_eq!(byte_width_of(b'N'), 16);
        assert_eq!(byte_width_of(b'x'), 0);
    }
}
