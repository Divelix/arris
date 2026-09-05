# 01 — Architecture

Arris is a library. It owns a *model* — an arena of geometry and topology —
and a set of *operations* that append to it and return handles. Everything a
consumer does is a call of the shape `op(&mut Model, inputs…) ->
Result<(Body, Provenance), OpError>`, followed by queries on the handle it
got back. There is no session object, no builder with hidden state, no
global. This document holds the crate layout, the arena and handle model,
the operation and error contract, where the checker runs, the threading and
wasm rules, and how a consumer's kernel facade maps onto the API. The
entities and geometry themselves are in [02-data-model](02-data-model.md).

## Crates and the layer rule

One Cargo workspace, `crates/arris-*`, plus the facade crate `arris` that
re-exports the public API. Lower crates never name types from upper ones.

| Crate | Owns | External deps | Layer |
|---|---|---|---|
| `arris-math` | `Point3`/`Vec3`/`UnitVec3` (over `nalgebra`, ADR-0001), `Frame`, `Frame2`, `Isometry`, `Interval`, exact orientation predicates (over `robust`), polynomial and interval-guarded Newton root finding, `Precision` and `Tolerance` | `nalgebra`, `robust`, `serde` (feature) | 0 — representation |
| `arris-geom` | `Surface`, `Curve`, `Curve2` (analytic + NURBS): evaluation, derivatives, point projection, curve/surface and surface/surface intersection; `GeomError` | `arris-math`, `thiserror` | 0 — representation |
| `arris-topo` | `Model` (the arena), typed ids, `Shape`/`Body`/`Face`/… handles, orientation, entities, pcurves, per-entity tolerances, Euler operators, adjacency and iteration, `Provenance` | `arris-geom`, `arris-math`, `serde` (feature) | 0 — representation |
| `arris-check` | The invariant checker: `check(&Model, Body, Level) -> Report` and the `Violation` list of 02-data-model §Invariants | `arris-topo` | 1 |
| `arris-ops` | Primitives, planar profiles, extrude, revolve, transform, booleans, later blends; `measure` (mass properties); each returns `Provenance` | `arris-check`, `rayon` (feature) | 2 — algorithms |
| `arris-mesh` | `TriMesh`, `Polyline`, `Aabb`, tessellation of faces and edges with shared edge discretisation | `arris-check`, `arris-topo`, `thiserror` | 2 — algorithms |
| `arris-io` | STEP AP214 Part 21 writer (later reader), the native `.arris` format | `arris-check`, `serde` (feature) | 2 — algorithms |
| `arris-debug` | Text dump, PNG render (own software rasteriser over `image`), Rerun stream (feature), the fixture loader and corpus lint, the seeded property-test runner and strategies | `arris-ops`, `arris-mesh`, `arris-io`, `arris-topo`, `arris-geom`, `arris-math`, `image`, `serde`, `serde_json`, `sha2`, `thiserror`, `proptest` (not on `wasm32`), `rerun` (feature) | 3 — dev-facing |
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
`serde` (on by default in `topo` and `io`; off by default in `math`, where
`topo`'s feature turns it on because `Precision` is part of the native
format), `parallel` (`rayon` inside `ops` and `mesh`; never enabled on
`wasm32`), `paranoid` (`ops`: run the checker after every operation in
release builds too, §The checker), `rerun` (`debug` only). The facade
forwards `serde`, `parallel` and `paranoid`.

## The model, the arena and handles

`Model` is the arena. It holds every vertex, edge, face, shell, body,
curve, surface and pcurve ever created in it, each behind a typed
generational id (`VertexId`, `EdgeId`, `FaceId`, `ShellId`, `BodyId`,
`CurveId`, `SurfaceId`, `Curve2Id`): a `u32` slot index and a `u32`
generation. Ids are allocated sequentially in creation order and never
reused until a compaction (below), so in the common case an id is also a
creation timestamp, and iteration in id order is deterministic on every
platform.

**Entities are immutable.** An operation never edits an entity in place; it
appends new ones and returns a handle to a new body that references the
untouched old entities by id. Two bodies that share a face share its id,
its geometry and its tolerance — structural sharing is the default, not an
optimisation. This is what makes provenance cheap (an untouched entity keeps
its id; a modified one has a new id and a record), undo free (an older body
handle is still valid), and background evaluation safe (a clone of the model
sees the same entities).

**The arena is chunked and the chunks are shared.** Storage is a list of
fixed-size chunks behind `Arc`; `Model::clone` copies the list of `Arc`s,
and the first append after a clone copies only the tail chunk. Cloning a
model for a background evaluation is therefore O(number of chunks), not
O(entities), and two clones that diverge share every chunk they both leave
untouched.

**A handle is an id plus an orientation.** `Shape { id: EntityId,
orientation: Orientation }` is the uniform handle used by provenance,
iteration and errors; `Body`, `Shell`, `Face`, `Edge`, `Vertex` are typed
newtypes over the same pair and convert to `Shape` for free. Copying a
handle is copying two integers. Orientation composes down the hierarchy
(02-data-model §Orientation); a handle never carries geometry.

**Failed operations leave the model as it was.** Because the arena is
append-only, an operation runs inside a transaction that records the arena's
length at entry and truncates to it on `Err`. A consumer never sees
half-built entities.

**Bodies move between models by import.** `Model::import(&mut self, &other,
body) -> (Body, IdMap)` deep-copies a body's closure and returns the id map.
This is how independent evaluations on clones are merged, how a consumer
holds several documents, and how provenance across models is translated.

**Compaction.** `Model::retain(&mut self, keep: &[Body])` drops every entity
not reachable from `keep` and bumps the generation of freed slots, so stale
handles fail to resolve instead of aliasing. It is the only operation that
invalidates handles and it is never called by the kernel itself.
`⚠ OPEN:` whether compaction also renumbers slots (denser arena, cheaper
serialisation, but every surviving id changes and the returned `IdMap`
becomes mandatory for the consumer) or keeps slots sparse. Decided with the
first consumer that outlives one evaluation, in cycle 2.

## Operations

Every operation in `arris-ops` has the same shape:

```rust
pub fn cut(m: &mut Model, target: Body, tool: Body) -> Result<(Body, Provenance), OpError>;
```

- Inputs are handles into `m`. The operation reads them, appends, and
  returns a new handle plus the provenance record (02-data-model
  §Provenance) that says which output entity came from which input entity
  and how. An operation without a provenance record is unfinished.
- The operation never mutates its inputs, never panics on geometry, never
  contains a numeric literal that stands for a tolerance, and never iterates
  a hash map where the order can reach a geometric decision or an id.
- Parameters that are geometry (`Axis`, `Frame`, `Profile`) are plain
  values from `arris-math`/`arris-geom`, not handles: a primitive is built
  from numbers, and only the result lives in the model.
- Same input, same output, same ids, on every platform. The tests assert
  this by dumping twice and diffing.

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
| `Unsupported` | the exhaustive dispatch reached a surface or curve pair the kernel has no formula for yet | the two `SurfaceKind`s or `CurveKind`s, the two entities |
| `Degenerate` | the requested result has no valid representation: zero-thickness intersection, a profile crossing its revolve axis, a sweep of zero length | the entities and a `Reason` enum |
| `Tolerance` | the result would need an entity tolerance above `Precision::max_tolerance` | the entity, the tolerance it wanted |
| `NotFound` | a handle does not resolve in this model (wrong model, or compacted away) | the `Shape` |
| `Internal` | the checker rejected the operation's own output — a kernel bug | the `Report` |

`Internal` is returned only in release builds with `paranoid` on; in debug
builds the same condition panics (below). A degenerate *result* that the
consumer might reasonably want anyway (the flush intersection that is a
face, not a solid) is `Degenerate` with a reason, never a silently empty
body: the kernel does not decide what fail-soft means.

## The checker

`arris-check` is its own crate so that no algorithm crate can skip it by
accident and so that its dependency list stays at exactly `arris-topo`.
`check(&model, body, Level) -> Report` returns every violation with the
entity that violates it; `Report::is_ok()` is what every test asserts and
what every operation asserts on its own output. The invariants are listed
in 02-data-model §Invariants; each has a `Violation` variant, a test that
constructs it and sees it reported, and a level:

- `Level::Fast` — combinatorial and local geometric checks (ids resolve,
  loops close, orientations compose, tolerances are ordered, pcurves match
  their 3D curves at sample points). Linear in the body. This is what runs
  after every operation.
- `Level::Full` — adds the global checks: faces of a shell intersect only
  at shared edges, shells nest, a solid encloses positive volume. Not
  linear. Runs on demand, in the fixture corpus and in `/close-cycle`.

**In debug builds every operation runs `Level::Fast` on its output before
returning `Ok`, and panics with the report if it fails.** A checker failure
after an operation is a kernel bug, and the kernel's own invariants are the
one place a panic is allowed (`.agents/rules/kernel.md`). A test that needs
to build an invalid body — every checker test does — constructs it through
`arris-topo`'s raw insert, which the checker does not guard, and says so by
name.

**In release builds nothing runs unless asked.** `model.check(body)` is
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
Tangent}` for a surface pair and `CurveSurfaceIntersection::{Points,
Coincident}` for a curve against a surface, so a caller matches the case
rather than counting curves or points. The NURBS variant is one arm like the
others; a NURBS–NURBS marcher, when it comes, is what that arm calls, and
analytic pairs never route through it.

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

- **STEP AP214** (`arris-io::step`): the writer is a cycle-1 deliverable
  because the oracle reads Arris's output through it. The entity subset is
  the B-Rep one — `MANIFOLD_SOLID_BREP`, `ADVANCED_FACE`, the analytic
  surfaces and curves, `B_SPLINE_*` for NURBS, with pcurves written as
  `PCURVE` so a reader does not have to recompute them.
- **Native format** (`arris-io::native`): `serde` of the model, 02-data-model
  §Native format.
- **Text dump** (`arris-debug::dump_text`): the deterministic, diffable
  rendering of a body that fixtures store and tests compare. Not a format:
  it has no reader.
- **The oracle** (`tools/oracle/`, Python 3.12, Open CASCADE through the
  `cadquery-ocp` wheels in a `uv` environment): `expected.py` builds each
  fixture's recipe in OCCT and writes `expected.json`; `compare.py` reads
  an Arris STEP file and compares it against that within the fixture's
  tolerances; `selftest.py` proves the oracle against closed forms and its
  own STEP. It is run, never linked; no crate depends on it. The fixture
  format is `tests/fixtures/README.md`; its role is 03-roadmap §Fixtures.
- **`arris-debug`** is dev-facing: the rasteriser (`render_png`) and the
  samplers that feed it a curve or a surface without a body (`polyline_of`,
  `wireframe_of`), the Rerun stream, the fixture loader and corpus lint
  (`fixtures`), and the seeded property-test runner and strategies (`prop`,
  with every analytic surface and curve in a random pose under
  `prop::geom`). It is a
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
| Mass properties (volume, centroid, inertia) | `ops::measure::mass_properties` (exact over the B-Rep); or the consumer's own integrator over `TriMesh` |
| STEP export of several bodies | `io::step::write(&model, &[bodies])` |
| Projecting an edge or vertex onto a sketch plane | `geom::project_to_plane` on the edge's `Curve` — a line stays a line, a circle becomes a circle or an ellipse, everything else a `Curve2::Nurbs` |
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
