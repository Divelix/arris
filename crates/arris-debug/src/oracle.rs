//! Running the Open CASCADE oracle on a file Arris wrote
//! (`tools/oracle/README.md`): the seam between a test and `compare.py`
//! for a STEP file, `mesh.py` for an STL one, and between a test's own
//! recipe and `expected.py` for a body the corpus has no fixture for. A
//! missing environment is a loud error naming the command that creates
//! it, never a skip. Every answer the oracle settles is kept in
//! [`cache`] by what produced it, so an unchanged call starts no Python.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

pub mod cache;

use crate::fixtures::Recipe;

/// Why the oracle did not answer, or answered no.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OracleError {
    /// The STEP text could not be written to the scratch directory.
    #[error("could not write {path}: {message}")]
    Write {
        /// The file.
        path: PathBuf,
        /// The cause.
        message: String,
    },
    /// `uv` could not be run, or `compare.py` or `expected.py` refused:
    /// no environment, a stale `expected.json`, an unknown variant, a
    /// recipe the oracle cannot build.
    #[error(
        "the oracle could not run: {message}\nrun `uv sync --project tools/oracle` (tools/oracle/README.md)"
    )]
    Environment {
        /// What `uv` or `compare.py` said.
        message: String,
    },
    /// `compare.py` ran and found a difference; the table says which.
    #[error("{fixture} does not match Arris's STEP ({file}):\n{table}")]
    Mismatch {
        /// The fixture compared against.
        fixture: String,
        /// The STEP file, kept for inspection.
        file: PathBuf,
        /// The comparison table.
        table: String,
    },
}

static SPAWNS: AtomicUsize = AtomicUsize::new(0);

/// How many `uv` processes this process has started for the oracle: the
/// number a test asserts on to show a call was answered from [`cache`].
pub fn spawns() -> usize {
    SPAWNS.load(Ordering::Relaxed)
}

/// `uv run --project tools/oracle tools/oracle/<script>` from the
/// workspace root, with its arguments still to add.
fn uv(script: &str) -> Command {
    let mut command = Command::new("uv");
    command
        .current_dir(workspace_root())
        .args(["run", "--project", "tools/oracle"])
        .arg(format!("tools/oracle/{script}"));
    command
}

/// Runs `command`, counted by [`spawns`].
fn spawn(command: &mut Command) -> Result<Output, OracleError> {
    SPAWNS.fetch_add(1, Ordering::Relaxed);
    command.output().map_err(|e| OracleError::Environment {
        message: format!("could not run `uv`: {e}"),
    })
}

/// Stores a settled answer. A cache that cannot be written is no reason
/// to fail a call the oracle answered, so the error is dropped.
fn keep(slot: Option<(PathBuf, cache::Key)>, bytes: &[u8]) {
    if let Some((dir, key)) = slot {
        let _ = cache::store(&dir, &key, bytes);
    }
}

fn environment(output: &Output) -> OracleError {
    OracleError::Environment {
        message: format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    }
}

/// The workspace root: two levels above this crate's manifest.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

/// The scratch directory STEP files go to for the oracle to read:
/// `target/inspect/` under the workspace root (gitignored, kept after a
/// failure so the file can be looked at).
pub fn scratch_dir() -> PathBuf {
    workspace_root().join("target/inspect")
}

/// Writes `recipe` as `target/inspect/<name>/fixture.json` and has
/// `expected.py` write its `expected.json` beside it: a scratch fixture
/// for a test that holds a body to the oracle's reading of its STEP
/// through [`compare_dir`] without a fixture in the corpus — a body whose
/// fixture the corpus cannot yet run, held to closed forms and to the
/// oracle's volume here instead. Returns the directory. Errors:
/// [`OracleError::Write`]; [`OracleError::Environment`] when `uv` or
/// `expected.py` could not run or the oracle refused the recipe.
pub fn scratch_fixture(name: &str, recipe: &Recipe) -> Result<PathBuf, OracleError> {
    let dir = scratch_dir().join(name);
    let file = dir.join("fixture.json");
    let text = serde_json::to_string_pretty(recipe).map_err(|e| OracleError::Write {
        path: file.clone(),
        message: e.to_string(),
    })?;
    std::fs::create_dir_all(&dir)
        .and_then(|()| std::fs::write(&file, &text))
        .map_err(|e| OracleError::Write {
            path: file.clone(),
            message: e.to_string(),
        })?;
    let expected = dir.join("expected.json");
    let slot = cache::slot("expected.py", &[Some(text.as_bytes())], None);
    if let Some(bytes) = slot.as_ref().and_then(|(d, k)| cache::load(d, k)) {
        std::fs::write(&expected, bytes).map_err(|e| OracleError::Write {
            path: expected.clone(),
            message: e.to_string(),
        })?;
        return Ok(dir);
    }
    let output = spawn(uv("expected.py").arg(&dir))?;
    if !output.status.success() {
        return Err(environment(&output));
    }
    if let Ok(bytes) = std::fs::read(&expected) {
        keep(slot, &bytes);
    }
    Ok(dir)
}

/// Writes `step_text` as `target/inspect/<tag>.step` and runs
/// `compare.py` on it against `tests/fixtures/<fixture>` (`variant`, or
/// `default`). Returns the comparison table on a match. Errors:
/// [`OracleError::Mismatch`] with the table; [`OracleError::Environment`]
/// when `uv` or the oracle could not run — never a silent skip.
pub fn compare(
    fixture: &str,
    step_text: &str,
    variant: Option<&str>,
    tag: &str,
) -> Result<String, OracleError> {
    compare_dir(
        &workspace_root().join("tests/fixtures").join(fixture),
        step_text,
        variant,
        tag,
    )
}

/// [`compare`] against a fixture directory anywhere — a scratch copy in
/// a test, a fixture outside the corpus.
///
/// A `MATCH` is kept in [`cache`] under the STEP text, the directory's
/// `fixture.json` and `expected.json` and the variant; a later call with
/// all of them unchanged returns the same table without running the
/// oracle. A mismatch is never kept.
pub fn compare_dir(
    dir: &Path,
    step_text: &str,
    variant: Option<&str>,
    tag: &str,
) -> Result<String, OracleError> {
    let fixture = dir.to_string_lossy().into_owned();
    let scratch = scratch_dir();
    let file = scratch.join(format!("{tag}.step"));
    std::fs::create_dir_all(&scratch)
        .and_then(|()| std::fs::write(&file, step_text))
        .map_err(|e| OracleError::Write {
            path: file.clone(),
            message: e.to_string(),
        })?;
    let read = |name: &str| std::fs::read(dir.join(name)).ok();
    let (spec, expected) = (read("fixture.json"), read("expected.json"));
    let slot = cache::slot(
        "compare.py",
        &[
            Some(step_text.as_bytes()),
            spec.as_deref(),
            expected.as_deref(),
        ],
        Some(variant.unwrap_or("default")),
    );
    if let Some(bytes) = slot.as_ref().and_then(|(d, k)| cache::load(d, k)) {
        return Ok(String::from_utf8_lossy(&bytes).into_owned());
    }
    let mut command = uv("compare.py");
    command.arg(dir).arg(&file);
    if let Some(v) = variant {
        command.args(["--variant", v]);
    }
    let output = spawn(&mut command)?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    match output.status.code() {
        Some(0) if stdout.contains("MATCH") => {
            keep(slot, stdout.as_bytes());
            Ok(stdout)
        }
        Some(1) => Err(OracleError::Mismatch {
            fixture,
            file,
            table: stdout,
        }),
        _ => Err(OracleError::Environment {
            message: format!("{stdout}{stderr}"),
        }),
    }
}

/// Open CASCADE's `RWStl` reading of an STL file: how many facets it saw,
/// their total area and their signed volume by the divergence theorem —
/// the same formula `arris_mesh::TriMesh::signed_volume` and `area` use —
/// so a test holds its own mesh's numbers to an independent reader of the
/// bytes `arris_io::stl` wrote rather than to itself. `tools/oracle/mesh.py`
/// is the script; unlike [`compare`], nothing here knows a fixture's
/// expected numbers, so there is no [`OracleError::Mismatch`] — a test
/// compares the fields itself.
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
pub struct StlReading {
    /// How many facets `RWStl` read.
    pub triangles: usize,
    /// The sum of the facets' areas.
    pub area: f64,
    /// `Σ a · (b × c) / 6` over the facets in file order.
    pub volume: f64,
}

/// Writes `stl` (ASCII text or binary bytes) to
/// `target/inspect/<tag>.stl` and has `tools/oracle/mesh.py` read it back
/// through Open CASCADE's `RWStl`. Errors: [`OracleError::Write`];
/// [`OracleError::Environment`] when `uv` could not run, the environment
/// is missing, or the file did not parse as STL.
pub fn compare_stl(stl: &[u8], tag: &str) -> Result<StlReading, OracleError> {
    let scratch = scratch_dir();
    let file = scratch.join(format!("{tag}.stl"));
    std::fs::create_dir_all(&scratch)
        .and_then(|()| std::fs::write(&file, stl))
        .map_err(|e| OracleError::Write {
            path: file.clone(),
            message: e.to_string(),
        })?;
    let slot = cache::slot("mesh.py", &[Some(stl)], None);
    if let Some(bytes) = slot.as_ref().and_then(|(d, k)| cache::load(d, k)) {
        if let Ok(reading) = serde_json::from_slice(&bytes) {
            return Ok(reading);
        }
    }
    let output = spawn(uv("mesh.py").arg(&file))?;
    if !output.status.success() {
        return Err(environment(&output));
    }
    let reading = serde_json::from_slice(&output.stdout).map_err(|e| OracleError::Environment {
        message: format!("mesh.py's output did not parse as JSON: {e}"),
    })?;
    keep(slot, &output.stdout);
    Ok(reading)
}
