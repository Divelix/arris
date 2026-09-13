//! `Isometry` composes like sequential application and inverts exactly
//! enough.

use arris_debug::prop::{DEFAULT_SCALE, check, point_in_box, pose};
use arris_math::{Isometry, UnitVec3};
use proptest::prelude::*;

/// Round trips through a pose in the default box.
const ROUND_TRIP: f64 = 1e-12 * DEFAULT_SCALE;

#[test]
fn composition_equals_sequential_application() {
    check(
        (
            pose(),
            pose(),
            point_in_box(DEFAULT_SCALE),
            point_in_box(DEFAULT_SCALE),
        ),
        |(a, b, p, v)| {
            let v = v.coords;
            let ab = a.then(&b);
            prop_assert!((ab.apply(p) - b.apply(a.apply(p))).norm() <= ROUND_TRIP);
            prop_assert!((ab.apply_vec(v) - b.apply_vec(a.apply_vec(v))).norm() <= ROUND_TRIP);
            // A vector is a difference of points.
            prop_assert!((a.apply(p + v) - a.apply(p) - a.apply_vec(v)).norm() <= ROUND_TRIP);
            // Lengths are preserved.
            prop_assert!((a.apply_vec(v).norm() - v.norm()).abs() <= ROUND_TRIP);
            Ok(())
        },
    );
}

#[test]
fn inverse_undoes_and_identity_does_nothing() {
    check((pose(), point_in_box(DEFAULT_SCALE)), |(m, p)| {
        prop_assert!((m.inverse().apply(m.apply(p)) - p).norm() <= ROUND_TRIP);
        prop_assert!((m.apply(m.inverse().apply(p)) - p).norm() <= ROUND_TRIP);
        let id = m.then(&m.inverse());
        prop_assert!((id.apply(p) - p).norm() <= ROUND_TRIP);
        prop_assert_eq!(Isometry::identity().apply(p), p);
        prop_assert_eq!(Isometry::default(), Isometry::identity());
        let u = UnitVec3::new_normalize(p.coords + m.translation());
        prop_assert!((m.apply_unit(u).norm() - 1.0).abs() <= 1e-15);
        Ok(())
    });
}
