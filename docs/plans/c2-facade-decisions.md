# Plan: c2-facade-decisions

- Started: 2026-09-15
- Milestone: C2, the application gate (docs/ROADMAP.md §C2, the line on
  `Model::retain`, the `f32` boundary and the origin-name helper)
- Idea (verbatim from the human): "decide yourself on options in facade
  idea, I just remind you our goal is a parasolid-grade CAD kernel that
  can be used by any downstream project (e.g. robocad and/or python
  binding lib)"
- Idea: docs/ideas/c2-facade-decisions.md (absorbed)

## Goal

The three kickoff `⚠ OPEN`s of C2's facade line are closed by ADRs taken
for any downstream project:
- `Model::retain` never renumbers a slot;
- the tessellation boundary is `f64`, the cast the consumer's;
- Arris owns no name grammar.

What a consumer cannot build without the kernel is now a contract. **The
outputs a record lists for one origin come in split order**, `Modified`
and `Generated` alike: an edge's pieces along its curve, and a face's
pieces by the origins that bound them. That order does not change under a
parameter edit that keeps which entities bound which piece; only a tie
between pieces bounded by the same origins falls back to geometry. It is proven on `provenance/` fixtures whose
variants move, resize and rotate the split without changing which
entities bound which piece, and by a property test in random poses.

## Non-goals

- Per-corner normals and (u, v) in `TriMesh`, face-local vertices, STL
  and OBJ. They are the queries and export plan's, on the roadmap line
  this plan edits (the idea's decision 4).
- A name grammar, a parser, or a typed lineage value (the idea's 3B, 3C).
- Attribute propagation through operations: a backlog line.
- Renumbering compaction; an `f32` accessor on `TriMesh`.
- An order among the outputs of a one-to-one record (`transform`, a blend's
  `Modified` faces, a sweep's roles): there is nothing to order.
- The consumer's adapter itself.

## Design deltas

- **ADR-0009** (step 1): Arris owns no name grammar. A consumer names from
  `Provenance`, and the kernel guarantees split order. It records the face
  rule step 1 establishes and the evidence, and defers attribute
  propagation (Parasolid's attribute definitions; the reference trees'
  FreeCAD element maps read for how an application builds names on a
  history).
- **ADR-0010** (step 4): `Model::retain` keeps slots sparse. A dense copy is
  `Model::import` into a fresh model, or the native format.
- **ADR-0011** (step 5): `TriMesh` stays `f64`. The cast is the consumer's,
  at its boundary.
- **`arris-topo::Provenance`, a change of public semantics** named in each
  commit that makes it:
  - `generated_from` and `modified_from` return an origin's outputs in the
    order the operation added them, deduplicated, and every operation adds
    pieces in split order. Today they are ascending by id, and ids follow
    assembly, which groups pieces by shell and lump.
  - `then` nests the order: the pieces of piece `i` of an origin come
    before those of piece `i + 1`. `mapped` keeps it.
  - `PartialEq` becomes order-sensitive.
- **Split order** (steps 1–2, DATA-MODEL §Provenance):
  - An edge's pieces ascend by their start parameter on the origin edge's
    own curve, a closed edge's from its range's start.
  - A face's pieces ascend by their **boundary key**: the sorted,
    deduplicated origins of the piece's boundary edges (an operand edge a
    boundary edge is a piece of, or the face a section edge is generated
    from), compared lexicographically. Those origins are the operation's
    inputs, whose ids a rebuild of the same upstream recipe repeats, so
    the key reads no output id and no geometry.
  - **A tie** (two pieces with equal keys) breaks by a representative
    interior point in the origin face's own (u, v), lexicographic, `u`
    from the face's seam. The one edit that can flip a tie moves a piece
    across that seam; ADR-0009 states the limit.
  - The same order for a `Generated` list of pieces, such as a cut's wall
    on a tool face that ends up in several pieces.
  - A section edge generated from a face pair is ordered along its own
    curve.
- **Fixtures.** A dump lists topology only, no record, and the assembly
  is untouched, so no blessed dump moves (step 1 finding; the plan
  assumed dumps might reorder).
- **ARCHITECTURE:**
  - §The model: the compaction `⚠ OPEN` closed by ADR-0010.
  - §Threading: the `f32` `⚠ OPEN` closed by ADR-0011.
  - §Tessellation: the "no `f32` output" sentence.
  - §How a consumer's kernel facade maps on: the naming row states
    ADR-0009 and the split order; the memoising row drops its `⚠ OPEN`.
  - §Open questions: three lines removed.
- **DATA-MODEL:**
  - §Provenance: the consumer-references `⚠ OPEN` closed by ADR-0009, and a
    paragraph on split order.
  - §Open questions: that line removed.
  - The quadric `⚠ OPEN` stays.

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness or
bound has to be established here.

- [x] Step 1 **[3]** — **A face's split order: the rule and its proof.**
  Done 2026-09-15, ADR-0009.
  - **Fixtures,** each with variants that keep the split's combinatorics,
    four as built (the plan named two and a possible tie):
    - `provenance/split-bar-cut`: a bar cut through a box, two lumps, top,
      bottom, front and back faces in two pieces each. Variants move the
      bar to either side of the centre and narrow it.
    - `provenance/split-frame-cut` (added): the same bar through a
      rectangular frame, so each X face of the bar survives in two pieces
      `Generated` from one tool face — the `Generated` list the design
      promises the same order for.
    - `provenance/split-cylinder-seam`: a slab through a rod along its
      axis, turned about it. **Finding:** a piece never contains a seam —
      the split's arrangement keeps the seam as a boundary, so a wall cut
      with the seam inside one region is *three* faces, on the oracle's
      side too. The plan's variants ("rotate the slab past the seam")
      would change the piece count, which no order survives; the variants
      turn within (0°, 180°) instead, the seam staying on one side.
    - The tie exists, on both closed walls: the two pieces on either
      side of a seam are bounded by the same origins. `split-cylinder-seam`
      holds one, and `provenance/split-cross-common` (the Steinmetz solid
      at three radii and lengths and two turns) holds one per wall.
  - **The test** (`crates/arris/tests/provenance.rs`): piece signatures
    from the neighbours' composed origins; a tied pair told apart by its
    side of the origin face's own frame (a world-axis sign failed on the
    turned tool, whose frame turns with it).
  - **Today's order held**: ascending by id passed every variant of all
    four fixtures — the walk's piece order and the lump order are
    combinatorial there — but nothing defined it. Against it, the key
    reorders eight boolean records: a face split across two lumps
    (`split-cut`, the frame) came in lump order, and the Steinmetz walls.
    No id and no dump changed.
  - **Gate:** did not fire; the key holds on every non-tied variant.
- [x] Step 2 **[2]** — **An edge's split order, and section edges.**
  Done 2026-09-16, under ADR-0009.
  - **Already true, now stated and tested.** An operand edge's pieces are
    cut at its paves, which come ascending by parameter, so the sub-edge
    index ascends along the edge's own curve from its range's start, and
    a closed edge's range is one interval across the seam;
    `Interferences::sections` is curve order and then along each curve.
    The three places that establish it (`Build::sub_edges`,
    `rebuild::write_provenance`'s edge loop, the section-edge loop) now
    say so and name the ADR. No code changed, so no dump moved and none
    was restaged.
  - **Fixture `provenance/split-edge-notch`**: two notches cut one after
    the other into the top-front edge of a box, the first splitting it in
    two and the second splitting one half — so the two cuts composed list
    three pieces of one edge, a list no single step makes. Variants slide
    the notches along the edge without swapping them, and narrow them.
    Four variants, every corpus stage green against Open CASCADE.
  - **The tests** (`crates/arris/tests/provenance.rs`): edge `k`'s
    signature — its two end vertices' origins through the whole recipe —
    equal across variants, on the notch recipe (the result step and the
    two cuts composed) and on step 1's four face fixtures; and a
    geometric one, the rule itself: edges of one origin that lie on one
    curve ascend by their range on it. Outputs on different curves are
    not compared — a face origin pairs with several faces of the other
    operand, and the pairs' own order separates them.
  - **Finding, the counterpart of step 1's:** within one operation the id
    order and the curve order coincide over the whole corpus today —
    ordering an edge's images by id leaves every fixture and every dump
    green. They part under composition, and the notch fixture separates
    them there: the piece the first cut left has a lower id than the two
    the second cut made and comes last, because it lies last along the
    curve. So the geometric test, not the variant test, is what holds the
    rule.
- [x] Step 3 **[2]** — **Property test** (seeded, `prop_shards!`).
  Done 2026-09-16. `crates/arris-ops/tests/provenance_prop.rs`, over
  `prop::body::bar_cut()` — a box in a random pose cut by one or two
  bars, each a slab thin along one axis of the box's own frame and past
  it in the other two, drawn from bands that keep the bars apart and off
  the box's ends.
  - **The second build is a real parameter edit**, not only a nudge: the
    bars moved and resized within their bands, *and* the box resized
    along each axis and posed for itself. Nothing a world axis decides
    survives that, which is what gives the property its teeth — ordering
    the pieces by a world coordinate fails every shard, while reversing
    the rule outright does not, since that is stable across builds.
  - **No id reaches the comparison.** Every input entity is named by the
    primitive it came from and its role, and a piece is described by its
    neighbours' labelled origins — a face's by the faces it shares an
    edge with, an edge's by its end vertices — so two builds in two
    models compare directly.
  - **Three properties:** the order equal between the builds and equal
    again when the second is built in the *first* model after a
    `Model::retain(&[])` that frees every slot it used; `then` nesting —
    the composed list of an origin is the blocks its first-cut pieces
    stand for, end to end, and nothing else; `mapped` through
    `Model::import` describing every origin's pieces as the record it
    came from does. Each fails when its own rule is broken (the tie-break
    put on a world coordinate, `mapped` reversing a list, `then`
    composing its pieces backwards).
- [x] Step 4 **[1]** — **ADR-0010: compaction keeps slots sparse.**
  Done 2026-09-16. The decision reads as one promise with two halves: a
  live id never moves, and a dead one never aliases.
  - `a_consumers_cycle_moves_no_live_id_and_aliases_no_dead_one` in
    `crates/arris-topo/tests/import_retain.rs`: two bodies built, one
    retained, a third built into the slots the other left. Every handle
    to the kept body resolves to the same entity after the refill; every
    handle to the dropped one is `NotFound` both before and after it, and
    the refilled ids are none of the ones the consumer holds; the third's
    ids and dump are the same on a second run of the same calls.
  - A doc test on `Model::import`: a model with holes imported into a
    fresh one gives a dense copy — ids from zero, no gaps, no bumped
    generations — with the `IdMap` from the old ones.
  - `Model::retain`'s rustdoc states ADR-0010 in place of the `⚠ OPEN`
    and points at `import` for a dense copy; `docs/ARCHITECTURE.md`
    §The model and the memoising row of the facade table likewise, and
    the §Open questions line is gone.
- [ ] Step 5 **[1]** — **ADR-0011: the tessellation boundary is `f64`.**
  - `TriMesh`'s rustdoc names the decision and shows the one-line cast
    at a consumer's boundary as a doc test.
  - The ARCHITECTURE §Threading and §Tessellation sentences.
  - The ROADMAP §C2 queries and export line gains per-corner normals and
    (u, v) with face-local vertices beside the watertight buffer. The
    backlog line on normals, (u, v) and `f32` goes, now on the roadmap
    and closed.

## Acceptance

- `cargo nextest run -p arris --test corpus --run-ignored all`: every
  `provenance/` fixture passes every stage in every variant. Restaged
  `boolean/` dumps pass, their oracle values unchanged.
- `cargo nextest run -p arris --test provenance`: split order equal across
  variants for faces and edges, the bolt pattern's chains unchanged.
- Step 3's property green at the configured case count; step 4's
  compaction test green.
- `grep '⚠ OPEN' docs/ARCHITECTURE.md docs/DATA-MODEL.md` lists only the
  quadric-curve question.

## Docs to update on completion

- `docs/ARCHITECTURE.md`:
  - §The model, §Threading, §Tessellation: the closed `⚠ OPEN`s.
  - §How a consumer's kernel facade maps on: the naming and memoising
    rows.
  - §Open questions: three lines gone.
- `docs/DATA-MODEL.md`: §Provenance, split order and the closed `⚠ OPEN`;
  §Open questions.
- `docs/ROADMAP.md` §C2: the `retain` / `f32` / origin-name line **Done**
  with ADR-0009 to 0011; the queries and export line with normals and
  (u, v).
- `docs/adr/README.md`: ADR-0009, 0010, 0011.
- `tests/fixtures/README.md`: `provenance/` fixtures asserting split order
  across variants.
- `docs/BACKLOG.md`:
  - attribute propagation through operations, for consumers without their
    own names (Parasolid's attribute definitions);
  - a typed lineage value, if a second consumer asks;
  - the normals, (u, v) and `f32` line removed (step 5).
- `AGENTS.md` current state: the three facade decisions, ADR-0009 to 0011.

## Open questions

None. The two raised at planning were decided 2026-09-15 by the agent
(the human: "decide on open questions yourself"):

- **The face split-order rule** is the boundary key, with a (u, v)
  tiebreak (§Design deltas). A (u, v) order alone flips whenever a piece
  crosses its face's seam. A boundary key alone cannot order two pieces
  bounded by the same origins, and nothing without geometry can. The key
  takes every case it can; the tiebreak takes the rest, with its limit
  stated.
- **A `Generated` list follows the same order.** A cut's wall generated
  from a tool face is named by a consumer just as a `Modified` piece is,
  so an order guaranteed for one and not the other would be a trap.

Step 1 findings, for steps 2 and 3: outputs of different kinds from one
origin come in the operation's recording order (a tool face's wall
piece, then the section edges, then the vertices it generated), so the
guarantee is among outputs of one kind; step 3's property may include a
frame target for a two-piece `Generated` list; and a tie's side is read
in the origin face's own frame, never a world axis.
