# 03 — Roadmap

Every milestone ends with a corpus run that prints numbers, and each retires
the scariest remaining unknown first. A milestone's "out" list is as binding
as its "in" list. One section per cycle; a finished cycle compresses to its
status line (`/close-cycle`), and the next is appended below.

Spine: **C1** M0 → M1 → M2 → M3 → M4 → M5 (the vertical slice, done), then
**C2** (the application gate, done 2026-09-19), then **C3** (every quadric
pair — closure: what Arris builds, Arris takes as an operand), then the
reader cycle (the STEP reader and a real-part corpus), and after it the
cycle that corpus's refusal histogram and the first consumer's
side-by-side regressions pick. An unopened cycle carries a name, not a
number: it takes its number when `/close-cycle` opens its section
(ADR-0020).

---

## Fixtures

The unit of acceptance. A fixture is a directory
`tests/fixtures/<area>/<slug>/` with:

- `fixture.json` — the **recipe**: operands as primitives, profiles and
  poses, the operations applied to them in order, the classification probe
  points, the comparison tolerances, the `precision` the model is built
  with — `Precision::DEFAULT` unless the fixture is in another unit — and
  the closed-form `analytic` values where a formula exists. Both sides evaluate the recipe: the oracle in
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
`blend-too-large`, `tangent-chain`, `vertex-blend` or
`elliptic-revolve`, the oracle's
numbers kept as the record of what Open CASCADE builds instead. A recipe may also say `analytic.counts_differ: "why"` and carry
its own counts, for the one place Arris's convention is deliberately not
Open CASCADE's (a tangent ruling left unimprinted); every other fixture
mirrors the oracle's counts exactly. Likewise `analytic.measure_differs:
"why"` with closed forms for the volume, area, centroid and inertia, for
a result Open CASCADE measurably builds wrong (ADR-0015).
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
  a one-face sheet is the healing cycle's); `ops::extrude` (planes and
  cylinders);
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

*Goal: Arris covers what the first consumer's facade uses, and its probe
corpus — the recorded kernel failures with their `#[ignore]`d twins — is
green as fixtures. The gate is a corpus run, not a judgement call; the swap
itself is the consumer's, on its own schedule (ADR-0017).*

**Status: done 2026-09-19, tag `c2`, released as `v0.1.1`.** Retired the
application gate: the first consumer's probe shapes pass every corpus
stage in its own units, over blends built in closed form on analytic face
pairs, multi-shell results as lumps of one solid, and quadric faces decided
through their meridians on a shared axis, with the facade's three open
decisions taken. ADR-0006 to ADR-0017.

- Fillet and chamfer, several edges in one call: plane–plane edges with
  miters and three-blend corners, plane–cylinder edges along a ruling and
  around a rim, a second blend on a blended body (ADR-0007).
- Cone, sphere and torus in the intersector wherever two surfaces share an
  axis or a plane holds it, and a line against each (ADR-0008); the
  checker's S5 and B1 and `classify_point` over them, so M5's quadric-faced
  revolves are corpus fixtures.
- Cylinder–cylinder booleans: coaxial, parallel, tangent inside and out,
  equal radii crossing, whatever the tool's seam (ADR-0015, ADR-0016).
- Results of more than one shell as lumps of one `Solid` (ADR-0006); a
  revolve profile touching its axis.
- The facade's three decisions: no name grammar and a guaranteed split
  order (ADR-0009), `Model::retain` never renumbers (ADR-0010), `TriMesh`
  stays `f64` (ADR-0011).
- Plane projection, face frames, inertia matched to an independent
  integrator, the mesh corner block (ADR-0012), STL and OBJ export
  (ADR-0013).
- Elliptic profile segments, swept to an elliptic cylinder (ADR-0014); the
  workspace published from a tag.

**Out:** NURBS–NURBS intersection, sweep along a path, loft, shell, healing,
the STEP reader.

**Accept:** the consumer's probe corpus in its own units — through hole
(8.7434e-5), blind hole (1.8743e-4), enclosed cavity (9.36e-4, two shells),
cylinder − cylinder transversal (2.2079e-5), flush union (2.0), revolve
touching the axis (2π), a fillet on a filleted body, a vertical and a cap
edge filleted in one call.
Every one of them is a fixture in metres at the micrometre default
tolerance a metre model carries (`docs/ARCHITECTURE.md` §How a consumer's
kernel facade maps on, *Units*): the six
`probe-*-m` fixtures under `boolean/`, `blend/` and `sweep/`
(**done 2026-09-17**, the recipe's own `precision`), the enclosed cavity
as `boolean/enclosed-cavity` and the transversal as
`boolean/parallel-cylinders-cut`;
`sweep/revolve-frustum`, `revolve-barrel` and `revolve-ring` passing every
corpus stage with nothing left unchecked; every row of the facade table
(`docs/ARCHITECTURE.md` §How a consumer's kernel facade maps on) present.
The consumer's naming fixtures, its facade over Arris and its twins
un-ignored are the consumer's own acceptance, not this cycle's (ADR-0017).

---

## C3 — every quadric pair

*Goal: a boolean takes a cone, sphere, torus or elliptic-cylinder face as
an operand in any pose, because every quadric pair meets in the
intersector: conics by closed form, and the curves that are not conics
stored as the ADR that closes the quadric-curve `⚠ OPEN` decides. Faces
that meet within a tolerance, touching or coincident, are a corpus of
their own and not left to luck.*

*It comes first because it is closure: `ops::fillet`, `ops::chamfer` and
`ops::revolve` return tori, spheres and cones, and a body Arris built has
to be a body Arris takes (ADR-0020).*

**Status: in progress.** Every pair of analytic surfaces meets in the
intersector in every pose — conics exact, the quartics of a ruled pair
traced by its rulings and fitted to NURBS inside a region (ADR-0018,
done 2026-09-19), a torus's section traced in the torus's own parameter
plane and fitted whole (ADR-0019, done 2026-09-20) — with one `Meets`
result for every meeting, and `Unsupported` left only where a
`Surface::Nurbs` is in the pair; a boolean of cylinders cuts through the
fitted edge it left, and S5 and B1 decide every such pair. A boolean
takes a cone, sphere, torus or elliptic-cylinder face as an operand, a
section through an apex or a pole included and one beside it refused by
name (ADR-0021, done 2026-09-22): every conic meets every analytic
surface, every curve on one has a pcurve, and the M4 identities hold
over quadric operands at random poses. Open: features a tolerance
apart.

- The general quadric pairs in `intersect_surfaces`. **Done**: a plane
  oblique to a cone's axis or parallel to it and off it; two
  cylinders on crossing axes of unequal radii or skew axes within the
  radii; cylinder, cone and sphere against a cone or a sphere sharing no
  axis; the elliptic cylinder against the quadrics; the result type of a
  pair that mixes kinds (ADR-0018). Every pair with a torus sharing no
  axis with the other surface, the spiric sections and the Villarceau
  circles among them, on the second tracer's interface and the same fit,
  with a tube circle the other surface holds returned exact (ADR-0019).
- Conics against a cone, a sphere or a torus in `intersect_curve_surface`,
  and two coplanar conics that are not the same conic in
  `intersect_curves`, for the pave model's edge–face hits. **Done**
  (`geom/c3-conic-hits`), as is a fitted `Curve::Nurbs` against every
  analytic surface and against a line, a circle or an ellipse (ADR-0018).
- A fitted pcurve fallback on cones, spheres and tori for the oblique
  sections a boolean leaves there. **Done**, over the surface's own
  projection on every analytic surface, the torus's too — the fitted
  section is held to its two surfaces, not to the exact branch, so the
  branch's (u, v) is no pcurve of the curve an edge carries — and split
  at an apex or a pole (ADR-0021).
- The pave model's quadric guard lifted: booleans with cone, sphere, torus
  and elliptic-cylinder operand faces, each a corpus fixture against the
  oracle. **Done**: the frustum, ball, ring, elliptic, filleted and
  chamfered `boolean/*` fixtures, a section through an apex or a pole, a
  contact along a circle, and `quadric_operands_obey_every_identity`.
- S5 and B1 over every such pair, so a posed blend's cylinders on skew axes
  and a corner's sphere against a blend cylinder are checked, not listed as
  unchecked. **Done** for every pair of analytic surfaces (ADR-0018,
  ADR-0019): `blend_prop.rs` holds nothing unchecked, and a torus face is
  decided against anything — a pin through the tube, a pipe elbow welded
  along a tube circle, two shells interlocked.
- Features a tolerance apart: section vertices clustered by closure rather
  than first-come merging (`regression/seam-a-tolerance-from-crossing-fuse`),
  and a corpus of faces touching and coincident within a tolerance.

**Out:** NURBS operands and NURBS–NURBS intersection (the NURBS cycle's);
blends on quadric face pairs (the blend-network cycle's); a spindle
torus; a revolve of an elliptic segment;
the STEP reader.

**Accept:** property tests at random poses of every quadric pair — every
intersection point on both surfaces within the tolerance, the curve's
image matching both surfaces at its samples; the boolean identities of M4
(volume additivity, cut-then-fuse, commutativity) over a quadric operand
against a box and a cylinder; every new `boolean/*` quadric fixture
passing every corpus stage against Open CASCADE;
`crates/arris-ops/tests/blend_prop.rs` with nothing unchecked in a pose;
`regression/seam-a-tolerance-from-crossing-fuse` moved into `boolean/`;
`docs/DATA-MODEL.md` with no `⚠ OPEN` left.

---

## Beside the cycles

Two lines of work that are not cycles. Neither changes a public type or a
signature, so neither earns a minor version (`.agents/rules/git.md`
§Tags); each stands beside whatever cycle is open (ADR-0020).

**The measuring harness.** What it measures decides what comes after the
reader cycle, so it exists before the reader does: benchmarks over
tessellation and the boolean corpus, so a robustness fix that costs 10×
shows up as a number; the oracle cached by `recipe_hash`, so an unchanged
recipe is not re-run on every test run — `oracle::scratch_fixture` runs it
today whatever the recipe says; a property tier above CI's case count, run
nightly; random *recipes* evaluated by both kernels — the recipe grammar
already carries the same eleven operations on each side (`Step` in
`arris-debug`, the `op` dispatch in `tools/oracle`), so the generator is
a `proptest` strategy over `Step` beside the oracle's existing evaluator
and needs no new interpreter on either side; `cargo-fuzz` targets over
the intersectors and, once it exists, the STEP reader, seeded from the
corpus. Its numbers — benchmark baselines, cases per night — live in
this section once they exist. Its one open question is the case count
the pre-commit hook runs against CI's, which is the human's call.

**The first-party binding.** Code-first and agent-driven modelling is one
of the consumers `SEED.md` §1 names, and a binding in this repository is
how that consumer exists before an application is written on top of it.
It starts once C3 closes: a second crate under every C3 API break pays
for the break twice. Its layer position, the `#![forbid(unsafe_code)]`
exception a binding needs, `publish`, the wasm job and PyPI beside
crates.io are its own idea and its own ADR.

---

## Next — the reader and the real-part corpus

*Goal: Arris reads a part it did not design, and every refusal it returns
over a public corpus of real parts is counted. That count, beside the
first consumer's side-by-side regressions, is what picks the cycle after
this one (ADR-0020). Until it exists the kernel's refusals are unranked:
every fixture in the corpus is a recipe written out of the operations
Arris has. The first consumer wants the cycle for its own reason — its
roadmap imports vendor parts (ADR-0017).*

**Status: not opened.** It takes its number from `/close-cycle`.

- A Part 21 parser: the exchange structure, references, the schema
  header, string and number encodings, and a typed error carrying the
  entity id for everything malformed.
- The AP203/214/242 B-Rep subset onto Arris's own geometry: the analytic
  surfaces and curves as themselves, B-spline surfaces and curves as
  `Nurbs`, the topology through the Euler operators like every other
  builder, provenance `Generated` from the file's entity.
- Pcurves rebuilt rather than read — a file's are optional, approximate,
  or absent. Closed form on the analytic surfaces (data-model §Pcurves),
  which pulls `project` onto a NURBS surface forward: it is
  `GeomError::Unsupported` today, by cycle-1 design.
- Each entity's tolerance assigned from its own measured gaps, not from
  the file's global value, so the per-entity model the kernel is built on
  survives the import.
- A typed refusal for everything outside the subset, named specifically
  enough to count in the histogram.

**Out:** sewing and repair, and open shells — healing is its own cycle;
booleans on NURBS faces (the NURBS cycle's); IGES; writing anything the
writer does not write today.

**Accept:** write → read round trip as a property, over the corpus's
shapes in random poses, to the entities' own tolerances; Open CASCADE's
STEP of every corpus fixture read back to the same counts, volume, area
and centroid the fixture asserts; a public corpus of real parts read
either to checker-green — mass properties within the fixture's tolerance
of the oracle's — or to a typed refusal, with no panic and no wrong
solid; and the refusal histogram over that corpus printed.

---

## Named cycles, unordered

None is scheduled, and the order below carries no meaning. The cycle
after the reader is picked from two numbers: the refusal histogram over
the real-part corpus, and the first consumer's side-by-side run of its
suite against both backends (ADR-0017). Where they disagree, the
consumer's regressions rank first while there is a consumer waiting on a
swap, and the histogram after (ADR-0020). Each cycle earns its own
section, with an acceptance corpus and a number, when `/close-cycle`
opens it.

- **The NURBS cycle** — NURBS–NURBS surface intersection (a marcher with
  explicit seam handling); NURBS operands in booleans.
- **The blend-network cycle** — edge chains, vertex blends, variable
  radius, blends over blends; blends on the quadric face pairs outside
  ADR-0007's table.
- **The sweep cycle** — sweep along a path, loft, shell, offset.
- **The healing cycle** — healing; sheet and wire bodies in every
  operation.
- **The query cycle** — distance, clash, ray fire, selection; none of
  them has its machinery yet, since nothing in the kernel holds a
  hierarchy over face boxes or fires a ray at a surface outside
  `classify_point`.
- **The attribute cycle** — attributes a consumer attaches to entities,
  carried through every operation by declared rules over the split order
  ADR-0009 fixes, for a consumer with no naming scheme of its own
  (Parasolid's attribute definitions).
- **The breadth-and-speed cycle** — IGES, read and write; same-domain face
  merging; the performance pass the design reserved room for.

## What not to spend agent time on

A sketch constraint solver, rendering, UI, physics, drawings, mesh
processing beyond tessellation, formats beyond STEP and the native one,
and speed as a headline (`SEED.md` §4 non-goals). A backlog line that
lands in one of these is rejected with that reference.
