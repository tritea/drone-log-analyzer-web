//! ULog record handlers, ported from `app/modules/parser/ulog/{session,records}.go`.

use std::collections::HashMap;

use crate::format::FormatDef;
use crate::logfile::LogFile;
use crate::types::{LogEvent, ModeChange};

use super::events::decode_event_metadata;
use super::fields::{cut_cstring, read_scalar_uint, read_topic_time};
use super::mission::{on_command_ack, on_mission_item, on_vehicle_command, PendingAck};
use super::schema::{assemble_layout, parse_type_spec};
use super::timebase::{carries_utc, plausible_utc, utc_millis_from_topic};
use super::values::coerce_scalar;

/// Mutable state accumulated while parsing one ULog: the output LogFile, the
/// subscription table, and bookkeeping for the stream's time bounds, mode
/// transitions, mission items and pending acks.
pub struct DecodeSession {
    pub file: LogFile,
    format_by_id: HashMap<u16, FormatDef>,
    topic_by_id: HashMap<u16, String>,
    inst_by_id: HashMap<u16, u8>,
    first_stamp: u64,
    last_stamp: u64,
    seen_stamp: bool,
    record_no: usize,
    prev_nav: i64,
    seen_nav: bool,
    pub(super) prev_mission_seq: i64,
    pub(super) seen_mission: bool,
    pub(super) pending_acks: Vec<PendingAck>,
    meta_event_buf: Vec<u8>,
}

impl DecodeSession {
    pub fn new(filename: &str) -> DecodeSession {
        DecodeSession {
            file: LogFile::new(filename, "ulog"),
            format_by_id: HashMap::new(),
            topic_by_id: HashMap::new(),
            inst_by_id: HashMap::new(),
            first_stamp: 0,
            last_stamp: 0,
            seen_stamp: false,
            record_no: 0,
            prev_nav: 0,
            seen_nav: false,
            prev_mission_seq: 0,
            seen_mission: false,
            pending_acks: Vec::new(),
            meta_event_buf: Vec::new(),
        }
    }

    /// Routes a single record to its typed handler.
    pub fn dispatch_record(&mut self, rec_type: u8, payload: &[u8]) {
        match rec_type {
            b'F' => self.on_format(payload),
            b'I' => self.on_info(payload),
            b'M' => self.on_info_multi(payload),
            b'P' => self.on_param(payload),
            b'A' => self.on_subscribe(payload),
            b'D' => self.on_data(payload),
            b'L' => self.on_string(payload),
            _ => {}
        }
    }

    /// Folds ts into the stream's first/last timestamp window.
    fn advance_time_bounds(&mut self, ts: u64) {
        if ts == 0 {
            return;
        }
        if !self.seen_stamp || ts < self.first_stamp {
            self.first_stamp = ts;
        }
        if !self.seen_stamp || ts > self.last_stamp {
            self.last_stamp = ts;
        }
        self.seen_stamp = true;
    }

    /// End-of-stream flush: resolve metadata events and the stream duration.
    pub fn finish_stream(&mut self) {
        if !self.meta_event_buf.is_empty() {
            self.file.px4_event_names = decode_event_metadata(&self.meta_event_buf)
                .map(|m| m.into_iter().collect())
                .unwrap_or_default();
        }
        if self.seen_stamp {
            self.file.summary.duration_secs = (self.last_stamp - self.first_stamp) as f64 / 1e6;
        }
    }

    fn on_format(&mut self, payload: &[u8]) {
        let Some(idx) = payload.iter().position(|&b| b == b':') else {
            return;
        };
        if idx == 0 {
            return;
        }
        let name = String::from_utf8_lossy(&payload[..idx]).to_string();
        let spec = String::from_utf8_lossy(&payload[idx + 1..]).to_string();
        let mut fd = FormatDef {
            name: name.clone(),
            ..Default::default()
        };
        assemble_layout(&mut fd, &parse_type_spec(&spec));
        self.file.formats_by_name.insert(name, fd);
    }

    fn on_info(&mut self, payload: &[u8]) {
        let Some((desc, value)) = split_info_desc(payload) else {
            return;
        };
        let (_type_str, key) = split_typed_key(&desc);
        let val = coerce_scalar(value, &_type_str);
        match key.as_str() {
            "ver_hw" | "ver_hw_uuid" | "ver_hw_subtype" => {
                let v = val.as_text();
                if !v.is_empty() {
                    self.file.summary.hardware_type = v;
                }
            }
            "ver_sw" | "ver_vendor_sw" => {
                let v = val.as_text();
                if !v.is_empty() {
                    self.file.summary.firmware_version = format!("PX4 {}", v);
                }
            }
            "sys_name" | "sys_os_name" => {
                let v = val.as_text();
                if !v.is_empty() && self.file.summary.vehicle_type.is_empty() {
                    self.file.summary.vehicle_type = v;
                }
            }
            _ => {}
        }
    }

    fn on_param(&mut self, payload: &[u8]) {
        let Some((desc, value)) = split_info_desc(payload) else {
            return;
        };
        let (type_str, key) = split_typed_key(&desc);
        if let Some(v) = coerce_scalar(value, &type_str).as_float64() {
            self.file.add_parameter(&key, v);
        }
    }

    fn on_info_multi(&mut self, payload: &[u8]) {
        if payload.len() < 2 {
            return;
        }
        let is_continued = payload[0];
        let key_len = payload[1] as usize;
        if 2 + key_len > payload.len() {
            return;
        }
        let key = String::from_utf8_lossy(&payload[2..2 + key_len]).to_string();
        let (_, name) = split_typed_key(&key);
        if name != "metadata_events" {
            return;
        }
        if is_continued == 0 {
            self.meta_event_buf.clear();
        }
        self.meta_event_buf
            .extend_from_slice(&payload[2 + key_len..]);
    }

    fn on_subscribe(&mut self, payload: &[u8]) {
        if payload.len() < 3 {
            return;
        }
        let inst = payload[0];
        let msg_id = u16::from_le_bytes([payload[1], payload[2]]);
        let name = cut_cstring(&payload[3..]);
        if let Some(fd) = self.file.formats_by_name.get(&name).cloned() {
            self.format_by_id.insert(msg_id, fd);
        }
        self.topic_by_id.insert(msg_id, name);
        self.inst_by_id.insert(msg_id, inst);
    }

    fn on_data(&mut self, payload: &[u8]) {
        if payload.len() < 2 {
            return;
        }
        let msg_id = u16::from_le_bytes([payload[0], payload[1]]);
        let Some(fd) = self.format_by_id.get(&msg_id).cloned() else {
            return;
        };
        let topic_data = &payload[2..];
        let ts = read_topic_time(&fd, topic_data);
        self.advance_time_bounds(ts);
        let mut time_ms = ts as f64 / 1000.0;

        let name = self.topic_by_id.get(&msg_id).cloned().unwrap_or_default();
        let store_name = match self.inst_by_id.get(&msg_id) {
            Some(&mid) if mid > 0 => format!("{}{}", name, mid + 1),
            _ => name.clone(),
        };

        if !self.file.has_time_base() && carries_utc(&name) {
            if let Some(utc_ms) = utc_millis_from_topic(&fd, topic_data) {
                if plausible_utc(utc_ms) {
                    self.file.set_time_base(utc_ms - time_ms);
                }
            }
        }
        if self.file.has_time_base() {
            time_ms += self.file.time_base_ms();
        }

        self.file
            .accum_store(&store_name, &fd, "", topic_data, time_ms);
        self.handle_topic_extras(&name, &fd, topic_data, time_ms);
    }

    /// Emits the side effects that depend on the decoded topic.
    fn handle_topic_extras(&mut self, name: &str, fd: &FormatDef, topic_data: &[u8], time_ms: f64) {
        let record_no = self.record_no;
        if name == "vehicle_status" {
            if let Some(v) = read_scalar_uint(fd, topic_data, "nav_state") {
                let nav = v as i64;
                if !self.seen_nav || nav != self.prev_nav {
                    self.file.add_mode_change(
                        record_no,
                        ModeChange {
                            mode: px4_nav_label(nav),
                            mode_num: nav,
                            time_ms,
                        },
                    );
                    self.prev_nav = nav;
                    self.seen_nav = true;
                }
            }
        }
        match name {
            "navigator_mission_item" => on_mission_item(self, fd, topic_data, time_ms),
            "vehicle_command" => on_vehicle_command(self, fd, topic_data, time_ms),
            "vehicle_command_ack" => on_command_ack(self, fd, topic_data, time_ms),
            "event" => {
                if let Some(id) = read_scalar_uint(fd, topic_data, "id") {
                    self.file.add_event(LogEvent {
                        time_ms,
                        id: id as i64,
                        lineno: record_no as i64,
                    });
                }
            }
            _ => {}
        }
    }

    fn on_string(&mut self, payload: &[u8]) {
        if payload.len() < 10 {
            return;
        }
        let mut b = [0u8; 8];
        b.copy_from_slice(&payload[1..9]);
        let ts = u64::from_le_bytes(b);
        let mut msg_ms = ts as f64 / 1000.0;
        if self.file.has_time_base() {
            msg_ms += self.file.time_base_ms();
        }
        let text = String::from_utf8_lossy(&payload[9..]).to_string();
        self.file.add_message(self.record_no, &text, msg_ms);
    }

    /// Increments the per-record counter (Go does this for every well-formed
    /// record before dispatch).
    pub fn next_record_no(&mut self) -> usize {
        self.record_no += 1;
        self.record_no
    }
}

/// Splits an info/param payload into its "type key" descriptor and the raw
/// value bytes that follow it.
fn split_info_desc(payload: &[u8]) -> Option<(String, &[u8])> {
    if payload.len() < 2 {
        return None;
    }
    let desc_len = payload[0] as usize;
    if 1 + desc_len > payload.len() {
        return None;
    }
    let desc = String::from_utf8_lossy(&payload[1..1 + desc_len]).to_string();
    Some((desc, &payload[1 + desc_len..]))
}

/// Splits a "<ctype> <key>" descriptor into its two parts.
fn split_typed_key(desc: &str) -> (String, String) {
    match desc.find(' ') {
        Some(sp) => (desc[..sp].to_string(), desc[sp + 1..].to_string()),
        None => (desc.to_string(), String::new()),
    }
}

/// Maps a PX4 navigator state number to its display name.
fn px4_nav_label(nav: i64) -> String {
    let name = match nav {
        0 => "MANUAL",
        1 => "ALTCTL",
        2 => "POSCTL",
        3 => "AUTO_MISSION",
        4 => "AUTO_LOITER",
        5 => "AUTO_RTL",
        6 => "POSITION",
        7 => "ACRO",
        8 => "OFFBOARD",
        9 => "STABILIZED",
        10 => "RATTITUDE",
        11 => "AUTO_TAKEOFF",
        12 => "AUTO_LAND",
        13 => "AUTO_FOLLOW_TARGET",
        14 => "AUTO_PRECLAND",
        15 => "ORBIT",
        16 => "AUTO_VTOL_TAKEOFF",
        17 => "AUTO_VTOL_LAND",
        _ => return format!("NAV_{}", nav),
    };
    name.to_string()
}
