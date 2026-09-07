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
- [ ] Step 3 **[2]** — Bounds and the region toolkit. `Aabb` in
  `arris-math` with the new methods, `Curve::bounds`, `Surface::bounds`,
  `region2::{Side, point_side, interior_point}`, the checker's
  `face_side` over `point_side` (its tests unchanged). Tests (1000
  cases): 1000 samples of every curve and surface kind over a random
  range lie in the bounds, and a line's and a plane rectangle's are
  tight; `interior_point` of random star polygons with holes and of
  every face of every sample body has non-zero winding and is farther
  from every segment than the chord deviation; two runs identical.
- [ ] Step 4 **[2]** — Point classification. `arris_check::classify`,
  B1 over it, `Fault::Classify`, the corpus runner's probe stage. Tests
  (1000 cases): random points against a box's and a cylinder's closed
  forms in random poses through `sample`/the primitives; a point on a
  face, on an edge, on a seam, at a vertex is `On` the right entity; a
  point inside a hole of `sample::frame` is `Outside`; every probe of the
  two `primitive/*` fixtures matches the oracle's class through the new
  stage; the checker's B1 tests unchanged.
- [ ] Step 5 **[1]** — `ops::transform` and its fixture. The operation,
  the runner's `transform` arm and `Made::inputs`, `transform/
  posed-cylinder` un-ignored and blessed, the unsupported-op test
  retargeted at `extrude`. Tests: a transformed cylinder measures as
  `primitive_cylinder` at the transformed axis to 1e-12 relative and the
  oracle reads its STEP; the identity motion gives new ids and the same
  dump up to ids; motion then inverse returns every vertex to 1e-12·
  scale; mass properties are covariant at 1000 random poses; provenance
  is one `Modified` per entity in iteration order and nothing else; the
  checker green at `Full`; two runs identical.
- [ ] Step 6 **[3]** — Interferences: the pave model. `ops::boolean::
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
- [ ] Step 7 **[3]** — Split, classify, assemble: `cut`. Per face, the
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
- [ ] Step 8 **[2]** — `fuse` and `common`. The other two selections,
  `Empty` for a disjoint `common`, `MultiShell` for a disjoint `fuse`.
  Fixtures un-ignored and blessed: `corner-union`, `corner-common`,
  `boss`, `oblique-hole`, `posed-through-hole`, `coaxial-cut`,
  `disjoint-common`. Tests: `fuse` of the through-hole result with its
  tool restores the plate's volume and counts; `common` of the corner
  cubes is the unit cube's numbers; `posed-through-hole`'s dump equals
  `through-hole`'s up to the transformed numbers.
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
three `sweep/*` fixtures still ignored; the property tests of steps 9
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
