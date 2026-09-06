# Plan: m2-topology

- Started: 2026-09-06
- Milestone: M2 (cycle C1, docs/03-roadmap.md)
- Idea (verbatim from the human): "/plan m2-topology" — the roadmap's M2
  section is the brief; no idea file.

## Goal

A box and a cylinder exist as bodies in the arena and Open CASCADE reads
them back with the right numbers. `arris-topo` is the arena of
01-architecture — chunked `Arc` storage, typed generational ids, the five
entities of 02-data-model with loops and coedges inside faces, adjacency
indices, deterministic iteration, transactions, `import`, a sparse
`retain`, the raw insert API for tests — and a builder whose Euler
operators are the only way an operation makes topology. `arris-check`
enforces every row of 02 §Invariants at `Fast`, plus L5, S5, B1, B2 and E8
at `Full`, with one test per row that constructs the violation and sees it
reported alone. `ops::primitive_box` and `ops::primitive_cylinder` (one
seam edge, two closed circular edges) build through the builder, run the
checker in debug builds, and return `Generated` provenance for every
entity. `io::step` writes the B-Rep subset with pcurves and seams so the
oracle reads it; `io::native` round-trips a model to an identical text
dump; `arris_debug::dump_text` is that dump. The corpus runner exists and
the two `primitive/*` fixtures pass end to end.

## Non-goals

No operation that takes a body as input: no transform, boolean, sweep or
measurement (M3–M5). No tessellation and no PNG of a body (M3). No 3D
point classification as a public operation (M4) — the checker's B1 needs a
private containment test and keeps it private. No STEP reader (C7). No
compaction that renumbers slots: `retain` is the sparse variant and the
`⚠ OPEN` in 01 §The model stays open for C2. No healing, no tolerance
growth beyond what a primitive sets. No `Sheet`, `Wire` or `General` body
from any operation — the representation and the checker rows (B3, S2's
per-kind counts) accept them; only the raw API and tests build them.

## Design deltas

- **`arris-topo` public API** (01 §The model, 02 §Topology): `Model` with
  `Model::new(Precision) -> Result<Model, TopoError>` (an inconsistent
  `Precision` is refused) and `Default` over `Precision::DEFAULT`;
  `precision()`; typed accessors `vertex(VertexId) -> Result<&Vertex,
  NotFound>` and likewise `edge`, `face`, `shell`, `body`, `curve`,
  `surface`, `curve2`; geometry insert `add_curve`, `add_surface`,
  `add_curve2` (values, never deduplicated); the entity structs of 02
  §Entities in `arris_topo::entity` (`Vertex`, `Edge`, `EdgeGeometry`,
  `Face`, `Loop`, `Coedge`, `Shell`, `Body`, `BodyKind`) with read-only
  field access; the raw insert API `Model::raw() -> RawInsert<'_>` that
  appends any entity unchecked (public because the checker's tests live in
  `arris-check`; documented as test scaffolding); adjacency queries
  `edge_uses(EdgeId) -> &[CoedgeRef]` (`CoedgeRef { face, loop_index,
  coedge_index }`), `vertex_edges(VertexId) -> &[EdgeId]`,
  `face_shells(FaceId) -> &[ShellId]`, model-wide (an entity may be shared
  by several bodies; per-body questions filter through the closure);
  iteration `shells(body)`, `faces(body)`, `edges(body)`, `vertices(body)`
  yielding handles with the *effective* orientation in the order of 02
  §Adjacency and iteration, and `closure(body) -> Closure` (sorted id sets
  per kind, what the checker, `import`, `retain` and the dump walk);
  `transaction(|m| …) -> Result<T, E>` truncating the arena and its indices
  on `Err`; `import(&mut self, &Model, Body) -> Result<(Body, IdMap),
  TopoError>`; `retain(&mut self, &[Body])`; `IdMap` (per-kind
  `BTreeMap`s, `map(Shape)`); `TopoError::{NotFound(Shape), Precision,
  …}` over `thiserror`. `arris-topo` re-exports `arris_geom` and
  `arris_math` (as `math` re-exports `nalgebra`, ADR-0001), so
  `arris-check`'s declared dependency list stays exactly `arris-topo` and
  it still evaluates surfaces.
- **The builder and Euler operators** (new section 02 §Euler operators;
  ADR-0002, step 9): entities are immutable and Euler operators mutate —
  the two meet in `arris_topo::Builder`, opened on a model inside a
  transaction, whose operators edit a body *under construction* and whose
  `finish(BodyKind) -> Result<Body, TopoError>` appends the frozen
  entities in a deterministic order and commits. The set is Mäntylä's
  (*An Introduction to Solid Modeling* ch. 9), adapted to coedges and
  seams: `mvfs`/`kvfs`, `mev`/`kev`, `mef`/`kef`, `kemr`/`mekr`,
  `kfmrh`/`mfkrh` — descriptive Rust names with the abbreviations in the
  docs. Insertion points are coedge positions (face, loop, index), never
  bare vertices, because a vertex may occur several times in one loop (a
  closed edge, a seam). Geometry is explicit: `mev` takes the new
  vertex's point, the edge's `CurveId` and range and the *two* pcurves of
  its two coedges (a seam is exactly a `mev` strut whose two pcurves
  differ by the period); `mef` takes the curve, the range, the new face's
  `SurfaceId` and one pcurve per side (decided, §Open questions). The
  builder never computes a pcurve and never runs the checker (it cannot:
  `topo` is below `check`); a primitive's correctness is proven by the
  checker on its output. Nothing outside `Builder` appends topology except
  `RawInsert` and `import`.
- **`arris-geom` gains the (u, v) toolkit** the checker, tessellation (M3)
  and classification (M4) share (02 §Pcurves gains a paragraph): `region2`
  — a `Polygon2` discretised from a sequence of `(Curve2, Interval,
  Orientation)` pieces at a chord tolerance (two points for a line, a
  deviation-bounded count for the conics and NURBS), `signed_area`,
  `winding_number(Point2)` by `orient2d` crossings, `segments_intersect`
  and self/mutual intersection over the polygons with exact predicates,
  the gap between consecutive pieces; and `integrate` — a surface-domain
  integral over a region bounded by pcurve pieces by Green's theorem along
  the pieces with Gauss–Legendre quadrature of a named order
  (`GAUSS_ORDER`, with the comment saying what it resolves). M1's non-goal
  "no 2D classification until M4" moves here because L4 is a `Fast` row.
- **`arris-check` public API** (01 §The checker): `check(&Model, Body,
  Level) -> Report`; `Report` gains `euler() -> Option<EulerLine>`
  (`EulerLine { vertices, edges, faces, loops, shells, genus }`, the
  genus *implied* by the counts as the oracle derives it, the line's
  parity being the check — 02 §Euler–Poincaré reworded to say so) and
  `unchecked() -> &[Unchecked]` (a `Full` row the kernel could not decide:
  an S5 face pair whose surfaces the intersector reports `Unsupported`;
  never silently passed, never a violation). 01 §The checker's
  `model.check(body)` becomes `arris_check::check(&model, body, level)`
  through the facade — `topo` cannot call `check` — a doc correction.
- **Provenance** (02 §Provenance): the `generated` and `modified` keys
  become `Origin::{Entity(Shape), Role(Role)}` so a primitive, which has
  no input body, can say what each entity *is*: `Role::Box(BoxPart)`,
  `Role::Cylinder(CylinderPart)` — exhaustive, M5 adds the profile roles.
  `origins(output)` returns `Vec<(Relation, Origin)>` and a chain through
  `then` ends at a `Role`, which is what a consumer's persistent name is a
  function of (01 §Facade). `Provenance::mapped(&IdMap)` translates a
  record across `import`. Decided, §Open questions; documented in
  ADR-0002's consequences.
- **`arris-ops` public API** (01 §Operations, §Errors): `primitive_box(m,
  min: impl Into<Point3>, max) -> Result<(Body, Provenance), OpError>`,
  `primitive_cylinder(m, Axis, radius, height)`; `OpError` with the six
  variants of 01 §Errors and `Reason`; the debug-build checker run (panic
  with the `Report`) and the `paranoid` feature (`OpError::Internal`).
  `arris_math::Axis { origin: Point3, direction: UnitVec3 }` with
  `Axis::z_at`, `Axis::new` — a plain value, as 01 says geometry
  parameters are. `arris-ops` declares `arris-check` only and reaches the
  lower crates through the re-exports.
- **`arris-io` public API** (01 §Formats and tools): `step::write(&Model,
  &[Body]) -> Result<String, StepError>` — AP214 Part 21; the B-Rep
  subset with `SEAM_CURVE` for a seam edge and `SURFACE_CURVE` with one
  `PCURVE` per coedge otherwise, `FACE_OUTER_BOUND`/`FACE_BOUND` by
  winding, `B_SPLINE_*` (rational as complex entities) so the writer is
  exhaustive over `Surface`, `Curve` and `Curve2`; the model's
  `default_tolerance` as the context's uncertainty; fixed float
  formatting; entity numbers in iteration order so the file is
  deterministic. `native::{to_json, from_json, to_bytes, from_bytes}` over
  `serde` — `postcard` for bytes (new external dependency; the §Crates
  row), `serde_json` for diffs — with a format version whose mismatch is
  `NativeError::Version`. `StepError`/`NativeError` name the body or
  entity.
- **`arris-debug`**: `dump_text(&Model, Body) -> String` per 02 §Native
  format's last paragraph (iteration order, ids, effective orientations,
  geometry and tolerances at fixed precision, pcurves, the Euler line);
  `sample::{unit_box, cylinder}` — hand-built through the raw API with
  explicit pcurves, the valid bodies the checker's tests start from (they
  cannot use `ops`); `corpus::run(dir, variant) -> Result<(), CorpusError>`
  — the fixture test of 03-roadmap §Fixtures: build the recipe (`box`,
  `cylinder`; every other `Step` is `CorpusError::Unsupported` naming the
  op), `check` at `Full`, counts and genus against `expected.json`, STEP
  to the scratch directory and `compare.py` through `uv run` (a missing
  environment is a loud failure, never a skip), provenance accounting,
  `dump.txt` diffed — written only under `ARRIS_BLESS=1`. M3 adds the
  `measure` comparison. The facade `arris` re-exports the crates as
  modules (`arris::{math, geom, topo, check, ops, io}`) — its first
  non-placeholder contents.
- **Fixtures**: `tests/fixtures/boolean/frame-cut` — box [0,0,0]–[40,30,10]
  − box [10,10,−1]–[30,20,11]; V 10000, A 4000, 16/24/10/12, genus 1 — a
  genus-1 body with two-loop faces that M2 builds by hand through the
  Euler operators (step 9) and M4 rebuilds by recipe: the cross-check
  between the two paths, as `sweep/extrude-plate-with-hole` is for
  `through-hole`. The roadmap's corpus table gains the row.
- **CI**: the `test` job runs `uv sync --project tools/oracle` before
  `cargo test`, since the corpus test runs `compare.py`.
- **ADR-0002** — Euler operators over a staging builder inside a
  transaction; explicit pcurves; provenance rooted in roles. Names
  Mäntylä ch. 9, OCCT's `BRep_Builder` (not Euler-based: the contrast) and
  `BRepTools_History` (roles as the missing root of its records). Step 9.

## Steps

- [x] Step 1 — The arena. `Model`: chunked storage behind `Arc` with a
  named `CHUNK_SIZE`, one typed slot vector per entity and geometry kind,
  generational ids minted in creation order, `Model::new(Precision)`,
  the accessors returning `NotFound` for a missing index *or* a stale
  generation, geometry insert, `entity::*` structs, `Model::raw()`
  appending unchecked, `Model: Clone + Send + Sync` sharing chunks.
  Tests: ids are sequential and identical across two builds; a clone
  shares every chunk (`Arc::ptr_eq`) and the first append after a clone
  copies only the tail chunk; a stale id is `NotFound` and never aliases;
  a `Precision` that is not consistent is refused; `serde` round trip of
  an entity; the raw insert accepts a dangling reference (it is raw).
- [x] Step 2 — Adjacency, iteration, closure, transactions. The three
  indices maintained on every append (raw or otherwise); `shells`,
  `faces`, `edges`, `vertices` in the order of 02 §Adjacency and
  iteration with effective orientations composed by XOR down the path;
  `closure(body)`; `transaction` recording every vector's length and
  truncating all of them, indices included, on `Err`. Tests: on a
  hand-built box the iteration visits each entity once, depth-first,
  first visit, and the order is the same on two builds; an edge shared
  by two faces has two uses naming (face, loop, index); a face used by
  two shells of one `General` body lists both; a transaction that fails
  after appending three entities leaves lengths, indices and the next id
  exactly as before; `closure` of a body that shares faces with another
  contains only its own reach.
- [x] Step 3 — The text dump and the sample bodies. `arris_debug::
  dump_text` and `sample::{unit_box, cylinder}` built through the raw API
  with the pcurves of 02 §Pcurves (the cylinder: one wall face whose one
  loop is bottom circle, seam up, top circle, seam down, the two seam
  pcurves at `u = 0` and `u = 2π`; each cap's circle pcurve a `Curve2::
  Circle` whose `Frame2` handedness follows the cap's normal against the
  circle's `Z`). Tests: the dump lists the seam edge twice with opposite
  signs and the Euler line `2/3/3/3 … = 0`; the box's line is `8/12/6/6`;
  two dumps of two fresh builds are byte-identical; every id in the dump
  resolves.
- [x] Step 4 — The STEP writer, and the seam through Open CASCADE (the
  risk step). `io::step::write` as in the design deltas; read
  `STEPControl` and `StepToTopoDS` in the reference tree and
  `truck-stepio` for the entity structure a reader accepts (nothing
  copied); the oracle's `step.py` is the reader. Tests: `compare.py`
  matches `primitive/box` and `primitive/cylinder` on the sample bodies —
  volume, area, centroid, counts (the seam once, 2/3/3/3), every probe;
  a box whose one edge carries a degree-1 `Curve::Nurbs` and whose one
  face is a bilinear `Surface::Nurbs` (raw-built) also matches
  `primitive/box`, proving the B-spline arms; the file is byte-identical
  across two writes; a `Wire` body is `StepError::Unsupported`. A
  convention mismatch here is fixed in Arris in this step.
- [x] Step 5 — Checker `Fast`, part 1: references, vertices, edges. `check`
  and the closure walk; rows M1–M3, V1–V3, E1–E7 (E4 at
  `Precision::check_samples` parameters along each coedge's pcurve
  against the 3D curve; E7 the seam's two pcurves differ by exactly the
  period). Tests: the sample box and cylinder are clean; one test per
  row that takes a sample body, breaks it through the raw API (a
  dangling `CurveId`, a NaN coordinate, a vertex tolerance below the
  floor, an edge whose range leaves the domain, a pcurve shifted by half
  a period, a seam with both coedges forward, a degenerate edge with a
  curve, …) and asserts the report holds that violation on that entity
  and nothing else; the report order is deterministic.
- [x] Step 6 — The (u, v) toolkit in `arris-geom`. `region2::{Polygon2,
  discretise, signed_area, winding_number, intersections}` and
  `integrate::{region_integral, GAUSS_ORDER}` as in the design deltas.
  Tests (1000 cases): the signed area of a discretised circle of either
  handedness is `±πr²` within the chord bound; the winding number of a
  random point against a random convex polygon agrees with the
  half-plane test, and is 0 outside; a polygon crossing the seam range
  (`u` beyond `2π`) has the area its unwrapped pcurves say; two loops
  built disjoint report no intersection and two built crossing report
  the pair; `region_integral` of 1 over a planar rectangle and over the
  cylinder wall's `[0, 2π] × [0, h]` recovers the areas to 1e-12·scale,
  and of the Gauss volume integrand over the sample cylinder's faces
  `π·16·12` to 1e-9 relative.
- [x] Step 7 — Checker `Fast`, part 2: loops, faces, shells, bodies, the
  Euler line. Rows L1–L4 (L4's winding through `region2`, one positive
  loop per connected component, holes negative and inside), F1–F2, S1–S4
  (S2 per body kind), B3, and `Report::euler()`. Tests: one violation
  test per row (an open loop; a junction gap above `parametric_
  tolerance`; an edge used twice by one loop off a periodic surface; a
  face with two counter-clockwise loops; a hole outside its outer; a
  face without loops; a face's tolerance above an edge's; a face used
  twice by a shell; a solid's edge with one coedge; two disconnected
  face sets in one shell; a wire with a shell); the sample bodies' Euler
  lines are `8/12/6/6/1 g0` and `2/3/3/3/1 g0`; a body with an odd line
  is reported.
- [x] Step 8 — Checker `Full`: L5, E8, S5, B1, B2. L5 over `region2`'s
  intersections; E8 trivially true for the analytic curves, by polyline
  self-intersection within the edge's tolerance for a NURBS; S5 over
  every face pair of a shell through M1's `intersect_surfaces` — `Empty`
  passes, `Coincident` goes on to a domain-overlap test in (u, v),
  `Transversal`/`Tangent` curves are sampled and each point classified
  against both faces' loops, a point interior to both (not within the
  edge's tolerance of a shared edge) is the violation, `Unsupported`
  pairs land in `unchecked`; B2 through `integrate` (the sign test, the
  value reported); B1 with the signed volume of each shell (exactly one
  positive) and a private containment test for voids (a ray from a
  void's vertex against the outer shell's faces through
  `intersect_curve_surface` and `region2`, direction chosen
  deterministically away from every vertex and edge within tolerance).
  Tests: the sample bodies are clean at `Full`; violations: a face
  whose loop self-crosses in (u, v); two overlapping faces in one shell;
  an inside-out cylinder (every face use flipped: negative volume, B2)
  and, for B1, a void shell outside its outer and two outer shells; a
  body with a `Nurbs` face pair reports it under `unchecked`, not as a
  violation.
- [x] Step 9 — ADR-0002, the builder and the Euler operators. `Builder`
  with the ten operators and `finish` as in the design deltas; the
  fixture `boolean/frame-cut` (recipe, `expected.py` run, `selftest.py`
  green). Tests: every operator keeps the Euler line at zero (counts
  tracked by the builder and asserted after each call); each operator
  followed by its inverse leaves the builder's dump identical
  (property test over random small sequences); the cylinder built as
  `mvfs` → `mef` to the same position with the bottom circle → `mev`
  with the seam line and its two pcurves → `mef` with the top circle is
  clean at `Full` and matches `primitive/cylinder` through `compare.py`;
  the frame built with `mef`/`mev` for the hole's rim and walls and
  `kfmrh` to join the bottom is clean at `Full` (genus 1, two two-loop
  faces) and matches `boolean/frame-cut`; a `finish` on a builder whose
  body is not closed as a `Solid` is a typed error, and the transaction
  leaves nothing behind.
- [ ] Step 10 — Provenance, `OpError`, the primitives. `Provenance` with
  `Origin`, the three relations, the queries, `then`, `mapped`;
  `arris_math::Axis`; `primitive_box`, `primitive_cylinder` through the
  builder in a transaction, tolerances at `default_tolerance`, the caps'
  planes taking the axis as `Z` with the bottom cap used `Reversed` (as
  the reference tree's cylinder primitive does, so a future reader test
  sees the same placements); the debug checker run and `paranoid`.
  Tests: both primitives are clean at `Full` and their dumps are
  identical across two runs and across two models; every entity of each
  output appears exactly once under a `Role` and nothing is `Modified`
  or `Deleted`; `then` is associative on random small records (1000
  cases) and `origins` inverts `generated_from`; zero or negative radius
  or height, `min` not below `max`, a non-finite coordinate →
  `OpError::Degenerate`/`InvalidInput` naming the reason, with the model
  unchanged; a cylinder whose axis is not unit-length is normalised by
  `Axis::new`; `compare.py` matches both `primitive/*` fixtures on the
  primitives' STEP.
- [ ] Step 11 — The native format. `native::{to_json, from_json,
  to_bytes, from_bytes}` with the version header. Tests: the box, the
  cylinder and the frame round-trip through both encodings to a
  byte-identical dump, with the same ids; two writes are byte-identical;
  a bumped version and a truncated byte stream are typed errors; the
  wasm32 build passes with `postcard` in.
- [ ] Step 12 — `import` and `retain`. `import` deep-copies the closure in
  iteration order and returns the `IdMap`; `retain` frees every slot not
  reachable from the kept bodies, bumps its generation and reuses it
  lowest-index-first for the next appends (decided, §Open questions).
  Tests: a cylinder imported into a fresh model dumps identically up to
  the id map, and `Provenance::mapped` carries its record; importing
  twice gives distinct ids; after `retain(&[box])` in a model holding a
  box and a cylinder, the box is clean at `Full`, every cylinder handle
  is `NotFound`, the next primitive reuses the freed slots at generation
  one and the box's ids are unchanged; two models built by the same
  calls agree on every id after a `retain`.
- [ ] Step 13 — The corpus runner and the facade. `arris_debug::corpus::
  run` and `crates/arris/tests/corpus.rs` with one test per fixture:
  `primitive/box` and `primitive/cylinder` live, the thirteen others
  `#[ignore = "M4: …"]`/`"M5: …"` naming the milestone; `dump.txt` for
  the two blessed and committed; the facade re-exports; CI's `test` job
  syncs the oracle. Tests: the two live fixtures pass every stage —
  `Full` checker, counts and genus, `compare.py` match, provenance
  accounting, dump diff; a recipe with an unsupported op fails with the
  op's name; a dump that differs by one id fails with the diff.

## Acceptance

`ARRIS_PROPTEST_CASES=1000 cargo test --workspace` green: the `primitive/*`
fixtures end to end (checker at `Full`, counts and genus exact, the
oracle's reading of Arris's STEP matching volume, area, centroid, counts
and every probe, provenance accounting, `dump.txt` diffed) and the
hand-built `boolean/frame-cut` twin matching its oracle values; every one
of the 29 invariant rows has a violation test that reports exactly its
violation; the native round trip dumps identically for three bodies; two
runs of every primitive produce identical dumps; `uv run --project
tools/oracle tools/oracle/selftest.py` green with the new fixture; the
layer check and the wasm build pass; CI green on `main`. Then tag `m2`
(the human's).

## Docs to update on completion

- `docs/03-roadmap.md` §M2 — status line: date, what was retired (the
  seam through the checker and through Open CASCADE's reader; Euler
  operators over immutable entities), ADR-0002, the provenance `Origin`
  delta; §C1 acceptance corpus — the `boolean/frame-cut` row; §Fixtures —
  `dump.txt` blessing, `measure` comparison deferred to M3.
- `docs/01-architecture.md` §Crates — `arris-topo` (builder, re-exports),
  `arris-check` (deps through `topo`), `arris-io` (`postcard`,
  `serde_json`); §The model — the builder, transactions, `import`,
  `retain` as implemented (the renumbering `⚠ OPEN` stays); §Operations
  — the two primitives and `Axis`; §The checker — `arris_check::check`
  wording, `euler`, `unchecked`; §Formats and tools — the STEP subset
  written, native encodings, `dump_text`, the corpus runner.
- `docs/02-data-model.md` §Entities — as implemented; new §Euler
  operators; §Adjacency and iteration — the index types and `closure`;
  §Pcurves — the (u, v) toolkit; §Invariants — the S5 `unchecked` rule,
  the Euler line as derived genus with parity; §Provenance — `Origin`,
  `Role`, `mapped`; §Native format — encodings and version.
- `docs/adr/README.md` — ADR-0002 in the table.
- `tests/fixtures/README.md`, `tools/oracle/README.md` — the runner,
  `ARRIS_BLESS`, the new fixture.
- `.agents/skills/inspect/SKILL.md` — the dump and checker rows now exist.
- `AGENTS.md` current state — "M2 done <date>; next `/plan
  m3-tessellation`".
- `docs/BACKLOG.md` — anything a step defers, under "Findings".

## Open questions

Each with a recommendation; the human decides before `/work` reaches the
step, as for M1.

- **Decided (agent, 2026-09-06, step 9 — the human ran steps 9–13
  non-interactively and delegated the open questions, so the
  recommendation was taken): Euler operators take explicit pcurves**
  (`mev` two, `mef` one per side, every one `Option`, `set_pcurve` for
  the rest) rather than computing them from the curve and the faces'
  surfaces — a seam's second pcurve is the first shifted by the period
  and `pcurve_on` cannot know which use it is building; the caller (a
  primitive now, the boolean in M4) always knows. ADR-0002.
- `⚠ OPEN:` **Provenance origins become `Origin::{Entity, Role}`** so a
  primitive's entities are `Generated` from a role rather than from
  nothing, and a naming chain has a root. Recommendation: yes, with
  `Role` an exhaustive enum extended per operation kind; the alternative
  (a fourth relation `Created` with no origin) leaves the bolt-pattern
  rebuild fixture with nothing to name a hole wall after. Human, by
  step 10.
- **Decided (human, 2026-09-06, step 6): the (u, v) toolkit and the
  region integral live in `arris-geom`**, shared by the checker (L4, L5,
  S5, B1, B2), M3's tessellation and `measure`, and M4's classification —
  while the checker's 3D containment test for voids stays private to
  `arris-check`. The checker must not depend on `ops`, so a shared
  integrator can only sit below both. 02 §Pcurves documents it.
- `⚠ OPEN:` **`retain` reuses freed slots** lowest-index-first at the
  bumped generation, so a long-lived model does not grow without bound
  before C2 decides on renumbering. Recommendation: reuse — it is what
  the generation exists for, and ids stay deterministic because the free
  list is ordered. Agent, by step 12; the C2 `⚠ OPEN` in 01 §The model is
  untouched.

## Findings

- Step 1: `NotFound` is a struct carrying `AnyId` (entity or geometry id),
  not `TopoError::NotFound(Shape)`, because the accessors for `curve`,
  `surface` and `curve2` must name a geometry id and a `Shape` cannot;
  `TopoError::NotFound` wraps it. `Interval` gained a `serde` derive (its
  `Deserialize` validates through `Interval::new`) and `arris-math`'s
  `serde` feature turns on `nalgebra/serde-serialize`, since a vertex's
  point is part of an entity's encoding.
- Step 2: the adjacency queries return `Result<&[_], NotFound>` rather
  than a bare slice, so a stale id is an error and never an empty answer;
  the indices are one `Arc` value shared by clones and copied whole on the
  first append after a clone (02 §Adjacency and iteration says so), since
  a per-chunk index would have to edit earlier chunks on every append. A
  vertex handle yielded by iteration carries the orientation of the edge
  use that reached it — meaningless geometrically, stated in the docs.
- Step 3: `sample::cuboid(m, min, max)` joins `unit_box` (which is the
  unit cube through it), so step 4 can compare the sample box against
  `primitive/box`'s 40×30×10 extents; `sample::cylinder(m, radius,
  height)` takes its sizes for the same reason. The samples return
  `SampleError` for an extent that is not finite and positive and leave
  the model untouched. `arris_debug::euler_line` is public beside
  `dump_text` (step 7's `Report::euler()` is the checker's own; the dump
  keeps computing its line from the closure). The dump writes a dangling
  reference as `<id> ?` rather than skipping it.
- Step 4: no convention mismatch — Open CASCADE reads the sample cylinder
  back valid with the seam's two pcurves exactly as written, and its own
  STEP of the same cylinder has the same structure (`SEAM_CURVE` with the
  forward use's pcurve first, `ADVANCED_FACE` and bound flags both from
  the face use). Two things STEP cannot hold are documented in
  `arris_io::step` and 01 §Formats and tools: a degenerate edge (its
  coedge is left out of the loop, as the reference writer does) and a
  left-handed pcurve conic (`AXIS2_PLACEMENT_2D` is always direct, so the
  traversal sense is lost; the reader ignores plane pcurves, where every
  such conic of cycle 1 lives). Solids with voids, sheets, wires and
  general bodies are `StepError::Unsupported` until an operation makes
  them. `arris-check` re-exports `arris-topo` and `arris-io` re-exports
  `arris-check`, so each crate above declares one dependency; `arris-io`
  gained `thiserror`. `sample::cuboid_nurbs` is the NURBS-probe box. The
  outer bound's winding is a private sampled signed area in `arris-io`
  until step 6's `region2::signed_area` replaces it. CI's `test` job syncs
  the oracle now, since the STEP tests run `compare.py`.
- Step 5: three rows of 02 §Invariants could not be implemented as
  written and were reworded (the drift test holds the two lists
  together). **M2** "reachable through a parent that lists it" names
  nothing in this arena — the walk *is* reachability — so it is now the
  one row about the adjacency indices: a reference that did not resolve
  when its parent was appended is never indexed (the only way to make
  one is a raw insert naming a later id); `Violation::NotInClosure` is
  `NotIndexed`. Every other row reads adjacency off the closure's own
  entities, so the rows do not depend on the indices being right. **E2**
  duplicated V2 (the same distance, reported on the edge instead of the
  vertex); it is now the structural half only — `start == end` exactly
  when the curve returns over the range — with
  `EndMismatch::OffVertex` replaced by `OpenWithOneVertex { vertex, gap }`
  and the unused `EdgeEnd` removed. **E6**'s "no curve" is unrepresentable
  (`EdgeGeometry::Degenerate` has none), so `DegenerateFault::HasCurve`
  is gone; and "singular along its pcurve" needs a range, which a
  degenerate edge had nowhere to keep: `EdgeGeometry::Degenerate` now
  carries `range: Interval` (02 §Entities; `Edge::range()` covers both
  kinds; the dump prints it). V2, V3 and E4 measure one triangle from
  three corners (vertex, curve end, surface at the pcurve end), so a
  moved range end reports under V2 and V3 and a shifted pcurve under V3
  and E4 — each violation test names its full set rather than pretending
  to one line. E7 compares seam pcurves in (u, v) with
  `parametric_tolerance` divided by the surface's speed in each
  direction, unscaled where a direction is singular to rounding.
- Step 6: `region2::Piece` carries `reversed: bool` rather than
  `Orientation`, which is `arris-topo`'s and cannot be named below it; a
  caller maps the coedge's orientation. `discretise` takes
  `f64::INFINITY` for "the minimum counts", which is what a sign needs
  (the STEP writer's outer bound uses it; L4 will too), while a finer
  chord tolerance is bounded by `MAX_SEGMENTS_PER_PIECE` and the deviation
  actually achieved is reported, never silently met. Gauss–Legendre
  nodes are computed by Newton to a fixed point rather than tabled, so
  there is no 17-digit literal to mistype and every platform gets the
  same values to rounding. Pieces built corner to corner meet only to
  rounding, and a ring that kept the rounding-length closing segment
  tripped its own self-intersection test (the segment touched a
  non-adjacent side); a loop is closed by definition, so `discretise`
  joins consecutive pieces at the earlier piece's last point and closes
  the last onto the first, reporting the gap through `gaps()` instead of
  drawing it — L2 is where a gap is judged.
- Step 7: three rows were reworded in 02 §Invariants (the drift test holds
  the codes together; the wording is the plan's to move). **L2**'s
  exception is not "a seam edge" but any jump of exactly one period in a
  periodic parameter, which is what a seam *and* a closed edge whose
  pcurve wraps the parameter both are; the bound is
  `parametric_tolerance` scaled to the surface's speed, as E7's is.
  **L4**'s "non-zero signed area" needed a bound that is the model's and
  not a literal: a loop encloses nothing when its mean width — its area
  over half its (u, v) perimeter — is at or below the parametric
  tolerance there. **S2**'s "orientations pairing up" is stated for every
  body kind, not `General` alone: as many forward uses as reversed, but
  for an odd count, where exactly one is left over; a `Wire` body's shell
  is left to B3. L1 and F1 report once per loop and per face
  respectively, at the first break — every later one is its consequence.
  `Report::euler()` is taken at both levels since it is linear in the
  closure, and 02 §Euler–Poincaré now says the genus is derived and the
  parity is the check. Six step-5 tests grew lines: the new rows are the
  same faults read from the loop's, the face's and the shell's side (a
  moved vertex opens a loop, a shifted pcurve gaps its junctions, a face
  coarser than its edge is E5 *and* F2), and each names its full set.
- Step 9: the builder is a value, `Builder::new(tolerance)`, and
  `finish(&mut Model, BodyKind)` takes the model, rather than a builder
  opened on a `&mut Model` — so a caller interleaves `add_curve` with the
  operators instead of front-loading every geometry value, and the
  transaction is the caller's (`finish` runs its own nested one, so a
  refused finish appends nothing). `finish` returns `Built { body, shell,
  vertices, edges, faces }` — the slot → id maps step 10's provenance
  needs — not a bare `Body`. Every pcurve parameter is `Option<Curve2Id>`
  with `Builder::set_pcurve` beside the operators: a strut a `mef` will
  move to another face has no pcurve worth giving on the face it is made
  in (a vertical strut in a horizontal face has none at all), and a
  coedge `mef` moves keeps an id on the wrong surface until replaced, so
  the sample frame gives every pcurve in one pass after the topology is
  done. `finish` makes `Solid` only: Euler operators build closed
  surfaces, so a sheet with a single-use edge is unreachable and the
  `Sheet` allowance was dead code. The inverse property needed three
  representation decisions the plan did not foresee — tombstoned slots
  with a LIFO free list, loops in a canonical rotation and order, and a
  `from = to + len` position for the split that moves a whole loop from
  a junction other than its first — each found by the property test.
  `kfmrh` requires one `SurfaceId` and opposite orientations on its two
  faces (the pocket floor on the bottom's plane), which the sample frame
  satisfies by creating the plug on the bottom's surface at `mef` time.
  `arris_debug::oracle::compare` is the seam to `compare.py` (STEP under
  `target/inspect/`, a typed `Mismatch` with the table, `Environment`
  for a missing `uv`) that the step-4 test duplicated inline and step
  13's runner will use; `sample::frame` is the `boolean/frame-cut` twin.
- Step 8: `Report::unchecked` needed a second variant. B1's ray cast can
  fail the same way S5's face pair can — no closed form, this time for a
  line against the shell's surface — so `Unchecked` is
  `{ FacePair, ShellNesting }`, not S5's alone; both are printed with a
  `?` after the row number and are outside `is_ok`. `Polygon2::
  self_intersections` and `intersections` became a sweep in `u`
  (`arris-geom`, step 6's module): L5 discretises a loop within the
  model's parametric tolerance, which is fourteen thousand segments for a
  cylinder cap's circle, and the quadratic pair test made a debug run of
  the corpus minutes long. The answer is unchanged — the same pairs,
  ascending — and the worst case is still every pair. S5's transversal arm
  samples the intersection curve over the parameters *both* faces'
  boundaries reach, found by projecting each face's polygon onto the
  curve; the coincident arm carries a grid of each face's interior points
  through 3D onto the other surface, so an overlap finer than the grid is
  not seen (L5 and the curve test cover a crossing). E8 is a no-op for the
  analytic curves — over a range E1 accepted they cannot cross — so only a
  NURBS is sampled, at `check_samples` per knot span. B1 reads each
  shell's Gauss volume for the outer/void split and casts a ray only to
  place a void; `ShellNestingFault::InsideOut` is what is left for a
  non-outer shell of zero volume.
