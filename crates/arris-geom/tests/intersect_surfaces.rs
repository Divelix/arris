//! Plane–plane, plane–cylinder and cylinder–cylinder intersections follow
//! the case table in any pose: the variant is the constructed case's,
//! every result curve lies on both surfaces, the ellipses' axes are the
//! closed form, a tangent line is the ruling at the touch, the result is
//! symmetric under swapping and bit-identical across runs. Coaxial pairs
//! with a cone, a sphere or a torus in them meet where their meridians
//! meet (ADR-0008): every circle is centred on the axis and is a genuine
//! crossing of the meridians, no crossing a sampled meridian sees is
//! missed, constructed touches are `Tangent` and constructed apexes and
//! poles are `Points`. Every other surface pair — two cylinders in a
//! quartic pose, a plane oblique to a cone's axis, two tori on different
//! axes — is `Unsupported`.

use core::f64::consts::{FRAC_PI_2, TAU};

use arris_debug::prop::geom::{HALF_ANGLE_RANGE, RADIUS_RANGE, cylinder, plane, surface};
use arris_debug::prop::{DEFAULT_SCALE, check, finite_f64, frame, point_in_box, unit_vec3};
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

/// The signed distance from `p` to the surface by its implicit form:
/// negative on a plane's back, inside a cylinder, a sphere or a torus's
/// tube, and inside a double cone's nappes.
fn signed_distance(s: &Surface, p: Point3) -> f64 {
    let q = s.frame().unwrap().to_local(p);
    let rho = q.x.hypot(q.y);
    match *s {
        Surface::Plane { .. } => q.z,
        Surface::Cylinder { radius, .. } => rho - radius,
        Surface::Cone {
            radius, half_angle, ..
        } => {
            let (sa, ca) = half_angle.sin_cos();
            let apex = -radius * ca / sa;
            rho * ca - (q.z - apex).abs() * sa
        }
        Surface::Sphere { radius, .. } => q.coords.norm() - radius,
        Surface::Torus {
            major_radius,
            minor_radius,
            ..
        } => (rho - major_radius).hypot(q.z) - minor_radius,
        Surface::Nurbs(_) => unreachable!("no NURBS pair has a closed form"),
    }
}

/// The distance from `p` to the surface by its implicit form.
fn implicit_distance(s: &Surface, p: Point3) -> f64 {
    signed_distance(s, p).abs()
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
        SurfaceIntersection::Empty
        | SurfaceIntersection::Coincident
        | SurfaceIntersection::Points(_) => &[],
    }
}

fn points_of(r: &SurfaceIntersection) -> &[Point3] {
    match r {
        SurfaceIntersection::Points(p) => p,
        SurfaceIntersection::Transversal(_)
        | SurfaceIntersection::Tangent(_)
        | SurfaceIntersection::Empty
        | SurfaceIntersection::Coincident => &[],
    }
}

/// The checks every result passes whatever its case: curves and points
/// on both surfaces, symmetry under swapping (the same curves and points,
/// in any order: two parallel cylinders order their rulings from the
/// first, a coaxial pair its circles along the first's axis),
/// determinism.
fn common_properties(a: &Surface, b: &Surface) -> Result<SurfaceIntersection, TestCaseError> {
    let r = intersect_surfaces(a, b, tol()).map_err(|e| TestCaseError::fail(e.to_string()))?;
    for c in curves_of(&r) {
        on_both(c, a, b)?;
    }
    for &p in points_of(&r) {
        let (da, db) = (implicit_distance(a, p), implicit_distance(b, p));
        prop_assert!(
            da <= tol().linear && db <= tol().linear,
            "{p} is off by {da} from {a:?} and {db} from {b:?}"
        );
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
    let (ps, qs) = (points_of(&r), points_of(&swapped));
    prop_assert_eq!(ps.len(), qs.len());
    for p in ps {
        prop_assert!(
            qs.iter().any(|q| (p - q).norm() <= EXACT),
            "swap changed a point: {} not in {:?}",
            p,
            qs
        );
    }
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

/// Whether two surfaces share an axis as the coaxial arm reads it
/// (`docs/DATA-MODEL.md` §Curves): a cylinder, cone or torus carries its
/// `Z`; two carriers are coaxial when the axes are parallel within the
/// angular tolerance and the second's origin is within the linear one of
/// the first's axis; a plane is coaxial with a carrier its normal is
/// parallel to; a sphere with a carrier whose axis passes through its
/// centre; every plane–sphere and sphere–sphere pair shares an axis.
fn coaxial(a: &Surface, b: &Surface) -> bool {
    let carries = |s: &Surface| {
        matches!(
            s.kind(),
            SurfaceKind::Cylinder | SurfaceKind::Cone | SurfaceKind::Torus
        )
    };
    let parallel =
        |x: &UnitVec3, y: &UnitVec3| x.cross(y).norm().atan2(x.dot(y).abs()) <= tol().angular;
    let on_axis = |f: &Frame, p: Point3| {
        let d = p - f.origin();
        (d - d.dot(&f.z()) * f.z().into_inner()).norm() <= tol().linear
    };
    let (fa, fb) = (a.frame().unwrap(), b.frame().unwrap());
    match (carries(a), carries(b)) {
        (true, true) => parallel(&fa.z(), &fb.z()) && on_axis(fa, fb.origin()),
        (true, false) | (false, true) => {
            let (carrier, other) = if carries(a) { (fa, b) } else { (fb, a) };
            match other.kind() {
                SurfaceKind::Plane => {
                    let plane = other.frame().unwrap();
                    let across = FRAC_PI_2
                        - plane
                            .z()
                            .cross(&carrier.z())
                            .norm()
                            .atan2(plane.z().dot(&carrier.z()).abs())
                        <= tol().angular;
                    let holds =
                        plane.z().dot(&(carrier.origin() - plane.origin())).abs() <= tol().linear;
                    parallel(&plane.z(), &carrier.z()) || (across && holds)
                }
                SurfaceKind::Sphere => on_axis(carrier, other.frame().unwrap().origin()),
                _ => unreachable!(),
            }
        }
        (false, false) => true,
    }
}

#[test]
fn every_other_pair_is_unsupported() {
    check((surface(), surface()), |(a, b)| {
        let closed_form = |k| matches!(k, SurfaceKind::Plane | SurfaceKind::Cylinder);
        let two_cylinders = a.kind() == SurfaceKind::Cylinder && b.kind() == SurfaceKind::Cylinder;
        // Two cylinders in random poses are almost always skew, apart or
        // within the radii, and a pair with a cone, a sphere or a torus
        // in it is almost never coaxial; the property decides the pose
        // rather than relying on that.
        let supported = if closed_form(a.kind()) && closed_form(b.kind()) {
            !two_cylinders || !pose(&a, &b).is_quartic()
        } else {
            coaxial(&a, &b)
        };
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
            // Z and X signed with their largest-magnitude component positive.
            for v in [frame.z().into_inner(), frame.x().into_inner()] {
                let k = (0..3).fold(0, |k, i| if v[i].abs() > v[k].abs() { i } else { k });
                prop_assert!(v[k] > 0.0, "{v} is not canonically signed");
            }
            for p in meets {
                let d = ellipse
                    .project(p)
                    .map_err(|e| TestCaseError::fail(e.to_string()))?
                    .distance;
                prop_assert!(d <= EXACT * scale, "{p} is {d} off {ellipse:?}");
            }
        }
        // Either operand order: the same two ellipses bit for bit, in the
        // same order, so a boolean fits each pcurve once whichever it is.
        let swapped = intersect_surfaces(&b, &a, Precision::DEFAULT.tolerance())
            .map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(&swapped, &r, "swapping the operands");
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

// --- coaxial surfaces of revolution (ADR-0008) ------------------------------

/// The kind of operand to place on the shared axis.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Coaxial {
    Plane,
    Cylinder,
    Cone,
    Sphere,
    Torus,
}

fn coaxial_kind() -> impl Strategy<Value = Coaxial> {
    prop_oneof![
        Just(Coaxial::Plane),
        Just(Coaxial::Cylinder),
        Just(Coaxial::Cone),
        Just(Coaxial::Sphere),
        Just(Coaxial::Torus),
    ]
}

/// Where an operand sits on the axis: slid `slide` along it, its own
/// frame at `phase` about it and reversed by `flip`; a plane's origin
/// `shift` off the axis in its own plane; a sphere's own `Z` off the axis
/// by `tilt` unless `aligned`; radii `r1` and `r2` and a cone's `angle`.
#[derive(Debug, Clone, Copy)]
struct Placement {
    slide: f64,
    phase: f64,
    flip: bool,
    shift: f64,
    tilt: f64,
    aligned: bool,
    r1: f64,
    r2: f64,
    angle: f64,
}

fn placement() -> impl Strategy<Value = Placement> {
    (
        finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
        finite_f64(0.0..=TAU),
        any::<bool>(),
        finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
        finite_f64(0.1..=FRAC_PI_2 - 0.1),
        any::<bool>(),
        finite_f64(RADIUS_RANGE),
        finite_f64(RADIUS_RANGE),
        finite_f64(HALF_ANGLE_RANGE),
    )
        .prop_map(
            |(slide, phase, flip, shift, tilt, aligned, r1, r2, angle)| Placement {
                slide,
                phase,
                flip,
                shift,
                tilt,
                aligned,
                r1,
                r2,
                angle,
            },
        )
}

/// A surface of `kind` on `axis`'s `Z`, placed by `p`.
fn on_axis(kind: Coaxial, axis: &Frame, p: Placement) -> Surface {
    let z = axis.z().into_inner();
    let sign = if p.flip { -1.0 } else { 1.0 };
    let around = tilted(axis, FRAC_PI_2, p.phase);
    let origin = axis.origin() + p.slide * z;
    let frame = Frame::new(origin, sign * z, around).unwrap();
    match kind {
        Coaxial::Plane => Surface::Plane {
            frame: frame.with_origin(origin + p.shift * around),
        },
        Coaxial::Cylinder => Surface::Cylinder {
            frame,
            radius: p.r1,
        },
        Coaxial::Cone => Surface::Cone {
            frame,
            radius: p.r1,
            half_angle: p.angle,
        },
        Coaxial::Sphere => {
            let own = if p.aligned {
                sign * z
            } else {
                tilted(axis, p.tilt, p.phase)
            };
            Surface::Sphere {
                frame: Frame::new(origin, own, tilted(axis, FRAC_PI_2, p.phase + 1.0)).unwrap(),
                radius: p.r1,
            }
        }
        Coaxial::Torus => Surface::Torus {
            frame,
            major_radius: p.r1 + p.r2,
            minor_radius: p.r1,
        },
    }
}

/// The meridian of `s` in the half-plane through `axis`'s `Z` along its
/// `X`, sampled as 3D points: a plane's ray from the axis, a cylinder's
/// ruling, a cone's two nappes as one V through the apex, a sphere's
/// half circle, a torus's outer tube circle. `t` is the parameter the
/// meridian is walked by, and [`meridian_at`] evaluates it.
fn meridian_at(s: &Surface, axis: &Frame, t: f64) -> Point3 {
    let z = axis.z().into_inner();
    let x = axis.x().into_inner();
    let height = |p: Point3| (p - axis.origin()).dot(&z);
    let at = |rho: f64, h: f64| axis.origin() + rho * x + h * z;
    match *s {
        Surface::Plane { ref frame } => at(t, height(frame.origin())),
        Surface::Cylinder { radius, .. } => at(radius, t),
        Surface::Cone {
            ref frame,
            radius,
            half_angle,
        } => {
            let (sa, ca) = half_angle.sin_cos();
            let apex = height(frame.origin() - (radius * ca / sa) * frame.z().into_inner());
            at((t - apex).abs() * sa / ca, t)
        }
        Surface::Sphere { ref frame, radius } => {
            at(radius * t.cos(), height(frame.origin()) + radius * t.sin())
        }
        Surface::Torus {
            ref frame,
            major_radius,
            minor_radius,
        } => at(
            major_radius + minor_radius * t.cos(),
            height(frame.origin()) + minor_radius * t.sin(),
        ),
        Surface::Nurbs(_) => unreachable!(),
    }
}

/// The range of the meridian parameter that reaches every meeting the
/// strategies can make: the slides, the radii and a cone's apex offset
/// are all within a few scales, and a cone meets a plane at most a few
/// scales further out.
fn meridian_range(s: &Surface) -> (f64, f64) {
    match s {
        Surface::Plane { .. } => (0.0, 100.0 * DEFAULT_SCALE),
        Surface::Cylinder { .. } | Surface::Cone { .. } => {
            (-8.0 * DEFAULT_SCALE, 8.0 * DEFAULT_SCALE)
        }
        Surface::Sphere { .. } => (-FRAC_PI_2, FRAC_PI_2),
        Surface::Torus { .. } => (0.0, TAU),
        Surface::Nurbs(_) => unreachable!(),
    }
}

/// The meridian parameter of a point on `s`'s meridian.
fn meridian_param(s: &Surface, axis: &Frame, p: Point3) -> f64 {
    let z = axis.z().into_inner();
    let x = axis.x().into_inner();
    let d = p - axis.origin();
    let (rho, h) = (d.dot(&x), d.dot(&z));
    let height = |q: Point3| (q - axis.origin()).dot(&z);
    match *s {
        Surface::Plane { .. } => rho,
        Surface::Cylinder { .. } | Surface::Cone { .. } => h,
        Surface::Sphere { ref frame, .. } => (h - height(frame.origin())).atan2(rho),
        Surface::Torus {
            ref frame,
            major_radius,
            ..
        } => (h - height(frame.origin())).atan2(rho - major_radius),
        Surface::Nurbs(_) => unreachable!(),
    }
}

/// Samples along a meridian.
const MERIDIAN_SAMPLES: usize = 8192;
/// How far a meeting the sampler brackets may be from the chord of the
/// bracket: the chord's deviation from a sphere's or a torus's meridian
/// at this sampling is below 1e-6.
const BRACKET: f64 = 1e-5 * DEFAULT_SCALE;
/// The step along a meridian either side of a crossing, where the
/// other surface's signed distance changes sign.
const ACROSS: f64 = 1e-6;

/// Every result circle is centred on the axis about it, every result
/// point is on the axis, every `Transversal` circle is a crossing of the
/// meridians and every `Tangent` one is not, and every crossing the
/// sampled meridian of `a` sees against `b` is one of the result's.
fn meets_where_the_meridians_meet(
    a: &Surface,
    b: &Surface,
    axis: &Frame,
    r: &SurfaceIntersection,
) -> Result<(), TestCaseError> {
    let z = axis.z().into_inner();
    let off_axis = |p: Point3| (p - axis.origin()).cross(&z).norm();
    let x = axis.x().into_inner();
    // The result's meetings in the half-plane, as 3D points on `a`'s
    // meridian.
    let mut meetings: Vec<Point3> = Vec::new();
    for c in curves_of(r) {
        let Curve::Circle { frame, radius } = c else {
            return Err(TestCaseError::fail(format!("{c:?} is no circle")));
        };
        prop_assert!(off_axis(frame.origin()) <= EXACT, "{c:?} is off the axis");
        prop_assert!(
            frame.z().cross(&z).norm() <= EXACT,
            "{c:?} is not about the axis"
        );
        let on_meridian = frame.origin() + *radius * x;
        let t = meridian_param(a, axis, on_meridian);
        let before = signed_distance(b, meridian_at(a, axis, t - ACROSS));
        let after = signed_distance(b, meridian_at(a, axis, t + ACROSS));
        match r {
            SurfaceIntersection::Transversal(_) => prop_assert!(
                before * after < 0.0,
                "{c:?} is no crossing: {before} and {after} either side"
            ),
            SurfaceIntersection::Tangent(_) => prop_assert!(
                before * after > 0.0,
                "{c:?} is a crossing: {before} and {after} either side"
            ),
            _ => unreachable!(),
        }
        meetings.push(on_meridian);
    }
    for &p in points_of(r) {
        prop_assert!(off_axis(p) <= EXACT, "{p} is off the axis");
        meetings.push(p);
    }
    let (lo, hi) = meridian_range(a);
    let mut previous: Option<(Point3, f64)> = None;
    for i in 0..=MERIDIAN_SAMPLES {
        let t = lo + (hi - lo) * i as f64 / MERIDIAN_SAMPLES as f64;
        let p = meridian_at(a, axis, t);
        let f = signed_distance(b, p);
        if let Some((q, g)) = previous {
            if g * f < 0.0 || g == 0.0 {
                let near = meetings.iter().any(|m| {
                    let (d, v) = (p - q, m - q);
                    let s = (v.dot(&d) / d.dot(&d)).clamp(0.0, 1.0);
                    (v - s * d).norm() <= BRACKET
                });
                prop_assert!(
                    near,
                    "the meridians of {a:?} and {b:?} cross between {q} and {p} but the result {r:?} has nothing there"
                );
            }
        }
        previous = Some((p, f));
    }
    Ok(())
}

#[test]
fn coaxial_pairs_meet_where_their_meridians_meet() {
    check(
        (
            frame(),
            coaxial_kind(),
            placement(),
            coaxial_kind(),
            placement(),
        ),
        |(axis, ka, pa, kb, pb)| {
            let (a, b) = (on_axis(ka, &axis, pa), on_axis(kb, &axis, pb));
            // Two planes and a plane against a cylinder are cycle 1's arms,
            // two cylinders the cylinder plan's: the closed forms this
            // property is about have a cone, a sphere or a torus in them.
            let quadric = |k| matches!(k, Coaxial::Cone | Coaxial::Sphere | Coaxial::Torus);
            prop_assume!(quadric(ka) || quadric(kb));
            let r = common_properties(&a, &b)?;
            meets_where_the_meridians_meet(&a, &b, &axis, &r)
        },
    );
}

/// The normals of both surfaces at `p`, parallel: what a touch means.
fn normals_parallel(a: &Surface, b: &Surface, p: Point3) -> Result<(), TestCaseError> {
    let normal = |s: &Surface| -> Result<UnitVec3, TestCaseError> {
        let uv = s
            .project(p)
            .map_err(|e| TestCaseError::fail(e.to_string()))?
            .uv;
        s.normal(uv.x, uv.y)
            .ok_or_else(|| TestCaseError::fail(format!("{s:?} is singular at {p}")))
    };
    let (na, nb) = (normal(a)?, normal(b)?);
    prop_assert!(
        na.cross(&nb).norm() <= 1e-9,
        "{a:?} and {b:?} are not tangent at {p}: normals {na:?} and {nb:?}"
    );
    Ok(())
}

/// The one circle of a `Tangent` result, or a failure naming it.
fn one_tangent_circle(r: &SurfaceIntersection) -> Result<(Point3, f64), TestCaseError> {
    match r {
        SurfaceIntersection::Tangent(c) => match c.as_slice() {
            [Curve::Circle { frame, radius }] => Ok((frame.origin(), *radius)),
            _ => Err(TestCaseError::fail(format!("{r:?}"))),
        },
        _ => Err(TestCaseError::fail(format!("{r:?}"))),
    }
}

/// The one point of a `Points` result, or a failure naming it.
fn one_point(r: &SurfaceIntersection) -> Result<Point3, TestCaseError> {
    match r {
        SurfaceIntersection::Points(p) if p.len() == 1 => Ok(p[0]),
        _ => Err(TestCaseError::fail(format!("{r:?}"))),
    }
}

#[test]
fn constructed_touches_are_tangent_and_touches_on_the_axis_are_points() {
    check(
        (frame(), placement(), placement(), any::<bool>()),
        |(axis, pa, pb, upper)| {
            let z = axis.z().into_inner();
            let sign = if upper { 1.0 } else { -1.0 };
            let torus = on_axis(Coaxial::Torus, &axis, pa);
            let Surface::Torus {
                frame: tf,
                major_radius: big,
                minor_radius: small,
            } = torus
            else {
                unreachable!()
            };
            // A torus on a plane at `z0 ± r`: the tube's top or bottom
            // circle, of radius `R`.
            let plane = Surface::Plane {
                frame: Frame::new(
                    tf.origin() + sign * small * z + pb.shift * tilted(&axis, FRAC_PI_2, pb.phase),
                    if pb.flip { -z } else { z },
                    tilted(&axis, FRAC_PI_2, pb.phase),
                )
                .unwrap(),
            };
            let r = common_properties(&torus, &plane)?;
            let (centre, radius) = one_tangent_circle(&r)?;
            prop_assert!((centre - (tf.origin() + sign * small * z)).norm() <= EXACT);
            prop_assert!((radius - big).abs() <= EXACT);
            normals_parallel(&torus, &plane, centre + radius * axis.x().into_inner())?;
            // A torus inside a cylinder of radius `R + r`, and around one of
            // `R − r`: the outer or the inner equator.
            let cylinder = Surface::Cylinder {
                frame: on_axis(Coaxial::Cylinder, &axis, pb)
                    .frame()
                    .unwrap()
                    .to_owned(),
                radius: big + sign * small,
            };
            let r = common_properties(&cylinder, &torus)?;
            let (centre, radius) = one_tangent_circle(&r)?;
            prop_assert!((centre - tf.origin()).norm() <= EXACT);
            prop_assert!((radius - (big + sign * small)).abs() <= EXACT);
            normals_parallel(&cylinder, &torus, centre + radius * axis.x().into_inner())?;
            // A sphere on a plane: the pole, one point on the axis.
            let sphere = on_axis(Coaxial::Sphere, &axis, pa);
            let Surface::Sphere {
                frame: sf,
                radius: ball,
            } = sphere
            else {
                unreachable!()
            };
            let cap = Surface::Plane {
                frame: plane.frame().unwrap().with_origin(
                    sf.origin() + sign * ball * z + pb.shift * tilted(&axis, FRAC_PI_2, pb.phase),
                ),
            };
            let r = common_properties(&sphere, &cap)?;
            let p = one_point(&r)?;
            prop_assert!((p - (sf.origin() + sign * ball * z)).norm() <= EXACT, "{p}");
            // A sphere on a cylinder of its radius: the equator.
            let hoop = Surface::Cylinder {
                frame: on_axis(Coaxial::Cylinder, &axis, pb)
                    .frame()
                    .unwrap()
                    .to_owned(),
                radius: ball,
            };
            let r = common_properties(&sphere, &hoop)?;
            let (centre, radius) = one_tangent_circle(&r)?;
            prop_assert!((centre - sf.origin()).norm() <= EXACT);
            prop_assert!((radius - ball).abs() <= EXACT);
            normals_parallel(&sphere, &hoop, centre + radius * axis.x().into_inner())?;
            // Two spheres touching from outside along a random direction,
            // and from inside.
            let direction = tilted(&axis, pb.tilt, pb.phase);
            for (distance, other) in [(ball + pb.r1, pb.r1), (ball - 0.5 * pb.r1, 0.5 * pb.r1)] {
                let touch = Surface::Sphere {
                    frame: Frame::new(
                        sf.origin() + distance * direction,
                        tilted(&axis, pb.tilt, pb.phase + 2.0),
                        z,
                    )
                    .unwrap(),
                    radius: other,
                };
                let r = common_properties(&sphere, &touch)?;
                let p = one_point(&r)?;
                prop_assert!(
                    (p - (sf.origin() + ball * direction)).norm() <= EXACT,
                    "{p}"
                );
            }
            Ok(())
        },
    );
}

#[test]
fn constructed_apexes_are_points_and_equal_cones_coincide() {
    check((frame(), placement(), placement()), |(axis, pa, pb)| {
        let z = axis.z().into_inner();
        let cone = on_axis(Coaxial::Cone, &axis, pa);
        let Surface::Cone {
            frame: cf,
            radius,
            half_angle,
        } = cone
        else {
            unreachable!()
        };
        let apex = cf.origin() - (radius / half_angle.tan()) * cf.z().into_inner();
        // A plane perpendicular to the axis through the apex.
        let through = Surface::Plane {
            frame: Frame::new(
                apex + pb.shift * tilted(&axis, FRAC_PI_2, pb.phase),
                if pb.flip { -z } else { z },
                tilted(&axis, FRAC_PI_2, pb.phase),
            )
            .unwrap(),
        };
        let p = one_point(&common_properties(&through, &cone)?)?;
        prop_assert!((p - apex).norm() <= EXACT, "{p} vs {apex}");
        // A second cone closing on the same apex at another angle,
        // opening either way: the apex alone.
        let other_angle = if (pb.angle - half_angle).abs() > 0.05 {
            pb.angle
        } else {
            (half_angle + 0.3).min(1.5)
        };
        let sign = if pb.flip { -1.0 } else { 1.0 };
        let other = |angle: f64| Surface::Cone {
            frame: Frame::new(
                apex + sign * (pb.r1 / angle.tan()) * z,
                sign * z,
                tilted(&axis, FRAC_PI_2, pb.phase),
            )
            .unwrap(),
            radius: pb.r1,
            half_angle: angle,
        };
        let p = one_point(&common_properties(&cone, &other(other_angle))?)?;
        prop_assert!((p - apex).norm() <= EXACT, "{p} vs {apex}");
        // The same apex at the same angle: the same double cone.
        prop_assert_eq!(
            common_properties(&cone, &other(half_angle))?,
            SurfaceIntersection::Coincident
        );
        // A cylinder on the axis cuts both nappes: two circles at
        // `R / tan α` either side of the apex.
        let cylinder = on_axis(Coaxial::Cylinder, &axis, pb);
        let r = common_properties(&cylinder, &cone)?;
        let SurfaceIntersection::Transversal(c) = &r else {
            return Err(TestCaseError::fail(format!("{r:?}")));
        };
        prop_assert_eq!(c.len(), 2, "{:?}", r);
        for circle in c {
            let Curve::Circle { frame, radius: rc } = circle else {
                return Err(TestCaseError::fail(format!("{circle:?}")));
            };
            prop_assert!((rc - pb.r1).abs() <= EXACT);
            let along = (frame.origin() - apex).dot(&z).abs();
            prop_assert!(
                (along - pb.r1 / half_angle.tan()).abs() <= EXACT * 100.0,
                "{along}"
            );
        }
        Ok(())
    });
}

/// A plane through the axis of a cone cuts its two rulings through the
/// apex, and through the axis of a torus its two tube circles: each
/// `Transversal`, on both surfaces, a ruling from the apex along the
/// cone's `∂P/∂v` at the half-angle to the axis and a circle of radius
/// `r` about `O ± R·w` in the plane with its `t` the torus's `v`, the
/// first on the side `w = Z × n`; the plane's origin anywhere in it, its
/// normal either way. The same plane a hair off the axis is C3's.
#[test]
fn a_plane_through_the_axis_cuts_the_meridian() {
    check(
        (frame(), placement(), placement(), any::<bool>()),
        |(axis, pa, pb, flip)| {
            let z = axis.z().into_inner();
            let around = tilted(&axis, FRAC_PI_2, pb.phase);
            let n = if flip { -around } else { around };
            let anchor = axis.origin() + pb.slide * z + pb.shift * z.cross(&around);
            let plane = Surface::Plane {
                frame: Frame::new(anchor, n, z).unwrap(),
            };
            let w = axis.z().cross(&UnitVec3::new_normalize(n));
            for kind in [Coaxial::Cone, Coaxial::Torus] {
                let carrier = on_axis(kind, &axis, pa);
                let own_z = carrier.frame().unwrap().z().into_inner();
                // `on_axis` may reverse the carrier's own Z, and the side
                // is taken from it.
                let w = w * own_z.dot(&z).signum();
                let r = common_properties(&plane, &carrier)?;
                let SurfaceIntersection::Transversal(c) = &r else {
                    return Err(TestCaseError::fail(format!("{kind:?}: {r:?}")));
                };
                prop_assert_eq!(c.len(), 2, "{:?}: {:?}", kind, r);
                prop_assert_eq!(
                    &intersect_surfaces(&carrier, &plane, tol()).unwrap(),
                    &r,
                    "either order"
                );
                for (curve, sign) in c.iter().zip([1.0, -1.0]) {
                    match (&carrier, curve) {
                        (
                            Surface::Cone {
                                frame,
                                radius,
                                half_angle,
                            },
                            Curve::Line { origin, direction },
                        ) => {
                            let apex = frame.origin() - (radius / half_angle.tan()) * own_z;
                            prop_assert!((origin - apex).norm() <= EXACT, "{origin} vs {apex}");
                            let (sa, ca) = half_angle.sin_cos();
                            let expected = sa * sign * w + ca * own_z;
                            prop_assert!((direction.into_inner() - expected).norm() <= EXACT);
                        }
                        (
                            Surface::Torus {
                                frame,
                                major_radius,
                                minor_radius,
                            },
                            Curve::Circle {
                                frame: cf,
                                radius: cr,
                            },
                        ) => {
                            let centre = frame.origin() + sign * major_radius * w;
                            prop_assert!((cf.origin() - centre).norm() <= EXACT);
                            prop_assert_eq!(cr, minor_radius);
                            for t in [0.0, 1.0, 2.5, 4.0] {
                                let u =
                                    (sign * w).dot(&frame.y()).atan2((sign * w).dot(&frame.x()));
                                let d = (curve.point(t) - carrier.point(u, t)).norm();
                                prop_assert!(d <= EXACT, "t = {t} is not the torus's v: {d}");
                            }
                        }
                        _ => return Err(TestCaseError::fail(format!("{kind:?}: {curve:?}"))),
                    }
                }
                // Off the axis by more than the tolerance: a hyperbola or
                // a spiric section, C3's.
                let off = Surface::Plane {
                    frame: plane
                        .frame()
                        .unwrap()
                        .with_origin(anchor + (pb.r2 + 0.5) * around),
                };
                let apart = intersect_surfaces(&off, &carrier, tol());
                prop_assert!(
                    matches!(apart, Err(GeomError::Unsupported { .. })),
                    "a plane off the axis: {:?}",
                    apart
                );
            }
            Ok(())
        },
    );
}

/// A non-coaxial pose from §Non-goals of the quadric plan.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Apart {
    /// A plane oblique to a cone's or a torus's axis.
    Oblique,
    /// A plane parallel to the axis and off it: through it, the plane
    /// cuts the meridian (`a_plane_through_the_axis_cuts_the_meridian`).
    Parallel,
    /// Two cones or two tori on parallel axes apart, or on crossing axes.
    OtherAxis,
    /// A sphere whose centre is off a carrier's axis.
    OffCentre,
}

fn apart() -> impl Strategy<Value = Apart> {
    prop_oneof![
        Just(Apart::Oblique),
        Just(Apart::Parallel),
        Just(Apart::OtherAxis),
        Just(Apart::OffCentre),
    ]
}

#[test]
fn every_non_coaxial_quadric_pose_is_unsupported() {
    check(
        (
            frame(),
            apart(),
            prop_oneof![
                Just(Coaxial::Cone),
                Just(Coaxial::Torus),
                Just(Coaxial::Cylinder)
            ],
            placement(),
            placement(),
            any::<bool>(),
        ),
        |(axis, pose, kind, pa, pb, through)| {
            let z = axis.z().into_inner();
            let carrier = on_axis(kind, &axis, pa);
            let around = tilted(&axis, FRAC_PI_2, pb.phase);
            let offset = pb.r1 + 0.5;
            let other = match pose {
                Apart::Oblique => Surface::Plane {
                    frame: Frame::from_z(
                        axis.origin() + pb.slide * z,
                        tilted(&axis, pb.tilt, pb.phase),
                    )
                    .unwrap(),
                },
                Apart::Parallel => Surface::Plane {
                    frame: Frame::new(axis.origin() + pb.slide * z + offset * around, around, z)
                        .unwrap(),
                },
                Apart::OtherAxis => {
                    let direction = if through {
                        z
                    } else {
                        tilted(&axis, pb.tilt, pb.phase)
                    };
                    let moved = Frame::new(
                        axis.origin() + pb.slide * z + offset * around,
                        direction,
                        z.cross(&around),
                    )
                    .unwrap();
                    on_axis(kind, &moved, pb)
                }
                Apart::OffCentre => Surface::Sphere {
                    frame: Frame::new(axis.origin() + pb.slide * z + offset * around, z, around)
                        .unwrap(),
                    radius: pb.r2,
                },
            };
            // A plane against a cylinder and two cylinders are cycle 1's
            // and the cylinder plan's arms, not this one's: a cylinder
            // carries the axis here only against an off-centre sphere.
            prop_assume!(kind != Coaxial::Cylinder || pose == Apart::OffCentre);
            for (x, y) in [(&carrier, &other), (&other, &carrier)] {
                match intersect_surfaces(x, y, tol()) {
                    Err(GeomError::Unsupported { a, b }) => {
                        prop_assert_eq!(a, GeomKind::Surface(x.kind()));
                        prop_assert_eq!(b, GeomKind::Surface(y.kind()));
                    }
                    other => return Err(TestCaseError::fail(format!("{x:?} vs {y:?}: {other:?}"))),
                }
            }
            Ok(())
        },
    );
}
