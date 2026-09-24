# Plan: measuring-harness

- Started: 2026-09-24
- Milestone: beside C4 (docs/ROADMAP.md §Beside the cycles, "The measuring
  harness"; ADR-0020 §2 and its first follow-up)
- Idea (verbatim from the human): "for the measuring harness, alongside C4"
- Related: `docs/ideas/reader-cycle-scope.md` (open) decision 4 — the
  oracle cache first, benchmarks beside the cycles. This plan takes that
  order. The idea stays open for the reader's own scope.

## Goal

Four things are true when this plan is done. The oracle is run only when
something it reads has changed: the corpus runner, the STEP, STL and
scratch-fixture seams answer from a cache whose key is every input the
oracle reads, and a warm `cargo nextest run -p arris --test corpus` starts
no Python. Random *recipes*, drawn from a seeded `proptest` strategy over
`Step`, run through both kernels and every outcome is sorted into a class.
A disagreement or a panic fails the run, shrunk to a recipe. A typed
refusal is counted per `Reason`. That count uses the same table the C4
histogram will read.
A nightly workflow runs the property tier above CI's case count on a new
seed each night, the differential run at a large count, the benchmarks and
the fuzz targets. The benchmarks time every corpus fixture's build and its
tessellation, so a change that costs 10× shows up as a ratio. The fuzz
targets over the intersectors live in a crate outside the workspace, seeded
from the geometry fixtures. The first numbers from each (cold and warm corpus
wall clock, cases per night, recipes per night, benchmark totals) are
recorded in the roadmap's harness line.

## Non-goals

- **The STEP reader's fuzz target.** The parser does not exist yet. That
  target is a step of the Part 21 parser plan, built on this plan's `fuzz/`
  crate and seeded from the STEP files the corpus writes. This plan does not
  hold a slot waiting for it.
- **Fixing what the harness finds** beyond what step 5 names. A
  disagreement becomes a regression fixture under the kernel rule. Its fix
  waits for its own plan, unless it is a test-side bound.
- **A benchmark gate in the pre-commit hook or in `ci.yml`.** Shared runners
  vary too much to gate on time. Benchmarks report ratios; the nightly run
  flags them, and nothing blocks a commit on them.
- **Changing the pre-commit hook's case count.** It stays at 256; ADR-0024
  gives the reason.
- **Performance work.** The harness measures. The breadth-and-speed cycle
  acts on what it measures.
- **New recipe operations.** The generator draws from the eleven that both
  interpreters already carry. It adds nothing to either.

## Design deltas

- **ADR-0024 — the measuring harness** (step 1). It records:
  - The oracle cache key: sha256 over the script's name, the bytes of
    every file it reads (the STEP or STL text, `fixture.json`,
    `expected.json`), the variant, and a digest of the oracle itself
    (`tools/oracle/**/*.py`, `pyproject.toml`, `uv.lock`). Only answers
    the kernel treats as settled are cached: a `MATCH` table, a scratch
    `expected.json`, an STL reading. A mismatch or an environment error
    always runs again.
  - The cache lives in `target/oracle-cache/`. `ARRIS_ORACLE_CACHE=off`
    bypasses it, and `ci.yml` and the nightly both set it. CI therefore
    never trusts a cache, and the cache is only a local speed-up.
  - Each tier and where it runs:
    - the pre-commit hook: 256 cases, the fixed seed;
    - CI: 1000 cases, the fixed seed;
    - nightly: a count above CI's on a seed rotated from the date and
      printed, plus the differential at its large count, benchmarks and
      fuzz.
    
    This closes the backlog question about the hook's count. The hook keeps
    256 so that one commit per step stays cheap, and depth comes from the
    nightly's new seeds, which add up across nights.
  - The differential's outcome classes and which of them fail the run.
  - The benchmarks use an in-house timer in `arris-debug` rather than a
    new dependency. The goal is to catch 10×, not 5%.
  - The fuzz crate sits outside the workspace. It uses the nightly
    toolchain, `libfuzzer-sys` and `arbitrary`, none of them workspace
    dependencies, and it is unpublished. The `#![forbid(unsafe_code)]` rule
    covers the kernel's crates, which this crate is not.
- **`arris-debug::oracle`** (dev-facing, unpublished; no public kernel type
  changes):
  - `compare`, `compare_dir`, `compare_stl` and `scratch_fixture` answer
    through a new `oracle::cache`.
  - `oracle::spawns()` returns the count of `uv` processes started, the
    number the tests assert on.
  - `oracle::expected_batch(dirs)` runs one `expected.py` over many
    scratch fixtures.
- **`arris-debug::prop::recipe`** — a new strategy producing `Recipe`s.
  **`arris-debug::differential`** — the runner, its `Outcome` classes, the
  histogram and the named exclusions. **`arris-debug::bench`** — the timer
  and its JSON report.
- **The corpus runner's stages** become callable one at a time: counts,
  measure, mesh and classify. The differential then holds a generated
  recipe to the same stages without a dump or a committed `expected.json`.
  This is the "one function per stage" split of `debug/corpus.rs` that the
  backlog names, done here because this plan is the next one to touch it.
- **New files:**
  - `crates/arris/tests/differential.rs`
  - `crates/arris/benches/corpus.rs` (`harness = false`)
  - `fuzz/` (excluded from `[workspace]`)
  - `.github/workflows/nightly.yml`
- **The workspace manifest:** `exclude = ["fuzz"]`, plus a `[[bench]]`
  entry in `crates/arris/Cargo.toml`. There are no new `[workspace.dependencies]`.
- No `⚠ OPEN:` in a design doc is closed by this plan.

## Steps

The work order is set by what C4 needs first and by risk. The cache is a
prerequisite: C4's corpus runs need it, and the differential shrinker
re-evaluates recipes it has already seen. The differential is the unproven
part, so it comes straight after the cache. Nightly, benchmarks and fuzz
are wiring on top of pieces that already exist.

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness or
bound has to be established here.

- [x] Step 1 **[1]** — **ADR-0024**, the measuring harness, covering the
  decisions in *Design deltas*. It includes the hook-count decision, which
  removes its line from `docs/BACKLOG.md` in the same commit.
- [x] Step 2 **[2]** — **The oracle cache.**
  - What it adds: `oracle::cache` with the key from ADR-0024, the oracle
    digest computed once per process, the `spawns()` counter, and
    `ARRIS_ORACLE_CACHE=off`, which `ci.yml`'s `test` and `parallel` jobs
    set.
  - Tests: a unit test against a scratch oracle tree shows that each input
    (a STEP byte, the variant, `expected.json`, one `.py` file, `uv.lock`)
    misses on its own. A second `corpus::run` of one fixture in the same
    process spawns nothing. A `Mismatch` is not cached.
  - Measured: `tools/test-timings.sh` on the corpus binary, cold and warm,
    with both numbers in the commit body.
- [x] Step 3 **[2]** — **The recipe strategy** (`prop::recipe`). It draws
  from the eleven operations:
  - boxes, cylinders and `profile` → `extrude`/`revolve` from the existing
    profile strategies;
  - poses from the existing body strategies, as `transform` steps;
  - one to three of `fuse`/`common`/`cut`;
  - `fillet`/`chamfer` on an edge of a primitive, before any boolean, where
    the edge's point is known in closed form.
  
  Probes: the operands' centroids, and points just in and out along each
  primitive's axes. Tolerances are the fixture defaults.
  
  Test: every drawn recipe survives the serde round trip, passes the
  fixture loader, hashes identically to `recipe_hash` on the Python side
  for one sample, and builds in Arris to `Ok` or a typed error — never
  a malformed recipe's error — at the hook's case count, its probes
  classified against each operand as labelled.
  
  *Found at step 3:* the draw reaches kernel panics at the hook's count —
  the debug build's checker guard, L4 — so the test lets a panic pass as
  not the draw's fault, and "no panic" is held by steps 4–5, which were
  always where a `Panic` or `CheckerViolation` fails the run. The first
  two are shrunk to `regression/tilted-cylinder-slot-cut` and
  `pin-at-disc-rim-common-fuse`.
- [x] Step 4 **[3]** — **The differential runner.**
  - Flow: N seeded recipes are written as scratch fixtures, and one
    `expected_batch` call answers all of them. Each recipe then runs
    through the corpus stages.
  - Outcome classes: `Agree`, `BothRefuse`, `ArrisRefuses(Reason)`,
    `OracleRefuses`, `Disagree(stage)`, `CheckerViolation`, `Panic`
    (caught with `catch_unwind`, which is the test side's job and never
    the kernel's).
  - The last three fail the run. The first four are counted, and the
    histogram prints per class and per `Reason`.
  - A failing case is shrunk through its `ValueTree`, with the oracle run
    per candidate through the cache. The run prints the shrunk recipe as
    `fixture.json`, ready to commit under `regression/`.
  - `crates/arris/tests/differential.rs` runs `ARRIS_DIFF_CASES` recipes,
    default 32. The number that settles "unproven" is written in the
    commit body: the share of the draw that reaches a comparison, not a
    refusal, and the time per recipe.

  *Found at step 4:* the fixed seed's first 32 draws hold seven
  `Disagree(measure)`, so `differential.rs` lands `#[ignore]`d and step 5
  takes the ignore off. An `OpError::Internal` is typed and never a
  panic, so it counts under `ArrisRefuses` as `Internal(<fault>)`, where
  the histogram shows it. Step 5 decides whether it fails the run.
- [x] Step 5a **[2]** — **`mass_properties` about the body, not the
  origin** (split out of step 5, which found it). The volume and first
  moments are integrated about the centre of the body's vertices. Faces
  whose pcurves of one section are fitted apart close only to the fit,
  and each gap leaks flux in proportion to its distance from the point
  the integral is taken about.
  - Test: a tilted cylinder 110 from the origin under an oblique plane
    measures to `π r² t` within 1e-11. Before the fix it was 4.9e-9 off.
  - `boolean_prop`'s `cut_then_fuse_of_a_cylinder_across_a_small_box`
    now holds to `REL`. It had pinned the leak as a fit difference.
- [x] Step 5b **[3]** — **The differential's first findings.**
  - Run at CI's count (1000) on the fixed seed.
  - Every `Disagree`, `CheckerViolation` or `Panic` class is shrunk into
    `tests/fixtures/regression/<slug>/` with its oracle value and the
    *desired* assertion, and `#[ignore]`d.
  - A test-side cause, such as the generator drawing a pose the recipe
    grammar cannot state, is fixed in the generator.
  - A kernel cause the draw keeps hitting becomes a named exclusion in the
    generator. The exclusion carries the regression fixture's slug, and the
    corpus lint fails an exclusion whose fixture is gone or has moved out
    of `regression/`, so the exclusion is lifted by the commit that fixes
    it.
  - `differential.rs` is green at the fixed seed and CI's count, its
    `#[ignore]` off, and the histogram is in the commit body.
- [x] Step 6 **[2]** — **The known high-count property failures.** These
  are the backlog's two properties that fail at 3000 cases:
  - `random_plane_and_cylinder_agree_on_every_common_property`: the bound
    scales with the point's magnitude.
  - `coaxial_pairs_meet_where_their_meridians_meet`: the apex is sampled.
    If the near-flat cone's missing circle turns out to be the kernel's,
    it is shrunk to a regression fixture, and the pose is excluded by name
    as in step 5.
  
  Then `tools/test-timings.sh` runs the whole suite at the nightly count
  chosen in step 7 on the fixed seed. It is green, or every failure is
  handled the same way. The two backlog lines are removed.

  *Found at step 6:* the whole suite was held green under `cargo nextest
  run --workspace` at 5000 rather than through `tools/test-timings.sh`,
  whose first pass runs each binary alone and kept seven percent of the
  machine busy for hours; nextest's per-test durations stand in for its
  per-binary column. Eight failures, in the order each property's first
  one hid the next — see *Found while executing*.
- [ ] Step 7 **[1]** — **The nightly workflow** (`nightly.yml`, on a
  schedule and on `workflow_dispatch`).
  - Seed: `sha256(date)`, printed as `ARRIS_PROPTEST_SEED=…`.
  - Case count: the largest multiple of CI's that keeps each job under the
    runner's six hours. It is measured from step 6's timings, and the job
    matrix is split by crate where one job cannot fit.
  - Also run: the corpus's ignored tests, as in `ci.yml`; the differential
    at `ARRIS_DIFF_CASES=1000` on the same seed; and
    `ARRIS_ORACLE_CACHE=off` throughout.
  - Proof: one `workflow_dispatch` run is green. The human pushes and
    triggers it, and its duration goes in the roadmap line at retirement.

  *Landed, box open:* the workflow is committed at 5000 cases in six
  matrix jobs, estimated from step 6's timings at 2 h 20 min at most on a
  4-core runner; the six filtersets partition the suite's 1238 tests
  exactly (`cargo nextest list`). The box is ticked by the human's green
  `workflow_dispatch` run, which no local check stands in for.
- [x] Step 8 **[2]** — **Benchmarks.**
  - `arris_debug::bench`: a warm-up, a fixed iteration count, the median
    and median absolute deviation, and a JSON report. It is unit-tested on
    a synthetic clock.
  - `crates/arris/benches/corpus.rs`: for each passing fixture under
    `boolean/`, `sweep/` and `blend/`, the recipe build and the
    tessellation at its `mesh_chord`, timed apart.
  - Flags: `--save <file>` writes a report. `--compare <file>` prints each
    case's ratio and flags those past a named `RATIO_FLAG` (3×, with a
    comment on why 3× and not 10×).
  - The reference machine's totals go in the commit body. `cargo bench
    --no-run` is already covered by the hook's `clippy --all-targets`.

  *Found at step 8:* `cargo bench` runs the binary in the crate's
  directory, so `--save` and `--compare` take a relative path from the
  workspace root. On the reference machine: 250 cases from 125 fixtures,
  build 1.60 s and mesh 0.73 s, 2.33 s in all; a second run against the
  first is at most 1.13× on any case. Thirteen fixtures are the corpus's
  designed refusals and are skipped by name.
- [ ] Step 9 **[1]** — **Benchmarks nightly.**
  - The nightly job runs the benches, uploads the report as an artifact,
    and compares it against the previous successful night's artifact,
    flagged in the job summary, never failing.
  - `tools/bench-compare.sh` does the same locally against a report under
    `target/`.
- [ ] Step 10 **[2]** — **Fuzz targets over the intersectors.**
  - `fuzz/` with `intersect_surfaces`, `intersect_curve_surface` and
    `intersect_curves`. Each decodes analytic kinds in a pose from
    `arbitrary` bytes, including the fitted `Nurbs` curves the sections
    make.
  - Each target asserts: no panic; every hit lies on both operands within
    their tolerances; the same input gives the same output twice.
  - `fuzz/seed.rs` writes a seed corpus from every `tests/fixtures/geom/`
    operand pair.
  - `cargo +nightly fuzz build` is added to the nightly workflow. The
    harness's own test is a 60-second run of each target.
- [ ] Step 11 **[2]** — **Fuzzing's first findings.**
  - Run each target for one hour locally, then each night for a fixed
    time, with the corpus kept as a workflow cache.
  - Every panic or off-surface hit is shrunk into a `geometry` regression
    fixture under `tests/fixtures/regression/` with its oracle values, and
    `#[ignore]`d.
  - The hour's executions per second and findings go in the commit body.
- [ ] Step 12 **[1]** — **Retire.** The docs list below. The roadmap's
  harness line holds every number the steps recorded.

## Acceptance

Every check below has to pass:

- `cargo nextest run --workspace` is green at the fixed seed and CI's
  count with `ARRIS_ORACLE_CACHE=off`. This includes `differential.rs` at
  1000 recipes, with no `Disagree`, `CheckerViolation` or `Panic` outside a
  named exclusion, and every exclusion's regression fixture present.
- A warm run of `cargo nextest run -p arris --test corpus` reports
  `oracle::spawns() == 0`. Its cold and warm wall clock are recorded.
- One nightly workflow run is green end to end: the property tier at its
  count on a date seed, the differential at 1000, the benchmarks and every
  fuzz target.
- `cargo bench -p arris --bench corpus -- --compare` runs against a saved
  report and prints a ratio for every case.
- The corpus lint passes with the exclusion rule.

## Docs to update on completion

- `docs/ROADMAP.md` §Beside the cycles, "The measuring harness": now in
  present tense, with its numbers:
  - corpus wall clock, cold and warm;
  - the nightly count and seed rule;
  - recipes per night and the differential's histogram;
  - benchmark totals on the reference machine;
  - fuzz executions per second.
  
  The open question about the hook's count is gone, answered by ADR-0024.
  The line says that the STEP reader's fuzz target lands with the parser
  plan.
- `docs/ARCHITECTURE.md` §Formats and tools:
  - the oracle seam's cache and `ARRIS_ORACLE_CACHE`;
  - `arris-debug`'s `differential`, `prop::recipe` and `bench`;
  - the `fuzz/` crate outside the workspace;
  - the benches;
  - `nightly.yml` beside `tools/test-timings.sh`.
  
  The §Crates table's `arris-debug` row gains the new modules.
- `tools/oracle/README.md`: the cache, how its key covers the scripts, and
  batch use of `expected.py`.
- `tests/fixtures/README.md` §Property-test failures: a differential or a
  fuzz finding becomes a fixture the same way, and a named exclusion
  cites it.
- `docs/BACKLOG.md`: remove the hook-count line (step 1), the two
  3000-case lines (step 6), and the `debug/corpus.rs` clause of the
  file-split line (step 4). Add one line per finding that is left
  unfixed.
- `AGENTS.md` setup: `cargo install cargo-fuzz` and a nightly toolchain,
  named as optional and needed only for `fuzz/`.
- `.agents/skills/inspect`: shrinking a differential or fuzz failure into a
  fixture.

## Open questions

None are left for the human. The charter delegates them, so each is decided
here and recorded by step 1's ADR:

- The hook's case count stays at 256. Depth comes from the nightly's rotating
  seed (ADR-0024).
- The benchmarks use an in-house timer, not a new dependency. Time is never a
  CI gate.
- The fuzz crate sits outside the workspace on the nightly toolchain. It is
  unpublished and not bound by the kernel crates' `forbid(unsafe_code)`.
- A nightly failure shows as a red run and nothing else. It opens no issue,
  because anything posted outward is the human's call.

Found while executing:

- Step 3's property could not also assert "no kernel panic": the first
  draw at 256 cases meets the checker's L4 guard. The assertion lives in
  the differential (steps 4–5); step 3 holds the generator only.
- The oracle refuses a draw that cuts an elliptic extrusion with a curved
  trim (the backlog's surface-of-extrusion line): a steady source of
  `OracleRefuses`, and why step 3's hash sample takes the first draw the
  oracle builds.
- Step 4: the first 32 draws of the fixed seed give seven
  `Disagree(measure)`. Each is a volume, centroid or inertia term 1e-9 to
  7e-8 apart, relative, on a posed cylinder (with a box, a fillet or a
  polygon extrusion) or on an elliptic extrusion — above the fixtures'
  default 1e-9. Which side is off is step 5's question, answered against
  a closed form where one exists. It is not answered by widening the
  default tolerance.
  At 256 draws (shrink off): 121 `Agree`, 21 `BothRefuse`, 37
  `OracleRefuses`, 6 `ArrisRefuses` (4 `Degenerate(Empty)`, one
  `Internal(Geometry)` and one `Internal(Split)`), and 71 `Disagree` (67
  measure, 3 counts, 1 mesh). There was no panic on this seed.
- Step 5, at CI's count (1000) on the fixed seed, shrink off: 530
  `Agree`, 69 `BothRefuse`, 137 `OracleRefuses`, 20 `ArrisRefuses`, 9
  `CheckerViolation` and 235 `Disagree` (222 measure, 11 counts, 2 mesh).
  The step-4 question, which side is off, has one answer for almost all
  of the measure cases: Arris. Case 172 is a posed cylinder in common
  with a box, and its closed form puts Open CASCADE 2.5e-10 off and Arris
  1.4e-9 off. The error came from `mass_properties` taking its first pass
  about the world origin. Taken about the body's own centroid, the
  same model is 1e-12 off, and cases 345 and 553 go from 3e-4 and 4.2e-7
  to 7e-7 and 9e-9. Before the fix, 220 of the 222 were already inside
  `(tol_arris + tol_occ) · A / V`, the most that the two shapes'
  tolerances allow. A tolerance-derived bound would have passed them and
  left the leak in place, so step 5 was split. Step 5a fixes the
  integral, and step 5b sorts what the differential still finds after it.
- Step 5b: Arris rebuilt at tolerances 1e-9 and 1e-10 sorted the 203
  measure disagreements left after 5a. In 147 cases Arris converges to
  1e-12 and the oracle stays away. A closed form confirms the oracle is
  off there: on case 431 Arris is 3e-16 off and Open CASCADE 1.1e-9. In
  54 cases the difference was Arris's own 1e-7 fit, and 2 did not build
  at the tighter tolerance. Open CASCADE's boolean output declares 1e-7
  to 4e-3. The differential therefore holds each case to the first-order
  bound for a boundary known to `t_arris + t_occ`, which is the bound
  `within_own_tolerance` already puts on the oracle's STEP round trip.
  The mesh's volume gets the same bound over its chord.
  Nine of the eleven counts disagreements were vertices that only split
  an edge. Open CASCADE keeps one where a section crossed an old seam, so
  counts are compared net of them. `OpError::Internal` now fails the run.
  An exclusion is a predicate over the failing outcome, because a recipe
  cannot state a kernel bug. The ADR-0024 amendment records all four.
- Step 5b at 1000 draws of the fixed seed, shrink off: 762 `Agree`, 67
  `BothRefuse`, 137 `OracleRefuses`, 14 `ArrisRefuses` (all
  `Degenerate(Empty)`), and 20 `Excluded`:
  - 8 L4, a hole loop outside every outer loop;
  - 7 `Internal(Split)`;
  - 1 `Internal(Geometry)`;
  - 1 L5, a loop crossing itself;
  - 1 missing cavity shell;
  - 1 case with extra faces;
  - 1 unclosed mesh.

  There are no failures. 762 of 1000 reach a comparison (76%). The run
  takes 265 s cold, 0.26 s per recipe of wall clock, and Arris 1.7 s
  per recipe on one thread. There are eight new regression fixtures,
  including two faults the shrinker drifted into (`Internal(Lumps)` and
  `Internal(Builder)`). A shrink now keeps the checker row and the fault
  name it started from.
- Step 4: `OpError::Internal` is a caught kernel bug, but it is typed.
  It counts as `ArrisRefuses/Internal(<fault>)` for now. Step 5 decides
  whether that class fails the run, which would be an ADR-0024 amendment.
  Step 5b decided that it does.
- Step 4: the differential skips the STEP round trip and the dump. The
  first reads a committed directory and would start one `compare.py` per
  recipe, and the second has no committed dump to diff. The corpus holds
  both for every fixture that a finding becomes.

- Step 6, at 5000 cases on the fixed seed, the nightly count (step 7).
  The two backlog properties were test-side: the plane–cylinder ellipse
  is held to rounding relative to its distance from the origin (4.5e5
  out, one ulp is past the absolute 1e-10), and the coaxial meridians are
  sampled at a cone's apex, since a chord across the V's kink misses a
  crossing beside it. Three kernel faults were fixed where they were
  found:
  - the `parallel` passes of the boolean and the mesh returned whichever
    error a `rayon` thread met first, so `tolerance_band` changed class
    by the schedule, and CI's `parallel` job had been red since fda163d
    (its own commit, 443719b);
  - `TriMesh::signed_volume` summed about the world origin and lost 9e-9
    of a cylinder of radius 0.1 about 120 out, the leak step 5a closed in
    `mass_properties`; it now sums about the mesh's box centre;
  - a fillet corner's squareness was read off directions from the ball's
    centre to points 95 out, 2e-12 off the faces' normals for a ball of
    0.012, past the angular tolerance: a square corner of a posed L was
    refused as `VertexBlend`. It reads the planes' normals now.

  Four are kernel findings, each an ignored regression beside its
  property and a named exclusion in it, and each a backlog line:
  - a touch within tolerance of a box face's rim — the corner on the
    wall, or a cap's rim grazing the edge — rebuilds those entities under
    new ids (`touch_at_the_face_rim`);
  - a cylinder whose seam is the tangent ruling on a face fails the cut
    with `Fault::Split` (`seam_on_the_touch`,
    `regression/tangent-seam-on-face-cut`);
  - `quadric_operands` reaches the L4 fault in eleven of sixteen shards,
    held to the differential's own `hole-loop-outside-every-outer-loop`
    through `differential::exclusion_of_panic`, so one fix lifts both;
  - `Provenance::then` breaks ADR-0009's nesting across a composition: a
    later record generating from a piece of a split lands before or
    after the other pieces' outputs by the bracketing, since the flat
    generated list no longer says which piece it came through. The
    exclusion is a predicate over the failure, as step 5b's are: the two
    bracketings differ only in a generated list's order. The fix is a
    representation change and an ADR-0009 amendment, a plan of its own.

  The suite at 5000: 99,600 CPU-seconds, 57 minutes of `cargo nextest
  run --workspace` on the reference machine's 32 cores. `boolean_prop` is
  72% of it: `quartic_cylinders` 19,000, `quadric_operands` 16,200,
  `crossing_cylinders` 15,300 and the rest of it 21,200; the rest of
  `arris-ops` 13,200 and every other crate together 14,600. At 1000 it
  was 22,300 CPU-seconds, and CI's `test` job 2 h 21 min on a 4-core
  runner, which is 0.385 runner-seconds per CPU-second here.
