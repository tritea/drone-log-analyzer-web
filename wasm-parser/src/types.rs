//! Public metadata types, ported from `app/modules/parser/types.go`.
//! JSON field names mirror the Go struct field names as serialized by
//! logservice DTOs (camelCase happens at the DTO layer, not here).

use serde::{Deserialize, Serialize};

/// High-level metadata extracted from a parsed log: vehicle, firmware,
/// timing, and size.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LogSummary {
    #[serde(rename = "Filename")]
    pub filename: String,
    #[serde(rename = "FileSizeKB")]
    pub file_size_kb: f64,
    #[serde(rename = "VehicleType")]
    pub vehicle_type: String,
    #[serde(rename = "FirmwareVersion")]
    pub firmware_version: String,
    #[serde(rename = "FirmwareHash")]
    pub firmware_hash: String,
    #[serde(rename = "HardwareType")]
    pub hardware_type: String,
    #[serde(rename = "FreeRAM")]
    pub free_ram: i64,
    #[serde(rename = "DurationSecs")]
    pub duration_secs: f64,
    #[serde(rename = "TotalLines")]
    pub total_lines: i64,
    #[serde(rename = "Frame")]
    pub frame: String,
    #[serde(rename = "StartUnixSecs")]
    pub start_unix_secs: i64,
    #[serde(rename = "HasUTC")]
    pub has_utc: bool,
    #[serde(rename = "Format")]
    pub format: String,
    #[serde(rename = "Airframe")]
    pub airframe: String,
}

/// A single flight-mode transition at a given time.
#[derive(Debug, Clone, Default)]
pub struct ModeChange {
    pub mode: String,
    pub mode_num: i64,
    pub time_ms: f64,
}

/// One row of the mission command list (CMD messages).
#[derive(Debug, Clone, Default)]
pub struct MissionCommand {
    pub time_ms: f64,
    pub command_total: i64,
    pub sequence: i64,
    pub command: i64,
    pub param1: f64,
    pub param2: f64,
    pub param3: f64,
    pub param4: f64,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
    pub frame: i64,
}

/// One observed MAVLink command with its acknowledgement.
#[derive(Debug, Clone, Default)]
pub struct MavlinkCommand {
    pub time_ms: f64,
    pub target_system: i64,
    pub target_component: i64,
    pub source_system: i64,
    pub source_component: i64,
    pub frame: i64,
    pub command: i64,
    pub param1: f64,
    pub param2: f64,
    pub param3: f64,
    pub param4: f64,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
    pub result: i64,
    pub was_command_long: bool,
}

/// One ERR row: which subsystem flagged which code, and when.
#[derive(Debug, Clone, Default)]
pub struct LogError {
    pub time_ms: f64,
    pub subsys: i64,
    pub ecode: i64,
    pub lineno: i64,
}

/// One EV row: an event id fired at a given time.
#[derive(Debug, Clone, Default)]
pub struct LogEvent {
    pub time_ms: f64,
    pub id: i64,
    pub lineno: i64,
}
