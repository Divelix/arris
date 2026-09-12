//! Running the Open CASCADE oracle on a STEP file Arris wrote
//! (`tools/oracle/README.md`): the seam between a test and `compare.py`,
//! and between a test's own recipe and `expected.py` for a body the
//! corpus has no fixture for. A missing environment is a loud error
//! naming the command that creates it, never a skip.

use std::path::{Path, PathBuf};
use std::process::Command;

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
        .and_then(|()| std::fs::write(&file, text))
        .map_err(|e| OracleError::Write {
            path: file.clone(),
            message: e.to_string(),
        })?;
    let output = Command::new("uv")
        .current_dir(workspace_root())
        .args([
            "run",
            "--project",
            "tools/oracle",
            "tools/oracle/expected.py",
        ])
        .arg(&dir)
        .output()
        .map_err(|e| OracleError::Environment {
            message: format!("could not run `uv`: {e}"),
        })?;
    if !output.status.success() {
        return Err(OracleError::Environment {
            message: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        });
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
    let root = workspace_root();
    let mut command = Command::new("uv");
    command
        .current_dir(&root)
        .args([
            "run",
            "--project",
            "tools/oracle",
            "tools/oracle/compare.py",
        ])
        .arg(dir)
        .arg(&file);
    if let Some(v) = variant {
        command.args(["--variant", v]);
    }
    let output = command.output().map_err(|e| OracleError::Environment {
        message: format!("could not run `uv`: {e}"),
    })?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    match output.status.code() {
        Some(0) if stdout.contains("MATCH") => Ok(stdout),
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
