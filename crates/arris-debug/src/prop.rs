//! Property-test configuration and numeric strategies.
//!
//! Every property test in the workspace runs through [`check`]: a seeded
//! `proptest` runner whose case count comes from `ARRIS_PROPTEST_CASES`
//! (default 256) and whose seed comes from `ARRIS_PROPTEST_SEED` (default: a
//! fixed one, so CI is deterministic — `.agents/rules/kernel.md`: no
//! randomness outside property tests, and those are seeded). A failure
//! panics with the shrunk input and the seed that reproduces it.
//!
//! Regressions are not persisted by proptest: a failure becomes a fixture
//! under `tests/fixtures/` with the seed in its commit body
//! (`tests/fixtures/README.md` §Property-test failures).
//!
//! ```
//! use arris_debug::prop::{check, unit_vec3};
//! use proptest::prelude::*;
//!
//! check(unit_vec3(), |v| {
//!     let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
//!     prop_assert!((len - 1.0).abs() <= 1e-15);
//!     Ok(())
//! });
//! ```

use core::fmt::Debug;
use core::ops::RangeInclusive;

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestCaseError, TestError, TestRng, TestRunner};

/// The environment variable that sets the number of cases per property.
pub const CASES_VAR: &str = "ARRIS_PROPTEST_CASES";
/// The environment variable that sets the seed: 64 hex digits, as printed
/// by a failure.
pub const SEED_VAR: &str = "ARRIS_PROPTEST_SEED";
/// Cases per property when [`CASES_VAR`] is unset.
pub const DEFAULT_CASES: u32 = 256;
/// The seed when [`SEED_VAR`] is unset. Arbitrary and fixed.
pub const DEFAULT_SEED: [u8; 32] = *b"arris-property-tests-seed-v1\0\0\0\0";

/// The number of cases: [`CASES_VAR`], or [`DEFAULT_CASES`].
pub fn cases() -> u32 {
    cases_from(std::env::var(CASES_VAR).ok().as_deref())
}

fn cases_from(value: Option<&str>) -> u32 {
    value
        .and_then(|v| v.trim().parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_CASES)
}

/// The seed: [`SEED_VAR`] as 64 hex digits, or [`DEFAULT_SEED`]. A value
/// that does not parse is an error, not a silent fallback.
pub fn seed() -> [u8; 32] {
    match std::env::var(SEED_VAR) {
        Ok(v) => parse_seed(&v).unwrap_or_else(|| panic!("{SEED_VAR}={v:?} is not 64 hex digits")),
        Err(_) => DEFAULT_SEED,
    }
}

fn parse_seed(text: &str) -> Option<[u8; 32]> {
    let text = text.trim();
    if text.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

/// The seed as the hex string [`SEED_VAR`] accepts.
pub fn seed_hex(seed: &[u8; 32]) -> String {
    seed.iter().map(|b| format!("{b:02x}")).collect()
}

/// The proptest configuration: [`cases`] cases, no failure persistence,
/// no forking. For `proptest!` blocks that want it:
/// `#![proptest_config(arris_debug::prop::config())]` — but such a block
/// seeds itself from entropy; [`check`] is the seeded path.
pub fn config() -> Config {
    Config {
        cases: cases(),
        failure_persistence: None,
        max_shrink_iters: 4096,
        ..Config::default()
    }
}

/// A runner over [`config`] seeded with `seed`.
pub fn runner_with_seed(seed: &[u8; 32]) -> TestRunner {
    TestRunner::new_with_rng(config(), TestRng::from_seed(RngAlgorithm::ChaCha, seed))
}

/// Runs `test` over [`cases`] values of `strategy` from [`seed`]. On
/// failure, panics with the shrunk input and the `ARRIS_PROPTEST_SEED=…`
/// that reproduces the run.
pub fn check<S, F>(strategy: S, test: F)
where
    S: Strategy,
    S::Value: Debug,
    F: Fn(S::Value) -> Result<(), TestCaseError>,
{
    let seed = seed();
    if let Err(message) = try_check(&seed, strategy, test) {
        panic!("{message}");
    }
}

/// [`check`] with an explicit seed, returning the failure message instead
/// of panicking. What a test of the harness itself uses.
pub fn try_check<S, F>(seed: &[u8; 32], strategy: S, test: F) -> Result<(), String>
where
    S: Strategy,
    S::Value: Debug,
    F: Fn(S::Value) -> Result<(), TestCaseError>,
{
    let mut runner = runner_with_seed(seed);
    match runner.run(&strategy, test) {
        Ok(()) => Ok(()),
        Err(TestError::Fail(reason, value)) => Err(format!(
            "property failed: {reason}\nminimal failing input: {value:#?}\nreproduce with {SEED_VAR}={} {CASES_VAR}={}",
            seed_hex(seed),
            cases()
        )),
        Err(TestError::Abort(reason)) => Err(format!("property aborted: {reason}")),
    }
}

/// Finite `f64` values in `range`, both ends included; never NaN or
/// infinite. Shrinks toward zero (or the range end nearest it).
pub fn finite_f64(range: RangeInclusive<f64>) -> impl Strategy<Value = f64> {
    let (lo, hi) = (*range.start(), *range.end());
    assert!(
        lo.is_finite() && hi.is_finite() && lo <= hi,
        "finite_f64 needs a finite, ordered range"
    );
    (lo..=hi).prop_filter("finite", |v| v.is_finite())
}

/// Unit vectors uniformly distributed on the sphere, as `[x, y, z]`, with
/// length within 1e-15 of one. Uses the area-preserving map from
/// `(z, θ)` uniform in `[−1, 1] × [0, 2π)`, then normalises.
pub fn unit_vec3() -> impl Strategy<Value = [f64; 3]> {
    (
        finite_f64(-1.0..=1.0),
        finite_f64(0.0..=core::f64::consts::TAU),
    )
        .prop_map(|(z, theta)| {
            let r = (1.0 - z * z).max(0.0).sqrt();
            normalize([r * theta.cos(), r * theta.sin(), z])
        })
}

/// Rotations as unit quaternions `[w, x, y, z]`, uniformly distributed over
/// SO(3) (Shoemake's subgroup algorithm over three uniforms), with norm
/// within 1e-15 of one.
pub fn rotation() -> impl Strategy<Value = [f64; 4]> {
    (
        finite_f64(0.0..=1.0),
        finite_f64(0.0..=1.0),
        finite_f64(0.0..=1.0),
    )
        .prop_map(|(u1, u2, u3)| {
            let (a, b) = ((1.0 - u1).sqrt(), u1.sqrt());
            let (t2, t3) = (core::f64::consts::TAU * u2, core::f64::consts::TAU * u3);
            let q = [b * t3.cos(), a * t2.sin(), a * t2.cos(), b * t3.sin()];
            let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
            if n > 0.0 {
                [q[0] / n, q[1] / n, q[2] / n, q[3] / n]
            } else {
                [1.0, 0.0, 0.0, 0.0]
            }
        })
}

fn normalize(v: [f64; 3]) -> [f64; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n > 0.0 {
        [v[0] / n, v[1] / n, v[2] / n]
    } else {
        [0.0, 0.0, 1.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::strategy::ValueTree;

    #[test]
    fn unit_vec3_has_unit_length() {
        check(unit_vec3(), |v| {
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            prop_assert!((len - 1.0).abs() <= 1e-15, "length {len}");
            Ok(())
        });
    }

    #[test]
    fn rotation_is_a_unit_quaternion() {
        check(rotation(), |q| {
            let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
            prop_assert!((n - 1.0).abs() <= 1e-15, "norm {n}");
            Ok(())
        });
    }

    #[test]
    fn finite_f64_stays_in_range_and_finite() {
        check(finite_f64(-1e3..=1e3), |v| {
            prop_assert!(v.is_finite() && (-1e3..=1e3).contains(&v));
            Ok(())
        });
    }

    #[test]
    fn unit_vec3_covers_every_octant() {
        // Uniform on the sphere: over the configured cases every octant is
        // hit. A strategy that always returned the same vector would pass
        // the length test and fail here.
        let mut runner = runner_with_seed(&DEFAULT_SEED);
        let mut octants = std::collections::BTreeSet::new();
        for _ in 0..cases().max(64) {
            let v = unit_vec3().new_tree(&mut runner).unwrap().current();
            octants.insert((v[0] > 0.0, v[1] > 0.0, v[2] > 0.0));
        }
        assert_eq!(octants.len(), 8);
    }

    #[test]
    fn a_failure_prints_a_seed_that_reproduces_it() {
        let failing = |v: f64| {
            prop_assert!(v < 0.5, "too large");
            Ok(())
        };
        let seed = DEFAULT_SEED;
        let message = try_check(&seed, finite_f64(0.0..=1.0), failing).unwrap_err();
        assert!(message.contains("too large"), "{message}");
        let printed = message
            .lines()
            .find_map(|l| l.strip_prefix("reproduce with ARRIS_PROPTEST_SEED="))
            .and_then(|rest| rest.split_whitespace().next())
            .expect("the message names the seed");
        let reparsed = parse_seed(printed).expect("the printed seed parses");
        assert_eq!(reparsed, seed);
        let again = try_check(&reparsed, finite_f64(0.0..=1.0), failing).unwrap_err();
        assert_eq!(message, again, "same seed, same shrunk input, same message");
        let other = try_check(&[7u8; 32], finite_f64(0.0..=1.0), failing).unwrap_err();
        assert!(other.contains("minimal failing input"));
    }

    #[test]
    fn cases_and_seed_parse_or_fall_back() {
        assert_eq!(cases_from(None), DEFAULT_CASES);
        assert_eq!(cases_from(Some("1000")), 1000);
        assert_eq!(cases_from(Some("0")), DEFAULT_CASES);
        assert_eq!(cases_from(Some("lots")), DEFAULT_CASES);
        assert_eq!(parse_seed(&seed_hex(&DEFAULT_SEED)), Some(DEFAULT_SEED));
        assert_eq!(parse_seed("abc"), None);
        assert_eq!(parse_seed(&"zz".repeat(32)), None);
    }
}
