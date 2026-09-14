# 03 — Roadmap

Every milestone ends with a corpus run that prints numbers, and each retires
the scariest remaining unknown first. A milestone's "out" list is as binding
as its "in" list. One section per cycle; a finished cycle compresses to its
status line (`/close-cycle`), and the next is appended below.

Spine: **C1** M0 → M1 → M2 → M3 → M4 → M5 (the vertical slice, done), then
**C2** (the application gate, under way), then the Parasolid-grade cycles one
at a time.

---

## Fixtures

The unit of acceptance. A fixture is a directory
`tests/fixtures/<area>/<slug>/` with:

- `fixture.json` — the **recipe**: operands as primitives, profiles and
  poses, the operations applied to them in order, the classification probe
  points, the comparison tolerances, and the closed-form `analytic` values
  where a formula exists. Both sides evaluate the recipe: the oracle in
  Open CASCADE, the test in Arris. A recipe, not a STEP file, so the corpus
  never depends on a reader that does not exist yet and so a change to an
  operand is a one-line diff.
- `expected.json` — the **oracle's answer**, one per recipe variant:
  volume, area, centroid, the inertia tensor about the centroid, counts
  (vertices, edges, faces, loops, shells), the Euler characteristic and
  genus, the in/out/on result for each probe point, whether Open CASCADE
  built a solid at all, the OCCT version and the recipe's hash. Written by
  `tools/oracle/expected.py`, never by hand, and committed. A change to it is a `fixtures:` commit that says why
  (`.agents/rules/git.md`).
- `dump.txt` — Arris's text dump of the result once the fixture passes,
  the regression guard for ids and provenance (`dump.<variant>.txt` for a
  variant other than `default`). Committed once the fixture passes; the
  corpus lint fails a fixture the runner compares under `primitive/`,
  `transform/`, `boolean/`, `sweep/`, `provenance/` or `blend/` without
  one, so those areas hold no ignored fixture by test. A failure waiting for its fix is
  a fixture under `regression/`, outside them, and moves into its area in
  the commit that makes it pass.

The test for a fixture (`arris_debug::corpus::run`, one `#[test]` per
fixture in `crates/arris/tests/corpus.rs`) builds the recipe in Arris,
runs the checker at `Level::Full` — nothing violated, nothing left
`unchecked` — compares counts and genus against `expected.json`, writes
STEP and runs `tools/oracle/compare.py` on it (volume, area, centroid
and every probe, read back by Open CASCADE), tessellates the result at
the fixture's `mesh_chord` and holds the mesh closed with its signed
volume within `mesh_volume_rel` of the oracle's, measures it over the
B-Rep (`ops::measure::mass_properties`) and holds volume, area, centroid
and inertia to the oracle's within `inertia_rel`, classifies every probe
point against the result itself (`arris_check::classify::classify_point`)
and holds it to the oracle's class exactly, asserts each step's
provenance accounting (data-model §Provenance), and diffs the dump —
written instead under `ARRIS_BLESS=1`.
A fixture whose result is no solid does not reach those stages: one the
oracle recorded none for (`degenerate`) must fail with
`OpError::Degenerate`, and one whose recipe says `analytic.expect_error`
must fail with that typed refusal — `tangent-contact`, `non-manifold`,
`blend-too-large`, `tangent-chain` or `vertex-blend`, the oracle's
numbers kept as the record of what Open CASCADE builds instead. A recipe may also say `analytic.counts_differ: "why"` and carry
its own counts, for the one place Arris's convention is deliberately not
Open CASCADE's (a tangent ruling left unimprinted); every other fixture
mirrors the oracle's counts exactly.
Tolerances are the fixture's: relative 1e-9 on volume and area for
analytic results, exact on counts and classifications. The oracle is run,
never linked (`SEED.md` §7).

---

## C1 — the vertical slice

*Goal: box − cylinder through every crate. A blind hole, an 8-hole bolt
pattern by repeated cut, and a flush box ∪ box union: checker green,
volumes and counts matching Open CASCADE, provenance naming every hole wall
stably across parameter changes (`SEED.md` §6).*

**Status: done 2026-09-12, tag `c1`.** Retired the representation:
analytic surfaces with explicit seams, per-entity tolerances and a pcurve
on every coedge make a plane/cylinder boolean one decomposition and one
selection table, proven by the corpus and property tests without a human
looking at a screen. ADR-0001 to ADR-0005.

### M0 — the harness

*Goal: everything the agent needs to see and to judge exists before the
first line of geometry.*

**Status: done 2026-09-05, tag `m0`.** Retired the oracle round trip
(Open CASCADE's own STEP of every fixture reads back clean) and the corpus
lint. No ADRs.

- The workspace of architecture, with the layer rule checked by CI and
  the hook.
- Ids, handles, `Orientation` and `Precision` (the bookkeeping types of
  data-model, no entities yet).
- `arris-check`'s skeleton: `Level`, `Report`, and a `Violation` variant per
  invariant in data-model §Invariants, with a test that the doc and the
  enum list the same set.
- `TriMesh` and `Polyline` in `arris-mesh`, with signed volume, area and
  closedness.
- `arris-debug`: the software rasteriser to PNG, `View`s and highlights;
  the property-test configuration and numeric strategies.
- The oracle: `tools/oracle/` as a `uv` project over `cadquery-ocp`,
  `expected.py`, `compare.py`, the recipe interpreter for every operation
  C1 will have, and a self-test that round-trips OCCT's own STEP.
- The fixture corpus laid out with every C1 recipe and its `expected.json`
  generated, plus a corpus lint that checks the Euler line and the
  `analytic` values against the oracle.

**Out:** any curve, surface, entity or operation.

**Accept:** `cargo test --workspace` green including the corpus lint;
`compare.py` matches OCCT's own STEP on every fixture; the layer check and
the wasm build pass; a PNG of a hand-built cube mesh rendered and read by
the agent.

### M1 — math and analytic geometry

*Goal: every curve and surface of C1 evaluates, projects and intersects,
proven by property tests against closed forms.*

**Status: done 2026-09-06, tag `m1`.** Retired the plane–cylinder case
table and the parametrisation agreement with Open CASCADE. ADR-0001.

- `arris-math`: points, vectors, unit vectors and frames over `nalgebra`;
  `Interval`; exact 2D orientation and in-circle predicates over `robust`;
  polynomial roots to quartic and interval-guarded Newton; tolerance types.
- `arris-geom`: `Surface` and `Curve` with the parametrisations of
  data-model; evaluation and first/second derivatives; point projection
  onto every variant; `Curve2` and the pcurve of every analytic curve on
  the plane and the cylinder; NURBS evaluation, knot insertion and
  least-squares fitting of a `Curve2::Nurbs` to a sampled curve.
- Intersections: plane–plane (line), plane–cylinder (circle, ellipse, one
  or two lines, or tangent), line–plane, line–cylinder, circle–plane,
  circle–cylinder — each an exhaustive match arm, everything else
  `Unsupported`.
- Geometric property-test strategies in `arris-debug`: random frames,
  poses, radii.

**Out:** cone, sphere, torus and cylinder–cylinder intersections; NURBS
surfaces beyond evaluation.

**Accept:** property tests at 1000 cases each — projection is idempotent
and lands on the surface to 1e-12·scale; every intersection point lies on
both operands to 1e-12·scale; pcurve image matches the 3D curve to the
fitting tolerance; the plane–cylinder case table agrees with the closed
forms in random poses.

### M2 — topology, the checker, primitives, formats

*Goal: a box and a cylinder exist as bodies in the arena, pass every
invariant, and Open CASCADE reads them back with the right numbers.*

**Status: done 2026-09-06, tag `m2`.** Retired the seam, through the
checker and through Open CASCADE's reader, and Euler operators over
immutable entities. ADR-0002.

- `arris-topo`: the chunked arena, entities, orientation composition,
  adjacency indices, deterministic iteration, transactions, `import`,
  `retain` (sparse), the raw insert API for tests.
- Euler operators (Mäntylä's set, adapted to coedges and seams) as the only
  way an operation builds topology.
- `arris-check` complete at `Level::Fast`, plus L5/S5/B1/B2 at `Full`; a
  violation test per invariant.
- `ops::primitive_box`, `ops::primitive_cylinder` (with its seam), each
  returning `Generated` provenance for every entity.
- `io::step` writer; `io::native` round trip; `arris_debug::dump_text`.

**Out:** any operation that takes a body as input.

**Accept:** the `primitive/*` fixtures pass end to end (oracle reads the
STEP: volume, area, counts exact); every invariant's violation test
reports its violation and nothing else; native round trip dumps
identically; ids identical across two runs.

### M3 — tessellation and measurement

*Goal: the agent can look at a body, and the kernel can measure one.*

**Status: done 2026-09-07, tag `m3`.** Retired a constrained Delaunay
triangulation of our own and mass properties as an exact flux integral
over the B-Rep. ADR-0003.

- `arris-mesh::tessellate`: edges discretised once at the model's
  tolerance and shared by both faces; faces triangulated in (u, v) by
  constrained Delaunay over `robust` predicates, seams and periods handled
  through the pcurves; `FaceRange`/`EdgeRange` in iteration order.
- `arris_debug::render_png` over bodies, faces coloured by id, one
  `Highlight` per call; `arris_debug::rerun` (feature) for the human.
- `ops::measure::mass_properties`: volume, area, centroid, inertia by
  Gauss over the faces with quadrature in (u, v).

**Out:** adaptive or curvature-driven refinement beyond a chord tolerance;
`f32` output.

**Accept:** the mesh of every `primitive/*` fixture is closed and its
signed volume matches the oracle within the closed form of an inscribed
polygon's error, `4δ/(3r)` at the runner's `mesh_chord` (`mesh_volume_rel
= 2e-3`, sized by the corpus's smallest radius); `measure` matches the
oracle to 1e-9 relative; a PNG of the cylinder shows one wall, two caps
and a seam, read by the agent.

### M4 — booleans on plane and cylinder (the risk milestone)

*Goal: box − cylinder, then everything C1's corpus asks of it.*

**Status: done 2026-09-11, tag `m4`.** Retired the risk the cycle is
named for: a plane–cylinder boolean is one decomposition over shared
paves and one selection table, and what a manifold `Solid` cannot hold is
a typed refusal rather than a tolerance accident. ADR-0004, ADR-0005.

- The General Fuse decomposition in `ops::boolean`: intersect every face
  pair (M1's table), split faces by the intersection edges with pcurves on
  both sides, share the split edges between operands (the pave model, read
  in the reference tree and rebuilt for coedges), classify each piece
  in/out/on with tolerance-aware point classification, assemble the result
  for `fuse`, `common`, `cut`; `boolean::interferences` is the same
  decomposition as a printable value.
- Coincident planar faces (flush) and tangent cylinder–plane contact as
  explicit cases, not tolerance accidents.
- Provenance built inside the algorithm: `Modified` for split pieces,
  `Generated` for intersection edges and tool-face images, `Deleted` for
  the swallowed rest; `Provenance::then` for chains.
- `ops::transform`.

**Out:** cone, sphere, torus, NURBS operands; two operands whose result has
two shells (enclosed cavity); merging of same-domain faces after a fuse.

**Accept:** every `boolean/*` fixture passes end to end, property tests
hold (volume additivity `V(A ∪ B) + V(A ∩ B) = V(A) + V(B)`,
`V(A − B) + V(A ∩ B) = V(A)`, commutativity of `fuse` and `common` up to
ids, cut-then-fuse restores the volume) at 200 random poses of a box
and a cylinder, and `provenance/bolt-pattern-rebuild` reports 8 of 8 walls
under the same origin across three parameter sets.

### M5 — sweeps and the cycle's corpus

*Goal: a sketched profile becomes a solid, and C1 closes on its numbers.*

**Status: done 2026-09-12, tag `m5`.** Retired the sketch as topology: a
profile is a `geom::Profile` value, and a sweep's faces enter the builder
through `Builder::assemble`, held to Pappus's theorems at a thousand
random profiles. No ADRs.

- A `geom::Profile` of lines and arcs with holes (no `ops::planar_face`:
  a one-face sheet is C7's); `ops::extrude` (planes and cylinders);
  `ops::revolve` (planes, cylinders, and cones/spheres/tori as *surfaces*
  where the profile demands them — their booleans are C2–C3's).
- Extrude and revolve provenance: side faces `Generated` from profile
  edges, caps from the profile face.
- The full C1 corpus run in CI, the corpus's ignored tests included, and
  zero ignored fixtures in `primitive/`, `transform/`, `boolean/`, `sweep/`,
  `provenance/` held by the corpus lint.

**Out:** sweep along a path, loft, draft; a revolve profile touching its
axis (C2).

**Accept:** every fixture of the C1 corpus passes every stage;
`/close-cycle` drift review empty; tag `m5` and `c1`.

### C1 acceptance corpus

The fixtures themselves: every directory under `tests/fixtures/`
`primitive/`, `transform/`, `boolean/`, `sweep/` and `provenance/`, its
recipe, purpose and closed forms in `fixture.json` (`description`,
`analytic`) and the oracle's answer in `expected.json`, run as one
`#[test]` each by `crates/arris/tests/corpus.rs`. The grammar and the
conventions the numbers assume are `tests/fixtures/README.md`'s.

---

## C2 — the application gate

*Goal: Arris replaces the truck-derived kernel behind the first consumer's
facade. The gate is a test run, not a judgement call: everything the facade
uses today, plus the consumer's probe corpus — the recorded kernel failures
with their `#[ignore]`d twins — green.*

- Single-edge fillet and chamfer, several edges in one call: box edges
  (cylinder blends), hole edges (torus blends), the consumer's fixtures.
  A second fillet on an already-filleted body. Two blended edges meeting
  at a corner whose third edge stays sharp — a vertical and a cap edge, or
  two cap edges, the same solid rotated — meet in a miter, two cylinders
  and one ellipse, not a vertex blend (Open CASCADE, checked). Blends are
  rolling-ball stripes built in closed form on analytic face pairs,
  ADR-0007: `ops::fillet` and `ops::chamfer` on a plane–plane edge with
  its ends trimmed by the face across are in, convex or concave and
  several disjoint edges in one call (`blend/box-edge-fillet`,
  `box-cap-edge-fillet`, `box-posed-edge-fillet`, `box-oblique-end`,
  `l-inner-edge`, `box-four-verticals`, `box-edge-chamfer`,
  `box-four-vertical-chamfers`); two chamfers at a corner meet in a line
  and pass every stage (`box-corner-chamfers`), and the fillet miter builds and
  matches the oracle (`regression/fillet-miter`, waiting on the
  cylinder–cylinder line below for its one S5 row).
- Cone, sphere and torus in the intersector in the positions a blend and
  M5's revolves put them, every one a conic: a plane against a quadric
  with the plane perpendicular to the axis, a cylinder against a quadric
  coaxial with it, a sphere against a cylinder through its centre,
  plane–sphere, and a line against each quadric. The general pairs —
  plane–cone, plane–sphere and plane–torus in general position — are
  C3's.
- The checker's S5 and B1 arms, and `classify_point`, against cone,
  sphere and torus faces over those arms, so M5's quadric-faced revolves
  become corpus fixtures (`sweep/revolve-frustum`, `revolve-barrel`,
  `revolve-ring`) instead of closed-form tests with the oracle reading a
  scratch STEP.
- Cylinder–cylinder booleans. Parallel axes (rulings, a tangent line)
  are what the transversal probe needs and what a blend along a ruling's
  S5 row needs; coaxial already passes (`boolean/coaxial-cut`,
  `coaxial-fuse`); equal radii with crossing axes, two ellipses, is the
  miter's S5 row. Crossing axes of unequal radii, a quartic, close the
  quadric-curve `⚠ OPEN` with an ADR and are C3's; no probe forces them.
- Results of more than one shell — an enclosed cavity, a disjoint `fuse`,
  a `cut` that splits its target, a full revolve of a profile with holes —
  as lumps of one `Solid`, two lumps touching along an edge or at a vertex
  refused as `Reason::NonManifold`. **Done 2026-09-13**, ADR-0006.
- The revolve profile touching its axis (apex and degenerate edges), which
  M5 refused. The probe is a rectangle with one side on the axis: plane
  and cylinder faces only. **Done 2026-09-13**: `sweep/revolve-onto-axis`
  and `sweep/revolve-notch-to-axis`, a full-turn pinch refused as
  `Reason::NonManifold`.
- `Model::retain` semantics (the compaction `⚠ OPEN`), the `f32`
  boundary `⚠ OPEN`, the origin-name helper `⚠ OPEN` — each an ADR with
  the consumer's adapter as the test.
- Projection of edges and vertices to a plane; face frames; mass
  properties with inertia matched to the consumer's integrator; STL and
  OBJ export from the tessellation, beside STEP.

**Out:** NURBS–NURBS intersection, sweep along a path, loft, shell, healing,
the STEP reader.

**Accept:** the consumer's probe corpus in its own units — through hole
(8.7434e-5), blind hole (1.8743e-4), enclosed cavity (9.36e-4, two shells),
cylinder − cylinder transversal (2.2079e-5), flush union (2.0), revolve
touching the axis (2π), a fillet on a filleted body, a vertical and a cap
edge filleted in one call — every twin un-ignored and every probe deleted
(through hole, blind hole and flush union already pass as C1 fixtures at
another scale, the enclosed cavity as `boolean/enclosed-cavity` in its own);
`sweep/revolve-frustum`, `revolve-barrel` and `revolve-ring` passing every
corpus stage with nothing left unchecked; the consumer's naming fixtures
pass through provenance with no matcher; the consumer's facade compiles
against Arris with the truck crates removed.

---

## Toward Parasolid grade — later cycles, one line each

Each earns its own section with an acceptance corpus when it is next; none
is scheduled here (`SEED.md` §6).

- **C3** — every quadric pair in the intersector; tolerance growth and
  coincident/tangent face handling as a corpus of its own.
- **C4** — NURBS–NURBS surface intersection (a marcher with explicit seam
  handling); NURBS operands in booleans.
- **C5** — sweep along a path, loft, shell, offset.
- **C6** — fillet networks: edge chains, vertex blends, variable radius,
  blends over blends.
- **C7** — STEP reader and healing; sheet and wire bodies in every
  operation.
- **C8** — IGES; same-domain face merging; the performance pass the design
  reserved room for.

## What not to spend agent time on

A sketch constraint solver, rendering, UI, physics, drawings, mesh
processing beyond tessellation, formats beyond STEP and the native one,
and speed as a headline (`SEED.md` §4 non-goals). A backlog line that
lands in one of these is rejected with that reference.
