//! Running the Open CASCADE oracle on a STEP file Arris wrote
//! (`tools/oracle/README.md`): the seam between a test and `compare.py`.
//! A missing environment is a loud error naming the command that creates
//! it, never a skip.

use std::path::{Path, PathBuf};
use std::process::Command;

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
    /// `uv` could not be run, or `compare.py` refused: no environment, a
    /// stale `expected.json`, an unknown variant.
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
    let dir = scratch_dir();
    let file = dir.join(format!("{tag}.step"));
    std::fs::create_dir_all(&dir)
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
        .arg(root.join("tests/fixtures").join(fixture))
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
            fixture: fixture.to_string(),
            file,
            table: stdout,
        }),
        _ => Err(OracleError::Environment {
            message: format!("{stdout}{stderr}"),
        }),
    }
}
