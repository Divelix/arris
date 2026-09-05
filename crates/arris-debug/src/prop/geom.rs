//! Strategies for every analytic surface and curve in a random pose, and
//! for random clamped NURBS curves and surfaces.
//!
//! Radii and angles come from the named ranges below, chosen so a
//! test's tolerance can be stated against [`super::DEFAULT_SCALE`]: every
//! coordinate a strategy produces is within a few multiples of it.
//! [`surface`] and [`curve`] draw the analytic variants only; the NURBS
//! strategies are separate because most cycle-1 queries are
//! `Unsupported` on them by design.

use core::ops::RangeInclusive;

use arris_geom::{Curve, NurbsCurve, NurbsSurface, Surface};
use proptest::prelude::*;

use super::{DEFAULT_SCALE, finite_f64, frame, point_in_box, radius, unit_vec3};

/// Radii of the strategies' surfaces and curves: far from zero, far
/// below [`DEFAULT_SCALE`].
pub const RADIUS_RANGE: RangeInclusive<f64> = 0.1..=10.0;
/// A cone's half-angle, inside `(0, π/2)` with a margin so the apex is
/// within a few scales of the origin.
pub const HALF_ANGLE_RANGE: RangeInclusive<f64> = 0.05..=1.5;

/// Planes in a random pose.
pub fn plane() -> impl Strategy<Value = Surface> {
    frame().prop_map(|frame| Surface::Plane { frame })
}

/// Cylinders in a random pose with a radius in [`RADIUS_RANGE`].
pub fn cylinder() -> impl Strategy<Value = Surface> {
    (frame(), radius(RADIUS_RANGE)).prop_map(|(frame, radius)| Surface::Cylinder { frame, radius })
}

/// Cones in a random pose with a radius in [`RADIUS_RANGE`] and a
/// half-angle in [`HALF_ANGLE_RANGE`].
pub fn cone() -> impl Strategy<Value = Surface> {
    (frame(), radius(RADIUS_RANGE), finite_f64(HALF_ANGLE_RANGE)).prop_map(
        |(frame, radius, half_angle)| Surface::Cone {
            frame,
            radius,
            half_angle,
        },
    )
}

/// Spheres in a random pose with a radius in [`RADIUS_RANGE`].
pub fn sphere() -> impl Strategy<Value = Surface> {
    (frame(), radius(RADIUS_RANGE)).prop_map(|(frame, radius)| Surface::Sphere { frame, radius })
}

/// Tori in a random pose with `minor_radius` in [`RADIUS_RANGE`] and
/// `major_radius` larger by at least the range's start, so the tube never
/// touches the axis.
pub fn torus() -> impl Strategy<Value = Surface> {
    (frame(), radius(RADIUS_RANGE), radius(RADIUS_RANGE)).prop_map(
        |(frame, minor_radius, extra)| Surface::Torus {
            frame,
            major_radius: minor_radius + extra,
            minor_radius,
        },
    )
}

/// Any analytic surface, each variant equally likely.
pub fn surface() -> impl Strategy<Value = Surface> {
    prop_oneof![plane(), cylinder(), cone(), sphere(), torus()]
}

/// Lines through a random point in the default box in a random direction.
pub fn line() -> impl Strategy<Value = Curve> {
    (point_in_box(DEFAULT_SCALE), unit_vec3())
        .prop_map(|(origin, direction)| Curve::Line { origin, direction })
}

/// Circles in a random pose with a radius in [`RADIUS_RANGE`].
pub fn circle() -> impl Strategy<Value = Curve> {
    (frame(), radius(RADIUS_RANGE)).prop_map(|(frame, radius)| Curve::Circle { frame, radius })
}

/// Ellipses in a random pose with `minor_radius` in [`RADIUS_RANGE`] and
/// `major_radius` at least as large, up to twice the range's end; a
/// circle (equal radii) is reachable.
pub fn ellipse() -> impl Strategy<Value = Curve> {
    (
        frame(),
        radius(RADIUS_RANGE),
        finite_f64(0.0..=*RADIUS_RANGE.end()),
    )
        .prop_map(|(frame, minor_radius, extra)| Curve::Ellipse {
            frame,
            major_radius: minor_radius + extra,
            minor_radius,
        })
}

/// Any analytic curve, each variant equally likely.
pub fn curve() -> impl Strategy<Value = Curve> {
    prop_oneof![line(), circle(), ellipse()]
}

/// Degrees of the NURBS strategies: from linear to quintic, so the
/// derivative tables are exercised past the second order they compute.
pub const NURBS_DEGREE_RANGE: RangeInclusive<usize> = 1..=5;
/// How many spans a random NURBS curve's domain has.
pub const NURBS_SPAN_RANGE: RangeInclusive<usize> = 1..=6;
/// Weights of the NURBS strategies: positive, within a factor of two of
/// one, so a rational evaluation is exercised without a nearly singular
/// homogeneous divisor.
pub const NURBS_WEIGHT_RANGE: RangeInclusive<f64> = 0.5..=2.0;

/// A clamped knot vector of `degree` with the interior knots at the
/// integers `1..spans` and the given multiplicities (each in
/// `1..=degree`), on `[0, spans]`.
fn clamped_knots(degree: usize, multiplicities: &[usize]) -> Vec<f64> {
    let spans = multiplicities.len() + 1;
    let mut knots = vec![0.0; degree + 1];
    for (i, &m) in multiplicities.iter().enumerate() {
        knots.extend(core::iter::repeat_n((i + 1) as f64, m));
    }
    knots.extend(core::iter::repeat_n(spans as f64, degree + 1));
    knots
}

/// Clamped rational B-spline curves: a degree in [`NURBS_DEGREE_RANGE`],
/// a domain of [`NURBS_SPAN_RANGE`] unit spans whose interior knots
/// carry random multiplicities up to the degree, control points uniform
/// in the default box and weights in [`NURBS_WEIGHT_RANGE`].
pub fn nurbs_curve() -> impl Strategy<Value = NurbsCurve> {
    (NURBS_DEGREE_RANGE, NURBS_SPAN_RANGE)
        .prop_flat_map(|(degree, spans)| {
            (
                Just(degree),
                proptest::collection::vec(1..=degree, spans - 1),
            )
        })
        .prop_flat_map(|(degree, mults)| {
            let knots = clamped_knots(degree, &mults);
            let n = knots.len() - degree - 1;
            (
                Just(degree),
                Just(knots),
                proptest::collection::vec(point_in_box(DEFAULT_SCALE), n),
                proptest::collection::vec(finite_f64(NURBS_WEIGHT_RANGE), n),
            )
        })
        .prop_map(|(degree, knots, points, weights)| {
            NurbsCurve::new(degree, knots, points, weights)
                .expect("the strategy builds a valid knot vector and positive weights")
        })
}

/// Clamped rational B-spline surfaces: degrees in [`NURBS_DEGREE_RANGE`]
/// capped at three, one to three uniform unit spans per direction,
/// control points uniform in the default box and weights in
/// [`NURBS_WEIGHT_RANGE`].
pub fn nurbs_surface() -> impl Strategy<Value = NurbsSurface> {
    (1..=3usize, 1..=3usize, 1..=3usize, 1..=3usize)
        .prop_flat_map(|(p, q, su, sv)| {
            let ku = clamped_knots(p, &vec![1; su - 1]);
            let kv = clamped_knots(q, &vec![1; sv - 1]);
            let n = (ku.len() - p - 1) * (kv.len() - q - 1);
            (
                Just([p, q]),
                Just([ku, kv]),
                proptest::collection::vec(point_in_box(DEFAULT_SCALE), n),
                proptest::collection::vec(finite_f64(NURBS_WEIGHT_RANGE), n),
            )
        })
        .prop_map(|(degree, knots, points, weights)| {
            NurbsSurface::new(degree, knots, points, weights)
                .expect("the strategy builds a valid net")
        })
}
