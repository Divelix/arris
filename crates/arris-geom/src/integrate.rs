//! Integrals over a region of a surface's (u, v) plane bounded by pcurve
//! pieces, by Green's theorem: `∬ f du dv = ∮ G dv` with `G(u, v) =
//! ∫_{u₀}^{u} f(s, v) ds`, the boundary integral taken along each piece
//! and both integrals by Gauss–Legendre quadrature. What the checker's
//! B2 row, `measure` and every mass property integrate with; the caller
//! supplies `f` — `|∂P/∂u × ∂P/∂v|` for an area, `P · (∂P/∂u × ∂P/∂v) / 3`
//! for a volume — over the surface's own parametrisation.

use core::f64::consts::{FRAC_PI_2, PI};

use crate::region2::Piece;

/// Points per Gauss–Legendre interval. Sixteen integrate a polynomial of
/// degree 31 exactly and a full turn of a trigonometric integrand (a
/// conic's `cos t`, `sin t` against a quadric's parametrisation) to below
/// 1e-20 relative, far under any model tolerance. What sixteen points do
/// not resolve is a kink or a knot inside one interval, which is why a
/// conic piece is split at quarter turns and a NURBS piece at its knots
/// before quadrature.
pub const GAUSS_ORDER: usize = 16;

/// Parameters per piece at which the region's least `u` is looked for, to
/// place the inner integral's origin `u₀` at the region's left edge so the
/// strip it integrates over stays inside the region's bounding box.
const ORIGIN_SAMPLES: usize = 8;

/// The nodes and weights of Gauss–Legendre quadrature of [`GAUSS_ORDER`]
/// on `[-1, 1]`, as `(node, weight)` pairs ascending in the node: the
/// roots of the Legendre polynomial by Newton from the Chebyshev guess,
/// iterated to a fixed point, so every platform computes the same values
/// to rounding.
pub fn gauss_legendre() -> [(f64, f64); GAUSS_ORDER] {
    let n = GAUSS_ORDER;
    let mut table = [(0.0, 0.0); GAUSS_ORDER];
    for (i, slot) in table.iter_mut().enumerate() {
        // The i-th root counted from the top; the guess interleaves them.
        let k = n - i;
        let mut x = (PI * (k as f64 - 0.25) / (n as f64 + 0.5)).cos();
        let mut derivative = 1.0;
        // Newton on P_n(x) with the three-term recurrence; the iteration
        // count bounds the loop, the fixed point ends it.
        for _ in 0..64 {
            let (mut p0, mut p1) = (1.0, x);
            for m in 2..=n {
                let p2 = ((2 * m - 1) as f64 * x * p1 - (m - 1) as f64 * p0) / m as f64;
                p0 = p1;
                p1 = p2;
            }
            derivative = n as f64 * (x * p1 - p0) / (x * x - 1.0);
            let step = p1 / derivative;
            let next = x - step;
            if next == x {
                break;
            }
            x = next;
        }
        *slot = (x, 2.0 / ((1.0 - x * x) * derivative * derivative));
    }
    table
}

/// `∫_a^b g(t) dt` by one Gauss–Legendre interval.
fn gauss(a: f64, b: f64, table: &[(f64, f64); GAUSS_ORDER], g: impl Fn(f64) -> f64) -> f64 {
    let half = (b - a) / 2.0;
    let mid = (a + b) / 2.0;
    table
        .iter()
        .map(|&(x, w)| w * g(mid + half * x))
        .sum::<f64>()
        * half
}

/// `∫_a^b g(t) dt` over sub-intervals split at `breaks` (ascending,
/// strictly inside `[a, b]`).
fn gauss_split(
    a: f64,
    b: f64,
    breaks: &[f64],
    table: &[(f64, f64); GAUSS_ORDER],
    g: impl Fn(f64) -> f64,
) -> f64 {
    let mut total = 0.0;
    let mut from = a;
    for &at in breaks {
        total += gauss(from, at, table, &g);
        from = at;
    }
    total + gauss(from, b, table, &g)
}

/// Where a piece's parameter range is cut before quadrature: a conic at
/// every quarter turn, a NURBS at its interior knots, a line nowhere.
fn breaks_of(piece: &Piece<'_>) -> Vec<f64> {
    match piece.curve {
        crate::Curve2::Line { .. } => Vec::new(),
        crate::Curve2::Circle { .. } | crate::Curve2::Ellipse { .. } => {
            let (lo, hi) = (piece.range.lo(), piece.range.hi());
            let mut breaks = Vec::new();
            let mut k = (lo / FRAC_PI_2).floor() + 1.0;
            while k * FRAC_PI_2 < hi {
                let at = k * FRAC_PI_2;
                if at > lo {
                    breaks.push(at);
                }
                k += 1.0;
            }
            breaks
        }
        crate::Curve2::Nurbs(_) => piece.interior_knots(),
    }
}

/// `∬_R f(u, v) du dv` over the region `R` bounded by `pieces` walked in
/// order — counter-clockwise for a positive result, so the result carries
/// the sign of the loop's turn and a face's hole loops subtract
/// themselves — by Green's theorem along the pieces with Gauss–Legendre
/// quadrature of [`GAUSS_ORDER`] points per interval, each piece split
/// at its quarter turns or knots. `f` is evaluated across the strip
/// between the region's least `u` and each boundary point, so it must be
/// defined over the region's bounding box in `u`. Exact to rounding for
/// `f` polynomial of degree below `2 · GAUSS_ORDER` in `u` along
/// polynomial pieces, and to well below any model tolerance for the
/// analytic surfaces and their conic pcurves. An empty `pieces` is zero.
///
/// ```
/// use arris_geom::integrate::region_integral;
/// use arris_geom::region2::Piece;
/// use arris_geom::Curve2;
/// use arris_math::{Frame2, Interval};
/// use core::f64::consts::PI;
///
/// let circle = Curve2::Circle { frame: Frame2::identity(), radius: 3.0 };
/// let area = region_integral(&[Piece::along(&circle, Interval::TURN)], |_, _| 1.0);
/// assert!((area - 9.0 * PI).abs() < 1e-12);
/// ```
pub fn region_integral(pieces: &[Piece<'_>], f: impl Fn(f64, f64) -> f64) -> f64 {
    let table = gauss_legendre();
    let u0 = pieces
        .iter()
        .flat_map(|piece| {
            (0..=ORIGIN_SAMPLES).map(move |i| {
                piece
                    .curve
                    .point(piece.range.lerp(i as f64 / ORIGIN_SAMPLES as f64))
                    .x
            })
        })
        .fold(f64::INFINITY, f64::min);
    if !u0.is_finite() {
        return 0.0;
    }
    let inner = |u: f64, v: f64| gauss(u0, u, &table, |s| f(s, v));
    let mut total = 0.0;
    for piece in pieces {
        let breaks = breaks_of(piece);
        let along = gauss_split(piece.range.lo(), piece.range.hi(), &breaks, &table, |t| {
            let e = piece.curve.eval(t);
            inner(e.point.x, e.point.y) * e.d1.y
        });
        total += if piece.reversed { -along } else { along };
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Curve2;
    use arris_math::{Frame2, Interval, Point2, UnitVec2, Vec2};

    #[test]
    fn nodes_are_symmetric_and_weights_sum_to_two() {
        let table = gauss_legendre();
        let sum: f64 = table.iter().map(|&(_, w)| w).sum();
        assert!((sum - 2.0).abs() < 1e-15);
        for i in 0..GAUSS_ORDER / 2 {
            let (x, w) = table[i];
            let (y, u) = table[GAUSS_ORDER - 1 - i];
            assert!((x + y).abs() < 1e-15 && (w - u).abs() < 1e-15);
        }
        assert!(table.windows(2).all(|p| p[0].0 < p[1].0));
        // Exact for x^30 over [-1, 1]: 2 / 31.
        let table = gauss_legendre();
        let v = gauss(-1.0, 1.0, &table, |x| x.powi(30));
        assert!((v - 2.0 / 31.0).abs() < 1e-15, "{v}");
    }

    #[test]
    fn a_rectangle_walked_either_way_integrates_a_polynomial() {
        let line = |ox: f64, oy: f64, dx: f64, dy: f64| Curve2::Line {
            origin: Point2::new(ox, oy),
            direction: UnitVec2::new_normalize(Vec2::new(dx, dy)),
        };
        let (w, h) = (4.0, 3.0);
        let sides = [
            line(0.0, 0.0, 1.0, 0.0),
            line(w, 0.0, 0.0, 1.0),
            line(w, h, -1.0, 0.0),
            line(0.0, h, 0.0, -1.0),
        ];
        let ranges = [w, h, w, h].map(|l| Interval::new(0.0, l).unwrap());
        let pieces: Vec<Piece<'_>> = sides
            .iter()
            .zip(ranges)
            .map(|(c, r)| Piece::along(c, r))
            .collect();
        // ∬ u² v du dv = w³/3 · h²/2.
        let v = region_integral(&pieces, |u, v| u * u * v);
        assert!((v - w.powi(3) / 3.0 * h * h / 2.0).abs() < 1e-12, "{v}");
        let reversed: Vec<Piece<'_>> = sides
            .iter()
            .zip(ranges)
            .rev()
            .map(|(c, r)| Piece::against(c, r))
            .collect();
        let v = region_integral(&reversed, |_, _| 1.0);
        assert!((v + w * h).abs() < 1e-12, "{v}");
    }

    #[test]
    fn a_circle_walked_clockwise_is_negative_and_quarter_turns_split() {
        let cw = Curve2::Circle {
            frame: Frame2::new(
                Point2::new(1.0, 2.0),
                Vec2::x(),
                arris_math::Handedness::Left,
            )
            .unwrap(),
            radius: 2.0,
        };
        let piece = Piece::along(&cw, Interval::TURN);
        assert_eq!(breaks_of(&piece).len(), 3);
        let v = region_integral(&[piece], |_, _| 1.0);
        assert!((v + 4.0 * PI).abs() < 1e-12, "{v}");
        let arc = Piece::along(&cw, Interval::new(0.3, 5.0).unwrap());
        assert_eq!(breaks_of(&arc), [FRAC_PI_2, PI, 3.0 * FRAC_PI_2]);
    }
}
