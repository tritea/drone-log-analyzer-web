//! Shared golden-parity comparison helpers. See tests/golden_dataflash.rs for
//! the full documentation of what is compared and why.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use dla_parser::logfile::LogFile;
use dla_parser::query;
use dla_parser::{next_generation, Session};
use serde_json::Value;

pub fn fixtures() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

pub fn parse_chunked(input: &Path, chunk_size: usize) -> Session {
    let bytes = fs::read(input).expect("fixture input readable");
    let mut session = Session::new(next_generation(), input.to_str().unwrap_or("log"));
    for chunk in bytes.chunks(chunk_size) {
        session.feed(chunk).expect("feed ok");
    }
    session.finish().expect("finish ok");
    session
}

pub fn compare_type_bodies(dir: &Path, lf: &LogFile) {
    let types_dir = dir.join("types");
    let mut checked = 0usize;
    for entry in fs::read_dir(&types_dir).expect("types dir") {
        let entry = entry.expect("entry");
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(type_name) = name.strip_suffix(".bin") else {
            continue;
        };
        let golden = fs::read(entry.path()).expect("golden bin");
        let actual = lf
            .type_body_bytes(type_name)
            .unwrap_or_else(|| panic!("missing TypeBody {}", type_name));
        assert_eq!(
            golden,
            actual,
            "TypeBody {} differs from Go golden (len golden={} actual={})",
            type_name,
            golden.len(),
            actual.len()
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "no golden type bodies found in {}",
        types_dir.display()
    );
}

pub fn compare_curves(dir: &Path, lf: &LogFile) {
    let curves_dir = dir.join("curves");
    let mut checked = 0usize;
    for entry in fs::read_dir(&curves_dir).expect("curves dir") {
        let entry = entry.expect("entry");
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(stem) = name.strip_suffix(".bin") else {
            continue;
        };
        let (type_name, field) = stem.split_once('.').expect("TYPE.FIELD naming");
        let golden = fs::read(entry.path()).expect("golden bin");
        let actual = lf
            .curve_bytes(type_name, field)
            .unwrap_or_else(|| panic!("missing curve {}.{}", type_name, field));
        assert_eq!(
            golden, actual,
            "curve {}.{} differs from Go golden",
            type_name, field
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "no golden curves found in {}",
        curves_dir.display()
    );
}

pub fn l2(dir: &Path) -> serde_json::Value {
    let raw = fs::read_to_string(dir.join("l2.json")).expect("l2.json");
    serde_json::from_str(&raw).expect("l2.json valid")
}

fn as_f64(v: &serde_json::Value) -> f64 {
    v.as_f64().expect("numeric golden value")
}

pub fn compare_summary(dir: &Path, lf: &LogFile) {
    let golden = l2(dir)["summary"].clone();
    let expect_eq = |k: &str, actual: f64| {
        assert!(
            (as_f64(&golden[k]) - actual).abs() < 1e-12,
            "summary.{}: golden={} actual={}",
            k,
            golden[k],
            actual
        );
    };
    expect_eq("fileSizeKB", lf.summary.file_size_kb);
    expect_eq("durationSecs", lf.summary.duration_secs);
    expect_eq("totalLines", lf.summary.total_lines as f64);
    expect_eq("startUnixSecs", lf.summary.start_unix_secs as f64);
    assert_eq!(
        golden["hasUTC"].as_bool().unwrap(),
        lf.summary.has_utc,
        "summary.hasUTC"
    );
    for (jk, actual) in [
        ("frame", lf.summary.frame.as_str()),
        ("vehicleType", lf.summary.vehicle_type.as_str()),
        ("firmwareVersion", lf.summary.firmware_version.as_str()),
        ("hardwareType", lf.summary.hardware_type.as_str()),
    ] {
        assert_eq!(golden[jk].as_str().unwrap_or(""), actual, "summary.{}", jk);
    }
    expect_eq("freeRAM", lf.summary.free_ram as f64);
    assert_eq!(
        golden["airframe"].as_str().unwrap_or(""),
        lf.summary.airframe,
        "summary.airframe"
    );
    // logservice-level derivations: StartTimeMs = EarliestBodyTimeMs,
    // TypeCount = len(Curves).
    expect_eq("startTimeMs", lf.earliest_body_time_ms());
    expect_eq("typeCount", lf.curves.len() as f64);
}

pub fn compare_parameters(dir: &Path, lf: &LogFile) {
    let golden_l2 = l2(dir);
    let golden = golden_l2["parameters"]
        .as_array()
        .expect("parameters array");
    let mut want: BTreeMap<String, f64> = BTreeMap::new();
    for p in golden {
        let name = p["name"].as_str().expect("name");
        match &p["value"] {
            serde_json::Value::Number(n) => {
                want.insert(name.to_string(), n.as_f64().expect("f64"));
            }
            // Go's custom marshaler renders NaN/±Inf as strings.
            serde_json::Value::String(s) => {
                let v = match s.as_str() {
                    "NaN" => f64::NAN,
                    "Inf" => f64::INFINITY,
                    "-Inf" => f64::NEG_INFINITY,
                    _ => panic!("unexpected param string {}", s),
                };
                want.insert(name.to_string(), v);
            }
            _ => panic!("unexpected param value"),
        }
    }
    assert_eq!(lf.parameters.len(), want.len(), "parameter count");
    for (k, v) in &want {
        let actual = lf
            .parameters
            .get(k)
            .unwrap_or_else(|| panic!("missing param {}", k));
        assert!(
            (v.is_nan() && actual.is_nan()) || v == actual,
            "param {}: golden={} actual={}",
            k,
            v,
            actual
        );
    }
}

pub fn compare_series(dir: &Path, lf: &LogFile) {
    let golden_l2 = l2(dir);
    let golden_series = golden_l2["series"].as_object().expect("series object");
    let earliest = lf.earliest_body_time_ms();
    assert!(
        !golden_series.is_empty(),
        "fixture has no series to compare"
    );
    for (key, sv) in golden_series {
        let (type_name, field) = key.split_once('.').expect("TYPE.FIELD key");
        // Go emits empty arrays for non-body fields; those need no Rust
        // counterpart.
        let g_times = sv["times"].as_array().expect("times");
        let g_values = sv["values"].as_array().expect("values");
        if g_times.is_empty() && g_values.is_empty() {
            continue;
        }
        let Some((times, values)) = lf.curve_series(type_name, field) else {
            panic!("missing series {}", key);
        };
        assert_eq!(g_times.len(), times.len(), "series {} times len", key);
        assert_eq!(g_values.len(), values.len(), "series {} values len", key);
        for (i, gt) in g_times.iter().enumerate() {
            let want = match gt {
                serde_json::Value::Number(n) => n.as_f64().unwrap(),
                _ => panic!("series time not numeric"),
            };
            let got = (times[i] - earliest) / 1000.0;
            assert!(
                (want - got).abs() < 1e-9,
                "series {} times[{}]: golden={} actual={}",
                key,
                i,
                want,
                got
            );
        }
        for (i, gv) in g_values.iter().enumerate() {
            match gv {
                serde_json::Value::Number(n) => {
                    let want = n.as_f64().unwrap();
                    assert!(
                        (want - values[i]).abs() < 1e-9,
                        "series {} values[{}]: golden={} actual={}",
                        key,
                        i,
                        want,
                        values[i]
                    );
                }
                serde_json::Value::String(_) => {
                    assert!(
                        !values[i].is_finite(),
                        "series {} values[{}] expected non-finite, actual={}",
                        key,
                        i,
                        values[i]
                    );
                }
                _ => panic!("unexpected series value"),
            }
        }
    }
}

pub fn compare_type_schema(dir: &Path, lf: &LogFile) {
    let golden_l2 = l2(dir);
    let golden = golden_l2["typeSchema"].as_object().expect("typeSchema");
    let actual = serde_json::to_value(lf.type_schema()).expect("schema serializable");
    let actual = actual.as_object().expect("schema object");
    assert_eq!(golden.len(), actual.len(), "schema type count");
    for (name, gentry) in golden {
        let aentry = actual
            .get(name)
            .unwrap_or_else(|| panic!("schema missing {}", name));
        let g = gentry.as_object().unwrap();
        let a = aentry.as_object().unwrap();
        assert_eq!(
            g["rowCount"].as_u64().unwrap(),
            a["rowCount"].as_u64().unwrap(),
            "schema {} rowCount",
            name
        );
        assert_eq!(
            g["stride"].as_u64().unwrap(),
            a["stride"].as_u64().unwrap(),
            "schema {} stride",
            name
        );
        assert!(
            (g["baseTimeMs"].as_f64().unwrap() - a["baseTimeMs"].as_f64().unwrap()).abs() < 1e-9,
            "schema {} baseTimeMs",
            name
        );
        let gfields = g["fields"].as_array().unwrap();
        let afields = a["fields"].as_array().unwrap();
        assert_eq!(gfields.len(), afields.len(), "schema {} field count", name);
        for (gf, af) in gfields.iter().zip(afields.iter()) {
            assert_eq!(
                gf["name"].as_str().unwrap(),
                af["name"].as_str().unwrap(),
                "schema {} field name",
                name
            );
            assert_eq!(
                gf["glType"].as_u64().unwrap(),
                af["glType"].as_u64().unwrap(),
                "schema {}.{} glType",
                name,
                gf["name"]
            );
            assert_eq!(
                gf["offset"].as_u64().unwrap(),
                af["offset"].as_u64().unwrap(),
                "schema {}.{} offset",
                name,
                gf["name"]
            );
            assert_eq!(
                gf["count"].as_u64().unwrap(),
                af["count"].as_u64().unwrap(),
                "schema {}.{} count",
                name,
                gf["name"]
            );
            for k in ["scale", "min", "max"] {
                let gv = gf[k].as_f64().unwrap();
                let av = af[k].as_f64().unwrap();
                assert!(
                    (gv.is_nan() && av.is_nan()) || (gv - av).abs() < 1e-9,
                    "schema {}.{} {}: golden={} actual={}",
                    name,
                    gf["name"],
                    k,
                    gv,
                    av
                );
            }
        }
    }
}

/// JSON value equality with numeric awareness: Go marshals a float64 1 as
/// `1` while serde renders `1.0`, so numbers compare by value, everything
/// else structurally.
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Null, Value::Null) => true,
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(x, y)| values_equal(x, y))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| values_equal(v, w)))
        }
        _ => false,
    }
}

fn expect_array(golden: &Value, actual: Value, section: &str) {
    let g = golden
        .as_array()
        .unwrap_or_else(|| panic!("golden {section} array"));
    let a = actual
        .as_array()
        .unwrap_or_else(|| panic!("actual {section} array"));
    assert_eq!(g.len(), a.len(), "{section} length");
    for (i, (gv, av)) in g.iter().zip(a.iter()).enumerate() {
        assert!(
            values_equal(gv, av),
            "{section}[{i}] differs:\n  golden: {gv}\n  actual: {av}"
        );
    }
}

/// Locks the logservice query projections (the shapes the frontend and the
/// agent consume) against the frozen l2 sections.
pub fn compare_query_sections(dir: &Path, lf: &LogFile) {
    let g = l2(dir);

    expect_array(
        &g["messageTypes"],
        serde_json::to_value(query::message_types(lf)).unwrap(),
        "messageTypes",
    );

    let golden_fields = g["fields"].as_object().expect("golden fields object");
    for (type_name, gfields) in golden_fields {
        let actual = query::fields(lf, type_name)
            .unwrap_or_else(|| panic!("fields missing for {type_name}"));
        expect_array(
            gfields,
            serde_json::to_value(actual).unwrap(),
            &format!("fields.{type_name}"),
        );
    }

    expect_array(
        &g["commands"],
        serde_json::to_value(query::commands(lf)).unwrap(),
        "commands",
    );
    expect_array(
        &g["mavlinkCommands"],
        serde_json::to_value(query::mavlink_commands(lf)).unwrap(),
        "mavlinkCommands",
    );
    expect_array(
        &g["modeChanges"],
        serde_json::to_value(query::mode_changes(lf)).unwrap(),
        "modeChanges",
    );
    expect_array(
        &g["messages"],
        serde_json::to_value(query::messages(lf)).unwrap(),
        "messages",
    );
    expect_array(
        &g["errors"],
        serde_json::to_value(query::errors(lf)).unwrap(),
        "errors",
    );
    expect_array(
        &g["events"],
        serde_json::to_value(query::events(lf)).unwrap(),
        "events",
    );

    let golden_defs = &g["logDefs"];
    let actual_defs = serde_json::to_value(query::log_defs(lf)).unwrap();
    assert!(
        values_equal(golden_defs, &actual_defs),
        "logDefs differs:\n  golden: {golden_defs}\n  actual: {actual_defs}"
    );

    compare_signal(lf, &g);
}

/// Locks the agent signal pipeline (wasm signalQuery) against the frozen
/// golden mirror computed over the Go logservice + fieldstats stack.
fn compare_signal(lf: &LogFile, g: &Value) {
    let entries = g["signal"].as_array().expect("golden signal array");
    assert!(!entries.is_empty(), "fixture has no signal queries");
    for (i, entry) in entries.iter().enumerate() {
        let req = serde_json::to_string(&entry["req"]).expect("req serializable");
        let resp = dla_parser::signal::run_query_json(lf, &req)
            .unwrap_or_else(|e| panic!("signal[{i}] failed: {e}"));
        let actual: Value =
            serde_json::from_str(&resp).unwrap_or_else(|e| panic!("signal[{i}] decode: {e}"));
        assert!(
            values_equal(&entry["resp"], &actual),
            "signal[{i}] differs:\n  req:    {}\n  golden: {}\n  actual: {}",
            entry["req"],
            entry["resp"],
            actual
        );
    }
}

/// Runs the full comparison battery for one fixture directory.
pub fn run_golden(dir: &Path, input: &Path) {
    // Chunk size 7: no record or line boundary alignment, exercises the
    // incremental Wait paths.
    let mut session = parse_chunked(input, 7);
    let lf = session.finish().expect("finish");
    compare_type_bodies(dir, lf);
    compare_curves(dir, lf);
    compare_summary(dir, lf);
    compare_parameters(dir, lf);
    compare_series(dir, lf);
    compare_type_schema(dir, lf);
    compare_query_sections(dir, lf);
}
