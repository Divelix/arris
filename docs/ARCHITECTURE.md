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
| `arris-math` | `Point3`/`Vec3`/`UnitVec3` (over `nalgebra`, ADR-0001), `Frame`, `Frame2`, `Axis`, `Isometry`, `Interval`, `Aabb`, `wrap_angle`, exact orientation predicates (over `robust`), polynomial and interval-guarded Newton root finding, `Precision` and `Tolerance` | `nalgebra`, `robust`, `serde` (feature) | 0 — representation |
| `arris-geom` | `Surface`, `Curve`, `Curve2` (analytic + NURBS): evaluation, derivatives, point projection, curve/curve, curve/surface and surface/surface intersection, bounding boxes over a parameter range, pcurves and the NURBS fit behind them; the (u, v) toolkit `region2` and `integrate` shared by the checker, tessellation, mass properties and classification; `Profile`, the planar sketch of lines and arcs a sweep takes, validated and oriented by `Profile::edges`; `GeomError` | `arris-math`, `thiserror`, `serde` (feature) | 0 — representation |
| `arris-topo` | `Model` (the arena), typed ids, `Shape`/`Body`/`Face`/… handles, orientation, entities, pcurves, per-entity tolerances, Euler operators including the assembly seam (`Assembly::of_body`, `effective_uses`, `AssemblySlots`), the Euler line (`euler::EulerLine`), adjacency and iteration, `Provenance` and its audit; re-exports `arris-geom` and `arris-math` | `arris-geom`, `arris-math`, `thiserror`, `serde` (feature) | 0 — representation |
| `arris-check` | The invariant checker: `check(&Model, Body, Level) -> Report` and the `Violation` list of data-model §Invariants; the shared face domain (`domain::FaceDomain`), point classifier (`classify::Classifier`, `classify_point`) and region flux (`flux::face_flux`) every `Full` row, the boolean and tessellation read a face through; re-exports `arris-topo` | `arris-topo`, `serde` (feature, forwarded to `arris-topo`) | 1 |
| `arris-ops` | Primitives, extrude and revolve of a `Profile`, transform, booleans, the blends; `measure` (mass properties); each returns `Provenance` | `arris-check`, `thiserror`, `rayon` (feature) | 2 — algorithms |
| `arris-mesh` | `TriMesh`, `Polyline`, the constrained Delaunay triangulation in (u, v) (`cdt`, ADR-0003), tessellation of faces and edges with shared edge discretisation; re-exports `arris-math`'s `Aabb` | `arris-check`, `arris-topo`, `thiserror`, `rayon` (feature) | 2 — algorithms |
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
declared edges of `cargo metadata` with a layer number per crate — its
place in the chain `math` ← `geom` ← `topo` ← `check` ←
`ops`/`mesh`/`io` ← `debug` ← `arris`, finer than the table's tiers — and
fails on any edge that does not go strictly downward; dev-dependencies
are exempt so a lower crate's tests may use `arris-debug`, which is itself a
dev-dependency of the facade and never reaches a consumer. CI runs the
script and its self-test (a scratch copy with a forbidden edge must fail);
the pre-commit hook runs the script.

Every crate has `#![forbid(unsafe_code)]` and `#![warn(missing_docs)]`.
Feature flags are few and named the same in every crate that has them:
`serde` (on by default in `topo` and `io`; off by default in `math` and
`geom`, where `topo`'s feature turns both on because `Precision` and the
geometry are part of the native format; `check` carries the same feature
forwarded to `arris-topo/serde` and nothing of its own, the one path `io`
has there without depending on `topo`'s own default features), `parallel`
(`rayon` inside `ops` and `mesh`; never enabled on
`wasm32`), `paranoid` (`ops`: run the checker after every operation in
release builds too, §The checker), `rerun` (`debug` only). Every internal
workspace dependency is declared `default-features = false` at
`[workspace.dependencies]`, so `default = [...]` on a member (`topo`,
`io`, `arris`) is what a plain path or version dependency on it actually
gets; a consumer that wants the layer-legal path without a crate's own
defaults depends on it directly with `default-features = false` and
forwards the feature itself, as `arris`'s own `serde` feature and
`cargo check -p arris --no-default-features` do (`arris-ops`, `-mesh`
and `-io` reach no serde type at all with it off — `cargo tree -p arris
--no-default-features -e normal` has neither `serde` nor `serde_json`,
which CI asserts). The facade forwards `serde`, `parallel` and
`paranoid`.

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

`ops::transform(m, body, motion: &Isometry)` moves a body rigidly. Every
curve and surface is appended transformed and every pcurve id is reused as
it stands — a rigid motion carries the parametrisation with it, so
parameter space does not move — and every vertex, edge, face, shell and
the body itself is appended new in the body's own iteration order through
`Builder::assemble`, each `Modified` one-to-one from the entity it moved.
The body's kind is kept and nothing of the input is shared, so the moved
body is an operand a boolean can take beside the original. It reaches
exactly as far as `assemble` does — shells that share nothing, each one
edge-connected, carried shell by shell in the body's stored order, each
`Modified` from the one it moved — and it is how the property tests put
their operands in random poses. It is `arris_topo::builder::Assembly::
of_body(m, body, &mut remap) -> Result<(Assembly, BodyIndex), NotFound>`
(data-model §Euler operators) plus a `GeometryRemap` that moves a point,
a curve and a surface by the motion: the walk over the body's closure, the
`Assembly` it describes and the `BodyIndex` provenance is built from are
shared with a boolean's own assembly, so `transform`'s own work is the
remap alone.

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
a value, computed without building anything. A face pair, or an edge
against a face, whose boxes overlap and that has a face on a cone, a
sphere or a torus is refused before any intersector is asked — the
*quadric guard*, `OpError::Unsupported` naming the pair exactly as the
intersector named it before the coaxial arm existed (ADR-0008) — so a
new intersector arm widens no boolean silently; booleans with quadric
operand faces are C3's, with their corpus. It holds every face pair
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
§Tolerances); the *section crossings* — two section curves of one
`Transversal` pair intersected with each other (`intersect_curves`), a
crossing on both faces being a section vertex by the same merge, since
two curves of one pair meet where the surfaces are tangent to each
other, the two ellipses of equal cylinders with crossing axes at `±R`
along the axes' common perpendicular, and no edge of either operand is
there to make a hit; a *touch* — a hit where the edge meets the surface
without crossing it — makes no vertex of its own, but one that lands on
a vertex made by the hits and crossings joins it, since the edge passes
through that vertex (a seam ruling or a rim circle through the crossing
of two ellipses, tangent to the other wall there because the walls are);
the paves each vertex puts on the edge that hit or touched it and on
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
edge; whether an edge runs along the curve is `curves_coincide`, the
verdict without the points, so a rim circle in the plane of the ellipse
its own cap plane cuts from the other wall — two short cylinders crossing
steeply — needs no closed form for where the two conics would meet. An
edge that lies in a face of the other operand
(`Interferences::coincident`) is paved and placed the same way whether
or not a face of its own is coincident with that face: a seam on the
ruling two parallel walls cross along splits the other wall as an image,
since no coincident neighbour is there to place it. A `Tangent` pair — a
plane and a cylinder, or two parallel cylinders, touching along a ruling
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
exactly two, two lumps of one solid that hold the identities like the
rest. The other pairs — the wall crossing an edge, a corner sliced off —
stay in `overlapping_pair`, where `cut` is held to the identity whatever
number of lumps it makes. Two cylinders come from `parallel_pair` — axes
apart between the two tangent distances, some with a seam on a ruling or
flush caps — and `crossing_pair` — equal radii crossing at 30° to 90°,
each through the other, some with a seam through a crossing vertex, its
`common` held to `16R³/(3 sin ψ)` and either `cut` to
`Reason::NonManifold`.

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
the *curvature rule*. With `n` the other face's effective outward normal
at the contact, each surface leaves the shared tangent plane across the
contact curve as `κ s² / 2` along `n`, `κ` its normal curvature across
the curve (`Surface::normal_curvature`, the second fundamental form over
the first) signed against `n`; the other body lies on the side of its
surface away from `n`, so a piece is inside it exactly when its own `κ`
is below the other's. The two surfaces agree along the curve to second
order, so any direction across it decides the same, and each surface is
read along its own normal crossed with the curve's tangent. For a plane
and a cylinder it is the plane outside the cylinder's surface and the
cylinder on its axis's side of the plane; two parallel cylinders touching
are `−1/R₁` against `+1/R₂` outside and two distinct radii inside, never
equal, since equal radii touching inside share their axis and are
`Coincident`. Equal curvatures, compared exactly, are a touch of higher
order the rule cannot decide, `OpError::Unsupported` naming the pair.
Before any face is
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
the pair. The survivors are grouped into shells by shared edges — none is
`Degenerate` with `Reason::Empty`, or with `Reason::ZeroThickness` when
what was dropped lay on the other operand (two solids sharing only a
face); an edge piece used by more than two faces, or a vertex two shells
reach, is two lumps touching and `Reason::NonManifold` naming it, before
anything is assembled — and several shells are ordered into lumps, each outer shell then
the voids inside it (ADR-0006), by assembling them once into a clone of
the model and reading `arris_check::lumps` of that body, so the order is
B1's own; a split target, a disjoint `fuse` and a cavity are each one
solid. The shells are then assembled through
`Builder::assemble` with every untouched entity of a kept-by-id operand
`Keep`: a face whose loops changed at all, even only by a split edge or
a re-tolerated vertex, is a new face `Modified` from the old; the tool
of a `cut` keeps nothing, every entity of it `Deleted` and each
surviving piece `Generated` from its parent (data-model §Provenance).
A `fuse` and a `common` have no tool: both operands are kept by id, so
an untouched face of either keeps it, and the result's body is `Modified`
from both operands' where a `cut`'s is `Modified` from the target's
alone; a result shell is `Modified` from the operand shells its pieces
came from, and a cavity made of a cut tool's pieces alone is `Generated`
from the tool's shell. Tolerances follow data-model §Tolerances' growth rule and a
piece keeps its parent's.

The keep-by-id assembly and the provenance writer above are
`arris-ops`'s own `rebuild` module (ADR-0004), not the boolean's: a
`Policy` per operand (`Reuse`, an untouched entity kept by id and a piece
`Modified` from its parent; `Regenerate`, every entity `Deleted` and a
surviving piece `Generated` from it, the tool of a `cut`) turns the
pieces a boolean has already decided on into an
`Assembly` and, once `Builder::assemble` returns it, writes the generic
half of their provenance — every operand entity kept, modified or
deleted, the shell reconciliation, a coincident piece's stand-in — from
the same `AssemblySlots` `transform` reads its own outputs through. The
boolean layers its own two relations on top — a section vertex's or
edge's `Generated` from the face pair that made it, meaningless without
`Interferences` — inline, over the `Provenance` the writer returns.
`boolean()` is `rebuild`'s first caller; a blend is its second, through
`rebuild::rewrite`: one operand, its faces kept by id unless their loops
change, its edges and vertices kept unless a new edge is a piece of one
or a face's new loops no longer reach them, the blend faces added after
each shell's own, and the same generic provenance — kept unrecorded, a
piece or a replacement `Modified`, the rest of what is gone `Deleted`,
each shell and the body `Modified` — over which the blend adds its
`Generated` records.

`ops::fillet(m, body, edges: &[Edge], radius)` blends the listed edges
with a rolling ball, one stripe per edge built in closed form from the
edge's two faces (ADR-0007). The table: two planes blend to a cylinder
of the radius on the line where the faces' offset planes meet — its
frame's `X` at one contact ruling and `Z` along the edge, so the contacts
sit at `u = 0` and `u = π − φ` for normals `φ` apart and `v` is the
edge's own parameter — with the contact on each plane the line at
`r tan(φ/2)` from the edge, a `Line` pcurve there and a ruling on the
cylinder. A plane against a cylinder along a ruling blends to a cylinder
too: the ball's centre is on the plane's offset by `r` and on the
cylinder coaxial with the face at `R − r` or `R + r` — the ball inside
the face's cylinder or outside it — the one of their two lines on the
edge's side of the axis, and its contact with the face is the ruling
through that centre, a `Line` pcurve at fixed `u` in the translate of
the face's own loop; the contacts sit at `u = 0` and at the angle
between them about the blend's axis, no longer `π − φ`. That pair has no
chamfer in the table, and no miter: its contact on the cylinder misses
the other blend's on the third edge, so a corner with a ruling blend is
`VertexBlend`. A plane against a cylinder along a circle — a hole's rim,
a boss's base — is a closed edge and blends with no ends: a torus
coaxial with the cylinder, its centre circle at `R + sσr` for `s` `−1`
on a convex edge and `σ` the side of the axis the cylinder's outward
normal points to, minor radius `r`, one quarter of its tube between the
contacts; it chamfers to the 45° cone through the same two circles at
`distance`. Each contact is the edge's own circle moved along the axis
or widened, so it keeps the edge's frame and range. The blend's frame
has the cylinder's `Z` and its `X` at the edge's vertex, so its `u` seam
— a tube circle of the torus, a ruling of the cone — runs between the
two contacts' vertices in the half-plane of the cylinder's own seam,
which is shortened to the contact on the cylinder; that vertex must
carry no other edge, else `VertexBlend`. A torus that would not be a
ring torus, a contact that reaches the axis or a seam shorter than the
trim is `BlendTooLarge`. `ops::chamfer(m, body, edges, distance)` is the same
operation cut flat: two planes chamfer to the plane through the lines at
`distance` from the edge along each face — its frame's origin on one
contact, `X` across to the other and `Y` along the edge — and every end
segment and corner line is the chord between two points of the
construction, exact on every plane it lies on, never the intersector's.
Convex or concave is read from the
dihedral, and the contact curves come from the construction, never from
the intersector. Each end is trimmed by the face across the corner — at a
vertex of three edges, the plane the corner's other two edges share:
the section is a circle when that plane is perpendicular to the edge
and an ellipse when oblique, exact on the plane and a `Line` or a
fitted `Nurbs` on the cylinder by the oblique-section rule, at the arc's
own tolerance; the corner vertex goes, the corner's other two edges are
shortened on their own curves to the arc's ends, and the face across
takes the arc in its loop. Two blends meeting at a vertex whose third
edge stays sharp meet in a miter: the two equal-radius cylinders' axes
cross at the ball's one centre, and the miter is the ellipse of the
plane through it bisecting the axes — minor radius `r` toward the shared
face, major radius `r / sin(ψ/2)` for the edges `ψ` apart — from the
point where the two contacts on the shared face cross to the point on
the third edge where the other two contacts meet it, written from the
construction and fitted as a `Nurbs` pcurve on each cylinder; the third
edge is shortened to that point, no arc enters any face, and the miter
edge belongs to both blend faces. The two far contacts meet the third
edge at one point exactly when the two edges' dihedrals are equal (a box
corner, any right-angled prism), which the operation requires; a corner
of unequal dihedrals is two arcs and C6's. Two chamfers at such a corner
meet in the line between the same two points; their far contacts meet
the third edge at one point exactly when the two edges make equal angles
with it, which a chamfer corner requires instead. Three blended edges at
a vertex of three planes, every blend convex or every one concave, meet
in a corner, and no corner edge is cut: each face's two contacts cross at
one point, the corner's three points, and each blend ends on the
corner face between the points on its two faces. Three fillets' axes meet
at the ball's one centre, and the corner is the sphere of the radius
about it, tangent to each cylinder along the great circle through the
centre square to its axis — no equal-dihedral condition, as a miter
needs. The sphere's frame has `Z` toward the point of a face square to
the other two, so the side between those two is the equator and the other
two sides meridians, every pcurve on the sphere a line; the meridians
meet at the pole, that point, crossed by a degenerate edge as a
revolve's sphere closes at its axis. A fillet corner with no face square
to the other two would put a side on a tilted great circle with a fitted
pcurve and is C6's. Three chamfers meet in the triangle of the three
points, each side a chord in one chamfer's plane, at any such corner. The edges are blended in the
body's iteration order, whatever order they are listed in, so the result
and its ids are the same for any order of one set; disjoint blends share
nothing but the faces across their ends, where a corner edge between two
of them is cut at both its ends in one edge. A contact line or an end arc that
would leave its face through an edge that is not the corner's own, or a
corner edge shorter than the trim — decided in the face's own (u, v)
through `FaceDomain::side` at `check_samples` interior parameters, a
contact inside its face and an end arc inside the face across when the
blend removes the corner and outside it when a concave blend adds the
corner to that face — is
`Reason::BlendTooLarge` naming the edge and the face or edge the blend
runs out of; a tangent dihedral, or an end at a vertex where a corner
edge's two faces are tangent — the contact every blend face meets its
neighbours along, so a second blend that reaches a first one's end — is
`Reason::TangentChain`; a vertex of
other than three edges, a miter of two fillets with unequal dihedrals,
of two chamfers whose edges make unequal angles with its third edge, or
of blends not both convex or both concave, or a corner of three blended
edges whose faces are not all planes, whose blends are mixed, or — three
fillets — none of whose faces is square to the other two, is
`Reason::VertexBlend` naming the vertex; a surface pair
outside the table, or a face across an end that is not a plane, is
`OpError::Unsupported` naming the kinds and the faces; an empty list,
an edge listed twice and an edge of another body are `Reason::NoEdges`,
`RepeatedEdge` and `EdgeNotInBody`. A blend that meets a third face while
its contacts stay inside their faces is not detected by the operation:
S5 catches it in the corpus. Provenance is rooted at the edge with no new
`Role` (data-model §Provenance).

Sweeps take a planar `geom::Profile` — an outer loop and holes of lines
and arcs in a plane's own (u, v), validated and oriented by
`Profile::edges` (data-model §Profiles) — so a consumer's sketch never has
to become topology before it becomes a solid. A sweep's faces are known
outright, so it enters the builder through `Builder::assemble` as
`transform` does, in one fixed order — vertices per loop in walking order
(the start ring, then the end ring), edges (start, end, rises), faces
(start cap, end cap, sides per loop per segment) — so the ids are a
function of the profile alone; every tolerance is `default_tolerance`,
and every entity is `Generated` from a `Role` naming the part of the
sketch it came from (data-model §Provenance, `SweepPart`).

`ops::revolve(m, &profile, axis: Axis, angle)` sweeps the profile about
an axis lying in its plane within the tolerances
(`Reason::AxisNotInProfilePlane` otherwise): a partial turn with two flat
ends — the profile face, its outward normal against the turn, and its
copy rotated by `angle` — or a full turn with seams when `angle` is within
`angular_tolerance` of `2π` (`Reason::AngleAboveTurn` above it,
`NotPositive` at or below zero). The profile lies wholly on one side of
the axis (`Reason::ProfileCrossesAxis` across it, `ZeroThickness` within
`default_tolerance` of it everywhere), and an arc whose centre is off the
axis and nearer it than its radius is `Reason::SpindleTorus`. It may
touch the axis: a vertex within `default_tolerance` of it is *on* it and
sweeps no rise — one vertex, shared by both flat ends of a partial turn
and not made in a full turn, since no face keeps it there — and a line
segment with both ends on it lies *along* it and sweeps no face: in a
partial turn the one edge both flat ends share, in a full turn nothing,
so a rectangle with a side on the axis turns into a solid cylinder of
three faces. A face closing at a vertex on the axis — a cone's apex, a
sphere's pole — holds a degenerate edge there in place of the rise, its
pcurve the line at the singular `v` over the rise's range, one per face
closing there, and a full turn keeps the vertex for it: the first
degenerate edges an operation makes, which `Builder::assemble` takes used
once and the Euler line leaves out. A full turn touching the axis at a
vertex with no segment along it is `Reason::NonManifold`, since the
surface would touch itself there; a partial turn's flat ends make that
vertex manifold. Every other segment sweeps one face: a segment
parallel to the axis a cylinder, perpendicular a plane (an annulus, or a
sector of one), oblique a cone with its apex on the axis; an arc centred
on the axis a sphere, elsewhere a torus of `R` its centre's distance and
`r` its radius. Every pair of faces a revolve makes shares its axis or
has a plane through it, so S5 and B1 decide them by the meridian arm and
`classify_point` casts against them by the line arms (ADR-0008); their
booleans are C3's, behind the quadric guard (§Operations). The
surfaces of revolution share one frame: origin on the axis, `X` the unit
radial from the axis into the profile's plane — so `u = 0` *is* the
profile plane and every seam lies in it — `Z` the axis direction, except
a cone whose radius shrinks along the axis, which takes `Z = −axis` since
the data model's cone grows along `+Z`. Every vertex sweeps a circular
*rise* about the axis over `[0, angle]`; each side face's loop is start
edge, rise, end edge, rise — the end edge the start edge's second use
across the seam in a full turn, a rise at a vertex on the axis left out,
so a side reaching the axis closes there — walked that way when the material sweeps
along the profile's normal and the other way otherwise, and the face's
use orientation is the surface normal against the segment's outward
in-plane normal at its midpoint (material on the loop's left), uniform
over a face by construction. Every pcurve is exact through `pcurve_on`,
then translated by whole periods into the copy of the domain the loop is
written in (the profile plane at `u = 0`, a seam's second use one period
on), since `pcurve_on` reports a periodic parameter in `[0, 2π)`. A full
turn closes the profile into one lump (ADR-0006) of a shell per *chain* —
a maximal run of a loop's segments off the axis, a loop that never lies
along it being one chain: the chain whose ends span every other's along
the axis is the lump's outer shell, stored first, and every other chain —
a hole, a notch cut in from the axis — a void directly inside it, stored
by loop and lowest segment and `Generated` from `SweepPart::Cavity {
loop_index, segment }`, `segment` the lowest index the consumer wrote
among its segments; a partial turn's flat ends join everything into one
shell.

`ops::extrude(m, &profile, direction: Vec3, length)` sweeps the profile
along its plane's normal, either way: `direction` is the normal or its
opposite within `angular_tolerance` (`Reason::DirectionNotNormal`
otherwise — an oblique extrusion of an arc is a cylinder of elliptical
section, a sweep along a path and cycle 5's), `length` finite and above
`default_tolerance` (`NotPositive` at or below zero, `ZeroThickness`
within the tolerance). The sweep is the plane's exact normal, never the
caller's rounding of it. The profile face keeps its plane's frame
whichever way the sweep goes — a planar face's frame *is* the answer a
consumer reads back — and is the cap whose outward normal opposes the
sweep; the other cap is its copy translated by the sweep. A line segment
sweeps a plane whose `X` is the segment and `Y` the sweep, an arc a
cylinder whose frame is the arc's centre with `Z` the sweep and `X` the
profile plane's, so a circle loop's seam stands at its vertex's rise as
Open CASCADE's does; every vertex sweeps a straight rise. Each side
face's loop is start edge, rise, end edge, rise — a circle loop's one
rise its seam, used twice — walked that way when the sweep runs along the
profile's normal and the other way otherwise, with the face use and the
pcurves decided as for a revolve. Both sweeps share the cap, side-face
and provenance construction. There is no `planar_face` operation — a
sheet of one face is C7's sheet bodies, and the cap construction is the
sweeps' private helper.

### Errors

`OpError` is a `thiserror` enum and every variant names the entities
involved, so the message a consumer shows — or the agent reads — says
*which* face pair, *which* edge, not "boolean failed":

| Variant | When | Carries |
|---|---|---|
| `InvalidInput` | an input body fails the checker (checked in debug builds before the operation starts, and in release when the `paranoid` feature is on) | `Body`, the `Report` |
| `Unsupported` | the exhaustive dispatch reached a surface or curve pair the kernel has no formula for yet — a boolean's face pair, a blend's face pair outside its table or the face across a blend's end | the two `GeomKind`s with their entities |
| `Degenerate` | the requested result has no valid representation: a parameter that makes no geometry (`Reason::NonFinite`, `Reason::NotPositive` naming it — a zero radius, a box whose `min` is not below its `max`, a revolve angle at or below zero, a zero extrude direction; `Reason::AngleAboveTurn` past `2π`), a zero-thickness intersection or an extrude of zero length (`Reason::ZeroThickness`), a revolve whose axis is off the profile's plane (`Reason::AxisNotInProfilePlane`), whose profile crosses its axis (`Reason::ProfileCrossesAxis`) or lies within the tolerance of it everywhere (`Reason::ZeroThickness`), or whose arc's circle crosses it (`Reason::SpindleTorus`); an extrude off its plane's normal (`Reason::DirectionNotNormal`); a boolean that selects no material (`Reason::Empty`: a target inside its tool, a `common` of disjoint operands); result shells that would touch along an edge or at a vertex, or a full revolve touching its axis at a vertex with no segment along it (`Reason::NonManifold`, naming the shared edges or vertices, none for a sweep); faces touching along a curve interior to both result faces (`Reason::TangentContact`); a blend asked for no edges (`Reason::NoEdges`), for an edge twice (`Reason::RepeatedEdge`) or for an edge of another body (`Reason::EdgeNotInBody`), one that leaves its face, outruns a corner edge or a seam, or around a closed edge would need a torus that is not a ring torus or a contact reaching the axis (`Reason::BlendTooLarge`), one at a tangent dihedral or ending on a tangent corner edge (`Reason::TangentChain`), or one at a corner the closed forms do not cover (`Reason::VertexBlend`, ADR-0007); a query on a body that is not a `Solid` (`Reason::NotSolid`) | the entities (none for a primitive or a sweep) and a `Reason` enum |
| `Profile` | a sweep's sketch is not a valid profile: `Profile::edges` refused it (data-model §Profiles). An invalid profile has no entities to name, so it is neither `InvalidInput` nor `Degenerate` | the `ProfileError`, naming the loop and segment |
| `Tolerance` | the result would need an entity tolerance above `Precision::max_tolerance` | the entity, the tolerance it wanted |
| `NotFound` | an id does not resolve in this model (wrong model, or compacted away) | the `AnyId` that failed to resolve itself, never an entity that merely holds it |
| `Internal` | a kernel bug the operation caught: the checker rejected its own output, the builder refused a step of its fixed sequence, a frame could not be placed from inputs it had validated, a point it had to classify could not be, a geometry query failed on validated input for a reason other than a missing closed form, a section edge crossed a seam the seam's own hit should have paved, a piece of a coincident face pair's edge matched no piece of the edge it lies along, the (u, v) arrangement of a face was not the subdivision the pave model promised (`SplitFault`: a dangling section edge, a cycle not turning once, a hole inside no piece, a piece with no interior point, a pave at an edge's end), the shells a boolean kept did not nest into lumps, an operation's own fixed sequence broke an invariant it should have kept — an internal lookup by index or key, never a model id, found nothing (`Fault::Invariant { what }`), a sweep's own later step needed an entity its earlier step did not make for a segment (`Fault::Unmade { segment }`), a surface had no normal at a point on a face an operation needed one at, every partial derivative degenerate where the checker's own tolerances should have ruled that out (`Fault::NoNormal { face }`), or a profile edge's curve was not one of the kinds `Profile::edges` makes (`Fault::ProfileCurve(GeomError)`) | a `Fault` — the `Report`, the `BuildError`, the `FrameError`, the `ClassifyError`, the `GeomError`, the two faces of the seam crossing, the edge and face of the unmatched common block, the `SplitFault` naming the face, the `LumpError`, or one of the four bookkeeping variants above |

`Internal(Fault::Checker)` is returned only in release builds with
`paranoid` on, since a debug build panics on the same report (below);
every other fault is returned as `Internal` in any build. A degenerate *result* that the
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

`arris_check::domain::FaceDomain::of(&model, face, tolerance) ->
Result<FaceDomain, NotFound>` is the one answer to "where is this (u, v)
point on this face": a face's loops read once as polygons within a chord
of their pcurves, its (u, v) and 3D boxes, and `FaceDomain::side(uv) ->
(Side, Vec2)`, which tries every period translate of the surface
(`domain::shifts`) before answering `Outside`, each try answered by the
domain's `region2::SideIndex` in the segments near the point rather than
a walk over tens of thousands of them, so a periodic face's loops
need only be written in one translate and S5, B1, the boolean and
tessellation can never disagree about a point past a period —
`FaceDomain::winds_around` and `FaceDomain::boundary_entity` resolve a
point on the boundary to its vertex, else its edge, the same way, also
trying periods on a closed edge; the free `domain::boundary_entity(model,
edges, point)` does the same over a whole body's edges, which the
classifier below asks of one. It replaced `Checker::face_side` and
`faces_fine`, the classifier's own polygons, `check::uv_bounds`, the
boolean's `FaceInfo`'s domain part and mesh's padded (u, v) box — one
definition, not five (ADR-0004). The checker's `Full` rows and the
classifier build it at the model's parametric tolerance; a boolean builds
it at the pair's own face tolerance — S5's `regions_overlap` and
`curve_is_interior_to_both`, and the classifier's boundary and ray tests,
take the larger of the two faces' own tolerances at a pair, not the
model's, so a face modelled looser than the default is judged by its own
tolerance there too. It lives in `check` because the classifier and B1
are already there, and `ops` and `mesh` already depend on `check`.

`arris_check::classify::Classifier` is B1's ray cast made public and
complete, built once and asked of many points: `Classifier::of_body(model,
body)` reads the body's faces — their `FaceDomain`s — once, and
`.classify(point) -> Result<Classification, ClassifyError>` answers
`Inside`, `Outside`, or `On(Shape)` naming the most specific entity the
point is within the tolerance of — the vertex, else the edge, else the
face. `classify_point(&model, body, point)` is the one-shot wrapper over
a fresh `Classifier`. The boundary test comes first, by the entities' own
tolerances; only a point that is on nothing is cast for, and then the
eight fixed directions are tried in order, a direction abandoned on a
boundary, tangent or coincident hit, with all eight abandoned reported as
`ClassifyError::Undecided` naming the body and the point. A ray meets
every analytic surface by closed form (ADR-0008), so only a NURBS face is
`ClassifyError::Geometry`; a hit at a cone's apex or a sphere's pole lands
on that face's degenerate edge, a boundary like any other, and abandons
its direction. B1 is the same
code over one shell's faces — `Checker::shell_contains` and `nesting`
build one `Classifier` per shell rather than one per ray, and a boolean's
piece selection builds one per operand rather than one per piece it
classifies — so the row that proves a shell nesting and the predicate
that decides which piece of a split face a boolean keeps can never
disagree about a point (ADR-0004). It lives in `check` because that is
where B1 already was, and `ops` depends on `check`; the facade re-exports
it.

`arris_check::flux::face_flux(&model, face, integrand) -> Result<f64,
FluxError>` is the one `∬ integrand(P, ∂P/∂u × ∂P/∂v) du dv` over a
face's region, by Green's theorem through its loops
(`geom::integrate::region_integral` at the surface's own `inner_step`):
B2's enclosed volume, `arris_check::lumps`'s per-shell volume (both
`P/3`'s flux, whose divergence is one) and `ops::measure`'s mass
properties (the same integral with the density's and the moments' fields)
are this integral with a different field each time, so the checker and a
measurement can never read one face's region differently.
`ops::measure::face_area` stays in `ops`: it integrates the surface's own
area element, not the flux of a vector field.

`arris_check::lumps(&model, body) -> Result<Vec<Lump>, LumpError>` is
B1's nesting as a value (ADR-0006): each outer shell of a solid with the
voids whose innermost container it is, in the order the body stores the
outer shells — what the STEP writer writes a solid entity per and the
corpus counts as `solids`. It runs B1's own code over the body and nothing
else, so it is `Ok` exactly where B1 passes and decides, and otherwise
names B1's first fault or its undecided rows; a body of one shell is one
lump and casts no ray. Lumps are derived, never stored on the body.

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
Tangent, Points}` for a surface pair — `Points` where the surfaces meet
only at isolated points, a touch or a crossing through an apex —
`CurveSurfaceIntersection::{Points, Coincident}` for a curve against a
surface and `CurveIntersection::{Points, Coincident}` for two curves, so
a caller matches the case rather than counting curves or points. A pair
may be supported in part: two cylinders meet by closed form when their
axes are parallel (rulings, or `Coincident` or `Empty` when coaxial),
when their axes cross at equal radii (two ellipses), and when skew axes
are further apart than the two radii (`Empty`); crossing axes of unequal
radii and skew axes within the radii are `Unsupported`, since the curve
is a quartic and C3's — which is still a named arm in the pose analysis,
not a wildcard (`docs/DATA-MODEL.md` §Curves has the table). Every pair
with a cone, a sphere or a torus in it is supported where the two share
an axis — a plane perpendicular to it, a cylinder, cone or torus on it, a
sphere centred on it, and every plane–sphere and sphere–sphere pair — by
one arm over the meridian sections in the plane through the axis rather
than a table of pairwise closed forms (ADR-0008): circles about the axis,
points on it, `Coincident` or `Empty` — and where a plane holds a cone's
or a torus's axis, its meridian: two rulings through the apex, or two
tube circles, a partial revolve's flat ends. The general positions — a
plane oblique to a cone's or a torus's axis or parallel to it and off
it, two tori on different axes, a sphere off the axis — are
`Unsupported` and C3's, as is a result that
would mix a circle with a point, whose type is decided with C3's ADR. The
NURBS variant is one arm like the others; a NURBS–NURBS marcher, when it
comes, is what that arm calls, and analytic pairs never route through
it.

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
  surface and not in the parameters (ADR-0003), and a ruled direction is
  then flattened to a thin ribbon — an eighth of a chord step of the
  curved parameter — so the criterion is left to the parameter the chord
  bound is written in (ADR-0005). No triangle of a face on a cylinder
  travels more than one chord step of the turn, whatever the shear of its
  region: the wall of a hole drilled at an angle is a strip oblique to
  the ruling, and in an isometric domain Delaunay would join its boundary
  across the hole rather than column by column.
- A face whose loops are not the simple nested polygons the checker
  promises is `MeshError::Face` with the `CdtError` naming the segments.
- The interior lattice is capped by its total point count, not per
  direction: a face whose curvature varies enormously over its domain — a
  torus with a minor radius far smaller than its major one, at a fine
  chord — that would need more than `MAX_INTERIOR_POINTS` points is
  `MeshError::GridTooLarge { face, points }` naming how many it would
  need, never an allocation the machine cannot make. `MeshError::Internal`
  is tessellation's own bookkeeping breaking on already-validated input —
  never a property of the body or the chord, and never the CDT's own
  fault, so never a `MeshError::Face`.

No adaptive refinement: interior points, where a face needs them, lie on
a uniform (u, v) grid sized by the chord bound. No `f32` output (the
`⚠ OPEN` under §Threading), no per-vertex normals or (u, v), no
mesh-based mass properties (`ops::measure` integrates the B-Rep).

## Threading and wasm

- Every public type is `Send + Sync`. There is no global mutable state, no
  thread-local cache, no interior mutability in the representation crates.
- Operations take `&mut Model`: one operation at a time per model. That is
  the contract, not a limitation to be worked around with locks. Parallelism
  *inside* an operation is `rayon` behind the `parallel` feature and must
  give identical output with the feature off — the tests run both ways.
  Today that is `arris-mesh`'s faces after the sequential edge pass, and
  `arris-ops`'s two read-only passes over a boolean: the intersection of
  every candidate face pair, and the splitting of every face in its own
  (u, v). Each collects its results in the sequential order before
  anything mutable sees them, which is what makes a result byte-identical
  either way. Parallelism *across* operations is clone-evaluate-import,
  above.
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
  representation lists a solid entity per lump of each solid body
  (`arris_check::lumps`, ADR-0006): a `MANIFOLD_SOLID_BREP` over its
  `CLOSED_SHELL`, or for a lump with voids a `BREP_WITH_VOIDS` adding an
  `ORIENTED_CLOSED_SHELL` of orientation false per void, whose
  `CLOSED_SHELL` holds the void's faces turned — the reversed shell the
  reference tree's writer emits and its reader turns back into a hole;
  per face an `ADVANCED_FACE` whose `same_sense` is the shell's use
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
  bodies are `Unsupported` until an operation produces them, and a solid
  whose shells do not nest into lumps is `StepError::Lumps`.
- **Native format** (`arris_io::native::{to_json, from_json, to_bytes,
  from_bytes}`): `serde` of the model under a version header, JSON for
  diffs and `postcard` bytes for storage; data-model §Native format.
- **Text dump** (`arris-debug::dump_text`): the deterministic, diffable
  rendering of a body that fixtures store and tests compare. Not a format:
  it has no reader.
- **The docs-refs lint** (`crates/arris/tests/docs_refs.rs`): a plan file
  is deleted on retirement (`.agents/rules/docs-lifecycle.md`), so a
  citation of `docs/plans/<slug>` or `plans/<slug>` — the commit-message
  form `(plans/<slug> step N)` included — left behind under `crates/`,
  `tools/`, `.githooks/` or the root `Cargo.toml` after that outlives the
  file it points at. Proven the way `corpus_lint.rs` proves its own
  rules: against a scratch tree with a citation deliberately left
  dangling next to one that still resolves, not just by running clean
  against the real tree.
- **`tools/test-timings.sh`**: the suite's wall clock, per test binary and
  whole, at whatever `ARRIS_PROPTEST_CASES` is set to. Sharding the boolean
  properties and moving the suite onto `cargo nextest` took
  `cargo nextest run --workspace` from 445.68 s to 70.92 s at 256 cases and
  from 1656.26 s to 254.13 s at 1000, measured with this script on a
  16-core machine — 6.3×, within a tenth of the floor that machine's total
  work over its cores allows. Not a benchmark harness: it times binaries,
  not operations.
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
  (`fixtures`; a solid fixture the runner compares under `primitive/`,
  `transform/`, `boolean/`, `sweep/`, `provenance/` or `blend/` without its
  committed dump per variant fails the lint, so an ignored fixture there does; a
  failure waiting for its fix sits under `regression/`, and fails the lint
  once it has a dump), the corpus runner (`corpus::run`, the fixture test of
  roadmap §Fixtures — a `profile` step built into a `geom::Profile` kept
  beside the bodies for the sweep steps that name it, no body and no
  accounting of its own; a `fillet` or `chamfer` step's edges named by a point each,
  the edge `classify_point` answers `On(Edge)` for when no second edge of
  the body passes within the fixture's `probe` of the point, a
  `CorpusError::EdgePoint` otherwise; checker, counts — a solid per lump — and genus,
  the oracle's reading
  of the STEP, the mass properties against the oracle's within the
  fixture's tolerances, the mesh closed and within `mesh_volume_rel`,
  every probe classified as the oracle classifies it
  (`classify_point`, exactly: both sides have their own tolerance for
  "on" and a probe is placed so the two agree, so a disagreement is a
  finding and never something a band is widened to cover), provenance
  accounting, the dump; `ARRIS_BLESS=1` writing `dump.txt` (`dump.<variant>.txt` for another
  variant); a result the
  oracle recorded no solid for must fail with `OpError::Degenerate`, and
  one the recipe marks `analytic.expect_error` with that typed refusal,
  the run ending there with the oracle's numbers kept as the record of
  what Open CASCADE builds; one whose recipe states a convention Arris
  does not follow, `analytic.counts_differ`, held to the recipe's own
  counts with the oracle's kept as the record)
  over the
  oracle seam (`oracle::compare`: STEP under `target/inspect/`, which the runner
  names `<area>-<slug>-<variant>`, plus a digest of the directory for a
  scratch copy of a fixture, so two runs of a recipe never write one file;
  then `compare.py` through `uv`, a missing environment a loud error;
  `oracle::scratch_fixture`: a test's own recipe written under
  `target/inspect/<name>/` with its `expected.json` from `expected.py`,
  for a body the corpus does not hold — the revolves closing at a cone's
  apex or a sphere's pole, and the STEP tests' own bodies — held to the
  oracle's reading of its STEP all the same), and
  the seeded property-test runner and strategies (`prop`,
  with every analytic surface and curve in a random pose and random
  clamped NURBS curves and surfaces under `prop::geom`; sketches under
  `prop::profile` — `star`, a polygon with arcs and holes in either
  orientation; `rectilinear`, a staircase of segments parallel and
  perpendicular to an axis beside it or, half the time, reaching it —
  sides along the axis and notches cut in from it — and `general`, a
  convex polygon beside an axis or, one time in three, with a side along
  it, the chords beside that side closing at a cone's apex or a sphere's
  pole, whose segments sweep cones both ways, spheres and tori,
  the last two each given as a `Sweep` with the axis, a revolve angle and an extrude
  length; and `prop::sweep`, Pappus's
  theorems as the oracle a sweep's volume and area are held to, taken
  in the profile's plane by `region_integral` and a quadrature over its
  boundary, an independent path from `measure`'s flux). `prop` runs a
  property whole through `check`, or split across `k` shards through
  `prop_shards!`, which writes one `#[test]` per shard over a body given
  once so libtest's pool runs them at once instead of one property holding
  one thread. A shard draws from `shard_seed(base, i) = sha256(base ‖ i)`,
  which does not take `k`: raising a property's shard count shortens every
  existing shard's stream to a prefix of what it was rather than re-rolling
  the corpus, and `cases_per_shard` rounds up, so `k` shards run at least
  the configured cases and never fewer. A failure names its shard and
  prints the base seed and total that reproduce the whole run. It is a
  dev-dependency of the workspace's crates and never of a consumer. For
  the crates below it (`math`, `geom`, `topo`) that dev-dependency is a
  cycle, so their property tests are integration tests under
  `crates/<crate>/tests/`, where the crate is linked once and its types
  unify; a `#[cfg(test)]` module would see two copies), and `testing`
  (dev-only, not built for wasm32 since it takes `prop`'s
  `TestCaseError`: the helpers that used to be copied across test files —
  a relative-tolerance and period-aware numeric comparison (`close`,
  `close_param`), the provenance accounting a generated body's record
  owes (`entities_of`, `recorded_parts`, data-model §Provenance), and
  central differences against a curve's or surface's own analytic
  derivatives (`central_differences_curve`, `central_differences_surface`)
  — now imported once by `ops`, `geom` and `mesh`'s tests instead of held
  per file).

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
| Fillet / chamfer of named edges, one call for all edges | `ops::fillet`, `ops::chamfer` (ADR-0007) |
| Tessellation into a render mesh with per-face and per-edge ranges | `arris_mesh::tessellate` → `TriMesh` with `FaceRange`/`EdgeRange` keyed by `FaceId`/`EdgeId` |
| A planar face's frame | `model.surface(model.face(id)?.surface())` is `Surface::Plane { frame }`; the frame *is* the answer, and it is stable across re-evaluation because a primitive's frame, or a sweep's profile plane, is |
| Mass properties (volume, area, centroid, inertia) | `ops::measure::mass_properties` → `MassProperties` (exact over the B-Rep, the tensor about the centroid); or the consumer's own integrator over `TriMesh` |
| STEP export of several bodies | `io::step::write(&model, &[bodies])` |
| Projecting an edge or vertex onto a sketch plane | `geom::project_to_plane` on the edge's `Curve` — a line stays a line, a circle becomes a circle or an ellipse, an ellipse stays an ellipse, a NURBS a `Curve2::Nurbs`; a point-set projection with the variant's own parameter (02 §Pcurves) |
| Persistent topological names (origin-based) | Emitted by the consumer from `Provenance`: an output face is named after the input face it was `Modified` from, `Split(k)` when one input yields several outputs, and after the tool face when `Generated`; edges and vertices derive from their faces exactly as today. No centroid matching. `⚠ OPEN:` whether Arris ships the origin-name grammar as a helper (`arris-topo::naming`) or leaves it to the consumer; the seed lists this among the kickoff questions. Decided in cycle 2 with the consumer's adapter |
| Memoising shapes by content, dropping unreferenced ones | Memoisation stays in the consumer (it is about features, not geometry); dropping is `Model::retain` (§The model, `⚠ OPEN`) |
| Units | Arris is unit-agnostic. The consumer sets `Precision` for its unit (metres: `default_tolerance` at the micrometre scale) when it creates the `Model` |

What the facade has today that Arris will not have: a tolerance nudge
(there is none; a degenerate boolean is `OpError::Degenerate`), and a
"which surface came from which" matcher (provenance replaces it).

## Open questions

Collected from this document; each closes with an ADR.

- `⚠ OPEN:` compaction renumbers slots or keeps them sparse (§The model).
- `⚠ OPEN:` `f32` positions at the tessellation boundary (§Threading).
- `⚠ OPEN:` origin-name helper in Arris or in the consumer (§How a consumer's kernel
  facade maps on).
