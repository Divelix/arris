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

    /// Whether the cylinder's wall clears every edge of the box: the
    /// distance from each of the twelve edge segments to the axis line
    /// exceeds the radius. Then the cylinder enters through the interior
    /// of one face and leaves through the interior of another (every
    /// ruling meets the box in one segment, so the wall's entry and exit
    /// curves are closed curves that cross no edge), so `box − cylinder`
    /// is one shell and `cylinder − box` exactly two. Decided before the
    /// motion, which both operands share.
    pub fn pierces(&self) -> bool {
        let (lo, hi) = (self.cuboid.min, self.cuboid.max);
        let corner = |i: usize| {
            Point3::new(
                if i & 1 == 0 { lo.x } else { hi.x },
                if i & 2 == 0 { lo.y } else { hi.y },
                if i & 4 == 0 { lo.z } else { hi.z },
            )
        };
        let axis = &self.cylinder.axis;
        let d = axis.direction.into_inner();
        // The component of `v` perpendicular to the axis.
        let across = |v: Vec3| v - d * v.dot(&d);
        (0..8)
            .flat_map(|i| {
                [1usize, 2, 4]
                    .into_iter()
                    .filter(move |b| i & b == 0)
                    .map(move |b| (i, i | b))
            })
            .all(|(i, j)| {
                // The nearest point of the segment to the axis line, by
                // minimising the perpendicular offset over the segment.
                let a = across(corner(i) - axis.origin);
                let b = across(corner(j) - corner(i));
                let bb = b.dot(&b);
                let s = if bb > 0.0 {
                    (-a.dot(&b) / bb).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                (a + b * s).norm() > self.cylinder.radius
            })
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

/// [`overlapping_pair`]s whose cylinder [`OverlappingPair::pierces`] the
/// box — clears every edge — so the outcome of every boolean is known by
/// construction: `fuse` and `common` are one shell, `box − cylinder` is
/// one shell and `cylinder − box` is two. The pairs that fail the test
/// (the wall crossing an edge, slicing a corner off, or fat enough that
/// the cylinder's two ends join around the box) are a large share of
/// [`overlapping_pair`] and stay in it.
pub fn piercing_pair() -> impl Strategy<Value = OverlappingPair> {
    overlapping_pair().prop_filter(
        "the cylinder clears every edge of the box",
        OverlappingPair::pierces,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prop::check;
    use arris_io::arris_check::{Level, check as check_body};
    use arris_math::nalgebra::UnitQuaternion;

    /// `pierces` is the wall clearing every edge: the through-hole does,
    /// the same hole widened to reach the long edges does not, and a post
    /// standing across a side face (`boolean/sliver-common`) does not.
    #[test]
    fn pierces_is_the_wall_clearing_every_edge() {
        let still = Isometry::new(UnitQuaternion::identity(), Vec3::zeros());
        let plate = Boxed {
            min: Point3::origin(),
            max: Point3::new(40.0, 30.0, 10.0),
            pose: still,
        };
        let post = |x: f64, y: f64, radius: f64| OverlappingPair {
            cuboid: plate,
            cylinder: Cylindrical {
                axis: Axis::z_at(Point3::new(x, y, -1.0)),
                radius,
                height: 12.0,
                pose: still,
            },
        };
        assert!(post(20.0, 15.0, 4.0).pierces());
        // The edges y = 0 and y = 30 are 15 away from the axis.
        assert!(post(20.0, 15.0, 14.5).pierces());
        assert!(!post(20.0, 15.0, 15.5).pierces());
        assert!(!post(43.8, 15.0, 4.0).pierces());
        // Along an edge: the axis on the edge x = 0, y = 0 itself.
        assert!(!post(0.0, 0.0, 1.0).pierces());
    }

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
