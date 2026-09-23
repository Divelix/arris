//! The oracle's answers kept by what produced them (ADR-0024 §1): a
//! `compare.py` table that said `MATCH`, a scratch fixture's
//! `expected.json`, an STL reading. The key is sha256 over the script's
//! name, the bytes of every file it reads, the variant and a digest of the
//! oracle itself, so any change to an input or to a line of the oracle
//! misses. A mismatch or an environment error is never stored, so the
//! cache cannot hide a failure.
//!
//! The entries live in `target/oracle-cache/`, one file per key.
//! [`VAR`]`=off` bypasses them for reading and writing; CI sets it, so the
//! cache is only ever a local speed-up.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use sha2::{Digest, Sha256};

use super::workspace_root;

/// The environment variable that turns the cache off: `off` bypasses it,
/// any other value or none uses [`default_dir`].
pub const VAR: &str = "ARRIS_ORACLE_CACHE";

/// Where the cache reads and writes, or that it does neither.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Setting {
    /// Every call runs the oracle and nothing is stored.
    Off,
    /// Entries are read from and written to this directory.
    At(PathBuf),
}

/// `target/oracle-cache/` under the workspace root.
pub fn default_dir() -> PathBuf {
    workspace_root().join("target/oracle-cache")
}

static OVERRIDE: Mutex<Option<Setting>> = Mutex::new(None);

/// Replaces the environment's setting for the rest of the process
/// (`None` gives it back). For a test that has to know the cache is on,
/// or where it is, whatever [`VAR`] says — the environment cannot be set
/// from safe code once threads run.
pub fn set(setting: Option<Setting>) {
    *OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()) = setting;
}

/// The setting in force: [`set`]'s, else [`VAR`]'s.
pub fn setting() -> Setting {
    if let Some(s) = OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return s;
    }
    match std::env::var(VAR) {
        Ok(v) if v == "off" => Setting::Off,
        _ => Setting::At(default_dir()),
    }
}

/// A cache key: sha256 over the oracle's digest, the script, the files it
/// reads and the variant, each part length-prefixed so no two different
/// lists of parts hash alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key([u8; 32]);

impl Key {
    /// The key of running `script` on `inputs` (a file the script reads,
    /// or `None` where it is absent — distinct from an empty file) at
    /// `variant`, with the oracle's sources at `oracle`
    /// ([`oracle_digest`]).
    pub fn new(
        oracle: &[u8; 32],
        script: &str,
        inputs: &[Option<&[u8]>],
        variant: Option<&str>,
    ) -> Key {
        let mut h = Sha256::new();
        h.update(oracle);
        part(&mut h, Some(script.as_bytes()));
        h.update((inputs.len() as u64).to_le_bytes());
        for input in inputs {
            part(&mut h, *input);
        }
        part(&mut h, variant.map(str::as_bytes));
        Key(h.finalize().into())
    }

    /// The key as 64 lower-case hex digits: its file name in the cache.
    pub fn hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }
}

fn part(h: &mut Sha256, bytes: Option<&[u8]>) {
    match bytes {
        None => h.update([0u8]),
        Some(b) => {
            h.update([1u8]);
            h.update((b.len() as u64).to_le_bytes());
            h.update(b);
        }
    }
}

/// A digest of the oracle's sources under `root`: every `*.py` below it
/// (outside hidden directories and `__pycache__`, so neither the venv nor
/// Python's byte code counts), `pyproject.toml` and `uv.lock`, each by
/// its path relative to `root` and its bytes, in path order. Errors: a
/// file or directory that could not be read, or a missing
/// `pyproject.toml` or `uv.lock`.
pub fn digest_tree(root: &Path) -> io::Result<[u8; 32]> {
    let mut files = vec!["pyproject.toml".to_owned(), "uv.lock".to_owned()];
    python_files(root, "", &mut files)?;
    files.sort();
    let mut h = Sha256::new();
    for rel in &files {
        let bytes = std::fs::read(root.join(rel))?;
        part(&mut h, Some(rel.as_bytes()));
        part(&mut h, Some(&bytes));
    }
    Ok(h.finalize().into())
}

fn python_files(dir: &Path, rel: &str, out: &mut Vec<String>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        if entry.file_type()?.is_dir() {
            if !name.starts_with('.') && name != "__pycache__" {
                python_files(&entry.path(), &path, out)?;
            }
        } else if name.ends_with(".py") {
            out.push(path);
        }
    }
    Ok(())
}

/// [`digest_tree`] of `tools/oracle/`, computed once per process; `None`
/// when it could not be read, and then nothing is cached.
pub fn oracle_digest() -> Option<[u8; 32]> {
    static DIGEST: OnceLock<Option<[u8; 32]>> = OnceLock::new();
    *DIGEST.get_or_init(|| digest_tree(&workspace_root().join("tools/oracle")).ok())
}

/// The entry for `key` under `dir`, if one was stored.
pub fn load(dir: &Path, key: &Key) -> Option<Vec<u8>> {
    std::fs::read(dir.join(key.hex())).ok()
}

/// Stores `bytes` as the entry for `key` under `dir`: written beside it
/// and renamed into place, so a reader in another process sees the whole
/// entry or none of it.
pub fn store(dir: &Path, key: &Key, bytes: &[u8]) -> io::Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        "{}.{}.{}.tmp",
        key.hex(),
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, dir.join(key.hex()))
}

/// Where a call to `script` on `inputs` at `variant` is cached under the
/// setting in force: the directory and the key, or `None` when the cache
/// is off or the oracle's sources could not be read.
pub(crate) fn slot(
    script: &str,
    inputs: &[Option<&[u8]>],
    variant: Option<&str>,
) -> Option<(PathBuf, Key)> {
    let Setting::At(dir) = setting() else {
        return None;
    };
    let oracle = oracle_digest()?;
    Some((dir, Key::new(&oracle, script, inputs, variant)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = super::super::scratch_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// A tree shaped like `tools/oracle/`: scripts at the top, a package,
    /// the project files, and a venv and byte code that must not count.
    fn oracle_tree(name: &str) -> PathBuf {
        let root = scratch(name);
        write(&root, "compare.py", "print('compare')\n");
        write(&root, "oracle/measure.py", "TOL = 1e-9\n");
        write(&root, "pyproject.toml", "[project]\nname = 'oracle'\n");
        write(&root, "uv.lock", "version = 1\n");
        write(&root, ".venv/lib/site.py", "venv\n");
        write(&root, "oracle/__pycache__/measure.py", "bytecode\n");
        root
    }

    #[test]
    fn every_input_misses_on_its_own() {
        let root = oracle_tree("oracle-cache-tree");
        let oracle = digest_tree(&root).unwrap();
        let step: &[u8] = b"ISO-10303-21;\nDATA;\n#1=POINT(0.0);\n";
        let fixture: &[u8] = br#"{"steps": []}"#;
        let expected: &[u8] = br#"{"results": {}}"#;
        let key = |oracle: &[u8; 32], step: &[u8], expected: Option<&[u8]>, variant: &str| {
            Key::new(
                oracle,
                "compare.py",
                &[Some(step), Some(fixture), expected],
                Some(variant),
            )
        };
        let base = key(&oracle, step, Some(expected), "default");
        let dir = scratch("oracle-cache-entries");
        store(&dir, &base, b"MATCH").unwrap();
        assert_eq!(load(&dir, &base).as_deref(), Some(&b"MATCH"[..]));
        assert_eq!(key(&oracle, step, Some(expected), "default"), base);

        // One STEP byte.
        let mut other = step.to_vec();
        other[20] = b'1';
        let mut misses = vec![(
            "a STEP byte",
            key(&oracle, &other, Some(expected), "default"),
        )];
        // The variant.
        misses.push(("the variant", key(&oracle, step, Some(expected), "big")));
        // `expected.json`, edited or gone.
        misses.push((
            "expected.json",
            key(&oracle, step, Some(br#"{"results": {} }"#), "default"),
        ));
        misses.push(("no expected.json", key(&oracle, step, None, "default")));
        misses.push((
            "an empty expected.json",
            key(&oracle, step, Some(b""), "default"),
        ));
        // One `.py` file of the package, and `uv.lock`.
        write(&root, "oracle/measure.py", "TOL = 1e-8\n");
        let edited = digest_tree(&root).unwrap();
        misses.push(("a .py file", key(&edited, step, Some(expected), "default")));
        write(&root, "oracle/measure.py", "TOL = 1e-9\n");
        assert_eq!(digest_tree(&root).unwrap(), oracle);
        write(&root, "uv.lock", "version = 2\n");
        let relocked = digest_tree(&root).unwrap();
        misses.push(("uv.lock", key(&relocked, step, Some(expected), "default")));
        write(&root, "uv.lock", "version = 1\n");

        for (what, k) in &misses {
            assert_ne!(*k, base, "{what} left the key unchanged");
            assert_eq!(load(&dir, k), None, "{what} hit the cache");
        }
        // The script's name is part of the key.
        let other_script = Key::new(
            &oracle,
            "mesh.py",
            &[Some(step), Some(fixture), Some(expected)],
            Some("default"),
        );
        assert_ne!(other_script, base);
    }

    #[test]
    fn the_venv_and_byte_code_are_not_the_oracle() {
        let root = oracle_tree("oracle-cache-venv");
        let oracle = digest_tree(&root).unwrap();
        write(&root, ".venv/lib/site.py", "another venv\n");
        write(&root, "oracle/__pycache__/measure.py", "other bytecode\n");
        write(&root, "notes.txt", "not python\n");
        assert_eq!(digest_tree(&root).unwrap(), oracle);
        // A new script is.
        write(&root, "oracle/new.py", "\n");
        assert_ne!(digest_tree(&root).unwrap(), oracle);
    }

    #[test]
    fn a_tree_without_its_lock_has_no_digest() {
        let root = oracle_tree("oracle-cache-unlocked");
        std::fs::remove_file(root.join("uv.lock")).unwrap();
        assert!(digest_tree(&root).is_err());
    }
}
