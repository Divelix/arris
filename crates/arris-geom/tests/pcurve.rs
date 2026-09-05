//! Every analytic curve on a plane and on a cylinder has a pcurve whose
//! image under the surface is the curve at the same parameter — exactly
//! on the exact arms, within the tolerance on the fitted one — and a
//! curve projected onto a plane is the expected conic
//! (`docs/plans/m1-geometry.md` step 10).

use core::f64::consts::{PI, TAU};

use arris_debug::prop::geom::{cylinder, nurbs_curve, plane};
use arris_debug::prop::{DEFAULT_SCALE, check, finite_f64, frame, radius, unit_vec3};
use arris_geom::{
    Curve, Curve2, Curve2Kind, NurbsCurve, Surface, SurfaceIntersection, intersect_surfaces,
    pcurve_on, project_to_plane,
};
use arris_math::{Frame, Interval, Point3, Precision, Tolerance, Vec3};
use proptest::prelude::*;

/// Exact arms: the image matches the curve to rounding at the scale.
const EXACT: f64 = 1e-12 * DEFAULT_SCALE;
/// Parameters the image is checked at per case, both ends included.
const CHECKS: usize = 64;

fn tol() -> Tolerance {
    Precision::DEFAULT.tolerance()
}

/// Which analytic curve to build in the plane.
#[derive(Debug, Clone, Copy)]
enum Flat {
    Line,
    Circle { flipped: bool },
    Ellipse { flipped: bool },
}

fn flat() -> impl Strategy<Value = Flat> {
    prop_oneof![
        Just(Flat::Line),
        any::<bool>().prop_map(|flipped| Flat::Circle { flipped }),
        any::<bool>().prop_map(|flipped| Flat::Ellipse { flipped }),
    ]
}

/// An analytic curve lying in the plane: origin at plane (u0, v0), in-plane
/// axes turned by `angle`, `Z` along or against the plane's normal.
fn curve_in(plane: &Frame, which: Flat, u0: f64, v0: f64, angle: f64, r: f64, extra: f64) -> Curve {
    let origin = plane.to_world(Point3::new(u0, v0, 0.0));
    let x = plane.vec_to_world(Vec3::new(angle.cos(), angle.sin(), 0.0));
    let z = |flipped: bool| {
        if flipped {
            -plane.z().into_inner()
        } else {
            plane.z().into_inner()
        }
    };
    match which {
        Flat::Line => Curve::Line {
            origin,
            direction: arris_math::UnitVec3::new_normalize(x),
        },
        Flat::Circle { flipped } => Curve::Circle {
            frame: Frame::new(origin, z(flipped), x).unwrap(),
            radius: r,
        },
        Flat::Ellipse { flipped } => Curve::Ellipse {
            frame: Frame::new(origin, z(flipped), x).unwrap(),
            major_radius: r + extra,
            minor_radius: r,
        },
    }
}

/// `surface.point(pcurve(t)) == curve.point(t)` at `CHECKS` parameters
/// over `range`, both ends included, to `within`.
fn image_matches(
    pc: &Curve2,
    curve: &Curve,
    surface: &Surface,
    range: Interval,
    within: f64,
) -> Result<(), TestCaseError> {
    for i in 0..=CHECKS {
        let t = range.lerp(i as f64 / CHECKS as f64);
        let uv = pc.point(t);
        let image = surface.point(uv.x, uv.y);
        let expected = curve.point(t);
        let err = (image - expected).norm();
        prop_assert!(
            err <= within,
            "at t = {t}: image {image} vs curve {expected}, off by {err} ({pc:?})"
        );
    }
    Ok(())
}

#[test]
fn every_analytic_curve_in_a_plane_has_an_exact_pcurve() {
    check(
        (
            plane(),
            flat(),
            finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
            finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
            finite_f64(0.0..=TAU),
            radius(0.1..=10.0),
            finite_f64(0.0..=10.0),
        ),
        |(s, which, u0, v0, angle, r, extra)| {
            let Surface::Plane { frame } = &s else {
                unreachable!("the plane strategy yields planes")
            };
            let c = curve_in(frame, which, u0, v0, angle, r, extra);
            let range = match which {
                Flat::Line => Interval::new(-DEFAULT_SCALE, DEFAULT_SCALE).unwrap(),
                _ => Interval::TURN,
            };
            let pc = pcurve_on(&c, range, &s, tol()).unwrap();
            image_matches(&pc, &c, &s, range, EXACT)?;
            match (which, &pc) {
                (Flat::Line, Curve2::Line { .. }) => {}
                (
                    Flat::Circle { flipped },
                    Curve2::Circle {
                        frame: f2,
                        radius: r2,
                    },
                ) => {
                    prop_assert_eq!(*r2, r);
                    // A circle whose Z opposes the normal runs clockwise in (u, v).
                    prop_assert_eq!(f2.is_right_handed(), !flipped);
                }
                (Flat::Ellipse { flipped }, Curve2::Ellipse { frame: f2, .. }) => {
                    prop_assert_eq!(f2.is_right_handed(), !flipped);
                }
                _ => prop_assert!(false, "{which:?} gave {:?}", pc.kind()),
            }
            // Lifting the curve off the plane by more than the tolerance is
            // refused, naming the distance.
            let lifted = c.transformed(&arris_math::Isometry::from_translation(
                3.0 * tol().linear * frame.z().into_inner(),
            ));
            let err = pcurve_on(&lifted, range, &s, tol()).unwrap_err();
            let off = matches!(err, arris_geom::GeomError::NotOnSurface { .. });
            prop_assert!(off, "{err}");
            Ok(())
        },
    );
}

#[test]
fn a_nurbs_in_a_plane_has_its_control_points_projected() {
    check(
        (plane(), nurbs_curve(), finite_f64(0.0..=1.0)),
        |(s, c, at)| {
            let Surface::Plane { frame } = &s else {
                unreachable!("the plane strategy yields planes")
            };
            // Flatten the control points onto the plane.
            let n = frame.z().into_inner();
            let points: Vec<Point3> = c
                .control_points()
                .iter()
                .map(|p| p - (p - frame.origin()).dot(&n) * n)
                .collect();
            let flat =
                NurbsCurve::new(c.degree(), c.knots().to_vec(), points, c.weights().to_vec())
                    .unwrap();
            let curve = Curve::Nurbs(flat);
            let range = curve.domain();
            let pc = pcurve_on(&curve, range, &s, tol()).unwrap();
            prop_assert_eq!(pc.kind(), Curve2Kind::Nurbs);
            image_matches(&pc, &curve, &s, range, EXACT)?;
            let t = range.lerp(at);
            let uv = pc.point(t);
            prop_assert!((s.point(uv.x, uv.y) - curve.point(t)).norm() <= EXACT);
            // The projection agrees, since the curve lies in the plane.
            let projected = project_to_plane(&curve, frame).unwrap();
            prop_assert!((projected.point(t) - uv).norm() <= EXACT);
            Ok(())
        },
    );
}

/// Which plane–cylinder section to cut.
#[derive(Debug, Clone, Copy)]
enum Section {
    Circle,
    Ellipse,
    Rulings,
}

fn section() -> impl Strategy<Value = Section> {
    prop_oneof![
        Just(Section::Circle),
        Just(Section::Ellipse),
        Just(Section::Rulings),
    ]
}

#[test]
fn every_section_of_a_cylinder_has_a_pcurve_on_it() {
    check(
        (
            cylinder(),
            section(),
            unit_vec3(),
            finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
            finite_f64(-0.9..=0.9),
        ),
        |(s, which, n, height, across)| {
            let Surface::Cylinder {
                frame: cyl,
                radius: r,
            } = &s
            else {
                unreachable!("the cylinder strategy yields cylinders")
            };
            let axis = cyl.z().into_inner();
            let on_axis = cyl.origin() + height * axis;
            let (origin, normal) = match which {
                Section::Circle => (on_axis, if n.z >= 0.0 { axis } else { -axis }),
                Section::Ellipse => {
                    if n.cross(&axis).norm() < 1e-3 || n.dot(&axis).abs() < 1e-3 {
                        return Ok(());
                    }
                    (on_axis, n.into_inner())
                }
                Section::Rulings => {
                    let perp = (n.into_inner() - n.dot(&axis) * axis).normalize();
                    (on_axis + across * r * cyl.x().into_inner(), perp)
                }
            };
            let plane = Surface::Plane {
                frame: Frame::from_z(origin, normal).unwrap(),
            };
            let SurfaceIntersection::Transversal(curves) =
                intersect_surfaces(&plane, &s, tol()).unwrap()
            else {
                prop_assert!(false, "no transversal section for {which:?}");
                return Ok(());
            };
            for c in &curves {
                let range = match c {
                    Curve::Line { .. } => Interval::new(-DEFAULT_SCALE, DEFAULT_SCALE).unwrap(),
                    _ => Interval::TURN,
                };
                let pc = pcurve_on(c, range, &s, tol()).unwrap();
                match (c, &pc) {
                    (Curve::Line { .. }, Curve2::Line { direction, .. }) => {
                        // A ruling: constant u.
                        prop_assert_eq!(direction.x, 0.0);
                        image_matches(&pc, c, &s, range, EXACT)?;
                    }
                    (Curve::Circle { .. }, Curve2::Line { direction, origin }) => {
                        // A parallel: constant v, u from the seam offset.
                        prop_assert_eq!(direction.y, 0.0);
                        prop_assert!((0.0..TAU).contains(&origin.x));
                        image_matches(&pc, c, &s, range, EXACT)?;
                    }
                    (Curve::Ellipse { .. }, Curve2::Nurbs(fit)) => {
                        image_matches(&pc, c, &s, range, tol().linear)?;
                        prop_assert_eq!(fit.domain(), range);
                        // Continuous across the seam: u never jumps, and
                        // over the full turn it covers one period.
                        let us: Vec<f64> = (0..=CHECKS)
                            .map(|i| pc.point(range.lerp(i as f64 / CHECKS as f64)).x)
                            .collect();
                        for w in us.windows(2) {
                            prop_assert!((w[1] - w[0]).abs() < PI / 4.0, "u jumps: {us:?}");
                        }
                        let span = us.iter().cloned().fold(f64::MIN, f64::max)
                            - us.iter().cloned().fold(f64::MAX, f64::min);
                        prop_assert!((span - TAU).abs() <= 1e-6, "u covers {span}, not a turn");
                    }
                    _ => prop_assert!(false, "{c:?} gave {:?}", pc.kind()),
                }
            }
            Ok(())
        },
    );
}

#[test]
fn a_circle_projected_to_an_oblique_plane_is_the_expected_ellipse() {
    check(
        (frame(), frame(), radius(0.1..=10.0), finite_f64(0.0..=TAU)),
        |(target, circle_frame, r, t)| {
            let circle = Curve::Circle {
                frame: circle_frame,
                radius: r,
            };
            let cos = circle_frame.z().dot(&target.z()).abs();
            let projected = project_to_plane(&circle, &target).unwrap();
            let Curve2::Ellipse {
                frame: f2,
                major_radius,
                minor_radius,
            } = &projected
            else {
                // Only a circle parallel to the target stays a circle.
                let stays_a_circle = matches!(projected, Curve2::Circle { .. });
                prop_assert!(stays_a_circle && cos > 1.0 - 1e-12, "{projected:?}");
                return Ok(());
            };
            prop_assert!(
                (major_radius - r).abs() <= EXACT,
                "major {major_radius} vs {r}"
            );
            prop_assert!(
                (minor_radius - r * cos).abs() <= EXACT,
                "minor {minor_radius} vs {}",
                r * cos
            );
            // The point set: the projection of every circle point lies on
            // the ellipse (its implicit form in the ellipse's own frame).
            let p = target.to_local(circle.point(t));
            let q = f2.to_local(arris_math::Point2::new(p.x, p.y));
            let implicit = (q.x / major_radius).powi(2) + (q.y / minor_radius).powi(2);
            prop_assert!(
                (implicit - 1.0).abs() <= 1e-9,
                "implicit {implicit} at t = {t}"
            );
            // The traversal sense is the sign of Z against the normal.
            prop_assert_eq!(
                f2.is_right_handed(),
                circle_frame.z().dot(&target.z()) > 0.0
            );
            // A projected line is the line of projected points.
            let line = Curve::Line {
                origin: circle_frame.origin(),
                direction: circle_frame.x(),
            };
            if let Ok(Curve2::Line { .. }) = project_to_plane(&line, &target) {
                let pl = project_to_plane(&line, &target).unwrap();
                let p3 = target.to_local(line.point(t));
                let proj = pl.project(arris_math::Point2::new(p3.x, p3.y)).unwrap();
                prop_assert!(proj.distance <= EXACT);
            }
            Ok(())
        },
    );
}
