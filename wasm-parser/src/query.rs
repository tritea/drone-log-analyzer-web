//! Logservice-level query projections over a finalized `LogFile`, ported
//! from `app/services/logservice/dataflash/logservice.go`. DTO field names
//! mirror the Go JSON tags exactly (camelCase); non-finite floats become the
//! string sentinels Go's custom marshalers emit ("NaN"/"Inf"/"-Inf") so the
//! frontend and the frozen l2 goldens see identical shapes.

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use serde::Serialize;
use serde_json::{json, Value};

use crate::logfile::LogFile;
use crate::typebody::base_type_name;

// ---- logdefs label tables (generated, embedded) ------------------------------

/// The static label tables shipped to the frontend, loaded once from the
/// generated `data/logdefs.json` (same source as Go's logdefs package).
struct LogDefs {
    events: HashMap<i64, String>,
    error_subsystems: HashMap<i64, String>,
    error_codes: HashMap<i64, HashMap<i64, String>>,
    general_error_codes: HashMap<i64, String>,
    commands: HashMap<i64, String>,
    frames: HashMap<i64, String>,
    results: HashMap<i64, String>,
}

fn parse_int_map(v: &Value) -> HashMap<i64, String> {
    v.as_object()
        .map(|o| {
            o.iter()
                .filter_map(|(k, name)| {
                    let key = k.parse::<i64>().ok()?;
                    let name = name.as_str()?.to_string();
                    Some((key, name))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn logdefs() -> &'static LogDefs {
    static TABLE: OnceLock<LogDefs> = OnceLock::new();
    TABLE.get_or_init(|| {
        let raw: Value =
            serde_json::from_str(include_str!("../data/logdefs.json")).expect("logdefs valid");
        let error_codes = raw["errorCodes"]
            .as_object()
            .map(|subs| {
                subs.iter()
                    .filter_map(|(subsys, codes)| {
                        let key = subsys.parse::<i64>().ok()?;
                        Some((key, parse_int_map(codes)))
                    })
                    .collect()
            })
            .unwrap_or_default();
        LogDefs {
            events: parse_int_map(&raw["events"]),
            error_subsystems: parse_int_map(&raw["errorSubsystems"]),
            error_codes,
            general_error_codes: parse_int_map(&raw["generalErrorCodes"]),
            commands: parse_int_map(&raw["commands"]),
            frames: parse_int_map(&raw["frames"]),
            results: parse_int_map(&raw["results"]),
        }
    })
}

fn map_get<'a>(m: &'a HashMap<i64, String>, id: i64) -> &'a str {
    m.get(&id).map(String::as_str).unwrap_or("")
}

/// MAV_CMD id → name ("" if unknown).
pub fn command_name(id: i64) -> &'static str {
    map_get(&logdefs().commands, id)
}

/// MAV_FRAME id → name ("" if unknown).
pub fn frame_name(id: i64) -> &'static str {
    map_get(&logdefs().frames, id)
}

/// MAV_RESULT id → name ("" if unknown).
pub fn result_name(id: i64) -> &'static str {
    map_get(&logdefs().results, id)
}

pub fn error_subsystem_name(id: i64) -> &'static str {
    map_get(&logdefs().error_subsystems, id)
}

/// Resolves a (subsystem, code) pair, falling back to the generic code table
/// when the subsystem defines nothing specific.
pub fn error_code_name(subsys: i64, code: i64) -> &'static str {
    if let Some(codes) = logdefs().error_codes.get(&subsys) {
        let name = map_get(codes, code);
        if !name.is_empty() {
            return name;
        }
    }
    map_get(&logdefs().general_error_codes, code)
}

pub fn event_name(id: i64) -> &'static str {
    map_get(&logdefs().events, id)
}

// ---- shared helpers ------------------------------------------------------------

/// Renders a float as a JSON-safe value: non-finite floats become the string
/// sentinels of Go's `finiteJSON` (Wails/JSON cannot carry NaN/Inf).
fn finite(v: f64) -> Value {
    if v.is_nan() {
        json!("NaN")
    } else if v.is_infinite() {
        json!(if v > 0.0 { "Inf" } else { "-Inf" })
    } else {
        json!(v)
    }
}

/// A curve's sample count post-finalize: the TypeBody column is the authority
/// (samples are released at freeze), 0 when no body exists (Go `CurveData.Count`).
fn curve_count(lf: &LogFile, type_name: &str, field: &str) -> i64 {
    lf.type_bodies
        .get(type_name)
        .and_then(|tb| tb.field_index(field).map(|i| tb.fields[i].count as i64))
        .unwrap_or(0)
}

// ---- DTOs ----------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryDto {
    pub filename: String,
    pub file_size_kb: f64,
    pub vehicle_type: String,
    pub firmware_version: String,
    pub firmware_hash: String,
    pub hardware_type: String,
    pub free_ram: i64,
    pub duration_secs: f64,
    pub total_lines: i64,
    pub frame: String,
    pub airframe: String,
    pub type_count: usize,
    pub start_unix_secs: i64,
    pub has_utc: bool,
    /// Absolute-ms origin the agent tool's relative seconds map back onto
    /// (= earliest TypeBody BaseTimeMs, including head types like FILE).
    pub start_time_ms: f64,
    pub format: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeInfoDto {
    pub name: String,
    pub fields: Vec<String>,
    pub count: i64,
    /// Go never assigns HasData, so it is always false on the wire.
    pub has_data: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldInfoDto {
    pub name: String,
    #[serde(rename = "type")]
    pub orig_type: String,
    pub min: f64,
    pub max: f64,
    pub count: i64,
    pub is_numeric: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeEntryDto {
    pub lineno: i64,
    pub time_ms: Value,
    pub mode: String,
    pub mode_num: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageEntryDto {
    pub lineno: i64,
    pub time_ms: Value,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEntryDto {
    pub lineno: i64,
    pub time_ms: Value,
    pub subsys: i64,
    pub e_code: i64,
    pub subsys_name: String,
    pub error_code: String,
    pub description: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventEntryDto {
    pub lineno: i64,
    pub time_ms: Value,
    pub id: i64,
    pub name: String,
}

/// One mission-command row. Param/coordinate floats go through `finite` like
/// Go's custom CommandEntry marshaler.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEntryDto {
    pub time_ms: Value,
    pub command_total: i64,
    pub sequence: i64,
    pub command: i64,
    pub command_name: String,
    pub param1: Value,
    pub param2: Value,
    pub param3: Value,
    pub param4: Value,
    pub latitude: Value,
    pub longitude: Value,
    pub altitude: Value,
    pub frame: i64,
    pub frame_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MavlinkCommandEntryDto {
    pub time_ms: Value,
    pub target_system: i64,
    pub target_component: i64,
    pub source_system: i64,
    pub source_component: i64,
    pub frame: i64,
    pub frame_name: String,
    pub command: i64,
    pub command_name: String,
    pub param1: Value,
    pub param2: Value,
    pub param3: Value,
    pub param4: Value,
    pub latitude: Value,
    pub longitude: Value,
    pub altitude: Value,
    pub result: i64,
    pub result_name: String,
    pub was_command_long: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogDefsDto {
    pub format: String,
    pub units: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub event_names: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub error_subsystems: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub error_codes: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub general_error_codes: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub nav_state_names: BTreeMap<String, String>,
}

// ---- projections ----------------------------------------------------------------

/// The load/summary response: parser summary plus the logservice-level
/// derivations (frame/airframe classification happens at finalize;
/// TypeCount/StartTimeMs are computed here).
pub fn summary(lf: &LogFile) -> SummaryDto {
    SummaryDto {
        filename: lf.summary.filename.clone(),
        file_size_kb: lf.summary.file_size_kb,
        vehicle_type: lf.summary.vehicle_type.clone(),
        firmware_version: lf.summary.firmware_version.clone(),
        firmware_hash: lf.summary.firmware_hash.clone(),
        hardware_type: lf.summary.hardware_type.clone(),
        free_ram: lf.summary.free_ram,
        duration_secs: lf.summary.duration_secs,
        total_lines: lf.summary.total_lines,
        frame: lf.summary.frame.clone(),
        airframe: lf.summary.airframe.clone(),
        type_count: lf.curves.len(),
        start_unix_secs: lf.summary.start_unix_secs,
        has_utc: lf.summary.has_utc,
        start_time_ms: lf.earliest_body_time_ms(),
        format: lf.summary.format.clone(),
    }
}

pub fn message_types(lf: &LogFile) -> Vec<TypeInfoDto> {
    let mut types = Vec::new();
    for (name, fields) in &lf.curves {
        let Some(fd) = lf.lookup_format(name) else {
            continue;
        };
        let numeric_fields: Vec<String> = fd
            .layout
            .iter()
            .filter(|fl| fl.in_body)
            .map(|fl| fl.name.clone())
            .collect();
        if numeric_fields.is_empty() {
            continue;
        }
        // Deterministic: the first in-body field's count (fields counts can
        // differ within one type).
        let count = fields
            .get(&numeric_fields[0])
            .map(|_| curve_count(lf, name, &numeric_fields[0]))
            .unwrap_or(0);
        types.push(TypeInfoDto {
            name: name.clone(),
            fields: numeric_fields,
            count,
            has_data: false,
        });
    }
    types.sort_by(|a, b| a.name.cmp(&b.name));
    types
}

pub fn fields(lf: &LogFile, type_name: &str) -> Option<Vec<FieldInfoDto>> {
    let fd = lf.lookup_format(type_name)?;
    let curves = lf.curves.get(type_name);
    Some(
        fd.layout
            .iter()
            .map(|fl| {
                let (min, max, count) = curves
                    .and_then(|c| c.get(&fl.name))
                    .map(|cd| (cd.min, cd.max, curve_count(lf, type_name, &fl.name)))
                    .unwrap_or((0.0, 0.0, 0));
                FieldInfoDto {
                    name: fl.name.clone(),
                    orig_type: fl.orig_type.clone(),
                    min,
                    max,
                    count,
                    is_numeric: fl.in_body,
                }
            })
            .collect(),
    )
}

pub fn parameters(lf: &LogFile) -> Vec<Value> {
    let mut params: Vec<Value> = lf
        .parameters
        .iter()
        .map(|(k, v)| json!({ "name": k, "value": finite(*v) }))
        .collect();
    params.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    params
}

pub fn mode_changes(lf: &LogFile) -> Vec<ModeEntryDto> {
    let mut modes: Vec<ModeEntryDto> = lf
        .mode_changes
        .iter()
        .map(|(lineno, m)| ModeEntryDto {
            lineno: *lineno as i64,
            time_ms: finite(m.time_ms),
            mode: m.mode.clone(),
            mode_num: m.mode_num,
        })
        .collect();
    modes.sort_by(|a, b| cmp_time_lineno(&a.time_ms, &b.time_ms, a.lineno, b.lineno));
    modes
}

pub fn messages(lf: &LogFile) -> Vec<MessageEntryDto> {
    let mut msgs: Vec<MessageEntryDto> = lf
        .messages
        .iter()
        .map(|(lineno, msg)| MessageEntryDto {
            lineno: *lineno as i64,
            time_ms: finite(*lf.message_times.get(lineno).unwrap_or(&0.0)),
            message: msg.clone(),
        })
        .collect();
    msgs.sort_by(|a, b| cmp_time_lineno(&a.time_ms, &b.time_ms, a.lineno, b.lineno));
    msgs
}

pub fn errors(lf: &LogFile) -> Vec<ErrorEntryDto> {
    let mut errs: Vec<ErrorEntryDto> = lf
        .errors
        .iter()
        .map(|e| {
            let code_name = error_code_name(e.subsys, e.ecode).to_string();
            let description = if code_name.is_empty() {
                format!("#{}", e.ecode)
            } else {
                code_name.clone()
            };
            ErrorEntryDto {
                lineno: e.lineno,
                time_ms: finite(e.time_ms),
                subsys: e.subsys,
                e_code: e.ecode,
                subsys_name: error_subsystem_name(e.subsys).to_string(),
                error_code: code_name,
                description,
            }
        })
        .collect();
    errs.sort_by(|a, b| cmp_time_lineno(&a.time_ms, &b.time_ms, a.lineno, b.lineno));
    errs
}

pub fn events(lf: &LogFile) -> Vec<EventEntryDto> {
    let is_ulog = lf.summary.format == "ulog";
    let mut out: Vec<EventEntryDto> = lf
        .events
        .iter()
        .map(|ev| {
            let name = if is_ulog {
                lf.px4_event_names
                    .get(&(ev.id & 0xFFFFFF))
                    .cloned()
                    .unwrap_or_default()
            } else {
                event_name(ev.id).to_string()
            };
            EventEntryDto {
                lineno: ev.lineno,
                time_ms: finite(ev.time_ms),
                id: ev.id,
                name,
            }
        })
        .collect();
    out.sort_by(|a, b| cmp_time_lineno(&a.time_ms, &b.time_ms, a.lineno, b.lineno));
    out
}

pub fn commands(lf: &LogFile) -> Vec<CommandEntryDto> {
    let mut out: Vec<CommandEntryDto> = lf
        .commands
        .iter()
        .map(|c| CommandEntryDto {
            time_ms: finite(c.time_ms),
            command_total: c.command_total,
            sequence: c.sequence,
            command: c.command,
            command_name: command_name(c.command).to_string(),
            param1: finite(c.param1),
            param2: finite(c.param2),
            param3: finite(c.param3),
            param4: finite(c.param4),
            latitude: finite(c.latitude),
            longitude: finite(c.longitude),
            altitude: finite(c.altitude),
            frame: c.frame,
            frame_name: frame_name(c.frame).to_string(),
        })
        .collect();
    out.sort_by(|a, b| {
        num(&a.time_ms)
            .partial_cmp(&num(&b.time_ms))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.sequence.cmp(&b.sequence))
    });
    out
}

pub fn mavlink_commands(lf: &LogFile) -> Vec<MavlinkCommandEntryDto> {
    let mut out: Vec<MavlinkCommandEntryDto> = lf
        .mavlink_commands
        .iter()
        .map(|c| MavlinkCommandEntryDto {
            time_ms: finite(c.time_ms),
            target_system: c.target_system,
            target_component: c.target_component,
            source_system: c.source_system,
            source_component: c.source_component,
            frame: c.frame,
            frame_name: frame_name(c.frame).to_string(),
            command: c.command,
            command_name: command_name(c.command).to_string(),
            param1: finite(c.param1),
            param2: finite(c.param2),
            param3: finite(c.param3),
            param4: finite(c.param4),
            latitude: finite(c.latitude),
            longitude: finite(c.longitude),
            altitude: finite(c.altitude),
            result: c.result,
            result_name: result_name(c.result).to_string(),
            was_command_long: c.was_command_long,
        })
        .collect();
    out.sort_by(|a, b| {
        num(&a.time_ms)
            .partial_cmp(&num(&b.time_ms))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// PX4 nav-state names (Go px4NavStateNames static table).
fn px4_nav_state_names() -> BTreeMap<String, String> {
    [
        (0, "MANUAL"),
        (1, "ALTCTL"),
        (2, "POSCTL"),
        (3, "AUTO_MISSION"),
        (4, "AUTO_LOITER"),
        (5, "AUTO_RTL"),
        (6, "POSITION"),
        (7, "ACRO"),
        (8, "OFFBOARD"),
        (9, "STABILIZED"),
        (10, "RATTITUDE"),
        (11, "AUTO_TAKEOFF"),
        (12, "AUTO_LAND"),
        (13, "AUTO_FOLLOW_TARGET"),
        (14, "AUTO_PRECLAND"),
        (15, "ORBIT"),
        (16, "AUTO_VTOL_TAKEOFF"),
        (17, "AUTO_VTOL_LAND"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

pub fn log_defs(lf: &LogFile) -> LogDefsDto {
    let mut units = BTreeMap::new();
    for (msg_name, fields) in &lf.curves {
        for (field_name, cd) in fields {
            if !cd.unit.is_empty() {
                units.insert(format!("{}.{}", msg_name, field_name), cd.unit.clone());
            }
        }
    }
    let mut dto = LogDefsDto {
        format: lf.summary.format.clone(),
        units,
        event_names: BTreeMap::new(),
        error_subsystems: BTreeMap::new(),
        error_codes: BTreeMap::new(),
        general_error_codes: BTreeMap::new(),
        nav_state_names: BTreeMap::new(),
    };
    match dto.format.as_str() {
        "ulog" => dto.nav_state_names = px4_nav_state_names(),
        "tlog" => {}
        _ => {
            let d = logdefs();
            dto.event_names = int_map_json(&d.events);
            dto.error_subsystems = int_map_json(&d.error_subsystems);
            dto.error_codes = nested_int_map_json(&d.error_codes);
            dto.general_error_codes = int_map_json(&d.general_error_codes);
        }
    }
    dto
}

fn int_map_json(m: &HashMap<i64, String>) -> BTreeMap<String, String> {
    m.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

fn nested_int_map_json(
    m: &HashMap<i64, HashMap<i64, String>>,
) -> BTreeMap<String, BTreeMap<String, String>> {
    m.iter()
        .map(|(k, v)| (k.to_string(), int_map_json(v)))
        .collect()
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or(f64::NAN)
}

/// The (timeMs, lineno) ordering Go applies to messages/errors/events/modes.
fn cmp_time_lineno(a_time: &Value, b_time: &Value, a_line: i64, b_line: i64) -> std::cmp::Ordering {
    num(a_time)
        .partial_cmp(&num(b_time))
        .unwrap_or(std::cmp::Ordering::Equal)
        .then(a_line.cmp(&b_line))
}

/// Strips a trailing numeric instance suffix (exported for parity tests).
pub fn base_name(name: &str) -> &str {
    base_type_name(name)
}
