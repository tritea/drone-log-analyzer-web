//! MAVLink tlog backend (.tlog), ported from
//! `app/modules/parser/tlog/{loader,session}.go`. The gomavlib reader becomes
//! incremental entry framing (8-byte micros + v1/v2 frame), CRC verification
//! via the dialect's CRC_EXTRA, and typed decoding through the rust-mavlink
//! ardupilotmega dialect. Any framing damage stops the parse silently, which
//! is exactly what the Go error handling does after the first frame.

pub mod dialect;
pub mod handlers;
pub mod scale;
pub mod vehicles;

use std::collections::{HashMap, HashSet};

use std::io::Cursor;

use mavlink::dialects::ardupilotmega::MavMessage;
use mavlink::peek_reader::PeekReader;
use mavlink::read_any_msg;

use crate::error::Result;
use crate::logfile::LogFile;
use crate::StreamingBackend;

const MAV_V1_START: u8 = 0xFE;
const MAV_V2_START: u8 = 0xFD;
/// tlog entries start with an 8-byte big-endian microsecond timestamp.
const TLOG_TIME_BYTES: usize = 8;

pub struct TlogBackend {
    log: LogFile,
    buf: Vec<u8>,
    fed: u64,
    line_no: usize,
    done: bool,

    vehicle_known: bool,
    vehicle_name: String,
    prev_mode: HashMap<u8, u32>,
    seen_seqs: HashSet<u16>,

    first_time_ms: f64,
    last_time_ms: f64,
    start_epoch: i64,
    got_epoch: bool,
    got_frames: bool,
}

impl TlogBackend {
    pub fn new(filename: &str) -> TlogBackend {
        TlogBackend {
            log: LogFile::new(filename, "tlog"),
            buf: Vec::new(),
            fed: 0,
            line_no: 0,
            done: false,
            vehicle_known: false,
            vehicle_name: String::new(),
            prev_mode: HashMap::new(),
            seen_seqs: HashSet::new(),
            first_time_ms: 0.0,
            last_time_ms: 0.0,
            start_epoch: 0,
            got_epoch: false,
            got_frames: false,
        }
    }

    /// Records first/last sample timestamps and captures the UTC epoch of the
    /// first frame. Returns the frame time in milliseconds.
    fn track_time(&mut self, ts_micros: i64) -> f64 {
        let time_ms = (ts_micros / 1000) as f64;
        if !self.got_frames {
            self.first_time_ms = time_ms;
            self.got_frames = true;
        }
        self.last_time_ms = time_ms;
        if !self.got_epoch {
            self.start_epoch = ts_micros.div_euclid(1_000_000);
            self.got_epoch = true;
        }
        time_ms
    }

    /// Packs a generic message payload into its binary body layout and feeds
    /// it to the curve accumulator. The payload is zero-padded to the full
    /// wire length because MAVLink2 frames may omit trailing zero bytes
    /// (Go decodes then re-packs, which restores them).
    fn store_payload(&mut self, msg_id: u32, payload: &[u8], time_ms: f64) {
        let Some(fd) = dialect::format_def(msg_id) else {
            return;
        };
        if fd.layout.is_empty() {
            return;
        }
        let full = dialect::wire_len(msg_id).unwrap_or(payload.len());
        let mut padded;
        let msg_data: &[u8] = if payload.len() < full {
            padded = vec![0u8; full];
            padded[..payload.len()].copy_from_slice(payload);
            &padded
        } else {
            payload
        };
        if !self.log.formats_by_name.contains_key(&fd.name) {
            let name = fd.name.clone();
            self.log.formats_by_name.insert(name, fd.clone());
        }
        self.log.accum_store(&fd.name, &fd, "", msg_data, time_ms);
    }

    /// Folds the captured time-range and UTC bookkeeping into the Summary.
    fn write_summary(&mut self) {
        if self.got_epoch {
            self.log.summary.start_unix_secs = self.start_epoch;
            self.log.summary.has_utc = true;
        }
        if self.got_frames {
            self.log.summary.duration_secs = (self.last_time_ms - self.first_time_ms) / 1000.0;
        }
        self.log.summary.total_lines = self.line_no as i64;
    }
}

impl StreamingBackend for TlogBackend {
    fn feed(&mut self, chunk: &[u8]) -> Result<()> {
        self.fed += chunk.len() as u64;
        if self.done {
            return Ok(());
        }
        self.buf.extend_from_slice(chunk);
        loop {
            if self.buf.len() < TLOG_TIME_BYTES + 3 {
                break; // need timestamp + magic + length + incompat flag
            }
            let mut tb = [0u8; 8];
            tb.copy_from_slice(&self.buf[..8]);
            // gomavlib writes tlog timestamps as BIG-endian microseconds
            // (reader.go: time.Unix(epoch/1e6, (epoch%1e6)*1e3)).
            let ts_micros = i64::from_be_bytes(tb);

            let magic = self.buf[8];
            // (payload offset, payload length, total frame length, msg id) —
            // only what frames the slice for the crate reader, which detects
            // the version itself.
            let (payload_off, payload_len, total, msg_id) = match magic {
                MAV_V2_START => {
                    let payload_len = self.buf[9] as usize;
                    let incompat = self.buf[10];
                    let sig = if incompat & 0x01 != 0 { 13 } else { 0 };
                    let total = 10 + payload_len + sig + 2;
                    if self.buf.len() < TLOG_TIME_BYTES + total {
                        break; // incomplete frame: wait
                    }
                    let msg_id = (self.buf[15] as u32)
                        | ((self.buf[16] as u32) << 8)
                        | ((self.buf[17] as u32) << 16);
                    (10, payload_len, total, msg_id)
                }
                MAV_V1_START => {
                    let payload_len = self.buf[9] as usize;
                    let total = 6 + payload_len + 2;
                    if self.buf.len() < TLOG_TIME_BYTES + total {
                        break;
                    }
                    (6, payload_len, total, self.buf[13] as u32)
                }
                _ => {
                    // Malformed stream position: stop like the Go reader.
                    self.done = true;
                    break;
                }
            };

            let frame_end = TLOG_TIME_BYTES + total;
            // Copy the frame out so later handler calls can borrow self freely.
            let frame: Vec<u8> = self.buf[TLOG_TIME_BYTES..frame_end].to_vec();
            let payload = &frame[payload_off..payload_off + payload_len];

            let time_ms = self.track_time(ts_micros);
            self.line_no += 1;

            // The crate reader validates CRC, handles v1/v2 (+signatures) and
            // decodes into the typed dialect — fed exactly one frame.
            let mut reader = PeekReader::new(Cursor::new(&frame));
            match read_any_msg::<MavMessage, _>(&mut reader) {
                Ok((hdr, msg)) => {
                    let is_handler = matches!(
                        &msg,
                        MavMessage::HEARTBEAT(_)
                            | MavMessage::STATUSTEXT(_)
                            | MavMessage::PARAM_VALUE(_)
                            | MavMessage::MISSION_ITEM(_)
                            | MavMessage::MISSION_ITEM_INT(_)
                            | MavMessage::COMMAND_INT(_)
                            | MavMessage::COMMAND_LONG(_)
                            | MavMessage::COMMAND_ACK(_)
                    );
                    handlers::route_message(self, &msg, hdr.system_id, hdr.component_id, time_ms);
                    if !is_handler {
                        self.store_payload(msg_id, payload, time_ms);
                    }
                }
                // Unknown id: gomavlib yields a raw frame the Go session
                // ignores (time tracking above already happened).
                Err(_) if dialect::format_def(msg_id).is_none() => {}
                // CRC or decode damage on a known message: stop like the Go
                // reader error handling.
                Err(_) => {
                    self.done = true;
                }
            }

            self.buf.drain(..frame_end);
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        self.write_summary();
        self.log.set_file_size_kb(self.fed as f64 / 1024.0);
        self.log.finalize();
        Ok(())
    }

    fn log(&mut self) -> &mut LogFile {
        &mut self.log
    }
}
