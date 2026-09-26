# Plan: real-part-corpus

- Started: 2026-09-26
- Milestone: C4 (docs/ROADMAP.md §C4, the reader and the real-part corpus)
- Idea (verbatim from the human): "real-part-corpus"
- Decided by: ADR-0025 §6 and §7. No idea file; the scope was argued
  there and is not re-argued here.

## Goal

A corpus of parts Arris did not design is read, and every result is
counted. A committed tier of public-domain parts runs under `cargo test`
as `part` fixtures, and a fetched tier, a pinned and hash-listed sample
of a public dataset, runs by script. Each solid of each part reads either
to a body the checker passes, with volume, area and centroid within the
fixture's tolerance of Open CASCADE's reading of the same file, or to a
typed `Refusal`. Nothing panics and no solid is wrong.

Every checker-green solid then goes through ADR-0025 §6's fixed battery:

1. tessellate and measure;
2. write and read back;
3. cut by a box through the centroid, and by a cylinder drilled along
   each principal axis;
4. fillet a deterministic sample of its edges.

Each battery operation Open CASCADE also builds is held to its answer.
Each one Arris refuses is counted by its `Reason`.

Every refusal, the reader's `RefusalKind` and the operations' `Reason`
alike, maps through one declared table to the named cycle it blocks. The
histogram prints as *cycle → parts blocked* over both tiers, and C4
closes on it.

## Non-goals

- **Fixing what the histogram ranks.** Healing, offsets, NURBS booleans
  and blend networks are the cycles the histogram picks between. This
  plan counts them and builds none of them. A refusal that is the kernel
  working is a count, not a bug.
- **A wrong solid, a panic or a checker violation is not a count.** Each
  is a kernel bug under the kernel rules: shrunk to a `regression/`
  fixture and fixed inside this plan (step 5).
- **The first consumer's side-by-side run.** It is the consumer's to run,
  on its own schedule (ADR-0017). This plan prints the histogram it sits
  beside and does not wait for it.
- **Committing a part whose licence does not allow it.** A fetched part
  that fails is re-expressed as a recipe, or committed as an excerpt only
  where its licence allows (ADR-0025 §6).
- **New reader features.** No `CARTESIAN_TRANSFORMATION_OPERATOR_3D`,
  offsets or composites. They are backlog lines until the histogram
  ranks them.
- **Picking the next cycle.** `/close-cycle` does that, from this plan's
  numbers.

## Design deltas

- **ADR-0026 — the real-part corpus and the refusal table** (step 1). It
  records:
  - **The committed tier.** The NIST MBE PMI test models: which files, in
    which exporters' AP203/AP242 editions, their licence (US-government
    works) confirmed from the source page and quoted, and the size budget
    the repository takes for them.
  - **The fetched tier.** The dataset, its licence confirmed and quoted,
    the sample's size, and how it is drawn (seeded, the hash list
    committed, the files never committed). If no candidate's licence
    allows the use, the ADR says so and the fetched tier is dropped. C4's
    accept line then rests on the committed tier, which is widened with
    whatever public-domain parts the search found.
  - **The table, whole.** ADR-0025 §6's table is coarse. This one gives
    each `RefusalKind` and each `Reason`, and `Unsupported` by surface
    pair, one row naming the cycle it blocks or "counted as itself". It
    settles the cases §6 leaves open: `Topology`, `Pcurve`, `Invalid`,
    `Degenerate`, and the operations' tangent and non-manifold reasons.
    This is an amendment's worth of decisions, so it takes an ADR of its
    own rather than an edit to an accepted one.
  - **The oracle's reading.** Open CASCADE's `STEPControl_Reader` heals on
    transfer by default. The oracle's measures are the **healed** reading:
    step 1 found that healing can be turned off, but that the unhealed
    reading of a valid file can be an invalid shape with a negative
    volume (ADR-0026 §3). A second, unhealed reading records `occt_heals`
    per solid. A solid Open CASCADE heals and Arris refuses is counted as
    a refusal, never as a disagreement, and its counts are compared only
    where `occt_heals` is false.
- **A third fixture kind, `part`** (`"kind": "part"`, area `real/`): a
  STEP file beside `fixture.json`, `expected.json` and one dump per solid
  (step 3).
  - `fixture.json` names the file, its source, its licence and its
    `sha256`. It records each solid's expected outcome, `read` or
    `refused: {kind, why}`, and the battery's operands (step 6).
  - A change of expected outcome is a `fixtures:` commit, as for any
    fixture. A refusal that starts to read fails its test until the
    fixture says so, as `occt_step_refused` does today.
  - `arris_debug::fixtures` gains `Kind::Part`. The corpus lint holds
    `real/` like the other areas: every `read` solid has its dump.
- **The recipe grammar gains a `step` operand** (step 2): `{"op": "step",
  "file": …, "sha256": …, "id": #id, "near": [x, y, z]}`, the solid of a
  file named by its entity and, where an assembly places it more than
  once, by the point its placement's centroid is nearest. Both
  interpreters take it, so the battery runs through the recipe machinery,
  `expected.py`, the corpus runner and the differential's classes
  unchanged.
  - Step 2 found that Open CASCADE's model keeps no `#id` and that
    reproducing Arris's order of placements in Python would mean writing
    the flattening a second time. So neither side's order of instances
    names a solid: the `#id` does, and `near` picks the placement.
    Python finds the `#id` by content (the outer shell's face count and
    first vertex). The mapping is the step's test.
- **`arris_debug::battery`** (step 6): the battery's operands derived from
  a read solid, deterministically. The box and the three drills are placed
  from the oracle's centroid and principal axes. The fillet's edges are
  every edge of the solid in id order, sampled at a fixed stride, each
  named by its curve's midpoint. The radius is a fixed fraction of the
  shortest sampled edge's length. It writes them into the fixture as
  variants of steps built on the `step` operand. Once written, they are
  data: a later kernel change that would pick other operands does not
  silently change the fixture.
- **`arris_debug::histogram`** (step 7):
  - `enum Cycle { Healing, Nurbs, BlendNetwork, Sweep, Itself(&'static
    str) }`, with `fn blocks_refusal(&Refusal) -> Cycle` and `fn
    blocks_reason(Stage, &OpError) -> Option<Cycle>` (`None` for
    `Internal`, a bug and never a count). Both are exhaustive matches with
    no wildcard, so a new kind or `Reason` fails to compile until the table
    gets its row. The refusal is taken whole, and not as its kind, because
    `Unsupported` splits by the entity it names: a shell-based model is
    the healing cycle's, a faceted B-rep counts as itself (ADR-0026 §5).
  - `Histogram` counts per battery stage and per cycle, and prints as
    markdown. The test is that every `RefusalKind::ALL` entry and every
    `Reason` has a row, which the compiler holds.
- **`tools/real-parts.sh`**, with its Rust side
  `crates/arris/examples/real_parts.rs` (step 8). It fetches the pinned
  sample into `target/real-parts/`, checks each `sha256`, runs the reader,
  the oracle and the battery on every part, and writes the histogram and
  a failure list. A failure is a panic, a checker-rejected `Ok`, or an
  `Ok` outside the oracle's measures. The nightly job runs it
  (`nightly.yml`, one job, the fetch cached like the fuzz corpora).
- **The reader's cost in a release build** (step 9): the corpus benchmark
  gains a read case per `real/` part, with and without the checker at
  `Fast`. This is the measurement ADR-0025 §Consequences promised.
- **No public type or signature of a published crate changes.** Every
  addition is in `arris-debug` (unpublished), `tools/` or the fixtures. A
  kernel fix in step 5 that changes one names it in its own commit, as
  always.

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness or
bound has to be established here.

The riskiest unknown is what real files do to the reader. That comes
first, at step 4, right after the least that is needed to hold a real
part to an oracle.

- [x] Step 1 **[2]** — ADR-0026, from the sources themselves.
  - Fetch the NIST MBE PMI models' page and licence and the fetched-tier
    candidates' licence texts. Candidates include the ABC dataset and the
    Fusion 360 Gallery; the ADR names whichever were weighed.
  - Quote the terms in the ADR.
  - Write the full refusal table (§Design deltas).
  - Download the NIST files into `target/real-parts/nist/`, uncommitted,
    and run `inspect_step` over them once. This sizes the committed tier
    against a budget and gives step 5 a first look. Nothing is fixed here.

  Test: none beyond the hook. The ADR is the output, and its first-look
  numbers (solids, read, refused by kind, panics) go in its Context.
- [x] Step 2 **[2]** — The `step` operand in both interpreters.
  - Rust: `arris_debug::fixtures`'s recipe evaluator calls `step::read` and
    takes the k-th `ReadSolid`. A refusal is the step's typed error.
  - Python: `tools/oracle/oracle/recipe.py` reads with `STEPControl_Reader`
    under ADR-0026's settings and maps solids by `#id` and placement.

  Test: a fixture under `boolean/` whose `step` operand is Arris's own
  STEP of an existing fixture's result, committed beside it, cut by a
  box. Both kernels agree through the whole corpus runner.
- [x] Step 3 **[1]** — The `part` fixture kind.
  - `Kind::Part` in `arris_debug::fixtures` and in Python's `fixture.py`.
  - `expected.py` measures each solid of the file (counts, genus, volume,
    area, centroid, inertia).
  - The runner reads each solid and holds it to its expected outcome: a
    `read` solid through every corpus stage the solid kind has (checker at
    `Full`, counts, measures, mesh, probes if any, dump), and a `refused`
    solid to exactly its `RefusalKind`.
  - The lint covers `real/`: the file's `sha256` matches, and every `read`
    solid has a dump.

  Test: one NIST part committed as `real/<slug>` passes. A copy of it
  spoiled by hand, with one face's surface swapped for an
  `OFFSET_SURFACE`, passes as `refused: offset`, and the lint accepts it
  in `real/`.
- [x] Step 4 **[2]** — The committed tier. Every NIST file ADR-0026 names
  becomes a `real/` fixture. Each solid either reads within the
  fixture's tolerances of the oracle, with a blessed dump, or is recorded
  as the refusal the reader returns. The refusal's `why` must say why the
  refusal is right, not only which kind it is.

  Every panic, `Ok` that fails the checker, or `Ok` outside the oracle's
  measures is **not** recorded as an outcome. Each is shrunk to a
  `regression/` fixture, ignored with its reason, and its part's fixture
  waits under an exclusion that names it.

  Test: `cargo nextest run -p arris --test corpus real_` green, with the
  exclusions asserted to be exactly the open `regression/` fixtures.
- [ ] Step 5 **[3]** — The wrong solids and panics from step 4 fixed, one
  commit per root cause, each moving its `regression/` fixture into its
  area and lifting its exclusion. Wrong refusals and reads past the time
  budget count as well (ADR-0026 §4 and §6). Step 1's first look already
  names nine causes:
  - the seamless cylinder band, left invalid;
  - the default reference direction of an `AXIS2_PLACEMENT_3D`, refused;
  - a pcurve fit that fails at the cap on a curve nearer the surface than
    the cap, reported as a `Gap` of twice the cap;
  - a `VERTEX_LOOP` seam drawn across the face's other bound;
  - a `CAMERA_MODEL_D3` followed where a placement belongs;
  - a `TESSELLATED_SOLID` read as no solid, instead of refused;
  - placements the file does not have: 2–19 instances of a solid where
    Open CASCADE's product structure holds one (nine AP242 editions);
  - an FTC-06 AP242 read that fails the checker at `Full` (S5);
  - reads taking 9–67 s, 790 s (FTC-10 AP242), and past 900 s (CTC-02
    AP242).

  Step 4 shrank the committed tier's to five `regression/` part fixtures,
  one box each:
  - [x] `seamless-cylinder-band` (CTC-03, FTC-11; FTC-10 then meets
    the next);
  - [x] `cone-face-without-its-apex` (FTC-10; new at step 5, behind the
    band: a cone bounded by its base circle alone, the apex implicit);
  - [x] `axis-placement-along-x-without-reference` (CTC-04, FTC-08);
  - [ ] `pcurve-fit-reported-as-gap` (CTC-01, FTC-07);
  - [x] `edge-through-sphere-pole` (FTC-06; new at step 4, a half sphere
    bounded by one meridian circle through both poles);
  - [ ] `slow-gap-refusal` (CTC-05, 66 s in release).
  - [x] an edge's tolerance measured only at the checker's samples, a
    fitted pcurve straying past it between them, and the mesh's corners
    held to the face's tolerance alone (CTC-04, FTC-08; found at step 5
    behind the placement, ADR-0026's amendment of step 5).

  The rest are the fetched tier's: the camera, the tessellated solid, the
  phantom placements, FTC-06 AP242's `Full` failure, and the 790 s and
  900 s reads. Step 8 shrinks them the same way.

  If a cause is a missing feature rather than a bug, the `Ok` becomes
  the refusal it should have been: named, counted, and never an
  approximation. This step may split into several at `/work` time, one
  box per cause.

  Test: no exclusion left; step 4's test green with every part's outcome
  recorded.
- [ ] Step 6 **[2]** — The battery.
  - `arris_debug::battery` writes each `read` solid's battery into its
    fixture as variants of steps over the `step` operand:
    - `write_read`, the round trip held as step 13 of the reader's plan
      held it;
    - `box_cut`;
    - `drill_x`, `drill_y`, `drill_z` along the principal axes;
    - `fillet`.
  - `expected.py` builds each variant in Open CASCADE.
  - The runner sorts each variant's outcome into the differential's
    classes (ADR-0024 §2).
  - `Agree`, `BothRefuse`, `OracleRefuses` and `ArrisRefuses(Reason)` are
    recorded in the fixture as the variant's expected class.
  - `Disagree`, `CheckerViolation` and `Panic` are kernel bugs. Each is
    shrunk to `regression/` as at step 4.

  Test: every `real/` fixture's battery runs green to its recorded class.
  A battery regenerated from the same file writes byte-identical
  operands.
- [ ] Step 7 **[1]** — `arris_debug::histogram`: ADR-0026's table as
  exhaustive matches.
  - `Histogram` over the committed tier: read refusals by kind, battery
    refusals by `Reason` and stage, each mapped to its cycle.
  - `cargo run -p arris --example real_parts -- --committed` prints it as
    markdown.

  Test: every `RefusalKind::ALL` entry maps (a loop over the array), the
  `Reason` match compiles without a wildcard (the kernel rule), and the
  printed histogram of the committed tier is byte-identical on two runs.
- [ ] Step 8 **[2]** — The fetched tier, if ADR-0026 kept one.
  - `tools/real-parts.sh` fetches the pinned sample into
    `target/real-parts/`, checks every hash, and runs
    `real_parts` over it with the oracle through the cache (ADR-0024).
  - It writes `target/real-parts/histogram.md` and `failures.md`.
  - `nightly.yml` gains the job, the fetch cached.
  - Every failure it lists on the first full run is a kernel bug, handled
    as at steps 4 and 5: re-expressed as a recipe fixture under
    `regression/`, or as an excerpt where the licence allows. A bug too
    deep for this plan is a named exclusion with its fixture, never a
    silent skip.

  Test: the script runs green end to end on the pinned sample locally,
  and its failure list is empty or every entry is excluded by a
  committed `regression/` fixture. If there is no fetched tier, this step
  becomes the committed tier's script only, and its box says so.
- [ ] Step 9 **[1]** — The reader in the corpus benchmark: a read case per
  `real/` part, timed with the checker at `Fast` and without it (a
  `bench`-only switch in `arris-debug`, not a public option). The
  release-build cost per solid goes in ARCHITECTURE §Formats and tools.

  Test: `tools/bench-compare.sh` runs with the new cases present.
- [ ] Step 10 **[1]** — The histogram recorded.
  - The committed and fetched tiers' histograms are printed and written
    into `docs/ROADMAP.md` §C4's status line. This is the table
    `/close-cycle` reads to pick the next cycle beside the first
    consumer's regressions (ADR-0020).
  - A backlog line per refusal family is re-ranked by its count.

  Test: the numbers in the roadmap are exactly the example's output on
  the pinned inputs, and a docs test checks that the roadmap's table and
  the committed tier's printed histogram agree.

## Acceptance

C4's accept line, second half:

- `cargo nextest run --workspace` passes. Every `real/` fixture holds:
  - each solid read checker-green at `Full`, within its tolerances of the
    oracle, or refused with its recorded `RefusalKind`;
  - each battery variant at its recorded class;
  - no exclusion left open (step 5).
- `tools/real-parts.sh` on the pinned fetched sample finishes with an
  empty failure list, or every entry excluded by a committed
  `regression/` fixture. It is dropped from the acceptance if ADR-0026
  dropped the tier.
- Over both tiers: no panic, and no `Ok` the checker rejects or the
  oracle's measures contradict. That is "no wrong solid".
- The refusal histogram, *cycle → parts blocked*, is printed and
  recorded in the roadmap.
- The first half's acceptance still holds (read-back stages, round-trip
  property, fuzz target).

## Docs to update on completion

- `docs/ARCHITECTURE.md`:
  - §Formats and tools: the reader's release-build cost per solid; the
    `real_parts` example and `tools/real-parts.sh`; the nightly job's new
    line;
  - §Crates: `arris-debug`'s row gains `battery` and `histogram`.
- `docs/ROADMAP.md`:
  - §Fixtures: the `part` kind and the `real/` area;
  - §C4: the second half done, with the histogram — then `/close-cycle`;
  - §Named cycles: nothing re-ordered here; the histogram is cited for
    `/close-cycle` to read.
- `tests/fixtures/README.md`: the `part` kind, the `step` operand, the
  battery variants and their recorded classes, the licence and hash
  fields.
- `tools/oracle/README.md`: `expected.py` on a `part` fixture, the `step`
  operand, and the reader settings ADR-0026 fixed.
- `docs/BACKLOG.md`: the refused-entity lines re-ranked or annotated with
  their counts; a line for each named exclusion step 8 leaves open.
- `.agents/skills/inspect/SKILL.md`: a `real/` part inspected by its
  fixture name.
- `AGENTS.md` current state: C4 done, the histogram's top line named.

## Open questions

- Decided at step 1 (ADR-0026): the fetched tier is NIST's other STEP
  editions and the D2MI models, since ABC's per-model licence cannot be
  confirmed and Fusion 360 Gallery's is non-commercial. The committed
  tier is the eleven AP203 geometry-only files, 4.4 MB, committed
  uncompressed under a 5 MB budget. The oracle reads healed and records
  `occt_heals` from a second, unhealed reading, because healing *can*
  be turned off but the unhealed reading of a valid file is not a solid.
- Decided at step 4 (ADR-0026, amendment of step 4): the `real_` tests
  stay in the hook, at 2.7 s with the waiting parts skipped. A slow read
  is a fixture with a `read_seconds` budget, and no other fixture has
  one.
- ⚠ OPEN: a `SHELL_BASED_SURFACE_MODEL` under a
  `CONSTRUCTIVE_GEOMETRY_REPRESENTATION` ('supplemental geometry', two
  in CTC-04) is the exporter's construction geometry, not a body of the
  part, yet ADR-0026 §5 counts it against the healing cycle. **Agent**,
  at step 7: whether the table counts it as itself, which would take the
  reader telling it apart and so a new kind or field, a design delta.
