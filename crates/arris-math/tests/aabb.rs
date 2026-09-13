//! `Aabb`: the box of a point set, the union, and the three the boolean's
//! cheap reject uses — `of_point`, `intersects` and `inflated`.

use arris_debug::prop::{DEFAULT_SCALE, check, finite_f64, point_in_box};
use arris_math::{Aabb, Point3};
use proptest::prelude::*;

fn coords(p: Point3) -> [f64; 3] {
    [p.x, p.y, p.z]
}

/// The box of a random set of points, and one point of it.
fn box_and_point() -> impl Strategy<Value = (Aabb, Point3)> {
    (
        proptest::collection::vec(point_in_box(DEFAULT_SCALE), 1..=8),
        point_in_box(DEFAULT_SCALE),
    )
        .prop_map(|(points, probe)| {
            let coords: Vec<[f64; 3]> = points.iter().copied().map(coords).collect();
            (Aabb::of_points(&coords).expect("at least one point"), probe)
        })
}

#[test]
fn a_box_holds_the_points_it_was_made_from() {
    check(
        proptest::collection::vec(point_in_box(DEFAULT_SCALE), 1..=8),
        |points| {
            let list: Vec<[f64; 3]> = points.iter().copied().map(coords).collect();
            let b = Aabb::of_points(&list).expect("at least one point");
            for p in &list {
                for (k, x) in p.iter().enumerate() {
                    prop_assert!(b.min[k] <= *x && *x <= b.max[k]);
                }
                prop_assert!(b.intersects(&Aabb::of_points(&[*p]).unwrap()));
            }
            prop_assert_eq!(Aabb::of_points(&[]), None);
            Ok(())
        },
    );
}

#[test]
fn a_point_box_intersects_exactly_the_boxes_that_hold_the_point() {
    check(box_and_point(), |(b, p)| {
        let point = Aabb::of_point(p);
        prop_assert_eq!(point.min, point.max);
        prop_assert_eq!(point.extent(), [0.0; 3]);
        let held = (0..3).all(|k| b.min[k] <= p[k] && p[k] <= b.max[k]);
        prop_assert_eq!(b.intersects(&point), held);
        prop_assert_eq!(point.intersects(&b), held, "intersects is symmetric");
        Ok(())
    });
}

#[test]
fn inflating_grows_every_side_and_never_shrinks_past_a_point() {
    check(
        (box_and_point(), finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE)),
        |((b, _), by)| {
            let grown = b.inflated(by);
            for k in 0..3 {
                prop_assert!(grown.min[k] <= grown.max[k], "min above max on axis {k}");
                if by >= 0.0 {
                    prop_assert!((grown.min[k] - (b.min[k] - by)).abs() <= 1e-12 * DEFAULT_SCALE);
                    prop_assert!((grown.max[k] - (b.max[k] + by)).abs() <= 1e-12 * DEFAULT_SCALE);
                    prop_assert!(grown.intersects(&b), "a grown box holds the original");
                }
            }
            // Two boxes a positive gap apart meet once one is inflated by
            // more than the gap.
            let gap = 1.0;
            let away = Aabb {
                min: [b.max[0] + gap, b.min[1], b.min[2]],
                max: [b.max[0] + gap + 1.0, b.max[1], b.max[2]],
            };
            prop_assert!(!b.intersects(&away));
            prop_assert!(b.inflated(gap).intersects(&away));
            prop_assert!(!b.inflated(0.5 * gap).intersects(&away));
            Ok(())
        },
    );
}

#[test]
fn a_union_holds_both_and_is_the_smallest_that_does() {
    check((box_and_point(), box_and_point()), |((a, _), (b, _))| {
        let u = a.union(b);
        for k in 0..3 {
            prop_assert_eq!(u.min[k], a.min[k].min(b.min[k]));
            prop_assert_eq!(u.max[k], a.max[k].max(b.max[k]));
        }
        prop_assert!(u.intersects(&a) && u.intersects(&b));
        Ok(())
    });
}
