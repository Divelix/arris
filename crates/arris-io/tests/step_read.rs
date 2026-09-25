//! The STEP reader on Arris's own files (ADR-0025, plans/step-reader
//! step 10): the STEP of every corpus fixture reads back to one solid
//! with the fixture's counts, and a volume, area and centroid within its
//! tolerances, its provenance naming the file entity of every entity.

use std::collections::BTreeMap;

use arris_debug::corpus::{self, Chain, Made};
use arris_debug::fixtures::{self, DUMPED_AREAS, Fixture};
use arris_io::arris_check::arris_topo::builder::{Assembly, Builder, FaceSpec};
use arris_io::arris_check::arris_topo::entity::BodyKind;
use arris_io::arris_check::arris_topo::entity::EdgeGeometry;
use arris_io::arris_check::arris_topo::provenance::{Origin, Relation, Role};
use arris_io::arris_check::arris_topo::{
    Body, Edge, Face, FileEntity, Model, Shape, Shell, Vertex,
};
use arris_io::arris_check::{Level, check};
use arris_io::step::{self, ReadBody, ReadOptions};

/// The fixtures whose result has a degenerate edge — a sphere's pole, a
/// cone's apex — which the writer leaves out and the reader does not
/// rebuild until step 11: each loop through one jumps in (u, v). The test
/// asserts this list is exactly the corpus's bodies with a degenerate
/// edge, so it can only shrink.
const DEGENERATE_EDGES: &[&str] = &[
    "blend/box-all-edges-fillet",
    "blend/box-corner-three-fillets",
    "boolean/ball-ball-common",
    "boolean/ball-corner-cut",
    "boolean/ball-offset-drill-cut",
    "boolean/ball-on-apex-common",
    "boolean/ball-polar-drill-cut",
    "boolean/ball-pole-drill-cut",
    "boolean/ball-pole-slice-cut",
    "boolean/cone-apex-slice-cut",
    "boolean/filleted-corner-notch-cut",
    "boolean/grazing-ball-bar-cut",
    "boolean/pole-slice-beside-seam-cut",
];

/// The fixtures whose result carries a tolerance an operation raised past
/// the model's default — a vertex merged from points a tolerance apart —
/// which the reader does not measure until step 12: until then every
/// entity it reads has the default, and the checker refuses the gap.
/// Each is asserted to be refused, so the list is lifted when step 12
/// lands.
const RAISED_TOLERANCES: &[&str] = &["boolean/seam-a-tolerance-from-crossing-fuse"];

/// `true` when the body has a degenerate edge.
fn has_degenerate_edge(m: &Model, body: Body) -> bool {
    let closure = m.closure(body).unwrap();
    closure.edges.iter().any(|&e| {
        matches!(
            m.edge(e).unwrap().geometry(),
            EdgeGeometry::Degenerate { .. }
        )
    })
}

/// Writes the fixture's result under `variant`, reads it back into a
/// model of the fixture's precision, and holds it to the fixture's
/// checker, counts and measure stages. `Ok(None)` for a variant with no
/// solid to write, `Ok(Some(true))` when the result has a degenerate
/// edge (and was not read).
fn read_back(fixture: &Fixture, variant: &str) -> Result<Option<bool>, String> {
    let Some(expected) = fixture.expected.results.get(variant) else {
        return Err("no expected result".into());
    };
    if expected.degenerate || fixture.recipe.analytic.expect_error.is_some() {
        return Ok(None);
    }
    let chain = corpus::chain(&fixture.dir, variant).map_err(|e| e.to_string())?;
    let body = chain.result().ok_or("no result")?;
    if has_degenerate_edge(&chain.model, body) {
        return Ok(Some(true));
    }
    let text = step::write(&chain.model, &[body]).map_err(|e| e.to_string())?;
    let mut model = Model::new(chain.model.precision()).map_err(|e| e.to_string())?;
    let read = step::read(&mut model, &text, &ReadOptions::default()).map_err(|e| e.to_string())?;
    // The writer writes a solid per lump (ADR-0006), and the oracle
    // counts them so.
    let lumps = arris_io::arris_check::lumps(&chain.model, body)
        .map_err(|e| e.to_string())?
        .len();
    if read.solids.len() != lumps {
        return Err(format!(
            "{} solids read of {lumps} lumps",
            read.solids.len()
        ));
    }
    let mut bodies = Vec::new();
    for solid in &read.solids {
        let back = solid.result.as_ref().map_err(|r| format!("refused: {r}"))?;
        provenance_names_the_file(&model, back, solid.entity)?;
        bodies.push(back.clone());
    }
    // The lumps as the one body the fixture's result is, each read body's
    // faces kept in a shell of it.
    let body = if bodies.len() == 1 {
        bodies[0].body
    } else {
        let mut shells = Vec::new();
        for b in &bodies {
            for shell in model.shells(b.body).map_err(|e| e.to_string())? {
                let faces = model.shell(shell.id).map_err(|e| e.to_string())?.faces();
                shells.push(faces.iter().copied().map(FaceSpec::Keep).collect());
            }
        }
        let assembly = Assembly {
            shells,
            ..Assembly::default()
        };
        let tolerance = model.precision().default_tolerance;
        let (builder, _) =
            Builder::assemble(&model, tolerance, assembly).map_err(|e| e.to_string())?;
        builder
            .finish(&mut model, BodyKind::Solid)
            .map_err(|e| e.to_string())?
            .body
    };

    let read_chain = Chain {
        model,
        steps: BTreeMap::from([(
            "read".to_string(),
            Made {
                body,
                provenance: bodies[0].provenance.clone(),
                inputs: Vec::new(),
            },
        )]),
        profiles: BTreeMap::new(),
        result: "read".into(),
        params: chain.params.clone(),
    };
    let fast = check(&read_chain.model, body, Level::Fast);
    if !fast.is_ok() {
        return Err(format!("the checker at Fast:\n{fast}"));
    }
    let (_, report) = corpus::check_stage(fixture, &read_chain).map_err(|e| e.to_string())?;
    corpus::counts_stage(fixture, &read_chain, &report, expected).map_err(|e| e.to_string())?;
    corpus::measure_stage(fixture, &read_chain, expected).map_err(|e| e.to_string())?;
    Ok(Some(false))
}

/// Every entity of the read body is generated from a file entity of the
/// solid's placement, and the body from the solid itself.
fn provenance_names_the_file(
    model: &Model,
    back: &ReadBody,
    solid: FileEntity,
) -> Result<(), String> {
    let closure = model.closure(back.body).map_err(|e| e.to_string())?;
    let shapes: Vec<Shape> = closure
        .vertices
        .iter()
        .map(|&v| Shape::from(Vertex::forward(v)))
        .chain(closure.edges.iter().map(|&e| Edge::forward(e).into()))
        .chain(closure.faces.iter().map(|&f| Face::forward(f).into()))
        .chain(closure.shells.iter().map(|&s| Shell::forward(s).into()))
        .collect();
    for s in shapes {
        match back.provenance.origins(s)[..] {
            [(Relation::Generated, Origin::Role(Role::File(FileEntity { instance: 0, .. })))] => {}
            ref other => return Err(format!("{s} comes from {other:?}")),
        }
    }
    let body_origin = back.provenance.origins(back.body);
    if body_origin != [(Relation::Generated, Origin::Role(Role::File(solid)))] {
        return Err(format!("the body comes from {body_origin:?}"));
    }

    Ok(())
}

/// Every variant of every fixture in `area` read back; failures listed
/// together.
fn area(area: &str) {
    let mut failures = Vec::new();
    let mut degenerate = Vec::new();
    let mut read = 0;
    let mut raised = 0;
    let root = fixtures::corpus_root().join(area);
    for dir in fixtures::corpus() {
        if !dir.starts_with(&root) {
            continue;
        }
        let fixture = fixtures::load(&dir).unwrap();
        for variant in fixture.recipe.variant_names() {
            match read_back(&fixture, &variant) {
                Ok(None) => {}
                Ok(Some(true)) => {
                    if !degenerate.contains(&fixture.name) {
                        degenerate.push(fixture.name.clone());
                    }
                }
                Err(e) if RAISED_TOLERANCES.contains(&fixture.name.as_str()) => {
                    assert!(e.contains("fails the checker"), "{}: {e}", fixture.name);
                    raised += 1;
                }
                Ok(Some(false)) if RAISED_TOLERANCES.contains(&fixture.name.as_str()) => {
                    panic!(
                        "{} [{variant}] reads back: lift it from RAISED_TOLERANCES",
                        fixture.name
                    )
                }
                Ok(Some(false)) => read += 1,
                Err(e) => failures.push(format!("{} [{variant}]: {e}", fixture.name)),
            }
        }
    }
    let listed: Vec<String> = DEGENERATE_EDGES
        .iter()
        .filter(|n| n.starts_with(&format!("{area}/")))
        .map(|n| n.to_string())
        .collect();
    assert_eq!(
        degenerate, listed,
        "the fixtures with a degenerate edge are the ones listed"
    );
    assert!(read > 0 || !listed.is_empty(), "nothing read in {area}");
    eprintln!(
        "{area}: {read} read back, {} with a degenerate edge, {raised} with a raised tolerance",
        degenerate.len()
    );
    assert!(
        failures.is_empty(),
        "{} of {} read back wrong:\n{}",
        failures.len(),
        failures.len() + read,
        failures.join("\n")
    );
}

#[test]
fn every_area_is_known() {
    for a in DUMPED_AREAS {
        assert!(
            [
                "primitive",
                "transform",
                "boolean",
                "sweep",
                "provenance",
                "blend"
            ]
            .contains(&a),
            "{a} has no read-back test"
        );
    }
}

#[test]
fn primitive_fixtures_read_back() {
    area("primitive");
}

#[test]
fn transform_fixtures_read_back() {
    area("transform");
}

#[test]
fn boolean_fixtures_read_back() {
    area("boolean");
}

#[test]
fn sweep_fixtures_read_back() {
    area("sweep");
}

#[test]
fn provenance_fixtures_read_back() {
    area("provenance");
}

#[test]
fn blend_fixtures_read_back() {
    area("blend");
}

/// A cylinder's STEP, as Arris writes it.
fn cylinder_step() -> String {
    let mut m = Model::default();
    let body = arris_debug::sample::cylinder(&mut m, 4.0, 12.0).unwrap();
    step::write(&m, &[body]).unwrap()
}

/// Two reads of one file are the same entities with the same ids.
#[test]
fn a_read_is_deterministic() {
    let text = cylinder_step();
    let dumps: Vec<String> = (0..2)
        .map(|_| {
            let mut m = Model::default();
            let read = step::read(&mut m, &text, &ReadOptions::default()).unwrap();
            let body = read.solids[0].result.as_ref().unwrap().body;
            arris_debug::dump_text(&m, body).unwrap()
        })
        .collect();
    assert_eq!(dumps[0], dumps[1]);
}

/// A bound whose edges do not close is the solid's topology refused,
/// named by the solid, and leaves nothing in the model.
#[test]
fn a_loop_that_does_not_close_is_refused() {
    let text = cylinder_step();
    // Turn the first oriented edge of the file.
    let at = text.find("ORIENTED_EDGE('',*,*,").unwrap();
    let flag = at + text[at..].find(".T.").unwrap();
    let broken = format!("{}.F.{}", &text[..flag], &text[flag + 3..]);
    let mut m = Model::default();
    let read = step::read(&mut m, &broken, &ReadOptions::default()).unwrap();
    let refusal = read.solids[0].result.as_ref().unwrap_err();
    assert_eq!(refusal.kind(), step::RefusalKind::Topology, "{refusal}");
    assert_eq!(refusal.entity(), read.solids[0].entity.id);
    // Nothing was left behind: the next read gets the ids a fresh model
    // gives.
    let again = step::read(&mut m, &text, &ReadOptions::default()).unwrap();
    let mut fresh = Model::default();
    let first = step::read(&mut fresh, &text, &ReadOptions::default()).unwrap();
    assert_eq!(
        again.solids[0].result.as_ref().unwrap().body,
        first.solids[0].result.as_ref().unwrap().body
    );
}

/// A faceted B-rep stands where a solid would: counted and refused by
/// name, beside the solid the file also holds.
#[test]
fn a_faceted_brep_is_refused_beside_the_solid() {
    let text = cylinder_step().replace(
        "ENDSEC;\nEND-ISO-10303-21;",
        "#90000 = FACETED_BREP('',#90001);\n#90001 = CLOSED_SHELL('',());\nENDSEC;\nEND-ISO-10303-21;",
    );
    let mut m = Model::default();
    let read = step::read(&mut m, &text, &ReadOptions::default()).unwrap();
    assert_eq!(read.solids.len(), 2);
    assert!(read.solids[0].result.is_ok());
    let refusal = read.solids[1].result.as_ref().unwrap_err();
    assert_eq!(refusal.kind(), step::RefusalKind::Unsupported);
    assert_eq!(refusal.entity(), 90000);
    assert_eq!(
        read.solids[0].uncertainty,
        Some(m.precision().default_tolerance)
    );
}

/// A file that does not parse fails whole.
#[test]
fn a_parse_error_fails_the_file() {
    let mut m = Model::default();
    let err = step::read(&mut m, "ISO-10303-21;\nHEADER;", &ReadOptions::default()).unwrap_err();
    assert!(matches!(err, step::ReadError::Parse(_)), "{err}");
}
