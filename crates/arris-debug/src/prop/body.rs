//! Bodies in random poses: the operands of the boolean property tests
//! (`docs/plans/m4-booleans.md`). Each strategy yields a *description*
//! of a body — its numbers and a rigid motion — that [`Boxed::build`],
//! [`Cylindrical::build`] and [`OverlappingPair::build`] turn into bodies
//! of a model through the primitives and `ops::transform`, so a failing
//! case prints as numbers a fixture can be written from.

use arris_math::{Axis, Isometry, Point3, UnitVec3, Vec3};
use arris_ops::{OpError, primitive_box, primitive_cylinder, transform};
use arris_topo::{Body, Model};
use proptest::prelude::*;

use super::{DEFAULT_SCALE, finite_f64, pose_in, radius, unit_vec3};

/// The smallest extent (a box side, a cylinder's diameter or height) the
/// strategies produce.
pub const MIN_EXTENT: f64 = 1.0;
/// The largest.
pub const MAX_EXTENT: f64 = 20.0;

/// An axis-aligned box moved by a rigid motion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Boxed {
    /// The corner before the motion.
    pub min: Point3,
    /// The opposite corner.
    pub max: Point3,
    /// The motion.
    pub pose: Isometry,
}

impl Boxed {
    /// The box as a body of `m`: `primitive_box` then `transform`.
    pub fn build(&self, m: &mut Model) -> Result<Body, OpError> {
        let (body, _) = primitive_box(m, self.min, self.max)?;
        Ok(transform(m, body, &self.pose)?.0)
    }
}

/// A cylinder moved by a rigid motion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cylindrical {
    /// The axis before the motion, its origin the base cap's centre.
    pub axis: Axis,
    /// The radius.
    pub radius: f64,
    /// The height along the axis.
    pub height: f64,
    /// The motion.
    pub pose: Isometry,
}

impl Cylindrical {
    /// The cylinder as a body of `m`: `primitive_cylinder` then
    /// `transform`.
    pub fn build(&self, m: &mut Model) -> Result<Body, OpError> {
        let (body, _) = primitive_cylinder(m, self.axis, self.radius, self.height)?;
        Ok(transform(m, body, &self.pose)?.0)
    }
}

/// A box and a cylinder whose axis passes through the box's interior
/// and whose length exceeds the box's diagonal, both under one motion:
/// every pair intersects, and a corner of the box lies outside the
/// cylinder.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlappingPair {
    /// The box.
    pub cuboid: Boxed,
    /// The cylinder.
    pub cylinder: Cylindrical,
}

impl OverlappingPair {
    /// Both bodies in `m`, the box first.
    pub fn build(&self, m: &mut Model) -> Result<(Body, Body), OpError> {
        let a = self.cuboid.build(m)?;
        let b = self.cylinder.build(m)?;
        Ok((a, b))
    }
}

/// Box extents, each in `[MIN_EXTENT, MAX_EXTENT]`.
fn extents() -> impl Strategy<Value = Vec3> {
    (
        finite_f64(MIN_EXTENT..=MAX_EXTENT),
        finite_f64(MIN_EXTENT..=MAX_EXTENT),
        finite_f64(MIN_EXTENT..=MAX_EXTENT),
    )
        .prop_map(|(x, y, z)| Vec3::new(x, y, z))
}

/// Boxes of random extents centred at the origin, under a rigid motion
/// whose translation is uniform in the box of half-width `scale`.
pub fn box_in(scale: f64) -> impl Strategy<Value = Boxed> {
    (extents(), pose_in(scale)).prop_map(|(e, pose)| Boxed {
        min: Point3::from(-e / 2.0),
        max: Point3::from(e / 2.0),
        pose,
    })
}

/// Cylinders of random radius and height on the `z` axis, centred at
/// the origin, under a rigid motion as [`box_in`]'s.
pub fn cylinder_in(scale: f64) -> impl Strategy<Value = Cylindrical> {
    (
        radius(MIN_EXTENT / 2.0..=MAX_EXTENT / 2.0),
        radius(MIN_EXTENT..=MAX_EXTENT),
        pose_in(scale),
    )
        .prop_map(|(r, h, pose)| Cylindrical {
            axis: Axis::z_at(Point3::new(0.0, 0.0, -h / 2.0)),
            radius: r,
            height: h,
            pose,
        })
}

/// [`OverlappingPair`]s: a box from [`box_in`], a cylinder whose axis
/// runs through a random interior point of the box in a random
/// direction, three diagonals long, with a radius between a tenth and
/// six tenths of the box's smallest side, both under the box's motion at
/// [`DEFAULT_SCALE`].
pub fn overlapping_pair() -> impl Strategy<Value = OverlappingPair> {
    (
        box_in(DEFAULT_SCALE),
        (
            finite_f64(0.1..=0.9),
            finite_f64(0.1..=0.9),
            finite_f64(0.1..=0.9),
        ),
        unit_vec3(),
        finite_f64(0.1..=0.6),
    )
        .prop_filter_map("a cylinder axis", |(cuboid, (fx, fy, fz), d, rf)| {
            let e = cuboid.max - cuboid.min;
            let through = cuboid.min + Vec3::new(fx * e.x, fy * e.y, fz * e.z);
            let height = 3.0 * e.norm();
            let base = through - (height / 2.0) * d.into_inner();
            let axis = Axis::new(base, UnitVec3::into_inner(d)).ok()?;
            Some(OverlappingPair {
                cuboid,
                cylinder: Cylindrical {
                    axis,
                    radius: rf * e.x.min(e.y).min(e.z),
                    height,
                    pose: cuboid.pose,
                },
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prop::check;
    use arris_io::arris_check::{Level, check as check_body};

    #[test]
    fn the_pairs_build_clean_bodies_with_the_axis_through_the_box() {
        check(overlapping_pair(), |pair| {
            let mut m = Model::default();
            let (a, b) = pair
                .build(&mut m)
                .map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert!(check_body(&m, a, Level::Fast).is_ok());
            prop_assert!(check_body(&m, b, Level::Fast).is_ok());
            // The axis's midpoint is the box's interior point, before
            // and after the motion.
            let mid = pair.cylinder.axis.at(pair.cylinder.height / 2.0);
            let (lo, hi) = (pair.cuboid.min, pair.cuboid.max);
            prop_assert!((0..3).all(|i| lo[i] < mid[i] && mid[i] < hi[i]));
            Ok(())
        });
    }
}
