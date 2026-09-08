//! The fixture corpus, one test per fixture and every variant of it
//! (`docs/03-roadmap.md` §Fixtures, `docs/plans/m2-topology.md` step 13):
//! the recipe built in Arris, the checker at `Full`, counts and genus
//! against the oracle, STEP read back by the oracle, provenance
//! accounting, the dump diffed against `dump.txt`. The `primitive/*`,
//! `transform/*` and `cut` fixtures are live; every fixture that needs
//! an operation of a later step or milestone is `#[ignore]`d naming it,
//! and `--include-ignored` runs it anyway, so the day the operation
//! lands the test says so.

use arris_debug::corpus::{self, CorpusError};
use arris_debug::fixtures;

fn run(name: &str) {
    let dir = fixtures::corpus_root().join(name);
    let fixture = fixtures::load(&dir).unwrap();
    for variant in fixture.recipe.variant_names() {
        if let Err(e) = corpus::run(&dir, &variant) {
            panic!("{name} [{variant}]: {e}");
        }
    }
}

#[test]
fn primitive_box() {
    run("primitive/box");
}

#[test]
fn primitive_cylinder() {
    run("primitive/cylinder");
}

#[test]
fn transform_posed_cylinder() {
    run("transform/posed-cylinder");
}

#[test]
fn boolean_through_hole() {
    run("boolean/through-hole");
}

#[test]
fn boolean_blind_hole() {
    run("boolean/blind-hole");
}

#[test]
fn boolean_bolt_pattern_8() {
    run("boolean/bolt-pattern-8");
}

#[test]
#[ignore = "M4: needs ops::fuse"]
fn boolean_flush_union() {
    run("boolean/flush-union");
}

#[test]
fn boolean_corner_union() {
    run("boolean/corner-union");
}

#[test]
fn boolean_corner_common() {
    run("boolean/corner-common");
}

#[test]
fn boolean_corner_cut() {
    run("boolean/corner-cut");
}

#[test]
#[ignore = "M4: needs ops::common, and the degenerate result"]
fn boolean_flush_common() {
    run("boolean/flush-common");
}

#[test]
fn boolean_disjoint_cut() {
    run("boolean/disjoint-cut");
}

#[test]
fn boolean_frame_cut() {
    run("boolean/frame-cut");
}

/// The target inside the tool: nothing survives, and the oracle records
/// no solid — the runner's degenerate path.
#[test]
fn boolean_swallow_cut() {
    run("boolean/swallow-cut");
}

/// A slab through the plate: Open CASCADE builds two solids, Arris
/// refuses with `Reason::MultiShell` — the runner's expected-error path.
#[test]
fn boolean_split_cut() {
    run("boolean/split-cut");
}

#[test]
fn boolean_boss() {
    run("boolean/boss");
}

/// The wall's (u, v) region is a strip between two oblique sections, and
/// the tessellator's rule that a ruled surface needs no interior points
/// does not hold there (`docs/plans/m4-booleans.md` step 8's findings,
/// `docs/BACKLOG.md`).
#[test]
#[ignore = "arris-mesh: a cylinder face bounded by an oblique section meshes with triangles that span the wall"]
fn boolean_oblique_hole() {
    run("boolean/oblique-hole");
}

#[test]
#[ignore = "M4: needs ops::cut and the tangent case (step 11); the oracle splits the touched face along the ruling"]
fn boolean_tangent_outside_cut() {
    run("boolean/tangent-outside-cut");
}

/// The same solid as `through-hole` in another pose: both operands moved
/// by one rigid motion before the cut.
#[test]
fn boolean_posed_through_hole() {
    run("boolean/posed-through-hole");
}

/// Two coaxial cylinders: the pair has no section curve, and the bore is
/// the tool's wall reversed.
#[test]
fn boolean_coaxial_cut() {
    run("boolean/coaxial-cut");
}

/// Operands that share no material: the runner's degenerate path through
/// `common`.
#[test]
fn boolean_disjoint_common() {
    run("boolean/disjoint-common");
}

/// A sliver: the top and bottom faces are a D of one straight edge and
/// one arc under an eighth of a turn, which the checker's minimum
/// discretisation once flattened to a chord (plan step 9).
#[test]
fn boolean_sliver_common() {
    run("boolean/sliver-common");
}

#[test]
#[ignore = "M5: needs ops::planar_face and ops::extrude"]
fn sweep_extrude_plate_with_hole() {
    run("sweep/extrude-plate-with-hole");
}

#[test]
#[ignore = "M5: needs ops::planar_face and ops::revolve"]
fn sweep_revolve_tube() {
    run("sweep/revolve-tube");
}

#[test]
#[ignore = "M5: needs ops::planar_face and ops::revolve"]
fn sweep_revolve_quarter() {
    run("sweep/revolve-quarter");
}

#[test]
#[ignore = "M4: the origins chain across three variants and dump.<variant>.txt (step 12)"]
fn provenance_bolt_pattern_rebuild() {
    run("provenance/bolt-pattern-rebuild");
}

/// A recipe with an op the kernel has no operation for fails naming it:
/// `extrude`, since a box, a cylinder and now `transform` all build (M5
/// is where this is retargeted again).
#[test]
fn an_unsupported_op_fails_with_its_name() {
    let dir = tempdir("unsupported-op");
    std::fs::write(
        dir.join("fixture.json"),
        r#"{
            "description": "a box referenced by an op the kernel does not have yet",
            "steps": [
                {"name": "base", "op": "box", "min": [0, 0, 0], "max": [1, 1, 1]},
                {"name": "result", "op": "extrude", "profile": "base", "direction": [0, 0, 1], "length": 1}
            ],
            "result": "result"
        }"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("expected.json"),
        r#"{
            "occt": "n/a",
            "recipe_sha256": "0",
            "results": {"default": {"degenerate": false, "counts": {"vertices": 0, "edges": 0, "faces": 0, "loops": 0}}}
        }"#,
    )
    .unwrap();
    let err = corpus::run(&dir, "default").unwrap_err();
    assert!(
        matches!(&err, CorpusError::Unsupported { step, op: "extrude", .. } if step == "result"),
        "{err}"
    );
    assert!(err.to_string().contains("op \"extrude\""), "{err}");
    let err = corpus::run(&dir, "nope").unwrap_err();
    assert!(matches!(err, CorpusError::Variant { .. }), "{err}");
}

/// A committed dump that differs by one id fails with the diff, and a
/// missing one says how to write it.
#[test]
fn a_dump_that_differs_by_one_id_fails_with_the_diff() {
    let scratch = tempdir("dump-diff");
    let from = fixtures::corpus_root().join("primitive/cylinder");
    for file in ["fixture.json", "expected.json"] {
        std::fs::copy(from.join(file), scratch.join(file)).unwrap();
    }
    let err = corpus::run(&scratch, "default").unwrap_err();
    assert!(
        matches!(&err, CorpusError::Dump { what, .. } if what.contains("not committed")),
        "{err}"
    );
    let dump = std::fs::read_to_string(from.join("dump.txt")).unwrap();
    assert!(dump.contains("coedge +e1 "), "the seam walked up");
    std::fs::write(
        scratch.join("dump.txt"),
        dump.replacen("coedge +e1 ", "coedge +e7 ", 1),
    )
    .unwrap();
    let err = corpus::run(&scratch, "default").unwrap_err();
    match &err {
        CorpusError::Dump { what, .. } => {
            let committed = what
                .lines()
                .any(|l| l.starts_with('-') && l.contains("coedge +e7"));
            let built = what
                .lines()
                .any(|l| l.starts_with('+') && l.contains("coedge +e1"));
            assert!(committed && built, "{what}");
        }
        other => panic!("{other}"),
    }
    std::fs::write(scratch.join("dump.txt"), &dump).unwrap();
    corpus::run(&scratch, "default").unwrap();
}

fn tempdir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("arris-corpus-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
