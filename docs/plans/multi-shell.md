# Plan: multi-shell

- Started: 2026-09-12
- Milestone: C2, the application gate (docs/ROADMAP.md §C2, the
  multi-shell line)
- Idea (verbatim from the human): "do all 3" — accepting "`/plan`
  multi-shell results as the first code"

## Goal

A boolean or a sweep whose result has more than one shell returns it
instead of refusing. A `Solid` body holds one or more *lumps* — an outer
shell enclosing positive volume, with the void shells inside it — so a
cavity, a cut that splits its target, a disjoint `fuse` and a full revolve
of a profile with holes are each one body the checker proves at `Full`,
the corpus measures against Open CASCADE, STEP writes and the consumer
holds as one shape. `Reason::MultiShell` is gone; the consumer's hollow-box
probe (0.1³ − 0.04³ = 9.36e-4, two shells) passes as a fixture.

## Non-goals

- `General` and `Sheet` bodies, a face used by two shells, an edge of more
  than two uses: two lumps touching along an edge or at a vertex stay a
  typed refusal (C7's non-manifold bodies).
- Merging same-domain faces after a fuse; any new surface pair; the revolve
  touching its axis (its own plan, next); fillet (`docs/ideas/fillet-and-chamfer.md`).
- Returning several bodies from one operation.
- A lump structure stored on the `Body` entity or in the native format
  (`⚠ OPEN` 3).

## Design deltas

- **ADR-0006, lumps in one `Solid`**, amending ADR-0004's consequence
  that a multi-shell result is "a `General` body … cycle 2's". Open
  CASCADE returns a compound of solids; Parasolid and ACIS hold disjoint
  regions or lumps in one body; the consumer's facade returns one shape
  per operation, so one body it is. A lump is derived, not stored: the
  body's shell list is written outer shell, then its voids, lump by lump,
  in a deterministic order, and B1 proves the nesting.
- `docs/DATA-MODEL.md` §Entities: `Solid` is "every shell closed, every
  edge used by exactly two coedges, the shells nesting into lumps".
  §Invariants B1: every shell of positive volume is an outer shell, every
  shell of negative volume a void; each void's innermost container is an
  outer shell and each outer shell's is none or a void (a lump inside a
  cavity of another is allowed); no two outer shells at one level
  overlap. S3 unchanged (per shell).
- `arris-topo::builder`: `Assembly::faces: Vec<FaceSpec>` →
  `Assembly::shells: Vec<Vec<FaceSpec>>` (**public type change**).
  `assemble` proves each shell one edge-connected component, no edge or
  vertex shared between shells, the Euler line at `S = shells.len()`.
  `Builder::finish` over the operators stays one shell. Found at step 1:
  `Built::shell: ShellId` → `Built::shells: Vec<ShellId>`,
  `StagedFace::shell()` (a face made by an operator takes its parent's),
  and `BuildError::{EmptyShell, SharedEdge, SharedVertex}`, the Euler line
  checked per shell with the builder's genus their sum (**public type
  changes**).
- `arris-check`: `lumps(&Model, Body) -> Result<Vec<Lump>, …>` with `Lump
  { outer: Shell, voids: Vec<Shell> }` (**new public item**), the nesting
  B1 proves, for the writer and the consumer.
- `arris-ops`: `Reason::MultiShell` removed and `Reason::NonManifold {
  entities }` added (**public enum change**). The boolean groups its
  survivors into shells, orders them into lumps by signed volume and
  containment, and assembles them. `SweepPart::Cavity { loop_index }`
  added: the shell a hole closes into on a full revolve (**public enum
  change**). `transform` carries every shell. Found at step 4: the lump
  order is read by assembling the shells once into a clone of the model
  and asking `arris_check::lumps`, so it is B1's own; a nesting that
  fails there is `Fault::Lumps(LumpError)` (**public enum change**);
  `Reason::MultiShell` stays until step 6, where the revolve stops
  refusing with it; `NonManifold` is fieldless, its entities in
  `OpError::Degenerate`'s list as every other reason's are.
- `docs/DATA-MODEL.md` §Provenance: a result shell is `Modified` from
  every operand shell a piece of it came from, except a shell made only of
  a cut tool's pieces, which is `Generated` from the tool's shell (the
  tool keeps nothing, as today).
- `arris-io` STEP: one `MANIFOLD_SOLID_BREP` per lump without voids, a
  `BREP_WITH_VOIDS` over its `CLOSED_SHELL` and `ORIENTED_CLOSED_SHELL`s per
  lump with them, all in the product's one shape representation;
  `Unsupported::Shells` removed. Found at step 3: `StepError::Lumps`
  for a solid whose shells do not nest, and the runner's
  `CorpusError::Lumps` (**public enum changes**).
- `arris-debug` corpus: `solids` is the lump count, not `1`; `expect_error`
  gains `non-manifold` (step 5) and loses `multi-shell` (step 4, with
  `split-cut`, its last user).

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness or
bound has to be established here.

- [x] Step 1 **[2]** — ADR-0006. `Assembly::shells`; `assemble` proves
  each shell connected and closed and no entity shared between shells;
  the Euler line over several shells; every caller moved to one-element
  `shells`. Tests: two boxes' faces kept into one body of two shells; an
  edge shared by two shells is `BuildError`; counts `16/24/12/12/2`.
- [x] Step 2 **[2]** — B1 over lumps and `arris_check::lumps`. Checker
  tests, each built through the raw insert and named: two disjoint boxes
  pass; a void outside every outer shell; two overlapping outer shells; a
  box inside the cavity of a hollow box passes (two lumps, one of them
  nested); a void inside a void fails; a shell no ray classifies is
  unchecked.
- [x] Step 3 **[2]** — STEP writes lumps and voids. The corpus runner's
  `solids` from `lumps`. Test: a hand-assembled hollow box and a
  two-box body written, read back by Open CASCADE (`arris_debug::oracle`)
  with the volume, the shell and solid counts of the closed form.
- [x] Step 4 **[2]** — the boolean returns several shells: survivors
  grouped, ordered into lumps, assembled; shell provenance as above;
  the boolean never returns `Reason::MultiShell`. Fixtures, each passing
  every corpus stage:
  `boolean/enclosed-cavity` (the consumer's hollow box, its units: volume
  9.36e-4, 2 shells, 1 solid), `boolean/split-cut` flipped from
  `expect_error` to passing (10800, 2 solids), `boolean/disjoint-fuse`,
  `boolean/cavity-cylinder` (a box − an interior cylinder),
  `boolean/lump-in-cavity` (a hollow box fused with a box inside its
  cavity). `boolean_prop`: the piercing pair's `cylinder − box` holds
  `V(A − B) + V(A ∩ B) = V(A)` at every pose instead of refusing.
- [x] Step 5 **[2]** — two result shells touching along an edge or at a
  vertex are `Reason::NonManifold` naming the shared entities, before
  anything is assembled. Fixture `boolean/edge-touching-fuse` (two boxes
  sharing an edge) with `expect_error: "non-manifold"`; the corpus grammar
  and lint learn it. Found at step 5: the `edge-touching-cut` first
  planned does not exist — a cut leaving two boxes that share an edge
  inside the target needs a tool filling the other two quadrants around
  that edge, which is itself non-manifold — so the fixture is the fuse of
  two such boxes, and the corner case is a unit test beside it. Open
  CASCADE's compound of two solids sharing an edge or a vertex has an odd
  Euler characteristic, so the oracle records no genus for a
  `non-manifold` recipe and the lint asks none.
- [ ] Step 6 **[2]** — a full revolve of a profile with holes: one void
  shell per hole, `SweepPart::Cavity { loop_index }`; `Reason::MultiShell`
  removed. Fixture
  `sweep/revolve-hollow-ring` (a rectangle with a rectangular hole about
  an axis clear of both: planes and cylinders only). `transform` of it
  in `provenance/` or a `transform/` fixture, so a lump body moves whole.

## Acceptance

`cargo nextest run --workspace` with the corpus: every fixture above
passing every stage (checker `Full` with nothing unchecked, counts and
genus, Open CASCADE's volume, area, centroid and probes through STEP, mesh
closed, mass properties and inertia, classification, provenance
accounting, dump) and `boolean/edge-touching-cut` refused as designed;
`boolean_prop` green at the configured case count; no fixture left
naming `multi-shell`.

## Docs to update on completion

- `docs/DATA-MODEL.md` §Entities (a `Solid`'s lumps), §Invariants (B1),
  §Provenance (shell relations, `SweepPart::Cavity`).
- `docs/ARCHITECTURE.md` §Operations (the boolean's grouping into lumps,
  the property-test paragraph's "designed `MultiShell` refusal", revolve
  with holes, `transform` "one edge-connected shell"), §Errors
  (`MultiShell` out, `NonManifold` in), §The checker (`lumps`), §Formats
  and tools (STEP lumps and voids).
- `tests/fixtures/README.md` — `expect_error` values, `split-cut` no longer
  the example.
- `docs/ROADMAP.md` §C2 — the multi-shell line's status.
- `AGENTS.md` current state — ADR-0006.

## Open questions

- ~~`⚠ OPEN 1:`~~ lumps in one `Solid` (preferred) versus a `General`
  body as ADR-0004 foresaw, versus several bodies per operation.
  **Decided at step 1 (the human delegated it): lumps in one `Solid`,
  ADR-0006.**
- ~~`⚠ OPEN 2:`~~ two lumps meeting at a single vertex: `NonManifold`
  (preferred, what a manifold `Solid` promises) or allowed — agent, step 5,
  checked against what Open CASCADE's `BRepCheck` says of the same shape.
  **Decided at step 5: `NonManifold`.** Run through the oracle,
  `BRepCheck_Analyzer` rejects a `TopoDS_Solid` of two outer shells
  whether they are apart or share a vertex or an edge, and accepts a
  compound of the same two solids in all three cases (ADR-0006): its
  solid is Arris's lump, its compound is where touching lives, so the
  check does not make the vertex case manifold and the refusal stands.
- ~~`⚠ OPEN 3:`~~ lumps derived by B1 and `lumps()` (preferred: no entity
  or format change) versus stored on `Body` — agent, step 2; storing
  becomes worth it only if `lumps()` shows up as a cost. **Decided at
  step 2: derived.** `lumps()` is B1's own code over the body; a body of
  one shell integrates its volume and casts no ray, so every fixture of
  C1 pays one Gauss integral per STEP write. Found at step 2: B1 needs
  a meeting test between shells (S5's face-pair test, nothing shared)
  before one vertex can decide containment, and a pair with no closed
  form is `Unchecked::ShellFacePair`; `ShellNestingFault::MultipleOuter`
  gives way to `VoidInVoid`, `OuterInOuter` and `Overlap` (**public enum
  changes**), and `Lump`, `LumpError`, `lumps` are new.
