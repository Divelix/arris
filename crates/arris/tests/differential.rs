//! The differential (ADR-0024 §2): `ARRIS_DIFF_CASES` recipes (default
//! 32) drawn from the property seed, built by Arris and by the Open
//! CASCADE oracle, every outcome classed. A disagreement, a checker
//! violation, a panic or an internal fault fails the run, shrunk to a `fixture.json` for
//! `tests/fixtures/regression/`; refusals are counted, per reason. The
//! histogram prints either way.
//!
//! A failure a named exclusion covers is counted under its name
//! (`differential::EXCLUSIONS`), each citing the regression fixtures that
//! pin it. CI runs 1000 recipes, and the hook 32.

use arris_debug::{differential, prop};

#[test]
fn arris_and_the_oracle_agree_on_drawn_recipes() {
    let run = differential::run(&prop::seed(), differential::cases())
        .unwrap_or_else(|e| panic!("the differential could not run: {e}"));
    println!("{}", run.report());
    assert!(
        run.failures.is_empty(),
        "{}\n{} failing recipes:\n\n{}",
        run.report(),
        run.failures.len(),
        run.failures_text()
    );
}
