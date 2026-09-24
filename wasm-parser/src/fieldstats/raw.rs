//! LTTB downsampling, ported from `app/modules/fieldstats/raw.go`.

use super::{Series, SeriesData};

/// Downsamples the series to at most `max_points` points with LTTB
/// (Largest-Triangle-Three-Buckets): interior samples are bucketed evenly
/// and each bucket contributes the REAL sample maximizing the triangle
/// area with (last picked point, next bucket's average point). This keeps
/// the shape (closest polyline fit) and outliers (spikes create big
/// triangles and are picked naturally); outputs are original samples, not
/// synthesized values. Non-finite samples are filtered first;
/// max_points<3 is treated as 3 (first + 1 bucket + last); short inputs
/// pass through unchanged.
pub fn downsample(s: Series<'_>, max_points: usize) -> SeriesData {
    let n = s.len();
    if max_points == 0 || n <= max_points {
        return SeriesData {
            times: s.times[..n].to_vec(),
            values: s.values[..n].to_vec(),
        };
    }
    // Filter non-finite values (keeping timestamp pairing).
    let mut ts = Vec::with_capacity(n);
    let mut vs = Vec::with_capacity(n);
    for i in 0..n {
        let v = s.values[i];
        if v.is_finite() {
            ts.push(s.times[i]);
            vs.push(v);
        }
    }
    let m = vs.len();
    let max_points = max_points.max(3);
    if m <= max_points {
        return SeriesData {
            times: ts,
            values: vs,
        };
    }

    let mut out = SeriesData {
        times: Vec::with_capacity(max_points),
        values: Vec::with_capacity(max_points),
    };
    // First and last points are always kept; the interior m-2 samples are
    // spread over max_points-2 buckets.
    out.times.push(ts[0]);
    out.values.push(vs[0]);
    let buckets = max_points - 2;
    let per_bucket = (m - 2) as f64 / buckets as f64;
    let (mut prev_x, mut prev_y) = (ts[0], vs[0]);
    for b in 0..buckets {
        let start = 1 + (b as f64 * per_bucket).round() as usize;
        let mut end = 1 + ((b + 1) as f64 * per_bucket).round() as usize;
        end = end.min(m - 1);
        if start >= end {
            continue;
        }
        // Next bucket's average anchor (standard LTTB: compare against the
        // NEXT bucket, not this one).
        let n_start = end;
        let mut n_end = (1 + ((b + 2) as f64 * per_bucket).round() as usize).min(m);
        if n_end <= n_start {
            n_end = n_start + 1;
        }
        let (mut avg_x, mut avg_y) = (0.0f64, 0.0f64);
        for i in n_start..n_end {
            avg_x += ts[i];
            avg_y += vs[i];
        }
        avg_x /= (n_end - n_start) as f64;
        avg_y /= (n_end - n_start) as f64;

        let (mut best, mut best_area) = (start, -1.0f64);
        for i in start..end {
            let area =
                ((avg_x - prev_x) * (vs[i] - prev_y) - (avg_y - prev_y) * (ts[i] - prev_x)).abs();
            if area > best_area {
                best = i;
                best_area = area;
            }
        }
        out.times.push(ts[best]);
        out.values.push(vs[best]);
        prev_x = ts[best];
        prev_y = vs[best];
    }
    out.times.push(ts[m - 1]);
    out.values.push(vs[m - 1]);
    out
}

#[cfg(test)]
mod tests {
    use super::super::SeriesData;

    #[test]
    fn downsample_keeps_endpoints_and_count() {
        let n = 100;
        let d = SeriesData {
            times: (0..n).map(|i| i as f64).collect(),
            values: (0..n).map(|i| (i as f64 * 0.37).sin()).collect(),
        };
        let out = super::downsample(d.as_series(), 10);
        assert!(out.times.len() <= 10);
        assert_eq!(out.times.first(), Some(&0.0));
        assert_eq!(out.times.last(), Some(&(n as f64 - 1.0)));
    }

    #[test]
    fn downsample_short_input_passthrough() {
        let d = SeriesData {
            times: vec![0.0, 1.0, 2.0],
            values: vec![1.0, 2.0, 3.0],
        };
        let out = super::downsample(d.as_series(), 100);
        assert_eq!(out.values, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn downsample_filters_nonfinite() {
        let d = SeriesData {
            times: vec![0.0, 1.0, 2.0, 3.0],
            values: vec![1.0, f64::NAN, 3.0, f64::INFINITY],
        };
        let out = super::downsample(d.as_series(), 3);
        assert_eq!(out.values, vec![1.0, 3.0]);
    }
}
