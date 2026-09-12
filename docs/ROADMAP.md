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
  volume, area, centroid, the inertia tensor about the centroid, counts
  (vertices, edges, faces, loops, shells), the Euler characteristic and
  genus, the in/out/on result for each probe point, whether Open CASCADE
  built a solid at all, the OCCT version and the recipe's hash. Written by
  `tools/oracle/expected.py`, never by hand, and committed. A change to it is a `fixtures:` commit that says why
  (`.agents/rules/git.md`).
- `dump.txt` — Arris's text dump of the result once the fixture passes,
  the regression guard for ids and provenance (`dump.<variant>.txt` for a
  variant other than `default`). Absent while the fixture is `#[ignore]`d.

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
must fail with that typed refusal — `multi-shell` or `tangent-contact`,
the oracle's numbers kept as the record of what Open CASCADE builds
instead. A recipe may also say `analytic.counts_differ: "why"` and carry
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

**Status: done 2026-09-06.** Retired the seam — through the checker
(E7, L2 as a one-period jump) and through Open CASCADE's reader, which
took Arris's `SEAM_CURVE` and both pcurves as written — and Euler
operators over immutable entities (a staging builder whose ten operators
keep the Euler line at zero and invert byte for byte). ADR-0002: the
builder, explicit pcurves, provenance rooted in `Origin::Role`. Deltas:
the `Origin::{Entity, Role}` provenance keys; `retain` reusing freed
slots lowest-first at a bumped generation; the (u, v) toolkit in
`arris-geom`; `Report::unchecked` for `Full` rows without a closed form.
Accepted on the two `primitive/*` fixtures end to end, the hand-built
`boolean/frame-cut` twin, 29 violation tests, native round trips of
three bodies, and 1000 property cases per test; tag `m2`.

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

**Status: done 2026-09-07.** Retired the constrained Delaunay
triangulation of our own over `robust` (ADR-0003) — edges discretised
once and shared through the pcurves, seams and poles as (u, v) copies of
one mesh vertex, interior points on a chord-bound grid rather than
adaptive refinement — and `ops::measure::mass_properties` as an exact
flux integral over the B-Rep, matching the oracle's newly recorded
inertia tensor to 1e-9 relative. The roadmap's mesh acceptance number did
not hold for a curved fixture (an inscribed-polygon prism's relative
volume error is `≈ 4δ/(3r)`, not a flat 1e-3); corrected below, sized by
the corpus's smallest radius. Accepted at 1000 property cases per test,
`parallel` byte-identical with the feature on and off, `cargo build -p
arris-debug --features rerun` green, the two `primitive/*` fixtures
passing the corpus's new mesh and `measure` stages; tag `m3`.

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

**Status: done 2026-09-11.** Retired the risk the cycle is named for: on
this representation a plane–cylinder boolean is one decomposition and one
selection table (ADR-0004). The paves are shared between the operands by
construction — an edge of one piercing a face of the other is one section
vertex both see — so the faces split in their own (u, v) through the
pcurves meet along the same edges, and the pieces, classified by B1's own
ray cast made public (`arris_check::classify::classify_point`), reassemble
through `Builder::assemble` with every untouched entity keeping its id.
Coincident faces (images, common blocks and the normals rule) and tangent
contact (the curvature rule, decided before any face is split) are named
cases, and what a manifold `Solid` cannot hold is a typed refusal rather
than a tolerance accident: `Reason::{Empty, MultiShell, TangentContact,
ZeroThickness}`. Two conventions were stated rather than inherited: Arris
leaves a tangent ruling unimprinted where Open CASCADE splits the touched
face (`analytic.counts_differ`, the corpus's one exception), and a `cut`'s
tool keeps nothing, its surviving pieces `Generated` from it. ADR-0005 came
out of the corpus: a ruled direction is flattened before the triangulation,
which an oblique hole's wall needed to mesh within the inscribed bound.
Accepted at 1000 property cases per test, with the identity between two
separately fitted pcurves held to the model's tolerance over the body's
size and not to a bare relative bound; every `primitive/*`, `transform/*`,
`boolean/*` fixture and `provenance/bolt-pattern-rebuild`'s three variants
passing every corpus stage, only the three `sweep/*` fixtures still
ignored; `parallel` byte-identical with the feature on and off; the layer
check and the wasm build green.

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
| `transform/posed-cylinder` | the cylinder above, rotated 30° about [1,1,0] through the origin and translated: `ops::transform`'s fixture, every entity `Modified` one-to-one | 603.1858 | 402.1239 | 2/3/3/3 | 0 |
| `boolean/through-hole` | box above − cylinder r 4 at (20,15), z −1…11 | 11497.3452 | 3950.7964 | 10/15/7/9 | 1 |
| `boolean/posed-through-hole` | `through-hole` with both operands under one rigid motion: the same solid in another pose, so `through-hole`'s numbers with the centroid moved | 11497.3452 | 3950.7964 | 10/15/7/9 | 1 |
| `boolean/blind-hole` | box above − cylinder r 4 at (20,15), z 4…16 (depth 6, floor kept) | 11698.4071 | 3950.7964 | 10/15/8/9 | 0 |
| `boolean/oblique-hole` | box above − cylinder r 3 whose axis is tilted 30° about x through (20,15,5): two ellipse sections, NURBS pcurves on the wall | 11673.5161 | 3952.3591 | 10/15/7/9 | 1 |
| `boolean/bolt-pattern-8` | plate [0,0,0]–[100,100,10] − 8 cylinders r 3 on a circle R 35 about (50,50), each a separate cut | 97738.0533 | 25055.5751 | 24/36/14/30 | 8 |
| `boolean/flush-union` | box [0,0,0]–[40,30,10] ∪ box [40,0,0]–[80,30,10]; the shared face vanishes, coplanar neighbours are *not* merged (as Open CASCADE) | 24000 | 7000 | 12/20/10/10 | 0 |
| `boolean/corner-union` | cube [−1,1]³ ∪ cube [0,2]³ | 15 | 42 | 20/30/12/12 | 0 |
| `boolean/corner-common` | same, ∩ | 1 | 6 | 8/12/6/6 | 0 |
| `boolean/corner-cut` | same, − | 7 | 24 | 14/21/9/9 | 0 |
| `boolean/sliver-common` | box ∩ a cylinder r 4 whose axis stands just outside the face x = 40, the arc inside spanning 36°: a sliver whose caps are a D of one chord and one short arc (`region2::MIN_SEGMENTS_PER_ARC`) | 3.2427 | 50.5026 | 4/6/4/4 | 0 |
| `boolean/flush-common` | the two flush boxes, ∩ | — | — | `OpError::Degenerate` (`Reason::ZeroThickness`: the two coincident faces are dropped by the normals and nothing survives) | — |
| `boolean/boss` | the plate ∪ a cylinder r 4 at (20,15), z 5…20: the wall crosses the top face, the tool's bottom cap is swallowed | 12502.6548 | 4051.3274 | 10/15/8/9 | 0 |
| `boolean/boss-flush` | the plate ∪ the boss of `boss` raised to z 10…20, its bottom cap coincident with the plate's top: the cap vanishes, the rim is the wall's own edge — `boss`'s numbers | 12502.6548 | 4051.3274 | 10/15/8/9 | 0 |
| `boolean/coaxial-cut` | cylinder r 2, z −1…1 − a coaxial r 1 running through it: a tube — the coaxial pair has no section curve and the bore's wall is the tool's wall reversed (`sweep/revolve-tube`'s numbers) | 18.8496 | 56.5487 | 4/6/4/6 | 1 |
| `boolean/coaxial-fuse` | the tube of `coaxial-cut` ∪ a cylinder r 1, z −1…1 filling its bore: the coincident walls vanish, the discs sit beside the annuli sharing the inner circles (as Open CASCADE) | 25.1327 | 50.2655 | 4/5/5/7 | 0 |
| `boolean/disjoint-cut` | box − a cylinder clear of it | 12000 | 3800 | 8/12/6/6, provenance: every tool entity `Deleted`, target kept | 0 |
| `boolean/disjoint-common` | box ∩ a cylinder clear of it | — | — | `OpError::Degenerate` (`Reason::Empty`) | — |
| `boolean/swallow-cut` | box [10,10,2]–[30,20,8] − the plate that contains it | — | — | `OpError::Degenerate` (`Reason::Empty`) | — |
| `boolean/split-cut` | the plate − a slab [18,−1,−1]–[22,31,11] straight through it | — | — | `OpError::Degenerate` (`Reason::MultiShell`); Open CASCADE's two solids recorded: 10800, 4080, 16/24/12/12 in two shells | — |
| `boolean/tangent-outside-cut` | the plate − a cylinder r 4 whose wall touches the face x = 40 from outside along a ruling: no material removed, the plate back with every id | 12000 | 3800 | 8/12/6/6 — `counts_differ`: Open CASCADE imprints the ruling and reports 10/15/7/7 | 0 |
| `boolean/tangent-hole` | the plate − a cylinder r 3 at (3,15), z 4…16, whose wall touches the face x = 0 from inside along a ruling interior to both | — | — | `OpError::Degenerate` (`Reason::TangentContact`); Open CASCADE's slit recorded: 11830.3540, 3913.0973, 12/19/9/11 | — |
| `boolean/frame-cut` | box [0,0,0]–[40,30,10] − box [10,10,−1]–[30,20,11]: a rectangular frame; M2 builds it by hand through the Euler operators (`sample::frame`), M4 by this recipe — the cross-check between the two paths, as `extrude-plate-with-hole` is for `through-hole` | 10000 | 4000 | 16/24/10/12 | 1 |
| `sweep/extrude-plate-with-hole` | rectangle 40×30 with a hole r 4 at (20,15), extruded 10 | 11497.3452 | 3950.7964 | 10/15/7/9 (same numbers as `through-hole`: the cross-check between the two paths) | 1 |
| `sweep/extrude-slot` | a stadium of two lines and two semicircular arcs, straight length 20, radius 5, with a hole r 2 at its centre, extruded 8: partial cylinder faces with no seam beside a seamed one, the three cylinders' boxes apart | 2127.7876 | 1203.8053 | 10/15/7/9 | 1 |
| `sweep/extrude-downward` | the plate-with-hole profile on the plane z = 10, extruded 10 along −z: the profile face on top keeping its frame | 11497.3452 | 3950.7964 | 10/15/7/9 (`through-hole`'s numbers by the third path) | 1 |
| `sweep/revolve-tube` | rectangle x∈[1,2], z∈[−1,1] revolved 2π about z | 18.8496 | 56.5487 | 4/6/4/6 | 1 |
| `sweep/revolve-quarter` | the same rectangle revolved π/2 | 4.7124 | 18.1372 | 8/12/6/6 | 0 |
| `sweep/revolve-l-profile` | an L of six segments, x∈[1,3], z∈[−1,1] less the notch [2,3]×[0,1], revolved 3π/2: every face a plane or a cylinder, the walls' u range past π with no seam | 25.9181 | 76.6858 | 12/18/8/8 | 0 |
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
- Results of more than one shell — an enclosed cavity, a disjoint `fuse`,
  a `cut` that splits its target — which M4 refuses as
  `Reason::MultiShell`; the revolve profile touching its axis (apex and
  degenerate edges), a `Degenerate` for a profile crossing it.
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
