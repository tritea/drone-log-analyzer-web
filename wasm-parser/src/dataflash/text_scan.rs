//! Streaming text scanner, ported from
//! `app/modules/parser/dataflash/text_scan.go` (line-splitting becomes
//! incremental over the carry buffer).

use crate::error::Result;
use crate::format::FormatDef;
use crate::logfile::LogFile;
use crate::textrows::TextValue;

use super::dispatch::{dispatch_record, is_dispatch_message};
use super::fielddecode::{
    is_instance_field, is_timestamp_field, normalize_time_micros, FieldValue,
};
use super::modetable::detect_firmware;
use super::timebase::gps_field_unix_millis;

/// Incremental ASCII DataFlash line scanner.
pub struct TextScanner {
    buf: Vec<u8>,
    pub lineno: usize,
    decided: bool,
}

impl Default for TextScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl TextScanner {
    pub fn new() -> TextScanner {
        // Implicit FMT self-description, exactly like Go's scanText.
        TextScanner {
            buf: Vec::new(),
            lineno: 0,
            decided: false,
        }
    }

    pub fn begin(&mut self, lf: &mut LogFile) {
        if self.decided {
            return;
        }
        self.decided = true;
        let fd = FormatDef {
            msg_type: super::FMT_MSG_ID,
            name: "FMT".to_string(),
            format_str: "BBnNZ".to_string(),
            field_names: vec![
                "Type".into(),
                "Length".into(),
                "Name".into(),
                "Format".into(),
                "Columns".into(),
            ],
            ..Default::default()
        };
        lf.formats.insert(super::FMT_MSG_ID, fd.clone());
        lf.formats_by_name.insert("FMT".into(), fd);
    }

    pub fn feed(&mut self, lf: &mut LogFile, chunk: &[u8]) -> Result<()> {
        self.buf.extend_from_slice(chunk);
        while let Some(idx) = self.buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=idx).collect();
            self.lineno += 1;
            ingest_text_line(lf, &line[..idx], self.lineno);
        }
        Ok(())
    }

    pub fn finish(&mut self, lf: &mut LogFile) -> Result<()> {
        if !self.buf.is_empty() {
            self.lineno += 1;
            let line = std::mem::take(&mut self.buf);
            ingest_text_line(lf, &line, self.lineno);
        }
        lf.summary.total_lines = self.lineno as i64;
        Ok(())
    }
}

/// Parses and dispatches a single text log line (trailing newline stripped).
pub fn ingest_text_line(lf: &mut LogFile, raw: &[u8], lineno: usize) {
    let line = String::from_utf8_lossy(raw);
    let line = line.trim_end_matches(['\r', '\n']);
    if line.is_empty() || is_skippable_banner(line) {
        return;
    }

    // Go splits on the literal ", " separator.
    let tokens: Vec<&str> = line.split(", ").collect();
    if tokens.len() == 1 {
        handle_untyped_line(lf, line);
        return;
    }

    let msg_name = tokens[0];
    if msg_name == "FMT" {
        register_text_format(lf, &tokens);
        return;
    }
    let Some(fd) = lf.formats_by_name.get(msg_name).cloned() else {
        return;
    };

    let strs: Vec<String> = tokens[1..].iter().map(|t| t.trim().to_string()).collect();
    let values: Vec<TextValue> = strs.iter().cloned().map(TextValue::Str).collect();
    let field_values: Vec<FieldValue> = strs.into_iter().map(FieldValue::Str).collect();

    let mut time_ms = text_line_millis(&fd, &values);
    sync_time_base_from_gps(lf, msg_name, &fd, &field_values, time_ms);
    if lf.has_time_base() {
        time_ms += lf.time_base_ms();
    }

    lf.store_text_curve_values(msg_name, &fd, &values, time_ms, lineno, "");
    if let Some(inst) = text_line_instance(&fd, &values) {
        let inst_name = crate::typebody::instance_type_name(msg_name, inst);
        let inst_str = inst.to_string();
        lf.store_text_curve_values(&inst_name, &fd, &values, time_ms, lineno, &inst_str);
    }
    if is_dispatch_message(msg_name) {
        dispatch_record(lf, msg_name, &fd, &field_values, time_ms, lineno);
    }
}

/// Boilerplate lines that carry no data.
fn is_skippable_banner(line: &str) -> bool {
    line == " Ready to drive."
        || line == " Ready to FLY."
        || line == "----------------------------------------"
}

/// Lines without comma separators: free-RAM, hardware banner, or firmware
/// banner.
fn handle_untyped_line(lf: &mut LogFile, line: &str) {
    let parts: Vec<&str> = line.split(' ').collect();
    if parts.len() == 3 && parts[0] == "Free" && parts[1] == "RAM:" {
        if let Ok(ram) = parts[2].parse::<i64>() {
            lf.summary.free_ram = ram;
        }
        return;
    }
    if parts[0] == "PX4" || parts[0] == "APM" || parts[0] == "MPNG" {
        lf.summary.hardware_type = line.to_string();
        return;
    }
    detect_firmware(lf, line);
}

/// Reads the timestamp field of a text log line as milliseconds.
fn text_line_millis(fd: &FormatDef, values: &[TextValue]) -> f64 {
    for (i, fn_) in fd.field_names.iter().enumerate() {
        let Some(v) = values.get(i) else { break };
        if is_timestamp_field(fn_) {
            return normalize_time_micros(fn_, v.to_f64());
        }
    }
    0.0
}

/// Extracts the instance id of a text log line.
fn text_line_instance(fd: &FormatDef, values: &[TextValue]) -> Option<usize> {
    for (i, fn_) in fd.field_names.iter().enumerate() {
        let Some(v) = values.get(i) else { break };
        if is_instance_field(fn_) {
            return super::fielddecode::to_instance_id(v.to_f64());
        }
    }
    None
}

/// Establishes the UTC time base from the first GPS record seen on a text
/// stream, when no base has been set yet.
fn sync_time_base_from_gps(
    lf: &mut LogFile,
    msg_name: &str,
    fd: &FormatDef,
    values: &[FieldValue],
    time_ms: f64,
) {
    if lf.has_time_base() || msg_name != "GPS" {
        return;
    }
    let Some(utc_ms) = gps_field_unix_millis(fd, values) else {
        return;
    };
    if utc_ms <= 0.0 {
        return;
    }
    lf.set_time_base(utc_ms - time_ms);
}

/// Decodes an "FMT, ..." line into a FormatDef and indexes it by both type
/// id and name.
fn register_text_format(lf: &mut LogFile, tokens: &[&str]) {
    if tokens.len() < 6 {
        return;
    }
    let type_id: u8 = tokens[1].trim().parse().unwrap_or(0);
    let msg_len: u8 = tokens[2].trim().parse().unwrap_or(0);
    let name = tokens[3].trim().to_string();
    let format_str = tokens[4].trim().to_string();

    let mut field_names: Vec<String> = Vec::new();
    if tokens.len() > 5 {
        // Go re-joins tokens[5:] with "," then splits on "," — net effect:
        // comma-separated labels, each trimmed.
        let raw = tokens[5..].join(",");
        field_names = raw.split(',').map(|s| s.trim().to_string()).collect();
    }

    let mut fd = FormatDef {
        msg_type: type_id,
        msg_len,
        name: name.clone(),
        format_str,
        field_names,
        layout: Vec::new(),
    };
    super::schema::assemble_layout(&mut fd);
    lf.formats.insert(type_id, fd.clone());
    lf.formats_by_name.insert(name, fd);
}
