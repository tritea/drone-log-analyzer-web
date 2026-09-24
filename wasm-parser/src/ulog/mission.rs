//! Mission/command topic handlers, ported from
//! `app/modules/parser/ulog/mission.go`.

use crate::format::FormatDef;
use crate::types::{MavlinkCommand, MissionCommand};

use super::fields::{read_scalar_float, read_scalar_uint};
use super::records::DecodeSession;

/// An unacknowledged MAVLink command awaiting a result.
pub struct PendingAck {
    pub idx: usize,
    pub command: u32,
    pub target_sys: u8,
}

/// Value of the first names field present in fd, else 0.
pub fn first_uint_of(fd: &FormatDef, data: &[u8], names: &[&str]) -> u64 {
    for n in names {
        if let Some(v) = read_scalar_uint(fd, data, n) {
            return v;
        }
    }
    0
}

/// Value of the first names field present in fd, else 0.
pub fn first_float_of(fd: &FormatDef, data: &[u8], names: &[&str]) -> f64 {
    for n in names {
        if let Some(v) = read_scalar_float(fd, data, n) {
            return v;
        }
    }
    0.0
}

/// Records a navigator mission item, skipping a repeat of the most recent
/// sequence number (but keeping items that jump backwards, which indicate a
/// mission retransmission).
pub fn on_mission_item(s: &mut DecodeSession, fd: &FormatDef, data: &[u8], time_ms: f64) {
    let seq = first_uint_of(fd, data, &["sequence_current", "seq", "sequence"]) as i64;
    if s.seen_mission && seq == s.prev_mission_seq {
        return;
    }
    s.prev_mission_seq = seq;
    s.seen_mission = true;
    s.file.append_command(MissionCommand {
        time_ms,
        sequence: seq,
        command: first_uint_of(fd, data, &["nav_cmd", "command", "cmd"]) as i64,
        latitude: first_float_of(fd, data, &["latitude", "lat"]),
        longitude: first_float_of(fd, data, &["longitude", "lon", "lng"]),
        altitude: first_float_of(fd, data, &["altitude", "alt"]),
        frame: first_uint_of(fd, data, &["frame", "frm"]) as i64,
        ..Default::default()
    });
}

/// Records a MAVLink command and queues it for ack matching.
pub fn on_vehicle_command(s: &mut DecodeSession, fd: &FormatDef, data: &[u8], time_ms: f64) {
    let cmd = MavlinkCommand {
        time_ms,
        command: first_uint_of(fd, data, &["command", "cmd"]) as i64,
        param1: first_float_of(fd, data, &["param1", "p1"]),
        param2: first_float_of(fd, data, &["param2", "p2"]),
        param3: first_float_of(fd, data, &["param3", "p3"]),
        param4: first_float_of(fd, data, &["param4", "p4"]),
        latitude: first_float_of(fd, data, &["param5", "x", "lat"]),
        longitude: first_float_of(fd, data, &["param6", "y", "lon", "lng"]),
        altitude: first_float_of(fd, data, &["param7", "z", "alt"]),
        target_system: first_uint_of(fd, data, &["target_system"]) as i64,
        target_component: first_uint_of(fd, data, &["target_component"]) as i64,
        source_system: first_uint_of(fd, data, &["source_system"]) as i64,
        source_component: first_uint_of(fd, data, &["source_component"]) as i64,
        ..Default::default()
    };
    let idx = s.file.append_mavlink_command(cmd.clone());
    s.pending_acks.push(PendingAck {
        idx,
        command: cmd.command as u32,
        target_sys: cmd.target_system as u8,
    });
}

/// Matches an ack to the most recent pending command with the same command id
/// and target system, applies the result, and drops the match.
pub fn on_command_ack(s: &mut DecodeSession, fd: &FormatDef, data: &[u8], _time_ms: f64) {
    let cmd_id = first_uint_of(fd, data, &["command", "cmd"]) as u32;
    let result = first_uint_of(fd, data, &["result", "res"]) as i64;
    let target_sys = first_uint_of(fd, data, &["target_system"]) as u8;
    for i in (0..s.pending_acks.len()).rev() {
        let p = &s.pending_acks[i];
        if p.command == cmd_id && p.target_sys == target_sys {
            let idx = p.idx;
            s.pending_acks.remove(i);
            s.file.set_mavlink_command_result(idx, result);
            return;
        }
    }
}
