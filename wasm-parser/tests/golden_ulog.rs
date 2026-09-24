//! Golden-parity test for the PX4 ULog fixture. See tests/golden_common/mod.rs
//! for the comparison rules.

mod golden_common;

use golden_common::{fixtures, run_golden};

#[test]
fn golden_px4_ulog() {
    let dir = fixtures().join("px4_ulog");
    let input = dir.join("px4_ulog.ulg");
    run_golden(&dir, &input);
}
