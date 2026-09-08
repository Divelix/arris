//! Checker `Full` (`docs/plans/m2-topology.md` step 8): one violation
//! test per row of E8, L5, S5, B1 and B2, the sample bodies clean at
//! `Full`, and a pair the geometry kernel has no closed form for landing
//! under `Report::unchecked` rather than passing or failing.

use arris_check::{Level, Report, ShellNestingFault, Unchecked, Violation, check};
use arris_debug::sample;
use arris_topo::arris_geom::{Curve, Curve2, NurbsCurve, Surface, SurfaceKind};
use arris_topo::arris_math::{Frame, Interval, Point2, Point3, UnitVec2, UnitVec3, Vec2, Vec3};
use arris_topo::entity::{
    Body as BodyEntity, BodyKind, Coedge, Edge, EdgeGeometry, Face, Loop, Shell, Vertex,
};
use arris_topo::{
    Body, Edge as EdgeHandle, EdgeId, Face as FaceHandle, FaceId, Model, Orientation,
    Shell as ShellHandle, ShellId, SurfaceId,
};

/// `(code, entity)` per line of the report.
fn lines(report: &Report) -> Vec<(String, String)> {
    report
        .violations()
        .iter()
        .map(|v| (v.code().to_string(), v.entity().to_string()))
        .collect()
}

fn assert_lines(report: &Report, expected: &[(&str, String)]) {
    let want: Vec<(String, String)> = expected
        .iter()
        .map(|(c, e)| (c.to_string(), e.clone()))
        .collect();
    assert_eq!(lines(report), want, "report:\n{report}");
}

fn p2(x: f64, y: f64) -> Point2 {
    Point2::new(x, y)
}

fn ground(m: &mut Model) -> SurfaceId {
    m.add_surface(Surface::Plane {
        frame: Frame::world(),
    })
}

/// A ring of the ground plane: a vertex per corner, a line edge per side
/// and a forward coedge whose pcurve is that side at the edge's own
/// parameter. The same helper `fast_part2.rs` builds its plates from.
struct Ring {
    edges: Vec<EdgeId>,
    coedges: Vec<Coedge>,
}

fn ring(m: &mut Model, corners: &[Point2]) -> Ring {
    let world: Vec<Point3> = corners.iter().map(|p| Point3::new(p.x, p.y, 0.0)).collect();
    ring_on(m, &Frame::world(), &world)
}

/// [`ring`] on any plane: the corners are world points on it and each
/// pcurve is the side in that plane's own (u, v), at the edge's own
/// parameter.
fn ring_on(m: &mut Model, frame: &Frame, corners: &[Point3]) -> Ring {
    let tol = m.precision().default_tolerance;
    let n = corners.len();
    let at = |p: Point3| {
        let local = frame.to_local(p);
        Point2::new(local.x, local.y)
    };
    let vertices: Vec<_> = corners
        .iter()
        .map(|&c| m.raw().add_vertex(Vertex::new(c, tol)))
        .collect();
    let mut edges = Vec::with_capacity(n);
    let mut coedges = Vec::with_capacity(n);
    for k in 0..n {
        let (a, b) = (corners[k], corners[(k + 1) % n]);
        let d = b - a;
        let range = Interval::new(0.0, d.norm()).unwrap();
        let curve = m.add_curve(Curve::Line {
            origin: a,
            direction: UnitVec3::new_normalize(d),
        });
        let edge = m.raw().add_edge(Edge::new(
            EdgeGeometry::Curve { curve, range },
            vertices[k],
            vertices[(k + 1) % n],
            tol,
        ));
        let pcurve = m.add_curve2(Curve2::Line {
            origin: at(a),
            direction: UnitVec2::new_normalize(at(b) - at(a)),
        });
        edges.push(edge);
        coedges.push(Coedge::new(edge, Orientation::Forward, pcurve));
    }
    Ring { edges, coedges }
}

fn body_of(m: &mut Model, uses: Vec<FaceHandle>, kind: BodyKind) -> (Body, ShellId) {
    let shell = m.raw().add_shell(Shell::new(uses));
    let body = m.raw().add_body(BodyEntity::new(
        kind,
        vec![ShellHandle::forward(shell)],
        Vec::new(),
        Vec::new(),
    ));
    (Body::forward(body), shell)
}

/// A sheet body of one planar face whose one loop walks `corners`.
fn plate(m: &mut Model, corners: &[Point2]) -> (Body, FaceId) {
    let tol = m.precision().default_tolerance;
    let surface = ground(m);
    let r = ring(m, corners);
    let face = m
        .raw()
        .add_face(Face::new(surface, vec![Loop::new(r.coedges)], tol));
    let (body, _) = body_of(m, vec![FaceHandle::forward(face)], BodyKind::Sheet);
    (body, face)
}

/// The shell of `body`, with every face use turned the other way: the
/// same surfaces with their normals into the material, so the volume it
/// encloses is the negative of the original's.
fn inverted_shell(m: &mut Model, body: Body) -> ShellId {
    let uses: Vec<FaceHandle> = m
        .faces(body)
        .unwrap()
        .iter()
        .map(|f| FaceHandle::new(f.id, f.orientation.flipped()))
        .collect();
    m.raw().add_shell(Shell::new(uses))
}

/// A solid body over `shells`, as they are.
fn solid_of(m: &mut Model, shells: Vec<ShellId>) -> Body {
    let uses = shells.into_iter().map(ShellHandle::forward).collect();
    Body::forward(m.raw().add_body(BodyEntity::solid(uses)))
}

#[test]
fn the_sample_bodies_are_clean_at_full_and_decide_every_row() {
    let mut m = Model::default();
    let bodies = [
        sample::unit_box(&mut m).unwrap(),
        sample::cuboid(&mut m, Point3::origin(), Point3::new(40.0, 30.0, 10.0)).unwrap(),
        sample::cylinder(&mut m, 4.0, 12.0).unwrap(),
    ];
    for body in bodies {
        let report = check(&m, body, Level::Full);
        assert!(report.is_ok(), "{body}:\n{report}");
        assert!(report.unchecked().is_empty(), "{body}:\n{report}");
    }
}

/// The B-spline probe box: the intersector has no closed form for a plane
/// against a NURBS surface, so the four pairs its NURBS face shares an
/// edge with are listed as undecided — not passed, and not a violation.
/// The fifth pair, with the opposite face, is decided by the boxes: they
/// are apart, so the faces share no point and no intersector is asked.
#[test]
fn a_nurbs_face_pair_is_unchecked_and_not_a_violation() {
    let mut m = Model::default();
    let body =
        sample::cuboid_nurbs(&mut m, Point3::origin(), Point3::new(40.0, 30.0, 10.0)).unwrap();
    let report = check(&m, body, Level::Full);
    assert!(report.is_ok(), "{report}");
    assert_eq!(report.unchecked().len(), 4, "{report}");
    for u in report.unchecked() {
        assert_eq!(u.code(), "S5");
        let Unchecked::FacePair { kinds, .. } = *u else {
            panic!("{u}")
        };
        assert!(
            kinds.0 == SurfaceKind::Nurbs || kinds.1 == SurfaceKind::Nurbs,
            "{u}"
        );
    }
    assert!(report.to_string().contains("S5? "), "{report}");
    // `Fast` neither runs the row nor claims it decided anything.
    assert!(check(&m, body, Level::Fast).unchecked().is_empty());
}

/// E8: a degree-1 NURBS whose control polygon crosses itself, on the one
/// free edge of a wire body. The crossing is off the sample grid, so one
/// pair of polyline segments meets and no neighbour of it does.
#[test]
fn e8_a_nurbs_edge_that_crosses_itself() {
    let mut m = Model::default();
    let tol = m.precision().default_tolerance;
    let corners = [
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(10.0, 10.0, 0.0),
        Point3::new(12.0, 0.0, 0.0),
        Point3::new(0.0, 7.0, 0.0),
    ];
    let curve = m.add_curve(Curve::Nurbs(
        NurbsCurve::new(
            1,
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 3.0],
            corners.to_vec(),
            vec![1.0; 4],
        )
        .unwrap(),
    ));
    let start = m.raw().add_vertex(Vertex::new(corners[0], tol));
    let end = m.raw().add_vertex(Vertex::new(corners[3], tol));
    let edge = m.raw().add_edge(Edge::new(
        EdgeGeometry::Curve {
            curve,
            range: Interval::new(0.0, 3.0).unwrap(),
        },
        start,
        end,
        tol,
    ));
    let id = m.raw().add_body(BodyEntity::new(
        BodyKind::Wire,
        Vec::new(),
        vec![EdgeHandle::forward(edge)],
        Vec::new(),
    ));
    let body = Body::forward(id);
    // The row is `Full`: nothing is claimed at `Fast`.
    assert!(check(&m, body, Level::Fast).is_ok());
    let report = check(&m, body, Level::Full);
    assert_lines(&report, &[("E8", edge.to_string())]);
    let Violation::EdgeSelfIntersects { t0, t1, .. } = report.violations()[0] else {
        panic!("{report}")
    };
    // The crossing is at (84/19, 84/19): four fifths of the way along the
    // first leg, and two thirds back along the third.
    assert!(
        (0.4 < t0 && t0 < 0.5) && (2.6 < t1 && t1 < 2.7),
        "{t0} {t1}"
    );
}

/// L5: a loop that crosses itself in (u, v) but still turns positively,
/// so L4 is happy and L5 alone reports it.
#[test]
fn l5_a_loop_that_crosses_itself() {
    let mut m = Model::default();
    let (body, face) = plate(
        &mut m,
        &[p2(0.0, 0.0), p2(0.0, 8.0), p2(12.0, 0.0), p2(10.0, 10.0)],
    );
    assert!(check(&m, body, Level::Fast).is_ok());
    let report = check(&m, body, Level::Full);
    assert_lines(&report, &[("L5", face.to_string())]);
    assert_eq!(
        report.violations()[0],
        Violation::LoopsIntersect {
            face,
            loop_a: 0,
            loop_b: 0,
        }
    );
}

/// S5: a zero-thickness sheet — the same square as two faces, one on the
/// plane seen from above and one from below, bounded by the same four
/// edges. Every row but S5 is satisfied, and the two faces coincide
/// everywhere rather than meeting along their shared edges.
#[test]
fn s5_two_coincident_faces_in_one_shell() {
    let mut m = Model::default();
    let tol = m.precision().default_tolerance;
    let up = ground(&mut m);
    let down = m.add_surface(Surface::Plane {
        frame: Frame::new(Point3::origin(), -Vec3::z(), Vec3::x()).unwrap(),
    });
    let corners = [p2(0.0, 0.0), p2(10.0, 0.0), p2(10.0, 10.0), p2(0.0, 10.0)];
    let r = ring(&mut m, &corners);
    // The same ring walked backwards, in the lower face's (u, v), which
    // is `(x, −y)`.
    let mut back = Vec::with_capacity(corners.len());
    for k in (0..corners.len()).rev() {
        let (a, b) = (corners[k], corners[(k + 1) % corners.len()]);
        let d = b - a;
        let pcurve = m.add_curve2(Curve2::Line {
            origin: p2(a.x, -a.y),
            direction: UnitVec2::new_normalize(Vec2::new(d.x, -d.y)),
        });
        back.push(Coedge::new(r.edges[k], Orientation::Reversed, pcurve));
    }
    let face_up = m
        .raw()
        .add_face(Face::new(up, vec![Loop::new(r.coedges)], tol));
    let face_down = m
        .raw()
        .add_face(Face::new(down, vec![Loop::new(back)], tol));
    let (body, shell) = body_of(
        &mut m,
        vec![FaceHandle::forward(face_up), FaceHandle::forward(face_down)],
        BodyKind::Sheet,
    );
    assert!(check(&m, body, Level::Fast).is_ok());
    let report = check(&m, body, Level::Full);
    assert_lines(&report, &[("S5", shell.to_string())]);
    assert_eq!(
        report.violations()[0],
        Violation::FacesIntersect {
            shell,
            face_a: face_up,
            face_b: face_down,
        }
    );
}

/// S5 through the transversal arm: a vertical face standing through the
/// middle of a horizontal one. Their planes cross along a line interior
/// to both faces, and the two share no edge to excuse it — nor any edge
/// at all, which is what S3 says alongside.
#[test]
fn s5_two_faces_that_cross_along_the_line_of_their_planes() {
    let mut m = Model::default();
    let tol = m.precision().default_tolerance;
    let ground_surface = ground(&mut m);
    let flat = ring(
        &mut m,
        &[p2(0.0, 0.0), p2(10.0, 0.0), p2(10.0, 10.0), p2(0.0, 10.0)],
    );
    // The plane `y = 5`, its (u, v) the world's `x` and `z + 5`.
    let frame = Frame::new(Point3::new(0.0, 5.0, -5.0), -Vec3::y(), Vec3::x()).unwrap();
    let upright_surface = m.add_surface(Surface::Plane { frame });
    let upright = ring_on(
        &mut m,
        &frame,
        &[
            Point3::new(0.0, 5.0, -5.0),
            Point3::new(10.0, 5.0, -5.0),
            Point3::new(10.0, 5.0, 5.0),
            Point3::new(0.0, 5.0, 5.0),
        ],
    );
    let face_flat = m.raw().add_face(Face::new(
        ground_surface,
        vec![Loop::new(flat.coedges)],
        tol,
    ));
    let face_upright = m.raw().add_face(Face::new(
        upright_surface,
        vec![Loop::new(upright.coedges)],
        tol,
    ));
    let (body, shell) = body_of(
        &mut m,
        vec![
            FaceHandle::forward(face_flat),
            FaceHandle::forward(face_upright),
        ],
        BodyKind::Sheet,
    );
    let s = shell.to_string();
    // S3: two faces sharing no edge are two components, which is the
    // price of a pair that meets nowhere it is allowed to.
    assert_lines(
        &check(&m, body, Level::Full),
        &[("S3", s.clone()), ("S5", s)],
    );
    assert_eq!(
        check(&m, body, Level::Full).violations()[1],
        Violation::FacesIntersect {
            shell,
            face_a: face_flat,
            face_b: face_upright,
        }
    );
}

/// B2, and B1's `NoOuter` with it: a cylinder whose every face use is
/// turned inwards encloses the negative of its volume, so it is no
/// solid at all.
#[test]
fn b2_an_inside_out_cylinder_encloses_negative_volume() {
    let mut m = Model::default();
    let c = sample::cylinder(&mut m, 4.0, 12.0).unwrap();
    let shell = inverted_shell(&mut m, c);
    let body = solid_of(&mut m, vec![shell]);
    assert!(check(&m, body, Level::Fast).is_ok());
    let report = check(&m, body, Level::Full);
    let id = body.id.to_string();
    assert_lines(&report, &[("B1", id.clone()), ("B2", id)]);
    let Violation::NonPositiveVolume { volume, .. } = report.violations()[1] else {
        panic!("{report}")
    };
    let expected = -core::f64::consts::PI * 16.0 * 12.0;
    assert!(
        (volume - expected).abs() < 1e-9 * expected.abs(),
        "{volume} vs {expected}"
    );
    assert_eq!(
        report.violations()[0],
        Violation::ShellNesting {
            body: body.id,
            fault: ShellNestingFault::NoOuter,
        }
    );
}

/// B2's value is the Gauss volume the fixtures assert: the sample box's
/// is its extents, the cylinder's is `πr²h`.
#[test]
fn the_gauss_volume_of_a_clean_solid_is_its_volume() {
    let mut m = Model::default();
    for (body, expected) in [
        (
            sample::cuboid(&mut m, Point3::origin(), Point3::new(40.0, 30.0, 10.0)).unwrap(),
            12000.0,
        ),
        (
            sample::cylinder(&mut m, 4.0, 12.0).unwrap(),
            core::f64::consts::PI * 16.0 * 12.0,
        ),
    ] {
        // An inverted copy reports the value through B2, which is how the
        // sign test reads it back.
        let shell = inverted_shell(&mut m, body);
        let inverted = solid_of(&mut m, vec![shell]);
        let report = check(&m, inverted, Level::Full);
        let Some(Violation::NonPositiveVolume { volume, .. }) = report
            .violations()
            .iter()
            .find(|v| v.code() == "B2")
            .cloned()
        else {
            panic!("{report}")
        };
        assert!(
            (-volume - expected).abs() < 1e-9 * expected,
            "{volume} vs {expected}"
        );
    }
}

/// B1: two shells that both enclose positive volume, so neither is the
/// void of the other.
#[test]
fn b1_two_outer_shells() {
    let mut m = Model::default();
    let near = sample::cuboid(&mut m, Point3::origin(), Point3::new(10.0, 10.0, 10.0)).unwrap();
    let far = sample::cuboid(
        &mut m,
        Point3::new(100.0, 100.0, 100.0),
        Point3::new(110.0, 110.0, 110.0),
    )
    .unwrap();
    let shells: Vec<ShellId> = [near, far]
        .iter()
        .map(|&b| m.shells(b).unwrap()[0].id)
        .collect();
    let body = solid_of(&mut m, shells.clone());
    let report = check(&m, body, Level::Full);
    assert_lines(&report, &[("B1", body.id.to_string())]);
    assert_eq!(
        report.violations()[0],
        Violation::ShellNesting {
            body: body.id,
            fault: ShellNestingFault::MultipleOuter { shells },
        }
    );
}

/// B1: an inward-facing shell is a void, but only where it is inside the
/// outer shell. This one is nowhere near it, and the ray cast says so.
#[test]
fn b1_a_void_shell_outside_its_outer() {
    let mut m = Model::default();
    let outer = sample::cuboid(
        &mut m,
        Point3::new(100.0, 100.0, 100.0),
        Point3::new(140.0, 130.0, 110.0),
    )
    .unwrap();
    let cylinder = sample::cylinder(&mut m, 4.0, 12.0).unwrap();
    let void = inverted_shell(&mut m, cylinder);
    let outer_shell = m.shells(outer).unwrap()[0].id;
    let body = solid_of(&mut m, vec![outer_shell, void]);
    let report = check(&m, body, Level::Full);
    assert_lines(&report, &[("B1", body.id.to_string())]);
    assert_eq!(
        report.violations()[0],
        Violation::ShellNesting {
            body: body.id,
            fault: ShellNestingFault::VoidOutside { shell: void },
        }
    );
    assert!(report.unchecked().is_empty(), "{report}");
}

/// A solid with no shell at all is B1's first fault, and the volume it
/// does not enclose is B2's.
#[test]
fn b1_a_solid_without_shells() {
    let mut m = Model::default();
    let id = m.raw().add_body(BodyEntity::solid(Vec::new()));
    let body = Body::forward(id);
    let report = check(&m, body, Level::Full);
    assert_lines(&report, &[("B1", id.to_string())]);
    assert_eq!(
        report.violations()[0],
        Violation::ShellNesting {
            body: id,
            fault: ShellNestingFault::NoShells,
        }
    );
}

#[test]
fn the_report_is_the_same_on_two_builds_at_full() {
    let build = |m: &mut Model| {
        let c = sample::cylinder(m, 4.0, 12.0).unwrap();
        let shell = inverted_shell(m, c);
        let body = solid_of(m, vec![shell]);
        check(m, body, Level::Full).to_string()
    };
    let (mut a, mut b) = (Model::default(), Model::default());
    let text = build(&mut a);
    assert_eq!(text, build(&mut b));
    assert!(text.lines().count() >= 2, "{text}");
}
