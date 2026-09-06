# Plan: m3-tessellation

- Started: 2026-09-06
- Milestone: M3 (cycle C1, docs/03-roadmap.md)
- Idea (verbatim from the human): "/plan m3-tessellation" — the roadmap's M3
  section is the brief; no idea file.

## Goal

The agent can look at a body and the kernel can measure one.
`arris_mesh::tessellate(&Model, Body, chord)` turns any valid body into a
closed `TriMesh`: every edge discretised once at the caller's chord
tolerance and shared by the faces on both sides through the same-parameter
pcurves, every face triangulated in its own (u, v) by a constrained
Delaunay triangulation over `robust` predicates — a seam face through its
two seam pcurves, a face with holes through its inner loops, a pole through
its degenerate edge — with `FaceRange`s and `EdgeRange`s in the body's
iteration order and vertices on the geometry. `ops::measure::
mass_properties` integrates volume, area, centroid and inertia exactly over
the B-Rep with the (u, v) toolkit and matches Open CASCADE to 1e-9. The
corpus runner compares both. `arris_debug::render_body` draws a body to a
PNG the agent reads, and `arris_debug::rerun` streams it to a viewer for
the human.

## Non-goals

No adaptive or curvature-driven refinement beyond a chord tolerance: a
face's interior points, where it needs them, lie on a uniform (u, v) grid
sized by the chord bound, never by a per-triangle error estimate. No `f32`
output (the `⚠ OPEN` in 01 §Threading stays for C2). No per-vertex
normals or (u, v) in `TriMesh`, and no mesh-based mass properties (a
consumer integrates the mesh itself, 01 §Crates). No operation that
produces a body (M4, M5): `measure` is a query with no provenance. No
mesh in the native format or STEP. No mesh processing (decimation,
remeshing). No window in `arris-debug`.

## Design deltas

- **`arris-geom` — chord bounds** (02 §Surfaces, §Curves):
  `Curve::chord_segments(range, chord) -> usize`, the 3D twin of
  `Piece::segment_count` (a line one segment, a conic by `|d2| h²/8`, a
  NURBS by sampled second derivatives, the same minimum and maximum
  counts); `Surface::chord_steps(chord, uv_bounds) -> [f64; 2]`, the
  largest parameter step in `u` and in `v` for which a *triangle* inscribed
  in the surface deviates from it by at most `chord` — `INFINITY` along a
  flat or ruled direction (both on a plane, `v` on a cylinder and a cone),
  from the radius at the region's far bound on a cone, from `R + r` and `r`
  on a torus, from sampled second derivatives on a NURBS. Exhaustive over
  both enums.
- **`arris-topo`**: `Model::loop_pieces(&Loop) -> Result<Vec<Piece<'_>>,
  NotFound>` — the pieces of a loop in walking order, today
  `pub(crate)` in the checker; moved down because the checker,
  tessellation and `measure` all ask it. The checker calls it.
- **`arris-mesh` public API** (01 §Crates, §Facade; new section 01
  §Tessellation): `tessellate(&Model, Body, chord: f64) -> Result<TriMesh,
  MeshError>`. Step 2 as built: `MeshError::NotFound` wraps
  `arris_topo::NotFound` (a curve or pcurve is not a `Shape`), and
  `MeshError` drops `Eq` (it carries an `f64` and a `Report`);
  `Curve2::speed_bounds(range)` is the per-direction `|d/dt|` bound the
  edge sample count needs; on a sphere and a torus `chord_steps` shares
  the chord between the two directions so a triangle spanning both stays
  within it; the CDT inserts points in bit-reversed index order (a
  deterministic hierarchy, not a shuffle) because a loop's samples in
  walking order made insertion quadratic. The chord tolerance is the consumer's request — a number
  like the `f32` boundary, not a model tolerance — validated finite and
  positive. In debug builds the input passes `Level::Fast` first (that is
  why `mesh` depends on `check`), as every operation's does. `MeshError`
  gains `NotFound(Shape)`, `InvalidInput { body, report }`, `Chord(f64)`,
  `Face { face, source: CdtError }`. `arris_mesh::cdt::{triangulate,
  Triangulation2, CdtError}` — the 2D constrained Delaunay triangulation
  of polygons with holes plus optional interior points, public so the
  debug crate can draw a face's domain and a test can probe it. The mesh
  guarantees, written into the crate doc and 01 §Tessellation: positions
  are exact evaluations of the geometry (a topo vertex's point, a curve at
  a sampled parameter, a surface at an interior grid point); a topo vertex
  is one mesh vertex and an edge's samples are one index run, so a mesh of
  a `Solid` is closed by construction; a seam edge is discretised once and
  its indices appear in the wall's triangles twice; a degenerate edge's
  (u, v) segment maps to one index and the triangles that collapse are
  dropped; triangles are counter-clockwise seen from outside, by the face
  use's effective orientation against the surface normal; same body, same
  chord, same mesh on every platform, with the `parallel` feature on or
  off.
- **Step 3 as built.** Four things the design did not have. (a) The
  triangulation is taken in a (u, v) *scaled* by the surface's mean
  `|∂P/∂u|` and `|∂P/∂v|` over the region: the property test found a
  torus patch (`R = 7.6`, `r = 0.1`) whose triangles ran three steps
  along `u` to gain a little in `v` and deviated `1.2 ×` the chord — the
  empty-circle criterion measures distance, and the parameters are not
  distances. ADR-0003 carries the paragraph. (b) `MeshError::Unsupported`
  is **removed**: no surface kind is unsupported any more (a public
  enum's variant gone). (c) S2 and S4 skip **degenerate** edges: a
  sphere's pole is used once by the one face that closes on it, and
  counting it called every sphere an open shell. `docs/02-data-model.md`
  §Invariants rows S2 and S4 say so. (d) `sample::torus` (genus 1, two
  seams, no pole) and `sample::patch` (a rectangular `Sheet` of any
  surface kind over exact iso-curves, `SampleError::Region` for a region
  no iso-curve bounds) join `sample::sphere`; `patch` is what the
  property test meshes, since a body of every kind is the only way to
  reach `tessellate`.
- **Step 5 as built.** Three things the design did not have. (a)
  `integrate::region_integral` gains an `inner_step` argument (a public
  signature change) and `integrate::{inner_step, MAX_INNER_INTERVALS}`
  are new: the *inner* integral `∫_{u₀}^{u} f ds` was taken in one
  Gauss–Legendre interval, and one interval cannot carry a whole turn of
  a quadric's integrand — the cylinder's tensor came out 5e-12 relative
  off its closed form. A caller passes a quarter period on the quadrics,
  a knot span on a NURBS, `f64::INFINITY` on a plane; the checker's B2
  row passes the same. (b) The second moments are integrated about the
  centroid (a second pass over the faces with the point translated),
  not about the origin and carried by the parallel-axis theorem. (c)
  `arris_math::Matrix3` (the `nalgebra::Matrix3<f64>` alias, ADR-0001) is
  new, and a body whose faces enclose no positive volume — reachable
  only in a release build, where the checker does not run — is
  `Reason::NotPositive { what: "the enclosed volume" }` rather than a
  new variant.
- **`arris-ops` — `measure`** (01 §Operations gains the query shape,
  §Facade row): `measure::mass_properties(&Model, Body) ->
  Result<MassProperties, OpError>` with `MassProperties { volume, area,
  centroid: Point3, inertia: Matrix3 }` — unit density, the inertia
  tensor about the centroid in the physical convention (`∫ (|r|² I − r
  rᵀ) dV`, products of inertia carried negated on the off-diagonal) and
  `inertia_about(Point3)` by the parallel-axis theorem. Every quantity is
  a flux integral over the faces by Green's theorem through
  `integrate::region_integral`, with the face use's sign, as B2 already
  computes the volume: `x` from `x²/2 · n_x`, `x²` from `x³/3 · n_x`, `xy`
  from `x² y / 2 · n_x`. A body that is not a `Solid` is
  `OpError::Degenerate` with the new `Reason::NotSolid`; an invalid one
  `InvalidInput`. Not an operation: no new body, no provenance, no
  transaction.
- **`arris-debug`** (01 §Formats and tools): `mesh_of(&Model, Body) ->
  Result<TriMesh, DebugMeshError>` at `RENDER_CHORD_FRACTION` of the
  body's bounding-box diagonal (a named constant with the comment that it
  is a picture's resolution, not a tolerance); `render_body(&Model, Body,
  View, Option<Highlight>, path)` over it, edges from the mesh's
  `EdgeRange`s so the seam shows; `render_domain(&Model, Face, path)` — the
  face's loops and its triangulation in (u, v), the picture to read when a
  face's mesh is wrong; `sample::sphere` built through the raw insert with
  one seam and two degenerate pole edges, the checker's and the mesh's
  first body with E6 edges; `arris_debug::rerun::{log, spawn}` behind the
  `rerun` feature — a `Mesh3D` per face coloured by id under
  `<body>/faces/<id>`, a `LineStrips3D` per edge under `<body>/edges/<id>`,
  `Points3D` for vertices, so each can be toggled in the viewer. The
  corpus runner gains two stages: the mesh stage (closed, positive,
  signed volume within `tolerances.mesh_volume_rel` of the oracle's at
  `tolerances.mesh_chord`) and the `measure` stage (volume, area,
  centroid and inertia against `expected.json` within the fixture's
  `volume_rel`, `area_rel`, `centroid_abs` and the new `inertia_rel`).
- **The oracle and the fixture schema** (`tests/fixtures/README.md`,
  `tools/oracle/README.md`): `measure.py` records `inertia` — the 3×3
  matrix about the centroid, same convention — in every result;
  every committed `expected.json` is regenerated in one `fixtures:` commit
  whose body says the oracle script changed and no recipe did (the hash
  covers recipes only, so nothing goes stale). `fixture.json` tolerances
  gain `mesh_chord` (default 1e-3), `mesh_volume_rel` (default 2e-3) and
  `inertia_rel` (default 1e-9); the first two are read by Arris only.
- **Features and dependencies** (01 §Crates, §Threading): `arris-mesh`'s
  `parallel` becomes real — `rayon` (new workspace dependency) over faces
  after the sequential edge pass, results collected in face order so the
  output is identical either way; `arris-debug`'s `rerun` becomes real —
  the `rerun` crate (new workspace dependency, that feature only, never
  on `wasm32`).
- **ADR-0003** — tessellation: a constrained Delaunay triangulation of
  Arris's own over `robust`, in the face's (u, v), with edges discretised
  once by parameter and shared through the same-parameter pcurves, seams
  and poles handled as (u, v) copies of one mesh vertex, and interior
  points on a chord-bound grid rather than adaptive refinement; the
  acceptance bound on a mesh's volume is the closed form of the inscribed
  polygon, not a round number. Names the reference-tree modules read:
  Open CASCADE's `BRepMesh` (edge discretisation shared across faces,
  the Delaunay in `BRepMesh_Delaun`, the deflection model) and
  `truck-meshalgo` (what tessellation without pcurves does at a seam);
  Shewchuk for the predicates and the flip-based constraint recovery.
  Step 1.
- **Fixtures**: none new — every C1 recipe already carries the oracle
  values M3 compares against. The `boolean/frame-cut` twin (`sample::
  frame`) and `sample::sphere` are the hand-built bodies the mesh and
  `measure` tests use beyond the two live fixtures.

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical (Sonnet); **[2]** careful — a geometric or numeric case to get
right within a given design (Opus); **[3]** unproven — an algorithm whose
robustness or bound has to be established here (Fable).

- [x] Step 1 **[3]** — ADR-0003 and the constrained Delaunay
  triangulation. `arris_mesh::cdt::triangulate(polygons: &[Polygon2],
  interior: &[Point2]) -> Result<Triangulation2, CdtError>`: incremental
  insertion in input order into a bounding triangle — locate by a walk
  (a scan when the walk cycles), split the triangle or edge, Lawson's
  flips over strict `orient2d`/`incircle` — constraint recovery by
  removing the triangles a segment crosses and retriangulating the two
  pseudo-polygons Delaunay-wise (Anglada), interior points inserted after
  the constraints, exterior and hole triangles removed by an exact
  winding number counted by the constraints crossed from the bounding
  triangle (a sliver's centroid rounds onto its own edge, so no
  centroid), the bounding triangle's vertices dropped. Duplicate
  consecutive points are merged (`Polygon2::from_points`, new); two
  constraints that cross or coincide, a polygon of fewer than three
  distinct points, a point given twice, or a point on a constraint it
  does not belong to are typed errors naming the segments — the
  checker's L4/L5 promise a valid face never
  produces them. Tests (1000 cases): random star-shaped outers with random
  disjoint star-shaped holes and random interior points — every polygon
  segment is a triangle edge, every triangle is counter-clockwise by
  `orient2d`, the triangle areas sum to the outer's area minus the holes'
  to 1e-12·scale, every edge not on a polygon is shared by exactly two
  triangles and is locally Delaunay by `incircle`, no triangle's centroid
  has zero winding; a regular polygon at every count from 3 to 64; a
  square with a square hole; collinear runs of points on one segment;
  points differing by 1 ulp; two runs give identical index lists; a
  crossing pair is the named error. ADR-0003 written and listed in
  `docs/adr/README.md`.
- [x] Step 2 **[3]** — Edges once, faces through their pcurves: the box,
  the cylinder and the frame mesh closed. `Model::loop_pieces` moved down
  and the checker on it; `Curve::chord_segments`, `Surface::chord_steps`;
  `tessellate` for bodies whose faces are planes, cylinders and cones (the
  other kinds reach step 3 with an explicit `MeshError::Unsupported` arm
  until then — never a wildcard). The pipeline: every topo vertex one
  mesh position; every edge sampled at `n` uniform parameters, `n` the
  maximum of the 3D chord count and, per coedge, the count that keeps
  each step's `u`- and `v`-travel under the surface's `chord_steps`
  (bounded by `max |du/dt| · Δt`, so the bound is rigorous), the samples
  pushed once as that edge's `EdgeRange`; per face, each loop's polygon
  in (u, v) from the *same* parameters through each coedge's pcurve
  (seam copies a period apart, a degenerate edge as a segment), the CDT,
  the (u, v) vertices mapped back to the shared indices (interior points
  none yet), collapsed triangles dropped, orientation from the effective
  face use, pushed as the `FaceRange`; the debug-build input check;
  `MeshError`'s new variants. The corpus runner's mesh stage and the
  three fixture tolerance keys. Tests: `sample::{unit_box, cylinder,
  frame}` and both primitives — `is_closed`, `signed_volume` positive and
  within `4δ/(3 r)` relative of the closed form (exact for the box and
  the frame), one `EdgeRange` per edge in `edges(body)` order and one
  `FaceRange` per face in `faces(body)` order, the seam's indices used by
  the wall twice with opposite direction, every position within
  `default_tolerance` of its entity's geometry, every triangle's three
  positions within `δ` of the face's surface at the (u, v) they came
  from, the two-loop faces of the frame meshed with the hole open (the
  hole's edges each in two faces); 1000 random cylinders (radius, height,
  pose from `prop`) closed and within the bound; a chord of `0`, `−1` or
  `NaN` is `MeshError::Chord`; a raw-broken body is `InvalidInput` in a
  debug build; two runs of every case are byte-identical; the two live
  fixtures pass the new corpus stage.
- [x] Step 3 **[2]** — The doubly curved kinds and the sphere. Interior
  grid points at `chord_steps` spacing over the region's (u, v) bounds
  for `Sphere`, `Torus` and `Nurbs` (and any face whose boundary alone
  leaves a step larger than the bound), those inside the loops by
  winding, inserted through `cdt`'s `interior`; the `Unsupported` arms of
  step 2 replaced; `sample::sphere`. Tests (1000 cases): for every surface
  kind in a random pose from `prop::geom` and a random rectangular (u, v)
  region, the triangles' centroids and edge midpoints project onto the
  surface within `δ` (M1's projection) and the grid is empty on a plane
  and a cylinder; `sample::sphere` is clean at `Fast` and reports the
  plane–sphere pairs as `unchecked` at `Full`, its mesh is closed and
  its volume within the closed-form bound of `4πr³/3`, the two pole
  edges yield one index each and no collapsed triangle survives; a torus
  face and a bilinear NURBS face raw-built on a box-sized region mesh
  closed together with their neighbours.
- [x] Step 4 **[1]** — A picture of a body. `arris_debug::{mesh_of,
  render_body, render_domain}` and `RENDER_CHORD_FRACTION`; the `inspect`
  skill updated to say the body step exists. Tests: the cylinder's `Iso`
  render has exactly three face colours plus line, dot and background in
  its histogram and a run of black pixels crossing the wall between the
  caps (the seam), `Highlight::Edge(seam)` turns that run red,
  `Highlight::Face(wall)` turns the wall red; the box shows three faces
  from `Iso` and one from `Top`; `render_domain` of the wall shows a
  rectangle `[0, 2π] × [0, h]` fully triangulated and of the frame's top
  face an outer with a hole; every PNG lands under `target/inspect/`. The
  agent reads the cylinder's PNG and states what it sees in the commit
  body (the roadmap's accept).
- [x] Step 5 **[2]** — `measure`, the oracle's inertia, the corpus stage.
  `mass_properties`, `MassProperties`, `Reason::NotSolid`; `measure.py`
  records `inertia`, every `expected.json` regenerated (`fixtures:`
  commit, body naming the script change), `selftest.py` green; the
  runner's `measure` stage and `inertia_rel`. Tests: the box's volume,
  area, centroid and diagonal inertia `m(b² + c²)/12` and the cylinder's
  `m r²/2`, `m(3r² + h²)/12` to 1e-12 relative, off-diagonals below
  `default_tolerance²`; `sample::frame` against `boolean/frame-cut`'s
  oracle values to 1e-9; 1000 random boxes and cylinders in random poses
  against the closed forms carried through the pose; `inertia_about` of
  the origin against the direct integral; a `Sheet` is `NotSolid`; a
  raw-broken body is `InvalidInput`; the two live fixtures pass the
  `measure` stage at 1e-9.
- [x] Step 6 **[1]** — Rerun for the human. The `rerun` workspace
  dependency behind `arris-debug`'s feature, `arris_debug::rerun::{log,
  spawn}` as in the design deltas, `sample` bodies logged under distinct
  entity paths. Tests (feature-gated): logging the cylinder into a memory
  recording yields one mesh row per face, one strip per edge, one point
  batch, under the paths the doc states; `cargo build -p arris-debug
  --features rerun` green; the default build unchanged. The `inspect`
  skill names it.
- [ ] Step 7 **[1]** — `parallel` over faces. `rayon` behind
  `arris-mesh`'s feature: faces triangulated in parallel after the edge
  pass, collected in face order. Tests: the mesh of every sample body and
  both primitives is byte-identical with the feature on and off (a test
  that runs the crate's tests both ways in CI's `test` job); the
  `wasm32` build stays on default features.

## Acceptance

`ARRIS_PROPTEST_CASES=1000 cargo test --workspace` green, and the same
with `-p arris-mesh --features parallel`: the `primitive/*` fixtures pass
every corpus stage — the checker at `Full`, counts and genus, the
oracle's reading of the STEP, `measure` matching volume, area, centroid
and inertia to 1e-9 relative, the mesh closed with its signed volume
within `mesh_volume_rel` of the oracle's at `mesh_chord`, provenance
accounting, the dump — and `sample::frame` and `sample::sphere` mesh
closed and measure to their closed forms; the CDT and the surface
property tests at 1000 cases; `uv run --project tools/oracle
tools/oracle/selftest.py` green with `inertia` in every `expected.json`;
`target/inspect/cylinder-iso.png` read by the agent showing one wall, two
caps and a seam; `cargo build -p arris-debug --features rerun` green; the
layer check and the wasm build pass; CI green on `main`. Then tag `m3`
(the human's).

## Docs to update on completion

- `docs/03-roadmap.md` §M3 — status line: date, what was retired (the
  seam face and the poles through the pcurves; a CDT of our own over
  `robust`), ADR-0003, the acceptance bound as the closed form; §Fixtures
  — `measure` and the mesh stage now in the runner, the three tolerance
  keys, `inertia` in `expected.json`.
- `docs/01-architecture.md` §Crates — `arris-mesh` (`rayon`, `cdt`),
  `arris-debug` (`rerun`), `arris-geom` (chord bounds); new
  §Tessellation with the mesh guarantees; §Operations — the query shape
  of `measure`; §Threading — `parallel` implemented in `mesh`, the `f32`
  `⚠ OPEN` restated; §Formats and tools — `mesh_of`, `render_body`,
  `render_domain`, the Rerun stream, `sample::sphere`, the corpus stages;
  §Facade — the `tessellate` and `mass_properties` rows as signed.
- `docs/02-data-model.md` §Surfaces and §Curves — `chord_steps`,
  `chord_segments`; §Pcurves — `Model::loop_pieces` in the toolkit
  paragraph and tessellation as a user of it; §Seams and closed faces — a
  sentence on what the mesh does with the two copies.
- `docs/adr/README.md` — ADR-0003 in the table.
- `tests/fixtures/README.md`, `tools/oracle/README.md` — `inertia`, the
  tolerance keys, the runner's new stages.
- `.agents/skills/inspect/SKILL.md` — `render_body`, `render_domain`,
  `mesh_of`, `rerun::spawn`; "M3 adds" removed.
- `AGENTS.md` current state — "M3 done <date>; next `/plan m4-booleans`".
- `docs/BACKLOG.md` — per-vertex normals and (u, v) in `TriMesh` (with
  the `f32` decision, C2); adaptive refinement; anything a step defers,
  under "Findings".

## Findings

- **A curved face's own shading, not its face id, drives the histogram**:
  step 4's design guessed the cylinder's `Iso` render would show "exactly
  three face colours" the way the hand-built cube's does. It shows nine —
  the per-triangle quantised shade (`render.rs`'s `shade = 0.35 + 0.65 ·
  (n · light)`, rounded to 1/32) varies continuously across the wall's
  curvature, so one `FaceId` paints several distinct `[u8; 3]` values
  where a planar face paints exactly one. `the_box_shows_three_faces_
  from_iso_and_one_from_top` keeps the design's original claim, since a
  box's faces are planar; the cylinder's test reads the picture the way
  the render is actually used — by highlighting an entity and comparing
  areas (`Highlight::Edge`'s run is a small fraction of `Highlight::
  Face`'s), not by counting raw colours. No code changed; the finding is
  the acceptance criterion, corrected here rather than in a rewrite of
  the step once it had already landed.
- **A degenerate edge still counts as an edge in the Euler line**, so
  `sample::sphere` prints `2/3/1/1/1 g1 = 0` where a sphere is genus 0:
  the two pole edges each take one off `V − E + F`. The residual is
  even, so the line does not fail and no row is broken — the genus it
  derives is simply not the surface's. Excluding degenerate edges from
  the count would change the printed counts of every fixture and the
  oracle's own derivation, so it is a backlog line, not this plan's.
- **`TriMesh` indices are `u32` and nothing guards the boundary**:
  `push_position` casts `positions.len()` down. A chord fine enough to
  ask for more than four billion vertices wraps silently instead of
  returning a typed error. Backlog.
- **A NURBS surface has no `project`** (cycle 1, by design), so the
  deviation of a NURBS face cannot be measured the way the analytic
  kinds' is. `a_nurbs_patch_meshes_through_its_grid` measures it against
  a closed form instead — a bilinear saddle `z = x y`, whose vertical
  distance bounds the true one.

- **A tensor's zero is not an absolute number.** Step 5's test text asked
  for the box's products of inertia "below `default_tolerance²`" — 1e-14.
  That bound is not scale-free: the moments a 40 × 30 × 10 box integrates
  are of order 1e6, so f64 rounding alone leaves ~1e-10 on a product that
  is mathematically zero. `the_box_measures_its_closed_forms` asserts
  them below 1e-12 *of the tensor's largest component* instead, which is
  the same claim made relative — and the same relative bound every other
  quantity in the step is held to.
- **Carrying a tensor to the centroid costs what a small body far from the
  origin is worth.** The random-pose property test found it: a cylinder of
  radius and height 0.1 at `z = 86` has `∫ x² dV ≈ V |c|² ≈ 23` and a
  tensor of 1e-5, so the parallel-axis subtraction spends thirteen digits
  before it starts. `mass_properties` integrates the second moments about
  the centroid in a second pass instead, which is why it makes two passes
  over the faces and not one. `MassProperties::inertia_about` carries the
  tensor *outward*, where nothing cancels.

## Open questions

Each with a recommendation; the human decides before `/work` reaches the
step.

- `⚠ OPEN:` **The roadmap's mesh acceptance number does not hold for the
  cylinder.** An inscribed polygon prism at sagitta `δ` on radius `r` has
  relative volume error `≈ θ²/6 = 4δ/(3r)`: at `δ = 0.01`, `r = 4` that is
  3.3e-3, above the roadmap's 1e-3. Recommendation: the runner's defaults
  `mesh_chord = 1e-3`, `mesh_volume_rel = 2e-3` (sized by the corpus's
  smallest radius, 1, at 1.3e-3), the unit tests asserting the closed-form
  bound `4δ/(3r)` at any `δ`, and the roadmap's accept line corrected at
  retirement to say so. Human, by step 2.
- `⚠ OPEN:` **Own CDT or a crate.** `spade` and `cdt` exist; both carry
  their own predicates and insertion orders, so determinism and the
  `robust` decision (`SEED.md` §7) would rest on them. Recommendation:
  our own, as the roadmap's M3 line already says — the algorithm is
  small, the property tests above are the proof, and the (u, v) picture
  of step 4 is the debugger. ADR-0003. Human, before step 1.
- `⚠ OPEN:` **Inertia in the oracle now, or closed forms only until C2.**
  Recording it regenerates fifteen `expected.json` files in one
  `fixtures:` commit and fixes a convention (about the centroid, physical
  tensor) before the consumer's integrator is compared in C2.
  Recommendation: record it now — every measured number should have an
  oracle, and C2 can change the convention with an ADR and one more
  `fixtures:` commit. Human, by step 5.
- **Where the `rerun` feature builds — resolved, step 6.** A separate
  blocking CI job `rerun` runs `cargo build -p arris-debug --features
  rerun`; the hook and the `test` job are untouched, as recommended.
- `⚠ OPEN:` **Keep step 7.** `parallel` is not in the roadmap's M3 "in"
  list; it is in 01 §Threading's contract and costs one afternoon.
  Recommendation: keep it — it is the first test of "identical output
  with the feature off", which M4's boolean will also need. Human, by
  step 7.
