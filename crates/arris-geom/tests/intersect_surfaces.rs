//! Plane–plane and plane–cylinder intersections follow the case table in
//! any pose: the variant is the constructed case's, every result curve
//! lies on both surfaces, the ellipse's axes are the closed form, the
//! tangent line is the ruling at the nearest point, the result is
//! symmetric under swapping and bit-identical across runs, and every other
//! surface pair is `Unsupported`.

use core::f64::consts::{FRAC_PI_2, TAU};

use arris_debug::prop::geom::{cylinder, plane, surface};
use arris_debug::prop::{DEFAULT_SCALE, check, finite_f64, point_in_box, unit_vec3};
use arris_geom::{
    Curve, GeomError, GeomKind, Surface, SurfaceIntersection, SurfaceKind, intersect_surfaces,
};
use arris_math::{Frame, Point3, Precision, Tolerance, UnitVec3, Vec3};
use proptest::prelude::*;

/// Points on the result curves against both surfaces, and the closed
/// forms of the ellipse's axes and the tangent ruling.
const EXACT: f64 = 1e-12 * DEFAULT_SCALE;
/// Parameters sampled along every result curve.
const SAMPLES: usize = 17;

fn tol() -> Tolerance {
    Precision::DEFAULT.tolerance()
}

/// The distance from `p` to the surface by its implicit form.
fn implicit_distance(s: &Surface, p: Point3) -> f64 {
    let q = s.frame().unwrap().to_local(p);
    match s {
        Surface::Plane { .. } => q.z.abs(),
        &Surface::Cylinder { radius, .. } => (q.x.hypot(q.y) - radius).abs(),
        Surface::Cone { .. }
        | Surface::Sphere { .. }
        | Surface::Torus { .. }
        | Surface::Nurbs(_) => {
            unreachable!("only planes and cylinders intersect in cycle 1")
        }
    }
}

/// Every sampled point of `c` lies on both `a` and `b`.
fn on_both(c: &Curve, a: &Surface, b: &Surface) -> Result<(), TestCaseError> {
    let range = match c {
        Curve::Line { .. } => -DEFAULT_SCALE..=DEFAULT_SCALE,
        Curve::Circle { .. } | Curve::Ellipse { .. } => 0.0..=TAU,
        Curve::Nurbs(c) => c.domain().lo()..=c.domain().hi(),
    };
    for i in 0..SAMPLES {
        let s = i as f64 / (SAMPLES - 1) as f64;
        let t = range.start() + s * (range.end() - range.start());
        let p = c.point(t);
        let (da, db) = (implicit_distance(a, p), implicit_distance(b, p));
        prop_assert!(
            da <= EXACT && db <= EXACT,
            "{c:?} at t = {t} is off by {da} from {a:?} and {db} from {b:?}"
        );
    }
    Ok(())
}

/// The same point set, up to a line's orientation.
fn same_curve(a: &Curve, b: &Curve) -> bool {
    let parallel = |x: &UnitVec3, y: &UnitVec3| x.cross(y).norm() <= EXACT;
    match (a, b) {
        (
            Curve::Line { origin, direction },
            Curve::Line {
                origin: o2,
                direction: d2,
            },
        ) => parallel(direction, d2) && (o2 - origin).cross(direction).norm() <= EXACT,
        (
            Curve::Circle { frame, radius },
            Curve::Circle {
                frame: f2,
                radius: r2,
            },
        ) => {
            (frame.origin() - f2.origin()).norm() <= EXACT
                && (radius - r2).abs() <= EXACT
                && parallel(&frame.z(), &f2.z())
        }
        (
            Curve::Ellipse {
                frame,
                major_radius,
                minor_radius,
            },
            Curve::Ellipse {
                frame: f2,
                major_radius: a2,
                minor_radius: b2,
            },
        ) => {
            (frame.origin() - f2.origin()).norm() <= EXACT
                && (major_radius - a2).abs() <= EXACT
                && (minor_radius - b2).abs() <= EXACT
                && parallel(&frame.z(), &f2.z())
                && parallel(&frame.x(), &f2.x())
        }
        _ => false,
    }
}

fn curves_of(r: &SurfaceIntersection) -> &[Curve] {
    match r {
        SurfaceIntersection::Transversal(c) | SurfaceIntersection::Tangent(c) => c,
        SurfaceIntersection::Empty | SurfaceIntersection::Coincident => &[],
    }
}

/// The checks every result passes whatever its case: curves on both
/// surfaces, symmetry under swapping, determinism.
fn common_properties(a: &Surface, b: &Surface) -> Result<SurfaceIntersection, TestCaseError> {
    let r = intersect_surfaces(a, b, tol()).map_err(|e| TestCaseError::fail(e.to_string()))?;
    for c in curves_of(&r) {
        on_both(c, a, b)?;
    }
    let swapped =
        intersect_surfaces(b, a, tol()).map_err(|e| TestCaseError::fail(e.to_string()))?;
    prop_assert_eq!(
        core::mem::discriminant(&swapped),
        core::mem::discriminant(&r),
        "swap changed the variant: {:?} vs {:?}",
        r,
        swapped
    );
    let (cs, ss) = (curves_of(&r), curves_of(&swapped));
    prop_assert_eq!(cs.len(), ss.len());
    for (c, s) in cs.iter().zip(ss) {
        prop_assert!(same_curve(c, s), "swap changed a curve: {:?} vs {:?}", c, s);
    }
    let again = intersect_surfaces(a, b, tol()).map_err(|e| TestCaseError::fail(e.to_string()))?;
    prop_assert_eq!(&again, &r, "two runs differ");
    Ok(r)
}

/// The plane–cylinder case to realise; picked first, then posed.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Case {
    Circle,
    Ellipse,
    TwoLines,
    TangentLine,
    Empty,
}

/// A plane that realises `case` against `cyl`, built from the raw
/// numbers: `height` along the axis, `phase` around it, `angle` the
/// normal's tilt off the axis for the ellipse, `fraction` the axis
/// distance over the radius for the line cases, `shift` an in-plane
/// offset of the plane's origin so it is not on the axis, `flip` the
/// normal's sign.
#[derive(Debug, Clone, Copy)]
struct Raw {
    height: f64,
    phase: f64,
    angle: f64,
    fraction: f64,
    shift: f64,
    flip: bool,
}

fn raw() -> impl Strategy<Value = Raw> {
    (
        finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
        finite_f64(0.0..=TAU),
        finite_f64(0.1..=FRAC_PI_2 - 0.1),
        finite_f64(0.0..=0.9),
        finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
        any::<bool>(),
    )
        .prop_map(|(height, phase, angle, fraction, shift, flip)| Raw {
            height,
            phase,
            angle,
            fraction,
            shift,
            flip,
        })
}

fn case() -> impl Strategy<Value = Case> {
    prop_oneof![
        Just(Case::Circle),
        Just(Case::Ellipse),
        Just(Case::TwoLines),
        Just(Case::TangentLine),
        Just(Case::Empty),
    ]
}

/// The plane for `case`, and what the result must be: the centre on the
/// axis (circle, ellipse), the foot of the axis (lines) and the major
/// axis direction (ellipse).
fn realise(case: Case, cyl: &Surface, raw: Raw) -> (Surface, Point3, Vec3) {
    let Surface::Cylinder { frame, radius } = cyl else {
        unreachable!()
    };
    let (x, y, z) = (
        frame.x().into_inner(),
        frame.y().into_inner(),
        frame.z().into_inner(),
    );
    let around = raw.phase.cos() * x + raw.phase.sin() * y;
    let across = z.cross(&around);
    let on_axis = frame.origin() + raw.height * z;
    let sign = if raw.flip { -1.0 } else { 1.0 };
    let (normal, anchor, major) = match case {
        Case::Circle => (z, on_axis + raw.shift * around, around),
        Case::Ellipse => {
            let n = raw.angle.cos() * z + raw.angle.sin() * around;
            let major = raw.angle.sin() * z - raw.angle.cos() * around;
            (n, on_axis + raw.shift * across, major)
        }
        Case::TwoLines | Case::TangentLine | Case::Empty => {
            let d = match case {
                Case::TwoLines => raw.fraction * radius,
                Case::TangentLine => *radius,
                _ => radius * (1.1 + raw.fraction),
            };
            let foot = on_axis + d * around;
            (around, foot + raw.shift * across, around)
        }
    };
    let plane = Surface::Plane {
        frame: Frame::from_z(anchor, sign * normal).unwrap(),
    };
    // A line's origin is its point nearest the cylinder's origin: the foot
    // of that origin on the plane, then a half-chord across.
    let reference = match case {
        Case::Circle | Case::Ellipse => on_axis,
        _ => frame.origin() + (anchor - frame.origin()).dot(&around) * around,
    };
    (plane, reference, major)
}

#[test]
fn plane_cylinder_follows_the_case_table() {
    check((case(), cylinder(), raw()), |(case, cyl, raw)| {
        let (plane, reference, major) = realise(case, &cyl, raw);
        let Surface::Cylinder { frame, radius } = cyl else {
            unreachable!()
        };
        let r = common_properties(&plane, &cyl)?;
        match (case, &r) {
            (Case::Circle, SurfaceIntersection::Transversal(c)) => {
                let [
                    Curve::Circle {
                        frame: cf,
                        radius: cr,
                    },
                ] = c.as_slice()
                else {
                    return Err(TestCaseError::fail(format!("{case:?}: {r:?}")));
                };
                prop_assert!((cf.origin() - reference).norm() <= EXACT);
                prop_assert_eq!(*cr, radius);
                // The circle takes the cylinder's X so the seam is shared.
                prop_assert_eq!(cf.x(), frame.x());
                prop_assert_eq!(cf.z(), frame.z());
            }
            (Case::Ellipse, SurfaceIntersection::Transversal(c)) => {
                let [
                    Curve::Ellipse {
                        frame: ef,
                        major_radius,
                        minor_radius,
                    },
                ] = c.as_slice()
                else {
                    return Err(TestCaseError::fail(format!("{case:?}: {r:?}")));
                };
                prop_assert!((ef.origin() - reference).norm() <= EXACT);
                prop_assert!((minor_radius - radius).abs() <= EXACT);
                prop_assert!((major_radius - radius / raw.angle.cos()).abs() <= EXACT);
                prop_assert!(
                    (ef.x().into_inner() - major).norm() <= EXACT,
                    "major axis {:?} vs {major}",
                    ef.x()
                );
                prop_assert!(ef.x().dot(&frame.z()) > 0.0, "major axis points down v");
            }
            (Case::TwoLines, SurfaceIntersection::Transversal(c)) => {
                let [
                    Curve::Line {
                        origin: o1,
                        direction: d1,
                    },
                    Curve::Line {
                        origin: o2,
                        direction: d2,
                    },
                ] = c.as_slice()
                else {
                    return Err(TestCaseError::fail(format!("{case:?}: {r:?}")));
                };
                prop_assert_eq!(*d1, frame.z());
                prop_assert_eq!(*d2, frame.z());
                let half = (radius * radius - (raw.fraction * radius).powi(2)).sqrt();
                prop_assert!(((o1 - reference).norm() - half).abs() <= EXACT);
                prop_assert!(((o2 - reference).norm() - half).abs() <= EXACT);
                prop_assert!((o1 - o2).norm() >= 2.0 * half - EXACT);
            }
            (Case::TangentLine, SurfaceIntersection::Tangent(c)) => {
                let [Curve::Line { origin, direction }] = c.as_slice() else {
                    return Err(TestCaseError::fail(format!("{case:?}: {r:?}")));
                };
                prop_assert_eq!(*direction, frame.z());
                // The ruling through the point of the cylinder nearest the
                // plane: the foot of the axis, one radius out.
                prop_assert!((origin - reference).norm() <= EXACT);
                prop_assert!((implicit_distance(&cyl, *origin)) <= EXACT);
            }
            (Case::Empty, SurfaceIntersection::Empty) => {}
            _ => return Err(TestCaseError::fail(format!("{case:?} gave {r:?}"))),
        }
        Ok(())
    });
}

#[test]
fn random_plane_and_cylinder_agree_on_every_common_property() {
    check((plane(), cylinder()), |(p, c)| {
        common_properties(&p, &c)?;
        Ok(())
    });
}

#[test]
fn two_random_planes_meet_along_a_line_on_both() {
    check((plane(), plane()), |(a, b)| {
        let r = common_properties(&a, &b)?;
        let SurfaceIntersection::Transversal(c) = &r else {
            return Err(TestCaseError::fail(format!("{r:?}")));
        };
        let [Curve::Line { origin, direction }] = c.as_slice() else {
            return Err(TestCaseError::fail(format!("{r:?}")));
        };
        prop_assert!(direction.dot(&a.frame().unwrap().z()).abs() <= EXACT);
        prop_assert!(direction.dot(&b.frame().unwrap().z()).abs() <= EXACT);
        // The origin is the point of the line nearest a's origin.
        prop_assert!((origin - a.frame().unwrap().origin()).dot(direction).abs() <= EXACT);
        Ok(())
    });
}

#[test]
fn parallel_planes_are_coincident_or_empty_by_the_gap() {
    check(
        (
            plane(),
            point_in_box(DEFAULT_SCALE),
            finite_f64(0.01..=DEFAULT_SCALE),
            any::<bool>(),
            any::<bool>(),
        ),
        |(a, anchor, gap, flip, lift)| {
            let n = a.frame().unwrap().z().into_inner();
            let in_plane = anchor - n.dot(&(anchor - a.frame().unwrap().origin())) * n;
            let origin = if lift { in_plane + gap * n } else { in_plane };
            let sign = if flip { -1.0 } else { 1.0 };
            let b = Surface::Plane {
                frame: Frame::from_z(origin, sign * n).unwrap(),
            };
            let r = common_properties(&a, &b)?;
            let expected = if lift {
                SurfaceIntersection::Empty
            } else {
                SurfaceIntersection::Coincident
            };
            prop_assert_eq!(r, expected);
            Ok(())
        },
    );
}

#[test]
fn every_other_pair_is_unsupported() {
    check((surface(), surface()), |(a, b)| {
        let closed_form = |k| matches!(k, SurfaceKind::Plane | SurfaceKind::Cylinder);
        let two_cylinders = a.kind() == SurfaceKind::Cylinder && b.kind() == SurfaceKind::Cylinder;
        // Two cylinders have a closed form only when they are coaxial;
        // two in random poses never are, but the property says so rather
        // than relying on it.
        let plane_and_cylinder =
            closed_form(a.kind()) && closed_form(b.kind()) && (!two_cylinders || coaxial(&a, &b));
        match intersect_surfaces(&a, &b, tol()) {
            Ok(_) => prop_assert!(plane_and_cylinder, "{a:?} vs {b:?} should be unsupported"),
            Err(GeomError::Unsupported { a: ka, b: kb }) => {
                prop_assert!(!plane_and_cylinder, "{a:?} vs {b:?} has a closed form");
                prop_assert_eq!(ka, GeomKind::Surface(a.kind()));
                prop_assert_eq!(kb, GeomKind::Surface(b.kind()));
            }
            Err(e) => prop_assert!(false, "{e}"),
        }
        Ok(())
    });
}

#[test]
fn a_tilted_plane_is_oblique_never_a_guess() {
    // A normal a hair off the axis is oblique, not a circle: the plan's
    // tolerance is angular and the ellipse it returns is the exact section.
    check((cylinder(), unit_vec3()), |(cyl, n)| {
        let axis = cyl.frame().unwrap().z();
        let tilt = n.cross(&axis).norm().atan2(n.dot(&axis).abs());
        let plane = Surface::Plane {
            frame: Frame::from_z(cyl.frame().unwrap().origin(), n.into_inner()).unwrap(),
        };
        let r = common_properties(&plane, &cyl)?;
        if tilt > tol().angular && FRAC_PI_2 - tilt > tol().angular {
            let oblique = matches!(&r, SurfaceIntersection::Transversal(c) if matches!(c.as_slice(), [Curve::Ellipse { .. }]));
            prop_assert!(oblique, "tilt {tilt} gave {:?}", r);
        }
        Ok(())
    });
}

/// The faces of `tests/fixtures/boolean/through-hole`: a box
/// `[0,0,0]–[40,30,10]` minus a cylinder of radius 4 at `(20, 15)` along
/// `z`. The four walls are clear of the hole; the two caps cut circles.
#[test]
fn through_hole_faces_against_the_hole() {
    let hole = Surface::Cylinder {
        frame: Frame::from_z(Point3::new(20.0, 15.0, -1.0), Vec3::z()).unwrap(),
        radius: 4.0,
    };
    let wall = |origin, normal| Surface::Plane {
        frame: Frame::from_z(origin, normal).unwrap(),
    };
    for (origin, normal) in [
        (Point3::new(0.0, 0.0, 0.0), -Vec3::x()),
        (Point3::new(40.0, 0.0, 0.0), Vec3::x()),
        (Point3::new(0.0, 0.0, 0.0), -Vec3::y()),
        (Point3::new(0.0, 30.0, 0.0), Vec3::y()),
    ] {
        assert_eq!(
            intersect_surfaces(&wall(origin, normal), &hole, tol()).unwrap(),
            SurfaceIntersection::Empty
        );
    }
    for (z, normal) in [(0.0, -Vec3::z()), (10.0, Vec3::z())] {
        let cap = wall(Point3::new(0.0, 0.0, z), normal);
        let SurfaceIntersection::Transversal(c) = intersect_surfaces(&cap, &hole, tol()).unwrap()
        else {
            panic!()
        };
        let [Curve::Circle { frame, radius }] = c.as_slice() else {
            panic!()
        };
        assert_eq!(*radius, 4.0);
        assert_eq!(frame.origin(), Point3::new(20.0, 15.0, z));
        assert_eq!(frame.z(), hole.frame().unwrap().z());
    }
}

// --- cylinder–cylinder ----------------------------------------------------

/// The two surfaces' axes are one line within the tolerance.
fn coaxial(a: &Surface, b: &Surface) -> bool {
    let (fa, fb) = (a.frame().unwrap(), b.frame().unwrap());
    let parallel = fa
        .z()
        .cross(&fb.z())
        .norm()
        .atan2(fa.z().dot(&fb.z()).abs())
        <= tol().angular;
    parallel && (fb.origin() - fa.origin()).cross(&fa.z()).norm() <= tol().linear
}

/// One cylinder, and a second on the same axis: the same radius, or a
/// different one, at a random slide along the axis and a random phase.
fn coaxial_pair() -> impl Strategy<Value = (Surface, Surface, bool)> {
    (
        cylinder(),
        finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
        finite_f64(0.0..=TAU),
        finite_f64(0.2..=4.0),
        any::<bool>(),
    )
        .prop_filter_map(
            "a second cylinder on the same axis",
            |(a, slide, phase, factor, same)| {
                let (frame, radius) = match a {
                    Surface::Cylinder { frame, radius } => (frame, radius),
                    _ => return None,
                };
                let origin = frame.origin() + slide * frame.z().into_inner();
                let x = phase.cos() * frame.x().into_inner() + phase.sin() * frame.y().into_inner();
                let other = Surface::Cylinder {
                    frame: Frame::new(origin, frame.z().into_inner(), x).ok()?,
                    radius: if same { radius } else { factor * radius },
                };
                // A "different" radius within the tolerance is the same
                // cylinder; the case is the one the radii say it is.
                let agree = match other {
                    Surface::Cylinder { radius: r, .. } => (r - radius).abs() <= tol().linear,
                    _ => false,
                };
                Some((a, other, agree))
            },
        )
}

#[test]
fn coaxial_cylinders_are_coincident_or_empty_by_their_radii() {
    check(coaxial_pair(), |(a, b, agree)| {
        let expected = if agree {
            SurfaceIntersection::Coincident
        } else {
            SurfaceIntersection::Empty
        };
        prop_assert_eq!(
            intersect_surfaces(&a, &b, tol()).map_err(|e| TestCaseError::fail(e.to_string()))?,
            expected.clone(),
            "{:?} vs {:?}",
            a,
            b
        );
        // Symmetric, and the same on a second run.
        prop_assert_eq!(
            intersect_surfaces(&b, &a, tol()).map_err(|e| TestCaseError::fail(e.to_string()))?,
            expected
        );
        Ok(())
    });
}

#[test]
fn cylinders_that_are_not_coaxial_are_unsupported_naming_the_pair() {
    check((cylinder(), cylinder()), |(a, b)| {
        if coaxial(&a, &b) {
            return Ok(());
        }
        match intersect_surfaces(&a, &b, tol()) {
            Err(GeomError::Unsupported { a: ka, b: kb }) => {
                prop_assert_eq!(ka, GeomKind::Surface(SurfaceKind::Cylinder));
                prop_assert_eq!(kb, GeomKind::Surface(SurfaceKind::Cylinder));
                Ok(())
            }
            other => Err(TestCaseError::fail(format!("{a:?} vs {b:?}: {other:?}"))),
        }
    });
}
