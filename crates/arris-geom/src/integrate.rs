//! Integrals over a region of a surface's (u, v) plane bounded by pcurve
//! pieces, by Green's theorem: `∬ f du dv = ∮ G dv` with `G(u, v) =
//! ∫_{u₀}^{u} f(s, v) ds`, the boundary integral taken along each piece
//! and both integrals by Gauss–Legendre quadrature. What the checker's
//! B2 row, `measure` and every mass property integrate with; the caller
//! supplies `f` — `|∂P/∂u × ∂P/∂v|` for an area, `P · (∂P/∂u × ∂P/∂v) / 3`
//! for a volume — over the surface's own parametrisation.

use core::f64::consts::{FRAC_PI_2, FRAC_PI_8, PI};

use crate::Surface;
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

/// The most sub-intervals the inner integral is ever split into,
/// whatever step the caller asks for. A quarter-period step over a whole
/// period is four; the cap is there so a step of zero, or one far below
/// the region's own scale, costs bounded time instead of hanging.
pub const MAX_INNER_INTERVALS: usize = 256;

/// The longest interval in `u` the inner integral of [`region_integral`]
/// should be taken over on `surface`: a quarter of the surface's period,
/// where it turns in `u`, so that no Gauss–Legendre interval spans more
/// than a quarter of an oscillation of the integrand — the same cadence
/// [`region_integral`] already splits a conic boundary piece at. A plane
/// is affine in (u, v) and every integrand this module is used with is a
/// low-degree polynomial along it, which the quadrature is exact for, so
/// it asks for no split at all; a NURBS surface asks for its smallest
/// knot span in `u`, the scale on which its polynomial pieces change. An
/// elliptic cylinder asks for a sixteenth of a turn: its area element
/// `√(a² sin²u + b² cos²u)` is no trigonometric polynomial, and has
/// complex singularities `atanh(b / a)` off the real axis at `u = 0` and
/// `π`, which the quadrature resolves only when they sit at an interval's
/// end (the grid [`region_integral`] aligns to) and the interval is short
/// against that distance — at an aspect of eighteen, a quarter turn
/// leaves `1e-7` of the area, a sixteenth `1e-11`.
///
/// ```
/// use arris_geom::Surface;
/// use arris_geom::integrate::inner_step;
/// use arris_math::Frame;
/// use core::f64::consts::FRAC_PI_2;
///
/// let cylinder = Surface::Cylinder { frame: Frame::world(), radius: 2.0 };
/// assert_eq!(inner_step(&cylinder), FRAC_PI_2);
/// assert_eq!(inner_step(&Surface::Plane { frame: Frame::world() }), f64::INFINITY);
/// ```
pub fn inner_step(surface: &Surface) -> f64 {
    match surface {
        Surface::Plane { .. } => f64::INFINITY,
        Surface::Cylinder { .. }
        | Surface::Cone { .. }
        | Surface::Sphere { .. }
        | Surface::Torus { .. } => FRAC_PI_2,
        Surface::EllipticCylinder { .. } => FRAC_PI_8,
        Surface::Nurbs(s) => s.knots()[0]
            .windows(2)
            .map(|w| w[1] - w[0])
            .filter(|&d| d > 0.0)
            .fold(f64::INFINITY, f64::min),
    }
}

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

/// How many equal sub-intervals a span of `length` is integrated in at a
/// step of `step`: at least one, at most [`MAX_INNER_INTERVALS`].
fn intervals(length: f64, step: f64) -> usize {
    let wanted = (length.abs() / step).ceil();
    // A zero span, a NaN step, or a step above the span: one interval.
    if wanted.is_nan() || wanted < 1.0 {
        return 1;
    }
    if wanted >= MAX_INNER_INTERVALS as f64 {
        return MAX_INNER_INTERVALS;
    }
    wanted as usize
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
/// defined over the region's bounding box in `u`. The inner integral is
/// itself split at every whole multiple of `inner_step` it crosses — the
/// grid the surface's own turns and quarter turns lie on, so an
/// integrand's feature there is an interval's end and never its middle
/// — at most [`MAX_INNER_INTERVALS`] intervals, beyond which it falls
/// back to that many equal ones: `f64::INFINITY` takes it in one, which
/// is exact for an `f` polynomial in `u` of degree below `2 ·
/// GAUSS_ORDER`, and [`inner_step`] of the surface `f` evaluates is what
/// a caller integrating over a periodic surface passes, since a strip
/// that spans a whole turn of `cos u` is not one interval's work. An
/// empty `pieces` is zero.
///
/// ```
/// use arris_geom::integrate::region_integral;
/// use arris_geom::region2::Piece;
/// use arris_geom::Curve2;
/// use arris_math::{Frame2, Interval};
/// use core::f64::consts::PI;
///
/// let circle = Curve2::Circle { frame: Frame2::identity(), radius: 3.0 };
/// let area = region_integral(
///     &[Piece::along(&circle, Interval::TURN)],
///     f64::INFINITY,
///     |_, _| 1.0,
/// );
/// assert!((area - 9.0 * PI).abs() < 1e-12);
/// ```
pub fn region_integral(pieces: &[Piece<'_>], inner_step: f64, f: impl Fn(f64, f64) -> f64) -> f64 {
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
    let inner = |u: f64, v: f64| {
        let g = |s: f64| f(s, v);
        let (lo, hi) = (u0.min(u), u0.max(u));
        let sign = if u >= u0 { 1.0 } else { -1.0 };
        // The grid multiples strictly inside `(lo, hi)`.
        let (first, last) = (
            (lo / inner_step).floor() + 1.0,
            (hi / inner_step).ceil() - 1.0,
        );
        let count = last - first + 1.0;
        if !(count.is_finite() && count >= 1.0) || count + 1.0 > MAX_INNER_INTERVALS as f64 {
            let n = intervals(hi - lo, inner_step);
            let step = (hi - lo) / n as f64;
            return sign
                * (0..n)
                    .map(|i| {
                        let a = lo + i as f64 * step;
                        gauss(a, a + step, &table, g)
                    })
                    .sum::<f64>();
        }
        let mut total = 0.0;
        let mut from = lo;
        let mut k = first;
        while k <= last {
            let at = k * inner_step;
            total += gauss(from, at, &table, g);
            from = at;
            k += 1.0;
        }
        sign * (total + gauss(from, hi, &table, g))
    };
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
    use core::f64::consts::{FRAC_PI_8, TAU};

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
        let v = region_integral(&pieces, f64::INFINITY, |u, v| u * u * v);
        assert!((v - w.powi(3) / 3.0 * h * h / 2.0).abs() < 1e-12, "{v}");
        let reversed: Vec<Piece<'_>> = sides
            .iter()
            .zip(ranges)
            .rev()
            .map(|(c, r)| Piece::against(c, r))
            .collect();
        let v = region_integral(&reversed, f64::INFINITY, |_, _| 1.0);
        assert!((v + w * h).abs() < 1e-12, "{v}");
    }

    #[test]
    fn the_inner_step_splits_what_one_interval_does_not_resolve() {
        assert_eq!(intervals(0.0, f64::INFINITY), 1);
        assert_eq!(intervals(10.0, f64::NAN), 1);
        assert_eq!(intervals(TAU, FRAC_PI_2), 4);
        assert_eq!(intervals(1.0, 0.0), MAX_INNER_INTERVALS);
        // A whole turn of `cos⁴ u` over [0, 2π] × [0, 1]: `2π · 3/8`. The
        // inner integral runs from `u₀ = 0` to each boundary point, so
        // one Gauss interval has to carry a full turn of the integrand
        // and leaves a thousand times more than rounding; at a quarter
        // turn a step it is exact.
        let (w, h) = (TAU, 1.0);
        let line = |ox: f64, oy: f64, dx: f64, dy: f64| Curve2::Line {
            origin: Point2::new(ox, oy),
            direction: UnitVec2::new_normalize(Vec2::new(dx, dy)),
        };
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
        let exact = TAU * 3.0 / 8.0;
        let f = |u: f64, _: f64| u.cos().powi(4);
        let coarse = region_integral(&pieces, f64::INFINITY, f);
        let fine = region_integral(&pieces, FRAC_PI_2, f);
        assert!((coarse - exact).abs() > 1e-12, "{coarse}");
        assert!((fine - exact).abs() < 1e-14, "{fine}");
    }

    #[test]
    fn the_inner_split_is_aligned_to_the_grid_and_resolves_an_ellipses_area_element() {
        // The area element of an elliptic cylinder of aspect eighteen over
        // `[0, 2π] × [0, 1]`: the perimeter of the ellipse, against a fine
        // composite Simpson reference. Aligned sixteenth turns hold it to
        // `1e-11`; a whole turn's worth of equal intervals one longer than
        // the grid — what a span a rounding past `2π` used to get — does
        // not.
        let (a, b) = (9.0, 0.5);
        let g = |u: f64| (a * a * u.sin().powi(2) + b * b * u.cos().powi(2)).sqrt();
        let n = 400_000;
        let h = TAU / n as f64;
        let reference = (1..n)
            .map(|i| if i % 2 == 1 { 4.0 } else { 2.0 } * g(i as f64 * h))
            .sum::<f64>()
            + g(0.0)
            + g(TAU);
        let reference = reference * h / 3.0;
        let line = |ox: f64, oy: f64, dx: f64, dy: f64| Curve2::Line {
            origin: Point2::new(ox, oy),
            direction: UnitVec2::new_normalize(Vec2::new(dx, dy)),
        };
        // The strip's far side a rounding past the turn.
        let w = TAU + 4.0 * f64::EPSILON;
        let sides = [
            line(0.0, 0.0, 1.0, 0.0),
            line(w, 0.0, 0.0, 1.0),
            line(w, 1.0, -1.0, 0.0),
            line(0.0, 1.0, 0.0, -1.0),
        ];
        let ranges = [w, 1.0, w, 1.0].map(|l| Interval::new(0.0, l).unwrap());
        let pieces: Vec<Piece<'_>> = sides
            .iter()
            .zip(ranges)
            .map(|(c, r)| Piece::along(c, r))
            .collect();
        let area = region_integral(&pieces, FRAC_PI_8, |u, _| g(u));
        assert!(
            ((area - reference) / reference).abs() < 1e-11,
            "{area} vs {reference}"
        );
        // The grid also leaves the trigonometric cases exact.
        let exact = TAU * 3.0 / 8.0;
        let fine = region_integral(&pieces, FRAC_PI_2, |u, _| u.cos().powi(4));
        assert!((fine - exact).abs() < 1e-13, "{fine}");
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
        let v = region_integral(&[piece], f64::INFINITY, |_, _| 1.0);
        assert!((v + 4.0 * PI).abs() < 1e-12, "{v}");
        let arc = Piece::along(&cw, Interval::new(0.3, 5.0).unwrap());
        assert_eq!(breaks_of(&arc), [FRAC_PI_2, PI, 3.0 * FRAC_PI_2]);
    }
}
