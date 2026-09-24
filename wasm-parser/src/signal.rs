//! The agent `query_data` signal pipeline: decodes one (type, field)
//! series from the frozen TypeBody and runs a fieldstats operation,
//! returning the computed payload as JSON. Ported from the Go pair
//! `logservice.Series` (dataflash/series.go + parser/series.go) and
//! `agentservice/tools/signal.go` runQuery — the numeric core moved here,
//! while knowledge thresholds / AbsTime columns / budgets / truncation
//! stay on the Go side, which post-processes this response.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::curvebin::{read_field_gl, TYPE_BIN_HEADER};
use crate::fieldstats::{self as fs, Op, SeriesData};
use crate::logfile::LogFile;

/// Default/limit for raw downsampling (mirrors the Go tool's constants;
/// the Go side passes the budget-adjusted value so these only guard
/// direct callers).
const DEFAULT_RAW_MAX_POINTS: usize = 300;
const MAX_RAW_MAX_POINTS: usize = 2000;
/// Default merge gap for abnormal segments (Go `abnMergeGapSecs`).
const ABN_MERGE_GAP_SECS: f64 = 1.0;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalRequest {
    #[serde(rename = "type")]
    pub type_name: String,
    pub field: String,
    pub start_sec: Option<f64>,
    pub end_sec: Option<f64>,
    pub op: String,
    /// Explicit abnormal threshold (condition op is "gt", Go semantics).
    #[serde(default)]
    pub threshold: Option<f64>,
    /// Fully-resolved abnormal conditions (knowledge levels or the
    /// explicit threshold); each yields one result level.
    #[serde(default)]
    pub conds: Vec<CondDto>,
    #[serde(default)]
    pub max_points: Option<usize>,
    #[serde(default)]
    pub merge_gap_secs: Option<f64>,
}

#[derive(Deserialize, Clone)]
pub struct CondDto {
    pub op: String,
    pub value: f64,
}

/// round3 converges to the thousandth — raw points and abnormal-segment
/// times/values are plenty for trend reading at half the serialized size
/// (exact values come from the stats operations). Mirrors Go `round3`.
fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// rlePoints compresses runs of identical values: a run of >= 3 samples
/// outputs [t0, v, t1] (value held from t0 to t1), isolated points keep
/// [t, v]. Input must be equal-length and already rounded (equal rounding
/// is what "identical" means). Mirrors Go `rlePoints`.
fn rle_points(times: &[f64], values: &[f64]) -> Vec<Vec<f64>> {
    if times.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(times.len());
    let mut i = 0;
    while i < times.len() {
        let mut j = i + 1;
        while j < times.len() && values[j] == values[i] {
            j += 1;
        }
        if j - i >= 3 {
            out.push(vec![times[i], values[i], times[j - 1]]);
        } else {
            for k in i..j {
                out.push(vec![times[k], values[k]]);
            }
        }
        i = j;
    }
    out
}

/// Smallest first-sample timestamp across all packed type bodies (0 when
/// none) — the log-wide origin Series rebases onto. Port of Go
/// `EarliestBodyTimeMs`.
fn earliest_body_time_ms(lf: &LogFile) -> f64 {
    let mut earliest = 0.0f64;
    let mut first = true;
    for tb in lf.type_bodies.values() {
        if tb.row_count == 0 {
            continue;
        }
        if first || tb.base_time_ms < earliest {
            earliest = tb.base_time_ms;
            first = false;
        }
    }
    earliest
}

fn read_u32_le(bin: &[u8], off: usize) -> u32 {
    bin.get(off..off + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .unwrap_or(0)
}

/// Decodes one curve into parallel (relative-seconds, value) series.
/// Mirrors Go `CurveSeries` + the logservice rebase: per row the i32 LE
/// time column plus BaseTimeMs gives absolute ms, EarliestBodyTimeMs is
/// subtracted and scaled to seconds; values decode via the field's GL
/// type and scale.
///
/// Not-found vs empty follows Go's split: a type/field missing from the
/// curves REGISTRY (all format field names, registered on first sample)
/// errors with the ErrTypeNotFound text, while a registered field that
/// never made it into the packed TypeBody (topic-time columns like ulog
/// `event.timestamp`, string fields) yields an EMPTY series — Go's
/// `cd.body == nil || cd.column >= len(body.Fields)` early return.
pub fn decode_series(lf: &LogFile, type_name: &str, field: &str) -> Result<SeriesData, String> {
    let fields = lf
        .curves
        .get(type_name)
        .ok_or_else(|| format!("type not found: {type_name}"))?;
    if !fields.contains_key(field) {
        return Err(format!("type not found: {type_name}.{field}"));
    }
    let Some(tb) = lf.type_bodies.get(type_name) else {
        return Ok(SeriesData::default());
    };
    let Some(fi) = tb.field_index(field) else {
        return Ok(SeriesData::default());
    };
    let tf = &tb.fields[fi];

    let origin = earliest_body_time_ms(lf);
    let n = tf.count;
    let mut times = Vec::with_capacity(n);
    let mut values = Vec::with_capacity(n);
    for r in 0..n {
        let row_base = TYPE_BIN_HEADER + r * tb.stride;
        let dms = read_u32_le(&tb.bin, row_base) as i32;
        times.push((tb.base_time_ms + f64::from(dms) - origin) / 1000.0);
        values.push(read_field_gl(&tb.bin, row_base + tf.offset, tf.gl_type) * tf.scale);
    }
    Ok(SeriesData { times, values })
}

/// Runs one signal query and returns the response JSON:
/// { name, op, win, n, res, pts, stats, rate, trend, peaks, abn, error }.
/// Stats-family payloads are full precision (Go keeps them unrounded);
/// raw points and abnormal rows are round3'd; absolute-time columns,
/// budgets and segment truncation are the Go tool's job.
pub fn run_query(lf: &LogFile, req: &SignalRequest) -> Value {
    let mut res = json!({ "name": format!("{}.{}", req.type_name, req.field) });
    let op = match Op::parse(req.op.trim().to_lowercase().as_str()) {
        Some(op) => op,
        None => {
            res["error"] = json!(format!("不支持的 operation: {}", req.op));
            return res;
        }
    };
    res["op"] = json!(op.as_str());

    let data = match decode_series(lf, &req.type_name, &req.field) {
        Ok(d) => d,
        Err(e) => {
            res["error"] = json!(e);
            return res;
        }
    };
    let t0 = req.start_sec.unwrap_or(0.0);
    let t1 = req.end_sec.unwrap_or(0.0);
    let win = fs::slice(data.as_series(), t0, t1);
    res["n"] = json!(win.len());
    if !win.is_empty() {
        res["win"] = json!([round3(win.times[0]), round3(win.times[win.len() - 1])]);
    }

    match op {
        Op::Raw => {
            let mut max_points = req.max_points.unwrap_or(DEFAULT_RAW_MAX_POINTS);
            if max_points == 0 {
                max_points = DEFAULT_RAW_MAX_POINTS;
            }
            max_points = max_points.min(MAX_RAW_MAX_POINTS);
            let ds = fs::downsample(win, max_points);
            let (pts_t, pts_v): (Vec<f64>, Vec<f64>) = ds
                .times
                .iter()
                .zip(ds.values.iter())
                .map(|(t, v)| (round3(*t), round3(*v)))
                .unzip();
            let rle = rle_points(&pts_t, &pts_v);
            // Go marshals a nil slice as JSON null (distinct from []).
            res["pts"] = if rle.is_empty() {
                Value::Null
            } else {
                json!(rle)
            };
            if ds.len() > 1 {
                let span = win.times[win.len() - 1] - win.times[0];
                res["res"] = json!(round3(span / ds.len() as f64));
            }
        }
        Op::Min | Op::Max | Op::Avg | Op::MinMax | Op::P2P => {
            let st = fs::stats(win);
            if !st.ok {
                res["stats"] = json!({ "ok": false });
            } else {
                res["stats"] = json!({
                    "ok": true, "count": st.count,
                    "min": st.min, "minAt": st.min_at,
                    "max": st.max, "maxAt": st.max_at,
                    "avg": if st.has_avg { json!(st.avg) } else { Value::Null },
                    "p2p": st.p2p, "rms": st.rms,
                });
            }
        }
        Op::Derivative => {
            let d = fs::derivative::derivative(win);
            if !d.ok {
                res["rate"] = json!({ "ok": false });
            } else {
                res["rate"] = json!({
                    "ok": true,
                    "maxRate": d.max_rate, "maxRateAt": d.max_rate_at,
                    "avgRate": if d.has_avg { json!(d.avg_rate) } else { Value::Null },
                    "samples": d.samples,
                });
            }
        }
        Op::Trend => {
            let tr = fs::trend::trend(win);
            res["trend"] = json!({
                "ok": tr.ok, "slope": tr.slope, "direction": tr.direction,
                "first": tr.first, "last": tr.last,
                "change": tr.change, "duration": tr.duration,
            });
        }
        Op::Peaks => {
            let ps = fs::peaks::peaks(win, 0.0);
            res["peaks"] = json!({
                "ok": ps.ok, "count": ps.count,
                "maxPeak": ps.max_peak, "maxPeakAt": ps.max_peak_at,
                "prominence": ps.prominence,
            });
        }
        Op::Abnormal => {
            // Explicit threshold (Go semantics: condition op is always "gt");
            // pre-resolved conds (knowledge levels) take precedence.
            let mut conds: Vec<CondDto> = req.conds.clone();
            if conds.is_empty() {
                if let Some(th) = req.threshold {
                    conds.push(CondDto {
                        op: "gt".to_string(),
                        value: th,
                    });
                }
            }
            let gap = req.merge_gap_secs.unwrap_or(ABN_MERGE_GAP_SECS);
            let mut levels = Vec::with_capacity(conds.len());
            for c in &conds {
                if !matches!(c.op.as_str(), "lt" | "le" | "gt" | "ge") {
                    continue; // invalid level skipped, Go `cond.ValidOp` guard
                }
                let cond = fs::Cond::new(
                    match c.op.as_str() {
                        "lt" => "lt",
                        "le" => "le",
                        "ge" => "ge",
                        _ => "gt",
                    },
                    c.value,
                );
                let segs = fs::abnormal(win, cond);
                let merged = fs::merge_segments(&segs, gap);
                let total: f64 = merged.iter().map(|s| s.duration).sum();
                let rows: Vec<Vec<f64>> = merged
                    .iter()
                    .map(|s| {
                        vec![
                            round3(s.start),
                            round3(s.end),
                            round3(s.worst),
                            round3(s.extent),
                        ]
                    })
                    .collect();
                levels.push(json!({
                    "op": c.op, "value": c.value,
                    "segs": rows, "total": round3(total),
                }));
            }
            res["abn"] = json!(levels);
        }
    }
    res
}

/// Parses a request JSON string and runs it (wasm boundary: strings keep
/// f64 values exact end-to-end).
pub fn run_query_json(lf: &LogFile, req: &str) -> Result<String, String> {
    let req: SignalRequest =
        serde_json::from_str(req).map_err(|e| format!("bad signal request: {e}"))?;
    serde_json::to_string(&run_query(lf, &req)).map_err(|e| format!("encode response: {e}"))
}
