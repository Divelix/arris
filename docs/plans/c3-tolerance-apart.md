# Plan: c3-tolerance-apart

- Started: 2026-09-22
- Milestone: C3, every quadric pair (docs/ROADMAP.md §C3: the last bullet,
  features a tolerance apart, and the accept line's
  `regression/seam-a-tolerance-from-crossing-fuse`; the last of C3's plans)
- Idea (verbatim from the human): "option B"
- Idea: docs/ideas/c3-tolerance-apart.md (absorbed)

## Goal

"The same within a tolerance" is an equivalence wherever the pave model
asks it, decided once per level, and no longer an accident of creation
order. **Points**: the section vertices of a boolean are the connected
components of its candidate points under "within tolerance", so three
points 1.7e-7 and 2.4e-7 apart at 1e-7 are one vertex whatever order they
were found in, and a conic hit one rounding under a whole turn is the hit
at `t = 0`. **Fits**: a fitted section is held to the exact branch its
tracer samples and not only to its two surfaces, so the edge's tube holds
the true section at every meeting angle and two fits of one section lie
within half a tolerance of each other. **Curves**: an operand edge whose
two faces lie on the pair's two surfaces is that pair's section by the
surfaces' identity, never by comparing two splines; and a block of a
section that lies within tolerance of an operand edge between the same two
vertices is that edge's block. **Faces**: operands a fraction of a
tolerance to several tolerances off flush, coaxial or tangent give the
flush body, the generic body or a named refusal, with the volume
continuous across the band — held by a property, its ends by fixtures
against the oracle.

When the plan is done: `seam-a-tolerance-from-crossing-fuse`,
`grazing-ball-bar-cut`, `frustum-stub-cut-then-fuse` and
`pole-slice-beside-seam-cut` have left `regression/` for `boolean/` with
blessed dumps; `singular-bore-cut` is a typed refusal naming its vertex
and no longer an `Internal` fault; `quadric_operands_obey_every_identity`
spares no pair and `singular_slice` keeps no clearance from the seam; no
blessed dump of a generic pose has changed; C3's accept line runs green
and the cycle can close.

## Non-goals

- A fuzzy value, a snap pre-pass, or S5 reading a lateral slack at a
  grazing angle: rejected by the idea (ADR-0004, `SEED.md` §9) and
  recorded as such in ADR-0022.
- `Curve::Nurbs` against `Curve::Nurbs` in `intersect_curves` and
  `curves_coincide` for two different splines in general: the NURBS
  cycle's marcher. Step 5 makes the boolean stop *asking* where the
  surfaces already answer.
- A pinched vertex admitted into a `Solid` (open question 1, decided):
  a data-model change with the `General` body, an idea of its own when a
  consumer's regression asks.
- A resolved tangent contact; `Reason::BesideSingularity` lifted (the
  polygon plan on the backlog); the tracers' `SectionFault` refusals; S5's
  coincident-surface grid (backlog); mass properties that depend on the
  body's position (backlog). A pose the band property meets that is one
  of these is a designed outcome there, as it is in the quadric property.
- NURBS operands, blends on quadric pairs, the STEP reader: C3 "Out".
- Anything step 1 or step 9 finds that is none of steps 2 to 6's
  mechanisms: a shrunk fixture under `regression/` and a backlog line,
  never a new step here.

## Design deltas

- **ADR-0022** (step 8): "the same within a tolerance is an equivalence,
  decided once per level". Records what steps 2 to 7 prove — the closure
  and its bound, the branch-held fit, a section edge known by its
  surfaces, a block that is an operand edge's, the pinch's ruling — and
  the alternatives the idea rejected. Follows ADR-0004, ADR-0006,
  ADR-0016, ADR-0021; **amends the fit target of ADR-0018 and ADR-0019**
  (a fraction of the tolerance of the two surfaces *and of the exact
  branch*). ADR-0018's tripwires and its option C stand.
- **`arris-ops` `boolean::pave`** (step 2): `vertex_near`'s first-come
  rule goes; `merge` builds every candidate point — hits, crossings,
  section crossings, singular points, resolved touches, the operand
  vertices they name — and takes components. A component is numbered by
  its first member in today's creation order and carries that member's
  `VertexSource`, so `Interferences`' `Display` and every blessed dump of
  a generic pose are what they were. Tolerance stays `base + spread`,
  `floor` and `max_tolerance` as today. A component holding two vertices
  of *one* operand would collapse that operand's edge and is
  `OpError::Tolerance` naming them — an existing variant; if its fields
  cannot name two vertices, the change is named in the commit body. No
  public signature change expected.
- **`arris_geom` conic hits** (step 3): a root of `conic2::trig2_roots`
  within its own rounding of a whole turn is reported at `0`, stated in
  `intersect_curve_surface`'s guarantee. No signature change.
- **`arris_geom::SECTION_FIT_FRACTION` and `section::fitted`** (step 4):
  the constant's guarantee changes — measured from the two surfaces *and
  from the exact branch* — a change to what a public item promises, named
  in the commit body; `intersect_surfaces`' rustdoc and
  `docs/DATA-MODEL.md` §Tolerances and §Curves restated in the same
  commit.
- **`boolean::pave::section_curve`, `coincident`, `common_block`**
  (steps 5 and 6): the `along` verdict asks the surfaces before it asks
  `curves_coincide`; a block-level verdict beside the whole-curve one. No
  public type changes expected; `CommonBlock`'s doc if step 6 makes one.
- **`Reason::NonManifold`** (step 7): its doc gains "a shell touching
  itself at a vertex"; the entity is the vertex. No new variant.
- **`arris_debug::prop::body::SEAM_CLEARANCE`** (step 6): removed from
  `singular_slice`'s spin; kept for a ball's tilt from `90°`, which is
  `BesideSingularity` by design. `QuadricPair`'s sparing in
  `boolean_prop.rs` removed (step 5). A new strategy `band_pair` and test
  file `crates/arris-ops/tests/tolerance_band.rs` (steps 1 and 9).
- **`tools/oracle`**: none expected; a band-end fixture whose counts the
  oracle gets wrong says so in `analytic.counts_differ` (ADR-0015).
- **DATA-MODEL:** §Tolerances (growth: a component of points; a fitted
  edge's tube holds the true section), §Curves (the fit target).
  **ARCHITECTURE:** the pave model's merge and the section-edge verdicts.
- **No crate boundary moves.**

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness or
bound has to be established here.

- [ ] Step 1 **[2]** — Size the unknown: the band survey. `band_pair` in
  `arris_debug::prop::body`: each designed contact the corpus holds —
  flush planes (`flush-union`, `boss-flush`), coincident and coaxial
  cylinders (`pin-in-bore-*`, `coaxial-*`), tangent walls
  (`tangent-cylinders-*`, `tangent-hole`, `tangent-outside-cut`), a tube
  circle (`pipe-elbow-fuse`), an edge on a face (`edge-touching-fuse`) —
  in a random pose, one operand moved off the contact by an offset along
  its normal, a tilt of that reach over the contact's extent, or a radius
  change, of ±¼, ½, 1, 1½, 2, 4 and 16 of the tolerance.
  `tolerance_band.rs` runs fuse, common and cut over it and *records*,
  not asserts: the outcome per perturbation — flush body, generic body,
  named refusal, `Internal`, checker-red — and the volume against the
  unperturbed body's. The property lands `#[ignore]`d with the histogram
  in its doc and in this plan under the step; each distinct failure is
  shrunk to a `regression/` fixture with its desired assertion and oracle
  values. Nothing is fixed here. The histogram orders what follows by
  open question 5's rule, and the agent rewrites the step list in the
  same commit if it changes.
- [ ] Step 2 **[3]** — Section vertices by closure. `merge` takes the
  components of the candidate points; to establish: the relation
  (pairwise within the larger of the two tolerances, or to a fixpoint
  where a grown ball reaches a further point), that the result does not
  depend on creation order (a test that permutes the hits), the numbering
  that keeps every blessed dump of a generic pose, the bound on a chain —
  measured as the largest vertex tolerance over the whole corpus and the
  boolean properties at 1000 cases, which must not move — and the refusal
  for two vertices of one operand in one component. Blocks between paves
  of one component are no blocks. `seam-a-tolerance-from-crossing-fuse`
  passes every corpus stage and moves to `boolean/` with its blessed dump
  (`fixtures:`, the body of `cross-cylinders-fuse` at −90°, 8/14);
  `crossing_cylinders_obey_every_identity` runs the band from 5.8e-6° to
  1.1e-5° past ±90° it skipped, if it skipped it.
- [ ] Step 3 **[1]** — A conic hit at a whole turn is the hit at `0`.
  The snap in `arris-geom` where the hits are wrapped and sorted, within
  the root's own rounding and no tolerance; the periodic tests of
  `intersect_curve.rs` compare parameters plainly instead of in the turn
  metric; a boolean test whose closed edge is hit at its start vertex
  paves no block shorter than the edge's tolerance.
- [ ] Step 4 **[2]** — The fit held to the exact branch.
  `section::fitted`'s deviation is the larger of today's surface term and
  the fit's distance from `branch.point` — to decide: at the same
  parameter, or to the branch as a curve (⚠ OPEN 3). Property in
  `arris-geom`: every traced pair of the existing strategies, the fit
  within `SECTION_FIT_FRACTION · tol` of the branch at 2000 parameters,
  and two fits of one pair over two regions within half a tolerance of
  each other. The control-point counts of `SECTION_FIT_DEGREE`'s doc
  measured again on the metre probes and at meeting angles of 20°, 5° and
  1°, recorded there. `grazing-ball-bar-cut` passes S5 at `Full` and
  moves to `boolean/` with oracle values; the blessed dumps of fitted
  edges that change are staged as `fixtures:` with the reason.
- [ ] Step 5 **[2]** — A section edge known by its surfaces. In
  `section_curve`'s `along` verdict, and wherever else `pave` asks whether
  an operand edge lies along a section (`coincident`, `common_block`,
  `resolve_touch`): an edge of face `fa` whose other face in its own
  operand lies on a surface `Coincident` with `fb`'s is on `fa ∩ fb` to
  its own tolerance (E4), so it is along the branch its midpoint projects
  onto within that tolerance, and `curves_coincide` is not asked. Sound at
  a grazing pair because of step 4. `frustum-stub-cut-then-fuse` moves to
  `boolean/` with oracle values; the frustum–cylinder sparing in
  `quadric_identities` is deleted and the property is green at 1000.
- [ ] Step 6 **[3]** — A block that is an operand edge's.
  `pole-slice-beside-seam-cut`: the seam and the section leave the pole
  2e-4 of a radian apart and cross again 1.8e-4 on — a touch in depth
  that stands for two crossings far apart in length, ADR-0016's case one
  level up, between two curves of one surface. To establish: the second
  crossing found exactly (two circles of a sphere crossing at 2e-4 are
  conditioned to about 1e-12) and paved on both; the section's block
  between the two vertices, every sample within tolerance of the seam
  between the same two, given to the seam as its block and not built as a
  section edge, so no face has a sliver L4 refuses; the arrival at the
  pole then the seam's own corner of the (u, v) box, which L2 accepts
  without changing. The same verdict for a section block along any
  operand edge, not the seam alone: a test with a rim circle beside a
  section ellipse a fraction of a tolerance apart over a stretch. The
  fixture moves to `boolean/`; `singular_slice`'s spin loses
  `SEAM_CLEARANCE` and `a_plane_through_an_apex_or_a_pole_cuts_additively`
  is green at 8000.
- [ ] Step 7 **[2]** — The pinch is refused by name (open question 1).
  `singular-bore-cut`: a result vertex whose face uses close into more
  than one fan is found in `result.rs` before `assemble` and returned as
  `OpError::Degenerate { reason: Reason::NonManifold }` naming the vertex,
  where it was `Internal(Builder(NotClosed))`. A test in
  `crates/arris-ops/tests/boolean.rs` asserts the refusal and its entity;
  the fixture stays in `regression/` holding the desired body, its
  `#[ignore]` reason restated; a backlog line for a shell that touches
  itself at a vertex, with the `General` body.
- [ ] Step 8 **[1]** — ADR-0022, indexed in `docs/adr/README.md`, with
  the measurements of steps 2, 4 and 6 as its grounds and the amendment
  of ADR-0018/0019's fit target stated as such.
- [ ] Step 9 **[3]** — The band held. `tolerance_band.rs` un-ignored and
  asserting: every outcome is the flush body, the generic body or one of
  the named refusals (`TangentContact`, `NonManifold`,
  `BesideSingularity`, `OpError::Tolerance`) — never `Internal`, never
  checker-red at `Full` — and the volume within the contact's area times
  the perturbation of the unperturbed body's. Step 1's regression
  fixtures that steps 2 to 6 fixed move to their areas; for each contact
  kind the two ends of its band — ¼ and 4 tolerances — become
  `boolean/*` fixtures with oracle values, `counts_differ` where the
  oracle merges what Arris keeps apart. What still fails is a
  `regression/` fixture and a backlog line each, listed in this plan
  under the step for `/retire-plan` to carry.

## Acceptance

`ARRIS_PROPTEST_CASES=1000 cargo nextest run --workspace` green, with:
the four fixtures named in the goal passing every corpus stage from
`boolean/` and the corpus lint green; `tolerance_band.rs` asserting, not
ignored; `quadric_operands_obey_every_identity` with no spared pair and
the singular-slice property with no seam clearance; `git diff` of
`tests/fixtures/**/dump.txt` against this plan's first commit showing only
the fixtures this plan names; and C3's accept line in full — the quadric
pair properties, the M4 identities over quadric operands, `blend_prop.rs`
with nothing unchecked, `docs/DATA-MODEL.md` with no `⚠ OPEN`.

## Docs to update on completion

- `docs/ROADMAP.md` §C3 — the last bullet **Done** with what it built and
  what it refuses by name; the status line; "Open:" gone. The cycle is
  then `/close-cycle`'s.
- `docs/DATA-MODEL.md` §Tolerances, §Curves — drift-check against steps 2
  and 4, which change them in their own commits.
- `docs/ARCHITECTURE.md` — the pave model's merge (components, the
  numbering rule), the section-edge verdicts of steps 5 and 6, the pinch
  beside `TangentContact`.
- `docs/BACKLOG.md` — step 7's line; step 9's residue; nothing else this
  plan absorbed returns.
- `docs/adr/README.md` — ADR-0022 (step 8 indexes it; check).
- `AGENTS.md` current state — C3's last plan done; next is
  `/close-cycle`, then the measuring harness and the reader.
- Rustdoc that cites this plan or `docs/ideas/c3-tolerance-apart.md` —
  `prop/body.rs`, `boolean_prop.rs`, `corpus.rs`' ignore reasons —
  re-pointed at ADR-0022.

## Open questions

- 1, **decided 2026-09-22** (the human delegated it): a pinched vertex
  in a `Solid` is refused by name, `Reason::NonManifold` with the vertex.
  A `Solid` is a manifold here at every level already — ADR-0006 refuses
  a vertex two lumps share and ADR-0004 a slit two faces touch along, and
  one shell touching itself at a point is the same statement; Euler's
  parity and L5 both read it as broken, so admitting it changes what
  every vertex-fan walk downstream may assume, for a pose that is exact
  tangency from inside, leaving a wall of zero thickness no consumer
  designs for. Open CASCADE builds the body, so it may
  come back as a consumer's regression: the fixture keeps the desired
  body in `regression/` and the backlog line says where it would go, the
  `General` body. ADR-0022 records this.
- ⚠ OPEN 2 (agent, step 2): the closure's relation and bound. Tripwire:
  if any corpus fixture or property case at a generic pose grows a vertex
  tolerance it did not have, stop — closure then needs a diameter bound,
  and the step reports the measurement before choosing one.
- ⚠ OPEN 3 (agent, step 4): same-parameter or curve distance to the
  branch. Tripwire: control points more than doubled on the metre probes
  means stop and report; the fallback is parking `grazing-ball-bar-cut`
  with the measuring harness, never an S5 excuse.
- ⚠ OPEN 4 (agent, step 6): whether the block verdict alone suffices or
  the curve–curve touch must first be resolved into its far crossing, as
  ADR-0016 resolves an edge–face touch; step 6 records which and why in
  ADR-0022's grounds.
- 5, **decided 2026-09-22** (the human delegated it): the agent reads
  step 1's histogram by a rule and does not wait. Step 2 stays first
  whatever it shows — the accept line names its fixture. Steps 4 to 6
  are taken in the order of the failures the survey attributes to each,
  most first, keeping 4 before 5 (5 is sound at a grazing pair only
  after 4). A class that is none of their mechanisms is fixtures and
  backlog lines, by the non-goal above, however large — and if it is the
  larger part of the band, the agent says so in step 1's commit body and
  in its reply, because that is the next plan's subject and the roadmap
  bullet's "Done" will have to say what it left. A band already held
  makes step 9 the assertion and the fixtures alone.
