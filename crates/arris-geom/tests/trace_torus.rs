//! The torus tracer (`docs/DATA-MODEL.md` §Curves): sections of known
//! topology, the singular points decided in the tolerance, closed forms
//! to hold a traced curve to, the poses refused by name, and at random
//! poses of every pair — each sample on both surfaces, the argument
//! order not reaching the result, and a dense scan of the torus finding
//! no point of the other surface away from every branch.

use core::f64::consts::TAU;

use arris_debug::prop::{self, check};
use arris_geom::{
    BranchEnd, GeomError, SectionBranch, SectionFault, SectionTrace, Surface, trace_torus,
};
use arris_math::{Frame, Point3, Precision, Tolerance, Vec3};
use proptest::prelude::*;

fn tol() -> Tolerance {
    Precision::DEFAULT.tolerance()
}

fn frame(origin: [f64; 3], z: [f64; 3]) -> Frame {
    Frame::from_z(Point3::from(origin), Vec3::from(z)).unwrap()
}

/// The ring every hand case cuts: `R = 2`, `r = 0.5`, at rest.
fn ring() -> Surface {
    Surface::Torus {
        frame: Frame::world(),
        major_radius: 2.0,
        minor_radius: 0.5,
    }
}

/// The plane `x = d`, parallel to the ring's axis.
fn wall(d: f64) -> Surface {
    Surface::Plane {
        frame: frame([d, 0.0, 0.0], [1.0, 0.0, 0.0]),
    }
}

fn distance(surface: &Surface, p: Point3) -> f64 {
    surface.project(p).map_or(f64::INFINITY, |on| on.distance)
}

/// `n` points along a branch, both ends included on an open one and the
/// start once on a closed one.
fn samples(branch: &SectionBranch, n: usize) -> Vec<Point3> {
    let domain = branch.domain();
    let steps = if branch.is_closed() { n } else { n - 1 };
    (0..n)
        .map(|i| branch.point(domain.lerp(i as f64 / steps as f64)))
        .collect()
}

/// The largest distance of any sample of any branch from either surface.
fn worst_off(trace: &SectionTrace, a: &Surface, b: &Surface) -> f64 {
    trace
        .branches()
        .iter()
        .flat_map(|branch| samples(branch, 257))
        .chain(trace.points().iter().map(|p| p.point))
        .map(|p| distance(a, p).max(distance(b, p)))
        .fold(0.0, f64::max)
}

fn closed(trace: &SectionTrace) -> usize {
    trace.branches().iter().filter(|b| b.is_closed()).count()
}

#[test]
fn a_plane_through_the_hole_meets_the_ring_in_two_ovals() {
    let (ring, wall) = (ring(), wall(1.0));
    let trace = trace_torus(&ring, &wall, tol()).unwrap();
    assert_eq!((trace.branches().len(), closed(&trace)), (2, 2));
    assert!(trace.points().is_empty());
    assert!(worst_off(&trace, &ring, &wall) < 1e-12);
    // One oval either side of the axis, each periodic over its domain.
    let sides: Vec<f64> = (trace.branches().iter())
        .map(|b| b.point(0.0).y.signum())
        .collect();
    assert_eq!(sides.iter().sum::<f64>(), 0.0);
    for b in trace.branches() {
        let side = b.point(0.0).y.signum();
        assert!(samples(b, 64).iter().all(|p| p.y.signum() == side));
        let t = 0.37 * b.domain().length();
        assert!((b.point(t) - b.point(t + b.domain().length())).norm() < 1e-12);
    }
}

#[test]
fn a_plane_through_the_tube_alone_meets_the_ring_in_one_oval() {
    let (ring, wall) = (ring(), wall(2.0));
    let trace = trace_torus(&ring, &wall, tol()).unwrap();
    assert_eq!((trace.branches().len(), closed(&trace)), (1, 1));
    assert!(trace.points().is_empty());
    assert!(worst_off(&trace, &ring, &wall) < 1e-12);
    let ys: Vec<f64> = (samples(&trace.branches()[0], 64).iter())
        .map(|p| p.y)
        .collect();
    assert!(ys.iter().any(|y| *y > 1.0) && ys.iter().any(|y| *y < -1.0));
}

#[test]
fn a_plane_tangent_to_the_hole_meets_the_ring_in_a_figure_eight() {
    let ring = ring();
    // Exactly tangent, a third of a tolerance short and a third past.
    for d in [1.5, 1.5 - 0.3e-7, 1.5 + 0.3e-7] {
        let wall = wall(d);
        let trace = trace_torus(&ring, &wall, tol()).unwrap();
        assert_eq!(trace.points().len(), 1, "d = {d}");
        let crossing = trace.points()[0];
        assert!(!crossing.isolated);
        assert!((crossing.point - Point3::new(1.5, 0.0, 0.0)).norm() < 1e-9);
        // Four arms, each from the crossing back to it through one
        // turning point: two open branches.
        assert_eq!((trace.branches().len(), closed(&trace)), (2, 0), "d = {d}");
        for b in trace.branches() {
            assert_eq!(
                b.ends(),
                Some([BranchEnd::Singular(0), BranchEnd::Singular(0)])
            );
            let domain = b.domain();
            assert_eq!(b.point(domain.lo()), crossing.point);
            assert_eq!(b.point(domain.hi()), crossing.point);
        }
        assert!(worst_off(&trace, &ring, &wall) <= tol().linear, "d = {d}");
    }
}

#[test]
fn a_plane_tangent_to_the_ring_outside_touches_it_at_one_point() {
    let ring = ring();
    for d in [2.5, 2.5 - 0.3e-7, 2.5 + 0.3e-7] {
        let trace = trace_torus(&ring, &wall(d), tol()).unwrap();
        assert!(trace.branches().is_empty(), "d = {d}");
        assert_eq!(trace.points().len(), 1, "d = {d}");
        assert!(trace.points()[0].isolated);
        assert!((trace.points()[0].point - Point3::new(2.5, 0.0, 0.0)).norm() < 1e-9);
    }
    let clear = trace_torus(&ring, &wall(2.6), tol()).unwrap();
    assert!(clear.branches().is_empty() && clear.points().is_empty());
}

/// A plane through the centre, tilted about `y` by the angle whose sine
/// is `r / R`, touches the ring at two points and cuts it in the two
/// Villarceau circles: radius `R`, centred `r` either side of the axis
/// along `y`.
#[test]
fn a_bitangent_plane_meets_the_ring_in_the_villarceau_circles() {
    let ring = ring();
    let tilt = (0.5f64 / 2.0).asin();
    let normal = [-tilt.sin(), 0.0, tilt.cos()];
    let plane = Surface::Plane {
        frame: frame([0.0; 3], normal),
    };
    let trace = trace_torus(&ring, &plane, tol()).unwrap();
    assert_eq!(trace.points().len(), 2);
    assert!(trace.points().iter().all(|p| !p.isolated));
    // Each circle is cut in two by the two points.
    assert_eq!((trace.branches().len(), closed(&trace)), (4, 0));
    let off = |p: Point3, side: f64| ((p - Point3::new(0.0, 0.5 * side, 0.0)).norm() - 2.0).abs();
    let mut sides = Vec::new();
    for b in trace.branches() {
        let middle = b.point(b.domain().lerp(0.5));
        let side = if off(middle, 1.0) < off(middle, -1.0) {
            1.0
        } else {
            -1.0
        };
        sides.push(side);
        for p in samples(b, 129) {
            assert!(
                off(p, side) < 1e-9,
                "{p} is {} off its circle",
                off(p, side)
            );
            assert!(distance(&plane, p) <= tol().linear);
        }
    }
    assert_eq!(sides.iter().sum::<f64>(), 0.0);
}

#[test]
fn a_drill_through_the_tube_meets_it_in_two_loops() {
    let ring = ring();
    let drill = Surface::Cylinder {
        frame: frame([2.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        radius: 0.2,
    };
    let trace = trace_torus(&ring, &drill, tol()).unwrap();
    assert_eq!((trace.branches().len(), closed(&trace)), (2, 2));
    assert!(worst_off(&trace, &ring, &drill) < 1e-12);
    let sides: Vec<f64> = (trace.branches().iter())
        .map(|b| b.point(0.0).z.signum())
        .collect();
    assert_eq!(sides.iter().sum::<f64>(), 0.0);
}

#[test]
fn a_thin_tilted_pin_through_the_hole_misses_the_ring() {
    let pin = Surface::Cylinder {
        frame: frame([0.2, -0.1, 0.0], [0.2, 0.1, 1.0]),
        radius: 0.05,
    };
    let trace = trace_torus(&ring(), &pin, tol()).unwrap();
    assert!(trace.branches().is_empty() && trace.points().is_empty());
}

#[test]
fn a_sphere_off_the_axis_meets_the_ring_in_a_loop_round_the_tube() {
    let ring = ring();
    let ball = Surface::Sphere {
        frame: frame([2.3, 0.4, 0.2], [0.0, 0.0, 1.0]),
        radius: 0.9,
    };
    let trace = trace_torus(&ball, &ring, tol()).unwrap();
    assert!(trace.points().is_empty());
    assert_eq!(closed(&trace), trace.branches().len());
    assert!(!trace.branches().is_empty());
    assert!(worst_off(&trace, &ring, &ball) < 1e-12);
}

#[test]
fn a_ring_round_the_tube_meets_it_in_two_loops_and_a_chain_link_does_not() {
    let ring = ring();
    // Threaded on the tube like a ring on a finger, and cutting into it.
    let band = Surface::Torus {
        frame: frame([2.0, 0.0, 0.0], [0.1, 1.0, 0.05]),
        major_radius: 0.7,
        minor_radius: 0.3,
    };
    for (a, b) in [(&ring, &band), (&band, &ring)] {
        let trace = trace_torus(a, b, tol()).unwrap();
        assert!(trace.points().is_empty());
        assert_eq!((trace.branches().len(), closed(&trace)), (2, 2));
        assert!(worst_off(&trace, &ring, &band) < 1e-11);
    }
    // The next link of a chain, through the hole and clear of the tube.
    let link = Surface::Torus {
        frame: frame([2.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        major_radius: 2.0,
        minor_radius: 0.6,
    };
    let trace = trace_torus(&ring, &link, tol()).unwrap();
    assert!(trace.branches().is_empty() && trace.points().is_empty());
}

#[test]
fn rings_touching_at_a_point_meet_in_that_point() {
    let ring = ring();
    // Side by side in one plane, their outer equators a third of a
    // tolerance apart.
    let beside = Surface::Torus {
        frame: frame([4.5 + 0.3e-7, 0.0, 0.0], [0.0, 0.0, 1.0]),
        major_radius: 1.6,
        minor_radius: 0.4,
    };
    let trace = trace_torus(&ring, &beside, tol()).unwrap();
    assert!(trace.branches().is_empty());
    assert_eq!(trace.points().len(), 1);
    assert!(trace.points()[0].isolated);
    assert!((trace.points()[0].point - Point3::new(2.5, 0.0, 0.0)).norm() < 1e-6);
}

/// `uv` is the walked torus's parameters of `point`, continuous along
/// the branch across both seams.
#[test]
fn the_parameters_of_a_branch_are_exact_and_continuous_across_the_seams() {
    let ring = ring();
    // A plane across the axis, tilted: one loop round the hole and one
    // round the outside, each crossing `u = 0`, the first `v = π` and the
    // second `v = 0`.
    let plane = Surface::Plane {
        frame: frame([0.0, 0.0, 0.1], [0.15, 0.1, 1.0]),
    };
    let trace = trace_torus(&ring, &plane, tol()).unwrap();
    assert_eq!((trace.branches().len(), closed(&trace)), (2, 2));
    for b in trace.branches() {
        let n = 4000;
        let length = b.domain().length();
        let mut last = b.uv(0.0).unwrap();
        for i in 1..=n {
            let t = length * i as f64 / n as f64 * (1.0 - 1e-12);
            let uv = b.uv(t).unwrap();
            assert!((ring.point(uv.x, uv.y) - b.point(t)).norm() < 1e-12);
            assert!(
                (uv - last).norm() < 0.05,
                "a jump at t = {t}: {last} to {uv}"
            );
            last = uv;
        }
        // Once round the torus's axis.
        let whole = last - b.uv(0.0).unwrap();
        assert!((whole.x.abs() - TAU).abs() < 1e-6, "{whole}");
    }
}

#[test]
fn a_branch_runs_at_a_steady_speed_through_its_turning_points() {
    let trace = trace_torus(&ring(), &wall(1.0), tol()).unwrap();
    let b = &trace.branches()[0];
    let n = 2000;
    let length = b.domain().length();
    let chords: Vec<f64> = (0..n)
        .map(|i| {
            let (t0, t1) = (
                length * i as f64 / n as f64,
                length * (i + 1) as f64 / n as f64,
            );
            (b.point(t1) - b.point(t0)).norm()
        })
        .collect();
    for w in chords.windows(2) {
        assert!((w[1] / w[0] - 1.0).abs() < 0.05, "{} then {}", w[0], w[1]);
    }
}

#[test]
fn the_poses_the_tracer_does_not_resolve_are_refused_by_name() {
    let ring = ring();
    let refused = |other: &Surface| match trace_torus(&ring, other, tol()) {
        Err(GeomError::DegenerateSection { fault, .. }) => fault,
        other => panic!("not refused: {other:?}"),
    };
    // The pipe elbow: a cylinder of the tube's radius along the tangent
    // to the centre circle.
    let elbow = Surface::Cylinder {
        frame: frame([2.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        radius: 0.5,
    };
    assert_eq!(refused(&elbow), SectionFault::TubeCircle);
    // A plane through the axis: two tube circles.
    let through = Surface::Plane {
        frame: frame([0.0; 3], [0.0, 1.0, 0.0]),
    };
    assert_eq!(refused(&through), SectionFault::TubeCircle);
    // A plane resting on the ring, and the ring itself.
    let lid = Surface::Plane {
        frame: frame([0.0, 0.0, 0.5], [0.0, 0.0, 1.0]),
    };
    assert_eq!(refused(&lid), SectionFault::TangentAlongCurve);
    assert_eq!(refused(&ring), SectionFault::TangentAlongCurve);

    let pipe = Surface::Cylinder {
        frame: Frame::world(),
        radius: 1.0,
    };
    assert!(matches!(
        trace_torus(&pipe, &wall(0.0), tol()),
        Err(GeomError::Unsupported { .. })
    ));
}

#[test]
fn a_plane_across_the_axis_meets_the_ring_in_two_circles() {
    // No turning point anywhere: both loops are seeded from `u = 0`.
    let ring = ring();
    let plane = Surface::Plane {
        frame: frame([0.0, 0.0, 0.3], [0.0, 0.0, 1.0]),
    };
    let trace = trace_torus(&ring, &plane, tol()).unwrap();
    assert_eq!((trace.branches().len(), closed(&trace)), (2, 2));
    // In the order of their `v` on `u = 0`: the outer one first.
    let radii = [2.0 + 0.4, 2.0 - 0.4];
    for (b, radius) in trace.branches().iter().zip(radii) {
        assert!((b.domain().length() - TAU).abs() < 1e-12);
        for p in samples(b, 64) {
            assert!((p.x.hypot(p.y) - radius).abs() < 1e-12 && (p.z - 0.3).abs() < 1e-12);
        }
    }
}

/// The other surface's polynomial at `p`, for its sign alone: negative
/// behind a plane, inside a cylinder, a nappe of a cone, a sphere or a
/// tube.
fn side(surface: &Surface, p: Point3) -> f64 {
    let Some(frame) = surface.frame() else {
        return f64::NAN;
    };
    let q = frame.to_local(p);
    let rho = q.x.hypot(q.y);
    match *surface {
        Surface::Plane { .. } => q.z,
        Surface::Cylinder { radius, .. } => rho - radius,
        Surface::EllipticCylinder {
            major_radius,
            minor_radius,
            ..
        } => (q.x / major_radius).hypot(q.y / minor_radius) - 1.0,
        Surface::Cone {
            radius, half_angle, ..
        } => {
            let (sin, cos) = half_angle.sin_cos();
            cos * rho - sin * (q.z + radius * cos / sin).abs()
        }
        Surface::Sphere { radius, .. } => q.coords.norm() - radius,
        Surface::Torus {
            major_radius,
            minor_radius,
            ..
        } => (rho - major_radius).hypot(q.z) - minor_radius,
        Surface::Nurbs(_) => f64::NAN,
    }
}

fn segment_distance(p: Point3, a: Point3, b: Point3) -> f64 {
    let ab = b - a;
    let t = if ab.norm_squared() > 0.0 {
        ((p - a).dot(&ab) / ab.norm_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p - (a + ab * t)).norm()
}

/// How many cells each way the scan cuts the torus into.
const SCAN: usize = 160;

/// Every cell of a grid over `torus` that `other` passes through — its
/// corners not all on one side — is near a branch or a point of the
/// trace, to what the cell's size and a polyline of the branches resolve.
fn assert_complete(trace: &SectionTrace, torus: &Surface, other: &Surface) {
    let polylines: Vec<Vec<Point3>> = (trace.branches().iter())
        .map(|b| samples(b, 2049))
        .collect();
    let to_trace = |p: Point3| {
        polylines
            .iter()
            .flat_map(|line| line.windows(2).map(|w| segment_distance(p, w[0], w[1])))
            .chain(trace.points().iter().map(|q| (q.point - p).norm()))
            .fold(f64::INFINITY, f64::min)
    };
    let at = |i: usize, j: usize| {
        torus.point(TAU * i as f64 / SCAN as f64, TAU * j as f64 / SCAN as f64)
    };
    let grid: Vec<Vec<f64>> = (0..=SCAN)
        .map(|i| (0..=SCAN).map(|j| side(other, at(i, j))).collect())
        .collect();
    for i in 0..SCAN {
        for j in 0..SCAN {
            let corners = [
                grid[i][j],
                grid[i + 1][j],
                grid[i][j + 1],
                grid[i + 1][j + 1],
            ];
            if corners.iter().all(|c| *c > 0.0) || corners.iter().all(|c| *c < 0.0) {
                continue;
            }
            let across =
                (at(i, j) - at(i + 1, j + 1)).norm() + (at(i + 1, j) - at(i, j + 1)).norm();
            let d = to_trace(at(i, j));
            assert!(
                d <= 2.0 * across,
                "the section through cell ({i}, {j}) of {torus:?} is {d} from the trace of {other:?}"
            );
        }
    }
}

/// `surface` with its frame's origin moved to `origin`.
fn placed(surface: &Surface, origin: Point3) -> Surface {
    let mut out = surface.clone();
    match &mut out {
        Surface::Plane { frame }
        | Surface::Cylinder { frame, .. }
        | Surface::EllipticCylinder { frame, .. }
        | Surface::Cone { frame, .. }
        | Surface::Sphere { frame, .. }
        | Surface::Torus { frame, .. } => *frame = frame.with_origin(origin),
        Surface::Nurbs(_) => {}
    }
    out
}

/// A rough size of a surface: how far from a point of the torus it can
/// sit and still be likely to meet it.
fn size(surface: &Surface) -> f64 {
    match *surface {
        Surface::Cylinder { radius, .. }
        | Surface::Sphere { radius, .. }
        | Surface::Cone { radius, .. } => radius,
        Surface::EllipticCylinder { minor_radius, .. } => minor_radius,
        Surface::Torus {
            major_radius,
            minor_radius,
            ..
        } => major_radius + minor_radius,
        Surface::Plane { .. } | Surface::Nurbs(_) => 0.0,
    }
}

/// A torus and another surface placed where it meets it: its origin
/// within its own size of a point of the torus.
fn meeting(other: impl Strategy<Value = Surface>) -> impl Strategy<Value = (Surface, Surface)> {
    (
        prop::geom::torus(),
        other,
        prop::finite_f64(0.0..=TAU),
        prop::finite_f64(0.0..=TAU),
        prop::unit_vec3(),
        prop::finite_f64(0.0..=0.9),
    )
        .prop_map(|(torus, other, u, v, direction, reach)| {
            let at = torus.point(u, v) + direction.into_inner() * (reach * size(&other));
            (torus, placed(&other, at))
        })
}

/// What every trace owes: each branch and point on both surfaces, `uv`
/// the torus's parameters of the point, the argument order not reaching
/// the result, and nothing of the section missed.
fn holds((a, b): (Surface, Surface)) -> Result<(), TestCaseError> {
    let trace = match trace_torus(&a, &b, tol()) {
        Ok(trace) => trace,
        Err(e) => return Err(TestCaseError::fail(format!("refused: {e}"))),
    };
    // A point's own rounding grows with its distance from the frames.
    let bound = |p: Point3| {
        let scale = 1.0 + p.coords.norm();
        let singular = if trace.points().is_empty() {
            0.0
        } else {
            tol().linear
        };
        singular + 1e-11 * scale * scale
    };
    for p in trace.branches().iter().flat_map(|br| samples(br, 65)) {
        prop_assert!(p.coords.iter().all(|x| x.is_finite()));
        let off = distance(&a, p).max(distance(&b, p));
        prop_assert!(off <= bound(p), "{p} is {off} off a surface");
    }
    for p in trace.points() {
        let off = distance(&a, p.point).max(distance(&b, p.point));
        prop_assert!(off <= bound(p.point), "{} is {off} off a surface", p.point);
    }
    let swapped = trace_torus(&b, &a, tol()).map_err(|e| TestCaseError::fail(e.to_string()))?;
    prop_assert_eq!(trace.points(), swapped.points());
    prop_assert_eq!(trace.branches().len(), swapped.branches().len());
    for (x, y) in trace.branches().iter().zip(swapped.branches()) {
        prop_assert_eq!(samples(x, 9), samples(y, 9));
    }
    // The walked torus: the only one, or the one `uv` is the parameters of.
    let walked = [&a, &b].into_iter().find(|s| {
        matches!(s, Surface::Torus { .. })
            && trace.branches().iter().all(|br| {
                let t = br.domain().lerp(0.3);
                br.uv(t).is_some_and(|uv| {
                    (s.point(uv.x, uv.y) - br.point(t)).norm() <= bound(br.point(t))
                })
            })
    });
    let Some(walked) = walked else {
        return Err(TestCaseError::fail("uv is on neither torus"));
    };
    let other = if core::ptr::eq(walked, &a) { &b } else { &a };
    assert_complete(&trace, walked, other);
    Ok(())
}

#[test]
fn random_torus_and_plane_pairs_trace() {
    check(meeting(prop::geom::plane()), holds);
}

#[test]
fn random_torus_and_cylinder_pairs_trace() {
    check(meeting(prop::geom::cylinder()), holds);
}

#[test]
fn random_torus_and_elliptic_cylinder_pairs_trace() {
    check(meeting(prop::geom::elliptic_cylinder()), holds);
}

#[test]
fn random_torus_and_cone_pairs_trace() {
    check(meeting(prop::geom::cone()), holds);
}

#[test]
fn random_torus_and_sphere_pairs_trace() {
    check(meeting(prop::geom::sphere()), holds);
}

#[test]
fn random_torus_pairs_trace() {
    check(meeting(prop::geom::torus()), holds);
}

/// Two tori a hundred times apart in size: the small one is the one
/// walked, whichever is passed first.
#[test]
fn random_pairs_of_a_large_and_a_small_torus_trace() {
    let small =
        (prop::geom::torus(), prop::finite_f64(0.01..=0.05)).prop_map(|(torus, by)| match torus {
            Surface::Torus {
                frame,
                major_radius,
                minor_radius,
            } => Surface::Torus {
                frame,
                major_radius: major_radius * by,
                minor_radius: minor_radius * by,
            },
            other => other,
        });
    check(meeting(small), holds);
}

/// `other` moved so that its point at `on_other` touches `torus` at
/// `at` — from inside the tube for `inside` — with a gap of `gap` along
/// the torus's normal, negative for an overlap, and turned about that
/// normal by a fixed angle.
fn touching(
    torus: &Surface,
    at: [f64; 2],
    other: &Surface,
    on_other: [f64; 2],
    inside: bool,
    gap: f64,
) -> Surface {
    use arris_math::Isometry;
    use arris_math::nalgebra::UnitQuaternion;
    let (p, n) = (
        torus.point(at[0], at[1]),
        torus.normal(at[0], at[1]).unwrap().into_inner(),
    );
    let (q, m) = (
        other.point(on_other[0], on_other[1]),
        other.normal(on_other[0], on_other[1]).unwrap().into_inner(),
    );
    let facing = if inside { n } else { -n };
    let turn = UnitQuaternion::from_scaled_axis(0.7 * n)
        * UnitQuaternion::rotation_between(&m, &facing).unwrap();
    let target = if inside { p - gap * n } else { p + gap * n };
    other.transformed(&Isometry::new(turn, target.coords - turn * q.coords))
}

/// Each kind of surface a fraction of the tube across, with the
/// parameters of the point it touches at and whether it is inside the
/// tube.
fn touchers(r: f64) -> Vec<(&'static str, Surface, [f64; 2], bool)> {
    let world = Frame::world();
    vec![
        ("plane", Surface::Plane { frame: world }, [0.0, 0.0], false),
        (
            "cylinder",
            Surface::Cylinder {
                frame: world,
                radius: 0.4 * r,
            },
            [0.9, 0.3 * r],
            false,
        ),
        (
            "elliptic cylinder",
            Surface::EllipticCylinder {
                frame: world,
                major_radius: 0.6 * r,
                minor_radius: 0.3 * r,
            },
            [0.9, 0.3 * r],
            false,
        ),
        (
            "cone",
            Surface::Cone {
                frame: world,
                radius: 0.4 * r,
                half_angle: 0.5,
            },
            [2.1, 0.2 * r],
            false,
        ),
        (
            "sphere",
            Surface::Sphere {
                frame: world,
                radius: 0.4 * r,
            },
            [0.4, 0.5],
            false,
        ),
        (
            "sphere in the tube",
            Surface::Sphere {
                frame: world,
                radius: 0.5 * r,
            },
            [0.4, 0.5],
            true,
        ),
        (
            "torus",
            Surface::Torus {
                frame: world,
                major_radius: 1.5 * r,
                minor_radius: 0.4 * r,
            },
            [1.3, 0.4],
            false,
        ),
    ]
}

/// Every kind of surface tangent to tori of every shape, where the torus
/// is convex and where it is a saddle: exactly tangent, half a tolerance
/// short and half a tolerance into it, the section has one singular
/// point, at the point of tangency, and every branch through it ends at
/// it; a hundred tolerances either way it has none, and is traced all the
/// same — the two turning points either side of a saddle, `√(tol·r)`
/// apart, told apart.
#[test]
fn a_tangency_within_the_tolerance_is_one_singular_point() {
    let tori = [
        (0.011, 0.01),
        (1.0, 0.01),
        (10.0, 9.0),
        (0.05, 0.02),
        (2.0, 0.5),
    ];
    for (major, minor) in tori {
        let torus = Surface::Torus {
            frame: Frame::world(),
            major_radius: major,
            minor_radius: minor,
        };
        for at in [[0.9, 0.5], [4.0, 2.6]] {
            for (name, other, on_other, inside) in touchers(minor) {
                for gap in [0.0, 0.5, -0.5, 100.0, -100.0] {
                    let label = format!("{name} at {at:?}, gap {gap} tol, R={major} r={minor}");
                    let other = touching(&torus, at, &other, on_other, inside, gap * tol().linear);
                    let trace = match trace_torus(&torus, &other, tol()) {
                        Ok(trace) => trace,
                        Err(e) => panic!("{label}: {e}"),
                    };
                    let touched = torus.point(at[0], at[1]);
                    if gap.abs() < 1.0 {
                        assert_eq!(trace.points().len(), 1, "{label}");
                        let p = trace.points()[0];
                        // As near as a gap of half a tolerance leaves the
                        // point of tangency defined.
                        let reach = (tol().linear * minor).sqrt();
                        assert!((p.point - touched).norm() <= 2.0 * reach, "{label}: {p:?}");
                        let ending = (trace.branches().iter())
                            .filter(|b| b.ends().is_some())
                            .count();
                        assert_eq!(ending, if p.isolated { 0 } else { 2 }, "{label}");
                    } else {
                        assert!(trace.points().is_empty(), "{label}");
                        assert_eq!(closed(&trace), trace.branches().len(), "{label}");
                    }
                    let off = worst_off(&trace, &torus, &other);
                    assert!(off <= tol().linear, "{label}: {off} off");
                }
            }
        }
    }
}
