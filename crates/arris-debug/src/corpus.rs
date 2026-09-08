//! The corpus runner: the fixture test of `docs/03-roadmap.md` §Fixtures,
//! one call per fixture and variant. [`run`] builds the recipe in Arris,
//! runs the checker at `Full`, compares counts and genus against the
//! oracle's `expected.json`, writes STEP and has the oracle read it back
//! (`compare.py`), measures the result over the B-Rep and holds its
//! volume, area, centroid and inertia to the oracle's within the
//! fixture's tolerances, tessellates the result and holds the mesh
//! closed with its signed volume within the fixture's `mesh_volume_rel`
//! of the oracle's at `mesh_chord`, asserts the provenance accounting of
//! every step, and diffs the text dump against the committed `dump.txt` —
//! written only under `ARRIS_BLESS=1`. Every stage that fails is a typed
//! [`CorpusError`] saying which fixture, which stage and what differed;
//! a recipe step the kernel has no operation for yet is
//! [`CorpusError::Unsupported`] naming the op, which is what an
//! `#[ignore]`d fixture reports until its milestone lands. A result the
//! oracle recorded no solid for (`expected.degenerate`) must fail with
//! `OpError::Degenerate`, and one the recipe marks `analytic.expect_error`
//! must fail with that typed refusal; either ends the run there, the
//! oracle's numbers recorded but not compared.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use arris_io::arris_check::arris_topo::arris_math::nalgebra::UnitQuaternion;
use arris_io::arris_check::arris_topo::arris_math::{
    Axis, FrameError, Isometry, Point3, UnitVec3, Vec3,
};
use arris_io::arris_check::arris_topo::{Body, Model, Orientation, Origin, Provenance, Shape};
use arris_io::arris_check::classify::{Classification, classify_point};
use arris_io::arris_check::{Level, Report, check};
use arris_io::step::{self, StepError};
use arris_mesh::tessellate;
use arris_ops::measure::mass_properties;
use arris_ops::{OpError, Reason, cut, primitive_box, primitive_cylinder, transform};

use crate::dump::dump_text;
use crate::fixtures::{
    self, Class, Counts, ExpectError, ExprError, Fixture, FixtureError, Measured, Num, Rotate,
    Step, Tolerances,
};
use crate::oracle::{self, OracleError};

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
    /// A step's op has no operation in the kernel yet.
    #[error("{fixture}: step {step:?} uses op {op:?}, which the kernel has no operation for yet")]
    Unsupported {
        /// The fixture.
        fixture: String,
        /// The step's name.
        step: String,
        /// The op.
        op: &'static str,
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
    /// The counts differ from the oracle's.
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
            Refusal::Error(ExpectError::MultiShell) => {
                "OpError::Degenerate with Reason::MultiShell".into()
            }
            Refusal::Error(ExpectError::TangentContact) => {
                "OpError::Degenerate with Reason::TangentContact".into()
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
                    Refusal::Error(ExpectError::MultiShell) => {
                        matches!(reason, Reason::MultiShell { .. })
                    }
                    Refusal::Error(ExpectError::TangentContact) => reason == Reason::TangentContact,
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
struct Made {
    body: Body,
    provenance: Provenance,
    inputs: Vec<Body>,
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
    for step in &fixture.recipe.steps {
        let built = build_step(&mut m, &fixture, step, &params, &made);
        if let Some(refusal) = refusal {
            if step.name() == fixture.recipe.result {
                return refusal.assert(&name, step.name(), built.map(|_| ()));
            }
        }
        made.insert(step.name().to_string(), built?);
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

    // Counts and genus.
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
    if found != expected.counts {
        return Err(CorpusError::Counts {
            fixture: name,
            expected: expected.counts,
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
    let tag = format!("{}-{variant}", name.replace('/', "-"));
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
    let mesh =
        tessellate(&m, body, tolerances.mesh_chord).map_err(|e| mesh_failure(e.to_string()))?;
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

    // Provenance accounting, every step.
    for step in &fixture.recipe.steps {
        let out = &made[step.name()];
        account(&m, out).map_err(|what| CorpusError::Provenance {
            fixture: name.clone(),
            step: step.name().to_string(),
            what,
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
    /// Every step before the result, by name.
    pub bodies: BTreeMap<String, Body>,
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
            | Step::Transform { .. } => return None,
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
    for step in &fixture.recipe.steps {
        if step.name() == fixture.recipe.result {
            let bodies = made.iter().map(|(k, v)| (k.clone(), v.body)).collect();
            return Ok(Inputs {
                model,
                bodies,
                result: step.clone(),
            });
        }
        let out = build_step(&mut model, &fixture, step, &params, &made)?;
        made.insert(step.name().to_string(), out);
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

fn build_step(
    m: &mut Model,
    fixture: &Fixture,
    step: &Step,
    params: &BTreeMap<String, f64>,
    made: &BTreeMap<String, Made>,
) -> Result<Made, CorpusError> {
    let name = fixture.name.clone();
    let op = |source: OpError| CorpusError::Op {
        fixture: name.clone(),
        step: step.name().to_string(),
        source,
    };
    let unsupported = |op: &'static str| CorpusError::Unsupported {
        fixture: name.clone(),
        step: step.name().to_string(),
        op,
    };
    match step {
        Step::Box { min, max, .. } => {
            let (min, max) = (
                point(fixture, step, min, params)?,
                point(fixture, step, max, params)?,
            );
            let (body, provenance) = primitive_box(m, min, max).map_err(op)?;
            Ok(Made {
                body,
                provenance,
                inputs: Vec::new(),
            })
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
            let (body, provenance) = primitive_cylinder(m, axis, radius, height).map_err(op)?;
            Ok(Made {
                body,
                provenance,
                inputs: Vec::new(),
            })
        }
        Step::Profile { .. } => Err(unsupported("profile")),
        Step::Extrude { profile, .. } => {
            reference(fixture, step, profile, made)?;
            Err(unsupported("extrude"))
        }
        Step::Revolve { profile, .. } => {
            reference(fixture, step, profile, made)?;
            Err(unsupported("revolve"))
        }
        Step::Transform {
            of,
            translate,
            rotate,
            ..
        } => {
            let of_body = reference(fixture, step, of, made)?.body;
            let motion = motion(fixture, step, rotate, translate, params)?;
            let (body, provenance) = transform(m, of_body, &motion).map_err(op)?;
            Ok(Made {
                body,
                provenance,
                inputs: vec![of_body],
            })
        }
        Step::Fuse { a, b, .. } => {
            reference(fixture, step, a, made)?;
            reference(fixture, step, b, made)?;
            Err(unsupported("fuse"))
        }
        Step::Common { a, b, .. } => {
            reference(fixture, step, a, made)?;
            reference(fixture, step, b, made)?;
            Err(unsupported("common"))
        }
        Step::Cut { target, tool, .. } => {
            let target = reference(fixture, step, target, made)?.body;
            let tool = reference(fixture, step, tool, made)?.body;
            let (body, provenance) = cut(m, target, tool).map_err(op)?;
            Ok(Made {
                body,
                provenance,
                inputs: vec![target, tool],
            })
        }
    }
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

/// Every entity of the output body (the body itself included) is kept
/// from an input or has an origin; every entity of every input body is
/// kept or recorded; nothing is both deleted and modified
/// (`docs/02-data-model.md` §Provenance).
fn account(m: &Model, made: &Made) -> Result<(), String> {
    let entities = |body: Body| -> Result<BTreeSet<Shape>, String> {
        let c = m.closure(body).map_err(|e| e.to_string())?;
        let mut set: BTreeSet<Shape> = BTreeSet::new();
        set.extend(
            c.vertices
                .iter()
                .map(|&v| Shape::new(v, Orientation::Forward)),
        );
        set.extend(c.edges.iter().map(|&e| Shape::new(e, Orientation::Forward)));
        set.extend(c.faces.iter().map(|&f| Shape::new(f, Orientation::Forward)));
        set.extend(
            c.shells
                .iter()
                .map(|&s| Shape::new(s, Orientation::Forward)),
        );
        set.insert(Shape::new(body.id, Orientation::Forward));
        Ok(set)
    };
    let p = &made.provenance;
    let output = entities(made.body)?;
    let mut inputs: BTreeSet<Shape> = BTreeSet::new();
    for &b in &made.inputs {
        inputs.extend(entities(b)?);
    }
    let recorded = |s: Shape| {
        let o = Origin::Entity(s);
        !p.generated_from(o).is_empty() || !p.modified_from(o).is_empty() || p.is_deleted(s)
    };
    for &e in &output {
        let kept = inputs.contains(&e) && !recorded(e);
        if !kept && p.origins(e).is_empty() {
            return Err(format!(
                "{e} is in the output with no origin and is not kept"
            ));
        }
    }
    for &e in &inputs {
        let kept = output.contains(&e) && !recorded(e);
        if !kept && !recorded(e) {
            return Err(format!("{e} is an input that is neither kept nor recorded"));
        }
        if p.is_deleted(e) && !p.modified_from(Origin::Entity(e)).is_empty() {
            return Err(format!("{e} is both deleted and modified"));
        }
    }
    for s in p.deleted() {
        if !inputs.contains(&s) {
            return Err(format!("{s} is deleted but is not an input"));
        }
    }
    Ok(())
}

/// Compares Arris's mass properties against the oracle's, quantity by
/// quantity: volume and area relative, the centroid's distance
/// absolute, each component of the inertia tensor relative to the
/// tensor's largest one, so a product of inertia that cancels to zero is
/// not compared against itself. A quantity the oracle did not record is
/// skipped. Errors: the first quantity that differs, named with both
/// values.
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
