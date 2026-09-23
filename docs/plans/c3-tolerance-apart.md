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
  cycle's marcher. Step 6 makes the boolean stop *asking* where the
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
- **Section edges' pcurve ends** (step 2b, ⚠ OPEN 6): a section
  edge's pcurve on a face ends on its vertex's own (u, v) there; the
  edge's tolerance raised by the move. ADR-0022 records it with step
  2's relation. No public signature change expected; `SectionVertex`'s
  doc if it gains its (u, v) per face.
- **`arris_geom` conic hits** (step 3): a root of `conic2::trig2_roots`
  within its own rounding of a whole turn is reported at `0`, stated in
  `intersect_curve_surface`'s guarantee. No signature change.
- **`arris_geom::SectionBranch` of a torus** (step 5): no signature
  change; its documented continuity now holds through a turning point.
- **`arris_geom::SECTION_FIT_FRACTION` and `section::fitted`** (step 5b):
  the constant's guarantee changes — measured from the two surfaces *and
  from the exact branch* — a change to what a public item promises, named
  in the commit body; `intersect_surfaces`' rustdoc and
  `docs/DATA-MODEL.md` §Tolerances and §Curves restated in the same
  commit.
- **`boolean::pave::section_curve`, `coincident`, `common_block`**
  (steps 6 and 4): the `along` verdict asks the surfaces before it asks
  `curves_coincide`; a block-level verdict beside the whole-curve one. No
  public type changes expected; `CommonBlock`'s doc if step 4 makes one.
  Step 4 made none: the block is an `EdgeImage`.
- **`arris_geom::conic_crossings`** (step 4, new public function): the
  common points of two conics in planes that are not parallel, along the
  line of the two planes, tangent only at a double root to rounding.
  **`pcurve_on`'s plane arm** (step 4): exact only for a curve lying in
  the plane; a line or conic tilted from it by more than `tol.angular`
  has its projection fitted — a change to what a public function
  promises, `docs/DATA-MODEL.md` §Pcurves restated.
- **`Reason::NonManifold`** (step 7): its doc gains "a shell touching
  itself at a vertex"; the entity is the vertex. No new variant.
- **`arris_debug::prop::body::SEAM_CLEARANCE`** (step 4): removed from
  `singular_slice`'s spin; kept for a ball's tilt from `90°`, which is
  `BesideSingularity` by design. `QuadricPair`'s sparing in
  `boolean_prop.rs` removed (step 6). A new strategy `band_pair` and test
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

- [x] Step 1 **[2]** — Size the unknown: the band survey. `band_pair` in
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

  **Done 2026-09-22.** `band_pair` holds ten contacts (the nine named
  and `coaxial-cut`'s bore); `the_band_survey` runs 256 pairs, 10752
  booleans off their contact, and `the_band_at_the_fixtures_numbers`
  the same band unposed at each fixture's own numbers, where nearly
  every failure reproduces. The histogram (`tolerance_band.rs`' doc):
  flush 3430, generic 2612, designed refusals 462, **failed 4248** —
  a third of the booleans at ±¼ of a tolerance, three quarters at +1½,
  still a fifth at ±16 — and TangentHole's fuse and common fail at the
  contact itself. Only `coaxial-cut`'s bore is held across the band.
  By the mechanism the fault names (`mechanism` in the test, the steps
  as renumbered below): two points a tolerance apart (step 2) 157, a
  section edge ending where nothing else does (step 2 or 4) 1356, two
  curves crossing with no vertex there (step 4, the block along an
  operand edge) 459, a fit off its branch (step 5) 56, two splines of
  one section (step 6) 0 — and **none of steps 2 to 6's, 2284: the
  larger part.** Those are
  no verdict for two surfaces, a curve and a surface or a point and a
  surface a hair off parallel or tangent (`Unsupported` 498, torus
  sections called degenerate at a tangency 672, 69 more in
  `Fault::Geometry`), sliver faces the polygons do not resolve (560),
  the builder and the checker refusing the result (458, 64 of them at
  TangentHole's contact) and shells that meet (27). They are the
  "faces" level of this plan's goal with no mechanism among steps 2
  to 6: the next plan's subject, three backlog lines, and what the
  roadmap bullet's "Done" has to say this plan left — step 9 asserts
  what steps 2 to 6 make hold and lists the rest. The volume was past
  its bound 17 times, every one a posed pipe-elbow flush fuse at a
  radius off by a quarter or half a tolerance, the same either sign:
  the measurement's drift with the distance from the origin
  (`docs/BACKLOG.md`), not a boolean's.

  Twenty-one `regression/` fixtures with oracle values, one per
  distinct failure: `flush-union-a-tolerance-off` (2: EmptySubEdge,
  CommonBlock; 4: Turn; none: plane–plane), `boss-flush-tilted` (none:
  plane–plane; 2/4), `boss-flush-posed-gap-fuse` (none),
  `pin-in-bore-a-tolerance-off` (2/4; none: NoInterior, a checker
  panic), `coaxial-fuse-a-tolerance-off` (2: CommonBlock; 2/4; none:
  NoInterior, EdgeUses, circle–plane), `coaxial-fuse-tilted-seam` (4:
  Seam), `coaxial-fuse-tilted-frame` (none),
  `tangent-cylinders-a-tolerance-in` (2/4),
  `tangent-cylinders-overlap-common` (none: L4 slivers),
  `tangent-cylinders-tilted-fuse` (none: Lumps), `tangent-hole-fuse`
  (none: EdgeUses at the contact), `tangent-hole-a-tolerance-out`
  (none: NoInterior, plane–cylinder; 2/4),
  `tangent-hole-tilted-posed-cut` (5: S5),
  `tangent-outside-a-tolerance-in` (none: NoInterior; 5: the fit; 2/4),
  `tangent-outside-grown-common` (none: Hole),
  `tangent-outside-tilted-posed-common` (none: B1),
  `tangent-outside-tilted-posed-fuse` (none: the ellipse off its
  surfaces), `pipe-elbow-a-tolerance-off` (2: CommonBlock; 4: Turn;
  none: torus sections, plane–plane, circle–plane),
  `pipe-elbow-posed-fuse` (4: Turn), `edge-touching-tilted-cut` (2: E2,
  two ends 1.41e-7 apart on one vertex) and
  `edge-touching-tilted-posed-cut` (none: NotClosed). The tangent-hole
  three want `TangentContact` by name, as `tangent-hole` does and as
  `Reason::TangentContact` names a section curve tangent to a loop edge
  at a vertex; the three commons of slivers want a refusal, the oracle
  building no solid of them. An edge-on-edge overlap (`Unsupported`,
  plane against point) has no fixture: Open CASCADE builds a shell of
  odd Euler characteristic there too, so it is a backlog line alone.

  Open question 5's rule applied: step 2 stays first; of steps 4 to 6
  the block along an operand edge has the most failures (459, and a
  share of the 1356), the fit next (56), the section edge known by its
  surfaces none — so the old step 6 is now step 4, the fit step 5 and
  the surfaces step 6, the fit still before the surfaces. References to
  them in this plan are renumbered.
- [x] Step 2 **[3]** — Section vertices by closure. `merge` takes the
  components of the candidate points; to establish: the relation
  (pairwise within the larger of the two tolerances, or to a fixpoint
  where a grown ball reaches a further point), that the result does not
  depend on creation order (a test that permutes the hits), the numbering
  that keeps every blessed dump of a generic pose, the bound on a chain —
  measured as the largest vertex tolerance over the whole corpus and the
  boolean properties at 1000 cases, which must not move — and the refusal
  for two vertices of one operand in one component. Blocks between paves
  of one component are no blocks.

  **Done 2026-09-22.** The relation (open question 2, decided): two
  candidates are one point when their balls meet, `|p − q| ≤ tp + tq`,
  or they name one operand vertex — each ball the tolerance of the
  entities that made the point, never a grown one, so the relation is
  fixed before any merge and its components are the closure; the named
  operand vertices are balls of their own beside the points. Neither
  option the step offered merges the fixture: its three points are
  1.7e-7 and 2.4e-7 apart at 1e-7, no two within one tolerance, and a
  grown ball never starts growing. It is also the vertex–vertex
  interference Open CASCADE's builder uses. A touch joins a vertex when
  its component holds one; the crossings an off touch stands for are
  candidates, and the components are taken again with them. Numbering by
  first member in today's order, source and representative point as
  before. `components` is a pure function with two unit tests: the
  fixture's three points one component under every rotation of their
  order and its reverse, and the label the first member. The bound,
  measured: every boolean the suite runs at `ARRIS_PROPTEST_CASES=1000`,
  72710 of them, logged with every section vertex's point, tolerance
  and members before and after — **identical, all 72710**, so no vertex
  tolerance moved and no chain formed anywhere outside the band; the
  tripwire did not fire, and no diameter bound is needed. Two vertices
  of one operand in one component is `OpError::Tolerance` naming the
  second (`one_per_operand`); the suite never reaches it, and its fields
  name one vertex, so the variant is unchanged. Blocks between paves of
  one component: `pave_edges` already paves an edge once per vertex, and
  a section curve's paves at one vertex twice are a traced branch's two
  ends, a loop — nothing to add. `place` holds a section block inside
  the ball of a vertex it ends on to that vertex's tolerance, not the
  faces': the stretch of a block up to a seam crossing inside the ball is
  the vertex. Band survey at 256 pairs after the closure: failed 4248 →
  4174, designed refusals 462 → 553 (`TangentContact` 294 → 384), flush
  3430 → 3432, generic 2612 → 2593; by mechanism step 2 157 → 173,
  step 4 459 → 474, none 2284 → 2185.

  **Found:** the fixture does not pass. With its three points one
  vertex, the seam's face has the seam ending at its touch and the
  section edges at the crossing vertex, 1.22e-7 apart in (u, v): L2
  holds every junction to `parametric_tolerance`, and with L2 relaxed
  the mesh finds the vertex's (u, v) evaluating 1.22e-7 from its point,
  above the face's 1e-7. Exact curves cannot meet there: a vertex whose
  members lie further apart than a face's tolerance needs its pcurves
  to end on its own (u, v). Step 2b (⚠ OPEN 6); the fixture's move and
  the property band moved with it.
- [x] Step 2b **[3]** — The pcurves of a merged vertex meet on its own
  (u, v) (⚠ OPEN 6). A section vertex has one (u, v) per face per side
  of a seam — an operand edge's pave where one paves an edge of the
  face, else its point's projection — and a section edge's pcurve ending
  there ends on it, its edge's tolerance raised to what the move costs;
  the vertex's point preferring a member on an operand edge.
  `seam-a-tolerance-from-crossing-fuse` passes every corpus stage and
  moves to `boolean/` with its blessed dump (`fixtures:`, the body of
  `cross-cylinders-fuse` at −90°, 8/14);
  `a_turn_of_the_tool_about_its_own_axis_changes_nothing` loses the
  lower bound that keeps its turns out of the band 5.8e-6° to 1.1e-5°
  past ±90°, and
  `a_sliver_within_the_tolerance_of_the_other_wall_is_decided_at_its_section_edges`
  runs that band too. No generic pose's dump changes: every vertex the
  suite makes has its members within a face tolerance.

  **Done 2026-09-22.** Open question 6 decided, option A. A section
  vertex's (u, v) on a face is an operand edge's pcurve where one of the
  face's edges is paved there or ends on an operand vertex it holds, else
  its point's projection; none at a singular vertex of the face, a whole
  line of (u, v). A section edge's placed pcurve whose end lies further
  from it than half of L2's band — so any two ends there meet within the
  band — has that end moved there (`ending_on`: the end control points
  of a clamped spline; a line as the degree-1 spline, anything else
  fitted first as `pcurve_on` fits), and its residual measured again,
  plus `RELATIVE_ROUNDING` at the positions' scale: a tolerance that is
  exactly the move failed E4 by 7e-17 after the property moved the body
  home from 50 away. The vertex's point prefers a member on an operand
  edge, as the step asked — without it Arris's volume is exact but Open
  CASCADE, reading the STEP, gets 5e-7 too much. **Found:** with the
  point on the seam, the two ellipses were paved at its projection, off
  their own crossing, and below one tolerance the volume drifted
  linearly with the turn, 1.1e-7 at half a tolerance; a section curve is
  now paved at its section crossing's own parameter where the vertex
  holds one, and every volume is exact again. Tripwire measured over the
  suite at 1000 cases: 3802 ends moved, the largest 1.92e-7 and at most 0.66 of its vertex's tolerance — all but six in the band tests, those six one vertex of one `quadric_operands_obey_every_identity` pose, 6.3e-8 each. No blessed dump changed but the
  fixture's own, 8/14, its four section edges at v9 at 1.22e-7.

  The property does not lose its bound outright. The touch joins the
  crossing vertex while it lies within two tolerances of it, which is
  `R sin δ / sin ψ` at a crossing angle `ψ`, not `R sin δ`; past it the
  touch stands for two crossings of their own a few tolerances off, and
  a common keeps the triangle between them at some turns — L4, zero
  signed area; at a right angle from two tolerances to 2.6, not at every
  turn, the fuse never. That is step 1's sliver class, none of this
  plan's mechanisms: `regression/seam-two-tolerances-from-crossing-common`
  (Open CASCADE builds 5/8/5 there, the sliver left out, its volume
  4.7e-7 short — `counts_differ`, and the fixture's tolerances at 2e-7) and a clause on the
  sliver backlog line. The property now draws one turn beside the vertex
  with the touch a tenth of a tolerance to 1.9 from it — held to the
  through turn's counts — and one from ten tolerances over `R` as before;
  the band between is the new fixture's. The sliver test runs seven
  turns from half a tolerance to 1.9, each the body at ±90° with its
  counts, the volume within 1e-9.
- [x] Step 3 **[1]** — A conic hit at a whole turn is the hit at `0`.
  The snap in `arris-geom` where the hits are wrapped and sorted, within
  the root's own rounding and no tolerance; the periodic tests of
  `intersect_curve.rs` compare parameters plainly instead of in the turn
  metric; a boolean test whose closed edge is hit at its start vertex
  paves no block shorter than the edge's tolerance.

  **Done 2026-09-22.** `conic2::trig2_roots` wraps every candidate as
  before and, within `WHOLE_TURN_ROUNDING` of `2π` — `roots::
  POLYNOMIAL_ROUNDING`, the budget the quartic's own coefficients
  already carry from the dot products they were expanded from, not an
  invented number — reports it at `0` instead; `dedup` after the sort
  then collapses it with a root the quartic also found there. No other
  wrap site changed: `conic_plane`'s and `conic_cylinder`'s own touches
  are direct formulas, not `trig2_roots` roots, and the design delta
  named `trig2_roots` alone. Found by search, not by the band survey: a
  circle offset by `big − small` inside a cylinder, posed at the
  model's default scale and away from the origin, lands its touch a few
  `f64` units short of `2π` without the snap — reproduced as
  `a_tangent_a_rounding_short_of_a_whole_turn_still_reports_zero` in
  `intersect_curve.rs`, asserting `hits[0].t == 0.0` plainly, and
  failing without the fix. `ARRIS_PROPTEST_CASES=1000 cargo nextest run
  --workspace` is green at 1160 tests, `intersect_curve_surface.rs` at
  3000; `intersect_surfaces.rs`'s two failures at that count
  (`coaxial_pairs_meet_where_their_meridians_meet`,
  `random_plane_and_cylinder_agree_on_every_common_property`) reproduce
  unchanged on `main` before this step and are none of its mechanisms.

  **Found:** `land`'s `at_vertex` reads the hit's point, never its `t`,
  so `pave_edges` already excludes a hit on a closed edge's own vertex
  whichever side of the wrap it lands on — the pave model's block
  decision was never exposed to this. The periodic tests of
  `intersect_curve_surface.rs` (`expect_hits`'s `param_diff`) keep their
  turn metric: it exists for expected parameters written outside
  `[0, 2π)` by convention (`circle_plane_follows_the_case_table`'s
  `phase − half`, `a_conic_against_a_quadric_follows_the_case_table`'s
  `[t, −t]`), unrelated to this snap, and changing it risks those. The
  "periodic tests... compare plainly" clause is instead a new test,
  `a_pin_tangent_at_its_cap_circles_own_start_paves_no_short_block` in
  `arris-ops/tests/boolean.rs`, built on the same found pose: it reads
  `Interferences::paves` directly and asserts every block on every
  circle edge is no shorter than the edge's tolerance — true with or
  without the snap here, since `at_vertex` already guards it, but it is
  the guarantee's boolean-level witness the step asked for.
- [x] Step 4 **[3]** — A block that is an operand edge's.
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

  **Done 2026-09-23.** Open question 4 decided: both. The seam and the
  circle are one touch by depth against the circle's plane (8e-10), and
  so against the circle itself, since `intersect_curves` goes through
  that plane — no second face runs through the seam to resolve it by, so
  the touch is resolved into its far crossing first. Two conics in planes
  not parallel meet where the line of the two planes crosses the first,
  a quadratic along it: `arris_geom::conic_crossings`, new and public,
  its `tangent` a double root to `POLYNOMIAL_ROUNDING` and never a
  tolerance, asked by `resolve_touch` wherever `intersect_curves` gave a
  touch. It finds the fixture's far crossing 1.8e-4 from the pole, and
  the pole itself to 5e-12; re-asking `intersect_curves` at the model's
  `min_tolerance`, tried first, found it too but not one 3.5e-7 out,
  whose depth is 3e-14. Then the block verdict (`along_block`): a block
  every check point of which lies within the tolerance of a piece of an
  operand edge of either face between the same two vertices is that
  piece, asked before `inside_both` (along an edge of one face it is on
  that face's boundary, and a rim's block 5e-8 inside the wall's never
  passed it), and the piece is placed on the other face as an image.
  Three more mechanisms the band needed, all this step's: `pcurve_on`'s
  plane arm maps a line or a conic into the plane exactly only when it
  lies in it — the seam's last stretch on a face 25° off had a circle
  0.19 off as its pcurve — and fits a tilted one's projection, held to it
  in the plane (a fit held to the curve cannot close the curve's own
  distance from the plane: a seam turned 4.9e-8 off a meridian plane
  failed so); a hit merged into the section vertex that holds its edge's
  own end paves nothing, as a crossing already did; and a section's
  arrival at a pole past the face's (u, v) box — the far crossing inside
  the pole's ball, where the seam's touch joins the pole and is not
  resolved — ends on the box's edge, the seam's corner, `place` reading
  no length in a step of `u` inside a singular end's ball. The fixture
  moves with `counts_differ` (Open CASCADE cuts the seam at the pole
  alone, 2/2/2 to Arris's 3/3/2) and `measure_differs`: its own cut is
  1.21e-4 short in volume where it matches the closed forms to 1e-8 at
  k = 0 and 0.7, and its measure of Arris's STEP matches them to 6e-10 —
  the inertia derived, the ball's less the cap's, and checked against
  its k = 0.7 tensor to 1e-8. The rim test is
  `a_section_block_along_an_operand_edge_is_that_edges_piece`: a quarter,
  half and three quarters of a tolerance, cut and fuse clean at `Full`
  with the rim cut twice and the wedge's closed-form volume; the common
  is the wedge alone, under a tolerance thick — step 1's sliver class,
  not asked. `singular_slice` draws its turn round the whole turn and a
  quarter of the time 1e-7 to 1e-1 of a radian beside the seam; the
  property is green at 8000, and its two shrunk failures on the way are
  `a_circle_through_a_pole_a_hair_off_the_seam_arrives_at_its_corner`.
  An along block's piece is imaged once per face, whichever pair placed
  it first — a frustum's notch refilled had its hyperbola edges imaged
  twice on the slab's faces, `TangentContact`, until it was.
  `ARRIS_PROPTEST_CASES=1000 cargo nextest run --workspace` is green at
  1165 tests. No blessed dump changed but the fixture's own.

  **Found:** at a ball's tilt of 90° exactly the face holds the axis, and
  a turn 1.3e-7 off the seam's puts a meridian plane that near the
  seam's: the seam lies within the tolerance of the face over most of its
  length and the section bounds a lune a few tolerances wide beside it —
  `TangentContact` where the section leaves a vertex along the seam, and,
  posed 93 out, the seam's crossings at the poles (placed only to
  rounding over that angle) reading as a section beside a pole. Step 1's
  sliver class, none of this plan's mechanisms: the two poses are
  `a_meridian_plane_a_hair_off_the_seams_halves_the_ball`, ignored (a
  recipe cannot carry their bits: turned by axis and angle the pose
  builds), a clause on the sliver backlog line, and the strategy keeps
  that tilt's turn off the seam's band. Open CASCADE's own cut there
  returns the whole ball in two shells.
- [x] Step 5 **[3]** — A torus branch continuous through its turning
  points. `SectionBranch` promises `point` continuous over its domain;
  a branch of `trace_torus` is not, where two arcs meet at a turning
  point. Each arc finds `v` from `u` by a root along the tube circle,
  and at a turn the section is tangent to the line of constant `u`, so
  a rounding `ε` in the turn's `u` is `√ε` along the curve: the two arcs
  end 2e-8 to 4.3e-7 apart on metre-scale rings (`R/r` 2 to 100) against
  a plane, a drill and a sphere — above the tolerance on the thin ring.
  The ruled tracer has the same fold and does not jump, because a
  turning arc is evaluated from its turning point (`Arc1::angle`'s
  anchor, the discriminant as a difference from the turn, zero there by
  construction); the torus walk is handed the same anchor by `located`
  and ignores it. To establish: an arc's point beside a turn that both
  arcs agree on at the turn to rounding and that stays smooth in the
  branch's parameter — the anchored difference in the torus's implicit,
  or the walk in `v` (`root_u`) inside the turn's cell, whichever the
  numbers hold. `a_loop_closes_through_its_turning_point_to_rounding`
  in `trace_torus.rs` (landed ignored, with this finding) un-ignored;
  the torus properties in `trace_torus.rs` and `intersect_surfaces.rs`
  green at 1000; no blessed dump changed but a torus section's, staged
  as `fixtures:` with the reason. Found by step 5b's first attempt,
  below.

  **Done 2026-09-23.** The walk in `v`, not the anchored difference: the
  difference in the torus's implicit still evaluates `f` near the turn to
  its rounding, and a rounding in `f` there is its square root along the
  curve as one in `u` is. Inside a turning point's cell a graph ending
  there is evaluated along `v` (`torus_walk::Fold`): `v − v_T = a₁r +
  a₂r² + a₃r³` in `r = √off`, `off` the offset from the turn that
  `Arc1::angle`'s anchor gives to full relative precision, and `u` the
  root along that `v`, which the cell's `∂f/∂u` certificate makes unique
  and well conditioned. Both arms are `v_T` at the turn, one point; `a₁ =
  1/√c` from the fold `|u − u_T| ≈ c·(v − v_T)²`, one value for both arms,
  so the branch is smooth through the turn; `a₂`, `a₃` meet the walk
  along `u` in value and slope where it takes over. A cubic that does not
  rise all the way falls back to `v` linear in `r`; over the torus
  properties of `trace_torus.rs` and `intersect_surfaces.rs` at 1000
  cases, 125 900 folds, none fell back and no root along `v` was missed.
  **Found:** walked in `v` over the whole turning cell, the four torus
  fixtures' fitted loops took 1.5–4× their control points (119 → 197,
  57 → 259): the handover matches slope, not curvature, and a break that
  far out costs the fit. `FOLD_REACH`, a thousandth of a radian of `v`
  (or the cell, if nearer), puts it where it weighs nothing — at `10⁻²`,
  `10⁻³` and `10⁻⁴` every loop has exactly its blessed count — while the
  walk along `u` is good to `ε/10⁻³` there. Sampled at 20 000 points, the
  nine poses' branches have no curvature above today's and speed jumps
  as low or lower; the loops close to rounding.
  `a_loop_closes_through_its_turning_point_to_rounding` is un-ignored and
  green. The dumps of `ring-pin-cut`, `ring-slab-common`,
  `ring-corner-common` and `filleted-boss-drill-cut` are re-blessed: same
  structure and counts, every number within 4.4e-8 — the jump removed.
- [x] Step 5b **[2]** — The fit held to the exact branch.
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

  **Tried 2026-09-23, tripwire fired, stopped.** Either distance bounds
  today's surface term (a projection's distance is 1-Lipschitz, and the
  branch lies on both surfaces to rounding away from a singular point's
  reach), so the deviation is that distance alone. Measured at degree 5
  and the default tolerance, control points per loop, today → same
  parameter → curve distance (a Newton foot along the branch, the
  tangent a central difference): metre cylinder pairs (radii 1 to 2,
  crossing, skew, tilted) 61–77 → 55–79 → 57–81; a cylinder against a
  sphere and against a crossing cylinder at a smallest meeting angle of
  20°, 5°, 1°: 47/101, 97/201, 177/341 → 47/101, 109/221, 201/397 → the
  same within 4. Torus sections (`R/r` 1.1 to 100, against a plane, a
  drill, a sphere, a cone): 39–382 → up to 522, and a drill on the
  `R/r = 10` ring refused (`the periodic normal equations are
  singular`) — with both options, and the curve distance seconds a fit.
  Not the metric: the torus branch jumps at its turning points (step
  5 above), which the surface term never saw because the jump runs
  along the curve, and the old fits sit up to 1.9e-7 from the branch
  there. With the check parameters within 1e-3 of the domain of every
  join left out, the torus counts are back: 39–390 with the curve
  distance, 41–486 at the same parameter (the cone's loops +27–30%, the
  rest within 10%), the latter as fast as today. Neither doubles once
  the branch is continuous; OPEN 3 is decided on those numbers after
  step 5.

  **Done 2026-09-23.** Measured again with step 5's continuous branch,
  control points per loop, surfaces → same parameter → curve distance:
  metre cylinder pairs 60–129 → 60–137 → 60–133; a cylinder against a
  sphere and a crossing cylinder at 20°, 5°, 1°: 105/77, 129/133,
  133/235 → 117/79, 133/143, 153/263 → 119/79, 133/140, 141/257; torus
  sections (`R/r` 1.1 to 100 against all six kinds) 27–107 → 27–109,
  +32% at most → the same within 10%, at five to eight times the time.
  Neither doubles; OPEN 3 decided for the same parameter. The deviation
  is `|fit(t) − branch(t)|` alone, which bounds the surface term. Degree
  re-measured at 3/4/5/6 and recorded in `SECTION_FIT_DEGREE`'s doc: 6
  saves a third on smooth loops and costs an eighth on two cylinders at
  a small angle, so 5 stays. The property is `held_to_branches` and
  `two_regions_agree` in `intersect_surfaces.rs`, run from
  `common_properties_in` and the quartic test — 2000 parameters per fit,
  and the fits of a second region shifted by a third of the box within
  half a tolerance where both boxes reach; `arris-geom` and `arris-ops`
  green at 1000.
  `grazing-ball-bar-cut` passes S5 at `Full` and moved to `boolean/`;
  it states `area_rel` 1e-7 because the oracle's solid is the far side
  (a section edge at 2.84e-6, its area 3.1e-8 from the 33.1158453338
  Arris converges to at model tolerances 1e-7 to 1e-10), as
  `ring-corner-common` does. Twelve blessed dumps re-blessed
  (`ball-offset-drill-cut`, `ball-polar-drill-cut`,
  `filleted-boss-drill-cut`, `frustum-cross-drill-cut`,
  `ring-corner-common`, `ring-pin-cut`, `ring-slab-common`, the five
  `tee-unequal-*`): same topology and tolerances, vertices and ranges
  within 1e-8, fits −4% to +35% control points.
- [x] Step 6 **[2]** — A section edge known by its surfaces. In
  `section_curve`'s `along` verdict, and wherever else `pave` asks whether
  an operand edge lies along a section (`coincident`, `common_block`,
  `resolve_touch`): an edge of face `fa` whose other face in its own
  operand lies on a surface `Coincident` with `fb`'s is on `fa ∩ fb` to
  its own tolerance (E4), so it is along the branch its midpoint projects
  onto within that tolerance, and `curves_coincide` is not asked. Sound at
  a grazing pair because of step 5b. `frustum-stub-cut-then-fuse` moves to
  `boolean/` with oracle values; the frustum–cylinder sparing in
  `quadric_identities` is deleted and the property is green at 1000.

  **Done 2026-09-23.** `known_by_surfaces` in `pave`: an edge of a
  pair's face whose other face in its own operand is `Coincident` with
  the pair's other face, by that face pair's own verdict, is on the
  section, and along a branch exactly when its midpoint projects onto it
  within the tolerance — `curves_coincide` is asked only without such a
  face. Asked in `section_curve`'s whole-curve verdict and in
  `resolve_touch` (an edge along the section curve crosses nothing
  there); in `crossings`, two edges of a coincident pair whose other
  faces lie on one surface are `same_curve` the same way, which feeds
  `matching_block` and `common_block`. `frustum-stub-cut-then-fuse`
  moved to `boolean/`: its counts are the oracle's, and it states volume
  1e-8, area 1e-7 and inertia 1e-8 because Open CASCADE's own solid is
  the far side — its section edges at 4.48e-7 in this fuse and in its
  direct `a ∪ b` alike, its volume 7.1e-9 relative from the
  0.6762668514840 Arris converges to, to 1e-12, at model tolerances 1e-7
  to 1e-10 — and `mesh_volume_rel` 7e-3, its own inscribed-chord bound
  at the stub's radius 0.106. The frustum–cylinder sparing is gone;
  `quadric_operands_obey_every_identity` green at 1000, and the
  workspace at 1000 cases green at 1168 tests. No blessed dump changed
  but the fixture's own.
- [x] Step 7 **[2]** — The pinch is refused by name (open question 1).
  `singular-bore-cut`: a result vertex whose face uses close into more
  than one fan is found in `result.rs` before `assemble` and returned as
  `OpError::Degenerate { reason: Reason::NonManifold }` naming the vertex,
  where it was `Internal(Builder(NotClosed))`. A test in
  `crates/arris-ops/tests/boolean.rs` asserts the refusal and its entity;
  the fixture stays in `regression/` holding the desired body, its
  `#[ignore]` reason restated; a backlog line for a shell that touches
  itself at a vertex, with the `General` body.

  **Done 2026-09-23.** `pinched` in `result.rs`, asked at the end of
  `shells` beside the two-lump checks: per result vertex, the corners of
  the kept pieces' loops there — a use arriving and the next leaving —
  joined wherever two share an edge piece; more than one fan is
  `Degenerate { reason: NonManifold }` naming the vertex as the other
  refusals do, a section vertex by the faces and edges that made it —
  here the main wall and the drill's, the singular point's section
  crossing. `a_wall_pinched_at_a_singular_point_is_non_manifold` asserts
  the refusal, its entities and the model unchanged; the fixture stays in
  `regression/`, its ignore reason restated; the backlog line is the
  `General` body's. `Reason::NonManifold`'s doc and ARCHITECTURE's
  boolean section and error table name the case. The check runs on every
  boolean: the workspace at 1000 cases is green at 1169 tests, so no
  other result closes into two fans.
- [ ] Step 8 **[1]** — ADR-0022, indexed in `docs/adr/README.md`, with
  the measurements of steps 2, 4 and 5b as its grounds and the amendment
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
  and 5b, which change them in their own commits.
- `docs/ARCHITECTURE.md` — the pave model's merge (components, the
  numbering rule), the section-edge verdicts of steps 4 and 6, the pinch
  beside `TangentContact`.
- `docs/BACKLOG.md` — step 7's line; step 9's residue; step 1's three
  lines for the classes none of steps 2 to 6 takes, trimmed by what
  steps 2 to 6 fixed of their fixtures; nothing else this plan absorbed
  returns.
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
- 2, **decided 2026-09-22** (step 2): the closure's relation is "the
  balls meet", with the entities' own tolerances; the bound, measured
  over the 72710 booleans of the suite at 1000 cases, is that nothing
  outside the band changes — the tripwire did not fire.
- 3, **decided 2026-09-23** (step 5b): the same parameter. Asked:
  same-parameter or curve distance to the branch; tripwire: control
  points more than doubled on the metre probes. It fired once on the
  torus probes from the torus branch's jumps at its turning points, not
  the metric; after step 5 the same parameter is at most +32% at
  today's speed and the curve distance today's counts at five to eight
  times the time. The same parameter is also the stricter bound and the
  one a property checks directly.
- 4, **decided 2026-09-23** (step 4): both — the curve–curve touch is
  resolved into its far crossing (`conic_crossings`, along the line of
  the two planes) and the block between the crossings is the edge's
  piece; ADR-0022's grounds record why (step 4's note).
- 6, **decided 2026-09-22** (step 2b): option A, the section edge's
  pcurve moved to the vertex's own (u, v), its tolerance by the move;
  the tripwire did not fire. What was asked:
  how a merged vertex's pcurves meet. Step 2
  found the tolerance model holds a vertex's (u, v) on a face to
  `parametric_tolerance` (L2) and its evaluation to the face's
  tolerance (the mesh), so a vertex whose members lie further apart
  than a face's tolerance cannot be met by exact pcurves. Options: (A)
  move the pcurve ends — a section edge's pcurve ends on the vertex's
  own (u, v), a fitted one by its end control point and an exact one
  refitted where it must move, its edge's tolerance raised by the move
  (the growth rule's own reason, `docs/DATA-MODEL.md` §Tolerances); (B)
  L2 and the mesh read the vertex's tolerance, converted to (u, v) —
  a checker tolerance widened, loops no longer closed in (u, v), which
  every polygon and winding number downstream assumes. Recommended: A.
  Tripwire: a generic pose whose dump changes, or a pcurve moved further
  than its vertex's tolerance, means stop and report.
- 5, **decided 2026-09-22** (the human delegated it): the agent reads
  step 1's histogram by a rule and does not wait. Step 2 stays first
  whatever it shows — the accept line names its fixture. Steps 4 to 6
  are taken in the order of the failures the survey attributes to each,
  most first, keeping the fit before the section edge known by its
  surfaces (that one is sound at a grazing pair only after the fit).
  Applied in step 1: the block, then the fit, then the surfaces —
  steps 4, 5 and 6 as now numbered. A class that is none of their
  mechanisms is fixtures and backlog lines, by the non-goal above,
  however large — and if it is the
  larger part of the band, the agent says so in step 1's commit body and
  in its reply, because that is the next plan's subject and the roadmap
  bullet's "Done" will have to say what it left. A band already held
  makes step 9 the assertion and the fixtures alone.
