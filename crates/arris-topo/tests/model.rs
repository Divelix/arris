//! The arena's contract (`docs/ARCHITECTURE.md` §The model): sequential,
//! reproducible ids; a stale id never resolves; a raw insert checks nothing.

use arris_geom::{Curve, Curve2, Surface};
use arris_math::{Frame, Interval, Point2, Point3, Precision, Vec2, Vec3};
use arris_topo::entity::{Body, Coedge, Edge, EdgeGeometry, Face, Loop, Shell, Vertex};
use arris_topo::{
    AnyId, BodyId, Curve2Id, CurveId, EdgeId, FaceId, Model, NotFound, Orientation, ShellId,
    SurfaceId, TopoError, VertexId,
};

/// A vertex, an edge on a line, a one-loop face on a plane, a shell and a
/// solid body — every kind once — plus one of each geometry.
fn build(m: &mut Model) -> (VertexId, EdgeId, FaceId, ShellId, BodyId) {
    let line = m.add_curve(Curve::Line {
        origin: Point3::origin(),
        direction: Vec3::x_axis(),
    });
    let plane = m.add_surface(Surface::Plane {
        frame: Frame::world(),
    });
    let pc = m.add_curve2(Curve2::Line {
        origin: Point2::origin(),
        direction: Vec2::x_axis(),
    });
    let tol = m.precision().default_tolerance;
    let v = m.raw().add_vertex(Vertex::new(Point3::origin(), tol));
    let e = m.raw().add_edge(Edge::new(
        EdgeGeometry::Curve {
            curve: line,
            range: Interval::UNIT,
        },
        v,
        v,
        tol,
    ));
    let f = m.raw().add_face(Face::new(
        plane,
        vec![Loop::new(vec![Coedge::new(e, Orientation::Forward, pc)])],
        tol,
    ));
    let s = m
        .raw()
        .add_shell(Shell::new(vec![arris_topo::Face::forward(f)]));
    let b = m
        .raw()
        .add_body(Body::solid(vec![arris_topo::Shell::forward(s)]));
    (v, e, f, s, b)
}

#[test]
fn ids_are_sequential_and_identical_across_two_builds() {
    let mut a = Model::default();
    let mut b = Model::default();
    let first = build(&mut a);
    assert_eq!(
        first,
        (
            VertexId::new(0, 0),
            EdgeId::new(0, 0),
            FaceId::new(0, 0),
            ShellId::new(0, 0),
            BodyId::new(0, 0)
        )
    );
    assert_eq!(build(&mut b), first);
    let second = build(&mut a);
    assert_eq!(second.0, VertexId::new(1, 0));
    assert_eq!(second.4, BodyId::new(1, 0));
    assert_eq!(build(&mut b), second);
    assert_eq!(
        a.add_curve(Curve::Line {
            origin: Point3::origin(),
            direction: Vec3::z_axis(),
        }),
        CurveId::new(2, 0),
        "geometry ids count their own kind"
    );
}

#[test]
fn a_stale_or_missing_id_is_not_found_and_never_aliases() {
    let mut m = Model::default();
    let (v, e, f, s, b) = build(&mut m);
    assert!(m.vertex(v).is_ok() && m.edge(e).is_ok() && m.face(f).is_ok());
    assert!(m.shell(s).is_ok() && m.body(b).is_ok());
    let stale = VertexId::new(0, 1);
    assert_eq!(m.vertex(stale), Err(NotFound::new(stale)));
    let past = FaceId::new(1, 0);
    assert_eq!(m.face(past), Err(NotFound::new(past)));
    assert_eq!(
        m.curve(CurveId::new(7, 0)).unwrap_err().id,
        AnyId::Geometry(CurveId::new(7, 0).into())
    );
    assert!(m.surface(SurfaceId::new(0, 0)).is_ok());
    assert!(m.curve2(Curve2Id::new(0, 3)).is_err());
    assert_eq!(
        m.body(BodyId::new(4, 2)).unwrap_err().to_string(),
        "b4g2 does not resolve in this model"
    );
    let err: TopoError = m.edge(EdgeId::new(9, 0)).unwrap_err().into();
    assert!(matches!(err, TopoError::NotFound(_)));
}

#[test]
fn an_inconsistent_precision_is_refused() {
    let p = Precision {
        min_tolerance: 1.0,
        ..Precision::DEFAULT
    };
    assert_eq!(
        Model::new(p).err().map(|e| e.to_string()),
        Some(format!("precision is inconsistent: {p:?}"))
    );
    let fine = Precision {
        default_tolerance: 1e-6,
        ..Precision::DEFAULT
    };
    assert_eq!(Model::new(fine).unwrap().precision(), fine);
    assert_eq!(Model::default().precision(), Precision::DEFAULT);
}

#[test]
fn the_raw_insert_accepts_a_dangling_reference() {
    let mut m = Model::default();
    let ghost = VertexId::new(99, 0);
    let e = m.raw().add_edge(Edge::new(
        EdgeGeometry::Degenerate {
            range: Interval::UNIT,
        },
        ghost,
        ghost,
        1e-7,
    ));
    let edge = m.edge(e).unwrap();
    assert_eq!(edge.start(), ghost);
    assert!(edge.is_degenerate() && edge.is_closed());
    assert_eq!(edge.curve(), None);
    assert!(
        m.vertex(ghost).is_err(),
        "raw means raw: nothing was created for it"
    );
}

#[cfg(feature = "serde")]
#[test]
fn entities_round_trip_through_serde() {
    let edge = Edge::new(
        EdgeGeometry::Curve {
            curve: CurveId::new(3, 0),
            range: Interval::new(0.5, 2.0).unwrap(),
        },
        VertexId::new(1, 0),
        VertexId::new(2, 1),
        1e-7,
    );
    let text = serde_json::to_string(&edge).unwrap();
    let back: Edge = serde_json::from_str(&text).unwrap();
    assert_eq!(back, edge);

    let face = Face::new(
        SurfaceId::new(0, 0),
        vec![Loop::new(vec![
            Coedge::new(EdgeId::new(0, 0), Orientation::Forward, Curve2Id::new(0, 0)),
            Coedge::new(
                EdgeId::new(1, 0),
                Orientation::Reversed,
                Curve2Id::new(1, 0),
            ),
        ])],
        2e-7,
    );
    let text = serde_json::to_string(&face).unwrap();
    let back: Face = serde_json::from_str(&text).unwrap();
    assert_eq!(back, face);

    let vertex = Vertex::new(Point3::new(1.0, -2.0, 0.5), 1e-6);
    let back: Vertex = serde_json::from_str(&serde_json::to_string(&vertex).unwrap()).unwrap();
    assert_eq!(back, vertex);

    let body = Body::new(
        arris_topo::entity::BodyKind::General,
        vec![arris_topo::Shell::forward(ShellId::new(0, 0))],
        vec![arris_topo::Edge::new(
            EdgeId::new(4, 0),
            Orientation::Reversed,
        )],
        vec![VertexId::new(6, 0)],
    );
    let back: Body = serde_json::from_str(&serde_json::to_string(&body).unwrap()).unwrap();
    assert_eq!(back, body);
}
