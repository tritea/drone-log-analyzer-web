//! Window slicing, ported from `app/modules/fieldstats/window.go`.

use super::Series;

/// Returns the sub-series within the `[t0, t1]` time window (closed
/// interval; t0>t1 is swapped; t1<=0 means open-ended to the end of the
/// series). Requires monotonically non-decreasing times.
pub fn slice<'a>(s: Series<'a>, t0: f64, t1: f64) -> Series<'a> {
    let n = s.len();
    if n == 0 {
        return Series::new(&[], &[]);
    }
    let (mut t0, mut t1) = (t0, t1);
    if t1 > 0.0 && t0 > t1 {
        std::mem::swap(&mut t0, &mut t1);
    }
    // Go sort.SearchFloat64s: first index with times[i] >= x.
    let times = &s.times[..n];
    let lo = times.partition_point(|&t| t < t0);
    let hi = if t1 > 0.0 {
        times.partition_point(|&t| t < t1 + 1e-9)
    } else {
        n
    };
    let (lo, hi) = if lo > hi { (hi, hi) } else { (lo, hi) };
    Series::new(&s.times[lo..hi], &s.values[lo..hi])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn series() -> super::super::SeriesData {
        super::super::SeriesData {
            times: vec![0.0, 1.0, 2.0, 3.0, 4.0],
            values: vec![10.0, 11.0, 12.0, 13.0, 14.0],
        }
    }

    #[test]
    fn slice_closed_interval() {
        let d = series();
        let w = slice(d.as_series(), 1.0, 3.0);
        assert_eq!(w.times, &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn slice_open_end_when_t1_nonpositive() {
        let d = series();
        let w = slice(d.as_series(), 2.0, 0.0);
        assert_eq!(w.times, &[2.0, 3.0, 4.0]);
    }

    #[test]
    fn slice_swaps_reversed_window() {
        let d = series();
        let w = slice(d.as_series(), 3.0, 1.0);
        assert_eq!(w.times, &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn slice_empty_input() {
        let w = slice(Series::new(&[], &[]), 0.0, 1.0);
        assert_eq!(w.len(), 0);
    }
}
