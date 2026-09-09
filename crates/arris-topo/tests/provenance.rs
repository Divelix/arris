//! The provenance record (`docs/DATA-MODEL.md` §Provenance, step 10):
//! `origins` inverts `generated_from` and `modified_from`, `then` is
//! associative on random small records and reports against the first
//! inputs, `mapped` translates through an `IdMap`, and `is_kept` is the
//! query for what is not recorded.

use std::collections::BTreeSet;

use arris_topo::provenance::{
    BoxPart, Coord, CylinderPart, Origin, Provenance, Relation, Role, Side,
};
use arris_topo::{EdgeId, FaceId, IdMap, Model, Orientation, Shape, VertexId};
use proptest::prelude::*;

fn face(i: u32) -> Shape {
    Shape::new(FaceId::new(i, 0), Orientation::Forward)
}

fn edge(i: u32) -> Shape {
    Shape::new(EdgeId::new(i, 0), Orientation::Forward)
}

#[test]
fn origins_inverts_generated_from_and_modified_from() {
    let mut p = Provenance::new();
    let tool = face(0);
    let target = face(1);
    p.add_generated(tool, face(10));
    p.add_generated(tool, edge(20));
    p.add_generated(target, edge(20));
    p.add_modified(target, face(11));
    p.add_modified(target, face(12));
    p.add_deleted(face(2));
    assert_eq!(
        p.generated_from(tool),
        [face(10), edge(20)]
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    );
    assert_eq!(p.modified_from(target), [face(11), face(12)]);
    assert_eq!(p.generated_pair(tool, target), [edge(20)]);
    assert!(p.generated_pair(tool, face(9)).is_empty());
    assert!(p.is_deleted(face(2)) && !p.is_deleted(face(1)));
    for origin in p.origins_recorded() {
        for &out in p.generated_from(origin) {
            assert!(p.origins(out).contains(&(Relation::Generated, origin)));
        }
        for &out in p.modified_from(origin) {
            assert!(p.origins(out).contains(&(Relation::Modified, origin)));
        }
    }
    assert_eq!(
        p.origins(edge(20)),
        [
            (Relation::Generated, Origin::Entity(tool)),
            (Relation::Generated, Origin::Entity(target))
        ]
    );
    assert!(p.origins(face(99)).is_empty());
    assert_eq!(
        p.to_string(),
        "+f0 generated +e20 +f10\n+f1 generated +e20\n+f1 modified +f11 +f12\n+f2 deleted\n"
    );
}

#[test]
fn then_reports_against_the_first_inputs() {
    // A primitive, then a cut that splits its top face and deletes a
    // side, then a cut that modifies one of the pieces.
    let top = Role::Box(BoxPart::Face(Coord::Z, Side::Max));
    let side = Role::Box(BoxPart::Face(Coord::X, Side::Min));
    let mut a = Provenance::new();
    a.add_generated(top, face(0));
    a.add_generated(side, face(1));
    let mut b = Provenance::new();
    b.add_modified(face(0), face(5));
    b.add_modified(face(0), face(6));
    b.add_deleted(face(1));
    b.add_generated(face(3), face(7)); // the tool's wall, not an output of `a`
    let mut c = Provenance::new();
    c.add_modified(face(5), face(8));
    c.add_deleted(face(6));
    let ab = a.then(&b);
    assert_eq!(
        ab.generated_from(top),
        [face(5), face(6)],
        "pieces of a generated face are generated from its role"
    );
    assert!(ab.generated_from(side).is_empty());
    assert!(
        !ab.is_deleted(face(1)),
        "an intermediate entity is nobody's input: it vanishes, and its role has no image"
    );
    assert_eq!(ab.generated_from(face(3)), [face(7)], "carried");
    let abc = ab.then(&c);
    assert_eq!(abc.generated_from(top), [face(8)]);
    assert!(
        !abc.is_deleted(face(6)) && !abc.is_deleted(face(1)),
        "intermediates vanish"
    );
    assert_eq!(
        abc.origins(face(8)),
        [(Relation::Generated, Origin::Role(top))],
        "the chain ends at the role"
    );
    assert_eq!(abc, a.then(&b.then(&c)));
    // An entity whose every image is deleted is deleted.
    let mut d = Provenance::new();
    d.add_modified(face(0), face(5));
    let mut e = Provenance::new();
    e.add_deleted(face(5));
    let de = d.then(&e);
    assert!(de.is_deleted(face(0)));
    assert!(de.modified_from(face(0)).is_empty());
    assert!(
        !de.is_deleted(face(5)),
        "an intermediate entity is nobody's input"
    );
    // Modified of modified stays modified; anything through generated is generated.
    let mut f = Provenance::new();
    f.add_modified(face(5), face(9));
    assert_eq!(
        d.then(&f).origins(face(9)),
        [(Relation::Modified, Origin::Entity(face(0)))]
    );
    let mut g = Provenance::new();
    g.add_generated(face(5), edge(9));
    assert_eq!(
        d.then(&g).origins(edge(9)),
        [(Relation::Generated, Origin::Entity(face(0)))]
    );
}

/// The raw entries of a random small record: `(origin, output, generated)`
/// over inputs `0..inputs` and fresh outputs `inputs..inputs + 4`, and
/// the inputs deleted.
type Raw = (Vec<(Origin, u32, bool)>, Vec<u32>);

fn raw(inputs: u32) -> impl Strategy<Value = Raw> {
    let origin = prop_oneof![
        6 => (0..inputs).prop_map(|i| Origin::Entity(face(i))),
        1 => Just(Origin::Role(Role::Box(BoxPart::Face(Coord::Z, Side::Max)))),
        1 => Just(Origin::Role(Role::Cylinder(CylinderPart::Wall))),
    ];
    (
        proptest::collection::vec((origin, inputs..inputs + 4, any::<bool>()), 0..6),
        proptest::collection::vec(0..inputs, 0..3),
    )
}

/// A well-formed chain of three records, as three operations in a row
/// produce: an output is a new entity, never an input, and a record
/// names only what exists when it runs — an entity an earlier record
/// modified or deleted is gone, one it generated from or left alone is
/// still there.
fn chain() -> impl Strategy<Value = (Provenance, Provenance, Provenance)> {
    (raw(4), raw(8), raw(12)).prop_map(|(a, b, c)| {
        let mut consumed: BTreeSet<Shape> = BTreeSet::new();
        let mut build = |(entries, deleted): Raw| {
            let mut p = Provenance::new();
            for (origin, out, generated) in entries {
                if let Origin::Entity(s) = origin {
                    if consumed.contains(&s) {
                        continue;
                    }
                }
                if generated {
                    p.add_generated(origin, face(out));
                } else {
                    p.add_modified(origin, face(out));
                }
            }
            for d in deleted {
                let s = face(d);
                if consumed.contains(&s) || !p.origins_recorded().all(|o| o != Origin::Entity(s)) {
                    continue;
                }
                p.add_deleted(s);
            }
            consumed.extend(p.deleted());
            for o in p.origins_recorded() {
                if let Origin::Entity(s) = o {
                    if !p.modified_from(o).is_empty() {
                        consumed.insert(s);
                    }
                }
            }
            p
        };
        let a = build(a);
        let b = build(b);
        let c = build(c);
        (a, b, c)
    })
}

#[test]
fn then_is_associative_on_random_small_records() {
    arris_debug::prop::check(chain(), |(a, b, c)| {
        prop_assert_eq!(a.then(&b).then(&c), a.then(&b.then(&c)));
        prop_assert_eq!(
            a.then(&Provenance::new()),
            a.clone(),
            "the empty record is neutral on the right"
        );
        prop_assert_eq!(Provenance::new().then(&a), a.clone(), "and on the left");
        Ok(())
    });
}

#[test]
fn mapped_translates_what_the_map_holds_and_keeps_the_rest() {
    let mut p = Provenance::new();
    let role = Role::Cylinder(CylinderPart::Seam);
    p.add_generated(role, edge(3));
    p.add_modified(face(1), face(4));
    p.add_deleted(face(2));
    let mut map = IdMap::default();
    map.edges.insert(EdgeId::new(3, 0), EdgeId::new(0, 0));
    map.faces.insert(FaceId::new(4, 0), FaceId::new(1, 0));
    let q = p.mapped(&map);
    assert_eq!(q.generated_from(role), [edge(0)]);
    assert_eq!(
        q.modified_from(face(1)),
        [face(1)],
        "the origin was not imported; the output was"
    );
    assert!(q.is_deleted(face(2)));
    assert_eq!(map.len(), 2);
    assert!(
        map.map(Shape::new(VertexId::new(0, 0), Orientation::Forward))
            .is_none()
    );
}

#[test]
fn is_kept_is_untouched_and_present() {
    let mut m = Model::default();
    let body = arris_debug::sample::cylinder(&mut m, 4.0, 12.0).unwrap();
    let c = m.closure(body).unwrap();
    let wall = Shape::new(c.faces[0], Orientation::Forward);
    let cap = Shape::new(c.faces[1], Orientation::Forward);
    let mut p = Provenance::new();
    p.add_modified(cap, face(9));
    assert!(p.is_kept(wall, &m, body));
    assert!(!p.is_kept(cap, &m, body), "modified");
    assert!(!p.is_kept(face(9), &m, body), "not in the body");
    assert!(p.is_kept(body, &m, body));
    p.add_deleted(wall);
    assert!(!p.is_kept(wall, &m, body));
    assert!(
        !Provenance::new().is_kept(
            wall,
            &m,
            Shape::new(arris_topo::BodyId::new(7, 0), Orientation::Forward)
                .try_into()
                .unwrap()
        )
    );
}
