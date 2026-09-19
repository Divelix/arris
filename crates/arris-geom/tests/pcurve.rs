//! Every analytic curve on a plane and on a cylinder has a pcurve whose
//! image under the surface is the curve at the same parameter — exactly
//! on the exact arms, within the tolerance on the fitted one — and a
//! curve projected onto a plane is the expected conic; and the six
//! exact arms a revolve makes on a cone, a sphere and a torus are lines
//! in (u, v) with the same property, while every other pair on those
//! three is `Unsupported` naming it.

use core::f64::consts::{PI, TAU};

use arris_debug::prop::geom::{cylinder, nurbs_curve, plane};
use arris_debug::prop::{DEFAULT_SCALE, check, finite_f64, frame, radius, unit_vec3};
use arris_geom::{
    Curve, Curve2, Curve2Kind, MeetKind, NurbsCurve, Surface, intersect_surfaces, pcurve_on,
    project_to_plane,
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

/// A region every traced section of these tests lies in; the closed
/// forms ignore it.
fn within() -> arris_math::Aabb {
    arris_math::Aabb {
        min: [-100.0; 3],
        max: [100.0; 3],
    }
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
            let r = intersect_surfaces(&plane, &s, &within(), tol()).unwrap();
            let crossing = r.points().is_empty()
                && !r.curves().is_empty()
                && r.curves().iter().all(|m| m.kind == MeetKind::Crossing);
            prop_assert!(crossing, "no transversal section for {which:?}: {r:?}");
            for c in r.curves().iter().map(|m| &m.curve) {
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

/// The direction `(cos θ, sin θ, 0)` of a frame, in the world.
fn radial(frame: &Frame, theta: f64) -> Vec3 {
    frame.vec_to_world(Vec3::new(theta.cos(), theta.sin(), 0.0))
}

/// `a` and `b` are the same angle modulo a turn, to rounding.
fn same_angle(a: f64, b: f64) -> bool {
    let d = (a - b).rem_euclid(TAU);
    d.min(TAU - d) <= 1e-9
}

/// The pcurve is a `Line` with the expected origin and direction, and its
/// image is the curve at the same parameter.
fn line_pcurve(
    pc: &Curve2,
    curve: &Curve,
    surface: &Surface,
    range: Interval,
    origin: arris_math::Point2,
    direction: arris_math::Vec2,
) -> Result<(), TestCaseError> {
    let Curve2::Line {
        origin: o,
        direction: d,
    } = pc
    else {
        prop_assert!(false, "{pc:?} is not a line in (u, v)");
        return Ok(());
    };
    prop_assert_eq!(d.into_inner(), direction, "{:?}", pc);
    if direction.x == 0.0 {
        prop_assert!(same_angle(o.x, origin.x), "u is {} not {}", o.x, origin.x);
        prop_assert!(
            (o.y - origin.y).abs() <= 1e-9,
            "v is {} not {}",
            o.y,
            origin.y
        );
    } else {
        prop_assert!(same_angle(o.x, origin.x), "u is {} not {}", o.x, origin.x);
        prop_assert!(
            (o.y - origin.y).abs() <= 1e-9 || same_angle(o.y, origin.y),
            "v is {} not {}",
            o.y,
            origin.y
        );
    }
    image_matches(pc, curve, surface, range, EXACT)
}

#[test]
fn a_ruling_and_a_parallel_of_a_cone_are_lines_in_uv() {
    check(
        (
            arris_debug::prop::geom::cone(),
            finite_f64(0.0..=TAU),
            finite_f64(0.0..=TAU),
            finite_f64(-5.0..=5.0),
            any::<bool>(),
            any::<bool>(),
        ),
        |(s, u0, beta, v0, up, flip)| {
            let Surface::Cone {
                frame: cone,
                radius,
                half_angle,
            } = &s
            else {
                unreachable!("the cone strategy yields cones")
            };
            let (sin, cos) = (half_angle.sin(), half_angle.cos());
            let axis = cone.z().into_inner();
            // A ruling: the line through the apex in the half-plane u0.
            let sense = if up { 1.0 } else { -1.0 };
            let ruling = Curve::Line {
                origin: s.point(u0, v0),
                direction: arris_math::UnitVec3::new_normalize(
                    sense * (sin * radial(cone, u0) + cos * axis),
                ),
            };
            let range = Interval::new(-4.0, 4.0).unwrap();
            let pc = pcurve_on(&ruling, range, &s, tol()).unwrap();
            line_pcurve(
                &pc,
                &ruling,
                &s,
                range,
                arris_math::Point2::new(u0, v0),
                arris_math::Vec2::new(0.0, sense),
            )?;
            // A parallel: the circle at v0, its own X at an arbitrary
            // angle and its Z either way round. Beyond the apex the
            // radial factor `R + v sin α` is negative, and the surface
            // reaches the circle's own `X` at `u + π`.
            let signed = radius + v0 * sin;
            if signed.abs() < 1e-3 {
                return Ok(()); // the apex is not a circle
            }
            let z = if flip { -axis } else { axis };
            let circle = Curve::Circle {
                frame: Frame::new(
                    cone.to_world(Point3::new(0.0, 0.0, v0 * cos)),
                    z,
                    radial(cone, beta),
                )
                .unwrap(),
                radius: signed.abs(),
            };
            let pc = pcurve_on(&circle, Interval::TURN, &s, tol()).unwrap();
            let expected_u = if signed > 0.0 { beta } else { beta + PI };
            line_pcurve(
                &pc,
                &circle,
                &s,
                Interval::TURN,
                arris_math::Point2::new(expected_u.rem_euclid(TAU), v0),
                arris_math::Vec2::new(if flip { -1.0 } else { 1.0 }, 0.0),
            )?;
            Ok(())
        },
    );
}

#[test]
fn a_parallel_and_a_meridian_of_a_sphere_are_lines_in_uv() {
    check(
        (
            arris_debug::prop::geom::sphere(),
            finite_f64(-1.4..=1.4),
            finite_f64(0.0..=TAU),
            finite_f64(0.0..=TAU),
            finite_f64(-0.7..=0.7),
            any::<bool>(),
        ),
        |(s, v0, beta, psi, phi, flip)| {
            let Surface::Sphere {
                frame: sphere,
                radius,
            } = &s
            else {
                unreachable!("the sphere strategy yields spheres")
            };
            let axis = sphere.z().into_inner();
            // A parallel at the latitude v0.
            let z = if flip { -axis } else { axis };
            let parallel = Curve::Circle {
                frame: Frame::new(
                    sphere.to_world(Point3::new(0.0, 0.0, radius * v0.sin())),
                    z,
                    radial(sphere, beta),
                )
                .unwrap(),
                radius: radius * v0.cos(),
            };
            let pc = pcurve_on(&parallel, Interval::TURN, &s, tol()).unwrap();
            line_pcurve(
                &pc,
                &parallel,
                &s,
                Interval::TURN,
                arris_math::Point2::new(beta, v0),
                arris_math::Vec2::new(if flip { -1.0 } else { 1.0 }, 0.0),
            )?;
            // A meridian: the great circle whose plane holds the axis,
            // its own X at the latitude phi of the half-plane psi + π/2.
            let u = psi + PI / 2.0;
            let cz = radial(sphere, psi);
            let cx = phi.cos() * radial(sphere, u) + phi.sin() * axis;
            let meridian = Curve::Circle {
                frame: Frame::new(sphere.origin(), if flip { -cz } else { cz }, cx).unwrap(),
                radius: *radius,
            };
            // A range that stays clear of the poles: v runs phi ± t.
            let range = Interval::new(-0.7, 0.7).unwrap();
            let pc = pcurve_on(&meridian, range, &s, tol()).unwrap();
            let Curve2::Line { origin, direction } = &pc else {
                prop_assert!(false, "{pc:?} is not a line in (u, v)");
                return Ok(());
            };
            prop_assert_eq!(direction.x, 0.0, "{:?}", pc);
            prop_assert_eq!(direction.y.abs(), 1.0, "{:?}", pc);
            prop_assert!(same_angle(origin.x, u), "u is {} not {}", origin.x, u);
            prop_assert!(
                (origin.y - phi).abs() <= 1e-9,
                "v is {} not {}",
                origin.y,
                phi
            );
            image_matches(&pc, &meridian, &s, range, EXACT)?;
            // Over a full turn the same line still names the curve: the
            // great circle runs up one meridian and down the other, which
            // is where v outside [−π/2, π/2] puts it.
            let pc = pcurve_on(&meridian, Interval::TURN, &s, tol()).unwrap();
            image_matches(&pc, &meridian, &s, Interval::TURN, EXACT)?;
            Ok(())
        },
    );
}

#[test]
fn a_parallel_and_a_tube_circle_of_a_torus_are_lines_in_uv() {
    check(
        (
            arris_debug::prop::geom::torus(),
            finite_f64(0.0..=TAU),
            finite_f64(0.0..=TAU),
            finite_f64(0.0..=TAU),
            finite_f64(0.0..=TAU),
            any::<bool>(),
        ),
        |(s, v0, beta, u0, phi, flip)| {
            let Surface::Torus {
                frame: torus,
                major_radius,
                minor_radius,
            } = &s
            else {
                unreachable!("the torus strategy yields tori")
            };
            let axis = torus.z().into_inner();
            // A circle about the axis at v0.
            let z = if flip { -axis } else { axis };
            let parallel = Curve::Circle {
                frame: Frame::new(
                    torus.to_world(Point3::new(0.0, 0.0, minor_radius * v0.sin())),
                    z,
                    radial(torus, beta),
                )
                .unwrap(),
                radius: major_radius + minor_radius * v0.cos(),
            };
            let pc = pcurve_on(&parallel, Interval::TURN, &s, tol()).unwrap();
            line_pcurve(
                &pc,
                &parallel,
                &s,
                Interval::TURN,
                arris_math::Point2::new(beta, v0),
                arris_math::Vec2::new(if flip { -1.0 } else { 1.0 }, 0.0),
            )?;
            // A circle of the tube at u0, its own X at the tube angle phi.
            let r = radial(torus, u0);
            let cz = r.cross(&axis);
            let cx = phi.cos() * r + phi.sin() * axis;
            let tube = Curve::Circle {
                frame: Frame::new(
                    torus.origin() + *major_radius * r,
                    if flip { -cz } else { cz },
                    cx,
                )
                .unwrap(),
                radius: *minor_radius,
            };
            let pc = pcurve_on(&tube, Interval::TURN, &s, tol()).unwrap();
            let Curve2::Line { origin, direction } = &pc else {
                prop_assert!(false, "{pc:?} is not a line in (u, v)");
                return Ok(());
            };
            prop_assert_eq!(direction.x, 0.0, "{:?}", pc);
            prop_assert_eq!(direction.y.abs(), 1.0, "{:?}", pc);
            prop_assert!(same_angle(origin.x, u0), "u is {} not {}", origin.x, u0);
            prop_assert!(same_angle(origin.y, phi), "v is {} not {}", origin.y, phi);
            prop_assert!((0.0..TAU).contains(&origin.y));
            image_matches(&pc, &tube, &s, Interval::TURN, EXACT)?;
            Ok(())
        },
    );
}

#[test]
fn what_a_surface_of_revolution_has_no_variant_for_is_unsupported_by_name() {
    let tol = tol();
    let sphere_frame = Frame::from_z(Point3::new(1.0, 2.0, 3.0), Vec3::new(0.0, 0.0, 1.0)).unwrap();
    let sphere = Surface::Sphere {
        frame: sphere_frame,
        radius: 5.0,
    };
    // A small circle on the sphere, about no axis of it: on the surface,
    // and with no `Curve2` variant.
    let centre = Point3::new(2.0, 3.0, 4.0);
    let offset = (centre - sphere_frame.origin()).norm();
    let oblique = Curve::Circle {
        frame: Frame::from_z(centre, centre - sphere_frame.origin()).unwrap(),
        radius: (25.0f64 - offset * offset).sqrt(),
    };
    for t in [0.0, 1.0, 2.0, 3.0] {
        assert!(((oblique.point(t) - sphere_frame.origin()).norm() - 5.0).abs() < 1e-12);
    }
    let err = pcurve_on(&oblique, Interval::TURN, &sphere, tol).unwrap_err();
    assert!(
        matches!(err, arris_geom::GeomError::Unsupported { a, b }
            if a == arris_geom::GeomKind::Curve(arris_geom::CurveKind::Circle)
            && b == arris_geom::GeomKind::Surface(arris_geom::SurfaceKind::Sphere)),
        "{err}"
    );
    // A rational quadratic Bézier quarter of the equator: exactly on the
    // sphere, and a NURBS, which has no exact arm here.
    let arc = NurbsCurve::new(
        2,
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        vec![
            sphere_frame.to_world(Point3::new(5.0, 0.0, 0.0)),
            sphere_frame.to_world(Point3::new(5.0, 5.0, 0.0)),
            sphere_frame.to_world(Point3::new(0.0, 5.0, 0.0)),
        ],
        vec![1.0, 0.5f64.sqrt(), 1.0],
    )
    .unwrap();
    let arc = Curve::Nurbs(arc);
    for i in 0..=8 {
        let t = i as f64 / 8.0;
        assert!(((arc.point(t) - sphere_frame.origin()).norm() - 5.0).abs() < 1e-12);
    }
    let err = pcurve_on(&arc, Interval::UNIT, &sphere, tol).unwrap_err();
    assert!(
        matches!(err, arris_geom::GeomError::Unsupported { a, .. }
            if a == arris_geom::GeomKind::Curve(arris_geom::CurveKind::Nurbs)),
        "{err}"
    );
    // An oblique plane section of a cone: an ellipse that lies on it.
    let (r, alpha) = (3.0, 0.6);
    let cone_frame = Frame::from_z(Point3::origin(), Vec3::z()).unwrap();
    let cone = Surface::Cone {
        frame: cone_frame,
        radius: r,
        half_angle: alpha,
    };
    let apex = Point3::new(0.0, 0.0, -r / alpha.tan());
    let gamma = (PI / 2.0 - alpha) / 2.0;
    let (k, h) = (alpha.tan().powi(2), 4.0);
    let a = gamma.cos().powi(2) - k * gamma.sin().powi(2);
    let s2c = k * h * gamma.sin() / a;
    let c = k * h * h * gamma.cos().powi(2) / a;
    let (e1, e2) = (Vec3::x(), Vec3::new(0.0, gamma.cos(), gamma.sin()));
    let centre = apex + Vec3::new(0.0, 0.0, h) + s2c * e2;
    let (a1, a2) = (c.sqrt(), (c / a).sqrt());
    let (major, minor, x, y) = if a2 >= a1 {
        (a2, a1, e2, e1)
    } else {
        (a1, a2, e1, e2)
    };
    let section = Curve::Ellipse {
        frame: Frame::new(centre, x.cross(&y), x).unwrap(),
        major_radius: major,
        minor_radius: minor,
    };
    for i in 0..16 {
        let t = TAU * i as f64 / 16.0;
        let d = section.point(t) - apex;
        let angle = Vec3::new(d.x, d.y, 0.0).norm().atan2(d.z);
        assert!(
            (angle - alpha).abs() < 1e-12,
            "the section is on the cone: {angle} vs {alpha}"
        );
    }
    let err = pcurve_on(&section, Interval::TURN, &cone, tol).unwrap_err();
    assert!(
        matches!(err, arris_geom::GeomError::Unsupported { a, b }
            if a == arris_geom::GeomKind::Curve(arris_geom::CurveKind::Ellipse)
            && b == arris_geom::GeomKind::Surface(arris_geom::SurfaceKind::Cone)),
        "{err}"
    );
    // A curve off any of the three is `NotOnSurface`, not unsupported.
    let torus = Surface::Torus {
        frame: Frame::world(),
        major_radius: 5.0,
        minor_radius: 1.0,
    };
    for surface in [&sphere, &cone, &torus] {
        let lifted = Curve::Circle {
            frame: Frame::from_z(Point3::new(0.0, 0.0, 50.0), Vec3::z()).unwrap(),
            radius: 2.0,
        };
        let err = pcurve_on(&lifted, Interval::TURN, surface, tol).unwrap_err();
        assert!(
            matches!(err, arris_geom::GeomError::NotOnSurface { .. }),
            "{surface:?}: {err}"
        );
    }
}
