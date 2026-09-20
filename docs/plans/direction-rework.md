# Plan: direction-rework

- Started: 2026-09-20
- Milestone: none — a docs-only plan between C3's second and third plans;
  it changes the order of what follows C3 (docs/ROADMAP.md §Toward
  Parasolid grade), not C3
- Idea (verbatim from the human): "we just talked with simpler model about
  where this project developemnt directed and concluded that orgering of
  steps in roadmap brobably a bit off. Your task is to review and analyze
  project vision review, correct it if needed and plan docs rework
  (roadmap, seed, architecture, etc.) before future development"

## Goal

The tracked docs say where Arris goes after C3 and why, in an order chosen
by what its consumers meet first rather than by geometric generality. The
consumers the order is judged for are the three kinds `SEED.md` §1 already
names: an application behind a kernel facade, code-first and agent-driven
modelling through a first-party binding, and tools that import other
systems' parts. The decision is one ADR, because `SEED.md` §6 states the
old order and the seed is frozen:

- **Closure before breadth.** Every body an Arris operation can return is a
  legal operand of every other Arris operation before the kernel takes new
  kinds of input. C3 is boolean closure — `fillet`, `chamfer` and `revolve`
  already build the tori, spheres and cones the pave model's quadric guard
  refuses — so C3 stays whole and comes first.
- **Measured, then chosen.** A measuring harness (benchmarks, the oracle
  cached by recipe hash, a nightly property tier, random *recipes* run
  through both kernels, fuzz targets) stands beside the cycles. The STEP
  reader and a real-part corpus are the next cycle. The cycle after that is
  picked by the corpus's refusal histogram and the first consumer's
  side-by-side regressions (ADR-0017), not by a list written at kickoff.
- **Unopened cycles carry names, not numbers.** `C4`–`C8` are cited in
  rustdoc, the backlog and accepted ADRs; a reorder that rebinds a number
  makes an append-only ADR wrong for good. A cycle gets its number when
  `/close-cycle` opens it, which also keeps "cycle Cn releases `0.n.0`"
  (`.agents/rules/git.md`) lining up.

When the plan is done: ADR-0020 is accepted and `SEED.md` §6 points at it;
`docs/ROADMAP.md` has the new spine, the harness and the binding as lines
beside the cycles, the reader cycle outlined with its staged scope, and the
remaining cycles named and unordered under a stated selection rule; no
living doc, rustdoc line or backlog line names an unopened cycle by number;
the backlog has lost the lines the roadmap took and gained the ones this
review found; `AGENTS.md` and `/close-cycle` agree with all of it.

## Non-goals

- **No change to C3's content, order or accept line,** and none to
  `docs/plans/c3-torus-pairs.md`. The review considered stopping C3 short
  and rejects it (ADR-0020's alternatives).
- **No code.** The harness, the binding and the reader are each their own
  `/plan` or `/idea` after this retires; this plan only gives them a place.
  The one `.rs` change is rustdoc wording in step 4.
- **No decision on the binding's mechanics** — the `#![forbid(unsafe_code)]`
  exception, the crate's layer position, `publish`, the wasm job, the hook,
  PyPI beside crates.io. ADR-0020 says a first-party binding is in scope
  and in-repo; its own idea and ADR decide the rest.
- **No rewrite of `SEED.md`.** One amendment line, as ADR-0017's. §4's
  non-goals stand: UI, rendering and plugin hosts stay outside this repo.
- **No edit to an accepted ADR**, and none to a fixture's `fixture.json` if
  step 4 finds its `description` inside the recipe hash.
- The hook's property case count against CI's stays the human's call; the
  harness plan puts the question, this one does not.

## Design deltas

- **ADR-0020, the direction after C3** (new; 0019 is taken by
  `c3-torus-pairs` step 4 and may land after it). Context: the kickoff
  order in `SEED.md` §6 ranks cycles by geometric generality; what it cost
  is that the kernel refuses its own output, is unmeasured for speed, and
  has no way to meet a real part. Decision: the three bullets of *Goal*,
  the reader cycle's staged scope, the named cycles, the selection rule,
  and the table mapping `C4`–`C8` in older ADRs to names. Alternatives:
  keep the kickoff order; stop C3 at what one consumer needs; reader before
  C3 closes; a binding in a separate repo; renumber.
- **`SEED.md` §6** — one italic line under "Toward Parasolid grade":
  amended by ADR-0020. Nothing else in the seed moves.
- **`docs/ROADMAP.md`** — the spine sentence; a short "Beside the cycles"
  section (the measuring harness; the first-party binding, gated on C3's
  close so a script that fillets then cuts does not meet the guard); "Next:
  the reader and the real-part corpus" with in/out/accept in outline — in:
  a Part 21 parser, the AP203/214/242 B-Rep subset onto analytic and NURBS
  geometry, pcurves rebuilt (closed form on analytic surfaces, which pulls
  `project` onto a NURBS surface forward), each entity's tolerance assigned
  from its measured gaps, typed refusals for the rest; out: sewing, repair,
  open shells, booleans on NURBS faces; accept: write→read round trip as a
  property, Open CASCADE's STEP of every corpus fixture read back to the
  same numbers, a public real-part corpus read to checker-green or a typed
  refusal with mass properties on the oracle's, and the refusal histogram
  printed. Then "Named cycles, unordered": NURBS operands · blend networks
  · sweep along a path, loft, shell, offset · healing with sheet and wire
  bodies in every operation · the query set (distance, clash, ray fire,
  selection) · attribute propagation · IGES, same-domain merging and the
  performance pass — with the selection rule.
- **No public type, signature or crate boundary changes.**
- **`.agents/skills/close-cycle`** — opening the next cycle assigns its
  number and, past the reader cycle, applies the selection rule and records
  the numbers it was chosen on.

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness or
bound has to be established here.

- [x] Step 1 **[2]** — ADR-0020 as *Design deltas* describes it, with the
  evidence in its context stated as things a reader can re-check: the
  blend and sweep fixtures whose results carry a torus, a sphere or a cone;
  the guard's two call sites in `boolean/pave.rs`; `project` onto a NURBS
  surface being `Unsupported`; the recipe grammar covering every operation
  on both kernels. The index row in `docs/adr/README.md`; the amendment
  line in `SEED.md` §6. Consumer-agnostic wording throughout.
- [ ] Step 2 **[2]** — `docs/ROADMAP.md`: the spine, "Beside the cycles",
  the reader cycle's outline, the named cycles and the selection rule,
  replacing "Toward Parasolid grade — later cycles, one line each". C3's
  section gains one sentence — why it is first: closure — and nothing
  else. Check: every item the old C4–C8 lines held appears under a name.
- [ ] Step 3 **[1]** — `docs/BACKLOG.md`: remove the lines the roadmap now
  holds (benchmarks, `cargo-fuzz` targets, the oracle re-run and
  `recipe_hash`, the STEP reader before the first consumer's import, IGES);
  add the query set's pieces not yet listed, random recipes as a generator,
  and `project` onto a NURBS surface as the reader's prerequisite where
  the existing M3 line only names mesh deviation. The hook-count line stays,
  pointing at the harness plan.
- [ ] Step 4 **[1]** — `docs(sync)`: every `C4`–`C8` in living docs,
  the backlog, `tests/fixtures/README.md`, `README.md` and rustdoc becomes
  the cycle's name ("the NURBS cycle", "the blend-network cycle", …). Two
  fixtures' `description`s name C6: read how `recipe_hash` is computed
  first; if the description is hashed, leave them and let ADR-0020's table
  translate. Accepted ADRs are not touched.
- [ ] Step 5 **[1]** — `docs/ARCHITECTURE.md` (§How a consumer's kernel
  facade maps on: "the STEP reader is a later cycle" restated; §Operations'
  guard sentence checked against the roadmap's wording),
  `.agents/skills/close-cycle/SKILL.md` as *Design deltas*, `AGENTS.md`
  current state's "Next" and rules lines, `docs/README.md` if its roadmap
  row or lifecycle text says "later cycles".

## Acceptance

- `grep -rnw 'C[4-8]'` over `crates/`, `docs/` outside `docs/adr/` and
  this plan, `tests/fixtures/README.md`, `README.md`, `AGENTS.md` and
  `.agents/` prints nothing (or only the fixture descriptions step 4
  deliberately left, named in its commit body).
- Every item of the old "later cycles" list and every backlog line removed
  in step 3 is findable in `docs/ROADMAP.md` under a name.
- ADR-0020's name table, the roadmap's named cycles and `/close-cycle`'s
  text list the same names.
- The pre-commit hook is green on every step (step 4 touches rustdoc, so
  `cargo doc -D warnings` is the check that matters).

## Docs to update on completion

Most of this plan *is* the docs update; retirement checks rather than
writes:

- `docs/ROADMAP.md` — present tense throughout the new sections; no "as of
  this review" prose.
- `AGENTS.md` current state — under ~15 lines after step 5.
- `docs/adr/README.md` — ADR-0020 listed; 0019 still absent or present
  according to where `c3-torus-pairs` stands, never renumbered.
- `docs/BACKLOG.md` — no line duplicates a roadmap line.
- Follow-ups to start after retirement, within the two-plan cap: `/plan
  measuring-harness` (small, obvious, no idea needed) beside C3's work;
  `/idea` for the first-party binding; `/idea` for the reader cycle's scope
  as C3's third plan nears its end.

## Open questions

- ⚠ OPEN 1 — **C3 in full, against the earlier session's "finish only what
  the first consumer needs"** (human, before step 1). **Answered
  2026-09-20: in full** — ADR-0020 §1 and its second alternative.
  Recommended: in full.
  The quadric guard refuses bodies Arris's own `fillet`, `chamfer` and
  `revolve` return, so fillet-then-cut fails today; and a real-part corpus
  is mostly filleted parts, so the reader's signal depends on the guard
  being down. The rare part, torus–torus in general pose, passed its
  go/no-go gate and rides the same tracer.
- ⚠ OPEN 2 — **the binding waits for C3's close** (human, before step 2).
  **Answered 2026-09-20: yes** — ADR-0020 §2, last paragraph.
  Recommended: yes. The earlier session put it before the reader and after
  the benchmarks; both still hold. The alternative, starting it now in the
  second plan slot, puts a second crate under every C3 API break.
- ⚠ OPEN 3 — **is the harness a cycle?** (agent, step 2.) **Closed in step
  1: no** — ADR-0020 §2, second paragraph. Preferred: no —
  it changes no public API, so it earns no minor version; it is a plan
  beside C3 and then a standing line in the roadmap with its numbers
  (cases per night, benchmark baselines) kept current.
- ⚠ OPEN 4 — **the selection rule's tie-break** (agent, step 1): when the
  refusal histogram and the first consumer's regressions disagree.
  **Closed in step 1** as preferred — ADR-0020 §2, fourth paragraph: the
  consumer's regressions first while there is one consumer waiting on a
  swap (ADR-0017), the histogram after.
