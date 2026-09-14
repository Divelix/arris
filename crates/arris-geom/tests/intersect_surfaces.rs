//! Plane–plane, plane–cylinder and cylinder–cylinder intersections follow
//! the case table in any pose: the variant is the constructed case's,
//! every result curve lies on both surfaces, the ellipses' axes are the
//! closed form, a tangent line is the ruling at the touch, the result is
//! symmetric under swapping and bit-identical across runs, and every other
//! surface pair — two cylinders in a quartic pose among them — is
//! `Unsupported`.

use core::f64::consts::{FRAC_PI_2, TAU};

use arris_debug::prop::geom::{RADIUS_RANGE, cylinder, plane, surface};
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
/// surfaces, symmetry under swapping (the same curves, in any order: two
/// parallel cylinders order their rulings from the first), determinism.
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
    let mut taken = vec![false; ss.len()];
    for c in cs {
        let found = ss
            .iter()
            .enumerate()
            .position(|(i, s)| !taken[i] && same_curve(c, s));
        prop_assert!(
            found.is_some(),
            "swap changed a curve: {:?} not in {:?}",
            c,
            ss
        );
        if let Some(i) = found {
            taken[i] = true;
        }
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
        // Two cylinders in random poses are almost always skew, apart or
        // within the radii; the property decides the pose rather than
        // relying on that.
        let supported = closed_form(a.kind())
            && closed_form(b.kind())
            && (!two_cylinders || !pose(&a, &b).is_quartic());
        match intersect_surfaces(&a, &b, tol()) {
            Ok(_) => prop_assert!(supported, "{a:?} vs {b:?} should be unsupported"),
            Err(GeomError::Unsupported { a: ka, b: kb }) => {
                prop_assert!(!supported, "{a:?} vs {b:?} has a closed form");
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

/// The pose of two cylinders' axes, as the case table reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Pose {
    Parallel,
    CrossingEqual,
    CrossingUnequal,
    SkewApart,
    SkewClose,
}

impl Pose {
    /// The poses whose curve is a quartic, with no closed form.
    fn is_quartic(self) -> bool {
        matches!(self, Pose::CrossingUnequal | Pose::SkewClose)
    }
}

fn pose(a: &Surface, b: &Surface) -> Pose {
    let (
        Surface::Cylinder {
            frame: fa,
            radius: ra,
        },
        Surface::Cylinder {
            frame: fb,
            radius: rb,
        },
    ) = (a, b)
    else {
        unreachable!("two cylinders")
    };
    let cross = fa.z().cross(&fb.z());
    if cross.norm().atan2(fa.z().dot(&fb.z()).abs()) <= tol().angular {
        return Pose::Parallel;
    }
    let gap = cross.normalize().dot(&(fb.origin() - fa.origin())).abs();
    match (gap > tol().linear, (ra - rb).abs() <= tol().linear) {
        (true, _) if gap > ra + rb + tol().linear => Pose::SkewApart,
        (true, _) => Pose::SkewClose,
        (false, true) => Pose::CrossingEqual,
        (false, false) => Pose::CrossingUnequal,
    }
}

/// The unit vector at `angle` off `frame`'s `Z`, at `phase` around it.
fn tilted(frame: &Frame, angle: f64, phase: f64) -> Vec3 {
    let around = phase.cos() * frame.x().into_inner() + phase.sin() * frame.y().into_inner();
    angle.cos() * frame.z().into_inner() + angle.sin() * around
}

/// The case of two parallel cylinders to realise.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Parallel {
    Two,
    Outside,
    Inside,
    Apart,
    Nested,
}

/// A cylinder and a second parallel to it realising the case: its axis
/// `d` away at `phase`, reversed or not, its origin slid along the axis,
/// its radius drawn apart. `fraction` places `d` between the tangent
/// distances (clear of each), beyond them or inside the smaller.
#[allow(clippy::type_complexity)]
fn parallel_pair() -> impl Strategy<Value = (Parallel, Surface, Surface)> {
    (
        prop_oneof![
            Just(Parallel::Two),
            Just(Parallel::Outside),
            Just(Parallel::Inside),
            Just(Parallel::Apart),
            Just(Parallel::Nested),
        ],
        cylinder(),
        finite_f64(RADIUS_RANGE),
        finite_f64(0.0..=TAU),
        finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
        finite_f64(0.05..=0.95),
        any::<bool>(),
        finite_f64(0.0..=TAU),
    )
        .prop_filter_map(
            "radii far enough apart for a case inside the larger",
            |(case, a, rb, phase, slide, fraction, flip, x_phase)| {
                let Surface::Cylinder { frame, radius: ra } = a else {
                    return None;
                };
                let (lo, hi) = ((ra - rb).abs(), ra + rb);
                if matches!(case, Parallel::Inside | Parallel::Nested) && lo < 0.05 * hi {
                    return None;
                }
                let d = match case {
                    Parallel::Two => lo + fraction * (hi - lo),
                    Parallel::Outside => hi,
                    Parallel::Inside => lo,
                    Parallel::Apart => hi * (1.1 + fraction),
                    Parallel::Nested => lo * fraction,
                };
                let z = frame.z().into_inner();
                let towards = tilted(&frame, FRAC_PI_2, phase);
                let origin = frame.origin() + d * towards + slide * z;
                let x = tilted(&frame, FRAC_PI_2, x_phase);
                let b = Surface::Cylinder {
                    frame: Frame::new(origin, if flip { -z } else { z }, x).ok()?,
                    radius: rb,
                };
                Some((case, Surface::Cylinder { frame, radius: ra }, b))
            },
        )
}

#[test]
fn parallel_cylinders_follow_the_case_table() {
    check(parallel_pair(), |(case, a, b)| {
        let r = common_properties(&a, &b)?;
        let fa = a.frame().unwrap();
        let z = fa.z();
        let rulings = match (case, &r) {
            (Parallel::Two, SurfaceIntersection::Transversal(c)) if c.len() == 2 => c,
            (Parallel::Outside | Parallel::Inside, SurfaceIntersection::Tangent(c))
                if c.len() == 1 =>
            {
                c
            }
            (Parallel::Apart | Parallel::Nested, SurfaceIntersection::Empty) => return Ok(()),
            _ => return Err(TestCaseError::fail(format!("{case:?} gave {r:?}"))),
        };
        let mut origins = Vec::new();
        for c in rulings {
            let Curve::Line { origin, direction } = c else {
                return Err(TestCaseError::fail(format!("{case:?}: {c:?}")));
            };
            // Along the first cylinder's Z, from the point nearest its
            // origin.
            prop_assert_eq!(*direction, z);
            prop_assert!((origin - fa.origin()).dot(&z).abs() <= EXACT);
            origins.push(*origin);
        }
        if let [o1, o2] = origins.as_slice() {
            // Ordered along Z × ŵ, ŵ from the first axis toward the second.
            let offset = b.frame().unwrap().origin() - fa.origin();
            let towards = offset - offset.dot(&z) * z.into_inner();
            let side = z.cross(&towards);
            prop_assert!((o2 - o1).dot(&side) > 0.0, "{o1} then {o2}");
        }
        Ok(())
    });
}

/// A cylinder and a second of the same radius whose axis crosses the
/// first's at a point slid along it, at an angle in [10°, 90°], reversed
/// or not, the second's origin slid along its own axis.
fn crossing_pair() -> impl Strategy<Value = (Surface, Surface, f64, Point3)> {
    (
        cylinder(),
        finite_f64(10f64.to_radians()..=FRAC_PI_2),
        finite_f64(0.0..=TAU),
        finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
        finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
        any::<bool>(),
    )
        .prop_filter_map(
            "a second cylinder crossing the first",
            |(a, psi, phase, slide, slide_b, flip)| {
                let Surface::Cylinder { frame, radius } = a else {
                    return None;
                };
                let z = frame.z().into_inner();
                let axis = tilted(&frame, psi, phase);
                let axis = if flip { -axis } else { axis };
                let crossing = frame.origin() + slide * z;
                let b = Surface::Cylinder {
                    frame: Frame::new(crossing + slide_b * axis, axis, z.cross(&axis)).ok()?,
                    radius,
                };
                Some((Surface::Cylinder { frame, radius }, b, psi, crossing))
            },
        )
}

#[test]
fn equal_cylinders_crossing_meet_in_the_two_bisecting_ellipses() {
    check(crossing_pair(), |(a, b, psi, crossing)| {
        let r = common_properties(&a, &b)?;
        let (fa, fb) = (a.frame().unwrap(), b.frame().unwrap());
        let Surface::Cylinder { radius, .. } = a else {
            unreachable!()
        };
        let SurfaceIntersection::Transversal(c) = &r else {
            return Err(TestCaseError::fail(format!("{r:?}")));
        };
        let [first, second] = c.as_slice() else {
            return Err(TestCaseError::fail(format!("{r:?}")));
        };
        // Where the two ellipses cross: R along the axes' common normal.
        let normal = fa.z().cross(&fb.z()).normalize();
        let meets = [crossing + radius * normal, crossing - radius * normal];
        for (ellipse, major) in [
            (first, radius / (psi / 2.0).sin()),
            (second, radius / (psi / 2.0).cos()),
        ] {
            let Curve::Ellipse {
                frame,
                major_radius,
                minor_radius,
            } = ellipse
            else {
                return Err(TestCaseError::fail(format!("{ellipse:?}")));
            };
            let scale = major.max(1.0);
            prop_assert!((frame.origin() - crossing).norm() <= EXACT);
            prop_assert_eq!(*minor_radius, radius);
            prop_assert!(
                (major_radius - major).abs() <= EXACT * scale,
                "{major_radius} vs {major}"
            );
            prop_assert!(frame.x().dot(&fa.z()) > 0.0, "X points down v");
            for p in meets {
                let d = ellipse
                    .project(p)
                    .map_err(|e| TestCaseError::fail(e.to_string()))?
                    .distance;
                prop_assert!(d <= EXACT * scale, "{p} is {d} off {ellipse:?}");
            }
        }
        Ok(())
    });
}

#[test]
fn skew_cylinders_further_apart_than_their_radii_are_empty() {
    check(
        (
            cylinder(),
            finite_f64(RADIUS_RANGE),
            finite_f64(0.05..=FRAC_PI_2),
            finite_f64(0.0..=TAU),
            finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
            finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
            finite_f64(0.05..=1.0),
        ),
        |(a, rb, psi, phase, slide, slide_b, fraction)| {
            let Surface::Cylinder { frame, radius: ra } = a else {
                unreachable!()
            };
            let z = frame.z().into_inner();
            let axis = tilted(&frame, psi, phase);
            let normal = z.cross(&axis).normalize();
            let gap = (ra + rb) * (1.0 + fraction);
            let origin = frame.origin() + slide * z + gap * normal + slide_b * axis;
            let b = Surface::Cylinder {
                frame: Frame::new(origin, axis, normal).unwrap(),
                radius: rb,
            };
            prop_assert_eq!(common_properties(&a, &b)?, SurfaceIntersection::Empty);
            Ok(())
        },
    );
}

/// A cylinder and a second in a quartic pose: its axis at an angle in
/// [0.1, π/2] off the first's, crossing it with a radius at least 1.2
/// times smaller or larger, or skew, its common perpendicular a fraction
/// of the two radii long.
fn quartic_pair() -> impl Strategy<Value = (Surface, Surface)> {
    (
        cylinder(),
        finite_f64(0.1..=FRAC_PI_2),
        finite_f64(0.0..=TAU),
        finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
        finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
        finite_f64(0.05..=0.95),
        finite_f64(1.2..=4.0),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_filter_map(
            "a second cylinder in a quartic pose",
            |(a, psi, phase, slide, slide_b, fraction, factor, skew, shrink)| {
                let Surface::Cylinder { frame, radius } = a else {
                    return None;
                };
                let z = frame.z().into_inner();
                let axis = tilted(&frame, psi, phase);
                let normal = z.cross(&axis).normalize();
                let rb = if shrink {
                    radius / factor
                } else {
                    radius * factor
                };
                let gap = if skew { fraction * (radius + rb) } else { 0.0 };
                let origin = frame.origin() + slide * z + gap * normal + slide_b * axis;
                let b = Surface::Cylinder {
                    frame: Frame::new(origin, axis, normal).ok()?,
                    radius: rb,
                };
                Some((Surface::Cylinder { frame, radius }, b))
            },
        )
}

#[test]
fn cylinders_in_a_quartic_pose_are_unsupported_naming_the_pair() {
    check(quartic_pair(), |(a, b)| {
        prop_assert!(pose(&a, &b).is_quartic(), "{a:?} vs {b:?}");
        for (x, y) in [(&a, &b), (&b, &a)] {
            match intersect_surfaces(x, y, tol()) {
                Err(GeomError::Unsupported { a: ka, b: kb }) => {
                    prop_assert_eq!(ka, GeomKind::Surface(SurfaceKind::Cylinder));
                    prop_assert_eq!(kb, GeomKind::Surface(SurfaceKind::Cylinder));
                }
                other => return Err(TestCaseError::fail(format!("{x:?} vs {y:?}: {other:?}"))),
            }
        }
        Ok(())
    });
}
