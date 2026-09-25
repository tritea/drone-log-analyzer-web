//! Rate-of-change statistics, ported from
//! `app/modules/fieldstats/derivative.go`.

use super::Series;

/// Rates are per-second changes. `max_rate` is the largest absolute
/// adjacent-sample rate; `avg_rate` is the endpoint slope
/// (last-first)/(tN-t0), reflecting net change.
#[derive(Debug, Clone, Default)]
pub struct DerivativeStats {
    pub ok: bool,
    pub max_rate: f64,
    pub max_rate_at: f64,
    pub avg_rate: f64,
    pub has_avg: bool,
    pub samples: usize,
}

/// Computes rate statistics. Skips non-finite samples and adjacent pairs
/// with repeated/backward timestamps (dt<=0).
pub fn derivative(s: Series<'_>) -> DerivativeStats {
    let mut d = DerivativeStats::default();
    let (mut prev_v, mut prev_t) = (0.0f64, 0.0f64);
    let mut has_prev = false;
    let (mut first_v, mut first_t, mut last_v, mut last_t) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut has_first = false;
    for i in 0..s.len() {
        let (v, t) = (s.values[i], s.times[i]);
        if !v.is_finite() {
            continue;
        }
        if !has_first {
            first_v = v;
            first_t = t;
            has_first = true;
        }
        last_v = v;
        last_t = t;
        if has_prev {
            let dt = t - prev_t;
            if dt > 0.0 {
                let rate = (v - prev_v).abs() / dt;
                if !d.ok || rate > d.max_rate {
                    d.max_rate = rate;
                    d.max_rate_at = t;
                }
                d.ok = true;
                d.samples += 1;
            }
        }
        prev_v = v;
        prev_t = t;
        has_prev = true;
    }
    if has_first && last_t > first_t {
        d.avg_rate = (last_v - first_v) / (last_t - first_t);
        d.has_avg = true;
    }
    d
}

#[cfg(test)]
mod tests {
    use super::super::SeriesData;

    #[test]
    fn derivative_rates() {
        let d = SeriesData {
            times: vec![0.0, 1.0, 2.0],
            values: vec![0.0, 5.0, 3.0],
        };
        let d_st = super::derivative(d.as_series());
        assert!(d_st.ok);
        assert_eq!(d_st.max_rate, 5.0);
        assert_eq!(d_st.max_rate_at, 1.0);
        assert_eq!(d_st.avg_rate, 1.5);
        assert_eq!(d_st.samples, 2);
    }

    #[test]
    fn derivative_skips_repeated_timestamps() {
        let d = SeriesData {
            times: vec![1.0, 1.0, 2.0],
            values: vec![0.0, 9.0, 9.0],
        };
        let d_st = super::derivative(d.as_series());
        // dt=0 pair skipped; only the (1.0→2.0) pair counts (rate 0).
        assert_eq!(d_st.samples, 1);
        assert_eq!(d_st.max_rate, 0.0);
    }
}
