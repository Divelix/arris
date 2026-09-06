//! Tessellation (`docs/plans/m3-tessellation.md` step 2, ADR-0003): the
//! sample bodies and both primitives mesh closed, with one range per
//! entity in iteration order, the seam used twice, every position on its
//! geometry, and a volume the closed form of an inscribed prism bounds;
//! a thousand random cylinders in random poses do the same; bad chords
//! and bodies are typed errors; two runs are identical.

use core::f64::consts::{PI, TAU};
use std::collections::{BTreeMap, BTreeSet};

use arris_debug::prop::{DEFAULT_SCALE, check, finite_f64, point_in_box, radius, unit_vec3};
use arris_debug::sample;
use arris_mesh::{MeshError, TriMesh, tessellate};
use arris_ops::{primitive_box, primitive_cylinder};
use arris_topo::arris_geom::region2::MIN_SEGMENTS_PER_TURN;
use arris_topo::arris_math::{Axis, Point2, Point3};
use arris_topo::entity::{Body as BodyEntity, EdgeGeometry};
use arris_topo::{Body, Model, Shell as ShellHandle, ShellId};
use proptest::prelude::*;

/// Rounding at the scale of a coordinate.
const EXACT: f64 = 1e-12;

fn p3(p: [f64; 3]) -> Point3 {
    Point3::new(p[0], p[1], p[2])
}

/// The relative volume error of an inscribed prism at sagitta `chord`
/// on radius `r`: the closed form of ADR-0003.
fn prism_bound(chord: f64, r: f64) -> f64 {
    4.0 * chord / (3.0 * r)
}

/// The structural guarantees every mesh of a solid keeps: closed, ranges
/// per entity in iteration order, every edge polyline from its start
/// vertex to its end vertex, every position on the geometry it came
/// from, every edge segment a triangle edge in both directions.
fn assert_structure(m: &Model, body: Body, mesh: &TriMesh) {
    assert!(mesh.is_closed(), "not closed");
    let tol = m.precision().default_tolerance;
    let faces = m.faces(body).unwrap();
    let edges = m.edges(body).unwrap();
    assert_eq!(
        mesh.faces().iter().map(|f| f.face).collect::<Vec<_>>(),
        faces.iter().map(|f| f.id).collect::<Vec<_>>(),
        "one FaceRange per face, in iteration order"
    );
    assert_eq!(
        mesh.edges().iter().map(|e| e.edge).collect::<Vec<_>>(),
        edges.iter().map(|e| e.id).collect::<Vec<_>>(),
        "one EdgeRange per edge, in iteration order"
    );
    let positions = mesh.positions();
    // Vertices: the polyline ends are the vertices' own points.
    for e in &edges {
        let edge = m.edge(e.id).unwrap();
        let polyline = mesh.edge_polyline(e.id).unwrap();
        let (start, end) = (
            m.vertex(edge.start()).unwrap().point(),
            m.vertex(edge.end()).unwrap().point(),
        );
        assert_eq!(p3(positions[polyline[0] as usize]), start);
        assert_eq!(p3(positions[*polyline.last().unwrap() as usize]), end);
        // Every sample on the curve.
        if let EdgeGeometry::Curve { curve, range } = edge.geometry() {
            let curve = m.curve(curve).unwrap();
            assert!(polyline.len() >= 2);
            for &i in polyline {
                let p = p3(positions[i as usize]);
                let projection = curve.project(p).unwrap();
                assert!(projection.distance <= tol, "{p} is off its curve");
                assert!(range.contains(projection.t) || curve.period().is_some());
            }
        }
    }
    // Faces: every corner on the surface, every triangle non-degenerate.
    let mut directed: BTreeMap<(u32, u32), Vec<usize>> = BTreeMap::new();
    for (k, f) in faces.iter().enumerate() {
        let face = m.face(f.id).unwrap();
        let surface = m.surface(face.surface()).unwrap();
        let triangles = mesh.face_triangles(f.id).unwrap();
        assert!(!triangles.is_empty(), "{} has no triangles", f.id);
        for t in triangles {
            assert!(t[0] != t[1] && t[1] != t[2] && t[2] != t[0]);
            for &i in t {
                let p = p3(positions[i as usize]);
                let projection = surface.project(p).unwrap();
                assert!(projection.distance <= tol, "{p} is off {}", f.id);
            }
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                directed.entry((a, b)).or_default().push(k);
            }
        }
    }
    // Every segment of every edge is a triangle edge once in each
    // direction, so the loops were honoured and every edge is shared.
    for e in &edges {
        let polyline = mesh.edge_polyline(e.id).unwrap();
        for w in polyline.windows(2) {
            assert_eq!(directed.get(&(w[0], w[1])).map(Vec::len), Some(1));
            assert_eq!(directed.get(&(w[1], w[0])).map(Vec::len), Some(1));
        }
    }
}

/// The faces (by iteration index) whose triangles run along `edge`'s
/// polyline, in either direction.
fn faces_along(m: &Model, body: Body, mesh: &TriMesh, edge: arris_topo::EdgeId) -> BTreeSet<usize> {
    let faces = m.faces(body).unwrap();
    let polyline = mesh.edge_polyline(edge).unwrap();
    let segments: BTreeSet<(u32, u32)> = polyline
        .windows(2)
        .flat_map(|w| [(w[0], w[1]), (w[1], w[0])])
        .collect();
    let mut out = BTreeSet::new();
    for (k, f) in faces.iter().enumerate() {
        for t in mesh.face_triangles(f.id).unwrap() {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                if segments.contains(&(a, b)) {
                    out.insert(k);
                }
            }
        }
    }
    out
}

#[test]
fn the_unit_box_meshes_exactly() {
    let mut m = Model::default();
    let body = sample::unit_box(&mut m).unwrap();
    let mesh = tessellate(&m, body, 1e-3).unwrap();
    assert_structure(&m, body, &mesh);
    assert_eq!(mesh.positions().len(), 8);
    assert_eq!(mesh.triangles().len(), 12);
    assert!((mesh.signed_volume().unwrap() - 1.0).abs() <= EXACT);
    assert!((mesh.area() - 6.0).abs() <= EXACT);
    assert_eq!(mesh, tessellate(&m, body, 1e-3).unwrap(), "two runs");
}

#[test]
fn the_frame_meshes_with_its_window_open() {
    let mut m = Model::default();
    let body = sample::frame(
        &mut m,
        Point3::origin(),
        Point3::new(40.0, 30.0, 10.0),
        Point2::new(10.0, 10.0),
        Point2::new(30.0, 20.0),
    )
    .unwrap();
    let mesh = tessellate(&m, body, 1e-3).unwrap();
    assert_structure(&m, body, &mesh);
    assert_eq!(mesh.positions().len(), 16);
    let volume = 40.0 * 30.0 * 10.0 - 20.0 * 10.0 * 10.0;
    assert!((mesh.signed_volume().unwrap() - volume).abs() <= EXACT * volume);
    // Every edge lies in exactly two faces; the window's edges in the
    // two-loop face and an inner wall.
    for e in m.edges(body).unwrap() {
        assert_eq!(faces_along(&m, body, &mesh, e.id).len(), 2, "{}", e.id);
    }
    let two_loop: Vec<_> = m
        .faces(body)
        .unwrap()
        .iter()
        .filter(|f| m.face(f.id).unwrap().loops().len() == 2)
        .map(|f| f.id)
        .collect();
    assert_eq!(two_loop.len(), 2);
    for f in two_loop {
        assert_eq!(mesh.face_triangles(f).unwrap().len(), 8);
    }
    assert_eq!(mesh, tessellate(&m, body, 1e-3).unwrap(), "two runs");
}

/// The mesh of a cylinder of radius `r`, height `h` at `chord`: closed,
/// the seam used twice, and its volume the inscribed prism's exactly —
/// `sin θ / θ` of the true volume for `θ = 2π / n` — with the closed
/// form `4δ / (3r)` bounding the error.
fn assert_cylinder(m: &Model, body: Body, mesh: &TriMesh, r: f64, h: f64, chord: f64) {
    assert_structure(m, body, mesh);
    let edges = m.edges(body).unwrap();
    let faces = m.faces(body).unwrap();
    // The seam is the one edge with distinct end vertices; it lies in
    // the wall alone, in both directions.
    let seam = edges
        .iter()
        .find(|e| !m.edge(e.id).unwrap().is_closed())
        .unwrap();
    let wall = faces
        .iter()
        .position(|f| m.face(f.id).unwrap().loops()[0].coedges().len() == 4)
        .unwrap();
    assert_eq!(faces_along(m, body, mesh, seam.id), BTreeSet::from([wall]));
    let rim = edges
        .iter()
        .find(|e| m.edge(e.id).unwrap().is_closed())
        .unwrap();
    let n = mesh.edge_polyline(rim.id).unwrap().len() - 1;
    assert!(n >= MIN_SEGMENTS_PER_TURN);
    let theta = TAU / n as f64;
    let exact = PI * r * r * h;
    let volume = mesh.signed_volume().unwrap();
    let ratio = volume / exact;
    assert!(
        (ratio - theta.sin() / theta).abs() <= 1e-9,
        "ratio {ratio} vs sin θ / θ {}",
        theta.sin() / theta
    );
    let error = 1.0 - ratio;
    assert!(error >= 0.0, "inscribed");
    assert!(
        error <= prism_bound(chord, r),
        "error {error} above 4δ/(3r) = {}",
        prism_bound(chord, r)
    );
    // The sagitta the mesh achieved is within the chord asked for.
    assert!(r * (1.0 - (theta / 2.0).cos()) <= chord * (1.0 + 1e-12));
}

#[test]
fn the_sample_cylinder_meshes_within_the_closed_form_bound() {
    let mut m = Model::default();
    let body = sample::cylinder(&mut m, 4.0, 12.0).unwrap();
    for chord in [1e-1, 1e-2, 1e-3, 1e-4] {
        let mesh = tessellate(&m, body, chord).unwrap();
        assert_cylinder(&m, body, &mesh, 4.0, 12.0, chord);
        assert_eq!(mesh, tessellate(&m, body, chord).unwrap(), "two runs");
    }
}

#[test]
fn both_primitives_mesh() {
    let mut m = Model::default();
    let (cube, _) = primitive_box(&mut m, Point3::origin(), Point3::new(40.0, 30.0, 10.0)).unwrap();
    let mesh = tessellate(&m, cube, 1e-3).unwrap();
    assert_structure(&m, cube, &mesh);
    assert!((mesh.signed_volume().unwrap() - 12000.0).abs() <= EXACT * 12000.0);
    let (cylinder, _) =
        primitive_cylinder(&mut m, Axis::z_at(Point3::origin()), 4.0, 12.0).unwrap();
    let mesh = tessellate(&m, cylinder, 1e-2).unwrap();
    assert_cylinder(&m, cylinder, &mesh, 4.0, 12.0, 1e-2);
}

#[test]
fn random_cylinders_in_random_poses_mesh_within_the_bound() {
    check(
        (
            radius(0.1..=10.0),
            radius(0.1..=DEFAULT_SCALE),
            point_in_box(DEFAULT_SCALE),
            unit_vec3(),
            finite_f64(-5.0..=-1.0),
        ),
        |(r, h, origin, direction, log_chord)| {
            let chord = 10f64.powf(log_chord) * r;
            let mut m = Model::default();
            let axis = Axis::new(origin, direction.into_inner()).unwrap();
            let (body, _) = primitive_cylinder(&mut m, axis, r, h).unwrap();
            let mesh =
                tessellate(&m, body, chord).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert!(mesh.is_closed());
            let n = mesh
                .edge_polyline(m.edges(body).unwrap()[0].id)
                .unwrap()
                .len()
                - 1;
            let theta = TAU / n as f64;
            let exact = PI * r * r * h;
            let volume = mesh.signed_volume().unwrap();
            let ratio = volume / exact;
            prop_assert!(
                (ratio - theta.sin() / theta).abs() <= 1e-9,
                "ratio {ratio} vs {}",
                theta.sin() / theta
            );
            prop_assert!(1.0 - ratio <= prism_bound(chord, r), "above the bound");
            prop_assert!(1.0 - ratio >= -1e-12, "not inscribed");
            prop_assert_eq!(
                &mesh,
                &tessellate(&m, body, chord).map_err(|e| TestCaseError::fail(e.to_string()))?
            );
            Ok(())
        },
    );
}

#[test]
fn a_bad_chord_is_a_typed_error() {
    let mut m = Model::default();
    let body = sample::unit_box(&mut m).unwrap();
    for chord in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        match tessellate(&m, body, chord) {
            Err(MeshError::Chord(c)) => {
                assert!(c.is_nan() == chord.is_nan() && (c.is_nan() || c == chord))
            }
            other => panic!("{chord}: {other:?}"),
        }
    }
    let err = tessellate(&m, body, -1.0).unwrap_err();
    assert_eq!(
        err.to_string(),
        "chord tolerance -1 is not finite and positive"
    );
}

#[test]
fn a_body_that_does_not_resolve_is_not_found() {
    let m = Model::default();
    let body = Body::forward(arris_topo::BodyId::new(3, 0));
    assert!(matches!(
        tessellate(&m, body, 1e-3),
        Err(MeshError::NotFound(_))
    ));
}

#[cfg(debug_assertions)]
#[test]
fn a_broken_body_is_invalid_input_in_a_debug_build() {
    let mut m = Model::default();
    // A solid whose one shell does not exist: the checker's M1.
    let body = m
        .raw()
        .add_body(BodyEntity::solid(vec![ShellHandle::forward(ShellId::new(
            9, 0,
        ))]));
    let err = tessellate(&m, Body::forward(body), 1e-3).unwrap_err();
    assert!(matches!(err, MeshError::InvalidInput { .. }), "{err}");
    assert!(err.to_string().contains("fails the checker"));
}
