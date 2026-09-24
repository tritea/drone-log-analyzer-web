//! Streaming binary scanner, ported from
//! `app/modules/parser/dataflash/binary_scan.go` (whole-file ReadAll becomes
//! an incremental cursor over the carry buffer; incomplete records wait for
//! more bytes and the truncated tail is dropped at finish, matching Go's
//! consume-to-end behavior).

use crate::error::Result;
use crate::logfile::LogFile;

use super::dispatch::{dispatch_record, is_dispatch_message};
use super::fielddecode::{read_all_fields, read_instance_id, read_time_millis};
use super::timebase::{gps_bytes_unix_millis, is_plausible_utc_sec};
use super::{FMT_MSG_ID, MAGIC_HI, MAGIC_LO};

/// Incremental DataFlash binary record scanner.
pub struct BinaryScanner {
    buf: Vec<u8>,
    pub lineno: usize,
    done: bool,
}

/// One scan step's outcome.
enum Step {
    /// Consumed a record; cursor advanced.
    Consumed(usize),
    /// Cursor should only step a single byte (unknown message id).
    Skip,
    /// Need more bytes for a complete record at the cursor.
    Wait,
}

impl BinaryScanner {
    pub fn new(utc_secs: u32, lf: &mut LogFile) -> BinaryScanner {
        if is_plausible_utc_sec(utc_secs) {
            lf.set_time_base(utc_secs as f64 * 1000.0);
        }
        BinaryScanner {
            buf: Vec::new(),
            lineno: 0,
            done: false,
        }
    }

    pub fn feed(&mut self, lf: &mut LogFile, chunk: &[u8]) -> Result<()> {
        if self.done {
            return Ok(());
        }
        self.buf.extend_from_slice(chunk);
        let mut cursor = 0usize;
        loop {
            if !self.advance_to_magic(cursor) {
                // End sentinel or exhausted buffer: stop consuming.
                if self.at_end_sentinel(cursor) {
                    self.done = true;
                }
                break;
            }
            match self.try_record(lf, cursor) {
                Step::Consumed(n) => cursor = n,
                Step::Skip => cursor += 1,
                Step::Wait => break,
            }
        }
        // Release the consumed prefix.
        if cursor > 0 {
            self.buf.drain(..cursor);
        }
        Ok(())
    }

    pub fn finish(&mut self, lf: &mut LogFile) -> Result<()> {
        // A truncated tail record is dropped (Go consumes to end-of-file).
        lf.summary.total_lines = self.lineno as i64;
        Ok(())
    }

    /// Moves the cursor forward until it lands on a magic prefix. Returns
    /// false when no further records can be read from the buffered window.
    fn advance_to_magic(&self, cursor: usize) -> bool {
        let mut i = cursor;
        while i + 3 <= self.buf.len() {
            if self.buf[i] == MAGIC_HI && self.buf[i + 1] == MAGIC_LO {
                return true;
            }
            i += 1;
        }
        false
    }

    fn at_end_sentinel(&self, cursor: usize) -> bool {
        cursor + 2 < self.buf.len()
            && self.buf[cursor] == 0xFF
            && self.buf[cursor + 1] == 0xFF
            && self.buf[cursor + 2] == 0xFF
    }

    /// Attempts to decode the record at the cursor.
    fn try_record(&mut self, lf: &mut LogFile, cursor: usize) -> Step {
        let msg_id = self.buf[cursor + 2];
        if msg_id == FMT_MSG_ID {
            return self.consume_fmt(lf, cursor);
        }
        let Some(fd) = lf.formats.get(&msg_id).cloned() else {
            return Step::Skip;
        };
        let msg_len = fd.msg_len as usize;
        if msg_len < 3 {
            // Degenerate FMT (would spin in Go); step past the magic.
            return Step::Skip;
        }
        if cursor + msg_len > self.buf.len() {
            return Step::Wait;
        }
        let payload = &self.buf[cursor + 3..cursor + msg_len];
        ingest_binary_record(lf, &fd, payload, self.lineno);
        self.lineno += 1;
        Step::Consumed(cursor + msg_len)
    }

    fn consume_fmt(&mut self, lf: &mut LogFile, cursor: usize) -> Step {
        if cursor + 89 > self.buf.len() {
            return Step::Wait;
        }
        if let Some(fd) = decode_fmt_record(&self.buf[cursor + 3..cursor + 89]) {
            lf.formats.insert(fd.msg_type, fd.clone());
            lf.formats_by_name.insert(fd.name.clone(), fd);
        }
        self.lineno += 1;
        Step::Consumed(cursor + 89)
    }
}

/// Parses an 86-byte FMT record payload into a FormatDef.
fn decode_fmt_record(data: &[u8]) -> Option<crate::format::FormatDef> {
    if data.len() < 86 {
        return None;
    }
    let mut fd = crate::format::FormatDef {
        msg_type: data[0],
        msg_len: data[1],
        name: trim_nul(&data[2..6]),
        format_str: trim_nul(&data[6..22]),
        ..Default::default()
    };
    let labels = trim_nul(&data[22..86]);
    if !labels.is_empty() {
        fd.field_names = labels.split(',').map(|s| s.to_string()).collect();
    }
    super::schema::assemble_layout(&mut fd);
    Some(fd)
}

fn trim_nul(b: &[u8]) -> String {
    String::from_utf8_lossy(b)
        .trim_end_matches('\0')
        .to_string()
}

/// Decodes one binary record into curves/records, optionally syncing the UTC
/// time base from the first GPS sample.
fn ingest_binary_record(
    lf: &mut LogFile,
    fd: &crate::format::FormatDef,
    data: &[u8],
    lineno: usize,
) {
    let msg_name = fd.name.as_str();

    let mut time_ms = read_time_millis(fd, data);
    if !lf.has_time_base() && msg_name == "GPS" {
        if let Some(utc_ms) = gps_bytes_unix_millis(fd, data) {
            lf.set_time_base(utc_ms - time_ms);
        }
    }
    if lf.has_time_base() {
        time_ms += lf.time_base_ms();
    }

    lf.accum_store(msg_name, fd, "", data, time_ms);
    if let Some(inst) = read_instance_id(fd, data) {
        lf.accum_store(
            &crate::typebody::instance_type_name(msg_name, inst),
            fd,
            &inst.to_string(),
            data,
            time_ms,
        );
    }

    if is_dispatch_message(msg_name) {
        let values = read_all_fields(fd, data);
        dispatch_record(lf, msg_name, fd, &values, time_ms, lineno);
    }
}
