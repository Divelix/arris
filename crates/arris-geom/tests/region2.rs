//! The (u, v) toolkit (`docs/plans/m2-topology.md` step 6): discretised
//! loops have the areas, winding numbers and intersections their pcurves
//! say, across the seam range too, and the region integral recovers areas
//! and the sample cylinder's volume.

use core::f64::consts::{PI, TAU};

use arris_debug::prop::{DEFAULT_SCALE, check, finite_f64, radius};
use arris_debug::sample;
use arris_geom::integrate::{self, region_integral};
use arris_geom::region2::{Piece, Polygon2, discretise};
use arris_geom::{Curve2, Surface};
use arris_math::predicates::{Sign, orient2d};
use arris_math::{Frame2, Handedness, Interval, Point2, Point3, UnitVec2, Vec2};
use arris_topo::{Model, Orientation};
use proptest::prelude::*;

/// Rounding at the scale.
const EXACT: f64 = 1e-12 * DEFAULT_SCALE;

fn line_between(a: Point2, b: Point2) -> (Curve2, Interval) {
    let d = b - a;
    (
        Curve2::Line {
            origin: a,
            direction: UnitVec2::new_normalize(d),
        },
        Interval::new(0.0, d.norm()).unwrap(),
    )
}

/// The polygon of straight pieces through `corners`, closed.
fn polygon_of(corners: &[Point2], chord: f64) -> Polygon2 {
    let sides: Vec<(Curve2, Interval)> = (0..corners.len())
        .map(|i| line_between(corners[i], corners[(i + 1) % corners.len()]))
        .collect();
    let pieces: Vec<Piece<'_>> = sides.iter().map(|(c, r)| Piece::along(c, *r)).collect();
    discretise(&pieces, chord)
}

/// An axis-aligned rectangle from `(u0, v0)` of size `w × h`, counter-clockwise.
fn rectangle(u0: f64, v0: f64, w: f64, h: f64) -> [Point2; 4] {
    [
        Point2::new(u0, v0),
        Point2::new(u0 + w, v0),
        Point2::new(u0 + w, v0 + h),
        Point2::new(u0, v0 + h),
    ]
}

#[test]
fn a_discretised_circle_of_either_handedness_has_its_signed_area_within_the_chord_bound() {
    check(
        (
            finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
            finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
            finite_f64(0.0..=TAU),
            radius(0.1..=DEFAULT_SCALE),
            any::<bool>(),
            finite_f64(-6.0..=-2.0),
        ),
        |(u, v, angle, r, right, log_tol)| {
            let handedness = if right {
                Handedness::Right
            } else {
                Handedness::Left
            };
            let circle = Curve2::Circle {
                frame: Frame2::new(
                    Point2::new(u, v),
                    Vec2::new(angle.cos(), angle.sin()),
                    handedness,
                )
                .unwrap(),
                radius: r,
            };
            let tol = 10f64.powf(log_tol) * r;
            let polygon = discretise(&[Piece::along(&circle, Interval::TURN)], tol);
            prop_assert!(polygon.chord_deviation() <= tol);
            let area = polygon.signed_area();
            let expected = if right { PI * r * r } else { -PI * r * r };
            // The polygon is inscribed: the area it misses is at most the
            // perimeter times the chord deviation.
            prop_assert!(
                (area - expected).abs() <= TAU * r * tol + EXACT * r,
                "area {area} vs {expected} at chord {tol}"
            );
            prop_assert!(polygon.self_intersections().is_empty());
            Ok(())
        },
    );
}

#[test]
fn the_winding_number_of_a_point_against_a_convex_polygon_is_the_half_plane_test() {
    check(
        (
            proptest::collection::vec(finite_f64(0.0..=TAU), 3..12),
            radius(1.0..=DEFAULT_SCALE),
            finite_f64(-2.0 * DEFAULT_SCALE..=2.0 * DEFAULT_SCALE),
            finite_f64(-2.0 * DEFAULT_SCALE..=2.0 * DEFAULT_SCALE),
        ),
        |(mut angles, r, px, py)| {
            angles.sort_by(f64::total_cmp);
            angles.dedup();
            prop_assume!(angles.len() >= 3);
            // Points on a circle in angular order: convex, counter-clockwise.
            let corners: Vec<Point2> = angles
                .iter()
                .map(|a| Point2::new(r * a.cos(), r * a.sin()))
                .collect();
            let polygon = polygon_of(&corners, f64::INFINITY);
            let p = Point2::new(px, py);
            let n = corners.len();
            let sides: Vec<Sign> = (0..n)
                .map(|i| orient2d(corners[i], corners[(i + 1) % n], p))
                .collect();
            prop_assume!(sides.iter().all(|&s| s != Sign::Zero));
            let inside = sides.iter().all(|&s| s == Sign::Positive);
            let winding = polygon.winding_number(p);
            prop_assert_eq!(winding, i32::from(inside), "{:?} against {:?}", p, corners);
            if !inside {
                prop_assert_eq!(winding, 0);
            }
            prop_assert!(polygon.signed_area() > 0.0);
            Ok(())
        },
    );
}

#[test]
fn a_rectangle_across_the_seam_range_has_its_unwrapped_area() {
    check(
        (
            finite_f64(0.0..=TAU),
            finite_f64(0.1..=TAU),
            finite_f64(0.1..=DEFAULT_SCALE),
            finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
        ),
        |(u0, w, h, v0)| {
            // u runs past 2π: the pieces are taken as written, never wrapped.
            let polygon = polygon_of(&rectangle(u0, v0, w, h), f64::INFINITY);
            prop_assert!((polygon.signed_area() - w * h).abs() <= EXACT * (w + h));
            prop_assert_eq!(
                polygon.winding_number(Point2::new(u0 + w / 2.0, v0 + h / 2.0)),
                1
            );
            prop_assert_eq!(
                polygon.winding_number(Point2::new(u0 + w / 2.0 - TAU, v0 + h / 2.0)),
                0,
                "a period away is outside: the toolkit does not wrap"
            );
            // Sides built corner to corner meet the next corner to rounding.
            prop_assert!(polygon.gaps().iter().all(|&g| g <= EXACT));
            Ok(())
        },
    );
}

#[test]
fn disjoint_loops_report_no_intersection_and_crossing_ones_report_the_pair() {
    check(
        (
            finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
            finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
            finite_f64(1.0..=DEFAULT_SCALE),
            finite_f64(1.0..=DEFAULT_SCALE),
            finite_f64(0.1..=0.9),
            finite_f64(1.1..=3.0),
        ),
        |(u0, v0, w, h, inside_fraction, apart)| {
            let a = polygon_of(&rectangle(u0, v0, w, h), f64::INFINITY);
            // Shifted by a fraction of its width: the two right sides cross
            // the other's top and bottom.
            let shifted = rectangle(u0 + inside_fraction * w, v0 + inside_fraction * h, w, h);
            let b = polygon_of(&shifted, f64::INFINITY);
            // Its right side (1) and top (2) are crossed, whatever rounding
            // does to the closing segment's index in `b`.
            let hits = a.intersections(&b);
            let crossed: std::collections::BTreeSet<usize> = hits.iter().map(|h| h.0).collect();
            prop_assert_eq!(crossed, [1, 2].into_iter().collect(), "{:?}", hits);
            // Moved clear of it: nothing.
            let far = rectangle(u0 + apart * w, v0, w, h);
            let c = polygon_of(&far, f64::INFINITY);
            prop_assert!(a.intersections(&c).is_empty());
            prop_assert!(a.self_intersections().is_empty());
            // A hole strictly inside: no intersection, winding one for its corners.
            let hole = rectangle(
                u0 + 0.25 * w,
                v0 + 0.25 * h,
                0.5 * w * inside_fraction,
                0.5 * h * inside_fraction,
            );
            let d = polygon_of(&hole, f64::INFINITY);
            prop_assert!(a.intersections(&d).is_empty());
            prop_assert!(hole.iter().all(|&p| a.winding_number(p) == 1));
            Ok(())
        },
    );
}

#[test]
fn the_region_integral_of_one_is_the_area_of_a_rectangle_and_of_the_cylinder_wall() {
    check(
        (
            finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
            finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
            finite_f64(0.1..=DEFAULT_SCALE),
            finite_f64(0.1..=DEFAULT_SCALE),
            radius(0.1..=DEFAULT_SCALE),
        ),
        |(u0, v0, w, h, r)| {
            let corners = rectangle(u0, v0, w, h);
            let sides: Vec<(Curve2, Interval)> = (0..4)
                .map(|i| line_between(corners[i], corners[(i + 1) % 4]))
                .collect();
            let pieces: Vec<Piece<'_>> = sides.iter().map(|(c, r)| Piece::along(c, *r)).collect();
            let area = region_integral(&pieces, f64::INFINITY, |_, _| 1.0);
            prop_assert!(
                (area - w * h).abs() <= EXACT * (w + h).max(1.0),
                "{area} vs {}",
                w * h
            );
            // The cylinder wall's domain [0, 2π] × [0, h] with the surface's
            // own area element: 2π r h.
            let wall = Surface::Cylinder {
                frame: arris_math::Frame::world(),
                radius: r,
            };
            let corners = rectangle(0.0, 0.0, TAU, h);
            let sides: Vec<(Curve2, Interval)> = (0..4)
                .map(|i| line_between(corners[i], corners[(i + 1) % 4]))
                .collect();
            let pieces: Vec<Piece<'_>> = sides.iter().map(|(c, r)| Piece::along(c, *r)).collect();
            let area = region_integral(&pieces, integrate::inner_step(&wall), |u, v| {
                let e = wall.eval(u, v);
                e.du.cross(&e.dv).norm()
            });
            let expected = TAU * r * h;
            prop_assert!(
                (area - expected).abs() <= 1e-12 * expected.max(1.0) * DEFAULT_SCALE,
                "{area} vs {expected}"
            );
            Ok(())
        },
    );
}

/// Gauss's volume: `∬ P · N / 3 dA` summed over the faces with their use
/// orientation, each face's region walked as its loop is stored.
fn gauss_volume(m: &Model, body: arris_topo::Body) -> f64 {
    let mut volume = 0.0;
    for face in m.faces(body).unwrap() {
        let entity = m.face(face.id).unwrap();
        let surface = m.surface(entity.surface()).unwrap();
        let sign = face.orientation.sign();
        for l in entity.loops() {
            let pcurves: Vec<(&Curve2, Interval, bool)> = l
                .coedges()
                .iter()
                .map(|c| {
                    let edge = m.edge(c.edge()).unwrap();
                    (
                        m.curve2(c.pcurve()).unwrap(),
                        edge.range(),
                        c.orientation() == Orientation::Reversed,
                    )
                })
                .collect();
            let pieces: Vec<Piece<'_>> = pcurves
                .iter()
                .map(|&(curve, range, reversed)| Piece {
                    curve,
                    range,
                    reversed,
                })
                .collect();
            volume += sign
                * region_integral(&pieces, integrate::inner_step(surface), |u, v| {
                    let e = surface.eval(u, v);
                    (e.point - Point3::origin()).dot(&e.du.cross(&e.dv)) / 3.0
                });
        }
    }
    volume
}

#[test]
fn the_gauss_volume_integrand_over_the_sample_cylinder_gives_its_volume() {
    let mut m = Model::default();
    let body = sample::cylinder(&mut m, 4.0, 12.0).unwrap();
    let volume = gauss_volume(&m, body);
    let expected = PI * 16.0 * 12.0;
    assert!(
        (volume - expected).abs() <= 1e-9 * expected,
        "{volume} vs {expected}"
    );
    let body = sample::cuboid(&mut m, Point3::origin(), Point3::new(40.0, 30.0, 10.0)).unwrap();
    let volume = gauss_volume(&m, body);
    assert!((volume - 12000.0).abs() <= 1e-9 * 12000.0, "{volume}");
    let body =
        sample::cuboid_nurbs(&mut m, Point3::origin(), Point3::new(40.0, 30.0, 10.0)).unwrap();
    let volume = gauss_volume(&m, body);
    assert!((volume - 12000.0).abs() <= 1e-9 * 12000.0, "{volume}");
}
