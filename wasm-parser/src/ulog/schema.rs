//! ULog type-spec parsing and layout assembly, ported from
//! `app/modules/parser/ulog/schema.go`.

use crate::format::{FieldGlType, FieldLayout, FormatDef};

/// One parsed "<ctype> <name>" token from a ULog format string.
pub struct DeclField {
    pub c_type: String,
    pub name: String,
}

/// Splits a ULog format body (e.g. "uint64_t timestamp;float x") into its
/// constituent field declarations.
pub fn parse_type_spec(spec: &str) -> Vec<DeclField> {
    let mut fields = Vec::new();
    for seg in spec.split(';') {
        let seg = seg.trim();
        if seg.is_empty() {
            continue;
        }
        let tok: Vec<&str> = seg.split_whitespace().collect();
        if tok.len() < 2 {
            continue;
        }
        fields.push(DeclField {
            c_type: tok[0].to_string(),
            name: tok[tok.len() - 1].to_string(),
        });
    }
    fields
}

/// Byte size of a scalar C type, defaulting to 1.
fn primitive_byte_len(ctype: &str) -> usize {
    match ctype {
        "uint8_t" | "int8_t" | "bool" | "char" => 1,
        "uint16_t" | "int16_t" => 2,
        "uint32_t" | "int32_t" | "float" => 4,
        "uint64_t" | "int64_t" | "double" => 8,
        _ => 1,
    }
}

/// Maps a (possibly array) C type to its graph-layout descriptor.
/// In-body scalars become chartable values; arrays and char buffers do not.
/// Returns (glType, size, scale, inBody).
pub fn map_scalar(ctype: &str) -> (FieldGlType, usize, f64, bool) {
    if let Some(i) = ctype.find('[') {
        if i > 0 {
            let elem = &ctype[..i];
            let cnt = array_length(&ctype[i..]);
            return (
                FieldGlType::Int8,
                primitive_byte_len(elem) * cnt,
                1.0,
                false,
            );
        }
    }
    match ctype {
        "uint8_t" | "bool" => (FieldGlType::Uint8, 1, 1.0, true),
        "int8_t" => (FieldGlType::Int8, 1, 1.0, true),
        "uint16_t" => (FieldGlType::Uint16, 2, 1.0, true),
        "int16_t" => (FieldGlType::Int16, 2, 1.0, true),
        "uint32_t" => (FieldGlType::Uint32, 4, 1.0, true),
        "int32_t" => (FieldGlType::Int32, 4, 1.0, true),
        "float" => (FieldGlType::Float32, 4, 1.0, true),
        "char" => (FieldGlType::Int8, 1, 1.0, false),
        "double" => (FieldGlType::Float32, 8, 1.0, true),
        "uint64_t" | "int64_t" => (FieldGlType::Float32, 8, 1.0, false),
        _ => (FieldGlType::Int8, 0, 1.0, false),
    }
}

/// Parses the integer inside "[n]", returning 0 on a malformed span.
fn array_length(s: &str) -> usize {
    let b = s.as_bytes();
    if b.len() < 3 || b[0] != b'[' {
        return 0;
    }
    let Some(end) = s.find(']') else { return 0 };
    let mut n: usize = 0;
    for c in s[1..end].chars() {
        if !c.is_ascii_digit() {
            return 0;
        }
        n = n * 10 + (c as usize - '0' as usize);
    }
    n
}

/// Populates fd.field_names and fd.layout from the parsed declarations,
/// computing per-field offsets and applying the lat/lon rule.
pub fn assemble_layout(fd: &mut FormatDef, fields: &[DeclField]) {
    fd.field_names = Vec::with_capacity(fields.len());
    let mut off = 0usize;
    for f in fields {
        if let Some((elem_type, count)) = expandable_array(&f.c_type) {
            let (elem_gl, elem_size, _, _) = map_scalar(&elem_type);
            for i in 0..count {
                let name = format!("{}[{}]", f.name, i);
                fd.field_names.push(name.clone());
                fd.layout.push(FieldLayout {
                    name,
                    gl_type: elem_gl,
                    size: elem_size,
                    scale: 1.0,
                    offset: off,
                    in_body: true,
                    orig_type: elem_type.clone(),
                });
                off += elem_size;
            }
            continue;
        }
        let (mut gl, size, mut scale, mut in_body) = map_scalar(&f.c_type);
        if is_geo_coord(&f.name) {
            match f.c_type.as_str() {
                "int32_t" | "uint32_t" | "double" => {
                    gl = FieldGlType::Int32;
                    scale = 1e-7;
                    in_body = true;
                }
                _ => {}
            }
        }
        fd.field_names.push(f.name.clone());
        fd.layout.push(FieldLayout {
            name: f.name.clone(),
            gl_type: gl,
            size,
            scale,
            offset: off,
            in_body,
            orig_type: f.c_type.clone(),
        });
        off += size;
    }
}

/// Whether name looks like a latitude/longitude field.
fn is_geo_coord(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    matches!(n.as_str(), "lat" | "lon" | "lng" | "latitude" | "longitude")
        || n.contains("latitude")
        || n.contains("longitude")
}

/// Returns the element type and count for array types the chart layer wants
/// expanded into indexed sub-fields.
fn expandable_array(ctype: &str) -> Option<(String, usize)> {
    let i = ctype.find('[')?;
    if i == 0 {
        return None;
    }
    let elem_type = &ctype[..i];
    let count = array_length(&ctype[i..]);
    match elem_type {
        "float" | "double" | "int16_t" | "uint16_t" | "int32_t" | "uint32_t" | "int64_t"
        | "uint64_t" => Some((elem_type.to_string(), count)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_mapping_matches_go() {
        // (ctype, want size, want inBody)
        for (ctype, size, in_body) in [
            ("uint8_t", 1, true),
            ("int16_t", 2, true),
            ("uint32_t", 4, true),
            ("float", 4, true),
            ("uint64_t", 8, false),
            ("double", 8, true),
            ("char[16]", 16, false),
            ("float[4]", 16, false),
        ] {
            let (_, got_size, _, got_body) = map_scalar(ctype);
            assert_eq!(
                (got_size, got_body),
                (size, in_body),
                "map_scalar({})",
                ctype
            );
        }
    }

    #[test]
    fn layout_offsets() {
        let mut fd = FormatDef::default();
        let spec = parse_type_spec("uint64_t timestamp;uint8_t mode;float x;float y");
        assemble_layout(&mut fd, &spec);
        assert_eq!(fd.layout.len(), 4);
        assert_eq!(
            (
                fd.layout[0].name.as_str(),
                fd.layout[0].offset,
                fd.layout[0].in_body
            ),
            ("timestamp", 0, false)
        );
        assert_eq!(
            (fd.layout[1].name.as_str(), fd.layout[1].offset),
            ("mode", 8)
        );
        assert_eq!((fd.layout[2].name.as_str(), fd.layout[2].offset), ("x", 9));
        assert_eq!((fd.layout[3].name.as_str(), fd.layout[3].offset), ("y", 13));
    }

    #[test]
    fn latlon_int32_rule() {
        let mut fd = FormatDef::default();
        let spec = parse_type_spec("double latitude_deg;double longitude_deg;double pressure");
        assemble_layout(&mut fd, &spec);
        for (i, n) in ["latitude_deg", "longitude_deg"].iter().enumerate() {
            let f = &fd.layout[i];
            assert_eq!(f.name, *n);
            assert_eq!(
                (f.gl_type, f.scale, f.size),
                (FieldGlType::Int32, 1e-7, 8),
                "{}",
                n
            );
        }
        assert_eq!(fd.layout[2].gl_type, FieldGlType::Float32);
    }

    #[test]
    fn arrays_expand_type_side_only() {
        let mut fd = FormatDef::default();
        // Go expands arrays declared on the TYPE side ("float[3] voltages");
        // a name-side suffix stays one field of the scalar size.
        let spec =
            parse_type_spec("uint64_t timestamp;float[3] voltages;uint16_t cells[3];char[8] pad");
        assemble_layout(&mut fd, &spec);
        let names: Vec<&str> = fd.field_names.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "timestamp",
                "voltages[0]",
                "voltages[1]",
                "voltages[2]",
                "cells[3]",
                "pad"
            ]
        );
        // float[3] expands at 8/12/16; "cells[3]" keeps scalar size 2 at 20.
        assert_eq!(fd.layout[1].offset, 8);
        assert_eq!(fd.layout[2].offset, 12);
        assert_eq!(fd.layout[3].offset, 16);
        assert_eq!(fd.layout[4].offset, 20);
        assert_eq!(fd.layout[4].size, 2);
        // char[8] pad: 8 bytes, not expanded, off the body.
        assert_eq!(fd.layout[5].offset, 22);
        assert_eq!(fd.layout[5].size, 8);
        assert!(!fd.layout[5].in_body);
    }
}
