//! Violation-segment merging, ported from
//! `app/modules/fieldstats/merge.go`.

use super::Segment;

/// Merges violation segments adjacent within `gap_secs`: a signal hovering
/// around its threshold makes the scanner emit thousands of single-sample
/// glitch segments that diagnostically belong to one violation storm. The
/// merged segment keeps the worse Worst/Extent (larger Extent wins,
/// regardless of lt/gt) and recomputes Duration from first/last times.
/// Input must be time-ascending (scanner output is).
pub fn merge_segments(segs: &[Segment], gap_secs: f64) -> Vec<Segment> {
    if segs.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(segs.len());
    let mut cur = segs[0];
    for s in &segs[1..] {
        if s.start - cur.end <= gap_secs {
            cur.end = s.end;
            if s.extent > cur.extent {
                cur.worst = s.worst;
                cur.extent = s.extent;
            }
            continue;
        }
        cur.duration = cur.end - cur.start;
        out.push(cur);
        cur = *s;
    }
    cur.duration = cur.end - cur.start;
    out.push(cur);
    out
}

#[cfg(test)]
mod tests {
    use super::super::Segment;

    fn seg(start: f64, end: f64, worst: f64, extent: f64) -> Segment {
        Segment {
            start,
            end,
            duration: end - start,
            worst,
            extent,
            ..Segment::default()
        }
    }

    #[test]
    fn merge_adjacent_keeps_worse() {
        let segs = [
            seg(0.0, 1.0, 6.0, 1.0),
            seg(1.4, 2.0, 9.0, 4.0),
            seg(9.0, 9.5, 7.0, 2.0),
        ];
        let merged = super::merge_segments(&segs, 1.0);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].start, 0.0);
        assert_eq!(merged[0].end, 2.0);
        assert_eq!(merged[0].worst, 9.0); // larger extent wins
        assert_eq!(merged[0].duration, 2.0);
    }

    #[test]
    fn merge_empty() {
        assert!(super::merge_segments(&[], 1.0).is_empty());
    }
}
