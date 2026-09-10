# 01 — Architecture

Arris is a library. It owns a *model* — an arena of geometry and topology —
and a set of *operations* that append to it and return handles. Everything a
consumer does is a call of the shape `op(&mut Model, inputs…) ->
Result<(Body, Provenance), OpError>`, followed by queries on the handle it
got back. There is no session object, no builder with hidden state, no
global. This document holds the crate layout, the arena and handle model,
the operation and error contract, where the checker runs, the threading and
wasm rules, and how a consumer's kernel facade maps onto the API. The
entities and geometry themselves are in [data-model](DATA-MODEL.md).

## Crates and the layer rule

One Cargo workspace, `crates/arris-*`, plus the facade crate `arris` that
re-exports the public API. Lower crates never name types from upper ones.

| Crate | Owns | External deps | Layer |
|---|---|---|---|
| `arris-math` | `Point3`/`Vec3`/`UnitVec3` (over `nalgebra`, ADR-0001), `Frame`, `Frame2`, `Isometry`, `Interval`, `Aabb`, `wrap_angle`, exact orientation predicates (over `robust`), polynomial and interval-guarded Newton root finding, `Precision` and `Tolerance` | `nalgebra`, `robust`, `serde` (feature) | 0 — representation |
| `arris-geom` | `Surface`, `Curve`, `Curve2` (analytic + NURBS): evaluation, derivatives, point projection, curve/curve, curve/surface and surface/surface intersection, bounding boxes over a parameter range, pcurves and the NURBS fit behind them; the (u, v) toolkit `region2` and `integrate` shared by the checker, tessellation, mass properties and classification; `GeomError` | `arris-math`, `thiserror`, `serde` (feature) | 0 — representation |
| `arris-topo` | `Model` (the arena), typed ids, `Shape`/`Body`/`Face`/… handles, orientation, entities, pcurves, per-entity tolerances, Euler operators, adjacency and iteration, `Provenance`; re-exports `arris-geom` and `arris-math` | `arris-geom`, `arris-math`, `thiserror`, `serde` (feature) | 0 — representation |
| `arris-check` | The invariant checker: `check(&Model, Body, Level) -> Report` and the `Violation` list of data-model §Invariants; re-exports `arris-topo` | `arris-topo` | 1 |
| `arris-ops` | Primitives, planar profiles, extrude, revolve, transform, booleans, later blends; `measure` (mass properties); each returns `Provenance` | `arris-check`, `thiserror`, `rayon` (feature) | 2 — algorithms |
| `arris-mesh` | `TriMesh`, `Polyline`, the constrained Delaunay triangulation in (u, v) (`cdt`, ADR-0003), tessellation of faces and edges with shared edge discretisation; re-exports `arris-math`'s `Aabb` | `arris-check`, `arris-topo`, `thiserror` | 2 — algorithms |
| `arris-io` | STEP AP214 Part 21 writer (later reader), the native format (`native`); re-exports `arris-check` | `arris-check`, `thiserror`, `serde`, `serde_json`, `postcard` (the last three behind the `serde` feature) | 2 — algorithms |
| `arris-debug` | Text dump, the hand-built sample bodies (`sample`), PNG render (own software rasteriser over `image`), Rerun stream (feature), the fixture loader and corpus lint, the corpus runner (`corpus`) and the oracle seam (`oracle`), the seeded property-test runner and strategies | `arris-ops`, `arris-mesh`, `arris-io`, `arris-topo`, `arris-geom`, `arris-math`, `image`, `serde`, `serde_json`, `sha2`, `thiserror`, `proptest` (not on `wasm32`), `rerun` (feature) | 3 — dev-facing |
| `arris` | Facade: re-exports | `math` through `io`; `debug` as a dev-dependency only | 4 |

`math`, `geom` and `topo` are the representation: they change rarely and a
change there is a design delta named in a plan. Everything from `check` up
is an algorithm crate that can be rewritten without touching its
neighbours. `ops`, `mesh` and `io` are siblings: none depends on another.
Mass properties live in `ops::measure` because they integrate over the
B-Rep, not over a mesh; a consumer that wants mesh-based inertia integrates
`arris-mesh`'s output itself.

The rule is enforced, not remembered: `tools/check-layers.sh` walks the
declared edges of `cargo metadata` with a layer number per crate and fails
on any edge that does not go strictly downward; dev-dependencies are exempt
so a lower crate's tests may use `arris-debug`, which is itself a
dev-dependency of the facade and never reaches a consumer. CI runs the
script and its self-test (a scratch copy with a forbidden edge must fail);
the pre-commit hook runs the script.

Every crate has `#![forbid(unsafe_code)]` and `#![warn(missing_docs)]`.
Feature flags are few and named the same in every crate that has them:
`serde` (on by default in `topo` and `io`; off by default in `math` and
`geom`, where `topo`'s feature turns both on because `Precision` and the
geometry are part of the native format), `parallel` (`rayon` inside `ops` and `mesh`; never enabled on
`wasm32`), `paranoid` (`ops`: run the checker after every operation in
release builds too, §The checker), `rerun` (`debug` only). The facade
forwards `serde`, `parallel` and `paranoid`.

## The model, the arena and handles

`Model` is the arena. It holds every vertex, edge, face, shell, body,
curve, surface and pcurve ever created in it, each behind a typed
generational id (`VertexId`, `EdgeId`, `FaceId`, `ShellId`, `BodyId`,
`CurveId`, `SurfaceId`, `Curve2Id`): a `u32` slot index and a `u32`
generation. Ids are allocated sequentially in creation order, and a slot is
reused only after a compaction (below) at a new generation, so in the
common case an id is also a creation timestamp, and iteration in id order
is deterministic on every platform. An accessor (`model.face(id)`) returns `NotFound` for an index
past the arena, a freed slot or a stale generation — never the slot's
current occupant. Geometry is inserted by value (`add_curve`,
`add_surface`, `add_curve2`) and never deduplicated: two faces share a
`SurfaceId` because the operation that split them handed both the same
id, not because the arena matched two equal surfaces.

**Entities are immutable.** An operation never edits an entity in place; it
appends new ones and returns a handle to a new body that references the
untouched old entities by id. Two bodies that share a face share its id,
its geometry and its tolerance — structural sharing is the default, not an
optimisation. This is what makes provenance cheap (an untouched entity keeps
its id; a modified one has a new id and a record), undo free (an older body
handle is still valid), and background evaluation safe (a clone of the model
sees the same entities).

**The arena is chunked and the chunks are shared.** Storage is a list of
fixed-size chunks (`arris_topo::CHUNK_SIZE` slots) behind `Arc`, one list
per entity and geometry kind; `Model::clone` copies the lists of `Arc`s,
and the first append after a clone copies only the tail chunk. Cloning a
model for a background evaluation is therefore O(number of chunks), not
O(entities), and two clones that diverge share every chunk they both leave
untouched.

**A handle is an id plus an orientation.** `Shape { id: EntityId,
orientation: Orientation }` is the uniform handle used by provenance,
iteration and errors; `Body`, `Shell`, `Face`, `Edge`, `Vertex` are typed
newtypes over the same pair and convert to `Shape` for free. Copying a
handle is copying two integers. Orientation composes down the hierarchy
(data-model §Orientation); a handle never carries geometry.

**Failed operations leave the model as it was.** Because the arena only
grows and refills slots `retain` freed, an operation runs inside
`Model::transaction(|m| …)`, which marks every arena at entry — its
length and its free set — and on `Err` drops everything appended since:
the tail of every arena and the freed slots the transaction filled,
emptied again at the generation they had, with the adjacency indices and
the next id rolled back with them — so the model, its ids and its clones
are exactly as before. Transactions nest; an inner `Err` undoes only the
inner appends. A consumer never sees half-built entities.

**Bodies move between models by import.** `Model::import(&mut self, &other,
body) -> Result<(Body, IdMap), TopoError>` deep-copies a body's closure —
sorted id order per kind, geometry first — inside a transaction and
returns the new handle with the id map; a reference in the other model
that does not resolve is `NotFound` and nothing is appended. This is how
independent evaluations on clones are merged, how a consumer holds several
documents, and how provenance across models is translated
(`Provenance::mapped`).

**Compaction.** `Model::retain(&mut self, keep: &[Body]) -> Result<usize,
NotFound>` frees every entity and geometry value not reachable from `keep`
— the slot's value dropped, its generation bumped so every handle to it
stops resolving instead of aliasing — rebuilds the adjacency indices, and
returns the count. A freed slot keeps its place and is filled by a later
append, lowest index first, at the bumped generation (ids `v3g1`, then
`v3g2`…), so a long-lived model does not grow without bound and ids stay
deterministic; a transaction that fails after filling freed slots empties
them again, and one that fails after a `retain` does not undo it. It is
the only operation that invalidates handles and it is never called by
the kernel itself. `⚠ OPEN:` whether compaction also renumbers slots
(denser arena, cheaper serialisation, but every surviving id changes and
the returned `IdMap` becomes mandatory for the consumer) or keeps slots
sparse, as it does now. Decided with the first consumer that outlives
one evaluation, in cycle 2.

## Operations

Every operation in `arris-ops` has the same shape:

```rust
pub fn cut(m: &mut Model, target: Body, tool: Body) -> Result<(Body, Provenance), OpError>;
```

- Inputs are handles into `m`. The operation reads them, appends, and
  returns a new handle plus the provenance record (data-model
  §Provenance) that says which output entity came from which input entity
  and how. An operation without a provenance record is unfinished.
- The operation never mutates its inputs, never panics on geometry, never
  contains a numeric literal that stands for a tolerance, and never iterates
  a hash map where the order can reach a geometric decision or an id.
- Parameters that are geometry (`Axis`, `Frame`, `Profile`) are plain
  values from `arris-math`/`arris-geom`, not handles: a primitive is built
  from numbers, and only the result lives in the model. `arris_math::Axis
  { origin, direction }` is a point and a unit direction, normalised by
  `Axis::new` (`Axis::z_at` for the common case); `primitive_cylinder(m,
  axis, radius, height)` places its frame by `Frame::from_z` of it, so it
  seams where the oracle's cylinder does, and `primitive_box(m, min,
  max)` takes two corners. Both build through the Euler operators
  (data-model §Euler operators) and return every entity `Generated`
  from a `Role`.
- Same input, same output, same ids, on every platform. The tests assert
  this by dumping twice and diffing.

A **query** has a different shape: it takes `&Model`, makes no body and
records no provenance, because there is nothing for a later operation to
name. `ops::measure::mass_properties(&Model, Body) ->
Result<MassProperties, OpError>` is the first — volume, area, centroid
and the inertia tensor about the centroid at unit density, in the
physical convention, with `MassProperties::inertia_about(point)` for any
other point. Every quantity is a flux integral over the body's faces by
Green's theorem in each face's own (u, v) (`geom::integrate`), as the
checker's B2 row already computes an enclosed volume: nothing is
discretised, so the numbers are the geometry's and not a mesh's, and the
corpus holds them to the oracle's within each fixture's tolerance. The
second moments are integrated about the centroid itself rather than
carried there by the parallel-axis theorem, which a small body far from
the origin would pay for in cancellation. A body that is not a `Solid`
is `OpError::Degenerate` with `Reason::NotSolid`; an invalid one
`InvalidInput`, as an operation's input is.

`ops::boolean::interferences(&Model, a, b) -> Result<Interferences,
OpError>` is the second query: the boolean decomposition of ADR-0004 as
a value, computed without building anything. It holds every face pair
whose boxes overlap with its `SurfaceIntersection`; every point where
an edge of one operand pierces a face of the other, kept when the
parameter is in the edge's range and the (u, v) is on the face
(`region2::point_side` on the face's loops, a `Boundary` verdict
resolved to the edge or vertex of the face within its own tolerance,
the edge's own end vertex when the hit is within its ball); those hits
merged into section vertices — a hit joins the first vertex whose point
is within the larger of the two tolerances or that shares an operand
vertex with it, and the vertex's tolerance is the largest of the
entities merged plus the spread of the points (data-model
§Tolerances); the paves each vertex puts on the edge that hit it and on
every section curve it projects onto within its tolerance; and the
section edges — the blocks between consecutive paves whose midpoint is
inside both faces, a closed curve with no pave seeded at its parameter
zero when it is interior to both — each with a pcurve on each face by
`pcurve_on`, translated by whole periods into the copy of the domain the
face's loops are written in, its tolerance the larger of the faces'
raised to the pcurves' residual. A `Coincident` pair — two faces on one
surface, the flush case — is decided by the same arrangement (ADR-0004):
the two faces' edges are intersected with one another and every
crossing is a section vertex; every edge of either face is paved by
every section vertex on it; and each piece of each edge between its
paves is matched to the piece of that face's edge it coincides with
as a *common block* when an edge of that face is `Coincident` with its
curve and the two pieces overlap (the same piece then, with the same
ends, or `Fault::CommonBlock`, a kernel bug: the vertices the edges
share paved them differently), placed on the other face as an *image*
with a pcurve there when it lies inside that face by its polygons, and
dropped when it lies outside — the coincidence is the curves' verdict,
never the polygon band's, which two fitted pcurves of one curve can
straddle. For the same reason a block of a section curve that is a
piece of an operand edge of either face is that edge and not a section
edge. A `Tangent` pair — a plane and a cylinder touching along a ruling
— contributes no section edge and no pave on any operand edge: the
ruling is paved by the *touches*, the hits of either face's edges on the
other face that lie on it (every curve in a face tangent to the other
surface is tangent to it there, so these are where the ruling leaves one
face inside the other), and each block between consecutive touches whose
midpoint is inside both faces is a *contact*, the segment the two faces
share. Every list is in a deterministic order and `Display` prints the
whole model, which is what the `inspect` skill reads when a boolean is
wrong. The property tests build their
operands through `arris_debug::prop::body` — a box and a cylinder whose
axis passes through the box, both under one random motion — and, for
the identities whose outcome has to be known in advance, its
`piercing_pair`: the cylinder clears every edge of the box, so `fuse`,
`common` and `box − cylinder` are one shell each and `cylinder − box` is
exactly two, the designed `MultiShell` refusal. The other pairs — the
wall crossing an edge, a corner sliced off — stay in `overlapping_pair`,
where `cut` is held to the identity when it succeeds and to that refusal
otherwise.

`ops::fuse(m, a, b)`, `ops::common(m, a, b)` and `ops::cut(m, target,
tool)` are three selections over that decomposition (ADR-0004), one
algorithm and one table. Every
face of both operands is split in its own (u, v): the pieces of its
loops between consecutive paves and the section edges on it make a
planar arrangement — half-edges ordered around each node by the pcurves'
tangent angle, a tie within the angular tolerance by the signed
curvature, a tie of both `Reason::TangentContact` — whose regions are
walked by taking the next half-edge clockwise from the direction one
arrived from; a cycle turning once counter-clockwise bounds a piece, one
turning clockwise is a hole, assigned by winding to the innermost piece
around it. Each piece is classified at `region2::interior_point`
carried to 3D by `classify_point` against the other operand, and the
table decides:

| Piece of | `fuse` | `common` | `cut` |
|---|---|---|---|
| A (the target) | kept when outside B | kept when inside B | kept when outside the tool |
| B (the tool) | kept when outside A | kept when inside A | kept when inside the target, reversed |
| a coincident face | once, from A, when the normals agree | once, from A, when they agree | once, from A, when they oppose |

The coincident row is read at a piece classified `On` a face of the
other operand that its own face is coincident with: the two effective
normals at the piece's interior point agree or oppose, and the piece is
kept from A alone, in A's orientation, `Modified` from A's face and
`Generated` from B's, or dropped by both. The images of B's edges split
A's face along B's boundary and B's images split A's, so the piece is
exactly the overlap; a common block is one edge of the result, A's
piece, and every use of B's piece is rewritten to it with a pcurve
fitted to A's curve in that use's translate of the domain (a seam's two
uses get two), B's piece `Modified` into A's. A piece classified `On` a
face its own face is tangent to has its interior point on the ruling
and lies to one side of the other operand everywhere else; which side is
the *curvature rule*: the cylinder lies on its axis's side of the shared
tangent plane and the plane lies outside the cylinder's surface, so the
plane's piece is inside the cylinder's body exactly when that body is
the outside of its wall (a bore, the wall's outward normal pointing at
the axis) and the cylinder's piece is inside the plane's body exactly
when the axis is on the material side of the plane. Before any face is
split, every contact is decided at its midpoint by the same rule and the
table: a contact whose two pieces would both survive is two result faces
touching along a curve interior to both, and the operation is
`Degenerate` with `Reason::TangentContact` naming the pair — a hole wall
tangent to a side face, or a `fuse` of two solids that touch along a
line — because the manifold `Solid` cannot carry the slit (ADR-0004). A
touch from outside in a `cut` or a `common` passes: the tool's piece is
dropped, the target's kept whole, and the ruling is no edge — Open
CASCADE imprints it, and `boolean/tangent-outside-cut` states that
convention. A piece `On` an edge or a vertex, or on a face its own is
neither coincident nor tangent with, is `OpError::Unsupported` naming
the pair. The survivors are grouped by shared edges — none is `Degenerate`
with `Reason::Empty`, or with `Reason::ZeroThickness` when what was
dropped lay on the other operand (two solids sharing only a face), more
than one group `Reason::MultiShell` — and assembled through
`Builder::assemble` with every untouched entity of a kept-by-id operand
`Keep`: a face whose loops changed at all, even only by a split edge or
a re-tolerated vertex, is a new face `Modified` from the old; the tool
of a `cut` keeps nothing, every entity of it `Deleted` and each
surviving piece `Generated` from its parent (data-model §Provenance).
A `fuse` and a `common` have no tool: both operands are kept by id, so
an untouched face of either keeps it, and the result's shell and body
are `Modified` from both operands' where a `cut`'s are `Modified` from
the target's alone. Tolerances follow §Tolerances' growth rule and a
piece keeps its parent's.

Sweeps take a planar `Profile` — an outer loop and holes of lines and arcs
in a plane's own (u, v) — and build the planar face themselves (`ops::
planar_face`), so a consumer's sketch never has to become topology before
it becomes a solid.

### Errors

`OpError` is a `thiserror` enum and every variant names the entities
involved, so the message a consumer shows — or the agent reads — says
*which* face pair, *which* edge, not "boolean failed":

| Variant | When | Carries |
|---|---|---|
| `InvalidInput` | an input body fails the checker (checked in debug builds before the operation starts, and in release when the `paranoid` feature is on) | `Body`, the `Report` |
| `Unsupported` | the exhaustive dispatch reached a surface or curve pair the kernel has no formula for yet | the two `GeomKind`s with their entities |
| `Degenerate` | the requested result has no valid representation: a parameter that makes no geometry (`Reason::NonFinite`, `Reason::NotPositive` naming it — a zero radius, a box whose `min` is not below its `max`), a zero-thickness intersection, a profile crossing its revolve axis, a sweep of zero length; a boolean that selects no material (`Reason::Empty`: a target inside its tool, a `common` of disjoint operands), whose survivors make more than one shell (`Reason::MultiShell { shells }`: a split target, a disjoint fuse, a cavity), or whose faces touch along a curve interior to both result faces (`Reason::TangentContact`) | the entities (none for a primitive) and a `Reason` enum |
| `Tolerance` | the result would need an entity tolerance above `Precision::max_tolerance` | the entity, the tolerance it wanted |
| `NotFound` | a handle does not resolve in this model (wrong model, or compacted away) | the `Shape` |
| `Internal` | a kernel bug the operation caught: the checker rejected its own output, the builder refused a step of its fixed sequence, a frame could not be placed from inputs it had validated, a point it had to classify could not be, a geometry query failed on validated input for a reason other than a missing closed form, a section edge crossed a seam the seam's own hit should have paved, the (u, v) arrangement of a face was not the subdivision the pave model promised (`SplitFault`: a dangling section edge, a cycle not turning once, a hole inside no piece, a piece with no interior point, a pave at an edge's end) | a `Fault` — the `Report`, the `BuildError`, the `FrameError`, the `ClassifyError`, the `GeomError`, the two faces of the seam crossing, or the `SplitFault` naming the face |

`Internal` is returned only in release builds with `paranoid` on; in debug
builds the same condition panics (below). A degenerate *result* that the
consumer might reasonably want anyway (the flush intersection that is a
face, not a solid) is `Degenerate` with a reason, never a silently empty
body: the kernel does not decide what fail-soft means. Every operation
runs inside `Model::transaction`, so on any `Err` the model — its ids
included — is as it was.

## The checker

`arris-check` is its own crate so that no algorithm crate can skip it by
accident and so that its dependency list stays at exactly `arris-topo`.
`arris_check::check(&model, body, Level) -> Report` (`topo` sits below the
checker, so it is a free function over the model, not a method) returns
every violation with the entity that violates it; `Report::is_ok()` is
what every test asserts and what every operation asserts on its own
output. A body handle that does not resolve is one M1 line; a reference
that does not resolve is reported under M1 and skipped by every other
row; adjacency is read off the body's own entities, so every row stands
without the arena's indices and M2 alone speaks for them. Every report also carries the body's Euler–Poincaré line,
`Report::euler()`, which is a line and not a violation. The invariants are
listed in data-model §Invariants; each has a `Violation` variant, a test
that constructs it and sees it reported, and a level:

- `Level::Fast` — combinatorial and local geometric checks (ids resolve,
  loops close, orientations compose, tolerances are ordered, pcurves match
  their 3D curves at sample points). Linear in the body. This is what runs
  after every operation.
- `Level::Full` — adds the global checks: an edge does not cross itself,
  the loops of a face do not cross, faces of a shell intersect only at
  shared edges, shells nest, a solid encloses positive volume. Not
  linear. Runs on demand, in the fixture corpus and in `/close-cycle`.

`arris_check::classify::classify_point(&model, body, point) ->
Result<Classification, ClassifyError>` is B1's ray cast made public and
complete: `Inside`, `Outside`, or `On(Shape)` naming the most specific
entity the point is within the tolerance of — the vertex, else the edge,
else the face. The boundary test comes first, by the entities' own
tolerances; only a point that is on nothing is cast for, and then the
eight fixed directions are tried in order, a direction abandoned on a
boundary, tangent or coincident hit, with all eight abandoned reported as
`ClassifyError::Undecided` naming the body and the point. B1 is the same
code over one shell's faces, so the row that proves a shell nesting and
the predicate that decides which piece of a split face a boolean keeps
can never disagree about a point (ADR-0004). It lives in `check` because
that is where B1 already was, and `ops` depends on `check`; the facade
re-exports it.

A `Full` row the kernel has no closed form for — a face pair whose
surfaces the intersector cannot intersect, a shell no containment ray
could be classified against — is never guessed at and never quietly
passed: it is listed by `Report::unchecked()`, printed with a `?` after
its row number, and left out of `is_ok()`. An operation that cannot afford
an undecided row asks for the list.

**In debug builds every operation runs `Level::Fast` on its output before
returning `Ok`, and panics with the report if it fails.** A checker failure
after an operation is a kernel bug, and the kernel's own invariants are the
one place a panic is allowed (`.agents/rules/kernel.md`). A test that needs
to build an invalid body — every checker test does — constructs it through
`arris-topo`'s raw insert, which the checker does not guard, and says so by
name.

**In release builds nothing runs unless asked.** `arris_check::check` is
public and cheap enough for a consumer to run after every feature; the
`paranoid` feature turns the debug behaviour on in release, returning
`OpError::Internal` instead of panicking.

The checker never repairs. Healing is an operation (a later cycle), and it
returns provenance like any other.

## Geometry dispatch

`Surface` and `Curve` are open enums, not trait objects (`SEED.md` §9).
Every intersection, projection and classification is an exhaustive `match`
over the pair of variants, so adding a variant makes every dispatch fail to
compile until it is handled, and a pair without an exact formula is a
`GeomError::Unsupported` arm naming both kinds (`OpError::Unsupported`
once an operation wraps it with the entities) — never a wildcard falling
back to a generic marcher where a closed form exists. The results are
enums too: `SurfaceIntersection::{Empty, Coincident, Transversal,
Tangent}` for a surface pair, `CurveSurfaceIntersection::{Points,
Coincident}` for a curve against a surface and `CurveIntersection::{
Points, Coincident}` for two curves, so a caller matches the case rather
than counting curves or points. A pair may be supported in part: two
cylinders are `Coincident` or `Empty` when they are coaxial and
`Unsupported` in every other pose, since the crossing curve is a quartic
and cycle 2's — which is still a named arm, not a wildcard. The NURBS variant is one arm like the
others; a NURBS–NURBS marcher, when it comes, is what that arm calls, and
analytic pairs never route through it.

## Tessellation

`arris_mesh::tessellate(&Model, Body, chord) -> Result<TriMesh,
MeshError>` turns a body into a triangle mesh with a `FaceRange` per
face and an `EdgeRange` per edge, both in the body's iteration order
(data-model §Adjacency and iteration). The chord tolerance is the
consumer's request — a number like a render's resolution, not a model
tolerance — validated finite and positive (`MeshError::Chord`); in debug
builds the body passes the checker at `Level::Fast` first, as every
operation's input does (`MeshError::InvalidInput`), which is why `mesh`
depends on `check`. The mesh guarantees (ADR-0003):

- Positions are `f64` and exact evaluations of the geometry: a topo
  vertex's point, an edge's curve at a sampled parameter, a surface at
  an interior grid point. Nothing is welded or snapped.
- A topo vertex is one mesh vertex and an edge's samples are one index
  run, so a mesh of a `Solid` is closed by construction. Every edge is
  discretised once, at `n` uniform parameters — the largest of its
  curve's `chord_segments` at the chord and, per coedge, the count that
  keeps each step's (u, v) travel under the face's surface's
  `chord_steps` — and its `EdgeRange` is the polyline from its start
  vertex to its end vertex along its curve's parameter.
- A face's loops are the *same* parameters through each coedge's
  pcurve, triangulated in (u, v) by the constrained Delaunay
  triangulation of `arris_mesh::cdt` and mapped back to the shared
  indices. A seam edge is discretised once and its indices appear in the
  wall's triangles twice, once from each copy of the pcurve; a
  degenerate edge's (u, v) segment maps to one index and the triangles
  that collapse are dropped.
- Triangles are counter-clockwise seen from outside, by the face use's
  effective orientation against the surface normal.
- Same body, same chord, same mesh on every platform, with the
  `parallel` feature on or off.
- A face whose surface curves in both directions — a sphere, a torus, a
  NURBS surface — carries interior points on a uniform (u, v) lattice at
  its `chord_steps` spacing, those its loops wind around; a ruled
  direction has an infinite step, so a plane, a cylinder and a cone take
  none and their loops' own samples bound the chord. The rings and the
  lattice are scaled by the surface's mean speeds before the
  triangulation, so Delaunay's criterion measures distance on the
  surface and not in the parameters (ADR-0003).
- A face whose loops are not the simple nested polygons the checker
  promises is `MeshError::Face` with the `CdtError` naming the segments.

No adaptive refinement: interior points, where a face needs them, lie on
a uniform (u, v) grid sized by the chord bound. No `f32` output (the
`⚠ OPEN` under §Threading), no per-vertex normals or (u, v), no
mesh-based mass properties (`ops::measure` integrates the B-Rep).

## Threading and wasm

- Every public type is `Send + Sync`. There is no global mutable state, no
  thread-local cache, no interior mutability in the representation crates.
- Operations take `&mut Model`: one operation at a time per model. That is
  the contract, not a limitation to be worked around with locks. Parallelism
  *inside* an operation (face pairs in a boolean, faces in a tessellation)
  is `rayon` behind the `parallel` feature and must give identical output
  with the feature off — the tests run both ways. Parallelism *across*
  operations is clone-evaluate-import, above.
- `wasm32-unknown-unknown` builds every crate with default features; CI
  checks it. No kernel crate touches the filesystem, the clock, threads or
  randomness; `arris-debug` is the only crate that writes files, and the
  oracle is not a crate at all.
- `f64` everywhere inside. `⚠ OPEN:` whether `TriMesh` offers an `f32`
  position buffer at the tessellation boundary or leaves the conversion to
  the consumer. Decided when the first renderer consumes it, in cycle 2.

## Formats and tools

- **STEP AP214** (`arris_io::step::write(&model, &[bodies]) -> Result<
  String, StepError>`): the writer is a cycle-1 deliverable because the
  oracle reads Arris's output through it. One product whose shape
  representation lists a `MANIFOLD_SOLID_BREP` per solid body of one
  shell; per face an `ADVANCED_FACE` whose `same_sense` is the shell's use
  of it, a `FACE_OUTER_BOUND` for the loop of positive winding and
  `FACE_BOUND`s for the rest, each with the same flag as `same_sense`
  (the stored loop is counter-clockwise about the surface normal, STEP's
  bound about the face's effective one); per edge an `EDGE_CURVE` over a
  `SURFACE_CURVE` — a `SEAM_CURVE` when both uses are in one loop — holding
  the 3D curve and one `PCURVE` per use, so a reader takes the model's own
  trimming; the analytic surfaces and curves on `AXIS2_PLACEMENT_3D`
  (origin, `Z`, `X`, as `gp_Ax3` reads them) and the `B_SPLINE_*` entities,
  rational ones as complex entities, so the writer is exhaustive over the
  geometry enums. Millimetres and radians, since the reader scales to
  millimetres by default and Arris carries no unit; the uncertainty is
  `default_tolerance`; the time stamp is empty and every real is the
  shortest round-trip decimal, so two writes are byte-identical. What
  STEP cannot hold: a degenerate edge's coedge is left out of its loop (as
  Open CASCADE's writer does; a loop of nothing else is
  `StepError::Unsupported`), and a left-handed pcurve conic is written on
  the direct placement STEP has, losing the traversal sense (the reader
  reprojects, and ignores plane pcurves anyway). Sheet, wire and general
  bodies and solids with voids are `Unsupported` until an operation
  produces them.
- **Native format** (`arris_io::native::{to_json, from_json, to_bytes,
  from_bytes}`): `serde` of the model under a version header, JSON for
  diffs and `postcard` bytes for storage; data-model §Native format.
- **Text dump** (`arris-debug::dump_text`): the deterministic, diffable
  rendering of a body that fixtures store and tests compare. Not a format:
  it has no reader.
- **The oracle** (`tools/oracle/`, Python 3.12, Open CASCADE through the
  `cadquery-ocp` wheels in a `uv` environment): `expected.py` builds each
  fixture's recipe in OCCT and writes `expected.json`; `compare.py` reads
  an Arris STEP file and compares it against that within the fixture's
  tolerances; `selftest.py` proves the oracle against closed forms and its
  own STEP. A second fixture kind, `geometry` (`tests/fixtures/geom/`),
  has no solid: named analytic surfaces and curves that the oracle
  evaluates, projects onto and intersects, and that
  `crates/arris-geom/tests/oracle.rs` compares Arris against — the
  parametrisation's ground truth. It is run, never linked; no crate
  depends on it. The fixture format is `tests/fixtures/README.md`; its
  role is roadmap §Fixtures.
- **`arris-debug`** is dev-facing: the text dump (`dump_text`), the
  sample bodies built by hand through the raw insert with explicit
  pcurves (`sample::{cuboid, cuboid_nurbs, unit_box, cylinder, sphere,
  torus, patch}` — what the checker's tests start from, since they
  cannot use `arris-ops`; `sphere` is the body with a seam and two
  degenerate pole edges, `torus` the genus-1 one with two seams and no
  pole, and `patch` a rectangular sheet of any surface kind, the
  tessellation's property tests' operand) and through the Euler
  operators (`sample::frame`, the genus-1 twin of `boolean/frame-cut`), the rasteriser (`render_png`) and the samplers
  that feed it a curve or a surface without a body (`polyline_of`,
  `wireframe_of`), the Rerun stream, the fixture loader and corpus lint
  (`fixtures`), the corpus runner (`corpus::run`, the fixture test of
  roadmap §Fixtures — checker, counts and genus, the oracle's reading
  of the STEP, the mass properties against the oracle's within the
  fixture's tolerances, the mesh closed and within `mesh_volume_rel`,
  every probe classified as the oracle classifies it
  (`classify_point`, exactly: both sides have their own tolerance for
  "on" and a probe is placed so the two agree, so a disagreement is a
  finding and never something a band is widened to cover), provenance
  accounting, the dump; `ARRIS_BLESS=1` writing `dump.txt`; a result the
  oracle recorded no solid for must fail with `OpError::Degenerate`, and
  one the recipe marks `analytic.expect_error` with that typed refusal,
  the run ending there with the oracle's numbers kept as the record of
  what Open CASCADE builds; one whose recipe states a convention Arris
  does not follow, `analytic.counts_differ`, held to the recipe's own
  counts with the oracle's kept as the record)
  over the
  oracle seam (`oracle::compare`: STEP under `target/inspect/`, then
  `compare.py` through `uv`, a missing environment a loud error), and
  the seeded property-test runner and strategies (`prop`,
  with every analytic surface and curve in a random pose and random
  clamped NURBS curves and surfaces under `prop::geom`). It is a
  dev-dependency of the workspace's crates and never of a consumer. For
  the crates below it (`math`, `geom`, `topo`) that dev-dependency is a
  cycle, so their property tests are integration tests under
  `crates/<crate>/tests/`, where the crate is linked once and its types
  unify; a `#[cfg(test)]` module would see two copies.

## How a consumer's kernel facade maps on

The first consumer programs against a facade trait of its own and swaps the
backend behind it. Its surface maps onto Arris one-to-one; nothing in the
facade needs Arris types above it.

| Facade needs | Arris provides |
|---|---|
| Primitives (box, cylinder) | `ops::primitive_box`, `ops::primitive_cylinder` |
| Extrude / revolve of a sketched profile with holes | `ops::extrude`, `ops::revolve` over `Profile` (lines and arcs) |
| Boolean union / intersect / cut | `ops::fuse`, `ops::common`, `ops::cut` |
| Transform (geometry only, topology and index order preserved) | `ops::transform` — new ids, provenance `Modified` one-to-one in iteration order |
| Fillet / chamfer of named edges, one call for all edges | `ops::fillet`, `ops::chamfer` (cycle 2) |
| Tessellation into a render mesh with per-face and per-edge ranges | `arris_mesh::tessellate` → `TriMesh` with `FaceRange`/`EdgeRange` keyed by `FaceId`/`EdgeId` |
| A planar face's frame | `Face::surface()` is `Surface::Plane { frame }`; the frame *is* the answer, and it is stable across re-evaluation because the primitive's frame is |
| Mass properties (volume, area, centroid, inertia) | `ops::measure::mass_properties` → `MassProperties` (exact over the B-Rep, the tensor about the centroid); or the consumer's own integrator over `TriMesh` |
| STEP export of several bodies | `io::step::write(&model, &[bodies])` |
| Projecting an edge or vertex onto a sketch plane | `geom::project_to_plane` on the edge's `Curve` — a line stays a line, a circle becomes a circle or an ellipse, an ellipse stays an ellipse, a NURBS a `Curve2::Nurbs`; a point-set projection with the variant's own parameter (02 §Pcurves) |
| Persistent topological names (origin-based) | Emitted by the consumer from `Provenance`: an output face is named after the input face it was `Modified` from, `Split(k)` when one input yields several outputs, and after the tool face when `Generated`; edges and vertices derive from their faces exactly as today. No centroid matching. `⚠ OPEN:` whether Arris ships the origin-name grammar as a helper (`arris-topo::naming`) or leaves it to the consumer; the seed lists this among the kickoff questions. Decided in cycle 2 with the consumer's adapter |
| Memoising shapes by content, dropping unreferenced ones | Memoisation stays in the consumer (it is about features, not geometry); dropping is `Model::retain` (§Compaction, `⚠ OPEN`) |
| Units | Arris is unit-agnostic. The consumer sets `Precision` for its unit (metres: `default_tolerance` at the micrometre scale) when it creates the `Model` |

What the facade has today that Arris will not have: a tolerance nudge
(there is none; a degenerate boolean is `OpError::Degenerate`), and a
"which surface came from which" matcher (provenance replaces it).

## Open questions

Collected from this document; each closes with an ADR.

- `⚠ OPEN:` compaction renumbers slots or keeps them sparse (§The model).
- `⚠ OPEN:` `f32` positions at the tessellation boundary (§Threading).
- `⚠ OPEN:` origin-name helper in Arris or in the consumer (§Facade).
