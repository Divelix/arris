//! The differential (ADR-0024 §2): recipes drawn by [`prop::recipe`]
//! built by Arris and by the Open CASCADE oracle, every outcome sorted
//! into an [`Outcome`] class.
//!
//! [`run`] writes each drawn recipe as a scratch fixture under
//! `target/inspect/differential/`, has one [`oracle::expected_batch`] call
//! answer all of them, then builds each in Arris and holds the result to
//! the corpus's [`corpus::stages`] — the checker, counts, measure, mesh,
//! probes and provenance — against the oracle's answer, with no dump and
//! no STEP round trip: those read committed files, and the corpus holds
//! them. A panic is caught here, on the test side, with `catch_unwind`;
//! the kernel never catches its own.
//!
//! `Agree`, `BothRefuse`, `ArrisRefuses` and `OracleRefuses` are counted;
//! `Disagree`, `CheckerViolation` and `Panic` fail the run
//! ([`Outcome::fails`]). Each failing case is shrunk through the
//! strategy's `ValueTree`, the oracle run per candidate through its
//! cache, and printed as a `fixture.json` ready to commit under
//! `tests/fixtures/regression/` (`tests/fixtures/README.md`
//! §Property-test failures).

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use arris_ops::{Fault, OpError};
use proptest::strategy::{Strategy, ValueTree};

use crate::corpus::{self, CorpusError, Stage};
use crate::fixtures::{self, Fixture, Recipe};
use crate::oracle::{self, OracleError};
use crate::prop::{self, recipe::recipe};

/// The environment variable that sets how many recipes [`run`] draws.
pub const CASES_VAR: &str = "ARRIS_DIFF_CASES";
/// Recipes per run when [`CASES_VAR`] is unset: what the pre-commit hook
/// runs, a draw small enough to stay a few seconds warm.
pub const DEFAULT_CASES: usize = 32;
/// The environment variable that caps the candidates one failing case's
/// shrink tries.
pub const SHRINK_VAR: &str = "ARRIS_DIFF_SHRINK";
/// Candidates per shrink when [`SHRINK_VAR`] is unset. Each costs an
/// oracle process on a cold cache, so the cap is what bounds a failing
/// run's time; a shrink it stops is still a smaller failing recipe.
pub const DEFAULT_SHRINK: usize = 64;

/// The panic message's start that says the debug build's guard caught an
/// operation's output failing the checker (`arris_ops`'s `verify`): a
/// [`Outcome::CheckerViolation`], not a plain [`Outcome::Panic`].
const CHECKER_GUARD: &str = "kernel bug: an operation's output fails the checker";

/// The number of recipes: [`CASES_VAR`], or [`DEFAULT_CASES`].
pub fn cases() -> usize {
    count(CASES_VAR, DEFAULT_CASES)
}

/// The shrink budget: [`SHRINK_VAR`], or [`DEFAULT_SHRINK`].
pub fn shrink_budget() -> usize {
    count(SHRINK_VAR, DEFAULT_SHRINK)
}

fn count(var: &str, default: usize) -> usize {
    std::env::var(var)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

/// What one recipe did in the two kernels.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Both build, and every stage passes.
    Agree,
    /// Open CASCADE builds no solid or refuses the recipe, and Arris
    /// returns a typed refusal.
    BothRefuse,
    /// Open CASCADE builds a solid and Arris returns a typed refusal,
    /// named by [`refusal`]: the kernel's rule working, counted per name.
    ArrisRefuses(String),
    /// Arris builds and Open CASCADE refuses the recipe, with why.
    OracleRefuses(String),
    /// Both answer and a stage differs — or Arris builds a body where
    /// Open CASCADE records no solid, a [`Stage::Build`] disagreement.
    Disagree {
        /// The stage.
        stage: Stage,
        /// What differed.
        what: String,
    },
    /// Arris returns `Ok` with a shape the checker rejects — at `Full`
    /// after the recipe, or at `Fast` in the debug build's guard after an
    /// operation — or an operation's input fails it.
    CheckerViolation(String),
    /// Arris panics.
    Panic(String),
}

impl Outcome {
    /// `true` for the classes that fail the run: a disagreement, a
    /// checker violation, a panic.
    pub fn fails(&self) -> bool {
        matches!(
            self,
            Outcome::Disagree { .. } | Outcome::CheckerViolation(_) | Outcome::Panic(_)
        )
    }

    /// The class's name, as the histogram prints it.
    pub fn class(&self) -> &'static str {
        match self {
            Outcome::Agree => "Agree",
            Outcome::BothRefuse => "BothRefuse",
            Outcome::ArrisRefuses(_) => "ArrisRefuses",
            Outcome::OracleRefuses(_) => "OracleRefuses",
            Outcome::Disagree { .. } => "Disagree",
            Outcome::CheckerViolation(_) => "CheckerViolation",
            Outcome::Panic(_) => "Panic",
        }
    }

    /// `true` when both kernels built the recipe and it was compared: an
    /// agreement, or a disagreement past the build.
    pub fn compared(&self) -> bool {
        match self {
            Outcome::Agree => true,
            Outcome::Disagree { stage, .. } => *stage != Stage::Build,
            _ => false,
        }
    }

    /// Whether a shrink candidate's outcome still shows the failure
    /// `self` is: the same class, and for a disagreement the same stage.
    fn same_failure(&self, other: &Outcome) -> bool {
        match (self, other) {
            (Outcome::Disagree { stage: a, .. }, Outcome::Disagree { stage: b, .. }) => a == b,
            _ => self.class() == other.class(),
        }
    }
}

impl core::fmt::Display for Outcome {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Outcome::Agree | Outcome::BothRefuse => f.write_str(self.class()),
            Outcome::ArrisRefuses(why) | Outcome::OracleRefuses(why) => {
                write!(f, "{}: {why}", self.class())
            }
            Outcome::Disagree { stage, what } => write!(f, "Disagree({stage}): {what}"),
            Outcome::CheckerViolation(what) | Outcome::Panic(what) => {
                write!(f, "{}: {what}", self.class())
            }
        }
    }
}

/// The name a typed refusal is counted under: the `Reason` of a
/// `Degenerate` — `Degenerate(TangentContact)` — the kinds of an
/// `Unsupported`, the fault of an `Internal`, the variant otherwise.
/// Without the entities or numbers, so one cause is one row.
pub fn refusal(e: &OpError) -> String {
    /// The variant's name from its `Debug`: up to the first field.
    fn head(debug: String) -> String {
        let end = debug.find([' ', '(', '{']).unwrap_or(debug.len());
        debug[..end].to_string()
    }
    match e {
        OpError::Degenerate { reason, .. } => {
            format!("Degenerate({})", head(format!("{reason:?}")))
        }
        OpError::Unsupported { a, b } => format!("Unsupported({} × {})", a.0, b.0),
        OpError::Internal(fault) => format!("Internal({})", head(format!("{fault:?}"))),
        OpError::InvalidInput { .. } => "InvalidInput".into(),
        OpError::Profile(_) => "Profile".into(),
        OpError::Tolerance { .. } => "Tolerance".into(),
        OpError::NotFound(_) => "NotFound".into(),
    }
}

/// The outcome of a caught panic: the debug build's checker guard is a
/// [`Outcome::CheckerViolation`], any other a [`Outcome::Panic`].
fn panicked(payload: Box<dyn std::any::Any + Send>) -> Outcome {
    let message = payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic with no message".into());
    if message.starts_with(CHECKER_GUARD) {
        Outcome::CheckerViolation(message)
    } else {
        Outcome::Panic(message)
    }
}

/// Builds `recipe` in Arris under the name `name` and sorts the result
/// against `oracle`: the fixture the oracle answered with (`recipe` and
/// its `expected.json`), or why it refused. Deterministic, and never
/// panics on the kernel's behalf — a kernel panic is the outcome.
///
/// ```
/// use arris_debug::differential::{Outcome, judge};
/// use arris_debug::fixtures::Recipe;
///
/// let recipe: Recipe = serde_json::from_str(
///     r#"{"steps": [{"op": "box", "name": "b", "min": [0, 0, 0], "max": [1, 2, 3]}], "result": "b"}"#,
/// )
/// .unwrap();
/// let outcome = judge("generated/box", &recipe, Err("not asked"));
/// assert!(matches!(outcome, Outcome::OracleRefuses(_)));
/// ```
pub fn judge(name: &str, recipe: &Recipe, oracle: Result<&Fixture, &str>) -> Outcome {
    let built = match catch_unwind(AssertUnwindSafe(|| corpus::build(name, recipe))) {
        Ok(built) => built,
        Err(payload) => return panicked(payload),
    };
    let oracle_solid =
        matches!(oracle, Ok(f) if f.expected.results.get("default").is_some_and(|r| !r.degenerate));
    let chain = match built {
        Ok(chain) => chain,
        Err(CorpusError::Op { source, .. }) => {
            return match &source {
                OpError::InvalidInput { .. } | OpError::Internal(Fault::Checker(_)) => {
                    Outcome::CheckerViolation(source.to_string())
                }
                _ if oracle_solid => Outcome::ArrisRefuses(refusal(&source)),
                _ => Outcome::BothRefuse,
            };
        }
        // The strategy writes only well-formed recipes, so anything else
        // is the generator's fault — and fails the run so it is fixed.
        Err(e) => {
            return Outcome::Disagree {
                stage: e.stage(),
                what: format!("the recipe does not build as written: {e}"),
            };
        }
    };
    let fixture = match oracle {
        Err(why) => return Outcome::OracleRefuses(why.to_string()),
        Ok(fixture) => fixture,
    };
    let Some(expected) = fixture.expected.results.get("default") else {
        return Outcome::OracleRefuses("expected.json has no default result".into());
    };
    if expected.degenerate {
        return Outcome::Disagree {
            stage: Stage::Build,
            what: "Open CASCADE records no solid and Arris builds a body".into(),
        };
    }
    match catch_unwind(AssertUnwindSafe(|| {
        corpus::stages(fixture, &chain, expected)
    })) {
        Ok(Ok(_)) => Outcome::Agree,
        Ok(Err(e)) if e.stage() == Stage::Check => Outcome::CheckerViolation(e.to_string()),
        Ok(Err(e)) => Outcome::Disagree {
            stage: e.stage(),
            what: e.to_string(),
        },
        Err(payload) => panicked(payload),
    }
}

/// One drawn recipe and what it did.
#[derive(Debug, Clone)]
pub struct Case {
    /// Its place in the draw, from 0.
    pub index: usize,
    /// The recipe.
    pub recipe: Recipe,
    /// Its outcome.
    pub outcome: Outcome,
    /// Arris's wall clock on it: the build and the stages.
    pub arris: Duration,
}

/// A failing case shrunk.
#[derive(Debug, Clone)]
pub struct Failure {
    /// The case as drawn.
    pub index: usize,
    /// The drawn outcome.
    pub outcome: Outcome,
    /// The smallest recipe found that fails the same way.
    pub recipe: Recipe,
    /// Its outcome.
    pub shrunk: Outcome,
    /// The candidates the shrink tried.
    pub tried: usize,
}

impl Failure {
    /// The shrunk recipe as a `fixture.json`, its description saying
    /// where it came from and how it fails — ready to commit under
    /// `tests/fixtures/regression/<slug>/` beside the oracle's
    /// `expected.json`.
    pub fn fixture_json(&self, seed: &[u8; 32]) -> String {
        let mut recipe = self.recipe.clone();
        recipe.description = format!(
            "drawn by the differential, case {} of seed {}, shrunk in {} candidates: {}",
            self.index,
            prop::seed_hex(seed),
            self.tried,
            first_line(&self.shrunk.to_string()),
        );
        serde_json::to_string_pretty(&recipe).unwrap_or_else(|e| e.to_string())
    }
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("")
}

/// A differential run: every case in draw order, the failing ones shrunk.
#[derive(Debug, Clone)]
pub struct Run {
    /// The seed drawn from.
    pub seed: [u8; 32],
    /// Every case, in draw order.
    pub cases: Vec<Case>,
    /// Every failing case, shrunk, in draw order.
    pub failures: Vec<Failure>,
    /// The wall clock of the whole run, shrinking apart.
    pub elapsed: Duration,
}

impl Run {
    /// Cases per class, and per refusal name under `ArrisRefuses`; the
    /// keys are the class names, and `ArrisRefuses/<name>` for a refusal.
    pub fn histogram(&self) -> BTreeMap<String, usize> {
        let mut out = BTreeMap::new();
        for case in &self.cases {
            *out.entry(case.outcome.class().to_string()).or_default() += 1;
            match &case.outcome {
                Outcome::ArrisRefuses(name) => {
                    *out.entry(format!("ArrisRefuses/{name}")).or_default() += 1;
                }
                Outcome::Disagree { stage, .. } => {
                    *out.entry(format!("Disagree/{stage}")).or_default() += 1;
                }
                _ => {}
            }
        }
        out
    }

    /// The summary a test prints: the seed, the count and time, the share
    /// of the draw that reached a comparison, and the histogram.
    pub fn report(&self) -> String {
        let n = self.cases.len();
        let compared = self.cases.iter().filter(|c| c.outcome.compared()).count();
        let arris: Duration = self.cases.iter().map(|c| c.arris).sum();
        let per = |d: Duration| d.as_secs_f64() / n.max(1) as f64;
        let mut out = format!(
            "differential: {n} recipes, {}={}\n\
             {compared} of {n} compared ({:.0}%), the rest refused by one kernel or both\n\
             {:.2} s wall clock, {:.3} s per recipe; Arris {:.3} s per recipe on one thread\n",
            prop::SEED_VAR,
            prop::seed_hex(&self.seed),
            100.0 * compared as f64 / n.max(1) as f64,
            self.elapsed.as_secs_f64(),
            per(self.elapsed),
            per(arris),
        );
        for (class, count) in self.histogram() {
            let indent = if class.contains('/') { "    " } else { "  " };
            let name = class.rsplit('/').next().unwrap_or(&class);
            out.push_str(&format!("{indent}{name:<40} {count:>5}\n"));
        }
        out
    }

    /// Every failure: how it failed as drawn, how it fails shrunk, and its
    /// `fixture.json`.
    pub fn failures_text(&self) -> String {
        let mut out = String::new();
        for f in &self.failures {
            out.push_str(&format!(
                "case {}: {}\nshrunk in {} candidates: {}\n{}\n\n",
                f.index,
                f.outcome,
                f.tried,
                f.shrunk,
                f.fixture_json(&self.seed)
            ));
        }
        out
    }
}

/// The scratch directory case `name` is written to.
fn case_dir(name: &str) -> PathBuf {
    oracle::scratch_dir().join("differential").join(name)
}

/// The oracle's answer for a scratch fixture it answered: the fixture
/// loaded, or why it could not be.
fn load(dir: &Path, answer: Result<(), String>) -> Result<Fixture, String> {
    answer?;
    fixtures::load(dir).map_err(|e| e.to_string())
}

/// Draws `n` recipes from `seed`, has the oracle answer them in one batch
/// and judges each ([`judge`]) on as many threads as the machine has,
/// then shrinks every failing case ([`shrink`], [`shrink_budget`]
/// candidates at most). Deterministic in everything but the times.
/// Errors: the oracle's, when it could not run or a scratch fixture could
/// not be written — never a refusal, which is an outcome.
///
/// ```no_run
/// use arris_debug::{differential, prop};
///
/// let run = differential::run(&prop::DEFAULT_SEED, 8).unwrap();
/// println!("{}", run.report());
/// assert!(run.failures.is_empty(), "{}", run.failures_text());
/// ```
pub fn run(seed: &[u8; 32], n: usize) -> Result<Run, OracleError> {
    let start = Instant::now();
    let strategy = recipe();
    let mut runner = prop::runner_with_seed(seed);
    let mut trees = Vec::with_capacity(n);
    for _ in 0..n {
        trees.push(
            strategy
                .new_tree(&mut runner)
                .map_err(|e| OracleError::Environment {
                    message: format!("the recipe strategy drew nothing: {e}"),
                })?,
        );
    }
    let recipes: Vec<Recipe> = trees.iter().map(|t| t.current()).collect();
    let names: Vec<String> = (0..n).map(|i| format!("{i:04}")).collect();
    let dirs: Vec<PathBuf> = names.iter().map(|name| case_dir(name)).collect();
    for (dir, recipe) in dirs.iter().zip(&recipes) {
        oracle::write_recipe(dir, recipe)?;
    }
    let answers = oracle::expected_batch(&dirs)?;
    let oracles: Vec<Result<Fixture, String>> = dirs
        .iter()
        .zip(answers)
        .map(|(dir, answer)| load(dir, answer))
        .collect();

    // Each case builds in its own model, so the cases run at once; the
    // results go back in draw order.
    let judged: Mutex<Vec<Option<(Outcome, Duration)>>> = Mutex::new(vec![None; n]);
    let next = AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map_or(1, |p| p.get());
    std::thread::scope(|s| {
        for _ in 0..threads.min(n) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= n {
                        break;
                    }
                    let begun = Instant::now();
                    let outcome = judge(
                        &format!("differential/{}", names[i]),
                        &recipes[i],
                        oracles[i].as_ref().map_err(String::as_str),
                    );
                    let took = begun.elapsed();
                    judged.lock().unwrap_or_else(|e| e.into_inner())[i] = Some((outcome, took));
                }
            });
        }
    });
    let judged = judged.into_inner().unwrap_or_else(|e| e.into_inner());
    let cases: Vec<Case> = recipes
        .into_iter()
        .zip(judged)
        .enumerate()
        .map(|(index, (recipe, judged))| {
            let (outcome, arris) =
                judged.unwrap_or_else(|| (Outcome::Panic("never judged".into()), Duration::ZERO));
            Case {
                index,
                recipe,
                outcome,
                arris,
            }
        })
        .collect();
    let elapsed = start.elapsed();

    let mut failures = Vec::new();
    let budget = shrink_budget();
    for (case, tree) in cases.iter().zip(trees.iter_mut()) {
        if !case.outcome.fails() {
            continue;
        }
        let dir = case_dir(&format!("{}-shrink", names[case.index]));
        let name = format!("differential/{}-shrink", names[case.index]);
        let (recipe, shrunk, tried) = shrink(tree, &case.outcome, budget, |candidate| {
            oracle::write_recipe(&dir, candidate)?;
            let answer = oracle::expected_batch(std::slice::from_ref(&dir))?
                .pop()
                .unwrap_or_else(|| Err("no answer".into()));
            let fixture = load(&dir, answer);
            Ok(judge(
                &name,
                candidate,
                fixture.as_ref().map_err(String::as_str),
            ))
        })?;
        failures.push(Failure {
            index: case.index,
            outcome: case.outcome.clone(),
            recipe,
            shrunk,
            tried,
        });
    }
    Ok(Run {
        seed: *seed,
        cases,
        failures,
        elapsed,
    })
}

/// Shrinks the failing case `tree` holds, whose outcome is `first`:
/// simplifies while `judge` still finds the same failure
/// ([`Outcome::fails`], the same class and for a disagreement the same
/// stage), complicates back when it does not, for at most `budget`
/// candidates. Returns the smallest failing recipe found, its outcome and
/// the candidates tried. Errors: `judge`'s.
pub fn shrink<T, F>(
    tree: &mut T,
    first: &Outcome,
    budget: usize,
    mut judge: F,
) -> Result<(Recipe, Outcome, usize), OracleError>
where
    T: ValueTree<Value = Recipe>,
    F: FnMut(&Recipe) -> Result<Outcome, OracleError>,
{
    let mut best = (tree.current(), first.clone());
    let mut tried = 0;
    if !tree.simplify() {
        return Ok((best.0, best.1, tried));
    }
    while tried < budget {
        let candidate = tree.current();
        let outcome = judge(&candidate)?;
        tried += 1;
        if outcome.fails() && first.same_failure(&outcome) {
            best = (candidate, outcome);
            if !tree.simplify() {
                break;
            }
        } else if !tree.complicate() {
            break;
        }
    }
    Ok((best.0, best.1, tried))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::Step;

    fn a_box(max: [f64; 3]) -> Recipe {
        serde_json::from_str(&format!(
            r#"{{"steps": [{{"op": "box", "name": "b", "min": [0, 0, 0], "max": {max:?}}}], "result": "b"}}"#
        ))
        .unwrap()
    }

    /// A box's own fixture: the corpus's `primitive/box` answer, which is
    /// the oracle's for a 40 × 30 × 10 box, under this recipe.
    fn answered(recipe: &Recipe) -> Fixture {
        let mut fixture = fixtures::load(&fixtures::corpus_root().join("primitive/box")).unwrap();
        fixture.recipe = recipe.clone();
        fixture
            .expected
            .results
            .get_mut("default")
            .unwrap()
            .probes
            .clear();
        fixture
    }

    #[test]
    fn each_answer_sorts_into_its_class() {
        let good = a_box([40.0, 30.0, 10.0]);
        let fixture = answered(&good);
        assert_eq!(judge("t/agree", &good, Ok(&fixture)), Outcome::Agree);

        // A different box against the same answer: counts agree, the
        // volume does not.
        let other = a_box([40.0, 30.0, 11.0]);
        let Outcome::Disagree { stage, .. } = judge("t/other", &other, Ok(&answered(&other)))
        else {
            panic!("a larger box is not the oracle's");
        };
        assert_eq!(stage, Stage::Measure);

        // Arris builds, the oracle refuses.
        assert!(matches!(
            judge("t/refused", &good, Err("no")),
            Outcome::OracleRefuses(_)
        ));

        // Arris refuses a box with no height: counted by its reason when
        // the oracle builds, both refuse when it does not.
        let flat = a_box([40.0, 30.0, 0.0]);
        assert_eq!(
            judge("t/flat", &flat, Ok(&answered(&flat))),
            Outcome::ArrisRefuses("Degenerate(NotPositive)".into())
        );
        assert_eq!(judge("t/flat", &flat, Err("no")), Outcome::BothRefuse);
        let mut none = answered(&flat);
        none.expected.results.get_mut("default").unwrap().degenerate = true;
        assert_eq!(judge("t/flat", &flat, Ok(&none)), Outcome::BothRefuse);

        // Arris builds a body where the oracle records none.
        let mut empty = answered(&good);
        empty
            .expected
            .results
            .get_mut("default")
            .unwrap()
            .degenerate = true;
        assert!(matches!(
            judge("t/empty", &good, Ok(&empty)),
            Outcome::Disagree {
                stage: Stage::Build,
                ..
            }
        ));
    }

    #[test]
    fn a_panic_is_a_checker_violation_only_from_the_guard() {
        let guard = panicked(Box::new(format!("{CHECKER_GUARD}\nL4 …")));
        assert!(matches!(guard, Outcome::CheckerViolation(_)));
        assert_eq!(
            panicked(Box::new("index out of bounds")),
            Outcome::Panic("index out of bounds".into())
        );
        assert!(guard.fails() && !Outcome::BothRefuse.fails());
    }

    #[test]
    fn the_histogram_counts_classes_and_refusals() {
        let case = |outcome| Case {
            index: 0,
            recipe: a_box([1.0, 1.0, 1.0]),
            outcome,
            arris: Duration::ZERO,
        };
        let run = Run {
            seed: prop::DEFAULT_SEED,
            cases: vec![
                case(Outcome::Agree),
                case(Outcome::ArrisRefuses("Degenerate(TangentContact)".into())),
                case(Outcome::ArrisRefuses("Degenerate(TangentContact)".into())),
                case(Outcome::Disagree {
                    stage: Stage::Probes,
                    what: String::new(),
                }),
            ],
            failures: Vec::new(),
            elapsed: Duration::ZERO,
        };
        let h = run.histogram();
        assert_eq!(h["Agree"], 1);
        assert_eq!(h["ArrisRefuses"], 2);
        assert_eq!(h["ArrisRefuses/Degenerate(TangentContact)"], 2);
        assert_eq!(h["Disagree/probes"], 1);
        assert!(run.report().contains("2 of 4 compared (50%)"));
    }

    /// The shrink keeps the failure while it simplifies: a stand-in judge
    /// that fails every recipe with a `cut` shrinks a drawn one to a
    /// recipe that still has one, and no longer than it was.
    #[test]
    fn a_shrink_keeps_the_failure() {
        let fails = |r: &Recipe| r.steps.iter().any(|s| matches!(s, Step::Cut { .. }));
        let mut runner = prop::runner_with_seed(&prop::DEFAULT_SEED);
        let failure = Outcome::Panic("a cut".into());
        let mut shrunk_one = false;
        for _ in 0..64 {
            let mut tree = recipe().new_tree(&mut runner).unwrap();
            let drawn = tree.current();
            if !fails(&drawn) {
                continue;
            }
            let (small, outcome, tried) = shrink(&mut tree, &failure, 256, |r| {
                Ok(if fails(r) {
                    failure.clone()
                } else {
                    Outcome::Agree
                })
            })
            .unwrap();
            assert!(fails(&small));
            assert_eq!(outcome, failure);
            assert!(small.steps.len() <= drawn.steps.len());
            assert!(tried > 0);
            shrunk_one = true;
            break;
        }
        assert!(shrunk_one, "no draw of 64 has a cut");
    }
}
