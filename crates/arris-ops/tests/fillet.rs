//! `ops::fillet` (ADR-0007): one convex box edge end to end — the blend
//! cylinder, its contacts and its end arcs — clean at `Full` with nothing
//! unchecked, the closed-form volume, the provenance rooted at the edge
//! and audited, identical over two runs; two edges meeting in a miter,
//! held to the oracle's numbers while its S5 row waits; a second fillet
//! on a filleted body, its records composed back to the extrude; and
//! every typed refusal.

use arris_debug::fixtures::Class;
use arris_debug::{corpus, dump_text, fixtures};
use arris_ops::arris_check::arris_topo::arris_geom::{Profile, ProfileLoop, ProfileSegment};
use arris_ops::arris_check::arris_topo::arris_math::{Axis, Frame, Point2, Point3, Vec3};
use arris_ops::arris_check::arris_topo::provenance::{Origin, Relation, Role, SweepPart, audit};
use arris_ops::arris_check::arris_topo::{Body, Edge, EntityId, Model, Orientation, Shape};
use arris_ops::arris_check::classify::{Classification, classify_point};
use arris_ops::arris_check::{Level, check};
use arris_ops::measure::mass_properties;
use arris_ops::{OpError, Reason, extrude, fillet, primitive_box, revolve};

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

fn cube(m: &mut Model, side: f64) -> Body {
    primitive_box(m, Point3::origin(), Point3::new(side, side, side))
        .unwrap()
        .0
}

fn reason(err: &OpError) -> Option<Reason> {
    match err {
        OpError::Degenerate { reason, .. } => Some(*reason),
        _ => None,
    }
}

/// The consumer's number: a 2-cube with one vertical edge filleted at
/// `r = 0.2` loses `(1 − π/4) r² · 2`.
#[test]
fn one_convex_box_edge_end_to_end() {
    let mut m = Model::default();
    let body = cube(&mut m, 2.0);
    let edge = edge_at(&m, body, Point3::new(2.0, 2.0, 1.0));
    let (blended, provenance) = fillet(&mut m, body, &[edge], 0.2).unwrap();

    let report = check(&m, blended, Level::Full);
    assert!(report.is_ok(), "{report}");
    assert!(report.unchecked().is_empty(), "{report}");
    let line = report.euler().unwrap();
    assert_eq!(
        (
            line.vertices,
            line.edges,
            line.faces,
            line.loops,
            line.genus
        ),
        (10, 15, 7, 7, 0)
    );

    let props = mass_properties(&m, blended).unwrap();
    let r: f64 = 0.2;
    let volume = 8.0 - (1.0 - core::f64::consts::FRAC_PI_4) * r * r * 2.0;
    assert!(
        (props.volume - volume).abs() <= 1e-9 * volume,
        "{}",
        props.volume
    );
    let area = 24.0 - 4.0 * r - 2.0 * (1.0 - core::f64::consts::FRAC_PI_4) * r * r
        + core::f64::consts::PI * r;
    assert!((props.area - area).abs() <= 1e-9 * area, "{}", props.area);

    // Provenance: rooted at the edge, and audited.
    audit(&m, &[body], blended, &provenance).unwrap();
    let generated = provenance.generated_from(edge.shape());
    let faces = generated
        .iter()
        .filter(|s| matches!(s.id, EntityId::Face(_)))
        .count();
    let edges = generated
        .iter()
        .filter(|s| matches!(s.id, EntityId::Edge(_)))
        .count();
    let vertices = generated
        .iter()
        .filter(|s| matches!(s.id, EntityId::Vertex(_)))
        .count();
    assert_eq!((faces, edges, vertices), (1, 4, 4));
    assert!(provenance.is_deleted(edge.shape()));
    let entity = m.edge(edge.id).unwrap();
    for v in [entity.start(), entity.end()] {
        assert!(provenance.is_deleted(Shape::new(v, Orientation::Forward)));
    }
    // The blended edge's two faces and the two caps across its ends are
    // modified; the two faces away from it are kept.
    let modified: Vec<_> = m
        .faces(body)
        .unwrap()
        .into_iter()
        .filter(|f| !provenance.modified_from(f.shape()).is_empty())
        .collect();
    assert_eq!(modified.len(), 4, "{provenance}");
    let kept = m
        .faces(body)
        .unwrap()
        .into_iter()
        .filter(|f| provenance.is_kept(f.shape(), &m, blended))
        .count();
    assert_eq!(kept, 2);

    // Deterministic.
    let mut again = Model::default();
    let body2 = cube(&mut again, 2.0);
    let edge2 = edge_at(&again, body2, Point3::new(2.0, 2.0, 1.0));
    let (blended2, provenance2) = fillet(&mut again, body2, &[edge2], 0.2).unwrap();
    assert_eq!(
        dump_text(&m, blended).unwrap(),
        dump_text(&again, blended2).unwrap()
    );
    assert_eq!(provenance, provenance2);
}

/// An extruded L, (0,0)–(2,0)–(2,1)–(1,1)–(1,2)–(0,2) at height 2: its
/// inner vertical edge at (1, 1) is concave.
fn ell(m: &mut Model) -> Body {
    let p = |u, v| Point2::new(u, v);
    let profile = Profile {
        plane: Frame::world(),
        outer: ProfileLoop::Path {
            start: p(0.0, 0.0),
            segments: vec![
                ProfileSegment::LineTo(p(2.0, 0.0)),
                ProfileSegment::LineTo(p(2.0, 1.0)),
                ProfileSegment::LineTo(p(1.0, 1.0)),
                ProfileSegment::LineTo(p(1.0, 2.0)),
                ProfileSegment::LineTo(p(0.0, 2.0)),
                ProfileSegment::LineTo(p(0.0, 0.0)),
            ],
        },
        holes: Vec::new(),
    };
    extrude(m, &profile, Vec3::z(), 2.0).unwrap().0
}

/// A concave edge adds material: the L's inner edge gains
/// `(1 − π/4) r² · 2`, its blend face is reversed against its cylinder,
/// and the arcs on the caps lie outside the caps as they were. A radius
/// the notch's walls cannot hold is still refused.
#[test]
fn a_concave_edge_adds_material() {
    let mut m = Model::default();
    let body = ell(&mut m);
    let edge = edge_at(&m, body, Point3::new(1.0, 1.0, 1.0));
    let (blended, provenance) = fillet(&mut m, body, &[edge], 0.2).unwrap();

    let report = check(&m, blended, Level::Full);
    assert!(report.is_ok(), "{report}");
    assert!(report.unchecked().is_empty(), "{report}");
    let line = report.euler().unwrap();
    assert_eq!(
        (line.vertices, line.edges, line.faces, line.loops),
        (14, 21, 9, 9)
    );
    let r: f64 = 0.2;
    let props = mass_properties(&m, blended).unwrap();
    let volume = 6.0 + (1.0 - core::f64::consts::FRAC_PI_4) * r * r * 2.0;
    assert!(
        (props.volume - volume).abs() <= 1e-9 * volume,
        "{}",
        props.volume
    );
    audit(&m, &[body], blended, &provenance).unwrap();
    let [face_id] = provenance
        .generated_from(edge.shape())
        .iter()
        .filter_map(|s| match s.id {
            EntityId::Face(id) => Some(id),
            _ => None,
        })
        .collect::<Vec<_>>()[..]
    else {
        panic!("{provenance}");
    };
    let blend = m
        .faces(blended)
        .unwrap()
        .into_iter()
        .find(|f| f.id == face_id)
        .unwrap();
    assert_eq!(blend.orientation, Orientation::Reversed);

    // The walls of the notch are 1 long: a ball of 1.2 does not fit.
    let err = fillet(&mut m, body, &[edge], 1.2).unwrap_err();
    assert_eq!(reason(&err), Some(Reason::BlendTooLarge), "{err}");
}

/// The four vertical edges of the consumer's cube in one call: each cap
/// edge is cut at both its ends by two different blends; the volume is
/// the consumer's number, and the same set listed in reverse is the same
/// result and the same record.
#[test]
fn four_disjoint_edges_in_one_call_in_either_order() {
    let corners = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)];
    let build = |reversed: bool| {
        let mut m = Model::default();
        let body = cube(&mut m, 2.0);
        let mut edges: Vec<Edge> = corners
            .iter()
            .map(|&(x, y)| edge_at(&m, body, Point3::new(x, y, 1.0)))
            .collect();
        if reversed {
            edges.reverse();
        }
        let (blended, provenance) = fillet(&mut m, body, &edges, 0.2).unwrap();
        (m, body, blended, provenance)
    };
    let (m, body, blended, provenance) = build(false);
    let report = check(&m, blended, Level::Full);
    assert!(report.is_ok(), "{report}");
    assert!(report.unchecked().is_empty(), "{report}");
    let line = report.euler().unwrap();
    assert_eq!(
        (line.vertices, line.edges, line.faces, line.loops),
        (16, 24, 10, 10)
    );
    let props = mass_properties(&m, blended).unwrap();
    assert!(
        (props.volume - 7.93132741).abs() <= 1e-8,
        "{}",
        props.volume
    );
    audit(&m, &[body], blended, &provenance).unwrap();
    // Each cap edge is shortened at both ends, into one edge.
    let cap = edge_at(&m, body, Point3::new(1.0, 0.0, 2.0));
    let cap = Shape::new(cap.id, Orientation::Forward);
    assert_eq!(provenance.modified_from(cap).len(), 1, "{provenance}");

    let (again, _, blended2, provenance2) = build(true);
    assert_eq!(
        dump_text(&m, blended).unwrap(),
        dump_text(&again, blended2).unwrap()
    );
    assert_eq!(provenance, provenance2);
}

/// A radius that is not finite or not positive is the parameter's own
/// refusal.
#[test]
fn a_bad_radius_is_refused_by_name() {
    let mut m = Model::default();
    let body = cube(&mut m, 2.0);
    let edge = edge_at(&m, body, Point3::new(2.0, 2.0, 1.0));
    let before = dump_text(&m, body).unwrap();
    assert!(matches!(
        reason(&fillet(&mut m, body, &[edge], f64::NAN).unwrap_err()),
        Some(Reason::NonFinite { what: "radius" })
    ));
    assert!(matches!(
        reason(&fillet(&mut m, body, &[edge], 0.0).unwrap_err()),
        Some(Reason::NotPositive { what: "radius", .. })
    ));
    assert_eq!(
        dump_text(&m, body).unwrap(),
        before,
        "the model is as it was"
    );
}

/// An empty list, an edge listed twice, and an edge of another body.
#[test]
fn the_edge_list_is_checked_before_anything_is_built() {
    let mut m = Model::default();
    let body = cube(&mut m, 2.0);
    let other = primitive_box(
        &mut m,
        Point3::new(5.0, 0.0, 0.0),
        Point3::new(7.0, 2.0, 2.0),
    )
    .unwrap()
    .0;
    let edge = edge_at(&m, body, Point3::new(2.0, 2.0, 1.0));
    let foreign = edge_at(&m, other, Point3::new(7.0, 2.0, 1.0));
    assert_eq!(
        reason(&fillet(&mut m, body, &[], 0.2).unwrap_err()),
        Some(Reason::NoEdges)
    );
    assert_eq!(
        reason(&fillet(&mut m, body, &[edge, edge], 0.2).unwrap_err()),
        Some(Reason::RepeatedEdge)
    );
    let err = fillet(&mut m, body, &[foreign], 0.2).unwrap_err();
    assert_eq!(reason(&err), Some(Reason::EdgeNotInBody));
    assert!(matches!(&err, OpError::Degenerate { entities, .. } if entities[0] == foreign.shape()));
}

/// A blend larger than its faces hold: a 2-cube at `r = 2.5`, whose
/// contact lines fall outside the faces, and a plate thinner than `r`,
/// whose corner edges are shorter than the trim.
#[test]
fn a_blend_too_large_for_its_faces_is_refused() {
    let mut m = Model::default();
    let body = cube(&mut m, 2.0);
    let edge = edge_at(&m, body, Point3::new(2.0, 2.0, 1.0));
    let err = fillet(&mut m, body, &[edge], 2.5).unwrap_err();
    assert_eq!(reason(&err), Some(Reason::BlendTooLarge), "{err}");
    // Exactly the face's width: the contact would lie on the far edge.
    let err = fillet(&mut m, body, &[edge], 2.0).unwrap_err();
    assert_eq!(reason(&err), Some(Reason::BlendTooLarge), "{err}");

    let plate = primitive_box(&mut m, Point3::origin(), Point3::new(2.0, 2.0, 0.5))
        .unwrap()
        .0;
    let top = edge_at(&m, plate, Point3::new(1.0, 2.0, 0.5));
    let err = fillet(&mut m, plate, &[top], 1.0).unwrap_err();
    assert_eq!(reason(&err), Some(Reason::BlendTooLarge), "{err}");
    // And a radius the plate holds builds.
    fillet(&mut m, plate, &[top], 0.2).unwrap();
}

/// The slot's arc-to-line edge: its two faces meet at a tangent
/// dihedral, so there is no corner.
#[test]
fn a_tangent_dihedral_is_a_tangent_chain() {
    let mut m = Model::default();
    let p = |u, v| Point2::new(u, v);
    let profile = Profile {
        plane: Frame::world(),
        outer: ProfileLoop::Path {
            start: p(0.0, -1.0),
            segments: vec![
                ProfileSegment::LineTo(p(4.0, -1.0)),
                ProfileSegment::ArcTo {
                    to: p(4.0, 1.0),
                    via: p(5.0, 0.0),
                },
                ProfileSegment::LineTo(p(0.0, 1.0)),
                ProfileSegment::ArcTo {
                    to: p(0.0, -1.0),
                    via: p(-1.0, 0.0),
                },
            ],
        },
        holes: Vec::new(),
    };
    let slot = extrude(&mut m, &profile, Vec3::z(), 2.0).unwrap().0;
    let seam = edge_at(&m, slot, Point3::new(4.0, -1.0, 1.0));
    let err = fillet(&mut m, slot, &[seam], 0.2).unwrap_err();
    assert_eq!(reason(&err), Some(Reason::TangentChain), "{err}");
}

/// A revolve's cone edge — a plane against a cone — is outside the
/// table and is named as such.
#[test]
fn a_pair_outside_the_table_is_unsupported() {
    let mut m = Model::default();
    let p = |u, v| Point2::new(u, v);
    let plane = Frame::new(Point3::origin(), -Vec3::y(), Vec3::x()).unwrap();
    let profile = Profile {
        plane,
        outer: ProfileLoop::Path {
            start: p(1.0, 0.0),
            segments: vec![
                ProfileSegment::LineTo(p(2.0, 0.0)),
                ProfileSegment::LineTo(p(1.0, 2.0)),
                ProfileSegment::LineTo(p(1.0, 0.0)),
            ],
        },
        holes: Vec::new(),
    };
    let ring = revolve(
        &mut m,
        &profile,
        Axis::z_at(Point3::origin()),
        core::f64::consts::FRAC_PI_2,
    )
    .unwrap()
    .0;
    // The rise of the outer vertex: an edge between the annulus and the cone.
    let rim = edge_at(
        &m,
        ring,
        Point3::new(
            2.0 * core::f64::consts::FRAC_PI_4.cos(),
            2.0 * core::f64::consts::FRAC_PI_4.sin(),
            0.0,
        ),
    );
    let err = fillet(&mut m, ring, &[rim], 0.1).unwrap_err();
    assert!(matches!(err, OpError::Unsupported { .. }), "{err}");
}

/// The miter (ADR-0007): the vertical and the cap edge at one corner of
/// the 2-cube blended in one call, the fixture `blend/fillet-miter`
/// held to the oracle's numbers here as well: `Full` is clean with nothing
/// unchecked, the two blend cylinders with crossing axes of equal radius
/// decided by the cylinder–cylinder arm; counts, volume, area, centroid
/// and every probe are the oracle's; the miter edge and its two vertices
/// are generated from both edges; the record audits; and the two edges in
/// either order build the same result.
#[test]
fn two_edges_at_a_vertex_meet_in_a_miter() {
    let dir = fixtures::corpus_root().join("blend/fillet-miter");
    let fixture = fixtures::load(&dir).unwrap();
    let chain = corpus::chain(&dir, "default").unwrap();
    let (m, blended) = (&chain.model, chain.result().unwrap());
    let result = &chain.steps["result"];

    let report = check(m, blended, Level::Full);
    assert!(report.is_ok(), "{report}");
    assert!(report.unchecked().is_empty(), "{report}");

    let expected = &fixture.expected.results["default"];
    let line = report.euler().unwrap();
    assert_eq!(
        (
            line.vertices,
            line.edges,
            line.faces,
            line.loops,
            line.shells,
            line.genus
        ),
        (
            expected.counts.vertices,
            expected.counts.edges,
            expected.counts.faces,
            expected.counts.loops,
            expected.counts.shells,
            expected.genus.unwrap()
        )
    );
    let tolerances = fixture.recipe.tolerances;
    let props = mass_properties(m, blended).unwrap();
    let (volume, area) = (expected.volume.unwrap(), expected.area.unwrap());
    assert!(
        (props.volume - volume).abs() <= tolerances.volume_rel * volume,
        "volume {} vs the oracle's {volume}",
        props.volume
    );
    assert!(
        (props.area - area).abs() <= tolerances.area_rel * area,
        "area {} vs the oracle's {area}",
        props.area
    );
    let centroid = expected.centroid.unwrap();
    let centroid = Point3::new(centroid[0], centroid[1], centroid[2]);
    assert!(
        (props.centroid - centroid).norm() <= tolerances.centroid_abs,
        "centroid {} vs the oracle's {centroid}",
        props.centroid
    );
    for probe in &expected.probes {
        let point = Point3::new(probe.point[0], probe.point[1], probe.point[2]);
        let found = match classify_point(m, blended, point).unwrap() {
            Classification::Inside => Class::In,
            Classification::Outside => Class::Out,
            Classification::On(_) => Class::On,
        };
        assert_eq!(found, probe.class, "probe {}", probe.label);
    }

    // Provenance: the miter edge and its two vertices from both edges,
    // the rest of each blend from its own, the third edge shortened.
    audit(m, &result.inputs, blended, &result.provenance).unwrap();
    let cube = result.inputs[0];
    let vertical = edge_at(m, cube, Point3::new(2.0, 2.0, 1.0));
    let cap = edge_at(m, cube, Point3::new(1.0, 2.0, 2.0));
    let third = edge_at(m, cube, Point3::new(2.0, 1.0, 2.0));
    let fwd = |e: Edge| Shape::new(e.id, Orientation::Forward);
    let shared = result.provenance.generated_pair(fwd(vertical), fwd(cap));
    let (edges, vertices): (Vec<Shape>, Vec<Shape>) = shared
        .iter()
        .copied()
        .partition(|s| matches!(s.id, EntityId::Edge(_)));
    assert_eq!((edges.len(), vertices.len()), (1, 2), "{shared:?}");
    for edge in [vertical, cap] {
        let generated = result.provenance.generated_from(fwd(edge));
        assert_eq!(generated.len(), 9, "{generated:?}");
    }
    assert_eq!(result.provenance.modified_from(fwd(third)).len(), 1);
    let corner = m.edge(vertical.id).unwrap().end();
    assert!(
        result
            .provenance
            .is_deleted(Shape::new(corner, Orientation::Forward))
    );

    // The same set in the other order is the same result.
    let mut again = Model::default();
    let cube2 = cube_body(&mut again);
    let vertical2 = edge_at(&again, cube2, Point3::new(2.0, 2.0, 1.0));
    let cap2 = edge_at(&again, cube2, Point3::new(1.0, 2.0, 2.0));
    let (blended2, provenance2) = fillet(&mut again, cube2, &[cap2, vertical2], 0.2).unwrap();
    assert_eq!(
        dump_text(m, blended).unwrap(),
        dump_text(&again, blended2).unwrap()
    );
    assert_eq!(result.provenance, provenance2);
}

/// A second fillet on a filleted body, `blend/second-fillet`: the
/// extruded square's rise at (2, 0) filleted, then the rise at (2, 2) of
/// that result. Each step's record audits, and the three records composed
/// with `Provenance::then` — in either bracketing — name every face of the
/// result from one `SweepPart` of the extrude: the six faces from their
/// caps and sides, the side face both blends trimmed still from its
/// segment, and each blend face with its two contacts, two arcs and four
/// vertices from the rise its edge was.
#[test]
fn a_second_fillet_composes_back_to_the_extrude() {
    let dir = fixtures::corpus_root().join("blend/second-fillet");
    let chain = corpus::chain(&dir, "default").unwrap();
    let m = &chain.model;
    let [cube, first, second] = ["cube", "first", "result"].map(|name| &chain.steps[name]);
    for step in [first, second] {
        audit(m, &step.inputs, step.body, &step.provenance).unwrap();
    }
    let whole = cube
        .provenance
        .then(&first.provenance)
        .then(&second.provenance);
    assert_eq!(
        whole,
        cube.provenance
            .then(&first.provenance.then(&second.provenance))
    );

    let side = |segment| SweepPart::Side {
        loop_index: 0,
        segment,
    };
    let rise = |vertex| SweepPart::Rise {
        loop_index: 0,
        vertex,
    };
    let mut expected: Vec<SweepPart> = vec![SweepPart::StartCap, SweepPart::EndCap];
    expected.extend((0..4).map(side));
    expected.extend([rise(1), rise(2)]);
    expected.sort();
    let mut found: Vec<SweepPart> = Vec::new();
    for face in m.faces(second.body).unwrap() {
        let origins = whole.origins(Shape::new(face.id, Orientation::Forward));
        let [(Relation::Generated, Origin::Role(Role::Extrude(part)))] = origins[..] else {
            panic!("{} from {origins:?}", face.id);
        };
        found.push(part);
    }
    found.sort();
    assert_eq!(found, expected);

    // Each blend whole from its rise; the shared side face one face.
    for vertex in [1, 2] {
        let generated = whole.generated_from(Role::Extrude(rise(vertex)));
        let count = |pick: fn(&EntityId) -> bool| generated.iter().filter(|s| pick(&s.id)).count();
        assert_eq!(
            (
                count(|id| matches!(id, EntityId::Face(_))),
                count(|id| matches!(id, EntityId::Edge(_))),
                count(|id| matches!(id, EntityId::Vertex(_)))
            ),
            (1, 4, 4),
            "rise {vertex}: {generated:?}"
        );
    }
    let shared = whole.generated_from(Role::Extrude(side(1)));
    assert_eq!(shared.len(), 1, "{shared:?}");
}

fn cube_body(m: &mut Model) -> Body {
    cube(m, 2.0)
}

/// Two cap edges at a corner: the same solid rotated, so the same
/// numbers as the vertical-plus-cap miter.
#[test]
fn two_cap_edges_are_the_same_miter_rotated() {
    let mut m = Model::default();
    let body = cube(&mut m, 2.0);
    let along_x = edge_at(&m, body, Point3::new(1.0, 2.0, 2.0));
    let along_y = edge_at(&m, body, Point3::new(2.0, 1.0, 2.0));
    let (blended, provenance) = fillet(&mut m, body, &[along_x, along_y], 0.2).unwrap();
    let report = check(&m, blended, Level::Full);
    assert!(report.is_ok(), "{report}");
    assert!(report.unchecked().is_empty(), "{report}");
    let line = report.euler().unwrap();
    assert_eq!(
        (line.vertices, line.edges, line.faces, line.loops),
        (11, 17, 8, 8)
    );
    let r: f64 = 0.2;
    let props = mass_properties(&m, blended).unwrap();
    let volume = 8.0 - 4.0 * (1.0 - core::f64::consts::FRAC_PI_4) * r * r
        + (5.0 / 3.0 - core::f64::consts::FRAC_PI_2) * r * r * r;
    assert!(
        (props.volume - volume).abs() <= 1e-9 * volume,
        "{}",
        props.volume
    );
    let area = 4.0 * (2.0 - r)
        + (2.0 - r) * (2.0 - r)
        + 4.0
        + 2.0 * (4.0 - (1.0 - core::f64::consts::FRAC_PI_4) * r * r)
        + 2.0 * (core::f64::consts::FRAC_PI_2 * r * (2.0 - r) + r * r);
    assert!((props.area - area).abs() <= 1e-9 * area, "{}", props.area);
    audit(&m, &[body], blended, &provenance).unwrap();
    // The third edge, the vertical one, is shortened to z = 2 − r.
    let vertical = edge_at(&m, body, Point3::new(2.0, 2.0, 1.0));
    let [shortened] = provenance.modified_from(Shape::new(vertical.id, Orientation::Forward))
    else {
        panic!("{provenance}");
    };
    let EntityId::Edge(id) = shortened.id else {
        panic!("{shortened:?}")
    };
    let entity = m.edge(id).unwrap();
    let (curve, range) = entity.curve().unwrap();
    let top = m.curve(curve).unwrap().point(range.hi());
    assert!((top - Point3::new(2.0, 2.0, 1.8)).norm() < 1e-9, "{top}");
}

/// A corner whose two blended edges have different dihedrals — the
/// slanted vertical edge of an extruded parallelogram and its cap edge
/// — is not one ellipse: the two far contacts meet the third edge at two
/// points. Refused by name, the model untouched (C6's).
#[test]
fn a_miter_of_unequal_dihedrals_is_a_vertex_blend() {
    let mut m = Model::default();
    let p = |u, v| Point2::new(u, v);
    let profile = Profile {
        plane: Frame::world(),
        outer: ProfileLoop::Path {
            start: p(0.0, 0.0),
            segments: vec![
                ProfileSegment::LineTo(p(2.0, 0.0)),
                ProfileSegment::LineTo(p(3.0, 2.0)),
                ProfileSegment::LineTo(p(1.0, 2.0)),
                ProfileSegment::LineTo(p(0.0, 0.0)),
            ],
        },
        holes: Vec::new(),
    };
    let prism = extrude(&mut m, &profile, Vec3::z(), 2.0).unwrap().0;
    let vertical = edge_at(&m, prism, Point3::new(3.0, 2.0, 1.0));
    let cap = edge_at(&m, prism, Point3::new(2.0, 2.0, 2.0));
    let before = dump_text(&m, prism).unwrap();
    let err = fillet(&mut m, prism, &[vertical, cap], 0.2).unwrap_err();
    assert_eq!(reason(&err), Some(Reason::VertexBlend), "{err}");
    assert_eq!(dump_text(&m, prism).unwrap(), before);
    // Each edge alone blends.
    fillet(&mut m, prism, &[vertical], 0.2).unwrap();
    fillet(&mut m, prism, &[cap], 0.2).unwrap();
}

/// Three blended edges at a vertex are the sphere corner, which is not
/// built yet: refused by name, the model untouched.
#[test]
fn three_edges_at_a_vertex_are_a_vertex_blend_for_now() {
    let mut m = Model::default();
    let body = cube(&mut m, 2.0);
    let vertical = edge_at(&m, body, Point3::new(2.0, 2.0, 1.0));
    let cap_x = edge_at(&m, body, Point3::new(1.0, 2.0, 2.0));
    let cap_y = edge_at(&m, body, Point3::new(2.0, 1.0, 2.0));
    let before = dump_text(&m, body).unwrap();
    let err = fillet(&mut m, body, &[vertical, cap_x, cap_y], 0.2).unwrap_err();
    assert_eq!(reason(&err), Some(Reason::VertexBlend), "{err}");
    assert_eq!(dump_text(&m, body).unwrap(), before);
}
