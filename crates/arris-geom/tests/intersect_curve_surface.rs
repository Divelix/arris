//! Line–plane, line–cylinder, circle–plane and circle–cylinder
//! intersections follow the case table in any pose: the hit count is the
//! constructed case's, every hit lies on both operands, a hit is tangent
//! when the case was built tangent, coincident when the curve was built
//! on the surface, hits are sorted with `t` in the domain, two runs agree
//! bit for bit, and every other pair is `Unsupported`
//! (`docs/plans/m1-geometry.md` step 6).

use core::f64::consts::{PI, TAU};

use arris_debug::prop::geom::{circle, curve, cylinder, line, plane, surface};
use arris_debug::prop::{DEFAULT_SCALE, check, finite_f64, point_in_box};
use arris_geom::{
    Curve, CurveKind, CurveSurfaceHit, CurveSurfaceIntersection, GeomError, GeomKind, Surface,
    SurfaceKind, intersect_curve_surface,
};
use arris_math::{Frame, Point3, Precision, Tolerance, Vec3};
use proptest::prelude::*;

/// Hit points against both operands, expected parameters in length
/// units, and expected points.
const EXACT: f64 = 1e-12 * DEFAULT_SCALE;
/// Parameters sampled around a circle to count the sign changes of its
/// distance to a cylinder.
const SAMPLES: usize = 3600;

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
            unreachable!("only planes and cylinders are hit in cycle 1")
        }
    }
}

/// `|a − b|` on a circle of period `period`, or plainly when there is
/// none.
fn param_diff(period: Option<f64>, a: f64, b: f64) -> f64 {
    match period {
        Some(p) => {
            let d = (a - b).rem_euclid(p);
            d.min(p - d)
        }
        None => (a - b).abs(),
    }
}

fn fail<T>(msg: String) -> Result<T, TestCaseError> {
    Err(TestCaseError::fail(msg))
}

fn hits_of(r: &CurveSurfaceIntersection) -> &[CurveSurfaceHit] {
    match r {
        CurveSurfaceIntersection::Points(h) => h,
        CurveSurfaceIntersection::Coincident => &[],
    }
}

/// The checks every result passes whatever its case: hits on both
/// operands, sorted with `t` in the domain, `uv` consistent with the
/// point, and determinism.
fn common_properties(c: &Curve, s: &Surface) -> Result<CurveSurfaceIntersection, TestCaseError> {
    let r = intersect_curve_surface(c, s, tol()).map_err(|e| TestCaseError::fail(e.to_string()))?;
    let hits = hits_of(&r);
    for h in hits {
        prop_assert!(
            c.domain().contains(h.t),
            "{c:?}: t = {} outside the domain",
            h.t
        );
        if c.period().is_some() {
            prop_assert!(h.t < TAU, "{c:?}: t = {} not in [0, 2π)", h.t);
        }
        prop_assert!((c.point(h.t) - h.point).norm() <= EXACT);
        let off = implicit_distance(s, h.point);
        let allowed = if h.tangent {
            tol().linear + EXACT
        } else {
            EXACT
        };
        prop_assert!(
            off <= allowed,
            "{c:?} vs {s:?}: hit at t = {} is off the surface by {off} (tangent {})",
            h.t,
            h.tangent
        );
        prop_assert!((s.point(h.uv.x, h.uv.y) - h.point).norm() <= allowed);
    }
    prop_assert!(
        hits.windows(2).all(|w| w[0].t < w[1].t),
        "unsorted: {hits:?}"
    );
    let again =
        intersect_curve_surface(c, s, tol()).map_err(|e| TestCaseError::fail(e.to_string()))?;
    prop_assert_eq!(&again, &r, "two runs differ");
    Ok(r)
}

/// The hits' parameters against the expected ones, in length units of
/// the curve's speed, in any order.
fn expect_hits(
    c: &Curve,
    r: &CurveSurfaceIntersection,
    expected: &[f64],
    tangent: bool,
) -> Result<(), TestCaseError> {
    let hits = hits_of(r);
    prop_assert_eq!(
        hits.len(),
        expected.len(),
        "expected {} hits at {:?}, got {:?}",
        expected.len(),
        expected,
        r
    );
    for e in expected {
        let speed = c.eval(*e).d1.norm();
        let near = hits
            .iter()
            .find(|h| param_diff(c.period(), h.t, *e) * speed <= EXACT);
        let Some(h) = near else {
            return fail(format!("no hit at t = {e} in {hits:?}"));
        };
        prop_assert_eq!(h.tangent, tangent, "{:?}", h);
    }
    Ok(())
}

fn expect_empty(r: &CurveSurfaceIntersection) -> Result<(), TestCaseError> {
    prop_assert_eq!(r, &CurveSurfaceIntersection::Points(Vec::new()));
    Ok(())
}

fn expect_coincident(r: &CurveSurfaceIntersection) -> Result<(), TestCaseError> {
    prop_assert_eq!(r, &CurveSurfaceIntersection::Coincident);
    Ok(())
}

/// The world-space direction at `phase` around a frame's `z`, and the one
/// a quarter turn on.
fn around(f: &Frame, phase: f64) -> (Vec3, Vec3) {
    let around = phase.cos() * f.x().into_inner() + phase.sin() * f.y().into_inner();
    (around, f.z().cross(&around))
}

// --- line–plane -----------------------------------------------------------

#[test]
fn a_line_crossing_a_plane_hits_once_at_the_built_parameter() {
    check(
        (
            plane(),
            finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
            finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
            finite_f64(0.0..=TAU),
            finite_f64(0.05..=core::f64::consts::FRAC_PI_2),
            finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
        ),
        |(p, u, v, phase, tilt, s)| {
            let point = p.point(u, v);
            let (a, _) = around(p.frame().unwrap(), phase);
            let d = tilt.cos() * a + tilt.sin() * p.frame().unwrap().z().into_inner();
            let c = Curve::Line {
                origin: point - s * d,
                direction: arris_math::UnitVec3::new_normalize(d),
            };
            let r = common_properties(&c, &p)?;
            expect_hits(&c, &r, &[s], false)?;
            let [h] = hits_of(&r) else { unreachable!() };
            prop_assert!((h.point - point).norm() <= EXACT);
            Ok(())
        },
    );
}

#[test]
fn a_line_parallel_to_a_plane_is_empty_or_coincident_by_the_gap() {
    check(
        (
            plane(),
            point_in_box(DEFAULT_SCALE),
            finite_f64(0.0..=TAU),
            finite_f64(0.01..=DEFAULT_SCALE),
            any::<bool>(),
        ),
        |(p, anchor, phase, gap, lift)| {
            let n = p.frame().unwrap().z().into_inner();
            let in_plane = anchor - n.dot(&(anchor - p.frame().unwrap().origin())) * n;
            let (a, _) = around(p.frame().unwrap(), phase);
            let c = Curve::Line {
                origin: if lift { in_plane + gap * n } else { in_plane },
                direction: arris_math::UnitVec3::new_normalize(a),
            };
            let r = common_properties(&c, &p)?;
            if lift {
                expect_empty(&r)
            } else {
                expect_coincident(&r)
            }
        },
    );
}

// --- line–cylinder --------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum LineCase {
    Two,
    Tangent,
    Empty,
    Ruling,
    ParallelOff,
}

fn line_case() -> impl Strategy<Value = LineCase> {
    prop_oneof![
        Just(LineCase::Two),
        Just(LineCase::Tangent),
        Just(LineCase::Empty),
        Just(LineCase::Ruling),
        Just(LineCase::ParallelOff),
    ]
}

#[test]
fn line_cylinder_follows_the_case_table() {
    check(
        (
            line_case(),
            cylinder(),
            finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
            finite_f64(0.0..=TAU),
            finite_f64(0.0..=0.9),
            finite_f64(-1.4..=1.4),
            finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
        ),
        |(case, cyl, height, phase, fraction, tilt, s)| {
            let Surface::Cylinder { frame, radius } = cyl else {
                unreachable!()
            };
            let z = frame.z().into_inner();
            let (a, across) = around(&frame, phase);
            let dist = match case {
                LineCase::Two => fraction * radius,
                LineCase::Tangent | LineCase::Ruling => radius,
                LineCase::Empty => radius * (1.1 + fraction),
                LineCase::ParallelOff => radius * (0.5 + fraction),
            };
            let foot = frame.origin() + height * z + dist * a;
            let d = match case {
                LineCase::Ruling | LineCase::ParallelOff => z,
                _ => tilt.cos() * across + tilt.sin() * z,
            };
            let c = Curve::Line {
                origin: foot - s * d,
                direction: arris_math::UnitVec3::new_normalize(d),
            };
            let r = common_properties(&c, &cyl)?;
            match case {
                LineCase::Two => {
                    let half = (radius * radius - dist * dist).sqrt() / tilt.cos();
                    expect_hits(&c, &r, &[s - half, s + half], false)
                }
                LineCase::Tangent => expect_hits(&c, &r, &[s], true),
                LineCase::Empty | LineCase::ParallelOff => expect_empty(&r),
                LineCase::Ruling => expect_coincident(&r),
            }
        },
    );
}

// --- circle–plane ---------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum CircleCase {
    Two,
    Tangent,
    Empty,
    Coincident,
    ParallelOff,
}

fn circle_case() -> impl Strategy<Value = CircleCase> {
    prop_oneof![
        Just(CircleCase::Two),
        Just(CircleCase::Tangent),
        Just(CircleCase::Empty),
        Just(CircleCase::Coincident),
        Just(CircleCase::ParallelOff),
    ]
}

#[test]
fn circle_plane_follows_the_case_table() {
    check(
        (
            circle_case(),
            circle(),
            finite_f64(0.0..=TAU),
            finite_f64(0.05..=core::f64::consts::FRAC_PI_2),
            finite_f64(-0.9..=0.9),
            finite_f64(0.01..=DEFAULT_SCALE),
            point_in_box(DEFAULT_SCALE),
            any::<bool>(),
        ),
        |(case, c, phase, tilt, fraction, gap, shift, flip)| {
            let Curve::Circle { frame, radius } = c else {
                unreachable!()
            };
            let z = frame.z().into_inner();
            let (a, _) = around(&frame, phase);
            // The normal tilted off the circle's axis towards `phase`; the
            // circle reaches `M = R sin(tilt)` out of any plane with it.
            let n = match case {
                CircleCase::Coincident | CircleCase::ParallelOff => z,
                _ => tilt.cos() * z + tilt.sin() * a,
            };
            let reach = radius * tilt.sin();
            // The signed height of the centre above the plane.
            let h = match case {
                CircleCase::Two => fraction * reach,
                CircleCase::Tangent => {
                    if flip {
                        -reach
                    } else {
                        reach
                    }
                }
                CircleCase::Empty => reach * (1.1 + fraction.abs()) * if flip { -1.0 } else { 1.0 },
                CircleCase::Coincident => 0.0,
                CircleCase::ParallelOff => gap,
            };
            let anchor = frame.origin() - h * n;
            let in_plane = shift - n.dot(&(shift - anchor)) * n;
            let sign = if flip { -1.0 } else { 1.0 };
            let p = Surface::Plane {
                frame: Frame::from_z(in_plane, sign * n).unwrap(),
            };
            let r = common_properties(&c, &p)?;
            match case {
                CircleCase::Two => {
                    // d(t) = h + M cos(t − phase) vanishes at phase ± half.
                    let half = (-h / reach).acos();
                    expect_hits(&c, &r, &[phase - half, phase + half], false)
                }
                CircleCase::Tangent => {
                    let t = if h < 0.0 { phase } else { phase + PI };
                    expect_hits(&c, &r, &[t], true)
                }
                CircleCase::Empty | CircleCase::ParallelOff => expect_empty(&r),
                CircleCase::Coincident => expect_coincident(&r),
            }
        },
    );
}

// --- circle–cylinder ------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum RingCase {
    Coincident,
    Four,
    Two,
    TangentOutside,
    TangentInside,
    EmptyOutside,
    EmptyInside,
}

fn ring_case() -> impl Strategy<Value = RingCase> {
    prop_oneof![
        Just(RingCase::Coincident),
        Just(RingCase::Four),
        Just(RingCase::Two),
        Just(RingCase::TangentOutside),
        Just(RingCase::TangentInside),
        Just(RingCase::EmptyOutside),
        Just(RingCase::EmptyInside),
    ]
}

#[test]
fn circle_cylinder_follows_the_case_table() {
    check(
        (
            ring_case(),
            cylinder(),
            finite_f64(-DEFAULT_SCALE..=DEFAULT_SCALE),
            finite_f64(0.0..=TAU),
            finite_f64(0.05..=0.95),
            finite_f64(0.1..=10.0),
            finite_f64(0.0..=TAU),
        ),
        |(case, cyl, height, phase, fraction, small, spin)| {
            let Surface::Cylinder { frame, radius: big } = cyl else {
                unreachable!()
            };
            let z = frame.z().into_inner();
            let (a, across) = around(&frame, phase);
            let on_axis = frame.origin() + height * z;
            // Cases in the plane across the axis need the two radii apart.
            let inside_ok = (small - big).abs() >= 0.05;
            let (c, expected): (Curve, Result<Vec<f64>, bool>) = match case {
                RingCase::Coincident => {
                    let (x, _) = around(&frame, spin);
                    let f = Frame::new(on_axis, z, x).unwrap();
                    (
                        Curve::Circle {
                            frame: f,
                            radius: big,
                        },
                        Err(true),
                    )
                }
                RingCase::Four => {
                    // In the plane through the axis, centred on it, wider
                    // than the cylinder: X = around, Y = −z.
                    let r = big * (1.1 + 2.0 * fraction);
                    let f = Frame::new(on_axis, across, a).unwrap();
                    let half = (big / r).acos();
                    (
                        Curve::Circle {
                            frame: f,
                            radius: r,
                        },
                        Ok(vec![half, PI - half, PI + half, TAU - half]),
                    )
                }
                RingCase::Two
                | RingCase::TangentOutside
                | RingCase::TangentInside
                | RingCase::EmptyOutside
                | RingCase::EmptyInside => {
                    // In the plane across the axis, centre `delta` off it:
                    // two circles of radii `big` and `small`.
                    let (near, far) = ((small - big).abs(), small + big);
                    let delta = match case {
                        RingCase::Two => near + fraction * (far - near),
                        RingCase::TangentOutside => far,
                        RingCase::TangentInside => near,
                        RingCase::EmptyOutside => far * (1.1 + fraction),
                        RingCase::EmptyInside => near * fraction * 0.9,
                        _ => unreachable!(),
                    };
                    let f = Frame::new(on_axis + delta * a, z, a).unwrap();
                    let c = Curve::Circle {
                        frame: f,
                        radius: small,
                    };
                    let expected = match case {
                        RingCase::Two => {
                            let x = (delta * delta + big * big - small * small) / (2.0 * delta);
                            let y = (big * big - x * x).sqrt();
                            let t = y.atan2(x - delta);
                            Ok(vec![t, -t])
                        }
                        RingCase::TangentOutside => Ok(vec![PI]),
                        RingCase::TangentInside => Ok(vec![if small < big { 0.0 } else { PI }]),
                        _ => Err(false),
                    };
                    (c, expected)
                }
            };
            if !inside_ok
                && matches!(
                    case,
                    RingCase::Two | RingCase::TangentInside | RingCase::EmptyInside
                )
            {
                return Ok(());
            }
            let r = common_properties(&c, &cyl)?;
            match expected {
                Err(true) => expect_coincident(&r),
                Err(false) => expect_empty(&r),
                Ok(ts) => {
                    let tangent =
                        matches!(case, RingCase::TangentOutside | RingCase::TangentInside);
                    expect_hits(&c, &r, &ts, tangent)
                }
            }
        },
    );
}

#[test]
fn a_random_circle_and_cylinder_cross_an_even_number_of_times() {
    check((circle(), cylinder()), |(c, cyl)| {
        let r = common_properties(&c, &cyl)?;
        let hits = hits_of(&r);
        let transversal = hits.iter().filter(|h| !h.tangent).count();
        prop_assert_eq!(transversal % 2, 0, "{:?}", r);
        // The sign changes of the radial distance around the circle,
        // sampled finely, are the crossings.
        let d = |t: f64| implicit_signed(&cyl, c.point(t));
        let mut changes = 0;
        let mut prev = d(0.0);
        for i in 1..=SAMPLES {
            let now = d(TAU * i as f64 / SAMPLES as f64);
            if (now < 0.0) != (prev < 0.0) {
                changes += 1;
            }
            prev = now;
        }
        if hits.iter().all(|h| !h.tangent) {
            prop_assert_eq!(changes, transversal, "{:?}", r);
        }
        Ok(())
    });
}

/// `ρ − R`: negative inside the cylinder.
fn implicit_signed(s: &Surface, p: Point3) -> f64 {
    let Surface::Cylinder { frame, radius } = s else {
        unreachable!()
    };
    let q = frame.to_local(p);
    q.x.hypot(q.y) - radius
}

// --- the rest of the table --------------------------------------------------

#[test]
fn every_other_pair_is_unsupported() {
    check((curve(), surface()), |(c, s)| {
        let closed_form = matches!(c.kind(), CurveKind::Line | CurveKind::Circle)
            && matches!(s.kind(), SurfaceKind::Plane | SurfaceKind::Cylinder);
        match intersect_curve_surface(&c, &s, tol()) {
            Ok(_) => prop_assert!(closed_form, "{c:?} vs {s:?} should be unsupported"),
            Err(GeomError::Unsupported { a, b }) => {
                prop_assert!(!closed_form, "{c:?} vs {s:?} has a closed form");
                prop_assert_eq!(a, GeomKind::Curve(c.kind()));
                prop_assert_eq!(b, GeomKind::Surface(s.kind()));
            }
            Err(e) => prop_assert!(false, "{e}"),
        }
        Ok(())
    });
}

#[test]
fn a_random_line_and_plane_or_cylinder_pass_the_common_properties() {
    check((line(), plane(), cylinder()), |(l, p, c)| {
        let r = common_properties(&l, &p)?;
        prop_assert_eq!(hits_of(&r).len(), 1, "{:?}", r);
        common_properties(&l, &c)?;
        Ok(())
    });
}
