//! The corpus lint (`docs/ROADMAP.md` §Fixtures): every fixture directory
//! has a recipe and the oracle's answer, the answer is not stale, the Euler
//! line is zero, and the closed forms agree with the oracle.

use std::path::Path;

use arris_debug::fixtures::{Kind, corpus, kind_of, lint, load};

#[test]
fn every_fixture_directory_is_clean() {
    let dirs = corpus();
    assert!(
        dirs.len() >= 18,
        "expected the C1 corpus and the two geometry fixtures, found {} directories",
        dirs.len()
    );
    let problems: Vec<String> = dirs.iter().flat_map(|d| lint(d)).collect();
    assert!(problems.is_empty(), "corpus lint:\n{}", problems.join("\n"));
}

#[test]
fn every_row_of_the_roadmap_table_has_a_fixture() {
    let names: Vec<String> = corpus()
        .iter()
        .filter(|d| kind_of(d).unwrap() == Kind::Solid)
        .map(|d| load(d).unwrap().name)
        .collect();
    for expected in [
        "primitive/box",
        "primitive/cylinder",
        "boolean/through-hole",
        "boolean/blind-hole",
        "boolean/bolt-pattern-8",
        "boolean/flush-union",
        "boolean/corner-union",
        "boolean/corner-common",
        "boolean/corner-cut",
        "boolean/flush-common",
        "boolean/disjoint-cut",
        "boolean/frame-cut",
        "boolean/boss-flush",
        "boolean/coaxial-fuse",
        "sweep/extrude-plate-with-hole",
        "sweep/extrude-slot",
        "sweep/extrude-downward",
        "sweep/revolve-tube",
        "sweep/revolve-quarter",
        "sweep/revolve-l-profile",
        "provenance/bolt-pattern-rebuild",
    ] {
        assert!(
            names.iter().any(|n| n == expected),
            "missing fixture {expected}; have {names:?}"
        );
    }
}

/// A scratch copy of a fixture with one deliberately wrong closed form
/// fails the lint, and a stale recipe hash does too.
#[test]
fn a_wrong_analytic_value_fails_the_lint() {
    let scratch = tempdir("wrong-analytic");
    copy_fixture("primitive/box", &scratch);
    let path = scratch.join("fixture.json");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"volume\": 12000"));
    std::fs::write(
        &path,
        text.replace("\"volume\": 12000", "\"volume\": 12001"),
    )
    .unwrap();
    let problems = lint(&scratch);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("analytic volume 12001 vs oracle 12000"),
        "{problems:?}"
    );

    let scratch = tempdir("stale-hash");
    copy_fixture("primitive/box", &scratch);
    let path = scratch.join("fixture.json");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace("\"max\": [40, 30, 10]", "\"max\": [40, 30, 11]"),
    )
    .unwrap();
    let problems = lint(&scratch);
    assert!(problems.iter().any(|p| p.contains("stale")), "{problems:?}");

    let scratch = tempdir("wrong-genus");
    copy_fixture("boolean/through-hole", &scratch);
    let path = scratch.join("fixture.json");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("\"genus\": 1", "\"genus\": 0")).unwrap();
    let problems = lint(&scratch);
    assert!(
        problems.iter().any(|p| p.contains("Euler line is -2")),
        "{problems:?}"
    );

    assert!(lint(Path::new("/nonexistent/fixture")).len() == 1);
}

/// The geometry kind is linted for presence, hash and shape: a stale
/// recipe and a result count that does not match the recipe both fail.
#[test]
fn a_geometry_fixture_is_linted_for_presence_hash_and_shape() {
    let geometry: Vec<String> = corpus()
        .iter()
        .filter(|d| kind_of(d).unwrap() == Kind::Geometry)
        .map(|d| arris_debug::fixtures::name_of(d))
        .collect();
    for expected in ["geom/analytic-eval", "geom/c1-intersections"] {
        assert!(geometry.iter().any(|n| n == expected), "missing {expected}");
    }

    let scratch = tempdir("geom-stale");
    copy_fixture("geom/c1-intersections", &scratch);
    let path = scratch.join("fixture.json");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"radius\": 2.0"));
    std::fs::write(
        &path,
        text.replacen("\"radius\": 2.0", "\"radius\": 2.5", 1),
    )
    .unwrap();
    let problems = lint(&scratch);
    assert!(problems.iter().any(|p| p.contains("stale")), "{problems:?}");

    let scratch = tempdir("geom-shape");
    copy_fixture("geom/c1-intersections", &scratch);
    let path = scratch.join("expected.json");
    let mut expected: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    expected["pairs"].as_array_mut().unwrap().pop();
    std::fs::write(&path, expected.to_string()).unwrap();
    let problems = lint(&scratch);
    assert!(
        problems.iter().any(|p| p.contains("pairs in the recipe")),
        "{problems:?}"
    );
}

fn tempdir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("arris-corpus-lint-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn copy_fixture(name: &str, to: &Path) {
    let from = arris_debug::fixtures::corpus_root().join(name);
    for file in ["fixture.json", "expected.json"] {
        std::fs::copy(from.join(file), to.join(file)).unwrap();
    }
}
