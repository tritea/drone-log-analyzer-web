//! LogFile aggregate, ported from `app/modules/parser/{logfile,api,series}.go`.
//! Owns the whole parse result; format backends populate it through the
//! store/append methods, then `finalize` packs everything into TypeBodies.

use std::collections::{BTreeMap, HashMap};

use crate::accumulate::RowAccum;
use crate::curvebin::{
    read_field_gl, CurveData, TypeBody, TypeSchemaEntry, TypeSchemaField, CURVE_BIN_HEADER,
    CURVE_BIN_MAGIC, CURVE_BIN_VERSION, TYPE_BIN_HEADER,
};
use crate::format::FormatDef;
use crate::types::{LogError, LogEvent, LogSummary, MavlinkCommand, MissionCommand, ModeChange};

/// The in-memory accumulation of one parsed flight log.
#[derive(Default)]
pub struct LogFile {
    pub summary: LogSummary,
    /// msgType → format definition.
    pub formats: HashMap<u8, FormatDef>,
    /// format name → definition (ulog formats are name-keyed only).
    pub formats_by_name: HashMap<String, FormatDef>,
    pub parameters: HashMap<String, f64>,
    /// lineno → text / timestamp (BTreeMap: deterministic, matches Go's
    /// sorted-map JSON serialization).
    pub messages: BTreeMap<usize, String>,
    pub message_times: BTreeMap<usize, f64>,
    pub mode_changes: BTreeMap<usize, ModeChange>,
    pub commands: Vec<MissionCommand>,
    pub mavlink_commands: Vec<MavlinkCommand>,
    pub errors: Vec<LogError>,
    pub events: Vec<LogEvent>,
    pub px4_event_names: HashMap<i64, String>,
    /// message name → field name → curve meta (samples empty after finalize).
    pub curves: BTreeMap<String, BTreeMap<String, CurveData>>,
    /// message name → packed body (filled at finalize).
    pub type_bodies: BTreeMap<String, TypeBody>,
    pub(crate) accumulators: HashMap<String, RowAccum>,
    pub seen_types: Vec<String>,

    epoch_ms: f64,
    epoch_locked: bool,
}

impl LogFile {
    /// Constructs an empty LogFile ready for a format backend to populate.
    pub fn new(filename: &str, format: &str) -> LogFile {
        LogFile {
            summary: LogSummary {
                filename: filename.to_string(),
                format: format.to_string(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    // ---- api.go surface ----------------------------------------------------

    pub fn set_file_size_kb(&mut self, kb: f64) {
        self.summary.file_size_kb = kb;
    }

    /// Locks the UTC epoch for the log (first caller wins).
    pub fn set_time_base(&mut self, base_ms: f64) {
        self.lock_epoch(base_ms);
    }

    pub fn has_time_base(&self) -> bool {
        self.epoch_locked
    }

    pub fn time_base_ms(&self) -> f64 {
        self.epoch_ms
    }

    pub fn add_parameter(&mut self, name: &str, v: f64) {
        self.parameters.insert(name.to_string(), v);
    }

    pub fn add_message(&mut self, lineno: usize, msg: &str, time_ms: f64) {
        self.messages.insert(lineno, msg.to_string());
        self.message_times.insert(lineno, time_ms);
    }

    pub fn add_error(&mut self, e: LogError) {
        self.errors.push(e);
    }

    pub fn add_event(&mut self, e: LogEvent) {
        self.events.push(e);
    }

    pub fn add_mode_change(&mut self, lineno: usize, m: ModeChange) {
        self.mode_changes.insert(lineno, m);
    }

    pub fn append_command(&mut self, c: MissionCommand) {
        self.commands.push(c);
    }

    /// Appends a MAVLink command and returns its index so the caller can
    /// later attach its acknowledgement result.
    pub fn append_mavlink_command(&mut self, c: MavlinkCommand) -> usize {
        self.mavlink_commands.push(c);
        self.mavlink_commands.len() - 1
    }

    pub fn set_mavlink_command_result(&mut self, idx: usize, result: i64) {
        if let Some(c) = self.mavlink_commands.get_mut(idx) {
            c.result = result;
        }
    }

    /// Packs all curves into type bodies and gathers seen types. Must be
    /// called once after the format backend finishes streaming records.
    pub fn finalize(&mut self) {
        self.gather_seen_types();
        self.freeze_all_curves();
    }

    // ---- logfile.go internals ----------------------------------------------

    /// Records the UTC epoch (ms) once and rebases every timestamp seen so
    /// far onto it. Subsequent calls are no-ops.
    fn lock_epoch(&mut self, base_ms: f64) {
        if base_ms <= 0.0 || self.epoch_locked {
            return;
        }
        self.epoch_ms = base_ms;
        self.epoch_locked = true;
        self.summary.start_unix_secs = (base_ms / 1000.0) as i64;
        self.summary.has_utc = true;
        self.rebase_times(base_ms);
    }

    /// Shifts all recorded timestamps by delta_ms (used when the UTC epoch is
    /// discovered after some data has already been accumulated).
    fn rebase_times(&mut self, delta_ms: f64) {
        for fields in self.curves.values_mut() {
            for cd in fields.values_mut() {
                for t in cd.times.iter_mut() {
                    *t += delta_ms;
                }
            }
        }
        for ac in self.accumulators.values_mut() {
            for t in ac.times.iter_mut() {
                *t += delta_ms;
            }
        }
        for t in self.message_times.values_mut() {
            *t += delta_ms;
        }
        for mode in self.mode_changes.values_mut() {
            mode.time_ms += delta_ms;
        }
        for c in self.commands.iter_mut() {
            c.time_ms += delta_ms;
        }
        for c in self.mavlink_commands.iter_mut() {
            c.time_ms += delta_ms;
        }
    }

    /// Estimates flight duration by scanning candidate topics for a usable
    /// time span, preferring accumulator-backed ones.
    pub fn derive_duration(&mut self, topics: &[&str]) {
        for name in topics {
            if let Some(ac) = self.accumulators.get(*name) {
                if !ac.times.is_empty() {
                    self.summary.duration_secs =
                        (ac.times[ac.times.len() - 1] - ac.times[0]) / 1000.0;
                    return;
                }
            }
        }
        for name in topics {
            let Some(gps_fields) = self.curves.get(*name) else {
                continue;
            };
            let mut time_field: Option<&CurveData> = None;
            for tn in ["TimeMS", "TimeUS", "Time"] {
                if let Some(cd) = gps_fields.get(tn) {
                    time_field = Some(cd);
                    break;
                }
            }
            let Some(tf) = time_field else { continue };
            if tf.samples.is_empty() {
                continue;
            }
            let mut first = tf.samples[0] as f64;
            let mut last = tf.samples[tf.samples.len() - 1] as f64;
            if gps_fields.contains_key("TimeUS") {
                first /= 1000.0;
                last /= 1000.0;
            }
            self.summary.duration_secs = (last - first) / 1000.0;
            return;
        }
    }

    /// Builds the de-duplicated list of message type names that actually
    /// produced curves.
    fn gather_seen_types(&mut self) {
        self.seen_types = self.curves.keys().cloned().collect::<Vec<_>>();
    }

    // ---- read paths (post-finalize) ----------------------------------------

    pub fn get_field_names(&self, msg_name: &str) -> Vec<String> {
        self.lookup_format(msg_name)
            .map(|fd| fd.field_names)
            .unwrap_or_default()
    }

    pub fn get_field_type(&self, msg_name: &str, field_idx: usize) -> u8 {
        match self.lookup_format(msg_name) {
            Some(fd) if field_idx < fd.format_str.len() => fd.format_str.as_bytes()[field_idx],
            _ => 0,
        }
    }

    pub fn type_body_bytes(&self, msg_name: &str) -> Option<&[u8]> {
        self.type_bodies.get(msg_name).map(|tb| tb.bin.as_slice())
    }

    /// Serializes a curve into the NUCB wire blob the frontend chart consumes.
    pub fn curve_bytes(&self, msg_name: &str, field_name: &str) -> Option<Vec<u8>> {
        let tb = self.type_bodies.get(msg_name)?;
        let col = tb.field_index(field_name)?;
        let tf = &tb.fields[col];
        let body = &tb.bin;
        let n = tf.count;
        let mut out = vec![0u8; CURVE_BIN_HEADER + n * 8];
        out[0..4].copy_from_slice(&CURVE_BIN_MAGIC.to_le_bytes());
        out[4..8].copy_from_slice(&CURVE_BIN_VERSION.to_le_bytes());
        out[8..12].copy_from_slice(&(n as u32).to_le_bytes());
        out[12..20].copy_from_slice(&tb.base_time_ms.to_bits().to_le_bytes());
        out[20..24].copy_from_slice(&(tf.min as f32).to_bits().to_le_bytes());
        out[24..28].copy_from_slice(&(tf.max as f32).to_bits().to_le_bytes());
        for r in 0..n {
            let row_base = TYPE_BIN_HEADER + r * tb.stride;
            let dms = i32::from_le_bytes(body[row_base..row_base + 4].try_into().ok()?);
            let val = read_field_gl(body, row_base + tf.offset, tf.gl_type) * tf.scale;
            let o = CURVE_BIN_HEADER + r * 8;
            out[o..o + 4].copy_from_slice(&(dms as f32).to_bits().to_le_bytes());
            out[o + 4..o + 8].copy_from_slice(&(val as f32).to_bits().to_le_bytes());
        }
        Some(out)
    }

    /// Decoded sample values for a curve straight from the packed body.
    pub fn curve_values(&self, msg_name: &str, field_name: &str) -> Option<Vec<f64>> {
        let tb = self.type_bodies.get(msg_name)?;
        let col = tb.field_index(field_name)?;
        let tf = &tb.fields[col];
        let body = &tb.bin;
        let mut out = Vec::with_capacity(tf.count);
        for r in 0..tf.count {
            let row_base = TYPE_BIN_HEADER + r * tb.stride;
            out.push(read_field_gl(body, row_base + tf.offset, tf.gl_type) * tf.scale);
        }
        Some(out)
    }

    /// Decodes a curve into parallel (timeMs, value) slices, where timeMs is
    /// the absolute per-row timestamp (BaseTimeMs + relative ms column).
    pub fn curve_series(&self, msg_name: &str, field_name: &str) -> Option<(Vec<f64>, Vec<f64>)> {
        let tb = self.type_bodies.get(msg_name)?;
        let col = tb.field_index(field_name)?;
        let tf = &tb.fields[col];
        let body = &tb.bin;
        let mut times = Vec::with_capacity(tf.count);
        let mut values = Vec::with_capacity(tf.count);
        for r in 0..tf.count {
            let row_base = TYPE_BIN_HEADER + r * tb.stride;
            let dms = i32::from_le_bytes(body[row_base..row_base + 4].try_into().ok()?);
            times.push(tb.base_time_ms + dms as f64);
            values.push(read_field_gl(body, row_base + tf.offset, tf.gl_type) * tf.scale);
        }
        Some((times, values))
    }

    /// The smallest first-sample timestamp across all packed type bodies
    /// (or 0 when none). Callers rebase per-group absolute timestamps onto
    /// this log-wide origin.
    pub fn earliest_body_time_ms(&self) -> f64 {
        let mut earliest = 0.0;
        let mut first = true;
        for tb in self.type_bodies.values() {
            if tb.row_count == 0 {
                continue;
            }
            if first || tb.base_time_ms < earliest {
                earliest = tb.base_time_ms;
                first = false;
            }
        }
        earliest
    }

    /// Projects every TypeBody into its JSON-friendly schema form.
    pub fn type_schema(&self) -> BTreeMap<String, TypeSchemaEntry> {
        let mut out = BTreeMap::new();
        for (msg_name, tb) in &self.type_bodies {
            out.insert(
                msg_name.clone(),
                TypeSchemaEntry {
                    fields: tb
                        .fields
                        .iter()
                        .map(|f| TypeSchemaField {
                            name: f.name.clone(),
                            gl_type: f.gl_type as u8,
                            scale: f.scale,
                            offset: f.offset,
                            min: f.min,
                            max: f.max,
                            count: f.count,
                        })
                        .collect(),
                    row_count: tb.row_count,
                    stride: tb.stride,
                    base_time_ms: tb.base_time_ms,
                },
            );
        }
        out
    }
}
