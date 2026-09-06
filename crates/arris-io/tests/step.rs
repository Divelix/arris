//! The STEP writer against the Open CASCADE oracle (`docs/plans/m2-topology.md`
//! step 4): the sample bodies, written by Arris and read by the oracle,
//! match the `primitive/*` fixtures' numbers; the file is deterministic;
//! what the writer cannot hold is a typed error.

use std::path::{Path, PathBuf};
use std::process::Command;

use arris_debug::sample;
use arris_io::arris_check::arris_topo;
use arris_io::step::{self, StepError, Unsupported};
use arris_topo::arris_math::Point3;
use arris_topo::entity::{Body as BodyEntity, BodyKind};
use arris_topo::{Body, Model};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root exists")
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("arris-io-step-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes `body` to a file and runs `compare.py` on it against `fixture`.
/// A missing oracle environment is a loud failure with the command that
/// creates it, never a skip (`tools/oracle/README.md`).
fn compare(m: &Model, body: Body, fixture: &str, tag: &str) {
    let text = step::write(m, &[body]).unwrap();
    let file = scratch(tag).join(format!("{tag}.step"));
    std::fs::write(&file, &text).unwrap();
    let root = repo_root();
    let output = Command::new("uv")
        .current_dir(&root)
        .args([
            "run",
            "--project",
            "tools/oracle",
            "tools/oracle/compare.py",
        ])
        .arg(root.join("tests/fixtures").join(fixture))
        .arg(&file)
        .output()
        .unwrap_or_else(|e| {
            panic!("could not run `uv` ({e}); install uv and run `uv sync --project tools/oracle`")
        });
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "compare.py {fixture} on {}:\n{stdout}\n{stderr}",
        file.display()
    );
    assert!(stdout.contains("MATCH"), "{stdout}");
}

#[test]
fn the_sample_box_matches_the_primitive_box_fixture() {
    let mut m = Model::default();
    let body = sample::cuboid(&mut m, Point3::origin(), Point3::new(40.0, 30.0, 10.0)).unwrap();
    compare(&m, body, "primitive/box", "box");
}

#[test]
fn the_sample_cylinder_matches_the_primitive_cylinder_fixture() {
    let mut m = Model::default();
    let body = sample::cylinder(&mut m, 4.0, 12.0).unwrap();
    compare(&m, body, "primitive/cylinder", "cylinder");
}

/// The B-spline arms: the box with one NURBS edge and one NURBS face reads
/// back with the same numbers as the analytic one.
#[test]
fn the_nurbs_box_matches_the_primitive_box_fixture() {
    let mut m = Model::default();
    let body =
        sample::cuboid_nurbs(&mut m, Point3::origin(), Point3::new(40.0, 30.0, 10.0)).unwrap();
    let text = step::write(&m, &[body]).unwrap();
    assert_eq!(text.matches("B_SPLINE_CURVE_WITH_KNOTS(").count(), 1);
    assert_eq!(text.matches("B_SPLINE_SURFACE_WITH_KNOTS(").count(), 1);
    compare(&m, body, "primitive/box", "nurbs-box");
}

#[test]
fn two_writes_and_two_builds_are_byte_identical() {
    let build = || {
        let mut m = Model::default();
        let body = sample::cylinder(&mut m, 4.0, 12.0).unwrap();
        (m, body)
    };
    let (a, body_a) = build();
    let (b, body_b) = build();
    let first = step::write(&a, &[body_a]).unwrap();
    assert_eq!(first, step::write(&a, &[body_a]).unwrap());
    assert_eq!(first, step::write(&b, &[body_b]).unwrap());
    assert!(!first.contains("NaN") && !first.contains("inf"));
}

#[test]
fn the_seam_is_one_edge_with_two_pcurves_and_the_wall_one_face() {
    let mut m = Model::default();
    let body = sample::cylinder(&mut m, 4.0, 12.0).unwrap();
    let text = step::write(&m, &[body]).unwrap();
    assert_eq!(text.matches("EDGE_CURVE(").count(), 3);
    assert_eq!(text.matches("SEAM_CURVE(").count(), 1);
    assert_eq!(text.matches("SURFACE_CURVE(").count(), 2);
    assert_eq!(
        text.matches("ORIENTED_EDGE(").count(),
        6,
        "the seam twice in the wall's loop"
    );
    assert_eq!(text.matches("CYLINDRICAL_SURFACE(").count(), 1);
    assert_eq!(text.matches("FACE_OUTER_BOUND(").count(), 3);
    assert_eq!(text.matches("VERTEX_POINT(").count(), 2);
    assert_eq!(
        text.matches("ADVANCED_FACE('',(#").count(),
        3,
        "every face has a bound list"
    );
    assert!(text.contains(",.F.);\n"), "the bottom cap is used reversed");
}

#[test]
fn several_bodies_share_one_shape_representation() {
    let mut m = Model::default();
    let a = sample::cuboid(&mut m, Point3::origin(), Point3::new(1.0, 1.0, 1.0)).unwrap();
    let b = sample::cylinder(&mut m, 4.0, 12.0).unwrap();
    let text = step::write(&m, &[a, b]).unwrap();
    assert_eq!(text.matches("MANIFOLD_SOLID_BREP(").count(), 2);
    assert_eq!(
        text.matches("ADVANCED_BREP_SHAPE_REPRESENTATION(").count(),
        1
    );
    assert_eq!(text.matches("PRODUCT(").count(), 1);
}

#[test]
fn a_wire_body_and_an_empty_list_are_typed_errors() {
    let mut m = Model::default();
    let wire = m.raw().add_body(BodyEntity::new(
        BodyKind::Wire,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    ));
    let wire = Body::forward(wire);
    assert_eq!(
        step::write(&m, &[wire]),
        Err(StepError::Unsupported {
            body: wire,
            what: Unsupported::Kind(BodyKind::Wire)
        })
    );
    assert_eq!(step::write(&m, &[]), Err(StepError::NoBodies));
    let missing = Body::forward(arris_topo::BodyId::new(7, 0));
    assert!(matches!(
        step::write(&m, &[missing]),
        Err(StepError::NotFound(_))
    ));
}
