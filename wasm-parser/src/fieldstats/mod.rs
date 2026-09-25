//! Time-window statistics over timestamped numeric series, ported verbatim
//! from `app/modules/fieldstats` (the Go implementation is the spec — loop
//! order and float semantics included, so results match bit-for-bit).
//! All operations skip non-finite samples (NaN/±Inf), keeping outputs safe
//! to JSON-serialize.

pub mod derivative;
pub mod merge;
pub mod peaks;
pub mod raw;
pub mod stats;
pub mod threshold;
pub mod trend;
pub mod window;

pub use derivative::{derivative, DerivativeStats};
pub use merge::merge_segments;
pub use peaks::{peaks, PeakStats};
pub use raw::downsample;
pub use stats::stats;
pub use threshold::{abnormal, Cond, Segment};
pub use trend::{trend, TrendStats};
pub use window::slice;

/// Query operation kind (mirrors Go `fieldstats.Op`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Raw,
    Min,
    Max,
    Avg,
    MinMax,
    P2P,
    Derivative,
    Trend,
    Peaks,
    Abnormal,
}

impl Op {
    /// Parses an op from its wire name; the "stats" alias (the payload family
    /// name models sometimes copy verbatim) normalizes to MinMax, matching
    /// the Go tool's fallback.
    pub fn parse(s: &str) -> Option<Op> {
        match s {
            "raw" => Some(Op::Raw),
            "min" => Some(Op::Min),
            "max" => Some(Op::Max),
            "avg" => Some(Op::Avg),
            "minmax" | "stats" => Some(Op::MinMax),
            "p2p" => Some(Op::P2P),
            "derivative" => Some(Op::Derivative),
            "trend" => Some(Op::Trend),
            "peaks" => Some(Op::Peaks),
            "abnormal" => Some(Op::Abnormal),
            _ => None,
        }
    }

    /// Echoes the op's wire name (Go writes `res.Op = string(op)` — the
    /// requested op, with the "stats" alias normalized to "minmax").
    pub fn as_str(self) -> &'static str {
        match self {
            Op::Raw => "raw",
            Op::Min => "min",
            Op::Max => "max",
            Op::Avg => "avg",
            Op::MinMax => "minmax",
            Op::P2P => "p2p",
            Op::Derivative => "derivative",
            Op::Trend => "trend",
            Op::Peaks => "peaks",
            Op::Abnormal => "abnormal",
        }
    }
}

/// A parallel (time-seconds, value) series view. Times are monotonically
/// non-decreasing. Borrowed: windowing hands out sub-slices of the owner.
#[derive(Debug, Clone, Copy)]
pub struct Series<'a> {
    pub times: &'a [f64],
    pub values: &'a [f64],
}

impl<'a> Series<'a> {
    pub fn new(times: &'a [f64], values: &'a [f64]) -> Self {
        Series { times, values }
    }

    /// Sample count (min of the two lengths; malformed input counts as 0).
    pub fn len(&self) -> usize {
        self.times.len().min(self.values.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Owned series storage (decode results, downsample output).
#[derive(Debug, Clone, Default)]
pub struct SeriesData {
    pub times: Vec<f64>,
    pub values: Vec<f64>,
}

impl SeriesData {
    pub fn as_series(&self) -> Series<'_> {
        Series::new(&self.times, &self.values)
    }

    pub fn len(&self) -> usize {
        self.times.len().min(self.values.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
