//! `Frame` is orthonormal and right-handed from any input, and its two
//! coordinate maps invert each other under any pose.

use arris_debug::prop::{
    DEFAULT_SCALE, check, finite_f64, frame, point_in_box, pose, rotation, unit_vec3,
};
use arris_math::{Frame, Point3, Vec3};
use proptest::prelude::*;

/// How far from orthonormal a frame may be: a few rounding errors on unit
/// vectors. A test tolerance, not an algorithm's.
const ORTHONORMAL: f64 = 1e-15;
/// Round trips through a frame in the default box.
const ROUND_TRIP: f64 = 1e-12 * DEFAULT_SCALE;

fn assert_orthonormal_right_handed(f: &Frame) -> Result<(), TestCaseError> {
    let (x, y, z) = (f.x().into_inner(), f.y().into_inner(), f.z().into_inner());
    for (name, axis) in [("x", x), ("y", y), ("z", z)] {
        let n = axis.norm();
        prop_assert!((n - 1.0).abs() <= ORTHONORMAL, "|{name}| = {n}");
    }
    prop_assert!(x.dot(&y).abs() <= ORTHONORMAL, "x·y = {}", x.dot(&y));
    prop_assert!(y.dot(&z).abs() <= ORTHONORMAL, "y·z = {}", y.dot(&z));
    prop_assert!(z.dot(&x).abs() <= ORTHONORMAL, "z·x = {}", z.dot(&x));
    // Orthonormal and a positive triple product is exactly right-handed.
    let triple = x.cross(&y).dot(&z);
    prop_assert!(triple > 0.0, "x × y · z = {triple}");
    Ok(())
}

#[test]
fn new_from_any_axis_and_hint_is_orthonormal_and_right_handed() {
    let lengths = || finite_f64(1e-3..=1e3);
    check(
        (
            point_in_box(DEFAULT_SCALE),
            unit_vec3(),
            lengths(),
            unit_vec3(),
            lengths(),
        ),
        |(origin, z, zl, hint, hl)| {
            let z = z.into_inner() * zl;
            let hint = hint.into_inner() * hl;
            prop_assume!(z.cross(&hint).norm() > 0.0);
            let f = Frame::new(origin, z, hint);
            prop_assert!(f.is_ok(), "{f:?}");
            let f = f.unwrap();
            assert_orthonormal_right_handed(&f)?;
            // `z` is the given axis, `x` lies in the plane of `z` and the hint on
            // the hint's side.
            prop_assert!((f.z().into_inner() - z.normalize()).norm() <= ORTHONORMAL);
            prop_assert!(f.x().dot(&hint) > 0.0);
            prop_assert_eq!(f.origin(), origin);
            Ok(())
        },
    );
}

#[test]
fn from_z_and_from_rotation_are_orthonormal_and_right_handed() {
    check((unit_vec3(), rotation()), |(z, q)| {
        let f = Frame::from_z(Point3::origin(), z.into_inner());
        prop_assert!(f.is_ok(), "{f:?}");
        let f = f.unwrap();
        assert_orthonormal_right_handed(&f)?;
        prop_assert!((f.z().into_inner() - z.into_inner()).norm() <= ORTHONORMAL);
        let g = Frame::from_rotation(Point3::origin(), &q);
        assert_orthonormal_right_handed(&g)?;
        // The frame's rotation is the one it was built from, up to sign.
        let r = g.rotation();
        let same = (r.coords - q.coords)
            .norm()
            .min((r.coords + q.coords).norm());
        prop_assert!(same <= 1e-14, "rotation round trip {same}");
        Ok(())
    });
}

#[test]
fn to_world_and_to_local_invert_each_other_under_random_poses() {
    check(
        (
            frame(),
            pose(),
            point_in_box(DEFAULT_SCALE),
            point_in_box(DEFAULT_SCALE),
        ),
        |(f, m, p, v)| {
            let v = v.coords;
            prop_assert!((f.to_world(f.to_local(p)) - p).norm() <= ROUND_TRIP);
            prop_assert!((f.to_local(f.to_world(p)) - p).norm() <= ROUND_TRIP);
            prop_assert!((f.vec_to_world(f.vec_to_local(v)) - v).norm() <= ROUND_TRIP);
            // The frame as an isometry is `to_world`.
            prop_assert!((f.as_isometry().apply(p) - f.to_world(p)).norm() <= ROUND_TRIP);
            // Moving the frame moves its image.
            let g = f.transformed(&m);
            assert_orthonormal_right_handed(&g)?;
            prop_assert!((g.to_world(p) - m.apply(f.to_world(p))).norm() <= ROUND_TRIP);
            prop_assert!((g.to_local(m.apply(f.to_world(p))) - p).norm() <= ROUND_TRIP);
            prop_assert_eq!(m.apply_frame(&f), g);
            Ok(())
        },
    );
}

#[test]
fn a_local_axis_maps_to_the_frame_axis() {
    check(frame(), |f| {
        let e = |v: Vec3| f.vec_to_world(v);
        prop_assert!((e(Vec3::x()) - f.x().into_inner()).norm() <= ORTHONORMAL);
        prop_assert!((e(Vec3::y()) - f.y().into_inner()).norm() <= ORTHONORMAL);
        prop_assert!((e(Vec3::z()) - f.z().into_inner()).norm() <= ORTHONORMAL);
        prop_assert!((f.to_world(Point3::origin()) - f.origin()).norm() == 0.0);
        Ok(())
    });
}
