//! Basic window statistics, ported from `app/modules/fieldstats/stats.go`.

use super::Series;

/// Min/max (with hit times), average, peak-to-peak and RMS of a window.
/// `ok=false` means no finite sample; numeric fields are then meaningless.
#[derive(Debug, Clone, Default)]
pub struct BasicStats {
    pub ok: bool,
    pub count: usize,
    pub min: f64,
    pub min_at: f64,
    pub max: f64,
    pub max_at: f64,
    pub avg: f64,
    pub p2p: f64,
    /// Root-mean-square: the standard strength measure for oscillating
    /// signals (vibration/ripple).
    pub rms: f64,
    pub has_avg: bool,
}

/// Computes basic window statistics. Non-finite samples are skipped.
pub fn stats(s: Series<'_>) -> BasicStats {
    let mut st = BasicStats::default();
    let mut first = true;
    let (mut sum, mut sum_sq) = (0.0f64, 0.0f64);
    let mut cnt = 0usize;
    for i in 0..s.len() {
        let v = s.values[i];
        if !v.is_finite() {
            continue;
        }
        let t = s.times[i];
        if first || v < st.min {
            st.min = v;
            st.min_at = t;
        }
        if first || v > st.max {
            st.max = v;
            st.max_at = t;
        }
        first = false;
        sum += v;
        sum_sq += v * v;
        cnt += 1;
    }
    if first {
        return st;
    }
    st.ok = true;
    st.count = cnt;
    st.avg = sum / cnt as f64;
    st.has_avg = true;
    st.p2p = st.max - st.min;
    st.rms = (sum_sq / cnt as f64).sqrt();
    st
}

#[cfg(test)]
mod tests {
    use super::super::{slice, SeriesData};

    #[test]
    fn stats_basic() {
        let d = SeriesData {
            times: vec![0.0, 1.0, 2.0, 3.0],
            values: vec![3.0, -1.0, 4.0, f64::NAN],
        };
        let st = super::stats(d.as_series());
        assert!(st.ok);
        assert_eq!(st.count, 3);
        assert_eq!(st.min, -1.0);
        assert_eq!(st.min_at, 1.0);
        assert_eq!(st.max, 4.0);
        assert_eq!(st.max_at, 2.0);
        assert!((st.avg - 2.0).abs() < 1e-12);
        assert_eq!(st.p2p, 5.0);
        assert!((st.rms - (26.0f64 / 3.0).sqrt()).abs() < 1e-12);
    }

    #[test]
    fn stats_all_nonfinite_is_not_ok() {
        let d = SeriesData {
            times: vec![0.0, 1.0],
            values: vec![f64::NAN, f64::INFINITY],
        };
        let st = super::stats(d.as_series());
        assert!(!st.ok);
    }

    #[test]
    fn stats_windowed() {
        let d = SeriesData {
            times: vec![0.0, 1.0, 2.0, 3.0],
            values: vec![0.0, 5.0, 6.0, 100.0],
        };
        let st = super::stats(slice(d.as_series(), 1.0, 2.0));
        assert_eq!(st.min, 5.0);
        assert_eq!(st.max, 6.0);
    }
}
