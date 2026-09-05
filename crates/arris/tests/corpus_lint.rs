//! The corpus lint (`docs/03-roadmap.md` §Fixtures): every fixture directory
//! has a recipe and the oracle's answer, the answer is not stale, the Euler
//! line is zero, and the closed forms agree with the oracle.

use std::path::Path;

use arris_debug::fixtures::{corpus, lint, load};

#[test]
fn every_fixture_directory_is_clean() {
    let dirs = corpus();
    assert!(
        dirs.len() >= 15,
        "expected the C1 corpus, found {} directories",
        dirs.len()
    );
    let problems: Vec<String> = dirs.iter().flat_map(|d| lint(d)).collect();
    assert!(problems.is_empty(), "corpus lint:\n{}", problems.join("\n"));
}

#[test]
fn every_row_of_the_roadmap_table_has_a_fixture() {
    let names: Vec<String> = corpus().iter().map(|d| load(d).unwrap().name).collect();
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
        "sweep/extrude-plate-with-hole",
        "sweep/revolve-tube",
        "sweep/revolve-quarter",
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
