//! The `part` fixture kind (ADR-0026): a STEP file Arris did not write,
//! beside `fixture.json`, `expected.json` and a dump per solid read. The
//! format is `tests/fixtures/README.md` §Part fixtures; this module is its
//! Rust reading, its runner ([`run`]) and its lint ([`lint`]).
//!
//! `fixture.json` names the file, where it came from, its licence and
//! its SHA-256, and records every solid instance Arris's reader returns,
//! in the reader's order, with its expected outcome: `read`, or the
//! refusal it is and why that refusal is right. `expected.json` is the
//! oracle's reading of the same file, healed as Open CASCADE heals by
//! default: one entry per solid it reads, with the `#id` behind it, and
//! `occt_heals` where it had to heal it (ADR-0026 §3).
//!
//! A `read` solid is held to the checker at `Full` (what it cannot decide
//! on a NURBS face left unchecked), to one of the oracle's solids of its
//! `#id` — the nearest by centroid, each taken once — in counts and genus
//! where healing changed no topology, in volume, area, centroid and
//! inertia within the fixture's tolerances widened to the body's own, to
//! a closed mesh, and to its committed dump. A `refused` one is held to
//! exactly its refusal kind. A solid of the oracle's that no read matches
//! must be one Arris refuses; an Arris read that no oracle solid matches
//! is a solid the file does not have.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use arris_io::arris_check::arris_topo::arris_math::Point3;
use arris_io::arris_check::arris_topo::{Body, Model};
use arris_io::step::{self, ReadOptions};
use arris_ops::measure::mass_properties;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::corpus::{
    self, CorpusError, Target, blessing, check_leaving_nurbs, compare_mass, mesh_check,
    within_own_tolerance,
};
use crate::dump::dump_text;
use crate::fixtures::{
    Counts, FixtureError, Measured, PrecisionSpec, REGRESSION_AREA, ReadRefused, Tolerances,
    name_of, recipe_hash,
};

/// What a part fixture expects of one solid instance of its file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    /// The reader returns a body, held to the oracle's reading.
    Read,
    /// The reader refuses it, with this kind (as `RefusalKind` prints it),
    /// for the reason given: why the refusal is right, not which it is
    /// (ADR-0026 §4).
    Refused(ReadRefused),
}

/// One solid instance of a part's file, as the reader numbers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartSolid {
    /// The `#id` of its `MANIFOLD_SOLID_BREP` or `BREP_WITH_VOIDS`, or
    /// of the entity that stands where one would.
    pub id: u64,
    /// Which placement of it (`FileEntity::instance`).
    pub instance: u32,
    /// What the reader is expected to make of it.
    pub outcome: Outcome,
}

/// A part fixture's `fixture.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Part {
    /// What the part is.
    #[serde(default)]
    pub description: String,
    /// The STEP file, beside `fixture.json`.
    pub file: String,
    /// Where the file came from: its URL and archive.
    pub source: String,
    /// Its licence, quoted from the source.
    pub licence: String,
    /// The file's SHA-256, lower-case hex.
    pub sha256: String,
    /// Every solid instance the reader returns, in its order.
    pub solids: Vec<PartSolid>,
    /// The precision the model is read into.
    #[serde(default)]
    pub precision: PrecisionSpec,
    /// Comparison tolerances, before each read body's own widens them.
    #[serde(default)]
    pub tolerances: Tolerances,
}

/// One solid of the oracle's reading of a part's file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OracleSolid {
    /// The `#id` of the solid entity it came from.
    pub id: u64,
    /// Open CASCADE had to heal it to make it a solid: unhealed, a solid
    /// of this `#id` fails `BRepCheck_Analyzer`, has other counts, or is
    /// not read at all.
    pub occt_heals: bool,
    /// The counts of the unhealed reading, where it has one: where they
    /// are [`Measured::counts`], healing changed no topology.
    pub unhealed_counts: Option<Counts>,
    /// What `measure` records of the healed solid.
    #[serde(flatten)]
    pub measured: Measured,
}

impl OracleSolid {
    /// The counts Arris's reading is held to, where healing changed no
    /// topology.
    pub fn held_counts(&self) -> Option<Counts> {
        (!self.occt_heals || self.unhealed_counts == Some(self.measured.counts))
            .then_some(self.measured.counts)
    }
}

/// A part fixture's `expected.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartExpected {
    /// The `cadquery-ocp` version that wrote it.
    pub occt: String,
    /// The hash of the fixture's [`crate::fixtures::PART_KEYS`].
    pub recipe_sha256: String,
    /// Every solid the oracle reads, in its transfer's order.
    pub solids: Vec<OracleSolid>,
}

/// A loaded part fixture.
#[derive(Debug, Clone, PartialEq)]
pub struct PartFixture {
    /// The directory.
    pub dir: PathBuf,
    /// `<area>/<slug>`.
    pub name: String,
    /// `fixture.json`.
    pub part: Part,
    /// The hash of `fixture.json` as loaded.
    pub recipe_sha256: String,
    /// `expected.json`.
    pub expected: PartExpected,
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, FixtureError> {
    let text = std::fs::read_to_string(path).map_err(|source| FixtureError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| FixtureError::Json {
        path: path.to_path_buf(),
        source,
    })
}

/// Loads a part fixture: both files, and the hash of `fixture.json`.
pub fn load(dir: &Path) -> Result<PartFixture, FixtureError> {
    let path = dir.join("fixture.json");
    let raw: serde_json::Value = read_json(&path)?;
    let part: Part = serde_json::from_value(raw.clone()).map_err(|source| FixtureError::Json {
        path: path.clone(),
        source,
    })?;
    let recipe_sha256 =
        recipe_hash(&raw).map_err(|kind| FixtureError::UnknownKind { path, kind })?;
    Ok(PartFixture {
        dir: dir.to_path_buf(),
        name: name_of(dir),
        part,
        recipe_sha256,
        expected: read_json(&dir.join("expected.json"))?,
    })
}

/// The dump of one solid read: `dump.<id>.<instance>.txt`.
pub fn dump_path(dir: &Path, id: u64, instance: u32) -> PathBuf {
    dir.join(format!("dump.{id}.{instance}.txt"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Runs a part fixture: the file read, every solid held to its outcome,
/// and every solid of the oracle's accounted for (module docs). Errors:
/// [`CorpusError::StepFile`] for a file that is missing, is not the one
/// hashed or does not parse; [`CorpusError::Part`] for a solid whose
/// outcome is not the one recorded, or that one side reads and the other
/// does not have; and the stage errors of a read solid.
///
/// ```no_run
/// use arris_debug::{fixtures, part};
///
/// part::run(&fixtures::corpus_root().join("real/nist-ftc-09")).unwrap();
/// ```
pub fn run(dir: &Path) -> Result<(), CorpusError> {
    let fixture = load(dir)?;
    let name = &fixture.name;
    let fail = |what: String| CorpusError::Part {
        fixture: name.clone(),
        what,
    };
    let file = |what: String| CorpusError::StepFile {
        fixture: name.clone(),
        step: fixture.part.file.clone(),
        what,
    };
    let path = dir.join(&fixture.part.file);
    let bytes = std::fs::read(&path).map_err(|e| file(e.to_string()))?;
    let digest = sha256_hex(&bytes);
    if digest != fixture.part.sha256 {
        return Err(file(format!(
            "hashes to {digest}, not the fixture's {}",
            fixture.part.sha256
        )));
    }
    let mut model = Model::new(fixture.part.precision.precision()).map_err(|source| {
        CorpusError::Precision {
            fixture: name.clone(),
            source,
        }
    })?;
    let text = String::from_utf8_lossy(&bytes);
    let read =
        step::read(&mut model, &text, &ReadOptions::default()).map_err(|e| file(e.to_string()))?;

    let found: Vec<(u64, u32)> = (read.solids.iter())
        .map(|s| (s.entity.id, s.entity.instance))
        .collect();
    let listed: Vec<(u64, u32)> = (fixture.part.solids.iter())
        .map(|s| (s.id, s.instance))
        .collect();
    if found != listed {
        return Err(fail(format!(
            "the file reads to the solid instances {found:?}, but the fixture lists {listed:?}"
        )));
    }
    let oracle = &fixture.expected.solids;
    let mut taken = vec![false; oracle.len()];
    let mut refused: BTreeMap<u64, usize> = BTreeMap::new();
    for (solid, spec) in read.solids.iter().zip(&fixture.part.solids) {
        let at = format!("#{}[{}]", spec.id, spec.instance);
        match (&spec.outcome, &solid.result) {
            (Outcome::Refused(expected), Err(refusal)) => {
                let kind = refusal.kind().to_string();
                if kind != expected.kind {
                    return Err(fail(format!(
                        "{at} is refused as {kind} ({refusal}), not as {}",
                        expected.kind
                    )));
                }
                *refused.entry(spec.id).or_default() += 1;
            }
            (Outcome::Refused(expected), Ok(_)) => {
                return Err(fail(format!(
                    "{at} reads, where the fixture records a refusal as {}: lift it",
                    expected.kind
                )));
            }
            (Outcome::Read, Err(refusal)) => {
                return Err(CorpusError::Refused {
                    fixture: name.clone(),
                    step: at,
                    refusal: Box::new(refusal.clone()),
                });
            }
            (Outcome::Read, Ok(back)) => {
                read_stages(&fixture, &model, back.body, spec, &mut taken)?;
            }
        }
    }
    // Every solid the oracle reads is one Arris reads, or one of the same
    // entity it refuses.
    let mut left: BTreeMap<u64, usize> = BTreeMap::new();
    for (solid, _) in oracle.iter().zip(&taken).filter(|(_, t)| !**t) {
        *left.entry(solid.id).or_default() += 1;
    }
    for (id, n) in left {
        let refusals = refused.get(&id).copied().unwrap_or(0);
        if n > refusals {
            return Err(fail(format!(
                "Open CASCADE reads {n} solid(s) of #{id} that Arris neither reads nor refuses"
            )));
        }
    }
    Ok(())
}

/// The stages of one solid read (module docs), matching it to the
/// oracle's solid of its entity nearest by centroid, which `taken` marks.
fn read_stages(
    fixture: &PartFixture,
    m: &Model,
    body: Body,
    spec: &PartSolid,
    taken: &mut [bool],
) -> Result<(), CorpusError> {
    let name = format!("{} #{}[{}]", fixture.name, spec.id, spec.instance);
    let fail = |what: String| CorpusError::Part {
        fixture: fixture.name.clone(),
        what: format!("#{}[{}] {what}", spec.id, spec.instance),
    };
    let report = check_leaving_nurbs(&name, m, body)?;
    let mass =
        mass_properties(m, body).map_err(|e| fail(format!("has no mass properties: {e}")))?;
    let oracle = &fixture.expected.solids;
    let mut nearest: Option<(f64, usize)> = None;
    for (k, solid) in oracle.iter().enumerate() {
        if solid.id != spec.id || taken[k] {
            continue;
        }
        let Some(c) = solid.measured.centroid else {
            continue;
        };
        let d = (Point3::new(c[0], c[1], c[2]) - mass.centroid).norm();
        if nearest.is_none_or(|(best, _)| d < best) {
            nearest = Some((d, k));
        }
    }
    let Some((_, k)) = nearest else {
        return Err(fail(format!(
            "reads to a solid (centroid {:?}) that Open CASCADE's reading of the file does not have",
            mass.centroid
        )));
    };
    taken[k] = true;
    let expected = &oracle[k];

    // Counts and genus, where healing changed no topology.
    if let Some(counts) = expected.held_counts() {
        let line = report.euler().ok_or_else(|| CorpusError::Check {
            fixture: name.clone(),
            report: Box::new(report.clone()),
        })?;
        let found = Counts {
            vertices: line.vertices,
            edges: line.edges,
            faces: line.faces,
            loops: line.loops,
            shells: line.shells,
            solids: 1,
        };
        if found != counts {
            return Err(CorpusError::Counts {
                fixture: name,
                expected: counts,
                found,
            });
        }
        if let Some(genus) = expected.measured.genus.filter(|&g| g != line.genus) {
            return Err(CorpusError::Genus {
                fixture: name,
                expected: genus,
                found: line.genus,
            });
        }
    }

    // Mass properties, to the fixture's tolerances or the body's own.
    let tolerances = within_own_tolerance(&fixture.part.tolerances, m, body).map_err(&fail)?;
    compare_mass(m, body, &expected.measured, "the oracle's", &tolerances).map_err(|what| {
        CorpusError::Measure {
            fixture: name.clone(),
            what,
        }
    })?;
    let target = Target {
        measured: expected.measured.clone(),
        by: "the oracle's",
    };
    mesh_check(&name, m, body, &tolerances, &target)?;

    // The dump.
    let path = dump_path(&fixture.dir, spec.id, spec.instance);
    let dump = dump_text(m, body).map_err(|e| CorpusError::Dump {
        fixture: name.clone(),
        path: path.clone(),
        what: e.to_string(),
    })?;
    corpus::check_dump(&name, &path, &dump, blessing())
}

/// The lint of a part fixture: both files present and parseable, the
/// file present and its SHA-256 the fixture's, `expected.json` not stale,
/// every oracle solid on the Euler line with its genus, every refusal
/// with a reason, and every `read` solid's dump committed — none under
/// [`REGRESSION_AREA`], where a fixture waits for its fix. Returns every
/// problem found, empty when clean.
pub fn lint(dir: &Path) -> Vec<String> {
    let fixture = match load(dir) {
        Ok(f) => f,
        Err(e) => return vec![format!("{}: {e}", dir.display())],
    };
    let name = &fixture.name;
    let mut problems = Vec::new();
    let mut problem = |text: String| problems.push(format!("{name}: {text}"));
    if fixture.expected.recipe_sha256 != fixture.recipe_sha256 {
        problem(format!(
            "expected.json is stale: recipe hash {} but the fixture hashes to {} — rerun tools/oracle/expected.py",
            fixture.expected.recipe_sha256, fixture.recipe_sha256
        ));
    }
    match std::fs::read(dir.join(&fixture.part.file)) {
        Ok(bytes) if sha256_hex(&bytes) != fixture.part.sha256 => problem(format!(
            "{} hashes to {}, not the fixture's {}",
            fixture.part.file,
            sha256_hex(&bytes),
            fixture.part.sha256
        )),
        Ok(_) => {}
        Err(e) => problem(format!("{}: {e}", fixture.part.file)),
    }
    for field in [&fixture.part.source, &fixture.part.licence] {
        if field.trim().is_empty() {
            problem("a part names its source and quotes its licence".into());
        }
    }
    for solid in &fixture.expected.solids {
        let m = &solid.measured;
        let (Some(chi), Some(genus)) = (m.euler_characteristic, m.genus) else {
            continue;
        };
        let line = 2 * (m.counts.shells as i64 - genus);
        if chi != line {
            problem(format!(
                "the oracle's #{} is off the Euler line: χ {chi}, 2(S − G) {line}",
                solid.id
            ));
        }
    }
    let area = name.split('/').next().unwrap_or_default();
    for solid in &fixture.part.solids {
        let dump = dump_path(dir, solid.id, solid.instance);
        match &solid.outcome {
            Outcome::Refused(r) if r.why.trim().is_empty() => problem(format!(
                "#{}[{}] is refused as {} with no reason why the refusal is right",
                solid.id, solid.instance, r.kind
            )),
            Outcome::Refused(_) => {}
            Outcome::Read if area == REGRESSION_AREA && dump.is_file() => problem(format!(
                "{} is committed under {REGRESSION_AREA}/: the part passes, so it moves into its area",
                dump.display()
            )),
            Outcome::Read if area != REGRESSION_AREA && !dump.is_file() => problem(format!(
                "#{}[{}] reads but {} is not committed (ARRIS_BLESS=1)",
                solid.id,
                solid.instance,
                dump.file_name().unwrap_or_default().to_string_lossy()
            )),
            Outcome::Read => {}
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch copy of the part fixture `name` whose `fixture.json` has
    /// `from` replaced by `to`.
    fn edited(name: &str, tag: &str, from: &str, to: &str) -> PathBuf {
        let source = crate::fixtures::corpus_root().join(name);
        let scratch = std::env::temp_dir().join(format!("arris-part-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).expect("a scratch directory");
        for entry in std::fs::read_dir(&source).expect("the fixture") {
            let path = entry.expect("an entry").path();
            std::fs::copy(&path, scratch.join(path.file_name().expect("a file"))).expect("a copy");
        }
        let recipe = scratch.join("fixture.json");
        let text = std::fs::read_to_string(&recipe).expect("the fixture");
        let changed = text.replace(from, to);
        assert_ne!(changed, text, "{from} is in {name}'s fixture.json");
        std::fs::write(&recipe, changed).expect("the edited fixture");
        scratch
    }

    /// The outcome recorded is the one held: a refusal of another kind
    /// fails, and so does a refusal recorded for a solid that reads.
    #[test]
    fn a_part_is_held_to_the_outcome_it_records() {
        let scratch = edited(
            "real/nist-ftc-09-offset",
            "kind",
            r#""kind": "offset""#,
            r#""kind": "gap past the cap""#,
        );
        let e = run(&scratch).expect_err("an offset is not a gap");
        assert!(
            matches!(&e, CorpusError::Part { what, .. } if what.contains("is refused as offset")),
            "{e}"
        );
        let _ = std::fs::remove_dir_all(&scratch);

        let scratch = edited(
            "real/nist-ftc-09",
            "lift",
            r#""outcome": "read""#,
            r#""outcome": {"refused": {"kind": "offset", "why": "none"}}"#,
        );
        let e = run(&scratch).expect_err("the part reads");
        assert!(
            matches!(&e, CorpusError::Part { what, .. } if what.contains("lift it")),
            "{e}"
        );
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
