//! PX4 metadata_events decoding (XZ-compressed JSON), ported from
//! `app/modules/parser/ulog/events.go`.

use std::collections::HashMap;

/// XZ header magic marking compressed payloads.
const XZ_HEADER: [u8; 6] = [0xFD, b'7', b'z', b'X', b'Z', 0x00];

/// Parses the PX4 metadata_events blob (optionally XZ-coded) into a map of
/// event id → human-readable message.
pub fn decode_event_metadata(data: &[u8]) -> Option<HashMap<i64, String>> {
    let json_bytes = if data.len() >= XZ_HEADER.len() && data[..XZ_HEADER.len()] == XZ_HEADER {
        inflate_xz(data).unwrap_or_default()
    } else {
        data.to_vec()
    };
    // Go falls through to json.Unmarshal on the (possibly original) bytes when
    // decompression fails; an empty vec simply fails to parse below.
    let root: serde_json::Value = serde_json::from_slice(&json_bytes).ok()?;
    let mut out: HashMap<i64, String> = HashMap::new();
    collect_events(&root, 0, false, &mut out);
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Decompresses an XZ payload (pure-Rust lzma-rs replaces the Go xz module).
fn inflate_xz(data: &[u8]) -> Result<Vec<u8>, lzma_rs::error::Error> {
    let mut out = Vec::new();
    let mut cursor = std::io::Cursor::new(data);
    lzma_rs::xz_decompress(&mut cursor, &mut out)?;
    Ok(out)
}

/// Walks the JSON tree, collecting {id, message} pairs into out. `num_key` is
/// the integer object key active on the current path (if any).
fn collect_events(
    node: &serde_json::Value,
    num_key: i64,
    has_num_key: bool,
    out: &mut HashMap<i64, String>,
) {
    if let serde_json::Value::Object(map) = node {
        if let Some(serde_json::Value::String(msg)) = map.get("message") {
            if !msg.is_empty() {
                if let Some(id) = resolve_event_id(map, num_key, has_num_key) {
                    out.insert(id, msg.clone());
                }
            }
        }
        for (k, child) in map {
            match k.parse::<i64>() {
                Ok(n) => collect_events(child, n, true, out),
                Err(_) => collect_events(child, num_key, has_num_key, out),
            }
        }
    } else if let serde_json::Value::Array(items) = node {
        for child in items {
            collect_events(child, num_key, has_num_key, out);
        }
    }
}

/// Determines the event id, preferring an explicit "id" field and falling
/// back to the active numeric object key.
fn resolve_event_id(
    obj: &serde_json::Map<String, serde_json::Value>,
    num_key: i64,
    has_num_key: bool,
) -> Option<i64> {
    if let Some(raw) = obj.get("id") {
        if let Some(id) = coerce_json_int(raw) {
            return Some(id);
        }
    }
    if has_num_key {
        return Some(num_key);
    }
    None
}

/// Converts a JSON number-or-string into an i64.
fn coerce_json_int(v: &serde_json::Value) -> Option<i64> {
    match v {
        serde_json::Value::Number(n) => n.as_i64(),
        serde_json::Value::String(s) => s.parse::<i64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_json_keyed_by_id() {
        let blob = br#"{"events":{"42":{"message":"Armed"},"7":{"message":"Disarmed"}}}"#;
        let names = decode_event_metadata(blob).unwrap();
        assert_eq!(names.get(&42).map(|s| s.as_str()), Some("Armed"));
        assert_eq!(names.get(&7).map(|s| s.as_str()), Some("Disarmed"));
    }

    #[test]
    fn xz_compressed_json_round_trip() {
        let json = br#"{"events":{"19":{"message":"Navigation mode changed"}}}"#;
        let mut xz = Vec::new();
        let mut cursor = std::io::Cursor::new(json);
        lzma_rs::xz_compress(&mut cursor, &mut xz).unwrap();
        assert!(xz.starts_with(&XZ_HEADER));

        let names = decode_event_metadata(&xz).unwrap();
        assert_eq!(
            names.get(&19).map(|s| s.as_str()),
            Some("Navigation mode changed")
        );
    }

    #[test]
    fn nested_groups_resolve_ids() {
        let blob = br#"{"components":{"1":{"event_groups":{"g":{"events":{"cmd_arm":{"id":42,"message":"Armed"}}}}}}}"#;
        let names = decode_event_metadata(blob).unwrap();
        assert_eq!(names.get(&42).map(|s| s.as_str()), Some("Armed"));
    }
}
