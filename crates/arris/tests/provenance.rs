//! The provenance chain of `provenance/bolt-pattern-rebuild`
//! (`docs/ROADMAP.md` §M4, `docs/DATA-MODEL.md` §Provenance): a plate and
//! eight bolt-hole tools built, the eight cuts composed with
//! [`Provenance::then`], and every entity of the result followed back to
//! the role it came from. The fixture is the same recipe under three
//! parameter sets, and the chain has to be the same in all three — a
//! consumer's persistent name is a function of the chain, so it must not
//! move when a dimension does. Eight of eight hole walls named through
//! the tool they were cut with is the milestone's number.
//!
//! The corpus test in `corpus.rs` holds the same fixture to the oracle's
//! numbers in each variant; this one reads only the records.

use std::collections::BTreeSet;

use arris::topo::provenance::{
    BoxPart, Coord, CylinderPart, Origin, Provenance, Relation, Role, Side,
};
use arris::topo::{EntityId, FaceId, Orientation, Shape};
use arris_debug::corpus::Chain;
use arris_debug::{corpus, fixtures};

/// The fixture every test here reads.
const FIXTURE: &str = "provenance/bolt-pattern-rebuild";

/// Its three parameter sets, `default` first.
const VARIANTS: [&str; 3] = ["default", "thicker-wider", "tighter"];

/// The steps that make a body from nothing: the plate, then the eight
/// tools in the order the cuts take them.
const PRIMITIVES: [&str; 9] = ["plate", "h0", "h1", "h2", "h3", "h4", "h5", "h6", "h7"];

/// The eight cuts, `result` being the last.
const CUTS: [&str; 8] = ["c0", "c1", "c2", "c3", "c4", "c5", "c6", "result"];

fn chain_of(variant: &str) -> Chain {
    let dir = fixtures::corpus_root().join(FIXTURE);
    corpus::chain(&dir, variant).unwrap_or_else(|e| panic!("{variant}: {e}"))
}

/// The records of `names` composed left to right.
fn composed(chain: &Chain, names: &[&str]) -> Provenance {
    names
        .iter()
        .skip(1)
        .fold(chain.steps[names[0]].provenance.clone(), |p, name| {
            p.then(&chain.steps[*name].provenance)
        })
}

/// The whole recipe: the nine bodies built, then the eight cuts.
fn whole(chain: &Chain) -> Provenance {
    let names: Vec<&str> = PRIMITIVES.iter().chain(CUTS.iter()).copied().collect();
    composed(chain, &names)
}

/// The faces among `shapes`, in order.
fn faces(shapes: &[Shape]) -> Vec<FaceId> {
    shapes
        .iter()
        .filter_map(|s| match s.id {
            EntityId::Face(f) => Some(f),
            _ => None,
        })
        .collect()
}

/// The forward handle to a face, which is how a face reaches a record.
fn forward(face: FaceId) -> Shape {
    Shape::new(face, Orientation::Forward)
}

/// The one face a step generated from `role`.
fn face_from(p: &Provenance, role: Role) -> FaceId {
    let found = faces(p.generated_from(role));
    assert_eq!(found.len(), 1, "{role} generated {found:?}");
    found[0]
}

/// Each of the eight hole walls is generated from the wall of the tool
/// that cut it, and from no other face: the chain of eight cuts names
/// which bolt hole a wall belongs to, which is what a rebuild needs.
#[test]
fn eight_of_eight_hole_walls_name_the_tool_they_came_from() {
    for variant in VARIANTS {
        let chain = chain_of(variant);
        let cuts = composed(&chain, &CUTS);
        let body = chain.result().unwrap();
        let closure = chain.model.closure(body).unwrap();
        let mut walls = Vec::new();
        for tool in &PRIMITIVES[1..] {
            let wall = face_from(
                &chain.steps[*tool].provenance,
                Role::Cylinder(CylinderPart::Wall),
            );
            let tool_wall = forward(wall);
            // The tool's face is gone and its image is the hole's wall
            // (`docs/DATA-MODEL.md` §Provenance, the tool rule).
            assert!(cuts.is_deleted(tool_wall), "{variant}: {tool}'s wall kept");
            let image = faces(cuts.generated_from(tool_wall));
            assert_eq!(image.len(), 1, "{variant}: {tool}'s wall made {image:?}");
            let hole = image[0];
            assert!(
                closure.faces.contains(&hole),
                "{variant}: {tool}'s image {hole:?} is not in the result"
            );
            assert!(
                matches!(
                    chain
                        .model
                        .surface(chain.model.face(hole).unwrap().surface()),
                    Ok(arris::geom::Surface::Cylinder { .. })
                ),
                "{variant}: {tool}'s image is not a cylindrical face"
            );
            let origins = cuts.origins(forward(hole));
            assert_eq!(
                origins,
                [(Relation::Generated, Origin::Entity(tool_wall))],
                "{variant}: {tool}'s image comes from more than its tool"
            );
            walls.push(hole);
        }
        let distinct: BTreeSet<FaceId> = walls.iter().copied().collect();
        assert_eq!(
            distinct.len(),
            8,
            "{variant}: {walls:?} are not eight walls"
        );
    }
}

/// Followed through the primitives too, every hole wall's origins end at
/// one role — `Role::Cylinder(CylinderPart::Wall)` — and the relation is
/// `Generated`, since a chain that passes through a role is generation
/// however the pieces were cut ([`Relation::then`]).
#[test]
fn every_hole_wall_ends_at_the_cylinder_wall_role() {
    let role = Role::Cylinder(CylinderPart::Wall);
    for variant in VARIANTS {
        let chain = chain_of(variant);
        let cuts = composed(&chain, &CUTS);
        let all = whole(&chain);
        let from_role: BTreeSet<FaceId> = faces(all.generated_from(role)).into_iter().collect();
        assert_eq!(from_role.len(), 8, "{variant}: {from_role:?}");
        for &wall in &from_role {
            assert_eq!(
                all.origins(forward(wall)),
                [(Relation::Generated, Origin::Role(role))],
                "{variant}: {wall:?} comes from more than the wall role"
            );
        }
        // The same eight faces the eight cuts name through their tools.
        let images: BTreeSet<FaceId> = PRIMITIVES[1..]
            .iter()
            .flat_map(|tool| {
                let wall = forward(face_from(&chain.steps[*tool].provenance, role));
                faces(cuts.generated_from(wall))
            })
            .collect();
        assert_eq!(from_role, images, "{variant}");
    }
}

/// The plate's top face is modified by the chain of cuts into one face,
/// and that face carries nine loops: its outer boundary and the eight
/// bolt holes. Through the primitives as well it is `Generated` from
/// `Role::Box(Face(Z, Max))`, not `Modified` — the relation of a chain
/// that starts at a role is generation.
#[test]
fn the_plates_top_face_is_one_face_of_nine_loops() {
    let role = Role::Box(BoxPart::Face(Coord::Z, Side::Max));
    for variant in VARIANTS {
        let chain = chain_of(variant);
        let top = forward(face_from(&chain.steps["plate"].provenance, role));
        let cuts = composed(&chain, &CUTS);
        let pieces = faces(cuts.modified_from(top));
        assert_eq!(pieces.len(), 1, "{variant}: the top became {pieces:?}");
        let drilled = pieces[0];
        assert_eq!(
            chain.model.face(drilled).unwrap().loops().len(),
            9,
            "{variant}: one outer loop and eight holes"
        );
        assert!(
            cuts.origins(forward(drilled))
                .contains(&(Relation::Modified, Origin::Entity(top))),
            "{variant}: the drilled top is not a piece of the plate's top"
        );
        let all = whole(&chain);
        assert_eq!(
            faces(all.generated_from(role)),
            [drilled],
            "{variant}: the chain from the role does not reach the drilled top"
        );
        assert!(all.modified_from(role).is_empty(), "{variant}");
        // The four sides the tools never touched are kept, id and all.
        let body = chain.result().unwrap();
        for side in [
            BoxPart::Face(Coord::X, Side::Min),
            BoxPart::Face(Coord::X, Side::Max),
            BoxPart::Face(Coord::Y, Side::Min),
            BoxPart::Face(Coord::Y, Side::Max),
        ] {
            let face = forward(face_from(&chain.steps["plate"].provenance, Role::Box(side)));
            assert!(
                cuts.is_kept(face, &chain.model, body),
                "{variant}: {side:?} is not kept"
            );
        }
    }
}

/// One recipe under three parameter sets is one chain: the composed
/// record, entity ids and all, is the same in every variant.
#[test]
fn the_chain_is_the_same_in_every_variant() {
    let chains: Vec<Chain> = VARIANTS.iter().map(|v| chain_of(v)).collect();
    let first = whole(&chains[0]);
    let first_cuts = composed(&chains[0], &CUTS);
    for (variant, chain) in VARIANTS.iter().zip(&chains).skip(1) {
        assert_eq!(whole(chain), first, "{variant} chains differently");
        assert_eq!(
            composed(chain, &CUTS),
            first_cuts,
            "{variant}'s cuts chain differently"
        );
    }
}

/// [`Provenance::then`] is associative over the chain, so a consumer may
/// compose the seventeen records however it brackets them and get the
/// same answer: left fold, right fold, and the primitives composed apart
/// from the cuts all agree.
#[test]
fn the_chain_is_associative_whatever_the_bracketing() {
    for variant in VARIANTS {
        let chain = chain_of(variant);
        let names: Vec<&str> = PRIMITIVES.iter().chain(CUTS.iter()).copied().collect();
        let left = composed(&chain, &names);
        let right = names[..names.len() - 1].iter().rev().fold(
            chain.steps[*names.last().unwrap()].provenance.clone(),
            |p, name| chain.steps[*name].provenance.then(&p),
        );
        assert_eq!(right, left, "{variant}: right fold");
        let split = composed(&chain, &PRIMITIVES).then(&composed(&chain, &CUTS));
        assert_eq!(split, left, "{variant}: the bodies apart from the cuts");
    }
}

/// The chain accounts for the result: every entity of the body is an
/// output of the composed record or an untouched input kept through it,
/// and the counts are the oracle's for that variant.
#[test]
fn the_chain_accounts_for_every_entity_of_the_result() {
    let fixture = fixtures::load(&fixtures::corpus_root().join(FIXTURE)).unwrap();
    for variant in VARIANTS {
        let chain = chain_of(variant);
        let body = chain.result().unwrap();
        let closure = chain.model.closure(body).unwrap();
        let counts = fixture.expected.results[variant].counts;
        assert_eq!(closure.vertices.len(), counts.vertices, "{variant}");
        assert_eq!(closure.edges.len(), counts.edges, "{variant}");
        assert_eq!(closure.faces.len(), counts.faces, "{variant}");
        let all = whole(&chain);
        let outputs: BTreeSet<EntityId> = all.outputs().iter().map(|s| s.id).collect();
        let cuts = composed(&chain, &CUTS);
        let entities = closure
            .vertices
            .iter()
            .map(|v| EntityId::Vertex(*v))
            .chain(closure.edges.iter().map(|e| EntityId::Edge(*e)))
            .chain(closure.faces.iter().map(|f| EntityId::Face(*f)));
        for id in entities {
            let shape = Shape::new(id, Orientation::Forward);
            assert!(
                outputs.contains(&id) || cuts.is_kept(shape, &chain.model, body),
                "{variant}: {shape} is neither an output of the chain nor kept"
            );
        }
    }
}
