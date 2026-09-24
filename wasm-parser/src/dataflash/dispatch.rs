//! Special-record dispatch (PARM/MSG/ERR/EV/CMD/MAVC/MODE), ported from
//! `app/modules/parser/dataflash/dispatch.go`.

use crate::format::FormatDef;
use crate::logfile::LogFile;
use crate::types::{LogError, LogEvent, MavlinkCommand, MissionCommand, ModeChange};

use super::fielddecode::FieldValue;
use super::modetable::{detect_firmware, vehicle_mode_label};

/// Routes the recognized record types to their dedicated handlers; other
/// types are ignored.
pub fn dispatch_record(
    lf: &mut LogFile,
    msg_name: &str,
    fd: &FormatDef,
    values: &[FieldValue],
    time_ms: f64,
    lineno: usize,
) {
    match msg_name {
        "PARM" => handle_parameter(lf, fd, values),
        "MSG" | "Message" => handle_message(lf, fd, values, time_ms, lineno),
        "ERR" => handle_error(lf, fd, values, time_ms, lineno),
        "EV" => handle_event(lf, fd, values, time_ms, lineno),
        "CMD" => lf.append_command(build_mission_command(fd, values, time_ms)),
        "MAVC" => {
            lf.append_mavlink_command(build_mavlink_command(fd, values, time_ms));
        }
        "MODE" => record_mode_change(lf, fd, values, time_ms, lineno),
        _ => {}
    }
}

/// Reports whether a message name has a dedicated record handler.
pub fn is_dispatch_message(msg_name: &str) -> bool {
    matches!(
        msg_name,
        "PARM" | "MSG" | "Message" | "ERR" | "EV" | "CMD" | "MAVC" | "MODE"
    )
}

/// Go stores parameters whose value text parses (NaN parses fine and is kept;
/// only unparseable strings are dropped).
fn parse_param_value(v: &FieldValue) -> Option<f64> {
    match v {
        FieldValue::Num(n) => Some(*n),
        FieldValue::Str(s) => s.parse::<f64>().ok(),
    }
}

fn handle_parameter(lf: &mut LogFile, fd: &FormatDef, values: &[FieldValue]) {
    let mut name_idx = index_of_field_by_aliases(fd, &["name"]);
    let value_idx = index_of_field_by_aliases(fd, &["value"]);
    if name_idx.is_none() && values.len() >= 2 {
        name_idx = Some(0);
    }
    let (ni, vi) = match (name_idx, value_idx) {
        (Some(n), Some(v)) if n < values.len() && v < values.len() => (n, v),
        _ => return,
    };
    let name = match &values[ni] {
        FieldValue::Str(s) => s.clone(),
        FieldValue::Num(n) => format_go_float(*n),
    };
    if name.is_empty() || name.eq_ignore_ascii_case("Name") {
        return;
    }
    if let Some(f) = parse_param_value(&values[vi]) {
        lf.add_parameter(&name, f);
    }
}

/// Formats an f64 the way Go's fmt %v does for the values we care about
/// (integral values print without a decimal point).
fn format_go_float(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

fn handle_message(
    lf: &mut LogFile,
    fd: &FormatDef,
    values: &[FieldValue],
    time_ms: f64,
    lineno: usize,
) {
    if values.is_empty() {
        return;
    }
    let msg = extract_message_text(fd, values);
    if msg.is_empty() {
        return;
    }
    if let Some(rest) = msg.strip_prefix("Frame: ") {
        lf.summary.frame = rest.to_string();
    } else if let Some(rest) = msg.strip_prefix("Frame ") {
        lf.summary.frame = rest.to_string();
    }
    detect_firmware(lf, &msg);
    lf.add_message(lineno, &msg, time_ms);
}

fn handle_error(
    lf: &mut LogFile,
    fd: &FormatDef,
    values: &[FieldValue],
    time_ms: f64,
    lineno: usize,
) {
    let mut subsys: i64 = -1;
    let mut ecode: i64 = -1;
    for_each_named_field(fd, values, &mut |field, val| match field {
        "subsys" | "subsystem" | "sub_system" | "sub" => subsys = val as i64,
        "ecode" | "errorcode" | "error_code" | "code" | "err" => ecode = val as i64,
        _ => {}
    });
    if subsys >= 0 || ecode >= 0 {
        lf.add_error(LogError {
            time_ms,
            subsys,
            ecode,
            lineno: lineno as i64,
        });
    }
}

fn handle_event(
    lf: &mut LogFile,
    fd: &FormatDef,
    values: &[FieldValue],
    time_ms: f64,
    lineno: usize,
) {
    for (i, fn_) in fd.field_names.iter().enumerate() {
        let Some(v) = values.get(i) else { break };
        if fn_.trim().eq_ignore_ascii_case("id") {
            lf.add_event(LogEvent {
                time_ms,
                id: v.to_f64() as i64,
                lineno: lineno as i64,
            });
            break;
        }
    }
}

/// Assembles a MissionCommand from a CMD record.
fn build_mission_command(fd: &FormatDef, values: &[FieldValue], time_ms: f64) -> MissionCommand {
    let mut cmd = MissionCommand {
        time_ms,
        ..Default::default()
    };
    for_each_named_field(fd, values, &mut |field, val| match field {
        "ctot" | "command_total" | "commandtotal" | "total" => cmd.command_total = val as i64,
        "cnum" | "seq" | "sequence" => cmd.sequence = val as i64,
        "cid" | "cmd" | "command" | "commandid" => cmd.command = val as i64,
        "prm1" | "p1" | "param1" => cmd.param1 = val,
        "prm2" | "p2" | "param2" => cmd.param2 = val,
        "prm3" | "p3" | "param3" => cmd.param3 = val,
        "prm4" | "p4" | "param4" => cmd.param4 = val,
        "lat" | "latitude" => cmd.latitude = sanitize_coord(val),
        "lng" | "lon" | "longitude" => cmd.longitude = sanitize_coord(val),
        "alt" | "altitude" => cmd.altitude = val,
        "frm" | "frame" => cmd.frame = val as i64,
        _ => {}
    });
    cmd
}

/// Assembles a MavlinkCommand from a MAVC record.
fn build_mavlink_command(fd: &FormatDef, values: &[FieldValue], time_ms: f64) -> MavlinkCommand {
    let mut cmd = MavlinkCommand {
        time_ms,
        ..Default::default()
    };
    for_each_named_field(fd, values, &mut |field, val| match field {
        "ts" | "target_system" | "targetsystem" | "targetsys" => cmd.target_system = val as i64,
        "tc" | "target_component" | "targetcomponent" => cmd.target_component = val as i64,
        "ss" | "source_system" | "sourcesystem" | "sourcesys" => cmd.source_system = val as i64,
        "sc" | "source_component" | "sourcecomponent" => cmd.source_component = val as i64,
        "fr" | "frame" => cmd.frame = val as i64,
        "cmd" | "command" | "commandid" | "cid" => cmd.command = val as i64,
        "p1" | "prm1" | "param1" => cmd.param1 = val,
        "p2" | "prm2" | "param2" => cmd.param2 = val,
        "p3" | "prm3" | "param3" => cmd.param3 = val,
        "p4" | "prm4" | "param4" => cmd.param4 = val,
        "x" | "lat" | "latitude" => cmd.latitude = sanitize_coord(val),
        "y" | "lng" | "lon" | "longitude" => cmd.longitude = sanitize_coord(val),
        "z" | "alt" | "altitude" => cmd.altitude = val,
        "res" | "result" => cmd.result = val as i64,
        "wl" | "was_command_long" | "wascommandlong" => cmd.was_command_long = val != 0.0,
        _ => {}
    });
    cmd
}

/// Normalizes a raw lat/lon integer to degrees, treating NaN/Inf as zero.
fn sanitize_coord(v: f64) -> f64 {
    if v.is_nan() || v.is_infinite() {
        return 0.0;
    }
    if v.abs() > 360.0 {
        v / 1e7
    } else {
        v
    }
}

/// Pulls the human-readable text out of a MSG/Message record, skipping time
/// fields.
fn extract_message_text(fd: &FormatDef, values: &[FieldValue]) -> String {
    for (i, fn_) in fd.field_names.iter().enumerate() {
        let Some(v) = values.get(i) else { break };
        match fn_.trim().to_ascii_lowercase().as_str() {
            "msg" | "message" | "text" => {
                return match v {
                    FieldValue::Str(s) => s.clone(),
                    FieldValue::Num(n) => format_go_float(*n),
                }
            }
            _ => {}
        }
    }
    for (i, v) in values.iter().enumerate() {
        if let Some(fn_) = fd.field_names.get(i) {
            let f = fn_.trim().to_ascii_lowercase();
            if f == "timems" || f == "timeus" || f == "time" {
                continue;
            }
        }
        if let FieldValue::Str(s) = v {
            return s.clone();
        }
    }
    String::new()
}

fn record_mode_change(
    lf: &mut LogFile,
    fd: &FormatDef,
    values: &[FieldValue],
    time_ms: f64,
    lineno: usize,
) {
    let mut mode_str = String::new();
    let mut mode_num: i64 = 0;
    for (i, fn_) in fd.field_names.iter().enumerate() {
        let Some(v) = values.get(i) else { break };
        match fn_.as_str() {
            "Mode" => {
                mode_str = match v {
                    FieldValue::Str(s) => s.clone(),
                    FieldValue::Num(n) => format_go_float(*n),
                }
            }
            "ModeNum" => {
                if let FieldValue::Str(s) = v {
                    if let Ok(n) = s.parse::<i64>() {
                        mode_num = n;
                    }
                } else if let FieldValue::Num(n) = v {
                    mode_num = *n as i64;
                }
            }
            _ => {}
        }
    }
    if mode_str.is_empty() {
        return;
    }
    if let Ok(n) = mode_str.parse::<i64>() {
        mode_str = vehicle_mode_label(lf, n);
    }
    lf.add_mode_change(
        lineno,
        ModeChange {
            mode: mode_str,
            mode_num,
            time_ms,
        },
    );
}

/// Position of the first field whose name matches any alias
/// (case-insensitive, trimmed).
fn index_of_field_by_aliases(fd: &FormatDef, aliases: &[&str]) -> Option<usize> {
    for (i, fn_) in fd.field_names.iter().enumerate() {
        let field = fn_.trim().to_ascii_lowercase();
        if aliases.iter().any(|a| field == *a) {
            return Some(i);
        }
    }
    None
}

/// Iterates every typed field in values, invoking fn with the lower-cased
/// trimmed field name and its float value. Iteration stops when values is
/// exhausted. Non-parseable values become NaN (Go ToFloat64 semantics).
fn for_each_named_field(fd: &FormatDef, values: &[FieldValue], f: &mut dyn FnMut(&str, f64)) {
    for (i, fname) in fd.field_names.iter().enumerate() {
        let Some(v) = values.get(i) else { return };
        f(fname.trim().to_ascii_lowercase().as_str(), v.to_f64());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fd(names: &[&str]) -> FormatDef {
        FormatDef {
            name: "TEST".into(),
            field_names: names.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn parameter_last_wins_and_name_skipped() {
        let mut lf = LogFile::new("t", "apm");
        let f = fd(&["TimeUS", "Name", "Value"]);
        let vals = |t: &str, n: &str, v: &str| {
            vec![
                FieldValue::Str(t.into()),
                FieldValue::Str(n.into()),
                FieldValue::Str(v.into()),
            ]
        };
        handle_parameter(&mut lf, &f, &vals("1000", "ATC_RAT_RLL_P", "0.135"));
        handle_parameter(&mut lf, &f, &vals("2000", "ATC_RAT_RLL_P", "0.145"));
        assert_eq!(lf.parameters["ATC_RAT_RLL_P"], 0.145);
        assert!(!lf.parameters.contains_key("1000"));
    }

    #[test]
    fn coords_sanitized() {
        assert!((sanitize_coord(341234567.0) - 34.1234567).abs() < 1e-9);
        assert_eq!(sanitize_coord(180.0), 180.0);
        assert_eq!(sanitize_coord(f64::NAN), 0.0);
        assert_eq!(sanitize_coord(f64::INFINITY), 0.0);
    }
}
