# Plan: test-parallelism

- Started: 2026-09-11
- Milestone: none — harness work between M4 and M5 (cycle C1,
  `docs/ROADMAP.md`). No roadmap line: it changes no kernel behaviour.
- Idea (verbatim from the human): "I noticed that tests were running quite
  long and loading single CPU core when I looked. Can we in future
  parallelize those tests somehow or it is fundamentally impossible?" …
  "/plan it now, no need for memory and next session" … "would be nice to
  measuure how much speedup we get from this plan implementation - add
  this comparison one of the steps in plan". No idea file: the two levers
  are mechanical and nothing about them is contested.

## Goal

The workspace test suite uses the machine it runs on. A property test's
cases are split across k seeded shards, each its own `#[test]`, so
libtest's thread pool runs them concurrently instead of one property
sitting on one thread; and the suite runs under `cargo nextest`, so the
53 test binaries overlap instead of being run one after another. Nothing
about what a property asserts changes, no property loses a case, and
every run stays reproducible from a printed seed. The plan opens and
closes with the same measurement, run by the same committed script, so
the speedup is a recorded number and not a claim.

## Non-goals

- **Per-case cost.** Making a single boolean cheaper is the S5 backlog
  line (the checker's super-linear coincident arm), and relaxing
  "the checker runs after every operation in debug builds"
  (`.agents/rules/kernel.md`) for property runs would need an `/idea`
  first. This plan makes the same work fit on more cores; it does not
  make the work smaller.
- **Coverage.** A property that ran 1000 cases still runs 1000 cases.
  Different draws, same count — never fewer.
- **A benchmark harness.** `divan`/`criterion` for tessellation and the
  boolean corpus stays its own backlog line. `tools/test-timings.sh` here
  is a wall-clock table for this plan's before/after, not a benchmark.
- **The `parallel` feature.** Recorded as a finding, not touched: rayon
  inside `ops` does not help a test run and may hurt, because it contends
  with the libtest threads already running the other properties. CI keeps
  its `--features parallel` jobs — they prove byte-identical output, which
  is a correctness claim, not a speed one.

## Measured baseline (2026-09-11, 32-core machine)

`tools/test-timings.sh` (step 1), the *before* that step 5 re-runs. Every
row is one test binary run alone; the serial total is therefore what
`cargo test --workspace` costs, since cargo runs the binaries one after
another.

`ARRIS_PROPTEST_CASES=256`, 32 cores, 2026-09-11.

| Target, run alone | Seconds |
|---|---|
| `arris-ops::boolean_prop` | 438.29 |
| `arris-mesh::tessellate` | 10.79 |
| `arris::corpus` | 2.22 |
| `arris-debug::builder` | 1.07 |
| `arris::provenance` | 1.05 |
| `arris-ops::primitives` | 1.03 |
| `arris-ops::boolean` | 0.96 |
| `arris-ops::transform` | 0.64 |
| `arris-io::step` | 0.59 |
| `arris-ops::measure` | 0.36 |
| `arris-check::classify` | 0.29 |
| `arris-geom::curve2` | 0.21 |
| `arris-debug::arris_debug` | 0.11 |
| *40 further targets, each under 0.10* | 0.43 |
| **serial total** — what `cargo test --workspace` costs | **458.04** |
| `cargo test --workspace --doc` | 2.91 |
| **`cargo nextest run --workspace`, wall clock** | **445.68** |

`ARRIS_PROPTEST_CASES=1000`, 32 cores, 2026-09-11.

| Target, run alone | Seconds |
|---|---|
| `arris-ops::boolean_prop` | 1624.35 |
| `arris-mesh::tessellate` | 41.15 |
| `arris-ops::boolean` | 3.59 |
| `arris-ops::transform` | 2.58 |
| `arris::corpus` | 2.21 |
| `arris-ops::measure` | 1.39 |
| `arris-debug::builder` | 1.12 |
| `arris::provenance` | 1.12 |
| `arris-check::classify` | 1.11 |
| `arris-ops::primitives` | 1.02 |
| `arris-geom::curve2` | 0.81 |
| `arris-io::step` | 0.57 |
| `arris-debug::arris_debug` | 0.42 |
| `arris-geom::bounds` | 0.35 |
| `arris-geom::pcurve` | 0.24 |
| `arris-geom::intersect_curve_surface` | 0.14 |
| *37 further targets, each under 0.10* | 0.50 |
| **serial total** — what `cargo test --workspace` costs | **1682.67** |
| `cargo test --workspace --doc` | 3.15 |
| **`cargo nextest run --workspace`, wall clock** | **1656.26** |

1624.35/438.29 = 3.71 against 1000/256 = 3.9: **cost is linear in the case
count**, with no fixed overhead to amortise.

Two findings set the rest of the plan.

**One binary is the suite.** `boolean_prop` is 438 of the 458 seconds at
256 cases and 1624 of 1683 at 1000. `arris-mesh::tessellate` is the only
other target above four seconds at either count, and the remaining 51
together cost nine seconds at 256 and seventeen at 1000. Sharding
`boolean_prop` is the whole job — and the floor it leaves is `tessellate`,
which step 3 shards too if `boolean_prop`'s shards come in under it.

**nextest alone buys nothing.** 445.68 s against a 458.04 s serial total at
256 cases, and 1656.26 against 1682.67 at 1000: 3 % and 2 %. The two levers
are not independent, and not in the order the plan assumed — overlapping
the binaries cannot help while one binary holds a single property that
runs longer than every other binary put together. Step 4 is worth doing
only because step 3 comes first, and step 5's ratio will be almost
entirely sharding's.

Per property, at 64 cases each, run one at a time:

| Property (`crates/arris-ops/tests/boolean_prop.rs`) | 64 cases |
|---|---|
| `cut_then_fuse_restores_the_union_at_random_poses` | 111.7 s |
| `fuse_and_common_commute_at_random_poses` | 68.2 s |
| `overlapping_pairs_fuse_and_common_additively` | 45.4 s |
| `piercing_pairs_obey_every_identity_and_refuse_the_two_shells` | 13.2 s |
| serial total | 238.5 s |

libtest already runs those four concurrently, so the binary's wall clock
is the **slowest single property**, not the sum: 111.7 s × 4 = 447 s at
256 cases, which is the 438.29 s measured. That is the whole diagnosis —
`ps` showed ~250–280 % CPU (≈3 of 32 cores) and only `boolean_prop` hot
for the entire 27-minute stretch, because `proptest`'s `TestRunner::run`
is sequential within a test and `cargo test` runs one test binary at a
time.

The cost is heavily skewed: `cut_then_fuse` alone is 47 % of the serial
total and 100 % of the wall clock, so shard counts are chosen per
property, not uniformly.

## Result (step 5, same machine, same script)

`ARRIS_PROPTEST_CASES=256`, 32 cores, 2026-09-11.

| Target, run alone | Seconds |
|---|---|
| `arris-ops::boolean_prop` | 69.05 |
| `arris-mesh::tessellate` | 10.78 |
| `arris::corpus` | 2.24 |
| `arris-debug::builder` | 1.05 |
| `arris::provenance` | 1.04 |
| `arris-ops::primitives` | 0.97 |
| `arris-ops::boolean` | 0.94 |
| `arris-ops::transform` | 0.63 |
| `arris-io::step` | 0.59 |
| `arris-ops::measure` | 0.36 |
| `arris-check::classify` | 0.29 |
| `arris-geom::curve2` | 0.21 |
| `arris-debug::arris_debug` | 0.11 |
| *40 further targets, each under 0.10* | 0.43 |
| **serial total** — what `cargo test --workspace` costs | **88.69** |
| `cargo test --workspace --doc` | 2.86 |
| **`cargo nextest run --workspace`, wall clock** | **70.92** |

`ARRIS_PROPTEST_CASES=1000`, 32 cores, 2026-09-11.

| Target, run alone | Seconds |
|---|---|
| `arris-ops::boolean_prop` | 240.30 |
| `arris-mesh::tessellate` | 40.95 |
| `arris-ops::boolean` | 3.57 |
| `arris-ops::transform` | 2.48 |
| `arris::corpus` | 2.17 |
| `arris-ops::measure` | 1.37 |
| `arris-check::classify` | 1.11 |
| `arris-debug::builder` | 1.06 |
| `arris::provenance` | 1.04 |
| `arris-ops::primitives` | 0.98 |
| `arris-geom::curve2` | 0.80 |
| `arris-io::step` | 0.58 |
| `arris-debug::arris_debug` | 0.42 |
| `arris-geom::bounds` | 0.34 |
| `arris-geom::pcurve` | 0.24 |
| `arris-geom::intersect_curve_surface` | 0.14 |
| *37 further targets, each under 0.10* | 0.49 |
| **serial total** — what `cargo test --workspace` costs | **298.04** |
| `cargo test --workspace --doc` | 2.87 |
| **`cargo nextest run --workspace`, wall clock** | **254.13** |

| | Before | After | Ratio |
|---|---|---|---|
| `cargo nextest run --workspace`, 256 cases | 445.68 | 70.92 | **6.28×** |
| `cargo nextest run --workspace`, 1000 cases | 1656.26 | 254.13 | **6.52×** |
| serial total (`cargo test --workspace`), 256 | 458.04 | 88.69 | 5.16× |
| serial total (`cargo test --workspace`), 1000 | 1682.67 | 298.04 | 5.65× |
| `arris-ops::boolean_prop`, 256 | 438.29 | 69.05 | 6.35× |
| `arris-ops::boolean_prop`, 1000 | 1624.35 | 240.30 | 6.76× |
| `arris-mesh::tessellate`, 256 | 10.79 | 10.78 | 1.00× |

Every other target is unchanged, as it should be: nothing but
`boolean_prop` was touched.

**6.3× against the plan's ≥ 7×, and the shortfall is the machine, not the
design.** The prediction assumed 32 cores because `nproc` says 32. The
machine is a Ryzen 9 7950X: **16 physical cores**, two SMT threads each.
The ceiling for this suite is its total work over the cores that can
actually run it at once, and `boolean_prop` is essentially all of that
work: 994 s of it at 256 cases, so 994/16 = **62 s** is the floor no shard
count can go below. It came in at 69.05 s — 90 % of the bound — and the
whole suite at 70.92 s, because under nextest every binary's tests share
one pool and the other 52 binaries fill the gaps around `boolean_prop`'s
shards. Against a 62 s floor the best achievable ratio was 445.68/65 ≈
7.0×; 6.28× is 90 % of what the hardware allows.

The remaining 7 s over the floor is tail variance, not imbalance between
properties: per-case cost varies enough that the heaviest of
`cut_then_fuse`'s 30 shards took 51.97 s under full load against a 15.3 s
mean. Raising the shard counts would trim that tail, but it cannot buy
more than the 7 s to the floor, and it costs the other way —
`cases_per_shard` rounds up, so k=60 would run 300 cases where 256 were
asked for, a 17 % overshoot that adds more work than the tail costs. The
counts stay as step 3 set them.

The way past 62 s is therefore not more parallelism but cheaper cases —
the S5 backlog line on the checker's super-linear coincident arm, which
this plan names as a non-goal. That is the finding step 5 exists to
produce.

## Design deltas

- **`arris-debug` — `prop` gains a sharded entry point**
  (`docs/ARCHITECTURE.md` §Formats and tools; `crates/arris-debug/src/
  prop.rs`):
  - `pub fn check_shard<S, F>(shard: u32, shards: u32, strategy: S, test: F)`
    beside `check`: runs `cases().div_ceil(shards)` cases of `strategy`
    from a seed derived for this shard, so `shards` shards together run at
    least `cases()` cases and never fewer.
  - `pub fn shard_seed(base: &[u8; 32], shard: u32) -> [u8; 32]` —
    `sha256(base ‖ shard.to_le_bytes())`, `sha2` already being an
    `arris-debug` dependency. **The shard count is deliberately not in the
    derivation**: shard *i*'s case stream depends only on the base seed
    and *i*, so raising the shard count leaves every existing shard's
    stream a prefix of what it was and only adds new ones. Changing `k` is
    then not a silent re-roll of the corpus.
  - `pub fn try_check_shard(...) -> Result<(), String>` beside
    `try_check`, which is what the harness's own tests use.
  - The failure message gains the shard: `reproduce with
    ARRIS_PROPTEST_SEED=<base> ARRIS_PROPTEST_CASES=<total>` and the
    failing `#[test]`'s own name, which already carries the index. The
    base seed stays the thing a human copies — a shard is addressed by
    running its test name.
  - A `#[macro_export] macro_rules!` that writes one `#[test]` per shard
    over a body given once, so a property is not copy-pasted k times.
- **`tools/test-timings.sh`** (new; `docs/ARCHITECTURE.md` §Formats and
  tools): runs the suite and prints a per-binary wall-clock table sorted
  slowest first, plus the total. The same script produces this plan's
  "before" and "after", so the comparison is one command run twice and
  not two different measurements.
- **`.githooks/pre-commit` and `.github/workflows/ci.yml`**: `cargo test
  --workspace` becomes `cargo nextest run --workspace` plus `cargo test
  --workspace --doc`, because nextest does not run doctests. CI installs
  nextest; the `--features parallel` jobs move to nextest too.
- **`AGENTS.md` §Setup**: the one-line setup grows a second line,
  `cargo install cargo-nextest --locked`, since the hook now needs it.
- **`tests/fixtures/README.md` §Property-test failures**: a sharded
  property's failure names its shard, and the commit body of the fixture
  it becomes records the base seed, the case count *and* the shard.
- No ADR. Neither lever decides anything about geometry, and both are
  reversible in one commit.

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness
or bound has to be established here.

- [x] Step 1 **[1]** — **The measurement, committed.**
  `tools/test-timings.sh`: runs each test binary, times it, prints the
  table sorted slowest first and the total, and takes the case count from
  the environment as everything else does. Run it at 256 and at 1000 and
  paste both tables into this plan under "Measured baseline" as the
  *before*, replacing the partial table above with the script's own
  output — so the "after" of step 5 compares like with like. This step
  first because every later step's value is read off it, and because it
  says whether any binary besides `boolean_prop` is worth sharding.

- [x] Step 2 **[2]** — **`check_shard`, `shard_seed` and the macro**, with
  the harness's own tests beside the existing `try_check` ones: two
  shards of one base draw disjoint streams; shard *i* at `k = 16` is a
  prefix of shard *i* at `k = 8` (the derivation ignores `k`); `shards`
  shards run at least `cases()` cases in total; a failure in shard *j*
  names *j* and reproduces from the printed base seed; `cases()` and
  `seed()` are still honoured. The careful part is the prefix property
  and the case-count arithmetic — `div_ceil` so a remainder rounds up and
  coverage never drops below the configured count.

- [x] Step 3 **[1]** — **Shard `boolean_prop`'s four properties**, shard
  counts cost-proportional to step 1's table rather than uniform, since
  `cut_then_fuse` is 47 % of the serial cost: roughly `k` chosen so every
  shard costs about the same, with the total shard count near twice the
  core count so a loaded machine still packs well. Nothing about the
  properties' assertions changes; the diff is the macro wrapper. Anything
  else step 1's table shows above the resulting wall clock gets the same
  treatment in this step.

- [x] Step 4 **[2]** — **nextest in the hook and CI.** The careful part is
  that nextest runs every test in its own process at much higher
  concurrency: the corpus runner writes STEP under `target/inspect/`, and
  per-test processes make a name collision there likelier than the
  current thread-per-test does — that path is checked and made
  per-test-unique if it is not already. Also verify no test depended on
  sharing a process with another. `AGENTS.md` §Setup, the hook, CI (both
  the plain and the `--features parallel` jobs), and `cargo test --doc`
  as its own step. The stale `docs/plans/m3-tessellation.md` and
  `docs/plans/m4-booleans.md` references in CI's comments — both files
  retired and deleted — are corrected here while the file is open.

- [x] Step 5 **[1]** — **The after measurement and the comparison.**
  Re-run `tools/test-timings.sh` at 256 and 1000, put the two tables
  beside step 1's in this plan, and state the speedup as a ratio per
  binary and for the whole suite. This is what `/retire-plan` moves into
  `docs/ARCHITECTURE.md` §Formats and tools as one sentence with the
  number in it. If the ratio is materially below the ~7× the baseline
  predicts, the step's job is to say *why* — that finding is the
  deliverable, not a passing box.

## Acceptance

`cargo nextest run --workspace` and `cargo test --workspace --doc` green,
and the same at `ARRIS_PROPTEST_CASES=1000`; every one of the 30 corpus
fixtures still passing every stage with the three `sweep/*` ignored; the
`--features parallel` runs still byte-identical; `tools/check-layers.sh`,
its self-test, `cargo fmt --check`, `cargo clippy -D warnings`, `cargo
doc -D warnings` and the wasm build green; the oracle self-test green.

The plan's own number: `tools/test-timings.sh` at 256 cases, before and
after, on the same machine, recorded in the plan. The baseline predicts
the suite's ~460 s falling to under 60 s (bounded by the largest shard,
not by any property), i.e. **≥ 7×**. That target is a recorded
comparison, not a CI gate — a 4-core laptop will not see it, and no test
may fail because a machine is small.

The harness invariants, which *are* gates: total cases per property
unchanged at every shard count; two runs of the suite give identical
results; a deliberately failing shard prints a base seed that reproduces
it.

## Docs to update on completion

**Agent note (2026-09-11):** the first three were done as the code landed,
not left for retirement — `.agents/rules/git.md` has docs change in the
same commit as the behaviour, and by step 4 all three described a runner
the code no longer matched. `/retire-plan` has the roadmap status line and
deleting this file left to do; check the three below rather than assume
they are pending.

- `docs/ARCHITECTURE.md` §Formats and tools — `prop` described as a
  sharded seeded runner (`check`, `check_shard`, the macro, the
  shard-seed derivation and its prefix property); `tools/test-timings.sh`
  named beside the oracle and the corpus runner; one sentence recording
  the measured speedup from step 5.
- `AGENTS.md` §Setup — `cargo install cargo-nextest --locked` beside the
  `git config core.hooksPath` line. "Current state" is untouched: no
  milestone moved.
- `tests/fixtures/README.md` §Property-test failures — the shard in the
  reproduce recipe and in the fixture's commit body.
- `.agents/rules/kernel.md` §Testing — one clause that a property may be
  sharded and that each shard is seeded, so "those are seeded" still
  reads true of the sharded form.
- `docs/BACKLOG.md` — nothing added if the plan lands whole; if step 4
  leaves nextest optional (`⚠ OPEN` 3), the line saying so.

## Open questions

- **Resolved (step 2) — the macro's shape.** Neither option as written.
  A literal list `[0 1 2 3]` cannot work, because the macro has to *name*
  each `#[test]` it writes and `macro_rules!` cannot build an ident out of
  a number. The shards are named instead of numbered —
  `[shard_0 shard_1 shard_2 shard_3]` — and the tests go in a module named
  after the property, so a failure reads `property::shard_2`. A shard's
  index is its name's position in the list and the count is the list's
  length, which is what the recommendation was after: no new dependency,
  and both visible where the property is written.
- **Resolved (step 3) — shard counts.** Per-property, as recommended.
  Measured per property with one `cargo nextest run` at 64 cases, which
  also priced the tangent property the baseline table had omitted:

  | Property | 64 cases | ×3.906 → 256 | `k` | s/shard |
  |---|---|---|---|---|
  | `cut_then_fuse_restores_the_union_at_random_poses` | 114.53 | 458 | 30 | 15.3 |
  | `fuse_and_common_commute_at_random_poses` | 70.69 | 283 | 18 | 15.7 |
  | `overlapping_pairs_fuse_and_common_additively` | 48.27 | 193 | 12 | 16.1 |
  | `piercing_pairs_obey_every_identity_and_refuse_the_two_shells` | 13.44 | 54 | 4 | 13.5 |
  | `a_cylinder_tangent_to_a_box_face_leaves_the_box_and_shares_nothing` | 1.66 | 7 | 1 | 7 |

  64 shards in all — twice the core count, as the step asked — each about
  16 s at 256 cases, so no one shard sets the wall clock. The tangent
  property stays unsharded: at 7 s it is already below the floor.

- **Resolved (step 3) — `arris-mesh::tessellate` is not sharded.** Step 3
  says anything the table shows above the resulting wall clock gets the
  same treatment. It does not: 10.79 s at 256 cases and 41.15 s at 1000,
  against a suite that cannot go below total-work-over-cores — about 31 s
  and 121 s on this machine. `tessellate` is comfortably under both, so
  sharding it would add shards without moving the wall clock. It becomes
  worth revisiting only if `boolean_prop` gets cheaper per case (the S5
  backlog line).
- **Resolved (step 4) by the human — nextest is required.** The hook
  fails with the `cargo install cargo-nextest --locked` line when it is
  absent rather than falling back to `cargo test`, and `AGENTS.md` §Setup
  carries that line beside `git config core.hooksPath`. One code path, so
  every committer runs the same suite. Nothing is added to the backlog.
- **Resolved (step 4) — CI keeps no plain `cargo test --workspace`.** As
  recommended: `cargo nextest run --workspace` plus `cargo test --workspace
  --doc` is the whole suite, and a second full run would cost more than the
  parallelism buys. The `--features parallel` jobs moved to nextest too.
  The reason is written into the job's comment, where the next person to
  wonder will be.

- **Found and fixed in step 4 — two tests wrote one STEP file.** The step
  asked whether `target/inspect/` needed making per-test-unique. It did,
  and not only because of nextest: `corpus::run` named the file after the
  fixture's *recipe*, so `primitive_cylinder` and the scratch copy of that
  recipe in `a_dump_that_differs_by_one_id_fails_with_the_diff` both wrote
  `primitive-cylinder-default.step` — already concurrently, as two libtest
  threads, before this plan. `corpus::step_tag` now appends a short digest
  of the directory for any copy outside the corpus; the canonical fixture
  keeps the name `docs/ARCHITECTURE.md` and the `inspect` skill quote.
  Nothing else collides: renders use distinct names per test, and the
  scratch dirs already carry the process id. No test depended on sharing a
  process — nothing in the workspace mutates the environment or shares a
  static across tests.
