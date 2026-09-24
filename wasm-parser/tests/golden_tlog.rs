//! Golden-parity test for the MAVLink tlog fixture. See
//! tests/golden_common/mod.rs for the comparison rules.

mod golden_common;

use golden_common::{fixtures, run_golden};

#[test]
fn golden_apm_tlog() {
    let dir = fixtures().join("apm_tlog");
    let input = dir.join("apm_tlog.tlog");
    run_golden(&dir, &input);
}
