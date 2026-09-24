//! TypeBody assembly at finalize, ported from `app/modules/parser/typebody.go`.
//! Unlike Go (which spills bodies to a temp file), packed bodies stay in wasm
//! linear memory — that memory IS the delivery channel for zero-copy views,
//! and it is released wholesale at the next parse generation.

use crate::curvebin::{
    TypeBody, TypeField, TIME_COL_BYTES, TYPE_BIN_HEADER, TYPE_BIN_MAGIC, TYPE_BIN_VERSION,
};
use crate::format::{align_offset, FieldGlType, FormatDef};
use crate::logfile::LogFile;

/// Turns NaN/Inf into 0 so a stray bad value can't poison a curve's min/max or
/// the serialized binary range.
fn scrub_finite(v: f64) -> f64 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

impl LogFile {
    /// Packs every message type into a TypeBody and releases the per-curve
    /// sample vectors. Mirrors Go `freezeAllCurves` minus the disk spool.
    pub(crate) fn freeze_all_curves(&mut self) {
        self.type_bodies.clear();
        // Merge accumulator stats first (Go updates curve stats inline).
        self.merge_accum_stats();
        // Scrub min/max before assembly, exactly like the Go loop order.
        for fields in self.curves.values_mut() {
            for cd in fields.values_mut() {
                cd.min = scrub_finite(cd.min);
                cd.max = scrub_finite(cd.max);
            }
        }

        let msg_names: Vec<String> = self.curves.keys().cloned().collect();
        for msg_name in msg_names {
            let tb = if self.accumulators.contains_key(&msg_name) {
                self.assemble_from_accum(&msg_name)
            } else {
                self.assemble_from_curves(&msg_name)
            };
            // Release per-curve sample vectors (Go: cd.times, cd.samples = nil).
            if let Some(fields) = self.curves.get_mut(&msg_name) {
                for cd in fields.values_mut() {
                    cd.times = Vec::new();
                    cd.samples = Vec::new();
                }
            }
            if let Some(tb) = tb {
                if !tb.bin.is_empty() {
                    self.type_bodies.insert(msg_name, tb);
                }
            }
        }
    }

    /// Builds a TypeBody by packing the per-field sample slices for a message
    /// type into a flat, stride-aligned row table. Used when the type was
    /// parsed from text (no accumulator buffer exists). All columns are stored
    /// as float32, matching the Go text path.
    fn assemble_from_curves(&mut self, msg_name: &str) -> Option<TypeBody> {
        let fd = self.lookup_format(msg_name)?;
        let fields = self.curves.get(msg_name)?;

        // Collect in-body fields that actually carry samples, in layout order.
        let mut spec_names: Vec<String> = Vec::new();
        for fl in &fd.layout {
            if !fl.in_body {
                continue;
            }
            if let Some(cd) = fields.get(&fl.name) {
                if !cd.samples.is_empty() {
                    spec_names.push(fl.name.clone());
                }
            }
        }
        if spec_names.is_empty() {
            return None;
        }
        let sample_lens: Vec<usize> = spec_names.iter().map(|n| fields[n].samples.len()).collect();
        let row_count = sample_lens.iter().copied().max().unwrap_or(0);
        let base = fields[&spec_names[0]].times.first().copied().unwrap_or(0.0);

        // Column layout: time col then one f32 per spec, each 4-aligned.
        let offsets: Vec<usize> = spec_names
            .iter()
            .scan(TIME_COL_BYTES, |off, _| {
                let start = align_offset(*off, 4);
                *off = start + 4;
                Some(start)
            })
            .collect();
        let stride = align_offset(TIME_COL_BYTES + 4 * spec_names.len(), 4);

        let mut bin = vec![0u8; TYPE_BIN_HEADER + stride * row_count];
        bin[0..4].copy_from_slice(&TYPE_BIN_MAGIC.to_le_bytes());
        bin[4..8].copy_from_slice(&TYPE_BIN_VERSION.to_le_bytes());
        bin[8..12].copy_from_slice(&(row_count as u32).to_le_bytes());
        bin[12..16].copy_from_slice(&(spec_names.len() as u32).to_le_bytes());
        bin[16..20].copy_from_slice(&(stride as u32).to_le_bytes());
        bin[20..24].copy_from_slice(&0u32.to_le_bytes());
        bin[24..32].copy_from_slice(&base.to_bits().to_le_bytes());

        let first_times = fields[&spec_names[0]].times.clone();
        for r in 0..row_count {
            let row_base = TYPE_BIN_HEADER + r * stride;
            let dms: i32 = first_times.get(r).map(|t| (t - base) as i32).unwrap_or(0);
            bin[row_base..row_base + TIME_COL_BYTES].copy_from_slice(&dms.to_le_bytes());
            for (i, name) in spec_names.iter().enumerate() {
                let cd = &fields[name];
                if r >= cd.samples.len() {
                    continue;
                }
                let o = row_base + offsets[i];
                bin[o..o + 4].copy_from_slice(&cd.samples[r].to_le_bytes());
            }
        }

        let prefix = msg_name.len() + 1;
        let mut tb = TypeBody {
            row_count,
            stride,
            base_time_ms: base,
            bin,
            fields: Vec::new(),
        };
        for (i, name) in spec_names.iter().enumerate() {
            let cd = &fields[name];
            let mut fname = cd.name.clone();
            if fname.len() > prefix {
                fname = fname[prefix..].to_string();
            }
            tb.fields.push(TypeField {
                name: fname,
                gl_type: FieldGlType::Float32,
                scale: 1.0,
                offset: offsets[i],
                min: cd.min,
                max: cd.max,
                count: cd.samples.len(),
            });
        }
        Some(tb)
    }

    /// Stamps relative timestamps onto an accumulator's packed buffer and
    /// wraps it as a TypeBody. Used for binary-parsed types. The accumulator
    /// is drained (buffer/times released) exactly like the Go caller does.
    fn assemble_from_accum(&mut self, msg_name: &str) -> Option<TypeBody> {
        let mut ac = std::mem::take(self.accumulators.get_mut(msg_name)?);
        if ac.cols.is_empty() || ac.times.is_empty() {
            return None;
        }
        let row_count = ac.times.len();
        let base = ac.times[0];
        let stride = ac.stride;
        for r in 0..row_count {
            let row_base = r * stride;
            if row_base + TIME_COL_BYTES <= ac.buf.len() {
                let dms = (ac.times[r] - base) as i32;
                ac.buf[row_base..row_base + TIME_COL_BYTES].copy_from_slice(&dms.to_le_bytes());
            }
        }
        let mut bin = Vec::with_capacity(TYPE_BIN_HEADER + ac.buf.len());
        bin.extend_from_slice(&TYPE_BIN_MAGIC.to_le_bytes());
        bin.extend_from_slice(&TYPE_BIN_VERSION.to_le_bytes());
        bin.extend_from_slice(&(row_count as u32).to_le_bytes());
        bin.extend_from_slice(&(ac.cols.len() as u32).to_le_bytes());
        bin.extend_from_slice(&(stride as u32).to_le_bytes());
        bin.extend_from_slice(&0u32.to_le_bytes());
        bin.extend_from_slice(&base.to_bits().to_le_bytes());
        bin.extend_from_slice(&ac.buf);
        ac.buf = Vec::new();
        ac.times = Vec::new();

        let prefix = msg_name.len() + 1;
        let curves = self.curves.get(msg_name)?;
        let mut tb = TypeBody {
            row_count,
            stride,
            base_time_ms: base,
            bin,
            fields: Vec::new(),
        };
        for f in &ac.cols {
            let mut name = format!("{}.{}", msg_name, f.field);
            if name.len() > prefix {
                name = name[prefix..].to_string();
            }
            let (min, max) = curves
                .get(&f.field)
                .map(|cd| (cd.min, cd.max))
                .unwrap_or((0.0, 0.0));
            tb.fields.push(TypeField {
                name,
                gl_type: f.gl_type,
                scale: f.scale,
                offset: f.dst_off,
                min,
                max,
                count: row_count,
            });
        }
        Some(tb)
    }

    /// The FormatDef for a message, trying the base name when the
    /// instance-suffixed name is absent.
    pub(crate) fn lookup_format(&self, msg_name: &str) -> Option<FormatDef> {
        if let Some(fd) = self.formats_by_name.get(msg_name) {
            return Some(fd.clone());
        }
        self.formats_by_name.get(base_type_name(msg_name)).cloned()
    }
}

/// Strips a trailing numeric instance suffix from a message name (e.g.
/// "BARO2" → "BARO") so callers can fall back to the base format.
pub fn base_type_name(msg_name: &str) -> &str {
    if msg_name.is_empty() {
        return msg_name;
    }
    let trimmed = msg_name.trim_end_matches(|c: char| c.is_ascii_digit());
    if trimmed.is_empty() || trimmed.len() == msg_name.len() {
        msg_name
    } else {
        trimmed
    }
}

/// Formats `MSG` + instance number as the per-instance type name
/// (Go `InstanceTypeName`: msgName + (instance+1)).
pub fn instance_type_name(msg_name: &str, instance: usize) -> String {
    format!("{}{}", msg_name, instance + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_names() {
        assert_eq!(base_type_name("BARO2"), "BARO");
        assert_eq!(base_type_name("GPS"), "GPS");
        assert_eq!(base_type_name("GPS12"), "GPS");
        assert_eq!(base_type_name(""), "");
        assert_eq!(base_type_name("123"), "123");
    }

    #[test]
    fn instance_names() {
        assert_eq!(instance_type_name("GPS", 0), "GPS1");
        assert_eq!(instance_type_name("GPS", 1), "GPS2");
    }
}
