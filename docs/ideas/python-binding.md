# Idea: python-binding

- Status: Open
- Raised: 2026-09-24
- Prompt (verbatim from the human): "for the first-party binding (the Python / agent-scripting consumer)"

## Problem

`SEED.md` §1 names code-first modelling among the consumers Arris is for,
and ADR-0020 §2 puts a first-party binding in this repository, starting
after C3. C3 closed on 2026-09-24. Today that consumer can't exist
without writing Rust: an agent that wants to fillet a box, cut a bore,
read the mass properties and write STEP has to compile a crate first.
The oracle side already works in Python (`tools/oracle/`), so the same
script can't be run against both kernels either.

ADR-0020 deferred five questions to this idea: layer position, the
`#![forbid(unsafe_code)]` exception, `publish`, the wasm job, and PyPI
beside crates.io. The machine-local review behind ADR-0020 settled two
things the human decided on 2026-09-20: a **thin** binding lives in this
repository, and a CadQuery-style ergonomic layer does not.

## What was checked, not assumed

In a scratch crate with pyo3 0.26 and `abi3-py310`:

- **pyo3 compiles under `#![forbid(unsafe_code)]`.** Tested with
  `#[pyclass]`, including a mutable class taking `&mut self`,
  `#[pymethods]`, `#[pymodule]`, `create_exception!` with a subclass
  chain, and `py.detach` (the GIL released around a call). The premise
  that "a binding needs an unsafe exception" (ADR-0020 §2, the review's
  cost 1) is false for everything a thin binding uses. What would need
  `unsafe` is hand-written buffer-protocol support (`__getbuffer__`), and
  the design below avoids it.
- **`cargo test` links** with the crate in the workspace, so the hook's
  nextest and clippy runs keep working.
- **`--target wasm32-unknown-unknown` fails** inside `pyo3-ffi`
  (`libc::wchar_t` unresolved). The CI `wasm` job builds `--workspace`, so
  the binding has to be gated: pyo3 as a
  `cfg(not(target_arch = "wasm32"))` dependency with an empty crate on
  wasm, or the crate excluded from the job.
- **The name `arris` is free on PyPI** (404 on 2026-09-24), and so are
  `arris-kernel` and `pyarris`.

## Constraints it runs into

- **Layer rule** (ADR-0013, `tools/check-layers.sh`): the binding goes
  above `arris` and depends on the facade only. Nothing depends on it. The
  script's table gains a layer 8.
- **`.agents/rules/kernel.md` §API**: exhaustive enums break every cycle
  on purpose. Each break now also updates the binding in the same commit,
  since `main` is always green. That cost was accepted when the human
  chose in-repo.
- **Ids have no model identity** (`docs/ARCHITECTURE.md` §The model): a
  `BodyId` is a slot and a generation, allocated deterministically, so a
  handle from one model usually resolves in another to a *different, real*
  entity rather than to `NotFound`. In Rust the caller owns that risk. In
  Python an agent juggling two models will hit it. The binding must close
  this itself, with no kernel change: a Python handle carries its model.
- **`arris-debug` is `publish = false` and dev-facing** (ARCHITECTURE
  §Formats and tools). The PNG render and the dump live there. SEED §4
  lists rendering as a non-goal.
- **`.agents/rules/git.md` §Tags**: releases are lockstep, a tag publishes,
  and publishing is the human's. PyPI would be a second registry behind
  the same tag.
- **Two active plans** (`.agents/rules/docs-lifecycle.md`): the measuring
  harness holds one slot and the reader's plans want the other
  (`docs/ideas/reader-cycle-scope.md`).

## Options

### A — A thin 1:1 binding over the facade (`crates/arris-py`)

pyo3 with abi3 (one wheel per platform for every Python ≥ 3.10), built
by maturin, keeping `forbid(unsafe_code)`. It mirrors the Rust API
rather than reinventing it:

- `Model` is a Python class. Handles are frozen, hashable classes that
  carry their model, and a foreign handle raises before reaching the
  kernel.
- Every operation returns `(body, provenance)`, and the provenance is
  readable as generated, modified and deleted lists.
- `OpError` and the other error enums map to one exception class per
  variant, under a common `ArrisError`. The entities they name are
  attributes, not just text, so an agent can branch on them.
- Queries (mass properties, face frames, the checker's report),
  tessellation and every writer (STEP, STL, OBJ, native).
- The mesh crosses as `bytes` of little-endian f64 plus counts, with a
  small pure-Python shim that offers `numpy.frombuffer` when numpy is
  installed. No rust-numpy dependency and no buffer protocol.
- Hand-written `.pyi` stubs and docstrings with runnable examples.
  Signatures matter more to an agent than to anyone else.
- Long operations release the GIL. `Model` is `Send + Sync`, so this is
  free.

Cost: about 9 plan steps (skeleton, layer table and wasm gate; model and
handles; operations with provenance; errors; queries; mesh and io; stubs
and pytest; CI wheel job; release to PyPI). It rules out nothing, and an
ergonomic layer in another repository sits on top of it.

### B — A recipe interpreter as the agent surface, no Python

A binary that takes the corpus recipe grammar (`Step` in `arris-debug`,
the same eleven operations the oracle runs) as JSON, and returns STEP,
measurements and the checker's verdict. It is cheap (about 3 steps),
builds on wasm and needs no second registry. But a recipe isn't a
program: it has no loops, no queries that feed back into the next
operation, and no selecting an edge from provenance. It lives in an
unpublished crate. It is useful as a fuzzing and oracle surface, which
the measuring harness already covers. It doesn't serve the consumer
ADR-0020 named.

### C — A, plus the ergonomic layer in this repository

Workplanes, selectors and fluent chains, as in CadQuery. This reverses
the human's 2026-09-20 decision, and it puts an opinion about modelling
style into a kernel repository. It is out of scope, not a trade-off.

### Do nothing

The consumer ADR-0020 put in scope keeps not existing, and the in-repo
decision goes stale while the API grows. Every cycle that passes widens
the surface the first binding commit has to cover.

## Recommendation

**A**, with the forbid kept, pyo3 gated off wasm, and PyPI publishing in
lockstep from the same tag. The first PyPI upload stays the human's act,
as the first crates.io one was. B isn't a binding. C is a decision
already taken. Rendering stays out: an agent gets the mesh and renders it
with its own tools, and the kernel's PNG path stays dev tooling.

Start it when a plan slot frees up, and before the reader lands if
possible. Its plans then gain Python exposure one commit at a time
instead of all at once.

What would change my mind: a pyo3 upgrade that starts emitting `unsafe`
the lint catches. The forbid then needs its ADR exception after all,
confined to this crate. The plan's first step re-runs the check.

## Decision for the human

1. A thin 1:1 binding over the facade, in `crates/arris-py`, above
   `arris`? *Preferred: yes (A).*
2. Keep `#![forbid(unsafe_code)]` on it, since the check above shows pyo3
   doesn't need the exception ADR-0020 anticipated? *Preferred: yes.*
3. Publish to PyPI as `arris`, in lockstep from the same `v*` tag, with
   the crate itself `publish = false` on crates.io? *Preferred: yes. The
   first upload waits for your go-ahead, and you reserve the name.*
4. Python tests (pytest over a `maturin develop` build in a `uv` venv) in
   CI only, not in the pre-commit hook? *Preferred: CI only. The hook's
   Rust checks still compile the crate, so a Rust API break still fails
   locally.*
5. Mesh as `bytes` with an optional numpy shim, and no PNG render in the
   wheel? *Preferred: yes.*
6. ADR: yes, one, at the plan's first step. It records the layer position,
   the forbid kept (with the evidence), the wasm gate, lockstep PyPI, and
   handles carrying their model. It amends ADR-0020 §2's "exception a
   binding needs".
