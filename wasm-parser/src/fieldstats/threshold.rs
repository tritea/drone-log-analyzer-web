//! Threshold-violation scanning, ported from
//! `app/modules/fieldstats/threshold.go`.

use super::Series;

/// A violation condition. `op` is one of lt/le/gt/ge (matching the
/// knowledge-base threshold definitions).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cond {
    pub op: &'static str,
    pub value: f64,
}

impl Cond {
    pub fn new(op: &'static str, value: f64) -> Cond {
        Cond { op, value }
    }

    /// Whether v satisfies the condition.
    pub fn matches(&self, v: f64) -> bool {
        match self.op {
            "lt" => v < self.value,
            "le" => v <= self.value,
            "gt" => v > self.value,
            "ge" => v >= self.value,
            _ => false,
        }
    }

    /// Whether `op` is supported.
    pub fn valid_op(&self) -> bool {
        matches!(self.op, "lt" | "le" | "gt" | "ge")
    }
}

/// One contiguous violation interval. `worst` is the most severe value in
/// the interval (smallest for lt-class, largest for gt-class); `extent` is
/// how far `worst` exceeds the threshold.
#[derive(Debug, Clone, Copy, Default)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub duration: f64,
    pub worst: f64,
    pub extent: f64,
}

/// Scans the series and returns the contiguous intervals matching `cond`
/// (a gap between adjacent valid samples closes a segment; boundaries are
/// the first/last hit sample times). Empty output means no violation.
pub fn abnormal(s: Series<'_>, cond: Cond) -> Vec<Segment> {
    if !cond.valid_op() {
        return Vec::new();
    }
    let mut segs = Vec::new();
    let mut in_seg = false;
    let mut seg = Segment::default();
    for i in 0..s.len() {
        let v = s.values[i];
        if !v.is_finite() {
            continue;
        }
        let hit = cond.matches(v);
        let t = s.times[i];
        if hit && !in_seg {
            seg = Segment {
                start: t,
                end: t,
                worst: v,
                ..Segment::default()
            };
            in_seg = true;
        } else if hit {
            seg.end = t;
            if worse(v, seg.worst, cond) {
                seg.worst = v;
            }
        } else if in_seg {
            seg.duration = seg.end - seg.start;
            seg.extent = (seg.worst - cond.value).abs();
            segs.push(seg);
            in_seg = false;
        }
    }
    if in_seg {
        seg.duration = seg.end - seg.start;
        seg.extent = (seg.worst - cond.value).abs();
        segs.push(seg);
    }
    segs
}

/// Whether v is "closer to danger" than cur: smaller is worse for lt-class,
/// larger for gt-class.
fn worse(v: f64, cur: f64, cond: Cond) -> bool {
    if cond.op == "lt" || cond.op == "le" {
        v < cur
    } else {
        v > cur
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Cond, SeriesData};

    #[test]
    fn abnormal_segments() {
        let d = SeriesData {
            times: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
            values: vec![0.0, 5.0, 6.0, 1.0, 7.0, 0.0],
        };
        let segs = super::abnormal(d.as_series(), Cond::new("gt", 4.0));
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].start, 1.0);
        assert_eq!(segs[0].end, 2.0);
        assert_eq!(segs[0].worst, 6.0);
        assert_eq!(segs[0].extent, 2.0);
        assert_eq!(segs[1].start, 4.0);
        assert_eq!(segs[1].worst, 7.0);
    }

    #[test]
    fn abnormal_trailing_segment_closes() {
        let d = SeriesData {
            times: vec![0.0, 1.0, 2.0],
            values: vec![1.0, 9.0, 9.0],
        };
        let segs = super::abnormal(d.as_series(), Cond::new("gt", 5.0));
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].end, 2.0);
        assert_eq!(segs[0].duration, 1.0);
    }

    #[test]
    fn abnormal_invalid_op_is_empty() {
        let d = SeriesData {
            times: vec![0.0],
            values: vec![100.0],
        };
        assert!(super::abnormal(d.as_series(), Cond::new("xx", 1.0)).is_empty());
    }
}
