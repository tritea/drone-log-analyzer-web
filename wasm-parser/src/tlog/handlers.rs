//! Typed message handlers, ported from
//! `app/modules/parser/tlog/handlers.go`. The typed structs come from the
//! rust-mavlink ardupilotmega dialect (the Go original decodes via gomavlib
//! and matches on struct types).

use mavlink::dialects::ardupilotmega::{self as apm, MavMessage};

use crate::types::{MavlinkCommand, MissionCommand, ModeChange};

use super::vehicles::{autopilot_name, decode_vehicle, flight_mode_label};
use super::TlogBackend;

/// Trims a fixed-size char array at the first NUL and decodes as UTF-8.
fn cstr(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).to_string()
}

/// Records vehicle type and firmware on first contact, and emits a
/// mode-change entry whenever a system's custom mode flips. Mode tracking
/// only runs once the autopilot family is known.
pub fn on_heartbeat(s: &mut TlogBackend, m: &apm::HEARTBEAT_DATA, sys_id: u8, time_ms: f64) {
    let ap = autopilot_name(m.autopilot as u8);
    if ap.is_empty() {
        return;
    }
    if !s.vehicle_known {
        let (vehicle, frame_name, airframe) = decode_vehicle(m.mavtype as u8);
        if !vehicle.is_empty() {
            s.log.summary.vehicle_type = vehicle.to_string();
            s.log.summary.frame = frame_name.to_string();
            s.log.summary.airframe = airframe.to_string();
            s.vehicle_name = vehicle.to_string();
        }
        s.log.summary.firmware_version = ap.to_string();
        s.vehicle_known = true;
    }
    let cur = m.custom_mode;
    if let Some(&prev) = s.prev_mode.get(&sys_id) {
        if prev != cur {
            s.log.add_mode_change(
                s.line_no,
                ModeChange {
                    mode: flight_mode_label(&s.vehicle_name, cur),
                    mode_num: cur as i64,
                    time_ms,
                },
            );
        }
    }
    s.prev_mode.insert(sys_id, cur);
}

/// Appends a non-empty STATUSTEXT payload to the message log.
pub fn on_status_text(s: &mut TlogBackend, m: &apm::STATUSTEXT_DATA, time_ms: f64) {
    let mut bytes = [0u8; 50];
    for (i, val) in m.text.iter().enumerate().take(50) {
        bytes[i] = *val;
    }
    let text = cstr(&bytes);
    if text.is_empty() {
        return;
    }
    s.log.add_message(s.line_no, &text, time_ms);
}

/// Records a PARAM_VALUE name/value pair.
pub fn on_param_value(s: &mut TlogBackend, m: &apm::PARAM_VALUE_DATA) {
    let mut bytes = [0u8; 16];
    for (i, val) in m.param_id.iter().enumerate().take(16) {
        bytes[i] = *val;
    }
    let name = cstr(&bytes);
    if name.is_empty() {
        return;
    }
    s.log.add_parameter(&name, m.param_value as f64);
}

/// Deduplicates mission items by sequence number, returning true when seq
/// was already recorded.
fn mark_seq_seen(s: &mut TlogBackend, seq: u16) -> bool {
    if s.seen_seqs.contains(&seq) {
        return true;
    }
    s.seen_seqs.insert(seq);
    false
}

/// Appends a MISSION_ITEM as a planned command, deduping by seq.
pub fn on_mission_item(s: &mut TlogBackend, m: &apm::MISSION_ITEM_DATA, time_ms: f64) {
    if mark_seq_seen(s, m.seq) {
        return;
    }
    s.log.append_command(MissionCommand {
        time_ms,
        sequence: m.seq as i64,
        command: m.command as i64,
        param1: m.param1 as f64,
        param2: m.param2 as f64,
        param3: m.param3 as f64,
        param4: m.param4 as f64,
        latitude: m.x as f64,
        longitude: m.y as f64,
        altitude: m.z as f64,
        frame: m.frame as i64,
        ..Default::default()
    });
}

/// Appends a MISSION_ITEM_INT as a planned command, scaling lat/lon from
/// 1e-7-degree integer units.
pub fn on_mission_item_int(s: &mut TlogBackend, m: &apm::MISSION_ITEM_INT_DATA, time_ms: f64) {
    if mark_seq_seen(s, m.seq) {
        return;
    }
    s.log.append_command(MissionCommand {
        time_ms,
        sequence: m.seq as i64,
        command: m.command as i64,
        param1: m.param1 as f64,
        param2: m.param2 as f64,
        param3: m.param3 as f64,
        param4: m.param4 as f64,
        latitude: m.x as f64 * 1e-7,
        longitude: m.y as f64 * 1e-7,
        altitude: m.z as f64,
        frame: m.frame as i64,
        ..Default::default()
    });
}

/// Records a COMMAND_INT issued during the flight.
pub fn on_command_int(
    s: &mut TlogBackend,
    m: &apm::COMMAND_INT_DATA,
    sys_id: u8,
    comp_id: u8,
    time_ms: f64,
) {
    s.log.append_mavlink_command(MavlinkCommand {
        time_ms,
        target_system: m.target_system as i64,
        target_component: m.target_component as i64,
        source_system: sys_id as i64,
        source_component: comp_id as i64,
        frame: m.frame as i64,
        command: m.command as i64,
        param1: m.param1 as f64,
        param2: m.param2 as f64,
        param3: m.param3 as f64,
        param4: m.param4 as f64,
        latitude: m.x as f64 * 1e-7,
        longitude: m.y as f64 * 1e-7,
        altitude: m.z as f64,
        ..Default::default()
    });
}

/// Records a COMMAND_LONG issued during the flight.
pub fn on_command_long(
    s: &mut TlogBackend,
    m: &apm::COMMAND_LONG_DATA,
    sys_id: u8,
    comp_id: u8,
    time_ms: f64,
) {
    s.log.append_mavlink_command(MavlinkCommand {
        time_ms,
        target_system: m.target_system as i64,
        target_component: m.target_component as i64,
        source_system: sys_id as i64,
        source_component: comp_id as i64,
        frame: 0,
        command: m.command as i64,
        param1: m.param1 as f64,
        param2: m.param2 as f64,
        param3: m.param3 as f64,
        param4: m.param4 as f64,
        latitude: m.param5 as f64,
        longitude: m.param6 as f64,
        altitude: m.param7 as f64,
        was_command_long: true,
        ..Default::default()
    });
}

/// Back-fills the Result of the most recent matching open command.
pub fn on_command_ack(s: &mut TlogBackend, m: &apm::COMMAND_ACK_DATA) {
    let cmd = m.command as i64;
    for i in (0..s.log.mavlink_commands.len()).rev() {
        if s.log.mavlink_commands[i].command == cmd && s.log.mavlink_commands[i].result == 0 {
            s.log.mavlink_commands[i].result = m.result as i64;
            return;
        }
    }
}

/// Routes a decoded MAVLink message to its handler. Unknown messages and
/// enum-decode failures fall through to the raw storage path.
pub fn route_message(s: &mut TlogBackend, msg: &MavMessage, sys_id: u8, comp_id: u8, time_ms: f64) {
    match msg {
        MavMessage::HEARTBEAT(m) => on_heartbeat(s, m, sys_id, time_ms),
        MavMessage::STATUSTEXT(m) => on_status_text(s, m, time_ms),
        MavMessage::PARAM_VALUE(m) => on_param_value(s, m),
        MavMessage::MISSION_ITEM(m) => on_mission_item(s, m, time_ms),
        MavMessage::MISSION_ITEM_INT(m) => on_mission_item_int(s, m, time_ms),
        MavMessage::COMMAND_INT(m) => on_command_int(s, m, sys_id, comp_id, time_ms),
        MavMessage::COMMAND_LONG(m) => on_command_long(s, m, sys_id, comp_id, time_ms),
        MavMessage::COMMAND_ACK(m) => on_command_ack(s, m),
        _ => {
            // Default branch handled by the caller via raw storage (it needs
            // the payload bytes, not the typed struct).
        }
    }
}
