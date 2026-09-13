# Plan: fillet-and-chamfer

- Started: 2026-09-13
- Milestone: C2, the application gate (docs/ROADMAP.md §C2, the line on
  single-edge fillet and chamfer, several edges in one call)
- Idea (verbatim from the human): "yes" — the idea's four decisions as
  recommended, the box edge and the miter first
- Idea: docs/ideas/fillet-and-chamfer.md (absorbed)

## Goal

`ops::fillet` and `ops::chamfer` blend a list of named edges of a solid in
one call: a constant-radius rolling-ball blend, or an equal-distance flat
chamfer. Each blend is built directly from its edge's two faces in closed
form, with no boolean tool (the idea's option B). Two planes blend to a
cylinder, and chamfer to a plane; a plane and a cylinder along a ruling
blend to a cylinder; along a circle they blend to a torus and chamfer to a
cone. Convex or concave is read from the dihedral angle, and the contact
curves come from the construction, never from the intersector. Each end
of a blend is trimmed by the face across the corner. Two blends at a
vertex whose third edge stays sharp meet in a miter; three blends at a
vertex of three planes meet in a sphere; a closed edge has no ends. Every
surface is exact, every result passes the checker at `Full` with nothing
unchecked and matches Open CASCADE's `BRepFilletAPI` on a `blend/`
corpus, and provenance is rooted at the edge, so the consumer names one
blend face per edge with no matcher. A second blend on a blended body
works. Everything outside that table is a typed refusal naming the edge,
and each such case is C6's.

## Non-goals

- C6's: tangent edge chains, vertex blends other than the three-plane
  sphere corner, a vertex of more than three edges, variable radius, a
  blend over a blend face, setbacks.
- Input faces on a cone, sphere, torus or NURBS surface; an edge between
  two cylinder faces.
- Chamfers with two distances, or a distance and an angle. The consumer
  passes one distance.
- The general quadric intersector: crossing cylinders of unequal radii,
  and plane–cone, plane–sphere, plane–torus in general position (C3).
- The cylinder–cylinder arms for parallel axes and for equal radii with
  crossing axes, and the checker's S5, B1 and `classify_point` arms
  against cone, sphere and torus faces. Both are plans of their own
  (§Dependencies); steps 7–9 here consume them.
- Swapping the consumer's facade and removing its twins (C2's accept
  line, after this plan).

## Design deltas

- **ADR-0007** (step 1): blends are rolling-ball stripes on analytic face
  pairs, one stripe per edge, the blend surface and its contact curves
  closed forms from a table; ends trimmed by the face across the corner;
  corners closed forms (the miter's ellipse from the construction, the
  sphere through the ball's one centre); everything else a typed
  refusal; the result assembled through `ops::rebuild` with untouched
  entities kept, and the `BlendTooLarge` bound. It names the
  reference-tree modules read — Open CASCADE's `ChFi3d` stripes,
  `ChFi3d_Builder_C2.cxx` for two stripes at a corner, `ChFi3d_Builder_
  CnCrn.cxx` for the n-corner it does not take, and `monstertruck-fillet`
  — and records the rejected boolean-tool option and why (the tangent
  contact ADR-0004 refuses; nothing of it survives into C6).
- **`arris-ops` public API (additions)**, in a new private `blend`
  module, re-exported from `arris-ops` and reached through the facade's
  `ops`:
  - `pub fn fillet(m: &mut Model, body: Body, edges: &[Edge], radius: f64) -> Result<(Body, Provenance), OpError>`
  - `pub fn chamfer(m: &mut Model, body: Body, edges: &[Edge], distance: f64) -> Result<(Body, Provenance), OpError>`

  An edge is passed as its handle, as every other operation takes its
  inputs; the orientation is ignored. The list is the consumer's
  selection in one call, in any order; the result and its ids are the
  same for any order of the same set (the blends are built in the body's
  iteration order of the edges).
- **`Reason` gains variants**, a breaking change named in each commit
  that adds one (the enum stays flat; grouping it by operation is a
  backlog line, not this plan's):
  - `NoEdges`: an empty list.
  - `RepeatedEdge`: an edge listed twice.
  - `EdgeNotInBody`: the id resolves but is not an edge of `body`.
  - `BlendTooLarge`: a contact curve or an end trim leaves its face
    through an edge that is not one of the corner's own (§Open
    questions).
  - `TangentChain`: the edge's two faces meet at a tangent dihedral, or
    the edge meets a blend face or a tangent edge at either end.
  - `VertexBlend`: a corner the closed forms do not cover — a vertex of
    more than three edges, three blended edges at a vertex not of three
    planes, or (until step 9) three blended edges at all.

  A radius or distance that is not finite or not positive is the existing
  `NonFinite` / `NotPositive` with `what: "radius"` or `"distance"`. A face
  pair outside the table is `OpError::Unsupported` naming the two kinds
  and the two faces.
- **Provenance.** No new `Role`, by the idea's decision: every record is
  against `Origin::Entity` of the blended edge —
  - `Generated`: the blend face, its two contact edges, its end edges and
    every vertex it makes;
  - `Modified`: the edge's two faces, the faces across each end, and the
    corner's other edges the trim shortens;
  - `Deleted`: the edge and the corner vertices it consumes.

  A miter edge is `Generated` from both edges, so `generated_pair` finds
  it; a sphere corner face is `Generated` from its three edges; a chamfer
  corner line likewise from both. The body is `Modified` from the input
  body, the shell from its shell. `audit` holds on every result, and this
  goes in DATA-MODEL §Provenance.
- **Recipes and the oracle:**
  - Recipe ops `fillet` `{of, edges, radius}` and `chamfer` `{of, edges,
    distance}`. `edges` is a list of points, one on each edge: Arris
    selects the edge that `classify_point` answers `On(Edge)` for, and
    the oracle the nearest edge by `BRepExtrema`. The lint refuses a
    point that classifies to a vertex or a face, or that is within
    `probe` of two edges. (Naming an edge by its two faces' roles was the
    alternative; a point survives a second blend and a transform, and a
    role does not name a blend's own edges.)
  - `analytic.expect_error` gains `"blend-too-large"`, `"tangent-chain"`
    and `"vertex-blend"`, the refusals Open CASCADE builds a solid for.
  - `arris_debug::fixtures::DUMPED_AREAS` gains `"blend"`;
    `tools/oracle/oracle/recipe.py` builds both ops with
    `BRepFilletAPI_MakeFillet` / `MakeChamfer`.
- **ARCHITECTURE:** §Operations gains the blend paragraph (the table,
  ends, corners, refusals, assembly through `rebuild`, where it is
  `rebuild`'s second caller); §Errors the new reasons; §How a consumer's
  kernel facade maps on drops "(cycle 2)" from the `ops::fillet` /
  `ops::chamfer` row.
- **ROADMAP §C2** (step 1, with the ADR): the intersector and checker
  lines are narrowed to the positions a blend and M5's revolves make —
  plane–quadric with the plane perpendicular to the axis, cylinder–quadric
  coaxial, sphere against a cylinder through its centre, plane–sphere,
  line against each quadric — and the general pairs move to C3's line.
  §Fixtures' area list gains `blend/`.
- **Unchanged here:** DATA-MODEL §Curves' quadric-curve `⚠ OPEN` (moved to
  C3 by the roadmap edit, closed by no step here); `ops::rebuild`'s
  signatures, which a blend calls as the boolean does — a change it turns
  out to need is named in that step's commit.

### Dependencies

Two plans this one does not contain, in the order the steps need them:

1. **`cylinder-cylinder-booleans`** (idea open, recommendation B). Its
   parallel-axis arm, `Tangent` at `d = |R₁ − R₂|`, is what S5 needs to
   check a blend cylinder against the cylinder face it is tangent to along
   a ruling (step 7); its equal-radius crossing arm, two ellipses, is what
   S5 needs on the miter's two blend cylinders (step 2's fixture, moved
   at step 7). Neither arm is built here; step 2 takes the miter ellipse
   from the construction and leaves exactly one S5 row unchecked.
2. **The quadric checker arms** (unnamed yet; C2's second and third
   lines, narrowed by step 1): the intersector arms above, S5, B1 and
   `classify_point` dispatching them, and the three M5 revolves into the
   corpus. Steps 8–9 wait on it.

With two active plans at most, the human sequences these (§Open
questions); steps 1–6 need neither.

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness or
bound has to be established here.

- [ ] Step 1 **[3]** — **One convex box edge, end to end.**
  - `ops::fillet` with the plane–plane arm: the blend cylinder on the
    line where the two faces' offset planes meet, radius `r`, its frame's
    `X` from its axis toward the deleted edge so the contact rulings sit
    at fixed `u`; contact lines on both faces at distance `r` from the
    edge, each a `Line` pcurve on its plane and a ruling on the cylinder.
  - Each end trimmed by the face across the corner: that plane against
    the blend cylinder is a circle arc when the plane is perpendicular to
    the edge and an ellipse arc when oblique, exact on the plane and a
    line or a fitted pcurve on the cylinder (the oblique-section rule).
    The corner vertex goes; the corner's two other edges are shortened to
    the arc's ends; the face across takes the arc in its loop.
  - The two faces' loops rewritten with the contact edges; the result
    assembled through `ops::rebuild` with every untouched entity kept;
    provenance as §Design deltas.
  - Refusals: `NoEdges`, `RepeatedEdge`, `EdgeNotInBody`, `NotPositive`
    / `NonFinite` on the radius, `BlendTooLarge` (a 2-cube at `r = 1.5`;
    a plate thinner than `r`), `TangentChain` at a tangent dihedral (the
    slot's arc-to-line edge), `Unsupported` for a pair outside the table
    (a revolve's cone edge).
  - Recipe op `fillet`, the edge-by-point rule, `blend/` in the lint and
    `DUMPED_AREAS`; the oracle's `fillet` op.
  - Fixture `blend/box-edge-fillet`: a 2-cube, `r = 0.2`, volume
    `8 − (1 − π/4)·r²·2 = 7.98283185` (the consumer's number). Variants:
    a cap edge; the box under a `transform` into an oblique pose.
    Fixture `blend/box-oblique-end`: a box whose end face is not
    perpendicular to the blended edge (an extruded parallelogram), so
    the end trim is an ellipse arc.
  - ADR-0007 and the ROADMAP §C2 narrowing in this commit.
- [ ] Step 2 **[3]** — **The miter: the spike that decides the corner
  steps and the cylinder–cylinder plan's scope.**
  - Two blended edges at a vertex whose third edge stays sharp — the
    consumer's vertical-plus-cap pair, and two cap edges, which is the
    same solid rotated.
  - The two equal-radius blend cylinders have axes crossing at the ball's
    centre; the miter curve is the ellipse in their bisecting plane,
    written from the construction, its pcurve on each cylinder fitted by
    the oblique-section rule. Each cylinder's far end is trimmed as in
    step 1; the two faces of the sharp third edge are each trimmed by one
    blend, and that edge is shortened to the vertex where the two
    contact lines on the shared face cross — the ellipse's end.
  - Fixture `regression/fillet-miter` with its oracle values,
    `#[ignore]`d because S5 has no closed form for the crossing pair. A
    test in `arris-ops` holds `Level::Full` to exactly that one unchecked
    row and volume, area, counts and probes to the oracle's (a scratch
    fixture).
  - **Gate:** if the corner needs anything beyond the equal-radius
    ellipse — a face the ellipse leaves, a second ellipse arc inside a
    blend face — stop and return to the human before step 3.
- [ ] Step 3 **[2]** — **The rest of the plane–plane table, several
  edges in one call.**
  - A concave edge: the inner edge of an extruded L, where the blend adds
    material and the contact lines lie on the faces' far sides.
  - Several disjoint edges: the four vertical edges of the consumer's
    cube, volume `8 − 4·(1 − π/4)·r²·2 = 7.93132741`; the same set in
    reversed order dumps identically.
  - Fixtures `blend/l-inner-edge`, `blend/box-four-verticals`.
- [ ] Step 4 **[2]** — **Chamfer.**
  - `ops::chamfer` over the same arms: the blend a plane through the two
    contact lines at distance `d` from the edge, ends trimmed as a
    fillet's, now line against plane.
  - The miter's chamfer twin: two chamfer planes meeting in a line, fully
    checkable today, so the corner logic of step 2 is proven in the
    corpus before the ellipse arm exists.
  - Recipe op `chamfer`. Fixtures `blend/box-edge-chamfer` (`8 − d²·2/2 =
    7.96`), `blend/box-four-vertical-chamfers` (`7.84`),
    `blend/box-corner-chamfers` (a vertical and a cap edge).
- [ ] Step 5 **[2]** — **A second blend on a blended body, and the C6
  refusals.**
  - Fillet an edge whose faces an earlier blend `Modified` and whose
    ends are clear of it (the consumer's twin: side edges 0–1, then 1–2
    — the two never meet, the box's side faces do). Fixture
    `blend/second-fillet`; a test composes the two records with
    `Provenance::then` back to the extrude's `SweepPart`s.
  - `TangentChain`: an edge that meets a blend face at a vertex (edge
    0–1 filleted, then the cap edge of face 0 at the same corner).
  - `VertexBlend`: a plane–plane edge ending at a vertex of more than
    three edges (two boxes fused, the upper turned 45° about the vertical
    with a bottom corner on the lower's top edge: five edges meet there;
    the agent confirms the fuse builds it, else the fixture waits at the
    vertex of three blended edges); three blended edges at one corner,
    refused until step 9.
  - Each refusal a fixture with `expect_error`, since Open CASCADE builds
    every one of them.
- [ ] Step 6 **[2]** — **Property tests** (seeded, `prop_shards!`).
  - Operands: a box and an extruded L in a random pose; a random subset
    of their plane–plane edges with no vertex holding three of them and
    no edge adjacent to a concave one at a vertex; `r` (or `d`) drawn
    below the bound the faces allow.
  - Checker `Full` green with nothing unchecked; `audit` green; volume
    equals the input's less `(1 − π/4)·r²·L` per convex edge, plus it
    per concave one — `d²/2·L` for a chamfer — corrected for each miter
    by its closed form; blending then transforming equals transforming
    then blending by volume and counts; two runs dump identically.
- [ ] Step 7 **[2]** — **The ruling arm, and the miter into the corpus**
  (waits on `cylinder-cylinder-booleans`).
  - Plane against cylinder along a ruling at a non-tangent dihedral: the
    chord edge of an extruded D, a cylinder blend tangent to the D's arc
    face along a ruling and ending on the caps, convex; the same edge on
    a C-shaped profile, concave.
  - `regression/fillet-miter` into `blend/fillet-miter` with its dump.
  - Fixtures `blend/d-chord-edge`, `blend/c-chord-edge`.
- [ ] Step 8 **[2]** — **Closed edges** (waits on the quadric checker
  arms).
  - A hole's rim: plane against cylinder along a circle, a convex torus
    blend (its frame coaxial with the hole's, `X` the cylinder's so the
    seams share a plane, `v` over one quarter turn), and a cone chamfer.
    A boss's base: the concave torus. No ends; the torus's `u` seam edge
    is a tube circle at `u = 0`.
  - Fixtures `blend/hole-rim-fillet`, `blend/hole-rim-chamfer`,
    `blend/boss-base-fillet`, the closed forms by Pappus.
- [ ] Step 9 **[2]** — **The sphere corner** (waits on the quadric
  checker arms).
  - Three blended edges at a vertex of three planes: a sphere octant
    through the ball's centre, tangent to each cylinder along a great
    circle; its frame's `Z` along one cylinder's axis so the three
    contacts are the equator and two meridians, and the vertex where the
    two meridians meet is the pole — a degenerate edge, as a revolve's.
    Three chamfers at the same corner meet in a triangular plane.
  - `VertexBlend` narrows to the vertex of more than three edges and
    the corner not of three planes.
  - Fixtures `blend/box-all-edges-fillet` (12 edges, 8 sphere corners,
    volume `8 − 12·(1 − π/4)·r²·(2 − 2r) − 8·(1 − π/6)·r³` at `r =
    0.2`), `blend/box-corner-three-chamfers`.

## Acceptance

- The corpus run, `cargo nextest run -p arris --test corpus --run-ignored all`:
  every `blend/` fixture passing every stage — checker `Full` with nothing
  unchecked, counts and genus, the oracle's reading of the STEP, mesh,
  measure, probes, provenance accounting, dump — against
  `BRepFilletAPI`; `blend/fillet-miter` among them; the lint holding
  `blend/` to blessed fixtures only.
- The consumer's numbers at its scale: one edge 7.98283185, four
  verticals 7.93132741, one chamfer 7.96, four chamfers 7.84, a second
  fillet, a vertical and a cap edge in one call.
- Step 6's properties green at the configured case count; step 5's
  `then` composition green.

## Docs to update on completion

- `docs/ARCHITECTURE.md`: §Operations the blend paragraph; §Errors the
  new `Reason`s in the `Degenerate` row and `Unsupported`'s face pair;
  §How a consumer's kernel facade maps on, drop "(cycle 2)".
- `docs/DATA-MODEL.md`: §Provenance the blend records; §Curves the
  quadric `⚠ OPEN` pointing at C3 (step 1 moves it).
- `docs/ROADMAP.md`: §C2 the fillet line **Done** with its fixtures;
  §Fixtures `blend/` in the area list.
- `tests/fixtures/README.md`: the `fillet` / `chamfer` ops, edge
  selection by point, the new `expect_error` values.
- `tools/oracle/README.md`: the two ops.
- `docs/BACKLOG.md`: the overflow — `Reason` grouped by operation; two-
  distance and distance-angle chamfers; a fitted pcurve fallback on the
  quadrics for the oblique corner cases C6 meets.
- `AGENTS.md` current state: fillet and chamfer landed, ADR-0007.

## Open questions

None. The three raised at planning were decided 2026-09-13 (the human:
"decide yourself"):

- **Sequencing under the two-plan limit.** Steps 1–6 run first and need
  nothing. After step 2's gate holds, `cylinder-cylinder-booleans` is
  planned as the second active plan in its idea's option B (parallel
  axes plus the equal-radius ellipse arm — the idea's own decision is
  taken when that plan is written, and this plan assumes B) and retired;
  then the quadric checker arms get their plan; this plan stays active
  throughout and steps 7–9 follow each in turn. A `regression/` fixture
  is what carries a blend result across the wait.
- **The `BlendTooLarge` bound.** A contact curve or an end arc that
  leaves its face through any edge but the corner's own is refused by
  name, decided in the face's own (u, v) through `FaceDomain::side`. A
  blend that meets a third face while its contacts stay inside their
  faces — a hole nearer the edge than `r` — is not detected by the
  operation in C2: S5 catches it in the corpus, and the fixture that
  shows it is a `regression/` entry for C6. Written into ADR-0007 at
  step 1.
- **The ellipse pcurves of the miter and an oblique end** are fitted
  `Nurbs` on the cylinder by the boolean's oblique-section rule, at the
  blend edge's own tolerance, as `boolean/oblique-hole` already passes
  the corpus's 1e-9 with. No provision is made for raising it; a fit
  that misses the corpus is a finding, and the fixture that shows it
  waits under `regression/` with the cause named.
