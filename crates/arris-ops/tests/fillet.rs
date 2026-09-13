//! `ops::fillet` (ADR-0007): one convex box edge end to end — the blend
//! cylinder, its contacts and its end arcs — clean at `Full` with nothing
//! unchecked, the closed-form volume, the provenance rooted at the edge
//! and audited, identical over two runs; and every typed refusal.

use arris_debug::dump_text;
use arris_ops::arris_check::arris_topo::arris_geom::{Profile, ProfileLoop, ProfileSegment};
use arris_ops::arris_check::arris_topo::arris_math::{Axis, Frame, Point2, Point3, Vec3};
use arris_ops::arris_check::arris_topo::provenance::audit;
use arris_ops::arris_check::arris_topo::{Body, Edge, EntityId, Model, Orientation, Shape};
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

/// Two blended edges meeting at a vertex are the miter, which is not
/// built yet: refused by name, the model untouched.
#[test]
fn two_edges_at_a_vertex_are_a_vertex_blend_for_now() {
    let mut m = Model::default();
    let body = cube(&mut m, 2.0);
    let vertical = edge_at(&m, body, Point3::new(2.0, 2.0, 1.0));
    let cap = edge_at(&m, body, Point3::new(1.0, 2.0, 2.0));
    let before = dump_text(&m, body).unwrap();
    let err = fillet(&mut m, body, &[vertical, cap], 0.2).unwrap_err();
    assert_eq!(reason(&err), Some(Reason::VertexBlend), "{err}");
    assert_eq!(dump_text(&m, body).unwrap(), before);
}
