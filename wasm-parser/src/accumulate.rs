//! Binary row accumulator, ported from `app/modules/parser/accumulate.go`.
//! Packs incoming rows for one message type into a flat, fixed-stride byte
//! buffer so the final TypeBody can be sliced straight out of it.
//!
//! Unlike the Go original (which tracks min/max on shared `*CurveData`
//! pointers), per-column stats live on the accumulator itself and are merged
//! into the curves map at finalize time — same result, no aliasing.

use crate::curvebin::{read_field_gl, TIME_COL_BYTES};
use crate::format::{align_offset, FieldGlType, FormatDef};
use crate::logfile::LogFile;

/// Maps one field of a message format onto its slot within a packed row.
#[derive(Debug, Clone)]
pub struct AccumCol {
    pub field: String,
    pub src_off: usize, // offset within the raw message payload
    pub src_len: usize, // byte width in the raw payload (may differ from stored width)
    pub dst_off: usize, // offset within the packed row
    pub gl_type: FieldGlType,
    pub scale: f64,
    pub min: f64,
    pub max: f64,
}

/// Packs rows for one message type into `buf` with per-row timestamps.
#[derive(Debug, Clone, Default)]
pub struct RowAccum {
    pub cols: Vec<AccumCol>,
    pub stride: usize,
    pub buf: Vec<u8>,
    pub times: Vec<f64>,
}

impl LogFile {
    /// Decodes one binary message instance, appending a packed row to the
    /// message's accumulator (if it has body fields).
    pub fn accum_store(
        &mut self,
        msg_name: &str,
        fd: &FormatDef,
        instance: &str,
        msg_data: &[u8],
        time_ms: f64,
    ) {
        self.init_curves(msg_name, fd, instance);
        if self.ensure_accumulator(msg_name, fd) {
            let ac = self.accumulators.get_mut(msg_name).expect("just ensured");
            append_row(ac, msg_data, time_ms);
        }
    }

    /// Lazily builds the row layout for a message type from its format
    /// definition, skipping types with no in-body fields. Returns true when an
    /// accumulator exists for the type. Like the Go original, curve min/max
    /// are reset to the +/-Inf sentinels here so the accumulator becomes the
    /// stats authority for the type.
    fn ensure_accumulator(&mut self, msg_name: &str, fd: &FormatDef) -> bool {
        if self.accumulators.contains_key(msg_name) {
            return true;
        }
        let Some(fields) = self.curves.get(msg_name) else {
            return false;
        };
        let mut cols: Vec<AccumCol> = Vec::new();
        let mut dst_off = TIME_COL_BYTES;
        for fl in &fd.layout {
            if !fl.in_body || !fields.contains_key(&fl.name) {
                continue;
            }
            dst_off = align_offset(dst_off, fl.gl_type.size());
            cols.push(AccumCol {
                field: fl.name.clone(),
                src_off: fl.offset,
                src_len: fl.size,
                dst_off,
                gl_type: fl.gl_type,
                scale: fl.scale,
                min: f64::INFINITY,
                max: f64::NEG_INFINITY,
            });
            dst_off += fl.gl_type.size();
        }
        if cols.is_empty() {
            return false;
        }
        let stride = align_offset(dst_off, 4);
        if let Some(fields) = self.curves.get_mut(msg_name) {
            for col in &cols {
                if let Some(cd) = fields.get_mut(&col.field) {
                    cd.min = f64::INFINITY;
                    cd.max = f64::NEG_INFINITY;
                }
            }
        }
        self.accumulators.insert(
            msg_name.to_string(),
            RowAccum {
                cols,
                stride,
                buf: Vec::new(),
                times: Vec::new(),
            },
        );
        true
    }

    /// Merges accumulator column stats into the curves map (called during
    /// finalize, before scrubbing/assembly).
    pub(crate) fn merge_accum_stats(&mut self) {
        type FieldStats = Vec<(String, f64, f64)>;
        let merged: Vec<(String, FieldStats)> = self
            .accumulators
            .iter()
            .map(|(name, ac)| {
                (
                    name.clone(),
                    ac.cols
                        .iter()
                        .map(|c| (c.field.clone(), c.min, c.max))
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        for (msg_name, cols) in merged {
            let Some(fields) = self.curves.get_mut(&msg_name) else {
                continue;
            };
            for (field, min, max) in cols {
                if let Some(cd) = fields.get_mut(&field) {
                    if min < cd.min {
                        cd.min = min;
                    }
                    if max > cd.max {
                        cd.max = max;
                    }
                }
            }
        }
    }
}

/// Writes one decoded row into the accumulator buffer and records its
/// timestamp, widening fields that are stored wider on disk than in the format.
fn append_row(ac: &mut RowAccum, msg_data: &[u8], time_ms: f64) {
    let row_base = ac.buf.len();
    ac.buf.resize(row_base + ac.stride, 0);
    for f in 0..ac.cols.len() {
        let (src_off, src_len, dst_off, gl_type, scale) = {
            let c = &ac.cols[f];
            (c.src_off, c.src_len, c.dst_off, c.gl_type, c.scale)
        };
        if src_off + src_len > msg_data.len() {
            continue;
        }
        let store_n = gl_type.size();
        let dst = &mut ac.buf[row_base + dst_off..row_base + dst_off + store_n];
        let fv: f64;
        if src_len == store_n {
            // Widths match: copy bytes verbatim and decode for stats.
            dst.copy_from_slice(&msg_data[src_off..src_off + store_n]);
            fv = read_field_gl(msg_data, src_off, gl_type) * scale;
        } else if gl_type == FieldGlType::Int32 {
            // Wide integer field (e.g. scaled lat/lon): round back into int32.
            let deg_e7 = (read_wide_float(msg_data, src_off, src_len) / scale).round() as i32;
            dst.copy_from_slice(&deg_e7.to_le_bytes());
            fv = deg_e7 as f64 * scale;
        } else {
            // Wide float field: store as float32.
            fv = read_wide_float(msg_data, src_off, src_len) * scale;
            dst.copy_from_slice(&(fv as f32).to_le_bytes());
        }
        let col = &mut ac.cols[f];
        if fv.is_finite() {
            if fv < col.min {
                col.min = fv;
            }
            if fv > col.max {
                col.max = fv;
            }
        }
    }
    ac.times.push(time_ms);
}

/// Decodes an 8-byte little-endian f64 from data; 0 otherwise.
fn read_wide_float(data: &[u8], off: usize, size: usize) -> f64 {
    if size == 8 && off + 8 <= data.len() {
        f64::from_le_bytes(data[off..off + 8].try_into().unwrap())
    } else {
        0.0
    }
}
