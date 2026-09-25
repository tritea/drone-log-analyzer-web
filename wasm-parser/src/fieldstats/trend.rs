//! Least-squares trend statistics, ported from
//! `app/modules/fieldstats/trend.go`.

use super::stats::stats;
use super::Series;

/// Direction is judged "flat" when |slope| * duration (total regression
/// change) stays below 50% of the value range — noisy series get a non-zero
/// LSQ slope but no net trend, and the 50% tolerance keeps jitter from
/// being reported as one.
#[derive(Debug, Clone, Default)]
pub struct TrendStats {
    pub ok: bool,
    pub slope: f64,
    pub direction: &'static str, // rising / falling / flat
    pub first: f64,
    pub last: f64,
    pub change: f64, // last - first
    pub duration: f64,
}

/// Computes the trend. Non-finite samples are skipped; fewer than 2 valid
/// samples yields only the first/last values.
pub fn trend(s: Series<'_>) -> TrendStats {
    let mut tr = TrendStats::default();
    let mut n = 0usize;
    let mut first_t = 0.0f64;
    let (mut sum_t, mut sum_v, mut sum_tt, mut sum_tv) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for i in 0..s.len() {
        let (v, t) = (s.values[i], s.times[i]);
        if !v.is_finite() {
            continue;
        }
        if n == 0 {
            tr.first = v;
            first_t = t;
        }
        tr.last = v;
        tr.duration = t - first_t;
        n += 1;
        sum_t += t;
        sum_v += v;
        sum_tt += t * t;
        sum_tv += t * v;
    }
    if n == 0 {
        return tr;
    }
    tr.ok = true;
    tr.change = tr.last - tr.first;
    if n >= 2 {
        let den = n as f64 * sum_tt - sum_t * sum_t;
        if den != 0.0 {
            tr.slope = (n as f64 * sum_tv - sum_t * sum_v) / den;
        }
    }
    let st = stats(s);
    let total_change = tr.slope.abs() * tr.duration;
    if !st.ok || total_change < 0.5 * st.p2p {
        tr.direction = "flat";
    } else if tr.slope > 0.0 {
        tr.direction = "rising";
    } else {
        tr.direction = "falling";
    }
    tr
}

#[cfg(test)]
mod tests {
    use super::super::SeriesData;

    #[test]
    fn trend_rising() {
        let d = SeriesData {
            times: vec![0.0, 1.0, 2.0, 3.0],
            values: vec![0.0, 10.0, 20.0, 30.0],
        };
        let tr = super::trend(d.as_series());
        assert!(tr.ok);
        assert_eq!(tr.direction, "rising");
        assert!((tr.slope - 10.0).abs() < 1e-9);
        assert_eq!(tr.change, 30.0);
    }

    #[test]
    fn trend_flat_noise() {
        let d = SeriesData {
            times: vec![0.0, 1.0, 2.0, 3.0, 4.0],
            values: vec![0.0, 100.0, 0.0, 100.0, 0.0],
        };
        let tr = super::trend(d.as_series());
        assert_eq!(tr.direction, "flat");
    }

    #[test]
    fn trend_single_sample_no_slope() {
        let d = SeriesData {
            times: vec![5.0],
            values: vec![7.0],
        };
        let tr = super::trend(d.as_series());
        assert!(tr.ok);
        assert_eq!(tr.slope, 0.0);
        assert_eq!(tr.change, 0.0);
    }
}
