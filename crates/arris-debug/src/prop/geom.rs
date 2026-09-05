//! Strategies for every analytic surface and curve in a random pose.
//!
//! Radii and angles come from the named ranges below, chosen so a
//! test's tolerance can be stated against [`super::DEFAULT_SCALE`]: every
//! coordinate a strategy produces is within a few multiples of it.

use core::ops::RangeInclusive;

use arris_geom::{Curve, Surface};
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
