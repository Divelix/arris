//! The booleans at random poses (`docs/plans/m4-booleans.md` step 9): a
//! box and a cylinder from `prop::body`, both operand orders — volume
//! and area additivity, the cut identity, commutativity of `fuse` and
//! `common`, the designed refusal where the result would have two shells,
//! and every result clean at `Full` with nothing unchecked. A failure
//! prints the shrunk pair and the seed, and becomes a fixture under
//! `tests/fixtures/boolean/` (`tests/fixtures/README.md` §Property-test
//! failures).

use arris_debug::prop::body::OverlappingPair;
use arris_debug::{dump_text, prop};
use arris_ops::arris_check::arris_topo::{Body, Model, Provenance};
use arris_ops::arris_check::{Level, check};
use arris_ops::measure::{MassProperties, mass_properties};
use arris_ops::{OpError, Reason, common, cut, fuse};
use proptest::prelude::*;

/// The relative tolerance every identity holds to.
const REL: f64 = 1e-9;

/// A boolean of two bodies: `fuse`, `common` or `cut`.
type Boolean = fn(&mut Model, Body, Body) -> Result<(Body, Provenance), OpError>;

fn fail(what: impl core::fmt::Display) -> TestCaseError {
    TestCaseError::fail(what.to_string())
}

/// `|a − b| ≤ REL · max(|a|, |b|, floor)`.
fn close(a: f64, b: f64, floor: f64) -> bool {
    (a - b).abs() <= REL * a.abs().max(b.abs()).max(floor)
}

/// `op(a, b)`, its result clean at `Full` with nothing unchecked, and its
/// mass properties.
fn run(
    m: &mut Model,
    name: &str,
    op: Boolean,
    a: Body,
    b: Body,
) -> Result<(Body, MassProperties), TestCaseError> {
    let (body, _) = op(m, a, b).map_err(|e| fail(format!("{name}: {e}")))?;
    let report = check(m, body, Level::Full);
    if !report.is_ok() || !report.unchecked().is_empty() {
        return Err(fail(format!("{name}: not clean at Full\n{report}")));
    }
    let props = mass_properties(m, body).map_err(|e| fail(format!("{name}: measure: {e}")))?;
    Ok((body, props))
}

/// The operands of `pair` in a fresh model, with their mass properties.
fn operands(
    pair: &OverlappingPair,
) -> Result<(Model, Body, Body, MassProperties, MassProperties), TestCaseError> {
    let mut m = Model::default();
    let (a, b) = pair.build(&mut m).map_err(fail)?;
    let pa = mass_properties(&m, a).map_err(fail)?;
    let pb = mass_properties(&m, b).map_err(fail)?;
    Ok((m, a, b, pa, pb))
}

/// `V(A ∪ B) + V(A ∩ B) = V(A) + V(B)`, and the same for the areas: the
/// boundary of the union and the boundary of the common partition the
/// two operands' boundaries between them.
fn assert_additive(
    union: &MassProperties,
    inter: &MassProperties,
    pa: &MassProperties,
    pb: &MassProperties,
) -> Result<(), TestCaseError> {
    let (v, s) = (pa.volume + pb.volume, pa.area + pb.area);
    prop_assert!(
        close(union.volume + inter.volume, v, v),
        "V(A ∪ B) + V(A ∩ B) = {} + {}, V(A) + V(B) = {} + {}",
        union.volume,
        inter.volume,
        pa.volume,
        pb.volume
    );
    prop_assert!(
        close(union.area + inter.area, s, s),
        "A(A ∪ B) + A(A ∩ B) = {} + {}, A(A) + A(B) = {} + {}",
        union.area,
        inter.area,
        pa.area,
        pb.area
    );
    Ok(())
}

/// `V(A − B) + V(A ∩ B) = V(A)`.
fn assert_cut_identity(
    diff: &MassProperties,
    inter: &MassProperties,
    pa: &MassProperties,
    pb: &MassProperties,
) -> Result<(), TestCaseError> {
    prop_assert!(
        close(diff.volume + inter.volume, pa.volume, pa.volume + pb.volume),
        "V(A − B) + V(A ∩ B) = {} + {}, V(A) = {}",
        diff.volume,
        inter.volume,
        pa.volume
    );
    Ok(())
}

/// A cylinder that clears every edge of the box: every outcome is known
/// by construction. `fuse`, `common` and `box − cylinder` are one clean
/// shell each and obey the identities; `cylinder − box` is the designed
/// refusal, two shells by name (`⚠ OPEN` 2).
#[test]
fn piercing_pairs_obey_every_identity_and_refuse_the_two_shells() {
    prop::check(prop::body::piercing_pair(), |pair| {
        let (mut m, a, b, pa, pb) = operands(&pair)?;
        let (_, union) = run(&mut m, "fuse(a, b)", fuse, a, b)?;
        let (_, inter) = run(&mut m, "common(a, b)", common, a, b)?;
        let (_, diff) = run(&mut m, "cut(a, b)", cut, a, b)?;
        assert_additive(&union, &inter, &pa, &pb)?;
        assert_cut_identity(&diff, &inter, &pa, &pb)?;
        let before = dump_text(&m, b).map_err(fail)?;
        match cut(&mut m, b, a) {
            Err(OpError::Degenerate {
                reason: Reason::MultiShell { shells: 2 },
                ..
            }) => {}
            Ok(_) => return Err(fail("cut(b, a): the cylinder minus the box is two shells")),
            Err(e) => return Err(fail(format!("cut(b, a): {e}"))),
        }
        prop_assert_eq!(
            dump_text(&m, b).map_err(fail)?,
            before,
            "the model is as it was"
        );
        Ok(())
    });
}

/// Any overlapping pair, the wall free to cross the box's edges: `fuse`
/// and `common` are one clean shell each and additive; `box − cylinder`
/// either obeys the identity or is the two-shell refusal (a corner sliced
/// off), never anything else.
#[test]
fn overlapping_pairs_fuse_and_common_additively() {
    prop::check(prop::body::overlapping_pair(), |pair| {
        let (mut m, a, b, pa, pb) = operands(&pair)?;
        let (_, union) = run(&mut m, "fuse(a, b)", fuse, a, b)?;
        let (_, inter) = run(&mut m, "common(a, b)", common, a, b)?;
        assert_additive(&union, &inter, &pa, &pb)?;
        match cut(&mut m, a, b) {
            Ok((body, _)) => {
                let report = check(&m, body, Level::Full);
                prop_assert!(
                    report.is_ok() && report.unchecked().is_empty(),
                    "cut(a, b): not clean at Full\n{}",
                    report
                );
                let diff = mass_properties(&m, body).map_err(fail)?;
                assert_cut_identity(&diff, &inter, &pa, &pb)?;
            }
            Err(OpError::Degenerate {
                reason: Reason::MultiShell { .. },
                ..
            }) => {}
            Err(e) => return Err(fail(format!("cut(a, b): {e}"))),
        }
        Ok(())
    });
}

/// The dump with every number, and the sign in front of every id,
/// replaced by `#`, its lines sorted: two results that differ only in
/// which entity got which id, the order the faces were assembled in and
/// which way a section edge runs have the same text.
fn up_to_ids(dump: &str) -> Vec<String> {
    let mut lines: Vec<String> = dump
        .lines()
        .map(|line| {
            let mut out = String::with_capacity(line.len());
            let c: Vec<char> = line.chars().collect();
            let mut i = 0;
            while i < c.len() {
                let id_sign = (c[i] == '+' || c[i] == '-')
                    && c.get(i + 1).is_some_and(|x| "bsfevp".contains(*x))
                    && c.get(i + 2).is_some_and(char::is_ascii_digit);
                if id_sign {
                    i += 1;
                    continue;
                }
                let number = c[i].is_ascii_digit()
                    || (c[i] == '-' && c.get(i + 1).is_some_and(char::is_ascii_digit));
                if !number {
                    out.push(c[i]);
                    i += 1;
                    continue;
                }
                out.push('#');
                i += usize::from(c[i] == '-');
                while c.get(i).is_some_and(|x| x.is_ascii_digit() || *x == '.') {
                    i += 1;
                }
                if c.get(i) == Some(&'e') {
                    let mut j = i + 1;
                    j += usize::from(c.get(j).is_some_and(|x| *x == '-' || *x == '+'));
                    if c.get(j).is_some_and(char::is_ascii_digit) {
                        i = j;
                        while c.get(i).is_some_and(char::is_ascii_digit) {
                            i += 1;
                        }
                    }
                }
            }
            out
        })
        .collect();
    lines.sort();
    lines
}

/// Volume, area, centroid and inertia equal to `REL`.
fn assert_same_properties(
    x: &MassProperties,
    y: &MassProperties,
    what: &str,
) -> Result<(), TestCaseError> {
    prop_assert!(
        close(x.volume, y.volume, 1.0),
        "{what}: volumes {} and {}",
        x.volume,
        y.volume
    );
    prop_assert!(
        close(x.area, y.area, 1.0),
        "{what}: areas {} and {}",
        x.area,
        y.area
    );
    let scale = x.centroid.coords.abs().max().max(1.0);
    prop_assert!(
        (x.centroid - y.centroid).norm() <= REL * scale,
        "{what}: centroids {} and {}",
        x.centroid,
        y.centroid
    );
    let scale = x.inertia.abs().max().max(1.0);
    prop_assert!(
        (x.inertia - y.inertia).abs().max() <= REL * scale,
        "{what}: inertia {} and {}",
        x.inertia,
        y.inertia
    );
    Ok(())
}

/// `fuse(a, b)` and `fuse(b, a)`, `common(a, b)` and `common(b, a)`: the
/// same mass properties, the same counts, the same dump up to ids.
#[test]
fn fuse_and_common_commute_at_random_poses() {
    prop::check(prop::body::overlapping_pair(), |pair| {
        for (name, op) in [("fuse", fuse as Boolean), ("common", common as Boolean)] {
            let (mut m, a, b, _, _) = operands(&pair)?;
            let (ab, pab) = run(&mut m, &format!("{name}(a, b)"), op, a, b)?;
            let (ba, pba) = run(&mut m, &format!("{name}(b, a)"), op, b, a)?;
            assert_same_properties(&pab, &pba, name)?;
            let (da, db) = (
                dump_text(&m, ab).map_err(fail)?,
                dump_text(&m, ba).map_err(fail)?,
            );
            prop_assert_eq!(
                arris_debug::dump::euler_line(&m, ab).map_err(fail)?,
                arris_debug::dump::euler_line(&m, ba).map_err(fail)?,
                "{}: counts",
                name
            );
            prop_assert_eq!(
                up_to_ids(&da),
                up_to_ids(&db),
                "{}: dumps\n{}\n{}",
                name,
                da,
                db
            );
        }
        Ok(())
    });
}
