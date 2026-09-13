//! The exact predicates agree with an exact integer determinant on every
//! integer-grid case, degenerate ones included.

use arris_debug::prop::check;
use arris_math::Point2;
use arris_math::predicates::{Sign, incircle, orient2d};
use proptest::prelude::*;

/// Grid coordinates up to ±2²⁰ (with the collinear multiplier, ±2²²):
/// exact in `f64`, and the degree-4 in-circle determinant stays far
/// inside `i128`.
const GRID: i64 = 1 << 20;

fn sign_of(det: i128) -> Sign {
    match det.signum() {
        -1 => Sign::Negative,
        0 => Sign::Zero,
        _ => Sign::Positive,
    }
}

fn exact_orient2d([ax, ay]: [i64; 2], [bx, by]: [i64; 2], [cx, cy]: [i64; 2]) -> Sign {
    let (ax, ay, bx, by, cx, cy) = (
        i128::from(ax),
        i128::from(ay),
        i128::from(bx),
        i128::from(by),
        i128::from(cx),
        i128::from(cy),
    );
    sign_of((bx - ax) * (cy - ay) - (by - ay) * (cx - ax))
}

fn exact_incircle(a: [i64; 2], b: [i64; 2], c: [i64; 2], d: [i64; 2]) -> Sign {
    let rel = |p: [i64; 2]| [i128::from(p[0] - d[0]), i128::from(p[1] - d[1])];
    let ([adx, ady], [bdx, bdy], [cdx, cdy]) = (rel(a), rel(b), rel(c));
    let (alift, blift, clift) = (
        adx * adx + ady * ady,
        bdx * bdx + bdy * bdy,
        cdx * cdx + cdy * cdy,
    );
    sign_of(
        alift * (bdx * cdy - cdx * bdy)
            + blift * (cdx * ady - adx * cdy)
            + clift * (adx * bdy - bdx * ady),
    )
}

fn pt(p: [i64; 2]) -> Point2 {
    Point2::new(p[0] as f64, p[1] as f64)
}

fn grid_point() -> impl Strategy<Value = [i64; 2]> {
    [-GRID..=GRID, -GRID..=GRID]
}

/// Three grid points: random, or `c` on the line `a b` at an integer
/// multiple so the exact answer is zero.
fn triple() -> impl Strategy<Value = [[i64; 2]; 3]> {
    prop_oneof![
        (grid_point(), grid_point(), grid_point()).prop_map(|(a, b, c)| [a, b, c]),
        (grid_point(), grid_point(), -3i64..=3)
            .prop_map(|(a, b, k)| { [a, b, [a[0] + k * (b[0] - a[0]), a[1] + k * (b[1] - a[1])]] }),
    ]
}

/// Four grid points: random, or four of the twelve grid points on a circle
/// of radius `5k` about a grid centre (the axis points and the 3-4-5
/// points), in a random order, so the exact answer is zero.
fn quad() -> impl Strategy<Value = [[i64; 2]; 4]> {
    let cocircular =
        (grid_point(), 1i64..=(GRID / 8), any::<[u8; 4]>()).prop_map(|(c, k, picks)| {
            let on_circle = [
                [5, 0],
                [-5, 0],
                [0, 5],
                [0, -5],
                [3, 4],
                [3, -4],
                [-3, 4],
                [-3, -4],
                [4, 3],
                [4, -3],
                [-4, 3],
                [-4, -3],
            ];
            // Four distinct indices from the picks, in the picks' order.
            let mut chosen: Vec<usize> = Vec::new();
            let mut i = 0;
            while chosen.len() < 4 {
                let candidate = (usize::from(picks[chosen.len()]) + i) % on_circle.len();
                if !chosen.contains(&candidate) {
                    chosen.push(candidate);
                }
                i += 1;
            }
            let mut out = [[0i64; 2]; 4];
            for (slot, idx) in out.iter_mut().zip(chosen) {
                let [dx, dy] = on_circle[idx];
                *slot = [c[0] + dx * k, c[1] + dy * k];
            }
            out
        });
    prop_oneof![
        (grid_point(), grid_point(), grid_point(), grid_point())
            .prop_map(|(a, b, c, d)| [a, b, c, d]),
        cocircular,
    ]
}

#[test]
fn orient2d_matches_the_exact_determinant() {
    check(triple(), |[a, b, c]| {
        prop_assert_eq!(orient2d(pt(a), pt(b), pt(c)), exact_orient2d(a, b, c));
        // Antisymmetric in any swap.
        prop_assert_eq!(
            orient2d(pt(b), pt(a), pt(c)),
            exact_orient2d(a, b, c).flipped()
        );
        Ok(())
    });
}

#[test]
fn incircle_matches_the_exact_determinant() {
    check(quad(), |[a, b, c, d]| {
        prop_assert_eq!(
            incircle(pt(a), pt(b), pt(c), pt(d)),
            exact_incircle(a, b, c, d)
        );
        Ok(())
    });
}

#[test]
fn degenerate_cases_are_reported_as_zero() {
    // Every collinear triple and every cocircular quad the strategies build
    // reads as zero; the property above proves the sign, this proves the
    // strategies actually produce the degenerate cases.
    let a = [1, 2];
    let b = [4, 8];
    assert_eq!(orient2d(pt(a), pt(b), pt([7, 14])), Sign::Zero);
    let sq = [[1, 0], [0, 1], [-1, 0], [0, -1]];
    assert_eq!(
        incircle(pt(sq[0]), pt(sq[1]), pt(sq[2]), pt(sq[3])),
        Sign::Zero
    );
    assert_eq!(
        incircle(pt(sq[0]), pt(sq[1]), pt(sq[2]), pt([0, 0])),
        Sign::Positive
    );
    assert_eq!(
        incircle(pt(sq[0]), pt(sq[1]), pt(sq[2]), pt([2, 0])),
        Sign::Negative
    );
    assert_eq!(Sign::of(f64::NAN), Sign::Zero);
    assert_eq!(Sign::of(-0.0), Sign::Zero);
}
