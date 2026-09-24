//! Local-extremum (peak) detection, ported from
//! `app/modules/fieldstats/peaks.go`.

use super::stats::stats;
use super::Series;

#[derive(Debug, Clone, Default)]
pub struct PeakStats {
    pub ok: bool,
    pub count: usize,
    pub max_peak: f64,
    pub max_peak_at: f64,
    /// Detection threshold: defaults to 5% of the window peak-to-peak
    /// (1e-12 minimum, so constant series never misreport).
    pub prominence: f64,
}

/// Detects local extrema (maxima AND minima both count). A "peak" is a
/// local extremum standing out from its adjacent plateau shoulder by at
/// least `prominence`; `max_peak` is the largest maximum.
pub fn peaks(s: Series<'_>, prominence: f64) -> PeakStats {
    let mut ps = PeakStats::default();
    let st = stats(s);
    if !st.ok {
        return ps;
    }
    ps.ok = true;
    let prominence = if prominence <= 0.0 {
        (0.05 * st.p2p).max(1e-12)
    } else {
        prominence
    };
    ps.prominence = prominence;

    // Collect finite-sample indices first, then find extrema on the
    // denoised sequence.
    let idx: Vec<usize> = (0..s.len()).filter(|&i| s.values[i].is_finite()).collect();
    for k in 1..idx.len().saturating_sub(1) {
        let (i, ip, in_) = (idx[k], idx[k - 1], idx[k + 1]);
        let (v, vp, vn) = (s.values[i], s.values[ip], s.values[in_]);
        let is_max = v > vp && v >= vn;
        let is_min = v < vp && v <= vn;
        if !is_max && !is_min {
            continue;
        }
        if (v - vp).abs() >= prominence {
            ps.count += 1;
            if is_max && v > ps.max_peak {
                ps.max_peak = v;
                ps.max_peak_at = s.times[i];
            }
        }
    }
    if ps.max_peak == 0.0 && st.max != 0.0 {
        // No sample judged a peak: fall back to the window maximum.
        ps.max_peak = st.max;
        ps.max_peak_at = st.max_at;
    }
    ps
}

#[cfg(test)]
mod tests {
    use super::super::SeriesData;

    #[test]
    fn peaks_counts_maxima_and_minima() {
        let d = SeriesData {
            times: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
            values: vec![0.0, 10.0, 1.0, 9.0, 0.0, 5.0],
        };
        let ps = super::peaks(d.as_series(), 0.0);
        assert!(ps.ok);
        // maxima at t=1 (10) and t=3 (9), minima at t=2 (1) and t=4 (0).
        assert_eq!(ps.count, 4);
        assert_eq!(ps.max_peak, 10.0);
        assert_eq!(ps.max_peak_at, 1.0);
        assert!((ps.prominence - 0.5).abs() < 1e-12); // 5% of p2p=10
    }

    #[test]
    fn peaks_constant_series_falls_back_to_max() {
        let d = SeriesData {
            times: vec![0.0, 1.0, 2.0],
            values: vec![7.0, 7.0, 7.0],
        };
        let ps = super::peaks(d.as_series(), 0.0);
        assert_eq!(ps.count, 0);
        assert_eq!(ps.max_peak, 7.0); // fallback to window maximum
    }
}
