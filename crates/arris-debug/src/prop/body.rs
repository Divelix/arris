//! Bodies in random poses: the operands of the boolean property tests.
//! Each strategy yields a *description* of a body — its numbers and a
//! rigid motion — that [`Boxed::build`], [`Cylindrical::build`] and
//! [`OverlappingPair::build`] turn into bodies of a model through the
//! primitives and `ops::transform`, so a failing case prints as numbers a
//! fixture can be written from.

use arris_math::nalgebra::UnitQuaternion;
use arris_math::{Axis, Frame, Isometry, Point3, UnitVec3, Vec3};
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

/// A box and a cylinder whose wall touches one face of the box from
/// outside along a ruling through the face's interior, both under one
/// motion: the tangent case with nothing else in contact (plan
/// m4-booleans step 11). Every boolean's outcome is known: `box −
/// cylinder` is the box, `common` selects nothing, and `fuse` would
/// hold the face and the wall touching along a slit, the designed
/// `TangentContact` refusal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TangentPair {
    /// The box.
    pub cuboid: Boxed,
    /// The cylinder.
    pub cylinder: Cylindrical,
    /// The touched face: the axis it is normal to (`0`, `1`, `2`) and
    /// whether it is the box's `max` side.
    pub face: (usize, bool),
}

impl TangentPair {
    /// Both bodies in `m`, the box first.
    pub fn build(&self, m: &mut Model) -> Result<(Body, Body), OpError> {
        let a = self.cuboid.build(m)?;
        let b = self.cylinder.build(m)?;
        Ok((a, b))
    }

    /// The outward normal of the touched face, before the motion.
    pub fn normal(&self) -> Vec3 {
        let mut n = Vec3::zeros();
        n[self.face.0] = if self.face.1 { 1.0 } else { -1.0 };
        n
    }

    /// Where the ruling crosses the touched face: the foot of the axis's
    /// midpoint on the face's plane, before the motion.
    pub fn foot(&self) -> Point3 {
        let mid = self.cylinder.axis.at(self.cylinder.height / 2.0);
        mid - self.cylinder.radius * self.normal()
    }
}

/// [`TangentPair`]s: a box from [`box_in`], a face of it, a point in the
/// face's interior (a tenth to nine tenths of each extent), an axis
/// direction in the face's plane at a random angle, and a cylinder of
/// random radius and height on that direction at the radius's distance
/// outside the face, its midpoint over the point — so the ruling
/// crosses the face's interior and the two faces touch along a segment
/// interior to both. The cylinder's ends may reach past the box or stop
/// short of it: both kinds of touch — a box edge on the wall and a rim
/// on the face — occur.
pub fn tangent_pair() -> impl Strategy<Value = TangentPair> {
    (
        box_in(DEFAULT_SCALE),
        0usize..3,
        any::<bool>(),
        (finite_f64(0.1..=0.9), finite_f64(0.1..=0.9)),
        finite_f64(0.0..=core::f64::consts::TAU),
        radius(MIN_EXTENT / 2.0..=MAX_EXTENT / 2.0),
        radius(MIN_EXTENT..=MAX_EXTENT),
    )
        .prop_filter_map(
            "a cylinder tangent to a box face",
            |(cuboid, axis_index, max_side, (fu, fv), angle, r, height)| {
                let e = cuboid.max - cuboid.min;
                let (i, j, k) = (axis_index, (axis_index + 1) % 3, (axis_index + 2) % 3);
                let mut foot = cuboid.min;
                foot[i] = if max_side {
                    cuboid.max[i]
                } else {
                    cuboid.min[i]
                };
                foot[j] += fu * e[j];
                foot[k] += fv * e[k];
                let mut normal = Vec3::zeros();
                normal[i] = if max_side { 1.0 } else { -1.0 };
                let mut d = Vec3::zeros();
                d[j] = angle.cos();
                d[k] = angle.sin();
                let mid = foot + r * normal;
                let axis = Axis::new(mid - (height / 2.0) * d, d).ok()?;
                Some(TangentPair {
                    cuboid,
                    cylinder: Cylindrical {
                        axis,
                        radius: r,
                        height,
                        pose: cuboid.pose,
                    },
                    face: (i, max_side),
                })
            },
        )
}

/// Where a [`ParallelPair`]'s second cylinder stands against the first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParallelKind {
    /// It reaches past both caps of the first, at a random angle about it.
    Clear,
    /// It reaches past both caps, and one ruling of the pair is the
    /// first cylinder's seam.
    SeamOnRuling,
    /// It is as tall as the first and their caps are coincident: the rim
    /// circles cross on each cap plane at the rulings' ends.
    FlushCaps,
}

/// Two cylinders on parallel axes whose walls cross in two rulings, both
/// under one motion: `a` on the
/// `z` axis centred at the origin, `b`'s axis `distance` from it, strictly
/// between the internal and external tangent distances `|R₁ − R₂|` and
/// `R₁ + R₂`, so neither wall holds the other and neither touches it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParallelPair {
    /// The first cylinder.
    pub a: Cylindrical,
    /// The second.
    pub b: Cylindrical,
    /// The distance between the axes.
    pub distance: f64,
    /// Where `b` stands.
    pub kind: ParallelKind,
}

impl ParallelPair {
    /// Both bodies in `m`, `a` first.
    pub fn build(&self, m: &mut Model) -> Result<(Body, Body), OpError> {
        Ok((self.a.build(m)?, self.b.build(m)?))
    }
}

/// [`ParallelPair`]s: two radii, the axes' distance drawn between the
/// tangent distances a tenth of the gap clear of each, and `b` placed at
/// a random angle about `a` — a quarter [`ParallelKind::SeamOnRuling`],
/// a quarter [`ParallelKind::FlushCaps`], the rest
/// [`ParallelKind::Clear`], reaching a tenth to a half of `a`'s height
/// past each cap.
pub fn parallel_pair() -> impl Strategy<Value = ParallelPair> {
    (
        radius(MIN_EXTENT / 2.0..=MAX_EXTENT / 2.0),
        radius(MIN_EXTENT / 2.0..=MAX_EXTENT / 2.0),
        finite_f64(0.1..=0.9),
        finite_f64(0.0..=core::f64::consts::TAU),
        radius(MIN_EXTENT..=MAX_EXTENT),
        finite_f64(0.1..=0.5),
        0u8..4,
        pose_in(DEFAULT_SCALE),
    )
        .prop_map(|(r1, r2, gap, angle, height, reach, kind, pose)| {
            let (near, far) = ((r1 - r2).abs(), r1 + r2);
            let distance = near + gap * (far - near);
            let kind = match kind {
                0 => ParallelKind::SeamOnRuling,
                1 => ParallelKind::FlushCaps,
                _ => ParallelKind::Clear,
            };
            // `a`'s seam runs along its frame's `x`, through (r1, 0); the
            // rulings are at `angle ± α` about `a`'s axis, with `cos α` from
            // the triangle of the two radii and the distance.
            let angle = if kind == ParallelKind::SeamOnRuling {
                ((distance * distance + r1 * r1 - r2 * r2) / (2.0 * distance * r1))
                    .clamp(-1.0, 1.0)
                    .acos()
            } else {
                angle
            };
            let (below, tall) = if kind == ParallelKind::FlushCaps {
                (0.0, height)
            } else {
                (reach * height, height * (1.0 + 2.0 * reach))
            };
            let centre = Point3::new(distance * angle.cos(), distance * angle.sin(), 0.0);
            ParallelPair {
                a: Cylindrical {
                    axis: Axis::z_at(Point3::new(0.0, 0.0, -height / 2.0)),
                    radius: r1,
                    height,
                    pose,
                },
                b: Cylindrical {
                    axis: Axis::z_at(centre - Vec3::new(0.0, 0.0, height / 2.0 + below)),
                    radius: r2,
                    height: tall,
                    pose,
                },
                distance,
                kind,
            }
        })
}

/// Two cylinders of one radius whose axes cross at their midpoints at an
/// angle `psi`, each through the other, both under one motion: `a` on the
/// `z` axis centred
/// at the origin, `b` in the `xz` plane, turned about its own axis first.
/// Their walls cross in two ellipses, which cross each other at
/// `(0, ±R, 0)` before the motion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrossingPair {
    /// The first cylinder.
    pub a: Cylindrical,
    /// The second; its pose is a turn about its own axis, then `a`'s.
    pub b: Cylindrical,
    /// The angle between the axes, in radians.
    pub psi: f64,
    /// Whether `b`'s turn puts its seam through the crossing vertex
    /// `(0, R, 0)`, where the seam is tangent to `a`'s wall.
    pub seam_through_crossing: bool,
}

impl CrossingPair {
    /// Both bodies in `m`, `a` first.
    pub fn build(&self, m: &mut Model) -> Result<(Body, Body), OpError> {
        Ok((self.a.build(m)?, self.b.build(m)?))
    }

    /// The volume of their common, the Steinmetz solid at `psi`:
    /// `16R³ / (3 sin ψ)`.
    pub fn common_volume(&self) -> f64 {
        16.0 * self.a.radius.powi(3) / (3.0 * self.psi.sin())
    }
}

/// [`CrossingPair`]s: a radius, `ψ ∈ [30°, 90°]`, both cylinders
/// `4R / sin ψ` to `8R / sin ψ` long — so each cap's disc stands clear of
/// the other wall, its centre more than `2R` from the other axis — and
/// `b` turned about its axis by a random angle, or in a quarter of cases
/// so its seam runs through the crossing vertex `(0, R, 0)`.
pub fn crossing_pair() -> impl Strategy<Value = CrossingPair> {
    (
        radius(MIN_EXTENT / 2.0..=MAX_EXTENT / 2.0),
        finite_f64(30.0..=90.0),
        finite_f64(1.0..=2.0),
        finite_f64(0.0..=core::f64::consts::TAU),
        0u8..4,
        pose_in(DEFAULT_SCALE),
    )
        .prop_filter_map(
            "two crossing cylinders",
            |(r, psi_deg, stretch, turn, kind, pose)| {
                let psi = psi_deg.to_radians();
                let length = stretch * 4.0 * r / psi.sin();
                let d = Vec3::new(psi.sin(), 0.0, psi.cos());
                let seam_through_crossing = kind == 0;
                let turn = if seam_through_crossing {
                    // The seam runs along the frame's `x`, perpendicular to
                    // the axis like `y`: the rotation taking one to the
                    // other is about the axis.
                    let seam = Frame::from_z(Point3::origin(), d).ok()?.x().into_inner();
                    UnitQuaternion::rotation_between(&seam, &Vec3::y())?
                } else {
                    UnitQuaternion::from_axis_angle(&UnitVec3::try_new(d, 0.0)?, turn)
                };
                Some(CrossingPair {
                    a: Cylindrical {
                        axis: Axis::z_at(Point3::new(0.0, 0.0, -length / 2.0)),
                        radius: r,
                        height: length,
                        pose,
                    },
                    b: Cylindrical {
                        axis: Axis::new(Point3::origin() - (length / 2.0) * d, d).ok()?,
                        radius: r,
                        height: length,
                        pose: Isometry::from_rotation(turn).then(&pose),
                    },
                    psi,
                    seam_through_crossing,
                })
            },
        )
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

    /// The tangent pairs touch: the axis is one radius outside the face's
    /// plane, the foot of the ruling is inside the face, and the bodies
    /// are clean.
    #[test]
    fn the_tangent_pairs_touch_one_face_from_outside() {
        check(tangent_pair(), |pair| {
            let mut m = Model::default();
            let (a, b) = pair
                .build(&mut m)
                .map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert!(check_body(&m, a, Level::Fast).is_ok());
            prop_assert!(check_body(&m, b, Level::Fast).is_ok());
            let (lo, hi) = (pair.cuboid.min, pair.cuboid.max);
            let (i, max_side) = pair.face;
            let plane = if max_side { hi[i] } else { lo[i] };
            let mid = pair.cylinder.axis.at(pair.cylinder.height / 2.0);
            let n = pair.normal();
            prop_assert!(((mid[i] - plane) * n[i] - pair.cylinder.radius).abs() < 1e-12);
            prop_assert!(pair.cylinder.axis.direction.dot(&n).abs() < 1e-12);
            let foot = pair.foot();
            prop_assert!((foot[i] - plane).abs() < 1e-12);
            prop_assert!(
                (0..3)
                    .filter(|&c| c != i)
                    .all(|c| lo[c] < foot[c] && foot[c] < hi[c])
            );
            Ok(())
        });
    }

    /// The parallel pairs cross: the axes `distance` apart, strictly
    /// between the tangent distances; a seam pair's seam on `b`'s wall; a
    /// flush pair's caps level with each other and every other `b` past
    /// both of `a`'s caps; the bodies are clean.
    #[test]
    fn the_parallel_pairs_cross_in_two_rulings() {
        check(parallel_pair(), |pair| {
            let mut m = Model::default();
            let (a, b) = pair
                .build(&mut m)
                .map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert!(check_body(&m, a, Level::Fast).is_ok());
            prop_assert!(check_body(&m, b, Level::Fast).is_ok());
            let (r1, r2, d) = (pair.a.radius, pair.b.radius, pair.distance);
            prop_assert!((r1 - r2).abs() < d && d < r1 + r2);
            let offset = pair.b.axis.origin - pair.a.axis.origin;
            let across = Vec3::new(offset.x, offset.y, 0.0);
            prop_assert!((across.norm() - d).abs() < 1e-12 * MAX_EXTENT);
            let (za, zb) = (pair.a.axis.origin.z, pair.b.axis.origin.z);
            let (ha, hb) = (pair.a.height, pair.b.height);
            match pair.kind {
                ParallelKind::SeamOnRuling => {
                    let seam = Vec3::new(r1, 0.0, 0.0);
                    prop_assert!(((seam - across).norm() - r2).abs() < 1e-12 * MAX_EXTENT);
                }
                ParallelKind::FlushCaps => {
                    prop_assert!((za - zb).abs() < 1e-12 * MAX_EXTENT && ha == hb);
                }
                ParallelKind::Clear => {}
            }
            if pair.kind != ParallelKind::FlushCaps {
                prop_assert!(zb < za && zb + hb > za + ha);
            }
            Ok(())
        });
    }

    /// The crossing pairs pass through each other: equal radii, the axes
    /// at `psi` through both midpoints, each cap's centre more than two
    /// radii from the other axis, and a seam pair's turned seam along `y`,
    /// through the crossing vertex; the bodies are clean.
    #[test]
    fn the_crossing_pairs_pass_through_each_other() {
        check(crossing_pair(), |pair| {
            let mut m = Model::default();
            let (a, b) = pair
                .build(&mut m)
                .map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert!(check_body(&m, a, Level::Fast).is_ok());
            prop_assert!(check_body(&m, b, Level::Fast).is_ok());
            let r = pair.a.radius;
            prop_assert_eq!(pair.b.radius, r);
            let cos = pair.a.axis.direction.dot(&pair.b.axis.direction);
            prop_assert!((cos - pair.psi.cos()).abs() < 1e-12);
            for c in [pair.a, pair.b] {
                prop_assert!(c.axis.at(c.height / 2.0).coords.norm() < 1e-12 * MAX_EXTENT);
                prop_assert!(c.height / 2.0 * pair.psi.sin() > 2.0 * r);
            }
            if pair.seam_through_crossing {
                let turn = pair.b.pose.then(&pair.a.pose.inverse());
                let frame = Frame::from_z(pair.b.axis.origin, pair.b.axis.direction.into_inner())
                    .map_err(|e| TestCaseError::fail(e.to_string()))?;
                let seam = turn.apply_vec(frame.x().into_inner());
                prop_assert!((seam.y.abs() - 1.0).abs() < 1e-12);
            }
            Ok(())
        });
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
