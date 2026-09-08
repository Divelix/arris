# Plan: m4-booleans

- Started: 2026-09-07
- Milestone: M4 (cycle C1, docs/03-roadmap.md)
- Idea (verbatim from the human): "/plan m4" — the roadmap's M4 section is
  the brief; no idea file.

## Goal

`ops::{fuse, common, cut}` take two `Solid` bodies whose faces lie on
planes and cylinders and return a `Solid` that passes the checker at
`Full` with nothing unchecked, matching Open CASCADE's volume, area,
centroid, inertia, counts, genus and probe classifications on every
`boolean/*` fixture. The algorithm is the General Fuse decomposition in
Arris's own representation: every face pair is intersected by M1's table;
the intersection curves and every edge are *paved* by the points where an
edge of one operand pierces a face of the other, so the pieces are shared
between the operands by construction; every face is split in its own
(u, v) through the pcurves, each piece is classified against the other
body by a ray cast with the entity tolerances, and the surviving pieces
are assembled through the builder with every untouched entity keeping its
id. Coincident planar and cylindrical faces (flush) and tangent
plane–cylinder contact are decided by named cases, never by a tolerance
accident. The provenance record is built inside the algorithm — `Modified`
for split pieces, `Generated` for section edges and tool-face images,
`Deleted` for the swallowed rest — and the bolt-pattern rebuild names 8 of
8 hole walls through the same chain in three parameter sets. `ops::
transform` moves a body rigidly with one-to-one `Modified` provenance, and
is how the property tests put operands in random poses.

## Non-goals

No cone, sphere, torus or NURBS *operands* (their intersections are C2 and
C3; a face on one of them is `OpError::Unsupported` naming the pair). No
cylinder–cylinder intersection curve beyond the coincident and coaxial
cases (the quadric-curve `⚠ OPEN` stays for C2). No result with more than
one shell — an enclosed cavity, a fuse of disjoint operands, a cut that
splits its target in two — each is a typed `Degenerate` in M4, never a
`General` body. No merging of same-domain faces after a fuse (C8; the
flush union keeps its coplanar neighbours, as Open CASCADE does). No
fuzzy tolerance, no tolerance nudge, no healing. No sheet or wire
operands. No sweeps (M5). No `Role` variant: a boolean generates from
entities, never from nothing.

## Design deltas

- **`arris-math`** (01 §Crates; backlog line "Move `Aabb`" picked up):
  `Aabb` moves down from `arris-mesh` unchanged, `arris-mesh` re-exports
  it, and it gains `intersects(&other) -> bool`, `inflated(by) -> Aabb`
  and `of_point(p)`. `wrap_turn`, today private to `arris-geom`, becomes
  `arris_math::wrap_angle(t) -> f64` (into `[0, 2π)`) with its single
  definition.
- **`arris-geom` — the intersections a boolean needs** (02 §Curves):
  - `intersect_curves(a, b, tol) -> Result<CurveIntersection, GeomError>`
    with `CurveIntersection::{Points(Vec<CurveCurveHit>), Coincident}` and
    `CurveCurveHit { ta, tb, point, tangent }`, hits ascending by `ta`,
    periodic parameters in `[0, 2π)`. Line–line by closed form. Every pair
    with a planar conic operand goes through the conic's plane:
    `intersect_curve_surface(a, plane of b)`, each hit kept when `b`'s
    projection of it lies within `tol.linear`, `tb` from that projection;
    a curve `Coincident` with the plane is the coplanar case, which has its
    own closed forms for line–circle, line–ellipse and circle–circle and is
    `Unsupported` for a coplanar circle–ellipse or ellipse–ellipse (no C1
    recipe makes one; C3 does). Any NURBS operand is `Unsupported`.
  - `intersect_curve_surface` gains **ellipse–plane** (a closed form in
    `tan(t/2)`, two crossings, one touch or none) and **ellipse–cylinder**
    (the circle–cylinder machinery generalised: the extrema of the radial
    distance through the quartic in `tan(t/2)`, crossings between them by
    bracketed Newton). Both are what an oblique section edge needs when it
    is tested against a third face.
  - `intersect_surfaces` gains the **cylinder–cylinder** arm as far as it
    has a closed form: `Coincident` when the axes coincide within
    `tol.angular` and `tol.linear` and the radii within `tol.linear`;
    `Empty` when coaxial with different radii; everything else stays
    `Unsupported` (C2 and its ADR). Never a wildcard.
  - **Bounds**: `Curve::bounds(range) -> Aabb` (a line's endpoints, a
    conic's extrema per axis, a NURBS's control hull over the range) and
    `Surface::bounds(uv: [Interval; 2]) -> Aabb` (closed forms on the
    analytic kinds, the control hull on a NURBS). A face's box is the
    union of its edges' boxes and its surface's over the loops' (u, v)
    bounds, inflated by the face's tolerance.
  - **`region2`** (02 §Pcurves): `Side::{Inside, Outside, Boundary}` and
    `point_side(polygons, p, boundary_tolerance) -> Side` — the checker's
    `face_side` moved down and the checker over it; `interior_point(
    polygons) -> Option<Point2>`, a point strictly inside the region
    (winding non-zero, farther from every segment than the polygons'
    chord deviation): the middle of the longest inside span of the
    horizontal through the region's mid-height, deterministic, `None`
    for a region without one.
- **`arris-topo` — assembly in the builder** (02 §Euler operators; ADR-
  0004, step 1): `Builder::assemble(model: &Model, tolerance, faces: Vec<
  FaceSpec>) -> Result<Builder, BuildError>` is the builder's second entry
  point beside `new` + operators, and the boolean's only one. A spec is
  `Keep` or `New`: `VertexSpec::{Keep(VertexId), New { point, tolerance }}`,
  `EdgeSpec::{Keep(EdgeId), New { geometry, start, end, tolerance }}` over
  `VertexKey::{Kept(VertexId), New(usize)}`, `FaceSpec::{Keep(FaceId),
  New { surface, orientation, loops: Vec<Vec<UseSpec>>, tolerance }}` with
  `UseSpec { edge: EdgeKey, orientation, pcurve: Curve2Id }` in
  *effective* orientation as the operators take it. A `Keep` face reads
  its loops from the arena and every edge and vertex it names is `Keep`
  by construction. `assemble` proves what the operators would have: every
  loop closes through effective vertices, every edge is used exactly
  twice with opposite orientation, no face twice, one edge-connected
  component, the Euler line closing at an integer genus — each failure a
  typed `BuildError` (`Disconnected`, `EdgeUses`, `SameDirection`,
  `LoopOpen`, …). `finish` retains the id of every slot still marked
  `Keep`, appending nothing for it, and appends the rest in slot order —
  structural sharing and the kept/modified distinction fall out of one
  rule. An operator applied to a kept slot drops its mark. Nothing else
  in the builder changes.
- **`arris-check` — the classifier** (01 §The checker): `pub mod
  classify` with `classify_point(&Model, Body, Point3) -> Result<
  Classification, ClassifyError>`, `Classification::{Inside, Outside,
  On(Shape)}` (the face, edge or vertex the point lies on within its
  tolerance). B1's ray cast, made public and complete: the eight fixed
  directions in order, each face's hits by `intersect_curve_surface`,
  a hit's `uv` decided by `region2::point_side`, parity of the crossings,
  a direction abandoned on a boundary, tangent or coincident hit; all
  eight abandoned is `ClassifyError::Undecided { body, point }`, never a
  guess. B1 calls it. The corpus runner's new **probe stage** holds it to
  the oracle's class for every probe of every fixture.
- **`arris-ops` — the operations** (01 §Operations, §Errors, §Facade):
  - `transform(m, body, motion: &Isometry) -> Result<(Body, Provenance),
    OpError>`: every curve and surface appended transformed, every pcurve
    id reused (parameter space does not move), every entity new in
    iteration order, `Modified` one-to-one, the body's kind kept.
  - `fuse(m, a, b)`, `common(m, a, b)`, `cut(m, target, tool)`, each
    `Result<(Body, Provenance), OpError>`; inputs verified as `measure`
    verifies its own (a shared `verify_input`), the whole operation in one
    transaction, the checker on the output in debug and `paranoid`.
  - `pub mod boolean` with `interferences(&Model, a, b) -> Result<
    Interferences, OpError>` — the pave model as a value the agent can
    print and draw: per face pair its `SurfaceIntersection`, the
    edge-on-face hits, the merged section vertices with their
    tolerances, the paves on every edge and on every section curve, and
    the section edges with their two pcurves. `Display` for the dump.
  - `Reason` gains `Empty` (no material: a `common` of disjoint operands,
    a target swallowed by its tool), `MultiShell { shells }` (the result
    has several shells: a disjoint fuse, a split target, a cavity — M4's
    "out"), `TangentContact` (a tangent ruling that would be interior to
    two result faces — `⚠ OPEN` 1). `Fault` gains `Classify(
    ClassifyError)`.
  - **Tolerances** (02 §Tolerances, the growth rule applied): a section
    vertex's tolerance is the largest of the tolerances of the entities
    whose hits it merges plus the spread of the merged points; a section
    edge's is the larger of the two faces' tolerances raised to a fitted
    pcurve's residual; a piece keeps its parent's; every vertex ≥ edge ≥
    face by construction; anything above `max_tolerance` is
    `OpError::Tolerance`.
  - **Provenance** (02 §Provenance): an entity of either operand is kept
    (same id, unrecorded), `Modified` into its surviving pieces (a face
    whose loop changed at all — even only by a split edge — is a new face
    `Modified` from the old), or `Deleted` when no piece survives. A
    section vertex is `Generated` from the edge and the face (or the two
    edges) whose hits it merges; a section edge `Generated` from both
    faces (`generated_pair`). In `cut` every entity of the tool is
    `Deleted`, and a tool piece that survives (reversed) is additionally
    `Generated` from the tool entity it is a piece of — the hole's wall
    from the tool's wall, as 02 says. A coincident piece kept once is
    `Modified` from the operand it is taken from and `Generated` from the
    other's face. The result's shell and body are `Modified` from both
    operands' in `fuse` and `common`, from the target's in `cut`.
  - **Selection** (01 §Operations, new table): a piece of A is kept in
    `fuse` when outside B, in `common` when inside B, in `cut` when
    outside the tool; a piece of B when outside A, inside A, and inside
    the target (reversed) respectively; a piece on a coincident face is
    kept once from A when the normals agree in `fuse` and `common`, once
    from A when they oppose in `cut`, and dropped otherwise.
- **`arris-debug`** (01 §Formats and tools): `corpus::build_step` fills its
  `transform`, `fuse`, `common` and `cut` arms and records each step's
  inputs for the accounting; `corpus::run` gains the **degenerate path**
  (a result whose `expected.degenerate` is set must fail with
  `OpError::Degenerate` and skips every later stage), the **expected-
  error path** (`analytic.expect_error: "multi-shell" | "tangent-contact"`,
  Arris's typed refusal asserted, the oracle's counts recorded but not
  compared — `⚠ OPEN` 3) and the **probe stage** (Arris's `classify_point`
  against the oracle's class within the fixture's `probe_abs`); `prop::
  body::{box_in, cylinder_in, overlapping_pair}` — a box and a cylinder
  in random poses whose axis passes through the box's interior so every
  case intersects; the `inspect` skill gains `interferences` as the
  thing to print and draw (`polyline_of` over each section curve) when a
  boolean fails. `crates/arris/tests/corpus.rs`'s unsupported-op test is
  retargeted at `extrude`.
- **Fixtures** (`tests/fixtures/README.md`; every new one with
  `expected.json` from `expected.py`, the corpus lint green, `#[ignore =
  "M4: step N"]` until its step): `transform/posed-cylinder` (the
  cylinder rotated 30° about `[1, 1, 0]` and translated); `boolean/boss`
  (box ∪ cylinder r 4 at (20, 15) z 5…20: the wall crosses the top, the
  bottom cap is swallowed — 10/15/8/9, genus 0); `boolean/boss-flush`
  (the same cylinder z 10…20: its bottom cap coincident with the top —
  the same numbers); `boolean/oblique-hole` (the plate minus a cylinder
  r 3 tilted 30° about x through (20, 15, 5): two ellipse sections,
  NURBS pcurves on the wall — 10/15/7/9, genus 1, volume `12000 − 9π·10
  / cos 30°`); `boolean/posed-through-hole` (`through-hole` with both
  operands transformed by one motion: its numbers, centroid moved);
  `boolean/coaxial-cut` (cylinder r 2 z −1…1 minus a coaxial r 1: the
  tube, `sweep/revolve-tube`'s 4/6/4/6 and 6π); `boolean/coaxial-fuse`
  (the tube ∪ the inner cylinder: coincident walls vanish, the discs sit
  beside the annuli unmerged — 4/5/5/7, 8π); `boolean/disjoint-common`
  and `boolean/swallow-cut` (`degenerate: true`); `boolean/split-cut` (a
  slab through the plate: `expect_error: multi-shell`);
  `boolean/tangent-outside-cut` (a cylinder touching the face x = 40
  from outside: the plate unchanged, 8/12/6/6);
  `boolean/tangent-outside-fuse` and `boolean/tangent-hole` (a blind
  hole whose wall touches the face x = 0 along a ruling interior to it:
  `⚠ OPEN` 1). Every property-test failure becomes one more.
- **ADR-0004** — the boolean's structure on this representation: the pave
  model rebuilt over coedges (paves from edge-on-face hits shared between
  the operands, section curves paved by the same hits, blocks decided in
  (u, v)); faces split in their own parameter space through the pcurves
  and reassembled through the builder's new assembly entry with kept ids,
  rather than through an operator sequence (Mäntylä's split-and-glue, the
  path not taken and why); classification by ray cast with entity
  tolerances; coincident and tangent faces as named cases. Names the
  reference-tree modules read: Open CASCADE's `BOPAlgo_PaveFiller`,
  `BOPAlgo_Builder`, `BOPDS` (pave blocks, common blocks, face info),
  `IntTools` (edge/face with tolerances), `BRepClass3d` (the solid
  classifier); Mäntylä ch. 14; Hoffmann ch. 4 for the classification
  consistency argument. Step 1 (the assembly half), completed at step 7.

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness
or bound has to be established here.

- [x] Step 1 **[2]** — ADR-0004 and assembly in the builder.
  `Builder::assemble`, the `Keep`/`New` specs, `finish` retaining kept
  ids, the new `BuildError` variants. Tests: every `sample` body and both
  primitives assembled from their own entities as all-`Keep` finish to
  the same ids with nothing appended (arena lengths unchanged, dump
  byte-identical); the same as all-`New` dump identically up to ids;
  `sample::frame`'s face list assembled from scratch reproduces its dump;
  an edge used once, three times or twice the same way, a loop that does
  not close, two components, a kept face over a new edge, a `Reversed`
  face use inverted — each the named error and the model untouched;
  an operator applied after `assemble` on a kept slot drops the mark and
  `finish` appends it. ADR-0004 written and indexed.
- [x] Step 2 **[2]** — The intersections. `intersect_curves`, the
  ellipse–plane and ellipse–cylinder arms, the cylinder–cylinder
  coincident/coaxial arm, `wrap_angle` in `arris-math`. Tests (1000
  cases, `prop::geom` poses): every curve–curve hit on both curves to
  1e-12·scale and `ta`, `tb` the curves' own parameters; two lines
  against the closed form, a line and a circle in a random plane against
  the 2D closed form, two coplanar circles against theirs, skew pairs
  empty, a circle against itself `Coincident`; an oblique section
  ellipse of a random cylinder is `Coincident` with it and cuts a plane
  through its centre at the two closed-form points; ellipse–cylinder
  hits on both operands and a touch reported once; coincident cylinders
  in random poses `Coincident`, coaxial ones `Empty`, crossing ones
  `Unsupported` naming the pair. `geom/c1-intersections` gains the
  ellipse pairs and the oracle regenerated (`fixtures:` commit, body
  naming the recipe change), `crates/arris-geom/tests/oracle.rs` green
  on them.
- [x] Step 3 **[2]** — Bounds and the region toolkit. `Aabb` in
  `arris-math` with the new methods, `Curve::bounds`, `Surface::bounds`,
  `region2::{Side, point_side, interior_point}`, the checker's
  `face_side` over `point_side` (its tests unchanged). Tests (1000
  cases): 1000 samples of every curve and surface kind over a random
  range lie in the bounds, and a line's and a plane rectangle's are
  tight; `interior_point` of random star polygons with holes and of
  every face of every sample body has non-zero winding and is farther
  from every segment than the chord deviation; two runs identical.
- [x] Step 4 **[2]** — Point classification. `arris_check::classify`,
  B1 over it, `Fault::Classify`, the corpus runner's probe stage. Tests
  (1000 cases): random points against a box's and a cylinder's closed
  forms in random poses through `sample`/the primitives; a point on a
  face, on an edge, on a seam, at a vertex is `On` the right entity; a
  point inside a hole of `sample::frame` is `Outside`; every probe of the
  two `primitive/*` fixtures matches the oracle's class through the new
  stage; the checker's B1 tests unchanged.
- [x] Step 5 **[1]** — `ops::transform` and its fixture. The operation,
  the runner's `transform` arm and `Made::inputs`, `transform/
  posed-cylinder` un-ignored and blessed, the unsupported-op test
  retargeted at `extrude`. Tests: a transformed cylinder measures as
  `primitive_cylinder` at the transformed axis to 1e-12 relative and the
  oracle reads its STEP; the identity motion gives new ids and the same
  dump up to ids; motion then inverse returns every vertex to 1e-12·
  scale; mass properties are covariant at 1000 random poses; provenance
  is one `Modified` per entity in iteration order and nothing else; the
  checker green at `Full`; two runs identical.
- [x] Step 6 **[3]** — Interferences: the pave model. `ops::boolean::
  {interferences, Interferences}` as in the deltas: box reject per face
  pair; `intersect_surfaces` per surviving pair; every edge of each
  operand against every face of the other by `intersect_curve_surface`,
  a hit decided by `point_side` on the face's polygons, a boundary hit
  resolved to the edge or vertex within tolerance; hits merged into
  section vertices within the tolerance rule, in `(edge id, t)` order;
  paves on every edge; section curves paved by the section vertices
  that project onto them within their tolerance, the blocks between
  consecutive paves kept when their midpoint is inside both faces, a
  closed curve with no pave impossible on the corpus (the seam pierces
  it) and otherwise given its own vertex at the curve's parameter zero;
  section edges with pcurves on both faces by `pcurve_on`, translated by
  whole periods into the fundamental domain and split where they cross
  a seam — the seam's own hit is that pave; tolerances as designed;
  `Display`; the `inspect` skill. Tests: `through-hole` yields two
  section circles with one pave each at the seam's hits and no other
  hit; `blind-hole` one; `frame-cut` eight segments paved by the
  window's vertical edges; `corner-union` six; `disjoint-cut` and
  `tangent-outside-cut` none; `boss` one circle; `oblique-hole` two
  ellipses whose NURBS pcurves pass E4's check on the wall; 200 random
  `overlapping_pair` poses: every section point on both surfaces within
  the section edge's tolerance, every pave on its edge within the
  vertex's, every section edge's two pcurves same-parameter; two runs
  identical.
- [x] Step 7 **[3]** — Split, classify, assemble: `cut`. Per face, the
  arrangement of its loops' pieces and its section edges in (u, v):
  half-edges ordered around each vertex by the pcurves' tangent
  direction, ties by curvature, an unresolved tie a typed error; the
  regions walked, outer loops by positive turn, untouched holes assigned
  to the region containing them by winding; each piece's `interior_point`
  carried to 3D and classified by `classify_point`; the `cut` selection;
  the surviving pieces grouped into shells by shared edges, more than
  one shell `Degenerate { MultiShell }`, none `Degenerate { Empty }`;
  `Builder::assemble` with `Keep` for every untouched entity; the
  provenance as designed; the runner's `cut` arm, degenerate and
  expected-error paths; ADR-0004 completed. Fixtures un-ignored and
  blessed: `through-hole`, `blind-hole`, `frame-cut`, `disjoint-cut`,
  `corner-cut`, `bolt-pattern-8`, `swallow-cut`, `split-cut`. Tests:
  `through-hole`'s provenance table exactly — the top and bottom faces
  `Modified` into one piece each with a hole loop, the four sides kept,
  the tool's wall `Deleted` and its piece `Generated` from it, the tool's
  caps and rims `Deleted`, the seam `Deleted` and its piece `Generated`,
  two section vertices `Generated` from the seam and each cap plane, two
  section edges `generated_pair` of the wall and each cap; `frame-cut`
  counts equal `sample::frame`'s and its mass properties match to 1e-12;
  the accounting on every step of every un-ignored fixture; two runs
  identical.
- [x] Step 8 **[2]** — `fuse` and `common`. The other two selections,
  `Empty` for a disjoint `common`, `MultiShell` for a disjoint `fuse`.
  Fixtures un-ignored and blessed: `corner-union`, `corner-common`,
  `boss`, `posed-through-hole`, `coaxial-cut`, `disjoint-common`
  (`oblique-hole` stays ignored — step 14). Tests: a `fuse` keeps what
  neither operand touched, including a whole face of the *tool*, which
  a `cut` never does; `common` of the corner cubes is the unit cube's
  numbers; `posed-through-hole`'s dump equals `through-hole`'s up to
  the transformed numbers; disjoint operands refuse by name.
- [ ] Step 9 **[3]** — Random poses. `prop::body` strategies and the
  property tests at 200 poses of a box and a cylinder, both operand
  orders: volume additivity `V(A ∪ B) + V(A ∩ B) = V(A) + V(B)` and
  `V(A − B) + V(A ∩ B) = V(A)` to 1e-9 relative; `fuse` and `common`
  commutative — mass properties to 1e-9, counts equal, dumps equal up to
  ids; every result clean at `Full` with nothing unchecked. Every failure
  is shrunk to a fixture under `boolean/` with an oracle value and fixed
  within the step or left `#[ignore]`d with its reason. Runs at
  `ARRIS_PROPTEST_CASES=1000` once before the step closes.
- [ ] Step 10 **[3]** — Coincident faces. A `Coincident` pair puts the
  other face's edges on this face through `pcurve_on` (exact on a
  plane; a circle or ruling on a cylinder), paved by `intersect_curves`
  against this face's own edges and by the vertices already merged;
  pieces classified `On` are kept or dropped by the normals as the
  selection table says; the cylinder–cylinder coincident arm exercised.
  Fixtures un-ignored and blessed: `flush-union`, `flush-common`,
  `boss-flush`, `coaxial-fuse`. Tests: cut-then-fuse `V((A − B) ∪ B) =
  V(A ∪ B)` at 200 poses (`⚠ OPEN` 4); `flush-common` takes the
  runner's degenerate path with `Reason::ZeroThickness`; the S5 backlog
  line reviewed — closed if the arrangement gives S5 its exact overlap
  test, else left.
- [ ] Step 11 **[3]** — Tangent contact. A `Tangent` pair is a named
  case: a touch from outside contributes no section edge and no split
  (`tangent-outside-cut` is the plate); a ruling that would be interior
  to two result faces is `Degenerate { TangentContact }` naming them,
  unless `⚠ OPEN` 1 decides otherwise. The oracle's `expected.json` for
  `tangent-outside-fuse` and `tangent-hole` is generated in the step's
  first commit, before the code, and put to the human. Fixtures
  un-ignored and blessed per the decision. Tests: a cylinder placed
  tangent to a random box face in 200 poses is the box after `cut`, and
  `common` is `Empty`.
- [ ] Step 12 **[2]** — Provenance stability. `provenance/
  bolt-pattern-rebuild` run in all three variants (a test per variant,
  `dump.<variant>.txt`), and `crates/arris/tests/provenance.rs`: for each
  variant the eight cuts composed by `then`, each hole wall's `origins`
  ending at `Role::Cylinder(Wall)` through the tool step it came from,
  the chain identical across the variants; the plate's top face
  `Modified` through the chain from `Role::Box(Face(Z, Max))` into one
  face of nine loops; the whole chain associative against the oracle's
  counts. 8 of 8 walls is the roadmap's number.
- [ ] Step 14 **[2]** — A ruled face bounded by an oblique section.
  `arris-mesh` gives a cylinder or cone no interior grid (ADR-0003,
  "a plane, a cylinder and a cone are ruled and take none"), which holds
  only while the region's two boundary chains run parallel in the ruled
  direction; an oblique section's do not, and the CDT of that strip
  joins boundary points across the whole face — 1.2 rad of the cylinder
  in one triangle where the boundary is sampled every 0.048.
  `boolean/oblique-hole`'s mesh volume is out by 5e-3 where the
  inscribed prism bounds it at 4e-4. Interior points, a strip-aware
  triangulation or a restated guarantee — an ADR if the rule changes.
  Fixture un-ignored: `boolean/oblique-hole`; test un-ignored:
  `an_oblique_hole_meshes_within_the_inscribed_bound` in
  `crates/arris-mesh/tests/tessellate.rs`.
- [ ] Step 13 **[1]** — `parallel` over face pairs. `rayon` behind
  `arris-ops`'s feature for the face-pair intersections of step 6 and
  the per-face splitting of step 7, collected in pair and face order.
  Tests: every boolean fixture's dump and every property test's result
  byte-identical with the feature on and off (CI's `test` job runs both
  ways, as for `mesh`); the `wasm32` build on default features green.

## Acceptance

`ARRIS_PROPTEST_CASES=1000 cargo test --workspace` green, and the same
with `-p arris-ops --features parallel`: every `boolean/*` fixture and
`provenance/bolt-pattern-rebuild` un-ignored and passing every corpus
stage — the checker at `Full` with nothing unchecked, counts and genus,
the oracle's reading of the STEP, `measure` to 1e-9, the mesh closed
within `mesh_volume_rel`, the probes classified as the oracle classifies
them, the provenance accounting on every step, the dump — with only the
three `sweep/*` fixtures still ignored (step 14 un-ignores
`oblique-hole`); the property tests of steps 9
and 10 at 200 poses in CI and at 1000 once; `crates/arris/tests/
provenance.rs` reporting 8 of 8 walls under one chain in all three
variants; `uv run --project tools/oracle tools/oracle/selftest.py` green
over the new fixtures; the layer check and the wasm build pass; CI green
on `main`. Then tag `m4` (the human's).

## Docs to update on completion

- `docs/03-roadmap.md` §M4 — status line: date, what was retired (the
  pave model over coedges, splitting through the pcurves, assembly with
  kept ids, coincident and tangent faces as named cases), ADR-0004, the
  numbers; §Fixtures — the probe stage, the degenerate and
  expected-error paths; §C1 acceptance corpus — the new rows.
- `docs/01-architecture.md` §Crates — `arris-math` (`Aabb`,
  `wrap_angle`), `arris-ops` (`rayon`, `boolean`), `arris-check`
  (`classify`); §Operations — the booleans' decomposition and selection
  table, `transform`, `interferences` as a query; §Errors — `Empty`,
  `MultiShell`, `TangentContact`, `Fault::Classify`; §The checker —
  `classify_point`, B1 over it; §Geometry dispatch — the new arms and
  the partial cylinder–cylinder one; §Threading — `parallel` in `ops`;
  §Formats and tools — the runner's new stages, `prop::body`, the
  `inspect` additions; §Facade — the boolean and transform rows now
  real.
- `docs/02-data-model.md` §Curves — `intersect_curves`, the ellipse
  arms, the cylinder–cylinder arm with the `⚠ OPEN` restated; §Pcurves —
  `region2::{Side, point_side, interior_point}`, the bounds; §Euler
  operators — `assemble` and kept ids; §Tolerances — the boolean's growth
  rule as an example; §Provenance — the boolean's records and the tool
  rule, `transform`'s.
- `docs/adr/README.md` — ADR-0004 indexed (written at step 1).
- `tests/fixtures/README.md` — the new fixtures, `expect_error`, the
  probe stage, `dump.<variant>.txt` in use.
- `.agents/skills/inspect/SKILL.md` — `interferences`.
- `docs/BACKLOG.md` — the S5 line closed
  or kept (step 10); new findings appended; a line for coplanar
  conic–conic curve intersection (C3) and one for multi-shell results
  (C2).
- `AGENTS.md` current state — M4 done, next `/plan m5-sweeps`.

## Findings

Recorded by the step that met them; each is a departure from the design
deltas above, not a new decision.

- **Step 1 — `assemble` takes three lists, not one.** The deltas write
  `assemble(model, tolerance, faces: Vec<FaceSpec>)`, but
  `VertexKey::New(usize)` and `EdgeKey::New(usize)` index lists that have
  to be passed: the signature is `assemble(&Model, tolerance, Assembly)`
  with `Assembly { vertices, edges, faces }`.
- **Step 1 — `FaceSpec::Keep` carries the orientation.** A face entity is
  orientation-neutral; which side is material is the *shell's* use of it.
  `Keep(handle::Face)` — an id and the orientation the new shell uses it
  with — rather than `Keep(FaceId)`.
- **Step 1 — "a kept face over a new edge" is not expressible.** A kept
  face keeps every edge and vertex it names, by construction, so there is
  no such spec to refuse. The error tests that replaced it are
  `Duplicate` (one arena entity kept twice) and `NotFound` (a kept id that
  does not resolve).
- **Step 2 — the ellipse arms are the conic arms.** The deltas call for
  an "ellipse–plane closed form in `tan(t/2)`" and the "circle–cylinder
  machinery generalised". Both already were: the signed distance to a
  plane is `h + a(n·X) cos t + b(n·Y) sin t` and the radial distance to a
  cylinder is `|p + cos t·X + sin t·Y|`, neither assuming `|X| = |Y|`. The
  circle arms became conic arms taking `[a, b]`; no new solver.
- **Step 2 — a coplanar line and conic go through a second plane.** Rather
  than a 2D quadratic whose tangency would be decided in a scaled space
  where `tol.linear` is not a distance, the pair is the conic against the
  plane that contains the line and stands perpendicular to the conic's:
  the same points, and the touch decided by `conic_plane`.
- **Step 3 — `bounds` returns an `Option`.** A box has finite corners
  ([`Aabb`]'s contract) and a line's range may be `Interval::REAL`, so
  `Curve::bounds(range)` and `Surface::bounds([u, v])` are
  `Option<Aabb>`, `None` for a range that is not finite — the same shape
  as `Aabb::of_points`.
- **Step 3 — `interior_point` takes the clearance.** The deltas write
  `interior_point(polygons)` with the clearance being "the polygons'
  chord deviation", but a polygon built by `Polygon2::from_points`
  reports a deviation of zero while its caller may still want a margin,
  and a piece's polygons may come from several loops with different
  deviations. The clearance is the caller's argument.
- **Step 4 — random *poses* wait for `transform`.** The classifier's
  property tests run over random sizes and positions of `sample::cuboid`
  and `sample::cylinder`, which the samples place axis-aligned; random
  orientations arrive with `ops::transform` at step 5, where
  `transform/posed-cylinder` lands.
- **Step 4 — the classifier tries a periodic parameter a period either
  way.** A face's loops may be written in any translate of the
  fundamental domain — a wall piece spanning `u ∈ [1.5π, 2.5π]` after a
  split — while `Surface::project` answers in the first copy. The
  checker's own `face_side` (E8, L5, S5) does not shift and is left as it
  was; `classify` and B1, which share the code, do.
- **Step 5 — `transform` goes through `Builder::assemble`'s `New` specs,
  not `RawInsert`.** `Model::raw`'s own doc says it is "never an
  operation's path into the arena"; `assemble` (step 1) is the sanctioned
  one, already shaped for a caller with a full entity table. `transform`
  walks the body's closure, transforms every curve, surface and vertex,
  reuses every pcurve id, and rebuilds the loops with `UseSpec::
  orientation` composed from the face's effective orientation and the
  original coedge's — the same conversion `assemble`'s own `Keep` case
  applies — since that field is documented as the *effective* direction,
  not the coedge's raw one. This reaches exactly as far as `assemble`
  does: one shell, no free edges or vertices (every operation's output
  today).
- **Step 5 — `transform/posed-cylinder` did not exist to un-ignore.** No
  fixture directory was committed for it yet; this step authored
  `fixture.json` (r 4, h 12 cylinder, rotated 30° about `[1, 1, 0]`
  through the origin, then translated) and generated `expected.json`
  fresh.
- **Step 5 — the unsupported-op test builds its fixture ad hoc.** Pointing
  it at a committed corpus fixture stopped working once `transform`
  became supported (every corpus fixture's still-unsupported step is
  `cut`, `fuse`, `common` or `profile`, none of them `extrude` first);
  `an_unsupported_op_fails_with_its_name` now writes a two-step `box` then
  `extrude` recipe into a `tempdir`, the same pattern the dump-diff test
  beside it already uses, rather than adding a fixture to the corpus
  purely to exercise an error path.
- **Step 6 — the seam split *is* the seam's hit.** The deltas say a
  section edge's pcurve is "split where they cross a seam — the seam's
  own hit is that pave", and the second clause is the whole mechanism:
  the seam is an edge of the wall, its hit on the other face is a section
  vertex like any other, and a block between consecutive paves never
  contains a seam crossing in its interior — a crossing inside the other
  face is a hit there, one outside it means the whole block is outside.
  The pcurve is translated by whole periods so its point at the block's
  midpoint is the face's own (u, v) there, then held to the face's (u, v)
  box within the edge's tolerance; a block that still leaves it is
  `Fault::Seam` naming both faces, a kernel bug, never a split by
  bisection.
- **Step 6 — a touch pierces nothing.** An edge-on-face hit
  `intersect_curve_surface` reports `tangent` is recorded with its
  `Landing` but merged into no section vertex and paves nothing: the
  edge does not cross the face there, so no piece boundary lies there.
  `tangent-outside-cut`'s plate edges touch the wall at (40, 15, 0) and
  (40, 15, 10); both are in `hits` with `vertex: None`, and the wall ×
  side-face pair is `Tangent` with no section edge. Step 11 reads them.
- **Step 6 — the oracle splits a face touched from outside.** The
  deltas expected `tangent-outside-cut` to be "the plate unchanged,
  8/12/6/6"; Open CASCADE cuts the touched face x = 40 along the contact
  ruling and reports 10/15/7/7 (two vertices, the ruling and the two
  rim edges it splits, one more face and loop). The fixture's `analytic`
  keeps the volume, area, centroid and genus and leaves the counts to
  step 11, which decides whether Arris reproduces the split or refuses
  it through `expect_error` (`⚠ OPEN` 1's neighbour: a touch from
  outside is not the interior slit, and either answer is a manifold
  solid).
- **Step 6 — probe coordinates may be expressions.**
  `tests/fixtures/README.md` promised expressions "anywhere in `steps`,
  `probes` and `analytic`", and the Rust loader reads `Probe.point` as
  `Num`, but the oracle's `measure` cast a probe's coordinates with
  `float()`. `oracle/recipe.py` gained `probes(fixture, variant)`, used
  by `expected.py`, `compare.py` and `selftest.py`; `oblique-hole`'s
  on-wall and on-section probes are written as expressions in `r` and
  `tilt`.
- **Step 6 — `prop::body` and `corpus::inputs` landed here, not at
  step 9.** The step's 200-pose test needs `overlapping_pair`, so the
  three strategies are step 6's; a strategy yields a description
  (`Boxed`, `Cylindrical`, `OverlappingPair` with their `build(&mut
  Model)`) rather than bodies, since a model is not a `proptest` value
  and a failing case should print as numbers a fixture is written from.
  `arris_debug::corpus::inputs(dir, variant)` builds a fixture's recipe
  up to its result step and hands back the operands, so the fixture
  tests of the pave model read the corpus rather than restating it.
- **Step 6 — public additions beyond the deltas.** `Fault::Geometry(
  GeomError)` for a geometry query that fails on validated input for a
  reason other than a missing closed form (a `Degenerate` or `Fit`
  from the intersectors or `pcurve_on`), `Fault::Seam { face, other }`
  above, `Curve2::translated(Vec2)` with `Frame2::translated` and
  `NurbsCurve2::translated` underneath, and `crate::verify_input` shared
  by `measure`, `transform` and `interferences` (the deltas placed it at
  step 7).
- **Step 7 — the classifier let a surface's extension abandon a ray.**
  `Classifier::contains` gave up a direction whenever the ray's origin
  was on a face's *surface* (`t ≈ 0`), whether or not its (u, v) was on
  the face; `corner-cut`'s bottom face is classified at (0, 0, −1),
  which lies on the extension of two of the tool's planes, and all eight
  directions were abandoned. A `t ≈ 0` hit whose (u, v) is `Outside` the
  face is no crossing and is skipped; `Inside` or `Boundary` still
  abandons the direction.
- **Step 7 — `interior_point` chooses its height between vertex
  heights.** The mid-height horizontal of `corner-cut`'s L-shaped tool
  piece runs along one of its own segments and finds no span. The
  height is now the midpoint between two consecutive distinct vertex
  heights, nearest the mid-height first and outward when that level
  holds no span at the clearance, so the line never runs along a segment
  or through a vertex; the C-shaped doc example answers as before.
- **Step 7 — S5 rejects face pairs by their boxes.** The checker's S5 row
  asked the intersector for every face pair of a shell and reported
  `bolt-pattern-8`'s eight hole walls as unchecked (cylinder–cylinder,
  no closed form) although they are 26 units apart. A pair whose boxes
  — the edges' curve boxes and the surface's box over the loops, grown by
  the tolerances — are apart is decided empty first. The acceptance
  corpus's "nothing unchecked" is met by this, not by a wildcard.
- **Step 7 — the flush case is refused, not guessed.** A `Coincident`
  pair in the pave model, or a piece whose interior point classifies
  `On` the other operand, is `OpError::Unsupported` naming the two faces
  (the pair the kernel has no recipe for) until step 10 wires the
  normals rule; `flush-union` cut is the test.
- **Step 7 — nothing of the tool is kept.** The deltas' "every entity of
  the tool is `Deleted`" is taken literally: a tool face that survives
  whole (the floor of `blind-hole`, the tool's bottom cap) is a *new*
  face `Generated` from it with new edges and vertices, never the same
  id used reversed by the result's shell, so an entity is never shared
  between the tool body and the result and `Deleted` never names an
  entity the output holds. The target's untouched entities are shared
  by id as designed.
- **Step 7 — a re-tolerated vertex cascades.** A section vertex that
  coincides with an operand vertex and grows past its stored tolerance
  makes a new vertex `Modified` from it, which makes every edge at it a
  new edge `Modified` from the old and every face on those edges a new
  face — the only way the loops still close through one vertex slot.
  The cascade is built in; no cut fixture reaches it (the flush fixtures
  of step 10 do).
- **Step 7 — `swallow-cut` and `split-cut` are authored here.** Neither
  existed; `swallow-cut` is box [10,10,2]–[30,20,8] minus the plate that
  contains it (`degenerate: true`), `split-cut` the plate minus a slab
  [18,−1,−1]–[22,31,11] (`expect_error: multi-shell`, the oracle's two
  solids recorded: 10800, 4080, 16/24/12/12 in two shells).
- **Step 7 — public additions beyond the deltas.** `Fault::Split(
  SplitFault)` with `SplitFault::{EmptySubEdge, Dangling, Turn, Hole,
  NoInterior}`; `fixtures::ExpectError` and `Analytic::expect_error`;
  `CorpusError::Expectation` for a result step that built a body or
  failed otherwise when a refusal was expected.
- **Step 8 — the cut-then-fuse test is step 10's.** The deltas put
  "`fuse` of the through-hole result with its tool restores the plate's
  volume and counts" at step 8, but the drilled plate's hole wall is
  *coincident* with the tool's wall: the pair is the flush case, refused
  by name until step 10 wires the normals rule. `⚠ OPEN` 4 already puts
  the identity at step 10; step 8's fuse tests are `boss`'s reuse table
  (a whole face of the tool kept by id, which a `cut` never does) and
  the disjoint refusals instead.
- **Step 8 — a whole turn can round one ulp past its period.** The
  wrap-around block of a section curve with a single pave ends at
  `t + period`, and for a `t` that is not a small multiple of the period
  the sum rounds up, so the range is one unit in the last place longer
  than a turn and the checker's E1 rejects the edge —
  `posed-through-hole`, whose seam hit lands at `2π − 1 ulp` instead of
  `0`. `arris_math::period_end(lo, period)` is the range's construction:
  `lo + period` stepped down to the representable value that keeps
  `end - lo <= period`. No tolerance is involved.
- **Step 8 — `oblique-hole`'s probe sat on the wall's x-silhouette.**
  The `on_wall` probe was at `(20 + r, 15, 5)`, the point where the
  wall's normal is `+x`. Open CASCADE's `BRepClass3d_SolidClassifier`,
  reading the STEP Arris writes, does not report `ON` there — nor on the
  opposite ruling, at any tolerance up to 1e-2 — while it does on its
  own build of the same solid and on the six other angles tested; the
  two shapes' cylinders, radii and face boxes are identical, and Arris's
  own `classify_point` says `On`. The ON detection is a ray cast whose
  direction comes from the shape's first face, so it is sensitive to
  face order, which the two builds do not share. The probe moved to
  `(20, 15 + r cos(tilt), 5 − r sin(tilt))`, where the two agree, as
  `tests/fixtures/README.md` asks; `expected.json` regenerated.
- **Step 8 — `oblique-hole` meets an `arris-mesh` limit.** Its wall is
  the first cylinder face whose (u, v) region is a strip between two
  wavy chains, and ADR-0003's "a ruled surface takes no interior grid"
  does not survive it: the mesh volume is out by 5e-3 where the
  inscribed prism bounds it at 4e-4. The fixture stays `#[ignore]`d, the
  shrunk case is `an_oblique_hole_meshes_within_the_inscribed_bound` in
  `crates/arris-mesh/tests/tessellate.rs` with the desired assertion,
  and step 14 (added here) is the fix. `docs/BACKLOG.md` holds the line.
- **Step 8 — `coaxial-cut`'s tool is taller than its target.** The
  deltas write "cylinder r 2 z −1…1 minus a coaxial r 1"; a tool of the
  same height would put its caps coincident with the target's, which is
  step 10's. The tool runs z −2…2, giving the same tube — the coaxial
  pair is `Empty`, so nothing but the caps' section circles is decided.
- **Step 1 — `sample::sphere` is not assemblable.** Its two degenerate
  pole edges are used once each, which `finish` has always refused (it is
  why the sample goes through the raw insert). The round trip runs over
  every other `sample` solid and both primitives; `assemble` inherits the
  rule rather than widening it, and M4's operands are planes and
  cylinders.

## Open questions

All five decided by the human on 2026-09-07, the recommendation each time;
kept here so a step can cite the decision.

- `⚠ OPEN` 1 — **Interior tangent contact.** A hole wall touching a side
  face along a ruling interior to both leaves no manifold `Solid` with
  a shared edge (a slit). **Decided:** `Degenerate { TangentContact }`
  naming the pair, the fixture asserting it through `expect_error`;
  step 11 still generates the oracle's numbers for `tangent-hole` and
  `tangent-outside-fuse` first, as the record of what Open CASCADE builds.
- `⚠ OPEN` 2 — **Several shells.** A disjoint `fuse`, a cut that splits
  its target and a cavity all give more than one shell. **Decided:**
  `Degenerate { MultiShell }` for all three in M4 (the roadmap's "out");
  a `General` body for the disjoint fuse is C2's.
- `⚠ OPEN` 3 — **`expect_error` in the recipe.** A fixture whose oracle
  result exists but which Arris refuses by design (`split-cut`, the
  tangent cases) needs a key the lint and runner honour. **Decided:** the
  key, with the oracle's counts kept as information (step 7).
- `⚠ OPEN` 4 — **The cut-then-fuse identity.** **Decided:** `V((A − B) ∪
  B) = V(A ∪ B)` at step 10, and `V((A − B) ∪ (A ∩ B)) = V(A)` too if it
  costs one more test.
- `⚠ OPEN` 5 — **Where `classify_point` lives.** **Decided:**
  `arris-check` (it is B1's ray cast, and `ops` already depends on
  `check`); the facade re-exports it.
