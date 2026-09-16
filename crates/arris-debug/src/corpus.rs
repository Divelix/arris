//! The corpus runner: the fixture test of `docs/ROADMAP.md` §Fixtures,
//! one call per fixture and variant. [`run`] builds the recipe in Arris,
//! runs the checker at `Full`, compares counts and genus against the
//! oracle's `expected.json` (the counts against the recipe's own where it
//! states a convention Arris does not follow, `analytic.counts_differ`),
//! writes STEP and has the oracle read it back
//! (`compare.py`), measures the result over the B-Rep and holds its
//! volume, area, centroid and inertia to the oracle's within the
//! fixture's tolerances, tessellates the result and holds the mesh
//! closed with its signed volume within the fixture's `mesh_volume_rel`
//! of the oracle's at `mesh_chord` — asking for the corner block and
//! holding every face-local vertex to ADR-0012's two invariants, so the
//! whole corpus covers them — asserts the provenance accounting of
//! every step, and diffs the text dump against the committed `dump.txt` —
//! written only under `ARRIS_BLESS=1`. Every stage that fails is a typed
//! [`CorpusError`] saying which fixture, which stage and what differed.
//! A `profile`
//! step builds a `geom::Profile` kept beside the bodies for the sweep
//! steps that name it; it makes no body and needs no accounting. A result the
//! oracle recorded no solid for (`expected.degenerate`) must fail with
//! `OpError::Degenerate`, and one the recipe marks `analytic.expect_error`
//! must fail with that typed refusal; either ends the run there, the
//! oracle's numbers recorded but not compared.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use arris_geom::Profile;
use arris_io::arris_check::arris_topo::arris_math::nalgebra::UnitQuaternion;
use arris_io::arris_check::arris_topo::arris_math::{
    Axis, FrameError, Isometry, Point3, UnitVec3, Vec3,
};
use arris_io::arris_check::arris_topo::{Body, Edge, EntityId, Model, Orientation, Provenance};
use arris_io::arris_check::classify::{Classification, classify_point};
use arris_io::arris_check::{Level, LumpError, Report, check, lumps};
use arris_io::step::{self, StepError};
use arris_mesh::{MeshRequest, TriMesh, tessellate_with};
use arris_ops::measure::mass_properties;
use arris_ops::{
    OpError, Reason, chamfer, common, cut, extrude, fillet, fuse, primitive_box,
    primitive_cylinder, revolve, transform,
};

use crate::dump::dump_text;
use crate::fixtures::geom::{self as geom_spec, build_profile};
use crate::fixtures::{
    self, Class, Counts, ExpectError, ExprError, Fixture, FixtureError, Measured, Num, Rotate,
    Step, Tolerances,
};
use crate::oracle::{self, OracleError};
use sha2::{Digest, Sha256};

/// The environment variable that makes [`run`] write `dump.txt` instead
/// of diffing against it.
pub const BLESS_VAR: &str = "ARRIS_BLESS";

/// Why a fixture did not pass, by stage.
#[derive(Debug, thiserror::Error)]
pub enum CorpusError {
    /// The fixture directory could not be loaded.
    #[error(transparent)]
    Fixture(#[from] FixtureError),
    /// The variant is not in the recipe.
    #[error("{fixture}: no variant {variant:?}")]
    Variant {
        /// The fixture.
        fixture: String,
        /// The variant asked for.
        variant: String,
    },
    /// A step refers to a step that does not exist or comes later.
    #[error("{fixture}: step {step:?} refers to {name:?}, which is not a step before it")]
    Reference {
        /// The fixture.
        fixture: String,
        /// The step's name.
        step: String,
        /// The name it refers to.
        name: String,
    },
    /// A number in the recipe could not be evaluated.
    #[error("{fixture}: step {step:?}: {source}")]
    Expression {
        /// The fixture.
        fixture: String,
        /// The step's name.
        step: String,
        /// The cause.
        source: ExprError,
    },
    /// A `profile` step's plane or numbers do not build.
    #[error("{fixture}: step {step:?}: profile: {source}")]
    Profile {
        /// The fixture.
        fixture: String,
        /// The step's name.
        step: String,
        /// The cause, boxed to keep the error small.
        source: Box<geom_spec::BuildError>,
    },
    /// A point naming an edge to blend is not on exactly one edge of the
    /// body: it classifies to a vertex, a face, the inside or the
    /// outside, or lies within `probe` of two edges.
    #[error("{fixture}: step {step:?}: the edge point {point:?} {what}")]
    EdgePoint {
        /// The fixture.
        fixture: String,
        /// The step's name.
        step: String,
        /// The point.
        point: [f64; 3],
        /// What it names instead of one edge.
        what: String,
    },
    /// A recipe axis is not one.
    #[error("{fixture}: step {step:?}: axis: {source}")]
    Axis {
        /// The fixture.
        fixture: String,
        /// The step's name.
        step: String,
        /// The cause.
        source: FrameError,
    },
    /// The result step was expected to fail with a typed error —
    /// `expected.degenerate`, or the recipe's `analytic.expect_error` —
    /// and built a body, or failed with another error.
    #[error("{fixture}: step {step:?}: expected {expected}, found {found}")]
    Expectation {
        /// The fixture.
        fixture: String,
        /// The step's name.
        step: String,
        /// The error expected.
        expected: String,
        /// What happened instead.
        found: String,
    },
    /// An operation failed.
    #[error("{fixture}: step {step:?}: {source}")]
    Op {
        /// The fixture.
        fixture: String,
        /// The step's name.
        step: String,
        /// The cause.
        source: OpError,
    },
    /// The result fails the checker at `Full`, or a `Full` row could not
    /// be decided.
    #[error("{fixture}: the result fails the checker:\n{report}")]
    Check {
        /// The fixture.
        fixture: String,
        /// The report.
        report: Box<Report>,
    },
    /// The result's lumps could not be read (`arris_check::lumps`), which
    /// for a result the checker passed at `Full` is a kernel bug.
    #[error("{fixture}: lumps: {source}")]
    Lumps {
        /// The fixture.
        fixture: String,
        /// Why.
        source: LumpError,
    },
    /// Arris classifies a probe point differently from the oracle.
    #[error(
        "{fixture}: probe {label:?} at {point:?}: Arris says {found}, the oracle says {expected:?}"
    )]
    Probe {
        /// The fixture.
        fixture: String,
        /// The probe's label.
        label: String,
        /// The point.
        point: [f64; 3],
        /// The oracle's class.
        expected: Class,
        /// What Arris said — a classification, or why it could not.
        found: String,
    },
    /// The counts differ from the oracle's (or from the recipe's own,
    /// under `analytic.counts_differ`).
    #[error("{fixture}: counts {found:?} but the oracle says {expected:?}")]
    Counts {
        /// The fixture.
        fixture: String,
        /// The oracle's counts.
        expected: Counts,
        /// Arris's.
        found: Counts,
    },
    /// The genus differs from the oracle's.
    #[error("{fixture}: genus {found} but the oracle says {expected}")]
    Genus {
        /// The fixture.
        fixture: String,
        /// The oracle's genus.
        expected: i64,
        /// Arris's.
        found: i64,
    },
    /// The result could not be written as STEP.
    #[error("{fixture}: STEP: {source}")]
    Step {
        /// The fixture.
        fixture: String,
        /// The cause.
        source: StepError,
    },
    /// The oracle did not match Arris's STEP, or could not run.
    #[error(transparent)]
    Oracle(#[from] OracleError),
    /// A mass property could not be computed, or is not the oracle's
    /// within the fixture's tolerance for it.
    #[error("{fixture}: measure: {what}")]
    Measure {
        /// The fixture.
        fixture: String,
        /// Which quantity differed, and by how much.
        what: String,
    },
    /// The mesh could not be built, is not closed, or its volume is not
    /// the oracle's within `mesh_volume_rel`.
    #[error("{fixture}: mesh at chord {chord}: {what}")]
    Mesh {
        /// The fixture.
        fixture: String,
        /// The chord tolerance the result was meshed at.
        chord: f64,
        /// What went wrong.
        what: String,
    },
    /// A step's provenance does not account for every entity.
    #[error("{fixture}: step {step:?}: provenance: {what}")]
    Provenance {
        /// The fixture.
        fixture: String,
        /// The step's name.
        step: String,
        /// What is unaccounted for.
        what: String,
    },
    /// The dump differs from the committed one, or none is committed.
    #[error("{fixture}: {path} {what}")]
    Dump {
        /// The fixture.
        fixture: String,
        /// The dump file.
        path: PathBuf,
        /// The difference, or that the file is missing.
        what: String,
    },
    /// A file could not be read or written.
    #[error("{path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The cause.
        source: std::io::Error,
    },
}

/// The dump file of a variant: `dump.txt` for `default`,
/// `dump.<variant>.txt` otherwise.
pub fn dump_path(dir: &Path, variant: &str) -> PathBuf {
    if variant == "default" {
        dir.join("dump.txt")
    } else {
        dir.join(format!("dump.{variant}.txt"))
    }
}

/// `true` when [`BLESS_VAR`] is set to anything but `0` or empty.
pub fn blessing() -> bool {
    std::env::var(BLESS_VAR).is_ok_and(|v| !(v.is_empty() || v == "0"))
}

/// How a result step is expected to fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Refusal {
    /// `OpError::Degenerate`, any reason: the oracle recorded no solid.
    Degenerate,
    /// The recipe's `analytic.expect_error`.
    Error(ExpectError),
}

impl Refusal {
    fn expected(self) -> String {
        match self {
            Refusal::Degenerate => "OpError::Degenerate".into(),
            Refusal::Error(ExpectError::TangentContact) => {
                "OpError::Degenerate with Reason::TangentContact".into()
            }
            Refusal::Error(ExpectError::NonManifold) => {
                "OpError::Degenerate with Reason::NonManifold".into()
            }
            Refusal::Error(ExpectError::BlendTooLarge) => {
                "OpError::Degenerate with Reason::BlendTooLarge".into()
            }
            Refusal::Error(ExpectError::TangentChain) => {
                "OpError::Degenerate with Reason::TangentChain".into()
            }
            Refusal::Error(ExpectError::VertexBlend) => {
                "OpError::Degenerate with Reason::VertexBlend".into()
            }
        }
    }

    /// `Ok` when `built` is the failure expected.
    fn assert(
        self,
        fixture: &str,
        step: &str,
        built: Result<(), CorpusError>,
    ) -> Result<(), CorpusError> {
        let found = match built {
            Ok(()) => "a body".to_string(),
            Err(CorpusError::Op {
                source: OpError::Degenerate { reason, .. },
                ..
            }) => {
                let matches = match self {
                    Refusal::Degenerate => true,
                    Refusal::Error(ExpectError::TangentContact) => reason == Reason::TangentContact,
                    Refusal::Error(ExpectError::NonManifold) => reason == Reason::NonManifold,
                    Refusal::Error(ExpectError::BlendTooLarge) => reason == Reason::BlendTooLarge,
                    Refusal::Error(ExpectError::TangentChain) => reason == Reason::TangentChain,
                    Refusal::Error(ExpectError::VertexBlend) => reason == Reason::VertexBlend,
                };
                if matches {
                    return Ok(());
                }
                format!("OpError::Degenerate with {reason}")
            }
            Err(e) => e.to_string(),
        };
        Err(CorpusError::Expectation {
            fixture: fixture.to_string(),
            step: step.to_string(),
            expected: self.expected(),
            found,
        })
    }
}

/// What one step produced: the body, its record and the bodies it took.
#[derive(Debug)]
pub struct Made {
    /// The body the step built.
    pub body: Body,
    /// The record the operation returned.
    pub provenance: Provenance,
    /// The bodies the step consumed, in the operation's argument order.
    pub inputs: Vec<Body>,
}

/// A recipe built whole: the model every step was built in, each body
/// step by name, the profiles the `profile` steps described, and the name
/// of the step the recipe calls its result — what a test of a chain of
/// operations reads a fixture through when it wants the steps' records,
/// which [`run`] only accounts for.
#[derive(Debug)]
pub struct Chain {
    /// The model the steps were built in.
    pub model: Model,
    /// Every body step of the recipe, by name.
    pub steps: BTreeMap<String, Made>,
    /// Every `profile` step, by name.
    pub profiles: BTreeMap<String, Profile>,
    /// The name of the result step in [`Chain::steps`].
    pub result: String,
}

impl Chain {
    /// The result step's body.
    pub fn result(&self) -> Option<Body> {
        Some(self.steps.get(&self.result)?.body)
    }
}

/// Builds every step of `dir`'s recipe under `variant` in one model and
/// returns them with their records. Errors: as [`run`]'s build stage.
/// A recipe whose result Arris refuses by design (`expected.degenerate`
/// or `analytic.expect_error`) fails here with that error; [`run`] is
/// what asserts a refusal.
///
/// ```no_run
/// use arris_debug::{corpus, fixtures};
///
/// let dir = fixtures::corpus_root().join("boolean/through-hole");
/// let chain = corpus::chain(&dir, "default").unwrap();
/// assert!(chain.result().is_some());
/// assert!(!chain.steps["result"].provenance.is_empty());
/// ```
pub fn chain(dir: &Path, variant: &str) -> Result<Chain, CorpusError> {
    let fixture = fixtures::load(dir)?;
    let Some(params) = fixture.recipe.params_of(variant) else {
        return Err(CorpusError::Variant {
            fixture: fixture.name.clone(),
            variant: variant.to_string(),
        });
    };
    let mut model = Model::default();
    let mut steps = BTreeMap::new();
    let mut profiles = BTreeMap::new();
    build_all(
        &mut model,
        &fixture,
        &params,
        None,
        &mut steps,
        &mut profiles,
    )?;
    Ok(Chain {
        model,
        steps,
        profiles,
        result: fixture.recipe.result.clone(),
    })
}

/// Builds every step of `fixture`'s recipe into `m` and `made`, a
/// `profile` step into `profiles`. Returns `false` when `refusal` is set
/// and the result step failed with the refusal expected — the recipe is
/// built no further and nothing after it is comparable; `true` when every
/// step built.
fn build_all(
    m: &mut Model,
    fixture: &Fixture,
    params: &BTreeMap<String, f64>,
    refusal: Option<Refusal>,
    made: &mut BTreeMap<String, Made>,
    profiles: &mut BTreeMap<String, Profile>,
) -> Result<bool, CorpusError> {
    for step in &fixture.recipe.steps {
        let built = build_step(m, fixture, step, params, made, profiles);
        if let Some(refusal) = refusal {
            if step.name() == fixture.recipe.result {
                refusal.assert(&fixture.name, step.name(), built.map(|_| ()))?;
                return Ok(false);
            }
        }
        if let Some(body) = built? {
            made.insert(step.name().to_string(), body);
        }
    }
    Ok(true)
}

/// The name of the STEP file [`run`] writes for the oracle:
/// `<area>-<slug>-<variant>` for a fixture in the corpus, and the same
/// with a short digest of the directory for a copy of that recipe
/// anywhere else.
///
/// Two runs of one recipe from different directories — the corpus's own
/// copy and a scratch copy in a test of this runner — otherwise name the
/// same file, and they run at the same time: under libtest as two threads
/// of one binary, and under `cargo nextest` as two processes. The
/// canonical directory keeps the plain name `docs/ARCHITECTURE.md`
/// §Formats and tools and the `inspect` skill quote, so a failure is still
/// looked at under the name the docs give it.
fn step_tag(name: &str, variant: &str, dir: &Path) -> String {
    let tag = format!("{}-{variant}", name.replace('/', "-"));
    let canonical = crate::fixtures::corpus_root().join(name);
    let same = dir == canonical
        || matches!((dir.canonicalize(), canonical.canonicalize()), (Ok(a), Ok(b)) if a == b);
    if same {
        return tag;
    }
    let digest = Sha256::digest(dir.to_string_lossy().as_bytes());
    format!("{tag}-{:02x}{:02x}{:02x}", digest[0], digest[1], digest[2])
}

/// Runs every stage on `dir`'s recipe under `variant`. Errors: the first
/// stage that fails, with what differed. Writes `target/inspect/<area>-
/// <slug>-<variant>.step` for the oracle, and the dump file under
/// [`BLESS_VAR`].
///
/// ```no_run
/// use arris_debug::{corpus, fixtures};
///
/// let dir = fixtures::corpus_root().join("primitive/box");
/// corpus::run(&dir, "default").unwrap();
/// ```
pub fn run(dir: &Path, variant: &str) -> Result<(), CorpusError> {
    let fixture = fixtures::load(dir)?;
    let name = fixture.name.clone();
    let Some(params) = fixture.recipe.params_of(variant) else {
        return Err(CorpusError::Variant {
            fixture: name,
            variant: variant.to_string(),
        });
    };
    let Some(expected) = fixture.expected.results.get(variant) else {
        return Err(CorpusError::Variant {
            fixture: name,
            variant: variant.to_string(),
        });
    };
    // A result the oracle records no solid for, or one the recipe says
    // Arris refuses by design, must fail with its typed error at the
    // result step; nothing after it is compared.
    let refusal = if expected.degenerate {
        Some(Refusal::Degenerate)
    } else {
        fixture.recipe.analytic.expect_error.map(Refusal::Error)
    };
    let mut m = Model::default();
    let mut made: BTreeMap<String, Made> = BTreeMap::new();
    let mut profiles: BTreeMap<String, Profile> = BTreeMap::new();
    if !build_all(&mut m, &fixture, &params, refusal, &mut made, &mut profiles)? {
        return Ok(());
    }
    let Some(result) = made.get(&fixture.recipe.result) else {
        return Err(CorpusError::Reference {
            fixture: name,
            step: "result".into(),
            name: fixture.recipe.result.clone(),
        });
    };
    let body = result.body;

    // The checker at Full, with nothing undecided.
    let report = check(&m, body, Level::Full);
    if !report.is_ok() || !report.unchecked().is_empty() {
        return Err(CorpusError::Check {
            fixture: name,
            report: Box::new(report),
        });
    }

    // Counts and genus; a solid per lump, as the oracle counts them.
    let line = report.euler().ok_or_else(|| CorpusError::Check {
        fixture: name.clone(),
        report: Box::new(report.clone()),
    })?;
    let solids = lumps(&m, body)
        .map_err(|source| CorpusError::Lumps {
            fixture: name.clone(),
            source,
        })?
        .len();
    let found = Counts {
        vertices: line.vertices,
        edges: line.edges,
        faces: line.faces,
        loops: line.loops,
        shells: line.shells,
        solids,
    };
    // The oracle's counts, unless the recipe states a convention Arris
    // does not follow and gives its own.
    let analytic = &fixture.recipe.analytic;
    let counts = match (&analytic.counts_differ, analytic.counts) {
        (Some(_), Some(own)) => own,
        _ => expected.counts,
    };
    if found != counts {
        return Err(CorpusError::Counts {
            fixture: name,
            expected: counts,
            found,
        });
    }
    if let Some(genus) = expected.genus {
        if line.genus != genus {
            return Err(CorpusError::Genus {
                fixture: name,
                expected: genus,
                found: line.genus,
            });
        }
    }

    // STEP through the oracle.
    let text = step::write(&m, &[body]).map_err(|source| CorpusError::Step {
        fixture: name.clone(),
        source,
    })?;
    let tag = step_tag(&name, variant, dir);
    oracle::compare_dir(dir, &text, Some(variant), &tag)?;

    let tolerances = fixture.recipe.tolerances;

    // `measure` over the B-Rep against what the oracle measured of the
    // same recipe: volume, area, centroid and the inertia tensor.
    measure_stage(&m, body, expected, &tolerances).map_err(|what| CorpusError::Measure {
        fixture: name.clone(),
        what,
    })?;

    // The mesh: closed, positive, and the oracle's volume within the
    // fixture's mesh tolerance at its chord.
    let mesh_failure = |what: String| CorpusError::Mesh {
        fixture: name.clone(),
        chord: tolerances.mesh_chord,
        what,
    };
    // The corner block on every fixture (ADR-0012), so the whole corpus
    // covers its invariants rather than one focused test.
    let request = MeshRequest::new(tolerances.mesh_chord).with_corners();
    let mesh = tessellate_with(&m, body, &request).map_err(|e| mesh_failure(e.to_string()))?;
    corners_stage(&m, body, &mesh).map_err(&mesh_failure)?;
    let Some(mesh_volume) = mesh.signed_volume() else {
        return Err(mesh_failure("the mesh is not closed".into()));
    };
    if !mesh_volume.is_finite() || mesh_volume <= 0.0 {
        return Err(mesh_failure(format!(
            "the mesh's signed volume is {mesh_volume}, not positive"
        )));
    }
    if let Some(oracle_volume) = expected.volume {
        let relative = (mesh_volume - oracle_volume).abs() / oracle_volume.abs();
        if relative.is_nan() || relative > tolerances.mesh_volume_rel {
            return Err(mesh_failure(format!(
                "mesh volume {mesh_volume} vs the oracle's {oracle_volume}: {relative:e} relative, above mesh_volume_rel {:e}",
                tolerances.mesh_volume_rel
            )));
        }
    }

    // The probes: Arris's classification of each point against the
    // oracle's, exactly. Both sides have their own tolerance for "on" —
    // the fixture's `probe` for the oracle, the entities' own for Arris
    // — and a probe is placed so that the two agree; a disagreement is a
    // finding, never something a band is widened to cover.
    for probe in &expected.probes {
        let point = Point3::new(probe.point[0], probe.point[1], probe.point[2]);
        let found = match classify_point(&m, body, point) {
            Ok(Classification::Inside) => Ok(Class::In),
            Ok(Classification::Outside) => Ok(Class::Out),
            Ok(Classification::On(_)) => Ok(Class::On),
            Err(e) => Err(e.to_string()),
        };
        let matches = match &found {
            Ok(class) => *class == probe.class,
            Err(_) => false,
        };
        if !matches {
            return Err(CorpusError::Probe {
                fixture: name,
                label: probe.label.clone(),
                point: probe.point,
                expected: probe.class,
                found: match found {
                    Ok(class) => format!("{class:?}"),
                    Err(e) => e,
                },
            });
        }
    }

    // Provenance accounting, every body step (a profile step makes none).
    for step in &fixture.recipe.steps {
        let Some(out) = made.get(step.name()) else {
            continue;
        };
        arris_topo::provenance::audit(&m, &out.inputs, out.body, &out.provenance).map_err(|e| {
            CorpusError::Provenance {
                fixture: name.clone(),
                step: step.name().to_string(),
                what: e.to_string(),
            }
        })?;
    }

    // The dump.
    let dump = dump_text(&m, body).map_err(|e| CorpusError::Dump {
        fixture: name.clone(),
        path: dump_path(dir, variant),
        what: e.to_string(),
    })?;
    let path = dump_path(dir, variant);
    if blessing() {
        std::fs::write(&path, &dump).map_err(|source| CorpusError::Io {
            path: path.clone(),
            source,
        })?;
        return Ok(());
    }
    let committed = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(CorpusError::Dump {
                fixture: name,
                path,
                what: format!("is not committed yet; run with {BLESS_VAR}=1 to write it"),
            });
        }
        Err(source) => return Err(CorpusError::Io { path, source }),
    };
    if committed != dump {
        return Err(CorpusError::Dump {
            fixture: name,
            path,
            what: format!(
                "differs from the dump of this build:\n{}",
                diff(&committed, &dump)
            ),
        });
    }
    Ok(())
}

/// A fixture's recipe built up to, but not including, its result step:
/// what a test of the boolean decomposition reads a `boolean/*` fixture
/// through before the boolean exists to run.
#[derive(Debug)]
pub struct Inputs {
    /// The model the steps were built in.
    pub model: Model,
    /// Every body step before the result, by name.
    pub bodies: BTreeMap<String, Body>,
    /// Every `profile` step before the result, by name.
    pub profiles: BTreeMap<String, Profile>,
    /// The result step, unbuilt.
    pub result: Step,
}

impl Inputs {
    /// The two bodies the result step combines — `a` and `b` of a `fuse`
    /// or `common`, the target and the tool of a `cut` — or `None` when
    /// the result is not a boolean.
    pub fn operands(&self) -> Option<(Body, Body)> {
        let (x, y) = match &self.result {
            Step::Fuse { a, b, .. } | Step::Common { a, b, .. } => (a, b),
            Step::Cut { target, tool, .. } => (target, tool),
            Step::Box { .. }
            | Step::Cylinder { .. }
            | Step::Profile { .. }
            | Step::Extrude { .. }
            | Step::Revolve { .. }
            | Step::Transform { .. }
            | Step::Fillet { .. }
            | Step::Chamfer { .. } => return None,
        };
        Some((*self.bodies.get(x)?, *self.bodies.get(y)?))
    }
}

/// Builds every step of `dir`'s recipe under `variant` before the one
/// named as the result, and returns them with the result step itself.
/// Errors: as [`run`]'s build stage, and [`CorpusError::Reference`] when
/// no step is named as the result.
///
/// ```no_run
/// use arris_debug::{corpus, fixtures};
///
/// let dir = fixtures::corpus_root().join("boolean/through-hole");
/// let inputs = corpus::inputs(&dir, "default").unwrap();
/// let (plate, hole) = inputs.operands().unwrap();
/// assert_ne!(plate, hole);
/// ```
pub fn inputs(dir: &Path, variant: &str) -> Result<Inputs, CorpusError> {
    let fixture = fixtures::load(dir)?;
    let name = fixture.name.clone();
    let Some(params) = fixture.recipe.params_of(variant) else {
        return Err(CorpusError::Variant {
            fixture: name,
            variant: variant.to_string(),
        });
    };
    let mut model = Model::default();
    let mut made: BTreeMap<String, Made> = BTreeMap::new();
    let mut profiles: BTreeMap<String, Profile> = BTreeMap::new();
    for step in &fixture.recipe.steps {
        if step.name() == fixture.recipe.result {
            let bodies = made.iter().map(|(k, v)| (k.clone(), v.body)).collect();
            return Ok(Inputs {
                model,
                bodies,
                profiles,
                result: step.clone(),
            });
        }
        if let Some(out) = build_step(&mut model, &fixture, step, &params, &made, &mut profiles)? {
            made.insert(step.name().to_string(), out);
        }
    }
    Err(CorpusError::Reference {
        fixture: name,
        step: "result".into(),
        name: fixture.recipe.result.clone(),
    })
}

fn number(
    fixture: &Fixture,
    step: &Step,
    n: &Num,
    params: &BTreeMap<String, f64>,
) -> Result<f64, CorpusError> {
    n.eval(params).map_err(|source| CorpusError::Expression {
        fixture: fixture.name.clone(),
        step: step.name().to_string(),
        source,
    })
}

fn point(
    fixture: &Fixture,
    step: &Step,
    p: &[Num; 3],
    params: &BTreeMap<String, f64>,
) -> Result<Point3, CorpusError> {
    Ok(Point3::new(
        number(fixture, step, &p[0], params)?,
        number(fixture, step, &p[1], params)?,
        number(fixture, step, &p[2], params)?,
    ))
}

fn vector(
    fixture: &Fixture,
    step: &Step,
    v: &[Num; 3],
    params: &BTreeMap<String, f64>,
) -> Result<Vec3, CorpusError> {
    Ok(Vec3::new(
        number(fixture, step, &v[0], params)?,
        number(fixture, step, &v[1], params)?,
        number(fixture, step, &v[2], params)?,
    ))
}

/// The rigid motion of a `Step::Transform`: the rotation about `origin`
/// (the world origin when absent) applied first, then the translation —
/// `tools/oracle/oracle/recipe.py`'s `gp_Trsf` composition.
fn motion(
    fixture: &Fixture,
    step: &Step,
    rotate: &Option<Rotate>,
    translate: &Option<[Num; 3]>,
    params: &BTreeMap<String, f64>,
) -> Result<Isometry, CorpusError> {
    let mut m = Isometry::identity();
    if let Some(rot) = rotate {
        let axis = vector(fixture, step, &rot.axis, params)?;
        let origin = match &rot.origin {
            Some(o) => point(fixture, step, o, params)?,
            None => Point3::origin(),
        };
        let angle = number(fixture, step, &rot.angle_deg, params)?.to_radians();
        let rotation = UnitQuaternion::from_axis_angle(&UnitVec3::new_normalize(axis), angle);
        let about_origin = Isometry::from_rotation(rotation);
        m = Isometry::new(
            rotation,
            origin.coords - about_origin.apply_vec(origin.coords),
        );
    }
    if let Some(t) = translate {
        m = m.then(&Isometry::from_translation(vector(
            fixture, step, t, params,
        )?));
    }
    Ok(m)
}

/// The profile a sweep step names, built by an earlier `profile` step.
fn profile_reference<'a>(
    fixture: &Fixture,
    step: &Step,
    name: &str,
    profiles: &'a BTreeMap<String, Profile>,
) -> Result<&'a Profile, CorpusError> {
    profiles.get(name).ok_or_else(|| CorpusError::Reference {
        fixture: fixture.name.clone(),
        step: step.name().to_string(),
        name: name.to_string(),
    })
}

/// Builds one step: a body step into the [`Made`] it returns, a `profile`
/// step into `profiles` and `None`, since a sketch is a value and makes no
/// body.
fn build_step(
    m: &mut Model,
    fixture: &Fixture,
    step: &Step,
    params: &BTreeMap<String, f64>,
    made: &BTreeMap<String, Made>,
    profiles: &mut BTreeMap<String, Profile>,
) -> Result<Option<Made>, CorpusError> {
    let name = fixture.name.clone();
    let op = |source: OpError| CorpusError::Op {
        fixture: name.clone(),
        step: step.name().to_string(),
        source,
    };
    let body = |(body, provenance): (Body, Provenance), inputs: Vec<Body>| {
        Ok(Some(Made {
            body,
            provenance,
            inputs,
        }))
    };
    match step {
        Step::Box { min, max, .. } => {
            let (min, max) = (
                point(fixture, step, min, params)?,
                point(fixture, step, max, params)?,
            );
            body(primitive_box(m, min, max).map_err(op)?, Vec::new())
        }
        Step::Cylinder {
            base,
            axis,
            radius,
            height,
            ..
        } => {
            let base = point(fixture, step, base, params)?;
            let direction = point(fixture, step, axis, params)?;
            let axis = Axis::new(base, Vec3::from(direction.coords)).map_err(|source| {
                CorpusError::Axis {
                    fixture: name.clone(),
                    step: step.name().to_string(),
                    source,
                }
            })?;
            let radius = number(fixture, step, radius, params)?;
            let height = number(fixture, step, height, params)?;
            body(
                primitive_cylinder(m, axis, radius, height).map_err(op)?,
                Vec::new(),
            )
        }
        Step::Profile {
            name,
            plane,
            outer,
            holes,
        } => {
            let profile = build_profile(name, plane, outer, holes, params).map_err(|source| {
                CorpusError::Profile {
                    fixture: fixture.name.clone(),
                    step: name.clone(),
                    source: Box::new(source),
                }
            })?;
            profiles.insert(name.clone(), profile);
            Ok(None)
        }
        Step::Extrude {
            profile,
            direction,
            length,
            ..
        } => {
            let profile = profile_reference(fixture, step, profile, profiles)?;
            let direction = vector(fixture, step, direction, params)?;
            let length = number(fixture, step, length, params)?;
            body(
                extrude(m, profile, direction, length).map_err(op)?,
                Vec::new(),
            )
        }
        Step::Revolve {
            profile,
            axis,
            angle_deg,
            ..
        } => {
            let profile = profile_reference(fixture, step, profile, profiles)?;
            let origin = point(fixture, step, &axis.origin, params)?;
            let direction = vector(fixture, step, &axis.direction, params)?;
            let axis = Axis::new(origin, direction).map_err(|source| CorpusError::Axis {
                fixture: name.clone(),
                step: step.name().to_string(),
                source,
            })?;
            let angle = number(fixture, step, angle_deg, params)?.to_radians();
            body(revolve(m, profile, axis, angle).map_err(op)?, Vec::new())
        }
        Step::Transform {
            of,
            translate,
            rotate,
            ..
        } => {
            let of_body = reference(fixture, step, of, made)?.body;
            let motion = motion(fixture, step, rotate, translate, params)?;
            body(transform(m, of_body, &motion).map_err(op)?, vec![of_body])
        }
        Step::Fuse { a, b, .. } => {
            let a = reference(fixture, step, a, made)?.body;
            let b = reference(fixture, step, b, made)?.body;
            body(fuse(m, a, b).map_err(op)?, vec![a, b])
        }
        Step::Common { a, b, .. } => {
            let a = reference(fixture, step, a, made)?.body;
            let b = reference(fixture, step, b, made)?.body;
            body(common(m, a, b).map_err(op)?, vec![a, b])
        }
        Step::Cut { target, tool, .. } => {
            let target = reference(fixture, step, target, made)?.body;
            let tool = reference(fixture, step, tool, made)?.body;
            body(cut(m, target, tool).map_err(op)?, vec![target, tool])
        }
        Step::Fillet {
            of,
            edges,
            radius: size,
            ..
        }
        | Step::Chamfer {
            of,
            edges,
            distance: size,
            ..
        } => {
            let of_body = reference(fixture, step, of, made)?.body;
            let probe = fixture.recipe.tolerances.probe;
            let mut selected = Vec::with_capacity(edges.len());
            for p in edges {
                let point = point(fixture, step, p, params)?;
                let edge =
                    edge_at(m, of_body, point, probe).map_err(|what| CorpusError::EdgePoint {
                        fixture: name.clone(),
                        step: step.name().to_string(),
                        point: [point.x, point.y, point.z],
                        what,
                    })?;
                selected.push(edge);
            }
            let size = number(fixture, step, size, params)?;
            let blended = if matches!(step, Step::Chamfer { .. }) {
                chamfer(m, of_body, &selected, size)
            } else {
                fillet(m, of_body, &selected, size)
            };
            body(blended.map_err(op)?, vec![of_body])
        }
    }
}

/// The edge of `body` a recipe names by `point`: the one
/// `classify_point` answers `On(Edge)` for, when no second edge of the
/// body passes within `probe` of the point — the rule the oracle's
/// nearest-edge search keeps too (`tests/fixtures/README.md`). Errors:
/// what the point names instead.
fn edge_at(m: &Model, body: Body, point: Point3, probe: f64) -> Result<Edge, String> {
    let on = match classify_point(m, body, point) {
        Ok(Classification::On(shape)) => shape,
        Ok(Classification::Inside) => return Err("is inside the body, on no edge".into()),
        Ok(Classification::Outside) => return Err("is outside the body, on no edge".into()),
        Err(e) => return Err(format!("could not be classified: {e}")),
    };
    let EntityId::Edge(id) = on.id else {
        return Err(format!("is on {on}, not on an edge"));
    };
    let mut near: Vec<EntityId> = Vec::new();
    for edge in m.edges(body).map_err(|e| e.to_string())? {
        let entity = m.edge(edge.id).map_err(|e| e.to_string())?;
        let Some((curve, range)) = entity.curve() else {
            continue;
        };
        let curve = m.curve(curve).map_err(|e| e.to_string())?;
        let Ok(projection) = curve.project(point) else {
            continue;
        };
        let t = match curve.period() {
            Some(p) => {
                let k = ((range.lo() - projection.t) / p).ceil();
                projection.t + k * p
            }
            None => projection.t,
        };
        if projection.distance <= probe && range.lo() - probe <= t && t <= range.hi() + probe {
            near.push(EntityId::Edge(edge.id));
        }
    }
    if near.len() != 1 {
        return Err(format!(
            "is within the probe tolerance of {} edges ({})",
            near.len(),
            near.iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    Ok(Edge::forward(id))
}

fn reference<'a>(
    fixture: &Fixture,
    step: &Step,
    name: &str,
    made: &'a BTreeMap<String, Made>,
) -> Result<&'a Made, CorpusError> {
    made.get(name).ok_or_else(|| CorpusError::Reference {
        fixture: fixture.name.clone(),
        step: step.name().to_string(),
        name: name.to_string(),
    })
}

/// Compares Arris's mass properties against the oracle's, quantity by
/// quantity: volume and area relative, the centroid's distance
/// absolute, each component of the inertia tensor relative to the
/// tensor's largest one, so a product of inertia that cancels to zero is
/// not compared against itself. A quantity the oracle did not record is
/// skipped. Errors: the first quantity that differs, named with both
/// values.
/// ADR-0012's invariants on every face-local vertex of the mesh: the
/// face's surface at the corner's own (u, v) is the shared position it
/// stands on, within the face's tolerance, and its normal is the
/// surface's own in the face use's sense — outward — wherever the
/// parametrisation is not singular, and a unit vector in the tangent
/// plane where it is.
fn corners_stage(m: &Model, body: Body, mesh: &TriMesh) -> Result<(), String> {
    let corners = mesh
        .corners()
        .ok_or_else(|| "the corner block was asked for and is missing".to_string())?;
    let faces = m.faces(body).map_err(|e| e.to_string())?;
    if corners.faces().len() != faces.len() {
        return Err(format!(
            "{} corner faces for {} faces",
            corners.faces().len(),
            faces.len()
        ));
    }
    for (used, cf) in faces.iter().zip(corners.faces()) {
        if cf.face != used.id {
            return Err(format!(
                "corner face {} where the body has {}",
                cf.face, used.id
            ));
        }
        let face = m.face(used.id).map_err(|e| e.to_string())?;
        let surface = m.surface(face.surface()).map_err(|e| e.to_string())?;
        let reversed = used.orientation == Orientation::Reversed;
        for i in cf.vertices.clone() {
            let ([u, v], normal, shared) = (
                corners.uvs()[i],
                corners.normals()[i],
                corners.positions()[i] as usize,
            );
            let at = Point3::from(mesh.positions()[shared]);
            let on_surface = surface.point(u, v);
            let off = (on_surface - at).norm();
            if off.is_nan() || off > face.tolerance() {
                return Err(format!(
                    "{}: ({u}, {v}) evaluates to {on_surface}, {off:e} from the {at} it stands on, \
                     above the face's tolerance {:e}",
                    cf.face,
                    face.tolerance()
                ));
            }
            let n = Vec3::from(normal);
            if let Some(own) = surface.normal(u, v) {
                let outward = if reversed {
                    -own.into_inner()
                } else {
                    own.into_inner()
                };
                if (n - outward).norm() > CORNER_NORMAL_SLACK {
                    return Err(format!(
                        "{}: the normal at ({u}, {v}) is {n}, not the outward {outward}",
                        cf.face
                    ));
                }
            } else {
                let e = surface.eval(u, v);
                for d in [e.du, e.dv] {
                    if n.dot(&d).abs() > CORNER_NORMAL_SLACK * d.norm().max(1.0) {
                        return Err(format!(
                            "{}: the normal at the singular ({u}, {v}) leaves the tangent plane",
                            cf.face
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

/// How far a corner normal may lie from the direction it should be: the
/// two are the same computation, so this is rounding on a unit vector,
/// not a geometric tolerance.
const CORNER_NORMAL_SLACK: f64 = 1e-12;

fn measure_stage(
    m: &Model,
    body: Body,
    expected: &Measured,
    tolerances: &Tolerances,
) -> Result<(), String> {
    let found = mass_properties(m, body).map_err(|e| e.to_string())?;
    let relative = |name: &str, a: f64, e: f64, tolerance: f64| -> Result<(), String> {
        let difference = (a - e).abs() / e.abs().max(a.abs()).max(f64::MIN_POSITIVE);
        if difference.is_nan() || difference > tolerance {
            return Err(format!(
                "{name} {a} vs the oracle's {e}: {difference:e} relative, above {tolerance:e}"
            ));
        }
        Ok(())
    };
    if let Some(volume) = expected.volume {
        relative("volume", found.volume, volume, tolerances.volume_rel)?;
    }
    if let Some(area) = expected.area {
        relative("area", found.area, area, tolerances.area_rel)?;
    }
    if let Some(centroid) = expected.centroid {
        let oracle = Point3::new(centroid[0], centroid[1], centroid[2]);
        let distance = (found.centroid - oracle).norm();
        if distance.is_nan() || distance > tolerances.centroid_abs {
            return Err(format!(
                "centroid {} vs the oracle's {oracle}: {distance:e} apart, above centroid_abs {:e}",
                found.centroid, tolerances.centroid_abs
            ));
        }
    }
    if let Some(inertia) = expected.inertia {
        let scale = inertia
            .iter()
            .flatten()
            .fold(0.0f64, |m, x| m.max(x.abs()))
            .max(f64::MIN_POSITIVE);
        for (i, row) in inertia.iter().enumerate() {
            for (j, &e) in row.iter().enumerate() {
                let a = found.inertia[(i, j)];
                let difference = (a - e).abs() / scale;
                if difference.is_nan() || difference > tolerances.inertia_rel {
                    return Err(format!(
                        "inertia[{i}][{j}] {a} vs the oracle's {e}: {difference:e} of the tensor, above inertia_rel {:e}",
                        tolerances.inertia_rel
                    ));
                }
            }
        }
    }
    Ok(())
}

/// The lines that differ, with their numbers: `-` for the committed
/// side, `+` for this build's.
fn diff(committed: &str, actual: &str) -> String {
    let a: Vec<&str> = committed.lines().collect();
    let b: Vec<&str> = actual.lines().collect();
    let mut out = String::new();
    let n = a.len().max(b.len());
    let mut shown = 0;
    for i in 0..n {
        let (x, y) = (a.get(i), b.get(i));
        if x != y {
            if let Some(x) = x {
                out.push_str(&format!("-{:>5} {x}\n", i + 1));
            }
            if let Some(y) = y {
                out.push_str(&format!("+{:>5} {y}\n", i + 1));
            }
            shown += 1;
            if shown >= 20 {
                out.push_str("… (more)\n");
                break;
            }
        }
    }
    if a.len() != b.len() {
        out.push_str(&format!(
            "({} lines committed, {} in this build)\n",
            a.len(),
            b.len()
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_recipe_in_two_directories_names_two_step_files() {
        // The corpus's own copy keeps the name the docs quote.
        let canonical = crate::fixtures::corpus_root().join("primitive/cylinder");
        assert_eq!(
            step_tag("primitive/cylinder", "default", &canonical),
            "primitive-cylinder-default"
        );
        // A scratch copy of the same recipe — what this module's own tests
        // build — does not, so the two never write one file while they run
        // at the same time.
        let scratch = Path::new("/tmp/arris-corpus-dump-diff-1");
        let other = Path::new("/tmp/arris-corpus-dump-diff-2");
        let a = step_tag("primitive/cylinder", "default", scratch);
        let b = step_tag("primitive/cylinder", "default", other);
        assert!(a.starts_with("primitive-cylinder-default-"), "{a}");
        assert_ne!(a, "primitive-cylinder-default");
        assert_ne!(a, b, "two scratch copies name two files");
        assert_eq!(a, step_tag("primitive/cylinder", "default", scratch));
    }

    #[test]
    fn diff_names_the_lines_that_differ() {
        let d = diff("a\nb\nc\n", "a\nx\nc\nd\n");
        assert_eq!(
            d,
            "-    2 b\n+    2 x\n+    4 d\n(3 lines committed, 4 in this build)\n"
        );
        assert_eq!(diff("same\n", "same\n"), "");
    }

    #[test]
    fn the_measure_stage_holds_the_box_to_the_oracle_and_names_what_differs() {
        let dir = fixtures::corpus_root().join("primitive/box");
        let fixture = fixtures::load(&dir).unwrap();
        let expected = fixture.expected.results["default"].clone();
        let tolerances = fixture.recipe.tolerances;
        let mut m = Model::default();
        let (body, _) = primitive_box(&mut m, Point3::origin(), Point3::new(40.0, 30.0, 10.0))
            .expect("the box of the recipe");
        measure_stage(&m, body, &expected, &tolerances).expect("the oracle's numbers");
        // Every quantity is actually compared: move each one just past
        // its tolerance and the stage says which.
        let mut wrong = expected.clone();
        wrong.volume = Some(expected.volume.unwrap() * (1.0 + 1e-6));
        assert!(
            measure_stage(&m, body, &wrong, &tolerances)
                .unwrap_err()
                .starts_with("volume ")
        );
        let mut wrong = expected.clone();
        wrong.area = Some(expected.area.unwrap() * (1.0 + 1e-6));
        assert!(
            measure_stage(&m, body, &wrong, &tolerances)
                .unwrap_err()
                .starts_with("area ")
        );
        let mut wrong = expected.clone();
        let mut centroid = expected.centroid.unwrap();
        centroid[1] += 1e-3;
        wrong.centroid = Some(centroid);
        assert!(
            measure_stage(&m, body, &wrong, &tolerances)
                .unwrap_err()
                .starts_with("centroid ")
        );
        let mut wrong = expected.clone();
        let mut inertia = expected.inertia.unwrap();
        inertia[0][1] += inertia[2][2] * 1e-6;
        wrong.inertia = Some(inertia);
        assert!(
            measure_stage(&m, body, &wrong, &tolerances)
                .unwrap_err()
                .starts_with("inertia[0][1] ")
        );
    }

    #[test]
    fn dump_paths_and_blessing() {
        let dir = Path::new("x");
        assert_eq!(dump_path(dir, "default"), Path::new("x/dump.txt"));
        assert_eq!(dump_path(dir, "tighter"), Path::new("x/dump.tighter.txt"));
    }
}
