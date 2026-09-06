//! `import` and `retain` (`docs/plans/m2-topology.md` step 12): an
//! imported body dumps identically up to the id map and its provenance
//! translates through `Provenance::mapped`; importing twice gives
//! distinct ids; `retain` frees what is unreachable, the survivors are
//! untouched and clean, freed slots are reused lowest-first at the next
//! generation, and two models built by the same calls agree on every id.

use arris_debug::{dump_text, sample};
use arris_math::{Point3, Precision};
use arris_topo::entity::Vertex;
use arris_topo::provenance::{CylinderPart, Role};
use arris_topo::{
    Body, BodyId, IdMap, Model, NotFound, Orientation, Provenance, Shape, TopoError, VertexId,
};

/// The dump with every id token translated through `map`.
fn translate(dump: &str, map: &IdMap) -> String {
    let mut out = String::new();
    let mut token = String::new();
    let flush = |token: &mut String, out: &mut String| {
        if token.is_empty() {
            return;
        }
        let mapped = map_token(token, map).unwrap_or_else(|| token.clone());
        out.push_str(&mapped);
        token.clear();
    };
    for ch in dump.chars() {
        if ch.is_ascii_alphanumeric() {
            token.push(ch);
        } else {
            flush(&mut token, &mut out);
            out.push(ch);
        }
    }
    flush(&mut token, &mut out);
    out
}

/// `v3` → the mapped id's text, for the id kinds the dump prints.
fn map_token(token: &str, map: &IdMap) -> Option<String> {
    let (prefix, rest) = token.split_at(1);
    let (index, generation) = match rest.split_once('g') {
        Some((i, g)) => (i.parse::<u32>().ok()?, g.parse::<u32>().ok()?),
        None => (rest.parse::<u32>().ok()?, 0),
    };
    Some(match prefix {
        "v" => map
            .vertices
            .get(&VertexId::new(index, generation))?
            .to_string(),
        "e" => map
            .edges
            .get(&arris_topo::EdgeId::new(index, generation))?
            .to_string(),
        "f" => map
            .faces
            .get(&arris_topo::FaceId::new(index, generation))?
            .to_string(),
        "s" => map
            .shells
            .get(&arris_topo::ShellId::new(index, generation))?
            .to_string(),
        "b" => map.bodies.get(&BodyId::new(index, generation))?.to_string(),
        "c" => map
            .curves
            .get(&arris_topo::CurveId::new(index, generation))?
            .to_string(),
        "S" => map
            .surfaces
            .get(&arris_topo::SurfaceId::new(index, generation))?
            .to_string(),
        "p" => map
            .curve2s
            .get(&arris_topo::Curve2Id::new(index, generation))?
            .to_string(),
        _ => return None,
    })
}

#[test]
fn an_imported_cylinder_dumps_identically_up_to_the_id_map() {
    let mut a = Model::default();
    let cylinder = sample::cylinder(&mut a, 4.0, 12.0).unwrap();
    // Into a model that already holds a box, so every id moves.
    let mut b = Model::default();
    sample::unit_box(&mut b).unwrap();
    let (copy, map) = b.import(&a, cylinder).unwrap();
    assert_eq!(map.map(cylinder.into()), Some(copy.into()));
    assert_eq!(map.len(), 2 + 3 + 3 + 1 + 1 + 3 + 3 + 6);
    let original = dump_text(&a, cylinder).unwrap();
    let imported = dump_text(&b, copy).unwrap();
    assert_ne!(original, imported, "the ids moved");
    assert_eq!(translate(&original, &map), imported);
    let report = arris_check::check(&b, copy, arris_check::Level::Full);
    assert!(report.is_ok(), "{report}");
    // Provenance follows through `mapped`.
    let c = a.closure(cylinder).unwrap();
    let mut p = Provenance::new();
    p.add_generated(
        Role::Cylinder(CylinderPart::Wall),
        Shape::new(c.faces[0], Orientation::Forward),
    );
    p.add_generated(Role::Cylinder(CylinderPart::Body), cylinder);
    let q = p.mapped(&map);
    assert_eq!(
        q.generated_from(Role::Cylinder(CylinderPart::Wall)),
        [Shape::new(map.faces[&c.faces[0]], Orientation::Forward)]
    );
    assert_eq!(
        q.generated_from(Role::Cylinder(CylinderPart::Body)),
        [copy.into()]
    );
    // Twice: distinct ids, the same shape.
    let (again, map2) = b.import(&a, cylinder).unwrap();
    assert_ne!(again, copy);
    assert!(
        map2.faces
            .values()
            .all(|f| !map.faces.values().any(|g| g == f))
    );
    assert_eq!(translate(&original, &map2), dump_text(&b, again).unwrap());
    // A reversed handle keeps its orientation.
    let (flipped, _) = b.import(&a, cylinder.reversed()).unwrap();
    assert_eq!(flipped.orientation, Orientation::Reversed);
}

#[test]
fn importing_a_body_with_a_dangling_reference_is_not_found_and_appends_nothing() {
    let mut a = Model::default();
    let dangling = a.raw().add_body(arris_topo::entity::Body::solid(vec![
        arris_topo::Shell::forward(arris_topo::ShellId::new(9, 0)),
    ]));
    let mut b = Model::default();
    let r = b.import(&a, Body::forward(dangling));
    assert!(
        matches!(r, Err(TopoError::NotFound(NotFound { .. }))),
        "{r:?}"
    );
    assert!(b.body(BodyId::new(0, 0)).is_err());
    assert!(
        b.import(&a, Body::forward(BodyId::new(5, 0))).is_err(),
        "a body that does not resolve"
    );
}

#[test]
fn retain_frees_the_unreachable_and_reuses_the_slots_at_the_next_generation() {
    let build = |m: &mut Model| {
        let cube = sample::cuboid(m, Point3::origin(), Point3::new(40.0, 30.0, 10.0)).unwrap();
        let cylinder = sample::cylinder(m, 4.0, 12.0).unwrap();
        (cube, cylinder)
    };
    let mut m = Model::default();
    let (cube, cylinder) = build(&mut m);
    let cube_dump = dump_text(&m, cube).unwrap();
    let cube_closure = m.closure(cube).unwrap();
    let cylinder_closure = m.closure(cylinder).unwrap();
    let freed = m.retain(&[cube]).unwrap();
    assert_eq!(freed, 2 + 3 + 3 + 1 + 1 + 3 + 3 + 6, "every cylinder slot");
    // The cube is untouched and clean; every cylinder handle is gone.
    assert_eq!(dump_text(&m, cube).unwrap(), cube_dump);
    assert_eq!(m.closure(cube).unwrap(), cube_closure);
    let report = arris_check::check(&m, cube, arris_check::Level::Full);
    assert!(report.is_ok(), "{report}");
    assert!(m.body(cylinder.id).is_err());
    for &v in &cylinder_closure.vertices {
        assert!(m.vertex(v).is_err());
        assert!(m.vertex_edges(v).is_err());
    }
    for &e in &cylinder_closure.edges {
        assert!(m.edge(e).is_err());
    }
    for &f in &cylinder_closure.faces {
        assert!(m.face(f).is_err());
    }
    for &s in &cylinder_closure.surfaces {
        assert!(m.surface(s).is_err());
    }
    // The next body fills the freed slots, lowest first, at generation one.
    let again = sample::cylinder(&mut m, 4.0, 12.0).unwrap();
    assert_eq!(again.id, BodyId::new(1, 1));
    let c = m.closure(again).unwrap();
    assert_eq!(
        c.vertices,
        cylinder_closure
            .vertices
            .iter()
            .map(|v| VertexId::new(v.index(), 1))
            .collect::<Vec<_>>()
    );
    assert!(c.faces.iter().all(|f| f.generation() == 1));
    assert!(
        m.vertex(cylinder_closure.vertices[0]).is_err(),
        "the stale id still fails"
    );
    assert_eq!(dump_text(&m, cube).unwrap(), cube_dump);
    let report = arris_check::check(&m, again, arris_check::Level::Full);
    assert!(report.is_ok(), "{report}");
    assert_eq!(
        m.edge_uses(c.edges[1]).unwrap().len(),
        2,
        "the indices were rebuilt"
    );
    // Two models built by the same calls agree on every id after a retain.
    let mut n = Model::default();
    let (cube2, _) = build(&mut n);
    n.retain(&[cube2]).unwrap();
    let again2 = sample::cylinder(&mut n, 4.0, 12.0).unwrap();
    assert_eq!(again2, again);
    assert_eq!(
        dump_text(&n, again2).unwrap(),
        dump_text(&m, again).unwrap()
    );
    // A body in `keep` that does not resolve frees nothing.
    let before = dump_text(&m, again).unwrap();
    assert!(m.retain(&[cube, Body::forward(BodyId::new(9, 0))]).is_err());
    assert_eq!(dump_text(&m, again).unwrap(), before);
    assert_eq!(m.retain(&[cube, again]).unwrap(), 0, "nothing left to free");
    // Everything gone.
    assert!(m.retain(&[]).unwrap() > 0);
    assert!(m.body(cube.id).is_err());
    let fresh = sample::unit_box(&mut m).unwrap();
    assert_eq!(
        fresh.id,
        BodyId::new(0, 1),
        "the lowest slot, one generation up"
    );
}

#[test]
fn a_failed_transaction_after_a_retain_empties_the_slots_it_filled() {
    let mut m = Model::default();
    let cube = sample::unit_box(&mut m).unwrap();
    let cylinder = sample::cylinder(&mut m, 4.0, 12.0).unwrap();
    m.retain(&[cube]).unwrap();
    let freed_vertex = VertexId::new(8, 1);
    let r: Result<(), ()> = m.transaction(|m| {
        let v = m.raw().add_vertex(Vertex::new(Point3::origin(), 1e-7));
        assert_eq!(v, freed_vertex, "the lowest freed vertex slot");
        Err(())
    });
    assert!(r.is_err());
    assert!(m.vertex(freed_vertex).is_err(), "emptied again");
    let v = m.raw().add_vertex(Vertex::new(Point3::origin(), 1e-7));
    assert_eq!(v, freed_vertex, "and handed out again with the same id");
    // A retain inside a failed transaction stays.
    let r: Result<(), ()> = m.transaction(|m| {
        m.retain(&[]).unwrap();
        Err(())
    });
    assert!(r.is_err());
    assert!(m.body(cube.id).is_err() && m.body(cylinder.id).is_err());
    let _ = Precision::DEFAULT;
}
