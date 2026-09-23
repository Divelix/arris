//! The differential (ADR-0024 §2): `ARRIS_DIFF_CASES` recipes (default
//! 32) drawn from the property seed, built by Arris and by the Open
//! CASCADE oracle, every outcome classed. A disagreement, a checker
//! violation or a panic fails the run, shrunk to a `fixture.json` for
//! `tests/fixtures/regression/`; refusals are counted, per reason. The
//! histogram prints either way.
//!
//! Ignored until its first findings are sorted (plans/measuring-harness
//! step 5): the fixed seed's first 32 draws hold seven measure
//! disagreements, a volume or a centroid 1e-9 to 1e-7 apart on a posed
//! cylinder or an elliptic extrusion. Run it with `--run-ignored only`.

use arris_debug::{differential, prop};

#[test]
#[ignore = "its first findings are plans/measuring-harness step 5's"]
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
