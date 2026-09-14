//! `ops::chamfer` (ADR-0007): one box edge clean at `Full` with nothing
//! unchecked, at its closed-form volume and area, its record rooted at
//! the edge and audited; the miter's chamfer twin, two planes meeting in
//! a line, fully checked; a concave edge; and the refusals a chamfer
//! words differently from a fillet.

use arris_debug::dump_text;
use arris_ops::arris_check::arris_topo::arris_geom::{Profile, ProfileLoop, ProfileSegment};
use arris_ops::arris_check::arris_topo::arris_math::{Frame, Point2, Point3, Vec3};
use arris_ops::arris_check::arris_topo::provenance::audit;
use arris_ops::arris_check::arris_topo::{Body, Edge, EntityId, Model, Orientation, Shape};
use arris_ops::arris_check::{Level, check};
use arris_ops::measure::mass_properties;
use arris_ops::{OpError, Reason, chamfer, extrude, primitive_box};

/// The edge of `body` whose curve's midpoint is `at`.
fn edge_at(m: &Model, body: Body, at: Point3) -> Edge {
    m.edges(body)
        .unwrap()
        .into_iter()
        .find(|e| {
            let entity = m.edge(e.id).unwrap();
            entity.curve().is_some_and(|(curve, range)| {
                (m.curve(curve).unwrap().point(range.midpoint()) - at).norm() < 1e-9
            })
        })
        .unwrap_or_else(|| panic!("no edge through {at}"))
}

fn cube(m: &mut Model) -> Body {
    primitive_box(m, Point3::origin(), Point3::new(2.0, 2.0, 2.0))
        .unwrap()
        .0
}

/// The polygon `points` in the world's XY plane extruded 2 along +Z.
fn prism(m: &mut Model, points: &[(f64, f64)]) -> Body {
    let p = |(u, v): (f64, f64)| Point2::new(u, v);
    let mut segments: Vec<ProfileSegment> = points[1..]
        .iter()
        .map(|&q| ProfileSegment::LineTo(p(q)))
        .collect();
    segments.push(ProfileSegment::LineTo(p(points[0])));
    let profile = Profile {
        plane: Frame::world(),
        outer: ProfileLoop::Path {
            start: p(points[0]),
            segments,
        },
        holes: Vec::new(),
    };
    extrude(m, &profile, Vec3::z(), 2.0).unwrap().0
}

fn reason(err: &OpError) -> Option<Reason> {
    match err {
        OpError::Degenerate { reason, .. } => Some(*reason),
        _ => None,
    }
}

fn forward(e: Edge) -> Shape {
    Shape::new(e.id, Orientation::Forward)
}

/// `body` clean at `Full` with nothing unchecked, with the Euler line's
/// `(V, E, F, L)` and `volume` to 1e-9 relative.
fn assert_clean(m: &Model, body: Body, counts: (usize, usize, usize, usize), volume: f64) {
    let report = check(m, body, Level::Full);
    assert!(report.is_ok(), "{report}");
    assert!(report.unchecked().is_empty(), "{report}");
    let line = report.euler().unwrap();
    assert_eq!(
        (line.vertices, line.edges, line.faces, line.loops),
        counts,
        "{report}"
    );
    let found = mass_properties(m, body).unwrap().volume;
    assert!(
        (found - volume).abs() <= 1e-9 * volume,
        "{found} vs {volume}"
    );
}

const D: f64 = 0.2;

/// The consumer's number: a 2-cube with one vertical edge chamfered at
/// `d = 0.2` loses a right prism of `d²/2 · 2`.
#[test]
fn one_box_edge_end_to_end() {
    let mut m = Model::default();
    let body = cube(&mut m);
    let edge = edge_at(&m, body, Point3::new(2.0, 2.0, 1.0));
    let (chamfered, provenance) = chamfer(&mut m, body, &[edge], D).unwrap();
    assert_clean(&m, chamfered, (10, 15, 7, 7), 7.96);
    let area = mass_properties(&m, chamfered).unwrap().area;
    let expected = 24.0 - 4.0 * D - D * D + 2.0 * core::f64::consts::SQRT_2 * D;
    assert!((area - expected).abs() <= 1e-9 * expected, "{area}");

    audit(&m, &[body], chamfered, &provenance).unwrap();
    let generated = provenance.generated_from(edge.shape());
    let faces = generated
        .iter()
        .filter(|s| matches!(s.id, EntityId::Face(_)))
        .count();
    assert_eq!((generated.len(), faces), (9, 1), "{provenance}");
    assert!(provenance.is_deleted(edge.shape()));

    let mut again = Model::default();
    let body2 = cube(&mut again);
    let edge2 = edge_at(&again, body2, Point3::new(2.0, 2.0, 1.0));
    let (chamfered2, provenance2) = chamfer(&mut again, body2, &[edge2], D).unwrap();
    assert_eq!(
        dump_text(&m, chamfered).unwrap(),
        dump_text(&again, chamfered2).unwrap()
    );
    assert_eq!(provenance, provenance2);
}

/// The miter's chamfer twin: the vertical and the cap edge at the corner
/// (2, 2, 2). The two planes meet in the line from (2 − d, 2, 2 − d) to
/// (2, 2 − d, 2), `Generated` from both edges; the third edge ends at the
/// second point; the prisms' overlap `d³/3` is counted once; and with no
/// cylinder in it the result is checked at `Full` with nothing unchecked.
/// Either order of the two edges is the same result.
#[test]
fn two_chamfers_at_a_corner_meet_in_a_line() {
    let build = |reversed: bool| {
        let mut m = Model::default();
        let body = cube(&mut m);
        let vertical = edge_at(&m, body, Point3::new(2.0, 2.0, 1.0));
        let cap = edge_at(&m, body, Point3::new(1.0, 2.0, 2.0));
        let edges = if reversed {
            [cap, vertical]
        } else {
            [vertical, cap]
        };
        let (chamfered, provenance) = chamfer(&mut m, body, &edges, D).unwrap();
        (m, body, chamfered, provenance)
    };
    let (m, body, chamfered, provenance) = build(false);
    assert_clean(
        &m,
        chamfered,
        (11, 17, 8, 8),
        8.0 - 2.0 * D * D + D * D * D / 3.0,
    );
    audit(&m, &[body], chamfered, &provenance).unwrap();

    let vertical = edge_at(&m, body, Point3::new(2.0, 2.0, 1.0));
    let cap = edge_at(&m, body, Point3::new(1.0, 2.0, 2.0));
    let shared = provenance.generated_pair(forward(vertical), forward(cap));
    let lines: Vec<_> = shared
        .iter()
        .filter_map(|s| match s.id {
            EntityId::Edge(id) => Some(id),
            _ => None,
        })
        .collect();
    let [line] = lines[..] else {
        panic!("one corner line: {shared:?}");
    };
    assert_eq!(shared.len(), 3, "the line and its two vertices");
    let (curve, range) = m.edge(line).unwrap().curve().unwrap();
    let curve = m.curve(curve).unwrap();
    let mut ends = [curve.point(range.lo()), curve.point(range.hi())];
    ends.sort_by(|a, b| a.x.total_cmp(&b.x));
    assert!((ends[0] - Point3::new(2.0 - D, 2.0, 2.0 - D)).norm() < 1e-9);
    assert!((ends[1] - Point3::new(2.0, 2.0 - D, 2.0)).norm() < 1e-9);

    let third = edge_at(&m, body, Point3::new(2.0, 1.0, 2.0));
    let [shortened] = provenance.modified_from(forward(third)) else {
        panic!("{provenance}");
    };
    let EntityId::Edge(id) = shortened.id else {
        panic!("{shortened:?}");
    };
    let (curve, range) = m.edge(id).unwrap().curve().unwrap();
    let curve = m.curve(curve).unwrap();
    let far_y = curve.point(range.lo()).y.max(curve.point(range.hi()).y);
    assert!((far_y - (2.0 - D)).abs() < 1e-9, "{far_y}");

    let (again, _, chamfered2, provenance2) = build(true);
    assert_eq!(
        dump_text(&m, chamfered).unwrap(),
        dump_text(&again, chamfered2).unwrap()
    );
    assert_eq!(provenance, provenance2);
}

/// A concave edge adds a prism: the inner edge of an extruded L gains
/// `d²/2 · 2`.
#[test]
fn a_concave_edge_adds_a_prism() {
    let mut m = Model::default();
    let body = prism(
        &mut m,
        &[
            (0.0, 0.0),
            (2.0, 0.0),
            (2.0, 1.0),
            (1.0, 1.0),
            (1.0, 2.0),
            (0.0, 2.0),
        ],
    );
    let edge = edge_at(&m, body, Point3::new(1.0, 1.0, 1.0));
    let (chamfered, provenance) = chamfer(&mut m, body, &[edge], D).unwrap();
    assert_clean(&m, chamfered, (14, 21, 9, 9), 6.0 + D * D);
    audit(&m, &[body], chamfered, &provenance).unwrap();
}

/// A distance that is not finite or not positive is named as the
/// distance, the model untouched.
#[test]
fn a_bad_distance_is_refused_by_name() {
    let mut m = Model::default();
    let body = cube(&mut m);
    let edge = edge_at(&m, body, Point3::new(2.0, 2.0, 1.0));
    let before = dump_text(&m, body).unwrap();
    assert!(matches!(
        reason(&chamfer(&mut m, body, &[edge], f64::INFINITY).unwrap_err()),
        Some(Reason::NonFinite { what: "distance" })
    ));
    assert!(matches!(
        reason(&chamfer(&mut m, body, &[edge], -0.2).unwrap_err()),
        Some(Reason::NotPositive {
            what: "distance",
            ..
        })
    ));
    assert_eq!(dump_text(&m, body).unwrap(), before);
}

/// Two chamfers at a corner whose edges make unequal angles with its
/// third edge — the slanted vertical edge of an extruded parallelogram
/// and its cap edge, 90° and 63.4° to the top of the slanted face — have
/// far contacts that meet it at two points: refused by name, the model
/// untouched (C6's). Each edge alone chamfers.
#[test]
fn a_corner_of_unequal_angles_is_a_vertex_blend() {
    let mut m = Model::default();
    let body = prism(&mut m, &[(0.0, 0.0), (2.0, 0.0), (3.0, 2.0), (1.0, 2.0)]);
    let vertical = edge_at(&m, body, Point3::new(3.0, 2.0, 1.0));
    let cap = edge_at(&m, body, Point3::new(2.0, 2.0, 2.0));
    let before = dump_text(&m, body).unwrap();
    let err = chamfer(&mut m, body, &[vertical, cap], D).unwrap_err();
    assert_eq!(reason(&err), Some(Reason::VertexBlend), "{err}");
    assert_eq!(dump_text(&m, body).unwrap(), before);
    chamfer(&mut m, body, &[vertical], D).unwrap();
    chamfer(&mut m, body, &[cap], D).unwrap();
}
