# Plan: m0-harness

- Started: 2026-09-05
- Milestone: M0 (cycle C1, docs/03-roadmap.md)
- Idea (verbatim from the human): "Next per the seed: the architecture
  document, the data model document, and the roadmap, then a plan for M0."

## Goal

Everything the agent needs to see and to judge geometry exists before the
first curve is written: the workspace with the layer rule enforced, the
bookkeeping types (ids, handles, orientation, precision), the checker's
skeleton with one `Violation` per invariant, a mesh type and a PNG
rasteriser the agent can read, the Open CASCADE oracle as a `uv` project
with a recipe interpreter and a self-test, the whole C1 fixture corpus laid
out with oracle values generated, and the property-test configuration. At
the end, `cargo test --workspace` is green on a workspace that contains no
geometry, and the oracle matches its own STEP on every fixture.

## Non-goals

No `Surface`, `Curve`, entity, arena or operation — those are M1 and M2.
No B-Rep tessellation (M3); the rasteriser draws a `TriMesh` that a test
built by hand. No STEP writer (M2); the oracle self-test uses OCCT's own
STEP. No Rerun (M3). No publishing of the workspace crates (backlog).

## Design deltas

- Root `Cargo.toml` becomes a virtual workspace manifest; the placeholder
  `arris` package moves to `crates/arris/` unchanged in name and version
  (0.0.1 stays until the backlog's publish line is picked up). Eight new
  crates under `crates/` per 01-architecture §Crates, each empty but for
  the crate doc, `#![forbid(unsafe_code)]`, `#![warn(missing_docs)]`.
  `[workspace.dependencies]` pins `nalgebra`, `robust`, `thiserror`,
  `serde`, `proptest`, `image`.
- New public types (all documented in 02-data-model §Conventions):
  `arris_math::Precision`; `arris_topo::{VertexId, EdgeId, FaceId,
  ShellId, BodyId, CurveId, SurfaceId, Curve2Id, EntityId, Orientation,
  Shape, Body, Shell, Face, Edge, Vertex}`.
- New public types in `arris_check`: `Level`, `Violation` (one variant per
  row of 02-data-model §Invariants, numbered in its doc comment), `Report`.
  `check()` itself is M2 — it needs a `Model`.
- New public types in `arris_mesh`: `TriMesh`, `FaceRange`, `EdgeRange`,
  `Polyline`, with `signed_volume`, `area`, `is_closed`, `aabb`.
- New public API in `arris_debug`: `render_png`, `View`, `Highlight`,
  `fixtures::{Fixture, Expected, load, corpus}`, `prop::{config,
  finite_f64, unit_vec3, rotation}`.
- `tools/oracle/` (Python, not a crate): `pyproject.toml`, `oracle/recipe.py`,
  `oracle/measure.py`, `expected.py`, `compare.py`, `selftest.py`, `README.md`.
- `tests/fixtures/README.md` states the fixture format (03-roadmap
  §Fixtures) and the `proptest-regressions` policy.
- CI and the pre-commit hook gain `tools/check-layers.sh`.
- No ADR expected. If the human overrules the recipe-vs-STEP question
  below, that decision gets ADR-0001.

## Steps

- [x] Step 1 — Workspace. Virtual root manifest, `crates/arris-{math,geom,
  topo,check,ops,mesh,io,debug}` and `crates/arris` with the dependency
  edges of 01-architecture and nothing else; workspace lints; feature names
  reserved (`serde`, `parallel`, `rerun`) but empty. `tools/check-layers.sh`
  walks `cargo metadata` and fails on any upper→lower edge; wired into CI
  and `.githooks/pre-commit`. Test: the script passes, then fails when a
  forbidden edge is added in a scratch copy; `cargo build --workspace
  --target wasm32-unknown-unknown` passes; README says "workspace".
- [x] Step 2 — Bookkeeping types. `Precision` with the six fields and
  documented defaults in `arris-math`; generational ids, `EntityId`,
  `Orientation` with XOR composition, `Shape` and the typed handles with
  `From` conversions in `arris-topo`; `serde` derives behind the feature.
  Tests: composition is XOR and associative (proptest); ids order by
  `(index, generation)`; serde round trip of a `Shape`.
- [x] Step 3 — Checker skeleton. `Level`, `Violation` and `Report` in
  `arris-check`; `Report` displays violations sorted by entity then
  variant, deterministic; `is_ok`. Test: a doc-drift test parses the
  invariant tables of `docs/02-data-model.md` and asserts the set of
  numbers `{M1…, V1…, E1…, L1…, F1…, S1…, B1…}` equals the set the
  `Violation` variants declare in their doc comments — the doc and the enum
  cannot diverge silently.
- [x] Step 4 — Mesh types. `TriMesh` (f64 positions, indices, per-face
  and per-edge ranges keyed by `FaceId`/`EdgeId`), `Polyline`,
  `signed_volume` (divergence theorem), `area`, `is_closed` (every
  directed edge has its opposite exactly once), `aabb`. Tests: a hand-built
  cube gives 8, 24, closed; the same cube with one triangle flipped is not
  closed; a cube with a triangle removed is open and its volume is reported
  as unreliable.
- [x] Step 5 — Rasteriser. `arris_debug::render_png(&TriMesh, &[Polyline],
  View, Option<Highlight>, path)`: orthographic projection for `Iso`,
  `Top`, `Front`, `Right`, z-buffer, flat shading, one deterministic colour
  per face id, polylines in black, vertices as dots, highlight in red,
  fixed 800×600, written under `target/inspect/`. Tests: the cube renders
  with exactly three visible face colours in `Iso` and one in `Top`,
  counted from the pixel buffer; the agent reads the PNG once and records
  in the commit body that it shows a cube. `inspect` skill's PNG row
  updated to the real signature.
- [x] Step 6 — Oracle environment and interpreter. `tools/oracle/` as a
  `uv` project pinned to Python 3.12 and `cadquery-ocp==8.0.1.*` (resolves
  on this machine, checked 2026-09-05); `oracle/recipe.py` builds a body
  from a `fixture.json` recipe — box, cylinder, planar profile of lines
  and arcs with holes, extrude, revolve, transform, fuse/common/cut,
  chained by name; `oracle/measure.py` — volume, area, centroid (`GProp`),
  counts by unique sub-shape, loops, shells, genus and the Euler line,
  point classification (`BRepClass3d`); `expected.py <dir>` writes
  `expected.json` with the OCCT version and the recipe hash; `compare.py
  <dir> <file.step>` reads STEP, measures, compares within the fixture's
  tolerances, prints a table, exits non-zero on mismatch and loudly when
  the venv is missing. `selftest.py` runs `expected.py`, writes OCCT's own
  STEP and runs `compare.py` on it for every fixture. `README.md` says how
  to create the venv. `inspect` skill's oracle row updated.
- [ ] Step 7 — Fixture corpus. `tests/fixtures/README.md`; a
  `fixture.json` for every row of 03-roadmap §C1 acceptance corpus with
  its probe points, tolerances and `analytic` closed forms; `expected.json`
  generated by the oracle and committed; `arris_debug::fixtures` loads
  both; a corpus lint test in `crates/arris/tests/` asserts every directory
  has both files, the Euler line is zero, `analytic` matches the oracle to
  1e-6 relative, and the recipe hash in `expected.json` matches the recipe.
  Test: the lint is green; a deliberately wrong `analytic` value in a
  scratch fixture fails it.
- [ ] Step 8 — Property-test configuration. `arris_debug::prop`:
  `config()` reading `ARRIS_PROPTEST_CASES` (default 256) with a fixed
  seed source so a failure prints a reproducible seed; strategies
  `finite_f64(range)`, `unit_vec3()`, `rotation()` (uniform unit
  quaternion); the regressions policy in `tests/fixtures/README.md`.
  Test: a smoke property (`unit_vec3` has unit length to 1e-15) runs under
  the config; the seed printed on a forced failure reproduces it.

## Acceptance

`cargo test --workspace` green, including the doc-drift test (step 3) and
the corpus lint (step 7); `uv run --project tools/oracle selftest.py`
matches on every fixture directory; `tools/check-layers.sh` passes; `cargo
build --workspace --target wasm32-unknown-unknown` passes; CI green on
`main`; `target/inspect/cube-iso.png` rendered and read by the agent. Then
tag `m0` (the human's).

## Docs to update on completion

- `docs/03-roadmap.md` §M0 — status line: date, what was retired (the
  oracle round trip and the corpus lint), no ADRs.
- `docs/01-architecture.md` §Crates — confirm the dependency table matches
  `cargo metadata` (drift check); §Formats and tools — the oracle's real
  file names.
- `docs/02-data-model.md` §Invariants — any renumbering the drift test
  forced.
- `.agents/skills/inspect/SKILL.md` — remove the "Status: cycle-1
  deliverable" paragraph; every command in the table is real except the
  B-Rep ones, which say "M2/M3".
- `AGENTS.md` current state — "M0 done <date>; next `/plan m1-geometry`".
- `README.md` — workspace layout, one paragraph.

## Findings while working

- Step 1: `arris-debug` is a dev-dependency of the facade, not a
  dependency — 01-architecture says it never reaches a consumer, and a
  normal edge would ship `image` and `proptest` to every one. `proptest`
  is a `cfg(not(target_arch = "wasm32"))` dependency of `arris-debug`
  because its random source does not build for wasm; step 8's `prop`
  module follows the same cfg. `ops` also reserves `paranoid`, which
  01-architecture names beside the three the plan lists.

- Step 2: `arris-math` has a `serde` feature too (off by default; `arris-
  topo/serde` enables it) because `Precision` is part of the native format.
  The handles `Body`/`Shell`/`Face`/`Edge`/`Vertex` (01) and the entity
  structs of the same names (02) collided; the entities go in
  `arris_topo::entity` at M2 and 02 §Entities now says so. Extra public
  types beside the plan's list: `EntityKind`, `GeometryId`, `WrongKind`
  (the error of a typed `TryFrom<Shape>`).

- Step 3: `Violation` is `#[non_exhaustive]` with a fault sub-enum where a
  row lists several conditions (E2, E6, E7, L1, L4, F1, S2, B1, B3), so a
  row stays one variant. Payloads are the plan's best guess at what M2's
  checker will have in hand; M2 may change them and names the change.
  Extra public types: `Reference`, `Quantity`, `EdgeEnd`, `ToleranceBound`
  and the fault enums. The Euler line is not on `Report` yet — it needs a
  `Model` (M2).

- Step 4: `TriMesh` positions are `[f64; 3]`, not a math point type — a
  mesh is a buffer a consumer uploads, and `arris-math` has no `Point3`
  until M1. Indices are validated on entry (`MeshError`) so the
  measurements never panic; `signed_volume` is `Option`, `None` for a mesh
  that is not closed. `Aabb` lives in `arris-mesh` for now; if M1 wants a
  bounding box in `arris-math`, mesh re-exports it (design delta then).
  `arris-mesh` depends on `arris-topo` directly for the ids, as well as
  on `arris-check`.

- Step 5: `render` (to a `Raster`) is split from `render_png` (to a file)
  so tests count pixels without touching the filesystem; `Highlight` has
  `Face`, `Edge` and `Point` (a probe point is the thing most worth
  pointing at). Lines get a small depth bias toward the viewer so an edge
  on a face wins the z-test; shading is quantised to 32 levels so the two
  triangles of one planar face never differ by a rounding bit.

- Step 6: recipes carry `params` (numbers) usable as string expressions
  in any numeric field, and `variants` overriding them, so
  `provenance/bolt-pattern-rebuild` is one recipe with three variants and
  the eight bolt holes are `50 + R * cos(radians(45 * k))`, not eight
  hand-computed floats. `expected.json` therefore holds `results` keyed by
  variant (`default` always). The "Euler line" check is: the genus the
  oracle derives from its counts (`S − (V − E + 2F − L) / 2`) equals the
  fixture's `analytic.genus` — with the genus taken from the counts alone
  the line would be zero by construction. Arcs in profiles are three-point
  (`arc_to` + `via`), which has no orientation convention to mismatch.
  Recipes-not-STEP and the `cadquery-ocp==8.0.1.*` pin (resolved
  2026-09-05 as 8.0.1.0.0) stand as the plan assumed; no ADR.

## Open questions

- `⚠ OPEN:` fixture operands as **recipes evaluated by both sides** (the
  plan's assumption, 03-roadmap §Fixtures) or as STEP files written by
  Open CASCADE. Recipes keep the corpus independent of a STEP reader,
  make an operand change a one-line diff, and are cross-checked by the
  `analytic` closed forms; the cost is a recipe interpreter that must
  mirror `arris-ops` conventions (profile orientation, axis, angle sign) —
  which the closed forms also catch. Human decides, by step 6; a
  different answer is ADR-0001.
- `⚠ OPEN:` the `cadquery-ocp` pin. 8.0.1 resolves for Python 3.12 today;
  the oracle records the OCCT version in every `expected.json`, so a later
  pin change that moves a number is a `fixtures:` commit. Human confirms
  the pin at step 6 or names another.
- `⚠ OPEN:` whether the corpus lint lives in `crates/arris/tests/` (the
  facade, so it can use every crate) or in `arris-debug`'s own tests.
  Agent decides at step 7; the facade is the assumption.
