//! `ops::boolean::interferences` (`docs/plans/m4-booleans.md` step 6):
//! the pave model on the corpus's boolean fixtures — the section curves,
//! their paves and the hits that made them — at random poses of a box
//! and a cylinder, and identical over two runs.

use arris_debug::{corpus, fixtures, prop, sample};
use arris_ops::OpError;
use arris_ops::arris_check::arris_topo::arris_geom::{
    Curve, Curve2, GeomKind, Surface, SurfaceIntersection, SurfaceKind,
};
use arris_ops::arris_check::arris_topo::arris_math::{Interval, Point3};
use arris_ops::arris_check::arris_topo::{Body, EdgeId, Model};
use arris_ops::boolean::{Interferences, Landing, interferences};
use arris_ops::primitive_box;
use proptest::prelude::*;

/// The operands of a boolean fixture, built by its recipe.
fn inputs(name: &str) -> (Model, Body, Body) {
    let dir = fixtures::corpus_root().join(name);
    let inputs = corpus::inputs(&dir, "default").unwrap();
    let (a, b) = inputs.operands().unwrap();
    (inputs.model, a, b)
}

fn interferences_of(name: &str) -> (Model, Body, Body, Interferences) {
    let (m, a, b) = inputs(name);
    let i = interferences(&m, a, b).unwrap();
    (m, a, b, i)
}

/// The edges of `body` whose curve is a line, in iteration order.
fn line_edges(m: &Model, body: Body) -> Vec<EdgeId> {
    m.edges(body)
        .unwrap()
        .into_iter()
        .filter(|e| {
            let edge = m.edge(e.id).unwrap();
            edge.curve()
                .is_some_and(|(c, _)| matches!(m.curve(c).unwrap(), Curve::Line { .. }))
        })
        .map(|e| e.id)
        .collect()
}

fn samples(range: Interval, n: usize) -> impl Iterator<Item = f64> {
    (0..n).map(move |i| range.lerp(i as f64 / (n - 1) as f64))
}

/// The surfaces of a section edge's two faces.
fn surfaces_of<'m>(m: &'m Model, i: &Interferences, section: usize) -> [&'m Surface; 2] {
    let pair = &i.pairs[i.curves[i.sections[section].curve].pair];
    [pair.a, pair.b].map(|f| m.surface(m.face(f).unwrap().surface()).unwrap())
}

/// Every section edge lies on both surfaces, its pcurves are
/// same-parameter with it within its tolerance, its ends are its
/// vertices, and its vertices are at least as tolerant as it.
fn assert_sections_consistent(m: &Model, i: &Interferences) -> Result<(), TestCaseError> {
    for (k, s) in i.sections.iter().enumerate() {
        let curve = &i.curves[s.curve].curve;
        let surfaces = surfaces_of(m, i, k);
        for t in samples(s.range, 23) {
            let p = curve.point(t);
            for (which, surface) in surfaces.iter().enumerate() {
                let d = surface
                    .project(p)
                    .map_err(|e| TestCaseError::fail(e.to_string()))?
                    .distance;
                prop_assert!(d <= s.tolerance, "s{k} off surface {which} by {d}\n{i}");
                let q = s.pcurves[which].point(t);
                let gap = (surface.point(q.x, q.y) - p).norm();
                prop_assert!(
                    gap <= s.tolerance,
                    "s{k} pcurve {which} not same-parameter: {gap} at t {t}\n{i}"
                );
            }
        }
        for (end, t) in [(s.start, s.range.lo()), (s.end, s.range.hi())] {
            let v = &i.vertices[end];
            let gap = (curve.point(t) - v.point).norm();
            prop_assert!(gap <= v.tolerance, "s{k} end v{end} off by {gap}\n{i}");
            prop_assert!(v.tolerance >= s.tolerance, "v{end} below s{k}\n{i}");
        }
    }
    for (edge, paves) in &i.paves {
        let e = m.edge(*edge).unwrap();
        let (c, range) = e.curve().unwrap();
        let curve = m.curve(c).unwrap();
        for p in paves {
            prop_assert!(
                range.contains(p.t),
                "{edge} pave t {} outside {range:?}",
                p.t
            );
            let v = &i.vertices[p.vertex];
            let gap = (curve.point(p.t) - v.point).norm();
            prop_assert!(
                gap <= v.tolerance,
                "{edge} pave v{} off by {gap}\n{i}",
                p.vertex
            );
        }
    }
    Ok(())
}

#[test]
fn through_hole_has_two_section_circles_each_paved_once_at_the_seam() {
    let (m, _, hole, i) = interferences_of("boolean/through-hole");
    let seam = line_edges(&m, hole);
    assert_eq!(seam.len(), 1);
    assert_eq!(i.hits.len(), 2, "{i}");
    for h in &i.hits {
        assert_eq!(h.edge, seam[0], "{i}");
        assert_eq!(h.landing, Landing::Interior);
        assert!(!h.tangent && h.at_vertex.is_none() && h.vertex.is_some());
    }
    assert_eq!(i.vertices.len(), 2, "{i}");
    assert_eq!(i.paves.len(), 1, "only the seam is paved\n{i}");
    assert_eq!(i.paves[&seam[0]].len(), 2);
    assert_eq!(i.curves.len(), 2, "{i}");
    assert_eq!(i.sections.len(), 2, "{i}");
    for s in &i.sections {
        let c = &i.curves[s.curve];
        assert!(
            matches!(c.curve, Curve::Circle { radius, .. } if (radius - 4.0).abs() < 1e-12),
            "{i}"
        );
        assert_eq!(c.paves.len(), 1);
        assert_eq!(c.paves[0].t, 0.0, "the seam is the circle's parameter zero");
        assert_eq!(s.start, s.end);
        assert_eq!(s.range, Interval::TURN);
        assert!(
            matches!(s.pcurves[0], Curve2::Circle { .. }),
            "on the cap plane"
        );
        assert!(matches!(s.pcurves[1], Curve2::Line { .. }), "on the wall");
        assert!((s.tolerance - m.precision().default_tolerance).abs() < 1e-15);
    }
    assert!(i.coincident.is_empty());
    assert_sections_consistent(&m, &i).unwrap();
}

#[test]
fn blind_hole_has_one_section_circle() {
    let (m, _, _, i) = interferences_of("boolean/blind-hole");
    assert_eq!(i.hits.len(), 1, "{i}");
    assert_eq!(i.sections.len(), 1, "{i}");
    assert!(matches!(
        i.curves[i.sections[0].curve].curve,
        Curve::Circle { .. }
    ));
    assert_eq!(i.curves[i.sections[0].curve].paves.len(), 1);
    assert_sections_consistent(&m, &i).unwrap();
}

#[test]
fn frame_cut_has_eight_segments_paved_by_the_windows_vertical_edges() {
    let (m, _, window, i) = interferences_of("boolean/frame-cut");
    let vertical: Vec<EdgeId> = line_edges(&m, window)
        .into_iter()
        .filter(|&e| {
            let (c, _) = m.edge(e).unwrap().curve().unwrap();
            matches!(m.curve(c).unwrap(), Curve::Line { direction, .. } if direction.z.abs() > 0.5)
        })
        .collect();
    assert_eq!(vertical.len(), 4);
    assert_eq!(i.hits.len(), 8, "{i}");
    assert!(i.hits.iter().all(|h| vertical.contains(&h.edge)), "{i}");
    assert_eq!(i.vertices.len(), 8, "{i}");
    assert_eq!(i.sections.len(), 8, "{i}");
    let mut lengths: Vec<f64> = i.sections.iter().map(|s| s.range.length()).collect();
    lengths.sort_by(f64::total_cmp);
    for (found, wanted) in lengths
        .iter()
        .zip([10.0, 10.0, 10.0, 10.0, 20.0, 20.0, 20.0, 20.0])
    {
        assert!((found - wanted).abs() < 1e-9, "{lengths:?}\n{i}");
    }
    for s in &i.sections {
        assert!(matches!(i.curves[s.curve].curve, Curve::Line { .. }));
        assert_ne!(s.start, s.end);
        assert_eq!(i.curves[s.curve].paves.len(), 2);
    }
    assert_sections_consistent(&m, &i).unwrap();
}

#[test]
fn corner_union_has_six_unit_segments() {
    let (m, _, _, i) = interferences_of("boolean/corner-union");
    assert_eq!(i.hits.len(), 6, "{i}");
    assert_eq!(i.vertices.len(), 6, "{i}");
    assert_eq!(i.sections.len(), 6, "{i}");
    for s in &i.sections {
        assert!((s.range.length() - 1.0).abs() < 1e-12, "{i}");
    }
    assert_sections_consistent(&m, &i).unwrap();
}

#[test]
fn a_disjoint_pair_and_a_tangent_touch_have_no_section() {
    let (_, _, _, i) = interferences_of("boolean/disjoint-cut");
    assert!(
        i.pairs.is_empty() && i.hits.is_empty() && i.sections.is_empty(),
        "{i}"
    );

    let (_, _, _, i) = interferences_of("boolean/tangent-outside-cut");
    assert!(
        i.pairs
            .iter()
            .any(|p| matches!(p.intersection, SurfaceIntersection::Tangent(_))),
        "{i}"
    );
    assert!(
        i.hits.iter().all(|h| h.tangent && h.vertex.is_none()),
        "{i}"
    );
    assert!(i.vertices.is_empty(), "{i}");
    assert!(i.sections.is_empty(), "{i}");
}

#[test]
fn a_boss_has_one_section_circle_and_swallows_its_bottom_cap() {
    let (m, _, _, i) = interferences_of("boolean/boss");
    assert_eq!(i.sections.len(), 1, "{i}");
    assert!(matches!(
        i.curves[i.sections[0].curve].curve,
        Curve::Circle { .. }
    ));
    assert_eq!(i.hits.len(), 1, "{i}");
    assert_sections_consistent(&m, &i).unwrap();
}

#[test]
fn an_oblique_hole_has_two_ellipses_with_nurbs_pcurves_on_the_wall() {
    let (m, _, _, i) = interferences_of("boolean/oblique-hole");
    assert_eq!(i.sections.len(), 2, "{i}");
    for (k, s) in i.sections.iter().enumerate() {
        assert!(
            matches!(i.curves[s.curve].curve, Curve::Ellipse { .. }),
            "{i}"
        );
        assert!(
            matches!(s.pcurves[0], Curve2::Ellipse { .. }),
            "on the plate"
        );
        assert!(matches!(s.pcurves[1], Curve2::Nurbs(_)), "on the wall");
        assert_eq!(s.start, s.end);
        assert!((s.range.length() - core::f64::consts::TAU).abs() < 1e-12);
        // E4 on the wall: the surface along the pcurve is the curve at
        // the same parameter within the edge's tolerance, at the
        // checker's sample count.
        let wall = surfaces_of(&m, &i, k)[1];
        for t in samples(s.range, m.precision().check_samples) {
            let q = s.pcurves[1].point(t);
            let gap = (wall.point(q.x, q.y) - i.curves[s.curve].curve.point(t)).norm();
            assert!(gap <= s.tolerance, "{gap} at t {t}\n{i}");
        }
        // The pcurve lies in the wall's own copy of the domain.
        let Curve2::Nurbs(n) = &s.pcurves[1] else {
            unreachable!()
        };
        for p in n.control_points() {
            assert!(
                p.x >= -1e-6 && p.x <= core::f64::consts::TAU + 1e-6,
                "{p}\n{i}"
            );
        }
    }
    assert_sections_consistent(&m, &i).unwrap();
}

#[test]
fn two_runs_are_identical() {
    let (m, a, b, i) = interferences_of("boolean/through-hole");
    let again = interferences(&m, a, b).unwrap();
    assert_eq!(i, again);
    assert_eq!(i.to_string(), again.to_string());
    assert!(i.to_string().starts_with("interferences "));
}

#[test]
fn an_unsupported_surface_pair_names_the_faces() {
    let mut m = Model::default();
    let (cube, _) = primitive_box(
        &mut m,
        Point3::new(-1.0, -1.0, -1.0),
        Point3::new(1.0, 1.0, 1.0),
    )
    .unwrap();
    let ball = sample::sphere(&mut m, Point3::origin(), 1.5).unwrap();
    let err = interferences(&m, cube, ball).unwrap_err();
    match err {
        OpError::Unsupported { a, b } => {
            assert_eq!(a.0, GeomKind::Surface(SurfaceKind::Plane));
            assert_eq!(b.0, GeomKind::Surface(SurfaceKind::Sphere));
            assert!(
                m.faces(cube)
                    .unwrap()
                    .iter()
                    .any(|f| f.shape().id == a.1.id)
            );
        }
        other => panic!("{other}"),
    }
}

#[test]
fn random_overlapping_pairs_pave_consistently() {
    prop::check(prop::body::overlapping_pair(), |pair| {
        let mut m = Model::default();
        let (a, b) = pair
            .build(&mut m)
            .map_err(|e| TestCaseError::fail(e.to_string()))?;
        let i = interferences(&m, a, b).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert!(
            !i.sections.is_empty(),
            "the axis passes through the box\n{i}"
        );
        assert_sections_consistent(&m, &i)?;
        let again = interferences(&m, a, b).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(i.to_string(), again.to_string());
        Ok(())
    });
}

// ---- `ops::cut` (plan step 7): split, classify, assemble ----

use arris_ops::arris_check::arris_topo::arris_math::Point2;
use arris_ops::arris_check::arris_topo::{
    EntityId, Face as FaceHandle, Orientation, Origin, Provenance, Shape,
};
use arris_ops::arris_check::{Level, check};
use arris_ops::measure::mass_properties;
use arris_ops::{Reason, cut};

/// The fixture's operands cut, the model with them.
fn cut_of(name: &str) -> (Model, Body, Body, Body, Provenance) {
    let (mut m, a, b) = inputs(name);
    let (body, p) = cut(&mut m, a, b).unwrap();
    (m, a, b, body, p)
}

fn shape(id: impl Into<EntityId>) -> Shape {
    Shape::new(id, Orientation::Forward)
}

/// The through-hole's provenance table, exactly: the plate's top and
/// bottom faces `Modified` into one piece each with a hole loop, the four
/// sides kept and unrecorded, the tool's wall `Deleted` and its piece
/// `Generated` from it, the tool's caps, rims and vertices `Deleted`, the
/// seam `Deleted` and its middle piece `Generated`, two section vertices
/// `Generated` from the seam and each cap plane, two section edges the
/// `generated_pair` of the wall and each cap.
#[test]
fn through_hole_provenance_is_the_designed_table() {
    let (m, plate, hole, body, p) = cut_of("boolean/through-hole");
    let report = check(&m, body, Level::Full);
    assert!(report.is_ok() && report.unchecked().is_empty(), "{report}");
    assert_eq!(m.faces(body).unwrap().len(), 7);

    // The plate: which faces are the caps (z = 0 and z = 10).
    let plate_faces = m.faces(plate).unwrap();
    let is_cap = |f: &FaceHandle| {
        let surface = m.surface(m.face(f.id).unwrap().surface()).unwrap();
        matches!(surface, Surface::Plane { frame } if frame.z().z.abs() > 0.5)
    };
    let (caps, sides): (Vec<&FaceHandle>, Vec<&FaceHandle>) =
        plate_faces.iter().partition(|f| is_cap(f));
    assert_eq!((caps.len(), sides.len()), (2, 4));
    for f in &caps {
        let pieces = p.modified_from(shape(f.id));
        assert_eq!(pieces.len(), 1, "{p}");
        let piece: FaceHandle = pieces[0].try_into().unwrap();
        assert_eq!(m.face(piece.id).unwrap().loops().len(), 2, "a hole loop");
        assert!(!p.is_deleted(shape(f.id)));
    }
    let out = m.closure(body).unwrap();
    for f in &sides {
        assert!(p.is_kept(shape(f.id), &m, body), "{p}");
        assert!(out.faces.contains(&f.id));
    }
    for e in m.edges(plate).unwrap() {
        assert!(p.is_kept(shape(e.id), &m, body), "{e} {p}");
    }
    for v in m.vertices(plate).unwrap() {
        assert!(p.is_kept(shape(v.id), &m, body), "{v} {p}");
    }

    // The tool: everything deleted; the wall and the seam with a piece
    // generated from each, the rest with none.
    let tool_faces = m.faces(hole).unwrap();
    let wall = tool_faces
        .iter()
        .find(|f| {
            matches!(
                m.surface(m.face(f.id).unwrap().surface()).unwrap(),
                Surface::Cylinder { .. }
            )
        })
        .unwrap();
    for f in &tool_faces {
        assert!(p.is_deleted(shape(f.id)), "{p}");
        let generated = p.generated_from(shape(f.id));
        if f.id == wall.id {
            let faces: Vec<_> = generated
                .iter()
                .filter(|s| matches!(s.id, EntityId::Face(_)))
                .collect();
            assert_eq!(faces.len(), 1, "the wall's piece\n{p}");
        } else {
            assert!(generated.is_empty(), "a cap generates nothing\n{p}");
        }
    }
    let seam = line_edges(&m, hole);
    assert_eq!(seam.len(), 1);
    for e in m.edges(hole).unwrap() {
        assert!(p.is_deleted(shape(e.id)), "{p}");
        let generated = p.generated_from(shape(e.id));
        if e.id == seam[0] {
            let edges: Vec<_> = generated
                .iter()
                .filter(|s| matches!(s.id, EntityId::Edge(_)))
                .collect();
            assert_eq!(edges.len(), 1, "the seam's middle piece\n{p}");
        } else {
            assert!(generated.is_empty(), "a rim generates nothing\n{p}");
        }
    }
    for v in m.vertices(hole).unwrap() {
        assert!(p.is_deleted(shape(v.id)) && p.generated_from(shape(v.id)).is_empty());
    }

    // Section vertices and edges.
    let section_vertices: Vec<Shape> = p
        .generated_from(shape(seam[0]))
        .iter()
        .copied()
        .filter(|s| matches!(s.id, EntityId::Vertex(_)))
        .collect();
    assert_eq!(section_vertices.len(), 2, "{p}");
    for cap in &caps {
        let from_cap = p.generated_from(shape(cap.id));
        let vertices: Vec<_> = from_cap
            .iter()
            .filter(|s| section_vertices.contains(s))
            .collect();
        assert_eq!(vertices.len(), 1, "one section vertex per cap\n{p}");
        let pair = p.generated_pair(shape(wall.id), shape(cap.id));
        let edges: Vec<_> = pair
            .iter()
            .filter(|s| matches!(s.id, EntityId::Edge(_)))
            .collect();
        assert_eq!(edges.len(), 1, "one section edge per cap\n{p}");
    }

    // The shell and the body: modified from the plate's, the tool's
    // deleted.
    assert_eq!(p.modified_from(Origin::Entity(shape(plate.id))).len(), 1);
    assert!(p.is_deleted(Shape::from(hole)));

    // Nothing else: the record has exactly these outputs.
    assert_eq!(p.outputs().len(), 10, "{p}");
}

/// `frame-cut` is `sample::frame` built the other way: the same counts,
/// the same mass properties to 1e-12.
#[test]
fn frame_cut_is_the_hand_built_frame() {
    let (m, _, _, body, _) = cut_of("boolean/frame-cut");
    let mut twin = Model::default();
    let frame = sample::frame(
        &mut twin,
        Point3::origin(),
        Point3::new(40.0, 30.0, 10.0),
        Point2::new(10.0, 10.0),
        Point2::new(30.0, 20.0),
    )
    .unwrap();
    let (a, b) = (
        check(&m, body, Level::Full).euler().unwrap(),
        check(&twin, frame, Level::Full).euler().unwrap(),
    );
    assert_eq!(a.to_string(), b.to_string());
    let (x, y) = (
        mass_properties(&m, body).unwrap(),
        mass_properties(&twin, frame).unwrap(),
    );
    assert!((x.volume - y.volume).abs() <= 1e-12 * y.volume);
    assert!((x.area - y.area).abs() <= 1e-12 * y.area);
    assert!((x.centroid - y.centroid).norm() <= 1e-12 * 40.0);
    for i in 0..3 {
        for j in 0..3 {
            assert!((x.inertia[(i, j)] - y.inertia[(i, j)]).abs() <= 1e-12 * y.inertia[(2, 2)]);
        }
    }
}

/// A tool clear of the target: the result is a new shell over the
/// target's own faces, and the tool is entirely `Deleted`.
#[test]
fn a_disjoint_cut_keeps_every_face_of_the_target() {
    let (m, plate, tool, body, p) = cut_of("boolean/disjoint-cut");
    assert_eq!(m.faces(body).unwrap(), m.faces(plate).unwrap());
    for f in m.faces(tool).unwrap() {
        assert!(p.is_deleted(shape(f.id)) && p.generated_from(shape(f.id)).is_empty());
    }
    assert_eq!(p.outputs().len(), 2, "the shell and the body\n{p}");
}

/// The target inside the tool selects nothing; a tool that splits its
/// target makes two shells. Each is the typed refusal, and the model is
/// as it was.
#[test]
fn a_swallowed_target_and_a_split_target_are_typed_refusals() {
    for (name, want) in [
        ("boolean/swallow-cut", "empty"),
        ("boolean/split-cut", "two"),
    ] {
        let (mut m, a, b) = inputs(name);
        let before = arris_debug::dump_text(&m, a).unwrap();
        let faces = m.faces(a).unwrap().len();
        let err = cut(&mut m, a, b).unwrap_err();
        match (want, &err) {
            (
                "empty",
                OpError::Degenerate {
                    reason: Reason::Empty,
                    entities,
                },
            ) => {
                assert_eq!(entities.as_slice(), [Shape::from(a), Shape::from(b)]);
            }
            (
                "two",
                OpError::Degenerate {
                    reason: Reason::MultiShell { shells: 2 },
                    ..
                },
            ) => {}
            other => panic!("{name}: {other:?}"),
        }
        assert_eq!(arris_debug::dump_text(&m, a).unwrap(), before);
        assert_eq!(m.faces(a).unwrap().len(), faces);
    }
}

/// Two runs of every cut fixture give the same dump.
#[test]
fn two_cuts_are_identical() {
    for name in [
        "boolean/through-hole",
        "boolean/blind-hole",
        "boolean/frame-cut",
        "boolean/corner-cut",
        "boolean/disjoint-cut",
    ] {
        let (m1, _, _, b1, p1) = cut_of(name);
        let (m2, _, _, b2, p2) = cut_of(name);
        assert_eq!(
            arris_debug::dump_text(&m1, b1).unwrap(),
            arris_debug::dump_text(&m2, b2).unwrap(),
            "{name}"
        );
        assert_eq!(p1, p2, "{name}");
    }
}

/// Coincident faces are plan step 10's: a flush pair is refused, naming
/// both faces, before anything is split.
#[test]
fn a_flush_pair_is_refused_naming_the_faces() {
    let (mut m, a, b) = inputs("boolean/flush-union");
    let err = cut(&mut m, a, b).unwrap_err();
    match err {
        OpError::Unsupported {
            a: (ka, fa),
            b: (kb, fb),
        } => {
            assert_eq!(ka, GeomKind::Surface(SurfaceKind::Plane));
            assert_eq!(kb, GeomKind::Surface(SurfaceKind::Plane));
            assert!(m.faces(a).unwrap().iter().any(|f| f.shape().id == fa.id));
            assert!(m.faces(b).unwrap().iter().any(|f| f.shape().id == fb.id));
        }
        other => panic!("{other}"),
    }
}
