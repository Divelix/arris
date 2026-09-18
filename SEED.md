# Project Seed Document: Arris

## 1. Project Overview
**Name:** Arris
**Tagline:** An open-source B-Rep geometric kernel in pure Rust, built to be trusted.
**Core Concept:** `arris` is a library, not an application. It owns the boundary representation of solids, sheets and wires; the analytic and free-form geometry underneath them; and the operations that create, combine and interrogate them — primitives, sweeps, booleans, blends, tessellation, mass properties, STEP. Any CAD, CAM, simulation or code-first modelling tool can be built on top of it.

Its first consumer is a parametric CAD application that today runs on the truck/monstertruck kernel behind a kernel facade. Arris replaces that backend once it passes that application's probe corpus (§6). The north star is further out: a kernel that competes with Parasolid-grade proprietary kernels on correctness and breadth, free and open source.

Development is fully agentic. The human sets direction and judges results; the agent designs, implements, tests and documents. That constraint shapes the project more than any technical choice (§5, §10): everything the kernel does must be checkable without a human looking at a screen.

## 2. The Naming Rationale
An **arris** is the sharp edge formed where two surfaces meet — in stonework, joinery and architecture. Computing that edge, and everything that follows from it (which side is inside, which pieces survive, what the result is called), is what a B-Rep kernel exists to do. The name is short, pronounceable, and a real word in the domain.

`arris` 0.0.1 is published on crates.io as a placeholder (2026-09-05).

## 3. The Problem Statement
There is no maintained, pure-Rust B-Rep kernel with working booleans. Every attempt to build a Rust CAD has hit this wall:

* **A truck-based parametric CAD hit it in August 2026.** On monstertruck 0.3.3 a round hole cannot be cut into an existing solid: box − cylinder returns empty or panics, cylinder − cylinder fails, flush faces fail. Shafts, bores, bearing seats and motor mounts — the vocabulary of every mechanical part — are cylinders meeting other solids. That application declared it a *product constraint* and routed around it.
* **The cause is representational, not algorithmic.** truck and its forks have NURBS-only surfaces (a cylinder is a revolved line, with a seam the surface-intersection marcher cannot cross), a single global tolerance of 1e-6, no 2D parametric curves on edges, and an `Arc`-per-entity topology graph. A boolean engine on that foundation inherits all four limits; the earlier "write a boolean crate on truck's topology" plan had to invent side tables to work around them.
* **Fornjot (2021–2026) started from zero and never reached booleans.** Its author's own diagnosis, in the 2024-10-30 experiment notes: the architecture reached "a local maximum … consistent and self-reinforcing", and the project spent its last two years in five ground-up rewrites of the core. It was shut down 2026-06-19 with "its goals were never reached".
* **Open CASCADE works but is not an option here.** It is C++, LGPL, binds into Rust only through opencascade-rs (killing the pure-Rust and WASM story), and its algorithms give no usable answer to "which output face came from which input face" — the topological-naming problem every parametric CAD then has to solve heuristically on top.
* **Mesh CSG is not a kernel.** manifold, csgrs and friends have no faces, no analytic surfaces, no persistent naming, no STEP fidelity.

So the problem is not "write booleans". It is: design a representation on which booleans, blends and naming are *natural*, and prove every operation on it mechanically, so a single developer plus an agent can keep it correct as it grows.

## 4. Competitive Landscape (as of September 2026)

| Kernel | What it is | Why it does not close the gap |
|---|---|---|
| Parasolid, ACIS, C3D | Industrial B-Rep kernels: robust booleans, blend networks, non-manifold bodies, decades of test corpora | Proprietary, per-seat licensed. The bar, not a competitor |
| [Open CASCADE](https://dev.opencascade.org/) 7.x | The only open kernel of industrial breadth: General Fuse booleans, per-shape tolerances, healing, STEP/IGES | C++/LGPL; heavy; no Rust story without FFI; weak provenance; known long-standing boolean failure classes |
| [truck](https://github.com/ricosjp/truck) / [monstertruck](https://github.com/virtualritz/monstertruck) | Pure-Rust NURBS B-Rep: topology, modelling, meshing, STEP, single-edge fillets (fork) | NURBS-only surfaces, global tolerance, no pcurves; booleans fail on curved and tangent operands; both effectively one-author, fork ships breaking minors monthly |
| [Fornjot](https://github.com/hannobraun/fornjot) | Pure-Rust B-Rep, validation-first, code-CAD API | Shut down 2026-06-19 without booleans; core rewritten repeatedly |
| CADmium | Rust + web CAD on truck | Archived 2025 |
| opencascade-rs | Rust bindings to OCCT | Not pure Rust; no WASM; inherits OCCT's limits |
| [SolveSpace](https://solvespace.com/) | Own small NURBS kernel with booleans (C++, GPL) | Application-bound, GPL, no analytic surface types, known boolean fragility |
| manifold, csgrs, CGAL Nef | Mesh / polyhedral CSG | No curved faces, no naming, no B-Rep |
| libfive, OpenSCAD | Implicit / CSG-tree modelling | Not B-Rep; no faces to name or export |

**Why Arris anyway — the differentiators, in priority order:**
1. **A representation designed for booleans from day one.** Analytic surfaces and curves as first-class types with explicit seams and periods; per-vertex/edge/face tolerances that grow and are checked; a 2D parametric curve for every edge on every face. These three are what truck lacks and what OCCT's boolean machinery quietly depends on.
2. **Provenance is a core contract, not an afterthought.** Every operation reports which output entities were *generated from*, *modified from* or *deleted from* which inputs. Inside the algorithm this is cheap; recovered afterwards by centroid matching (what a CAD on truck has to do today) it is the weakest link in any parametric history. No open kernel exposes this well.
3. **Immutable shapes over an arena.** Operations never mutate inputs; they append entities and return a new shape that shares the untouched ones. Undo, background evaluation, parallel evaluation and serialization fall out of this for free — the properties a parametric CAD needs from its kernel.
4. **Verification-first, because the developer is an agent.** An invariant checker runs after every operation in debug builds; every fixture has a differential oracle computed by Open CASCADE; algebraic property tests generate cases nobody thought of. A bug is found one operation after it is introduced, as a number, not a picture.
5. **Pure Rust, WASM-capable, MIT/Apache.** True but thin on its own — the floor, not the pitch.

**Explicit non-goals:** a sketch constraint solver (it belongs to the application), rendering or UI, physics, drawings, mesh processing beyond tessellation (decimation, remeshing, convex decomposition), file formats other than STEP and the native format (IGES may follow), and speed as a headline. Performance is a design constraint only in the sense that nothing in the kernel may preclude it: no global mutable state, `Send + Sync` everywhere, data laid out for parallel evaluation.

## 5. The Solution & Developer Experience
Arris is meant to feel like a well-typed Rust library, not a port of a C++ API:

* **A `Model` owns everything; shapes are cheap handles.** Vertices, edges, faces, shells, bodies and their geometry live in one arena with typed, generational ids. A `Shape` is an id plus an orientation. Cloning a shape is copying two integers.
* **Operations are functions from shapes to `Result<(Shape, Provenance), Error>`.** No builders with hidden state, no "is done" flags. An operation either returns a valid shape or a typed, actionable error naming the entities involved. The kernel never panics on geometry; it is the consumer's job to decide what fail-soft means.
* **Validity is checkable, always.** `model.check(shape)` runs the full invariant suite and returns every violation with the entity that violates it. Debug builds run it after every operation; release builds run it on demand.
* **Geometry is an open enum, not a trait object.** `Surface::{Plane, Cylinder, Cone, Sphere, Torus, Nurbs, …}` and likewise for curves. Intersection dispatch is an exhaustive match over pairs, so the compiler reports what is unimplemented and there is never a silent generic fallback where an exact formula exists.
* **Everything is inspectable without a GUI.** A shape, an intersection curve, a failed split — each dumps to a PNG the agent can read as an image, to a Rerun stream the human can orbit, and to a text form that diffs in a test. A CAD application becomes the human-facing debugger only once Arris sits behind its facade.

```rust
let mut m = Model::new();
let block = ops::primitive_box(&mut m, [0.0, 0.0, 0.0], [40.0, 30.0, 10.0])?;
let bore  = ops::primitive_cylinder(&mut m, Axis::z_at([20.0, 15.0, -1.0]), 4.0, 12.0)?;
let (part, prov) = ops::cut(&mut m, block, bore)?;
assert!(m.check(part).is_ok());
let hole_wall = prov.generated_from(bore.faces()[0]);  // the new cylindrical face
let mesh = tessellate(&m, part, Tolerance::linear(0.01))?;
let step = io::step::write(&m, &[part])?;
```

## 6. Core Features

### Cycle 1 — the vertical slice
One path through every crate before any crate is broad. Acceptance: **box − cylinder blind hole, then an 8-hole bolt pattern by repeated cut, then a flush box ∪ box union** — checker green, volumes and face counts matching Open CASCADE, provenance naming every hole wall stably across parameter changes.

1. **Math:** points/vectors/frames over `nalgebra`, intervals, exact orientation predicates, polynomial and Newton root finding, tolerance types.
2. **Geometry:** analytic surfaces (plane, cylinder, cone, sphere, torus) and curves (line, circle, ellipse) plus NURBS; evaluation, derivatives, point projection; curve/surface and analytic surface/surface intersection for the plane–cylinder–plane triangle.
3. **Topology:** the arena; vertex/edge/wire/face/shell/body with orientation; pcurves; per-entity tolerances; Euler operators; iteration and adjacency; the invariant checker.
4. **Operations:** box and cylinder primitives; extrude and revolve of planar profiles; booleans in the General-Fuse decomposition (intersect → split → classify → assemble) for plane/cylinder operands; transform.
5. **Provenance:** the generated/modified/deleted record emitted by every operation above.
6. **Tessellation:** faces to triangles with shared, consistent edge discretisation; edges to polylines.
7. **STEP AP214 writer** — needed in cycle 1 because the oracle reads Arris output through Open CASCADE.
8. **Debug and test harness:** PNG/Rerun/text dumps, the OCCT oracle environment, property tests, fixture corpus with shrinking.

### Cycle 2 — the application gate
Arris replaces monstertruck behind the first consumer's kernel facade when it covers everything that facade uses today **and** passes that application's probe corpus (the recorded truck/monstertruck failures, each with an `#[ignore]`d twin asserting the wanted result): primitives, extrude, revolve, booleans on planar and cylindrical operands including tangent/flush faces, transform, single-edge fillet and chamfer, tessellation, face frames, mass properties, STEP export, topological-reference projection, shape retention. Until then the two projects develop in parallel; the application is never blocked on Arris.

*Amended by ADR-0017 (2026-09-18): the cycle closes on the probe corpus as Arris fixtures; the swap itself is the consumer's, on its own schedule.*

### Toward Parasolid grade — later cycles, roughly in order
Cone/sphere/torus intersection pairs · coincident and tangent face handling with tolerance growth · NURBS–NURBS surface intersection · sweep, loft, shell, offset · fillet networks, vertex blends, variable radius · STEP reader and healing · sheet and wire bodies in every operation · IGES. Each earns its own cycle with an acceptance corpus; none is scheduled here.

## 7. Tech Stack (decided)

| Layer | Choice | Why / notes |
|---|---|---|
| Language | Rust, edition 2024, MIT OR Apache-2.0 | Pure Rust, WASM-capable; no C/C++ dependencies anywhere in the kernel |
| Linear algebra | `nalgebra` (f64) | Distinguishes points, vectors and unit vectors; ships the small dense solvers intersection code needs. Consumers convert to `glam` at the boundary |
| Exact predicates | `robust` | Shewchuk-style adaptive orientation/in-circle for 2D classification and triangulation |
| Errors | `thiserror` | Typed errors naming entities; no panics on geometry |
| Serialization | `serde` | Native model format; arena ids serialize as integers |
| Parallelism | `rayon` behind a feature flag, off on wasm | Nothing in the design may require threads |
| Property tests | `proptest` | Random operands in random poses; algebraic identities; shrinking to fixtures |
| Oracle | Open CASCADE via Python (`cadquery-ocp` wheels in a `uv` venv under `tools/`) | Volumes, areas, centroids, counts, point classification for every fixture; reads Arris STEP. Legally clean: OCCT is *run*, never copied |
| Visual debug | Own software rasterizer to PNG (`image`), `rerun` SDK as dev-dependency | The agent reads PNGs; the human orbits in Rerun |
| STEP | Own Part 21 writer (and later reader) | No adequate crate; the schema subset is small and well specified |
| Reference trees (read-only) | `~/Documents/code/rust/{truck,monstertruck,fornjot}`, `~/Documents/code/pet/cad/OCCT` | Read for algorithms and mistakes; nothing is copied or transliterated (§9) |

### Workspace layout
```
arris-math      points, vectors, frames, intervals, predicates, root finding, tolerances
arris-geom      curves and surfaces (analytic + NURBS), evaluation, projection, intersection
arris-topo      arena, entities, orientation, pcurves, tolerances, Euler ops, iteration
arris-check     the invariant checker (its own crate so it can never be skipped by accident)
arris-ops       primitives, sweeps, booleans, transforms, later blends — each returns Provenance
arris-mesh      tessellation and the mesh type
arris-io        STEP and the native format
arris-debug     PNG/Rerun/text dumps, fixture and oracle helpers (dev-facing)
arris           facade: re-exports the public API
```
**Layer rule:** lower crates never name types from upper ones. `arris-math`, `arris-geom` and `arris-topo` are the representation and change rarely; everything above them is an algorithm crate that can be rewritten without touching its neighbours. This is the answer to "ship of Theseus or monolith": **monolithic in representation, modular in algorithms.** truck is modular in the wrong place; Fornjot rebuilt the representation five times.

## 8. Heritage and References
**From the truck-based CAD attempt that preceded Arris:**
* A kernel facade trait a parametric CAD already programs against — the concrete API surface cycle 2 must cover, and what makes the swap a one-crate change.
* A probe corpus of truck/monstertruck failures (curved and tangent booleans, revolves) — the first fixtures, and the flip criterion for the swap.
* A decomposition of "own kernel" into rows with difficulty and necessity, a tolerance-model analysis and kill criteria — this seed is that analysis's phase 0, taken further.
* Origin-based topological naming and a kernel-free mass-properties integrator — the naming and inertia designs Arris must serve.
* The docs discipline: charter, living design docs, append-only ADRs, ideas/plans tiers, drift review at every milestone.

**From truck / monstertruck:** NURBS evaluation and knot algorithms, the tessellation approach, the STEP writer's structure — all as *reference for what a working implementation looks like*, plus a catalogue of what to avoid: revolved-line cylinders, global tolerance, no pcurves, per-face-use edge duplication.

**From Fornjot:** validation as a layer that observes every inserted object (adopted as `arris-check` run after every operation); loosely coupled topology/geometry/validation layers communicating by events (adopted in spirit: the arena separates topology from geometry); and the cautionary record of a representation redesigned until the project ended.

**From Open CASCADE (read-only):** the General Fuse decomposition (`BOPAlgo_PaveFiller` → `Builder` → `BOP`), the pave/pave-block model of shared edges, `IntAna` quadric-pair case analysis, `BRepCheck`'s invariant list, the per-shape tolerance model, `BRepTools_History` as the shape of a provenance record. Arris reads these for the ideas and reimplements them to fit its own representation; every design decision informed by an OCCT module names that module in its ADR so provenance is auditable.

**From the public Parasolid documentation:** the body model (solid, sheet, wire, general bodies) and the vocabulary of a complete kernel API — the checklist for "Parasolid grade".

**Literature:** Patrikalakis & Maekawa, *Shape Interrogation for Computer Aided Design and Manufacturing* (intersections); Hoffmann, *Geometric and Solid Modeling* (robustness, classification); Mäntylä, *An Introduction to Solid Modeling* (Euler operators); Shewchuk, *Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric Predicates*; Piegl & Tiller, *The NURBS Book*.

## 9. Decisions taken in this seed

| Question | Decision | Reason |
|---|---|---|
| Ship of Theseus on truck, or from scratch? | **From scratch, monolithic in representation, modular in algorithms** | truck's limits are in its data model; keeping it keeps them. Fornjot shows the cost of not fixing the representation once |
| Analytic surfaces? | **First-class enum variants with explicit seam/period; NURBS is one variant** | Exact intersections for the pairs that cover ~95 % of mechanical geometry; exhaustive dispatch |
| Tolerance model | **Per-vertex/edge/face tolerances from day one, vertex ≥ edge ≥ face, only growing, enforced by the checker** | A global epsilon is the shortcut that made truck brittle; retrofitting tolerances into a boolean is the classic mistake |
| Pcurves | **Every edge carries a 2D curve on every adjacent face; exact for analytic pairs, fitted for NURBS; checked against the 3D curve** | Face splitting, classification and tessellation become 2D problems |
| Topology storage | **Arena with typed generational ids; `Shape` = id + orientation** | Fast, serializable, trivial structural sharing, natural provenance |
| Mutability | **Shapes are immutable values; operations append and return new shapes sharing untouched entities** | Undo, background and parallel evaluation for free; matches a parametric document model |
| Provenance | **Every operation returns a generated/modified/deleted record; part of the API contract** | Cheap inside, impossible outside; the biggest gap in every open kernel |
| Numerics | **f64 with the tolerance model; exact predicates for combinatorial decisions; interval-guarded root finding** | Exact rational arithmetic cannot coexist with NURBS; predicates make classification decisions consistent |
| Body model | **Non-manifold from the start (solid, sheet, wire, general); algorithms ship manifold-first** | Parasolid grade demands it and it cannot be added to a manifold-only topology later |
| Linear algebra crate | **`nalgebra`** | Point/vector/unit distinction and small solvers; `glam` at consumer boundaries |
| Sketch solver | **Out of scope** | Belongs to the application; the kernel is debuggable without it |
| Speed | **Design constraint, not goal**: no global state, `Send + Sync`, parallel-friendly layouts | "Blazingly fast" was a pun; "proper kernel, no shortcuts" is the brief |
| Licences of references | **Read everything, copy nothing** | OCCT is LGPL; more importantly, a transliterated algorithm never fits the representation it was not written for |
| Test oracle | **Open CASCADE via Python, differential on every fixture** | The only open ground truth of industrial quality; running it is legally clean |
| First milestone | **Vertical slice (box − cylinder through every crate), not bottom-up libraries** | Proves the representation before it is broad; Fornjot's libraries were broad and never met |

## 10. Directives for the AI Agent
With this seed agreed, and before writing kernel code:
1. **Scaffold the docs system**: `docs/README.md` with the three tiers, `docs/adr/`, `docs/ideas/`, `docs/plans/`, `BACKLOG.md`, `AGENTS.md`/`CLAUDE.md` with a ≤15-line current state, and the `/idea` `/plan` `/work` `/retire-plan` `/close-cycle` skills adapted to a library without a GUI (seeing geometry means "render a fixture to PNG and look at it").
2. **Architecture document** (`docs/ARCHITECTURE.md`): crate boundaries and the layer rule; the arena and handle model; the operation signature and error model; the checker's place in debug and release builds; threading and the wasm constraint; how a consumer's kernel facade maps onto the API.
3. **Data model document** (`docs/DATA-MODEL.md`): the geometry enums with their parametrisation and seam conventions; topology entities, orientation and adjacency; pcurve and tolerance semantics with the full invariant list the checker enforces; the provenance record; the native format.
4. **Roadmap** (`docs/ROADMAP.md`): cycle 1 as in §6 with its acceptance corpus spelled out as numbers (volume, area, counts, Euler characteristic, provenance stability); cycle 2 as the application gate; later cycles as one line each.
5. **Set up the oracle before the first boolean.** The OCCT Python environment, the fixture format (STEP + expected-values JSON), the PNG dump, and the property-test harness are cycle-1 deliverables in their own right and come first.
6. **Every failure becomes a fixture.** A reproduced bug is shrunk to a minimal case and committed with the desired assertion, `#[ignore]`d until it passes; the failure is never kept as accepted behaviour.
7. **Write an ADR for each `⚠ OPEN` when it closes.** Known open questions at kickoff: whether cylinder–cylinder intersection curves are stored as exact parametrisations or fitted splines (STEP needs a representable curve); the arena's garbage-collection strategy for long-lived models; f32 versus f64 at the tessellation boundary; how a consumer's persistent topological references map onto provenance ids.
