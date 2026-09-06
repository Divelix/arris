//! Adjacency, iteration order, closure and transactions
//! (`docs/02-data-model.md` §Adjacency and iteration,
//! `docs/01-architecture.md` §The model). The box here is topology only:
//! every entity shares one dummy curve, surface and pcurve, since none of
//! this reads geometry.

use Orientation::{Forward, Reversed};
use arris_geom::{Curve, Curve2, Surface};
use arris_math::{Frame, Interval, Point2, Point3, Vec2, Vec3};
use arris_topo::entity::{
    Body, BodyKind, Coedge, Edge as EdgeEntity, EdgeGeometry, Face, Loop, Shell, Vertex,
};
use arris_topo::{
    Body as BodyHandle, BodyId, Closure, CoedgeRef, Curve2Id, CurveId, Edge, EdgeId,
    Face as FaceHandle, FaceId, Model, NotFound, Orientation, Shell as ShellHandle, ShellId,
    SurfaceId, Vertex as VertexHandle, VertexId,
};

struct Box3 {
    vertices: [VertexId; 8],
    edges: [EdgeId; 12],
    faces: [FaceId; 6],
    shell: ShellId,
    body: BodyId,
    pcurve: Curve2Id,
}

/// The box of `docs/02-data-model.md`: vertex `i` at the corner with bits
/// (x, y, z) of `i`; twelve edges, x-parallel first; six faces whose
/// loops walk the corners so that every edge is used twice in opposite
/// directions and every normal points out.
fn build_box(m: &mut Model) -> Box3 {
    let curve = m.add_curve(Curve::Line {
        origin: Point3::origin(),
        direction: Vec3::x_axis(),
    });
    let surface = m.add_surface(Surface::Plane {
        frame: Frame::world(),
    });
    let pcurve = m.add_curve2(Curve2::Line {
        origin: Point2::origin(),
        direction: Vec2::x_axis(),
    });
    let tol = m.precision().default_tolerance;
    let vertices: [VertexId; 8] = core::array::from_fn(|i| {
        let p = Point3::new((i & 1) as f64, ((i >> 1) & 1) as f64, ((i >> 2) & 1) as f64);
        m.raw().add_vertex(Vertex::new(p, tol))
    });
    let ends: [(usize, usize); 12] = [
        (0, 1),
        (2, 3),
        (4, 5),
        (6, 7), // along x
        (0, 2),
        (1, 3),
        (4, 6),
        (5, 7), // along y
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7), // along z
    ];
    let edges: [EdgeId; 12] = core::array::from_fn(|i| {
        let (a, b) = ends[i];
        m.raw().add_edge(EdgeEntity::new(
            EdgeGeometry::Curve {
                curve,
                range: Interval::UNIT,
            },
            vertices[a],
            vertices[b],
            tol,
        ))
    });
    let cycles: [[usize; 4]; 6] = [
        [0, 2, 3, 1], // bottom, normal −z
        [4, 5, 7, 6], // top, +z
        [0, 1, 5, 4], // front, −y
        [2, 6, 7, 3], // back, +y
        [0, 4, 6, 2], // left, −x
        [1, 3, 7, 5], // right, +x
    ];
    let faces: [FaceId; 6] = core::array::from_fn(|fi| {
        let cycle = cycles[fi];
        let coedges = (0..4)
            .map(|k| {
                let (a, b) = (cycle[k], cycle[(k + 1) % 4]);
                let ei = ends
                    .iter()
                    .position(|&(p, q)| (p, q) == (a, b) || (p, q) == (b, a))
                    .expect("a box edge between two adjacent corners");
                let orientation = if ends[ei].0 == a { Forward } else { Reversed };
                Coedge::new(edges[ei], orientation, pcurve)
            })
            .collect();
        m.raw()
            .add_face(Face::new(surface, vec![Loop::new(coedges)], tol))
    });
    let shell = m.raw().add_shell(Shell::new(
        faces.iter().map(|&f| FaceHandle::forward(f)).collect(),
    ));
    let body = m
        .raw()
        .add_body(Body::solid(vec![ShellHandle::forward(shell)]));
    Box3 {
        vertices,
        edges,
        faces,
        shell,
        body,
        pcurve,
    }
}

fn ids<H: Into<arris_topo::Shape> + Copy>(handles: &[H]) -> Vec<String> {
    handles.iter().map(|&h| h.into().to_string()).collect()
}

#[test]
fn iteration_visits_each_entity_once_depth_first_at_first_visit() {
    let mut m = Model::default();
    let b = build_box(&mut m);
    let body = BodyHandle::forward(b.body);
    assert_eq!(ids(&m.shells(body).unwrap()), ["+s0"]);
    assert_eq!(
        ids(&m.faces(body).unwrap()),
        ["+f0", "+f1", "+f2", "+f3", "+f4", "+f5"]
    );
    // Bottom's loop first (e4 e1 −e5 −e0), then top's, then what front,
    // back, left and right add at their first visit.
    assert_eq!(
        ids(&m.edges(body).unwrap()),
        [
            "+e4", "+e1", "-e5", "-e0", "+e2", "+e7", "-e3", "-e6", "+e9", "-e8", "+e10", "-e11"
        ]
    );
    // Each edge's effective start then end: e4 forward is v0 → v2, e1 is
    // v2 → v3, e5 reversed is v3 → v1, …
    assert_eq!(
        ids(&m.vertices(body).unwrap()),
        ["+v0", "+v2", "+v3", "-v1", "+v4", "+v5", "+v7", "-v6"]
    );

    let mut again = Model::default();
    let b2 = build_box(&mut again);
    assert_eq!(
        (b2.vertices, b2.edges, b2.faces, b2.shell, b2.body),
        (b.vertices, b.edges, b.faces, b.shell, b.body)
    );
    assert_eq!(again.edges(body).unwrap(), m.edges(body).unwrap());
    assert_eq!(again.vertices(body).unwrap(), m.vertices(body).unwrap());
}

#[test]
fn a_reversed_body_handle_flips_every_effective_orientation() {
    let mut m = Model::default();
    let b = build_box(&mut m);
    let body = BodyHandle::new(b.body, Reversed);
    assert_eq!(ids(&m.shells(body).unwrap()), ["-s0"]);
    assert_eq!(ids(&m.faces(body).unwrap())[0], "-f0");
    assert_eq!(
        ids(&m.edges(body).unwrap())[..4],
        ["-e4", "-e1", "+e5", "+e0"]
    );
    assert_eq!(
        ids(&m.vertices(body).unwrap())[..4],
        ["-v2", "-v0", "-v3", "+v1"]
    );
    let missing = BodyHandle::forward(BodyId::new(5, 0));
    assert_eq!(m.faces(missing), Err(NotFound::new(BodyId::new(5, 0))));
}

#[test]
fn an_edge_shared_by_two_faces_has_two_uses_naming_face_loop_and_index() {
    let mut m = Model::default();
    let b = build_box(&mut m);
    let uses = m.edge_uses(b.edges[0]).unwrap();
    assert_eq!(
        uses,
        [
            CoedgeRef {
                face: b.faces[0],
                loop_index: 0,
                coedge_index: 3
            },
            CoedgeRef {
                face: b.faces[2],
                loop_index: 0,
                coedge_index: 0
            },
        ]
    );
    assert_eq!(uses[0].to_string(), "f0/0/3");
    for &e in &b.edges {
        assert_eq!(m.edge_uses(e).unwrap().len(), 2, "{e}");
    }
    assert_eq!(
        m.vertex_edges(b.vertices[0]).unwrap(),
        [b.edges[0], b.edges[4], b.edges[8]]
    );
    for &v in &b.vertices {
        assert_eq!(m.vertex_edges(v).unwrap().len(), 3, "{v}");
    }
    assert_eq!(m.face_shells(b.faces[3]).unwrap(), [b.shell]);
    assert!(m.edge_uses(EdgeId::new(12, 0)).is_err());
    assert!(m.vertex_edges(VertexId::new(0, 1)).is_err());
    assert!(m.face_shells(FaceId::new(9, 0)).is_err());
}

#[test]
fn a_closed_edge_is_listed_once_at_its_vertex_and_a_dangling_one_not_at_all() {
    let mut m = Model::default();
    let v = m.raw().add_vertex(Vertex::new(Point3::origin(), 1e-7));
    let closed = m.raw().add_edge(EdgeEntity::new(
        EdgeGeometry::Degenerate {
            range: Interval::UNIT,
        },
        v,
        v,
        1e-7,
    ));
    let ghost = VertexId::new(40, 0);
    let dangling = m.raw().add_edge(EdgeEntity::new(
        EdgeGeometry::Degenerate {
            range: Interval::UNIT,
        },
        v,
        ghost,
        1e-7,
    ));
    assert_eq!(m.vertex_edges(v).unwrap(), [closed, dangling]);
    let f = m.raw().add_face(Face::new(
        SurfaceId::new(3, 0),
        vec![Loop::new(vec![Coedge::new(
            EdgeId::new(77, 0),
            Forward,
            Curve2Id::new(0, 0),
        )])],
        1e-7,
    ));
    assert!(
        m.face(f).is_ok(),
        "raw stores the face with its dangling edge"
    );
    assert_eq!(m.edge_uses(closed).unwrap(), []);
}

#[test]
fn a_face_used_by_two_shells_of_one_general_body_lists_both() {
    let mut m = Model::default();
    let b = build_box(&mut m);
    let s1 = m.raw().add_shell(Shell::new(vec![
        FaceHandle::new(b.faces[0], Reversed),
        FaceHandle::forward(b.faces[1]),
    ]));
    let general = m.raw().add_body(Body::new(
        BodyKind::General,
        vec![ShellHandle::forward(b.shell), ShellHandle::forward(s1)],
        vec![],
        vec![],
    ));
    assert_eq!(m.face_shells(b.faces[0]).unwrap(), [b.shell, s1]);
    assert_eq!(m.face_shells(b.faces[2]).unwrap(), [b.shell]);
    let body = BodyHandle::forward(general);
    assert_eq!(ids(&m.shells(body).unwrap()), ["+s0", "+s1"]);
    let faces = m.faces(body).unwrap();
    assert_eq!(
        faces.len(),
        6,
        "the shared faces are visited once, under the first shell"
    );
    assert_eq!(faces[0], FaceHandle::forward(b.faces[0]));
}

#[test]
fn free_edges_and_vertices_follow_the_shells() {
    let mut m = Model::default();
    let b = build_box(&mut m);
    let lone = m
        .raw()
        .add_vertex(Vertex::new(Point3::new(9.0, 9.0, 9.0), 1e-7));
    let strut = m.raw().add_edge(EdgeEntity::new(
        EdgeGeometry::Curve {
            curve: CurveId::new(0, 0),
            range: Interval::UNIT,
        },
        b.vertices[7],
        lone,
        1e-7,
    ));
    let free = m
        .raw()
        .add_vertex(Vertex::new(Point3::new(-1.0, 0.0, 0.0), 1e-7));
    let general = m.raw().add_body(Body::new(
        BodyKind::General,
        vec![ShellHandle::forward(b.shell)],
        vec![Edge::new(strut, Reversed)],
        vec![free],
    ));
    let body = BodyHandle::forward(general);
    let edges = m.edges(body).unwrap();
    assert_eq!(edges.len(), 13);
    assert_eq!(edges[12], Edge::new(strut, Reversed));
    let vertices = m.vertices(body).unwrap();
    assert_eq!(vertices.len(), 10);
    assert_eq!(
        vertices[8],
        VertexHandle::new(lone, Reversed),
        "the strut's effective start"
    );
    assert_eq!(vertices[9], VertexHandle::forward(free));
    let wire = m.raw().add_body(Body::new(
        BodyKind::Wire,
        vec![],
        vec![Edge::forward(strut)],
        vec![],
    ));
    let c = m.closure(BodyHandle::forward(wire)).unwrap();
    assert_eq!(c.edges, [strut]);
    assert_eq!(c.vertices, [b.vertices[7], lone]);
    assert_eq!(c.curves, [CurveId::new(0, 0)]);
    assert!(c.faces.is_empty() && c.shells.is_empty() && c.surfaces.is_empty());
}

#[test]
fn a_failed_transaction_leaves_lengths_indices_and_the_next_id_as_before() {
    let mut m = Model::default();
    let b = build_box(&mut m);
    let untouched = m.clone();
    let uses_before = m.edge_uses(b.edges[0]).unwrap().to_vec();
    let edges_before = m.vertex_edges(b.vertices[0]).unwrap().to_vec();
    let shells_before = m.face_shells(b.faces[0]).unwrap().to_vec();

    let r: Result<(), &str> = m.transaction(|m| {
        let v = m
            .raw()
            .add_vertex(Vertex::new(Point3::new(2.0, 2.0, 2.0), 1e-7));
        let e = m.raw().add_edge(EdgeEntity::new(
            EdgeGeometry::Curve {
                curve: CurveId::new(0, 0),
                range: Interval::UNIT,
            },
            b.vertices[0],
            v,
            1e-7,
        ));
        let f = m.raw().add_face(Face::new(
            SurfaceId::new(0, 0),
            vec![Loop::new(vec![
                Coedge::new(b.edges[0], Reversed, b.pcurve),
                Coedge::new(e, Forward, b.pcurve),
            ])],
            1e-7,
        ));
        m.raw().add_shell(Shell::new(vec![
            FaceHandle::forward(b.faces[0]),
            FaceHandle::forward(f),
        ]));
        m.add_curve2(Curve2::Line {
            origin: Point2::origin(),
            direction: Vec2::y_axis(),
        });
        assert_eq!(m.edge_uses(b.edges[0]).unwrap().len(), 3);
        assert_eq!(m.vertex_edges(b.vertices[0]).unwrap().len(), 4);
        assert_eq!(m.face_shells(b.faces[0]).unwrap().len(), 2);
        Err("no")
    });
    assert_eq!(r, Err("no"));

    assert_eq!(m.edge_uses(b.edges[0]).unwrap(), uses_before);
    assert_eq!(m.vertex_edges(b.vertices[0]).unwrap(), edges_before);
    assert_eq!(m.face_shells(b.faces[0]).unwrap(), shells_before);
    assert!(m.vertex(VertexId::new(8, 0)).is_err());
    assert!(m.edge(EdgeId::new(12, 0)).is_err());
    assert!(m.face(FaceId::new(6, 0)).is_err());
    assert!(m.shell(ShellId::new(1, 0)).is_err());
    assert!(m.curve2(Curve2Id::new(1, 0)).is_err());

    // The next ids are exactly those a model that never ran the
    // transaction hands out.
    let mut twin = untouched.clone();
    let fresh = |m: &mut Model| {
        (
            m.raw().add_vertex(Vertex::new(Point3::origin(), 1e-7)),
            m.raw().add_shell(Shell::default()),
            m.add_curve2(Curve2::Line {
                origin: Point2::origin(),
                direction: Vec2::y_axis(),
            }),
        )
    };
    assert_eq!(fresh(&mut m), fresh(&mut twin));
    assert_eq!(
        m.edges(BodyHandle::forward(b.body)).unwrap(),
        untouched.edges(BodyHandle::forward(b.body)).unwrap()
    );

    let ok: Result<VertexId, ()> =
        m.transaction(|m| Ok(m.raw().add_vertex(Vertex::new(Point3::origin(), 1e-7))));
    assert!(
        m.vertex(ok.unwrap()).is_ok(),
        "a successful transaction keeps its appends"
    );
}

#[test]
fn nested_transactions_undo_only_the_inner_appends() {
    let mut m = Model::default();
    let outer: Result<VertexId, ()> = m.transaction(|m| {
        let kept = m.raw().add_vertex(Vertex::new(Point3::origin(), 1e-7));
        let inner: Result<(), ()> = m.transaction(|m| {
            m.raw().add_vertex(Vertex::new(Point3::origin(), 1e-7));
            Err(())
        });
        assert!(inner.is_err());
        Ok(kept)
    });
    let kept = outer.unwrap();
    assert!(m.vertex(kept).is_ok());
    assert_eq!(
        m.raw().add_vertex(Vertex::new(Point3::origin(), 1e-7)),
        VertexId::new(1, 0)
    );
}

#[test]
fn the_closure_of_a_body_sharing_faces_holds_only_its_own_reach() {
    let mut m = Model::default();
    let b = build_box(&mut m);
    // A second body: the box's bottom face used from the other side plus
    // one new face over the same four edges, a flat pillow.
    let other_surface = m.add_surface(Surface::Plane {
        frame: Frame::world(),
    });
    let lid = m.raw().add_face(Face::new(
        other_surface,
        vec![Loop::new(vec![
            Coedge::new(b.edges[0], Forward, b.pcurve),
            Coedge::new(b.edges[5], Forward, b.pcurve),
            Coedge::new(b.edges[1], Reversed, b.pcurve),
            Coedge::new(b.edges[4], Reversed, b.pcurve),
        ])],
        1e-7,
    ));
    let s1 = m.raw().add_shell(Shell::new(vec![
        FaceHandle::new(b.faces[0], Reversed),
        FaceHandle::forward(lid),
    ]));
    let pillow = m
        .raw()
        .add_body(Body::solid(vec![ShellHandle::forward(s1)]));

    let box_closure = m.closure(BodyHandle::forward(b.body)).unwrap();
    assert_eq!(
        box_closure,
        Closure {
            shells: vec![b.shell],
            faces: b.faces.to_vec(),
            edges: b.edges.to_vec(),
            vertices: b.vertices.to_vec(),
            curves: vec![CurveId::new(0, 0)],
            surfaces: vec![SurfaceId::new(0, 0)],
            curve2s: vec![b.pcurve],
        }
    );
    let pillow_closure = m.closure(BodyHandle::forward(pillow)).unwrap();
    assert_eq!(pillow_closure.shells, [s1]);
    assert_eq!(pillow_closure.faces, [b.faces[0], lid]);
    assert_eq!(
        pillow_closure.edges,
        [b.edges[0], b.edges[1], b.edges[4], b.edges[5]]
    );
    assert_eq!(
        pillow_closure.vertices,
        [b.vertices[0], b.vertices[1], b.vertices[2], b.vertices[3]]
    );
    assert_eq!(
        pillow_closure.surfaces,
        [SurfaceId::new(0, 0), other_surface]
    );
    assert_eq!(m.face_shells(b.faces[0]).unwrap(), [b.shell, s1]);
    assert_eq!(
        m.edge_uses(b.edges[0]).unwrap().len(),
        3,
        "model-wide, the pillow's use counts"
    );
}
