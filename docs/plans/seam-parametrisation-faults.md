# Plan: seam-parametrisation-faults

- Started: 2026-09-18
- Milestone: C2's acceptance gate (docs/ROADMAP.md §C2 — the application gate)
- Idea (verbatim from the human): "I see tests failed in ci" → "Hold, fix,
  cut 0.1.1" → "yes, write the plan"

## Goal

A boolean's outcome does not depend on where an operand's seam sits.
Turning one of two crossing cylinders about its own axis leaves the solid
unchanged and moves only its seam, and today that turn decides whether
`fuse` succeeds, returns `Fault::Seam` or refuses with `Unsupported`; when
this plan is done it always succeeds, and the fixture that records the case
passes every corpus stage. The corpus gains the one mechanism the case
needs — a fixture may state that the oracle is wrong and be held to its own
closed forms instead — because Open CASCADE mishandles the same band.
`arris-ops`' `crossing_cylinders_obey_every_identity` is then green at the
1000 cases CI runs, and `/release` can cut 0.1.1 over the five crates of
0.1.0 that went up before CI first ran.

## Non-goals

- The near-tangent *geometry* cases that are genuinely hard: two cylinders
  whose axes nearly meet, or meet at a near-zero angle. This plan is about
  a configuration that is not near-degenerate at all — the solid is the
  ordinary Steinmetz union — where only the parametrisation is awkward.
- Quadric–quadric curves that are not conics (C3, the quadric-curve
  `⚠ OPEN` in `docs/DATA-MODEL.md`).
- Making Open CASCADE right. The oracle's error in this band is recorded,
  not fixed, and `tools/oracle/` keeps computing what it computes.
- Raising the property-test case count the pre-commit hook runs. That is a
  real question this failure raises — see *Open questions* — but changing
  it is not this plan's work.

## Design deltas

- **`arris_debug::fixtures::Analytic` gains `measure_differs:
  Option<String>`** (public type, breaking for anyone constructing
  `Analytic` literally; `#[serde(default)]` keeps every existing
  `fixture.json` valid). It is the measurement twin of `counts_differ`:
  when set, `volume`, `area` and `centroid` are required and are what the
  corpus runner and `tools/oracle/compare.py` hold the result to; the
  oracle's values stay in `expected.json` as the record; the lint skips the
  1e-6 analytic-against-oracle comparison for exactly those fields and
  fails if they do *not* in fact differ, the way `counts_differ` already
  does. **`Analytic` also gains `inertia: Option<[[Num; 3]; 3]>`**,
  required under `measure_differs` and cross-checked against the oracle
  like the other closed forms where a fixture states it anyway
  (ADR-0015 — decided in step 1, which settled the open question
  below).
- **`tests/fixtures/README.md` §`fixture.json`:** the new key beside
  `counts_differ`, and what an author must show before using it.
- **ADR-0015** (step 1): a fixture may declare the oracle wrong. The rule
  in `.agents/rules/kernel.md` §Testing — "every fixture has an oracle" —
  is not repealed; it gains the case where the oracle is demonstrably
  wrong and the closed form is exact, and the evidence a fixture must
  carry to claim it.
- **`docs/ARCHITECTURE.md` §Operations** (the pave model, the paragraph on
  hits, touches and section vertices): whatever step 4 finds is stated
  there, since that paragraph is where the seam's touch on the other wall
  and its pave are described today.
- **No new crate, no layer change, no change to `OpError` or `Fault`.**
  `Fault::Seam` stays exactly what it is — a kernel bug, caught. This plan
  stops raising it, rather than making it something else.

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness or
bound has to be established here.

- [x] Step 1 **[2]** — ADR-0015: a fixture may declare the oracle wrong.
  The evidence is in hand and goes in the ADR: for a body that is exactly
  centrally symmetric and provably independent of the turn (rotating a
  cylinder about its own axis is the identity on the solid), Open CASCADE
  returns 9 vertices and 15 edges where the same solid at a generic turn
  gives 11 and 17, a volume above the exact `2πR²L − 16R³/3` and a
  centroid off the origin, both linear in the turn past −90° — a sliver
  proportional to the offset. (Re-measured for the ADR: at −90.02° the
  volume is 1.46e-3 high, 4.5e-5 relative, and the centroid 3.77e-6 off
  in x; the plan's first draft had −90.01°'s relative volume and called
  −90.02° → −90.03° a doubling, which is −90.01° → −90.02°. The band is
  one-sided, −90° itself and −89.98° are exact, and it ends between
  −90.045° and −90.049°.) Sets what a
  fixture must show to claim `measure_differs`: a closed form for every
  field it claims, and a statement of why the oracle's answer is wrong
  rather than merely different. Commit: `docs(adr)`.
- [x] Step 2 **[1]** — `measure_differs` in `arris-debug`'s fixture
  format, the corpus lint, the corpus runner and `tools/oracle/compare.py`,
  per the design delta. Tests: the lint's own mutation harness gains the
  two new failures (the key set with a field missing; the key set where the
  closed forms and the oracle agree), every existing fixture unchanged, and
  `tools/oracle/selftest.py` green. The public type change is named in the
  commit body.
- [x] Step 3 **[1]** — The regression fixture. `regression/
  seam-beside-crossing-fuse`: `cross-cylinders-fuse`'s two cylinders with
  `turn` at −90.02° (default) and a `closed-form-band` variant at −90.03°,
  its `analytic` carrying `measure_differs` and the closed forms, its
  `expected.json` regenerated. Its test `regression_seam_beside_crossing_fuse`
  in `crates/arris/tests/corpus.rs`, `#[ignore]`d with both symptoms named.
  The recipe and `expected.json` parked in the writing session's
  scratchpad did not survive it; the step rebuilds them from
  `cross-cylinders-fuse`'s recipe, as step 1's measurements already did.
  Arris's counts are 10/16/8/8, as at a generic turn: each seam still
  meets each ellipse once, only nearer the crossing vertex.
  Found here: Open CASCADE's own body in the band does not survive its
  own STEP round trip (volume 32.367241 → 32.366737 at −90.02°), so
  `selftest.py` round-trips a `measure_differs` fixture on counts, genus
  and probes only; ADR-0015 carries a dated amendment saying so. The
  symptoms are confirmed as planned, the face names being f7 and f1
  here: `Fault::Seam` ("a section edge of f7 and f1 crosses a seam of f7
  without a pave there") at −90.02°, `Unsupported` ("no closed form for
  +f7 … against +f1") at −90.03°.
  Commit: `test(fixtures)`.
- [ ] Step 4 **[3]** — The `Fault::Seam` band, −90.01° to −90.02°. The
  unknown this plan turns on. What is known: `interferences` itself raises
  it, before any splitting; at the passing −90.05° the seam's two hits on
  the other wall sit at ±δ radians from the crossing vertex (±8.7e-4 at
  0.05°, so linear in δ, not `√δ`) with vertex tolerance 1e-7, so the
  vertices are distinct by four orders of magnitude and this is not a
  tolerance merge. Find why the pave the section edge expects is not there,
  fix it, and state the rule in `docs/ARCHITECTURE.md` §Operations. The
  fixture's default variant passes every stage, and the fixture moves from
  `regression/` into `boolean/` with its blessed dump (leaving the variant
  behind if step 5 is still open, which the corpus lint forbids — so if
  step 5 has not landed, the move waits for it and this step says so).
- [ ] Step 5 **[2]** — The `Unsupported` band, −90.03° to −90.04°:
  `no closed form for +f10 (cylinder surface) against +f4 (cylinder
  surface)` on a pair `interferences` reports as `transversal` with both
  ellipses, so the refusal comes from a later ask, not from the pave. Find
  which one, fix it, and the `closed-form-band` variant passes. If step 4
  finds one root cause for both bands, this step collapses into it and is
  ticked with a note saying so rather than a second commit.
- [ ] Step 6 **[1]** — The sweep that proves the invariant, not just the
  two poses: a property (or an extension of the existing one) that fuses
  the same pair of solids at a spread of turns and asserts the result is
  the same body — same volume, area and counts — for every one of them.
  Seeded, sharded like its neighbours, and it fails today at −90.02° in one
  line. This is what stops the next seam-shaped bug from waiting for a
  1000-case run to find it.

## Acceptance

- `cargo nextest run --workspace` green, and `ARRIS_PROPTEST_CASES=1000
  cargo nextest run -p arris-ops --test boolean_prop` green — the run CI
  makes, with the counterexample the seed `arris-property-tests-seed-v1`
  found at shard 5 gone.
- `boolean/seam-beside-crossing-fuse` passing every corpus stage in both
  variants — checker at `Full` with nothing unchecked, counts, the closed
  forms it is held to under `measure_differs`, probes, provenance and its
  blessed dump — and nothing left under `tests/fixtures/regression/`.
- The corpus lint green, including its two new mutation cases, and
  `tools/oracle/selftest.py` green.
- Step 6's property green at its configured case count.
- Then, outside the plan: `/release` cuts 0.1.1, which is what the five
  crates already at 0.1.0 are waiting for.

## Docs to update on completion

- `docs/ARCHITECTURE.md` §Operations — the pave-model paragraph, with the
  rule step 4 establishes.
- `tests/fixtures/README.md` §`fixture.json` — `measure_differs` beside
  `counts_differ`, and §Property-test failures if step 6 changes what a
  property failure produces.
- `docs/adr/README.md` — ADR-0015.
- `.agents/rules/kernel.md` §Testing — "every fixture has an oracle" gains
  the pointer to ADR-0015 for the case where the oracle is wrong.
- `docs/ROADMAP.md` §C2 — the crossing-cylinder line notes the
  parametrisation invariant now held by a property.
- `docs/BACKLOG.md` — whatever step 4 or 5 finds and does not fix.
- `AGENTS.md` current state only if C2's line stops being true meanwhile.

## Open questions

- `⚠ OPEN:` **The case count the hook runs** (human, any time). This bug
  sat in `main` from the cylinder–cylinder work until the repository's
  first CI run, because the pre-commit hook runs 256 cases and CI runs
  1000, and the counterexample lives past case 32 of shard 5. Raising the
  hook's count slows every commit; leaving it means CI is the first place a
  property failure appears, which is exactly what happened here and cost a
  half-published release. A third option is a nightly run at a higher count
  than CI's. Not this plan's to decide, but this plan is its evidence.
- **Decided in step 1 (ADR-0015): `measure_differs` needs an inertia
  closed form.** Skipping it left nothing checking Arris's inertia where
  the oracle cannot; for the Steinmetz union it is the two cylinders'
  tensors less the bicylinder's, and matches the oracle's generic-turn
  tensor to 1e-11.
- **Found in this session, not a question:** Open CASCADE is wrong in the
  same band, in the same direction, by an amount proportional to the turn.
  Both kernels degrade where a seam nearly meets a crossing; only ours says
  so out loud.
