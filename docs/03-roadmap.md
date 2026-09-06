# 03 — Roadmap

Every milestone ends with a corpus run that prints numbers, and each retires
the scariest remaining unknown first. A milestone's "out" list is as binding
as its "in" list. One section per cycle; a finished cycle compresses to its
status line (`/close-cycle`), and the next is appended below.

Spine: **C1** M0 → M1 → M2 → M3 → M4 → M5 (the vertical slice), then
**C2** (the application gate), then the Parasolid-grade cycles one at a
time.

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
  volume, area, centroid, counts (vertices, edges, faces, loops, shells),
  the Euler characteristic and genus, the in/out/on result for each probe
  point, the OCCT version and the recipe's hash. Written by
  `tools/oracle/expected.py`, never by hand, and committed. A change to it is a `fixtures:` commit that says why
  (`.agents/rules/git.md`).
- `dump.txt` — Arris's text dump of the result once the fixture passes,
  the regression guard for ids and provenance. Absent while the fixture is
  `#[ignore]`d.

The test for a fixture builds the recipe in Arris, runs the checker at
`Level::Full`, compares its own `measure` against `expected.json`, writes
STEP and runs `tools/oracle/compare.py` on it, asserts the provenance
accounting (02-data-model §Provenance), and diffs the dump. Tolerances are
the fixture's: relative 1e-9 on volume and area for analytic results,
exact on counts and classifications. The oracle is run, never linked
(`SEED.md` §7).

---

## C1 — the vertical slice

*Goal: box − cylinder through every crate. A blind hole, an 8-hole bolt
pattern by repeated cut, and a flush box ∪ box union: checker green,
volumes and counts matching Open CASCADE, provenance naming every hole wall
stably across parameter changes (`SEED.md` §6).*

The risk this cycle retires is the representation: that analytic surfaces
with explicit seams, per-entity tolerances and a pcurve on every coedge
make a plane/cylinder boolean *natural* — and that it can all be proven
without a human looking at a screen.

### M0 — the harness

*Goal: everything the agent needs to see and to judge exists before the
first line of geometry.*

**Status: done 2026-09-05.** Retired the oracle round trip (OCCT's own
STEP of every fixture reads back clean) and the corpus lint (all fifteen
C1 recipes' oracle values match their closed forms, counts and genus).
No ADRs; the recipes-not-STEP and `cadquery-ocp==8.0.1.*` questions
closed as assumed.

- The workspace of 01-architecture, with the layer rule checked by CI and
  the hook.
- Ids, handles, `Orientation` and `Precision` (the bookkeeping types of
  02-data-model, no entities yet).
- `arris-check`'s skeleton: `Level`, `Report`, and a `Violation` variant per
  invariant in 02-data-model §Invariants, with a test that the doc and the
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

**Status: done 2026-09-06.** Retired the plane–cylinder case table (circle,
ellipse, two rulings, one tangent ruling, empty — agreeing with the closed
forms in random poses) and the parametrisation agreement with Open CASCADE
(the geometry oracle matched every evaluation, seam, pole and projected
parameter at the first run). ADR-0001: `nalgebra`'s types by alias. The
`Curve2` delta: a pcurve's circle or ellipse is placed by a `Frame2` whose
handedness is its direction of traversal. Accepted at 1000 property cases
per test, the two `geom/*` fixtures matching the oracle, the layer check
and the wasm build green.

- `arris-math`: points, vectors, unit vectors and frames over `nalgebra`;
  `Interval`; exact 2D orientation and in-circle predicates over `robust`;
  polynomial roots to quartic and interval-guarded Newton; tolerance types.
- `arris-geom`: `Surface` and `Curve` with the parametrisations of
  02-data-model; evaluation and first/second derivatives; point projection
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
signed volume matches the oracle to 1e-3 relative at tolerance 0.01;
`measure` matches the oracle to 1e-9 relative; a PNG of the cylinder shows
one wall, two caps and a seam, read by the agent.

### M4 — booleans on plane and cylinder (the risk milestone)

*Goal: box − cylinder, then everything C1's corpus asks of it.*

- The General Fuse decomposition in `ops::boolean`: intersect every face
  pair (M1's table), split faces by the intersection edges with pcurves on
  both sides, share the split edges between operands (the pave model, read
  in the reference tree and rebuilt for coedges), classify each piece
  in/out/on with tolerance-aware point classification, assemble the result
  for `fuse`, `common`, `cut`.
- Coincident planar faces (flush) and tangent cylinder–plane contact as
  explicit cases, not tolerance accidents.
- Provenance built inside the algorithm: `Modified` for split pieces,
  `Generated` for intersection edges and tool-face images, `Deleted` for
  the swallowed rest; `Provenance::then` for chains.
- `ops::transform`.

**Out:** cone, sphere, torus, NURBS operands; two operands whose result has
two shells (enclosed cavity); merging of same-domain faces after a fuse.

**Accept:** the `boolean/*` fixtures of the table below pass end to end,
property tests hold (volume additivity `V(A ∪ B) + V(A ∩ B) = V(A) +
V(B)`, `V(A − B) + V(A ∩ B) = V(A)`, commutativity of `fuse` and `common`
up to ids, cut-then-fuse restores the volume) at 200 random poses of a box
and a cylinder, and `provenance/bolt-pattern-rebuild` reports 8 of 8 walls
under the same origin across three parameter sets.

### M5 — sweeps and the cycle's corpus

*Goal: a sketched profile becomes a solid, and C1 closes on its numbers.*

- `ops::planar_face` from a `Profile` of lines and arcs with holes;
  `ops::extrude` (planes and cylinders); `ops::revolve` (planes, cylinders,
  and cones/spheres/tori as *surfaces* where the profile demands them —
  their booleans are C3).
- Extrude and revolve provenance: side faces `Generated` from profile
  edges, caps from the profile face.
- The full C1 corpus run in CI, `cargo test --workspace -- --include-ignored`
  reporting zero ignored fixtures in `primitive/`, `boolean/`, `sweep/`,
  `provenance/`.

**Out:** sweep along a path, loft, draft; a revolve profile touching its
axis (C2).

**Accept:** every fixture in the table passes; `/close-cycle` drift review
empty; tag `m5` and `c1`.

### C1 acceptance corpus

Recipes in model units; the oracle's `expected.json` holds full precision
and these are the closed forms to four decimals. V/E/F/L are vertices,
edges (a seam counted once), faces, loops. `χ` is the Euler line
`V − E + F − (L − F) − 2(S − G)`, always 0.

| Fixture | Recipe | Volume | Area | V/E/F/L | Genus |
|---|---|---|---|---|---|
| `primitive/box` | box [0,0,0]–[40,30,10] | 12000 | 3800 | 8/12/6/6 | 0 |
| `primitive/cylinder` | r 4, h 12, axis z, base at origin | 603.1858 | 402.1239 | 2/3/3/3 | 0 |
| `boolean/through-hole` | box above − cylinder r 4 at (20,15), z −1…11 | 11497.3452 | 3950.7964 | 10/15/7/9 | 1 |
| `boolean/blind-hole` | box above − cylinder r 4 at (20,15), z 4…16 (depth 6, floor kept) | 11698.4071 | 3950.7964 | 10/15/8/9 | 0 |
| `boolean/bolt-pattern-8` | plate [0,0,0]–[100,100,10] − 8 cylinders r 3 on a circle R 35 about (50,50), each a separate cut | 97738.0533 | 25055.5751 | 24/36/14/30 | 8 |
| `boolean/flush-union` | box [0,0,0]–[40,30,10] ∪ box [40,0,0]–[80,30,10]; the shared face vanishes, coplanar neighbours are *not* merged (as Open CASCADE) | 24000 | 7000 | 12/20/10/10 | 0 |
| `boolean/corner-union` | cube [−1,1]³ ∪ cube [0,2]³ | 15 | 42 | 20/30/12/12 | 0 |
| `boolean/corner-common` | same, ∩ | 1 | 6 | 8/12/6/6 | 0 |
| `boolean/corner-cut` | same, − | 7 | 24 | 14/21/9/9 | 0 |
| `boolean/flush-common` | the two flush boxes, ∩ | — | — | `OpError::Degenerate` (zero-thickness result) | — |
| `boolean/disjoint-cut` | box − a cylinder clear of it | 12000 | 3800 | 8/12/6/6, provenance: every tool entity `Deleted`, target kept | 0 |
| `boolean/frame-cut` | box [0,0,0]–[40,30,10] − box [10,10,−1]–[30,20,11]: a rectangular frame; M2 builds it by hand through the Euler operators (`sample::frame`), M4 by this recipe — the cross-check between the two paths, as `extrude-plate-with-hole` is for `through-hole` | 10000 | 4000 | 16/24/10/12 | 1 |
| `sweep/extrude-plate-with-hole` | rectangle 40×30 with a hole r 4 at (20,15), extruded 10 | 11497.3452 | 3950.7964 | 10/15/7/9 (same numbers as `through-hole`: the cross-check between the two paths) | 1 |
| `sweep/revolve-tube` | rectangle x∈[1,2], z∈[−1,1] revolved 2π about z | 18.8496 | 56.5487 | 4/6/4/6 | 1 |
| `sweep/revolve-quarter` | the same rectangle revolved π/2 | 4.7124 | 18.1372 | 8/12/6/6 | 0 |
| `provenance/bolt-pattern-rebuild` | `bolt-pattern-8` at (t 10, r 3, R 35), (12, 3.5, 35), (10, 3, 30) | — | — | 8/8 hole walls with the same `origins` chain in all three; the plate's top face `Modified` through the chain into one face with 9 loops | — |

Each also carries probe points (one inside, one outside, one on a face,
one on an edge, one on the hole's axis) classified by both sides.

---

## C2 — the application gate

*Goal: Arris replaces the truck-derived kernel behind the first consumer's
facade. The gate is a test run, not a judgement call: everything the facade
uses today, plus the consumer's probe corpus — the recorded kernel failures
with their `#[ignore]`d twins — green.*

- Single-edge fillet and chamfer, several edges in one call: box edges
  (cylinder blends), hole edges (torus blends), the consumer's fixtures.
- Cone, sphere and torus in the intersector as far as the blends and the
  probe corpus need: plane–cone, plane–sphere, plane–torus, cylinder–torus
  at the hole-edge fillet.
- Cylinder–cylinder booleans (transversal and coaxial), closing the
  quadric-curve `⚠ OPEN` with an ADR.
- Two-shell results (enclosed cavity), the revolve profile touching its
  axis (apex and degenerate edges), a `Degenerate` for a profile crossing
  it.
- `Model::retain` semantics (the compaction `⚠ OPEN`), the `f32`
  boundary `⚠ OPEN`, the origin-name helper `⚠ OPEN` — each an ADR with
  the consumer's adapter as the test.
- Projection of edges and vertices to a plane; face frames; mass
  properties with inertia matched to the consumer's integrator.

**Out:** NURBS–NURBS intersection, sweep along a path, loft, shell, healing,
the STEP reader.

**Accept:** the consumer's probe corpus in its own units — through hole
(8.7434e-5), blind hole (1.8743e-4), enclosed cavity (9.36e-4, two shells),
cylinder − cylinder transversal (2.2079e-5), flush union (2.0), revolve
touching the axis (2π) — every twin un-ignored and every probe deleted; the
consumer's naming fixtures pass through provenance with no matcher; the
consumer's facade compiles against Arris with the truck crates removed.

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
