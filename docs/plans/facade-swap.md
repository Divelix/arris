# Plan: facade-swap

- Started: 2026-09-17
- Milestone: C2's acceptance gate (docs/ROADMAP.md §C2 — the application gate)
- Idea (verbatim from the human): "what is next step to develop?" → "yes, go ahead" (planning the facade swap `AGENTS.md` names as next)

## Goal

The first consumer's kernel facade runs on Arris. Its adapter implements
every facade method over `arris` (primitives, extrude, revolve, boolean,
transform, fillet, chamfer, tessellate, face frame, mass properties,
export, projection, retention). Its persistent names come from
`Provenance` and ADR-0009's split order, with no geometric matcher. Its
probe corpus is green in its own units: every `#[ignore]`d twin runs and
passes, and every probe of the old backend is deleted. The truck-lineage
crates are gone from its manifest and lockfile, and it still builds for
`wasm32`. On the Arris side, everything the facade feeds the kernel today
is built or refused by a typed error at `Full` check. That includes the
elliptic profile segments its sketcher emits. The C2 **Accept** line then
holds, and `/close-cycle` can run.

## Non-goals

- Booleans with an elliptic face as an operand: a typed refusal, like the
  quadric guard (C3 for the analytic pair, C4 if the surface is NURBS).
- Non-rigid transforms (scale, mirror). The consumer passes only
  translations and axis–angle rotations today. Its adapter refuses any
  other matrix.
- Any consumer feature the facade does not reach (assemblies, robotics
  export, sketch UX), and the consumer's own roadmap.
- Publishing to crates.io, creating a remote or pushing. Those belong to
  the human (open question 1).
- Anything on the C2 **Out** list.

## Where the work lands

Steps 1–6 are Arris commits, and they follow this repo's rules. Step 7 is
the swap itself, which is code in the consumer's repository. That repo
has rules of its own: it keeps a plan in its own `docs/plans/`, and it
needs an ADR superseding its kernel-of-record decision before the work
starts. The consumer's plan is written and executed there, and its
commits carry its own plan-step suffix. Its outline is sketched under
step 7 so this plan shows the whole gate. Step 7's box is ticked here
when the consumer's acceptance passes. Tracked Arris docs keep calling
the application "the consumer" and never give its path.

## Design deltas

- **Public API, breaking:** `geom::ProfileSegment` gains an elliptic-arc
  segment, and `geom::ProfileLoop` a full ellipse. Every `match` on them
  grows an arm. `Profile::edges` validates and orients them the way it
  already does arcs.
- **The profile grammar** (ADR-0014): `ProfileLoop::Ellipse { center,
  major, minor_radius }` and `ProfileSegment::EllipseTo { to, center,
  major, minor_radius, ccw }`, with `ProfileError::DegenerateEllipse` and
  `ProfileError::OffEllipse`. Radii equal within `tol.linear` build a
  circle edge.
- **The surface an extruded ellipse sweeps** (ADR-0014):
  `Surface::EllipticCylinder { frame, major_radius, minor_radius }` and
  its `SurfaceKind`. This is breaking: every surface `match` gets a real
  arm or a named `Unsupported`. It also brings:
  - `Line` pcurves for the section ellipse and the rulings;
  - closed-form arms for plane against it in every pose, for parallel
    axes against a cylinder or another elliptic cylinder, and for a
    line against it;
  - STEP `SURFACE_OF_LINEAR_EXTRUSION` over an `ELLIPSE`;
  - `project_to_plane` unchanged, since the new edges are ellipses and
    lines.
- **A revolve with an elliptic segment is refused** (ADR-0014):
  `OpError::Degenerate` with `Reason::EllipticRevolve { loop_index,
  segment }`.
- **Pave model guard:** it refuses an elliptic face as it refuses a
  quadric, with a typed error naming the face.
- **`arris-io`:** `stl::write` and `obj::write` take several bodies'
  meshes into one file, beside `step::write(&model, &[bodies])`. Whether
  that is a slice of `TriMesh`es or a `TriMesh::append` in `arris-mesh`
  is step 5's call, named in its commit body.
- **docs/DATA-MODEL.md §Profiles, §Surfaces:** the new segment and
  surface.
- **docs/ARCHITECTURE.md §How a consumer's kernel facade maps on:** the
  rows get the facts step 7 proves:
  - the adapter's per-shape index table stands in for the backend's
    iteration order;
  - an elliptic profile maps onto the new segment;
  - several bodies export into one mesh file;
  - "what the facade has today that Arris will not have" is revised.
- **No new crate and no layer change.** The adapter lives in the
  consumer.

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness or
bound has to be established here.

- [x] Step 1 **[2]** — ADR-0014: elliptic profile segments. Sets the
  surface an extruded ellipse sweeps, what a revolve does with one, the
  STEP entities, and the refusal in the pave model (design deltas above).
  Before deciding, read how the consumer's sketcher emits ellipses and
  whether its documents revolve them. Commit: `docs(adr)`.
- [x] Step 2 **[3]** — Extruding a profile with elliptic segments. Covers
  the representation step 1 chose, both pcurves (cap plane and side
  face), tessellation, the checker's arms for the side against its caps
  and against its neighbours, `classify_point`, mass properties and the
  STEP writer. Fixtures, each against the oracle and passing every corpus
  stage with nothing unchecked:
  - `sweep/extrude-ellipse`: a full ellipse;
  - `sweep/extrude-elliptic-slot`: two half-ellipses joined by lines;
  - `sweep/extrude-plate-elliptic-hole`: an ellipse as a hole loop.

  Plus a property: random ellipses in random poses extrude clean at
  `Full`, with volume πab·h. The API change is named in the commit body.
- [x] Step 3 **[2]** — A revolve with an elliptic segment, as step 1
  decided. If refused, a refusal-by-fixture (`sweep/revolve-ellipse`)
  with its typed `Reason`, and a backlog line for the build. If built,
  a fixture against the oracle.
- [ ] Step 4 **[1]** — Booleans with an elliptic-faced operand are
  refused by the pave guard with a typed error naming the face.
  Refusal-by-fixture: `boolean/elliptic-operand-cut`. A backlog line
  points to C3 or C4. The guard's arm landed with step 2 (the closed
  forms it holds back went in there, and ADR-0014 wants the guard with
  them); this step is the fixture and the backlog line.
- [ ] Step 5 **[1]** — Several bodies in one STL or OBJ file, per the
  design delta. The test reads the file back with `RWStl` through
  `tools/oracle/mesh.py` and with the test's own OBJ parser: the triangle
  count is the sum, and each body's positions are unchanged.
- [ ] Step 6 **[2]** — The probe corpus in the consumer's units (metres,
  `Precision` at the micrometre scale ARCHITECTURE §Units gives it).
  Fixtures, each against the oracle:
  - `boolean/probe-through-hole-m` (8.7434e-5);
  - `boolean/probe-blind-hole-m` (1.8743e-4);
  - `boolean/probe-flush-union-m`;
  - `blend/probe-second-fillet-m`;
  - `blend/probe-cap-and-vertical-fillet-m`;
  - `sweep/probe-revolve-onto-axis-m` (2π at the consumer's size).

  The enclosed cavity (9.36e-4) and the transversal cut (2.2079e-5)
  already pass in these units. This step retires the "at another scale"
  caveat in the **Accept** line. A fixture that fails is a regression
  fixture, and it is fixed within this step or split into a step of its
  own.
- [ ] Step 7 **[2]** — The swap, in the consumer's repository, under its
  own ADR and plan; ticked here when that plan's acceptance passes. It
  needs open questions 1 and 2 answered first. Its plan's outline:
  1. The consumer ADR superseding its kernel-of-record decision and its
     "no curved boolean operands" product constraint, then its plan.
  2. An `ArrisKernel` beside the old backend behind the same trait:
     - cgmath ↔ nalgebra at the adapter;
     - its `Profile` → `geom::Profile` (arc → `ArcTo` with a computed
       `via`, a ±τ arc → `Circle`, an ellipse → step 2's segment,
       `SegmentKey` ↔ `(loop_index, segment)`);
     - the centred box and cylinder conventions, and `symmetric` as a
       pre-translation;
     - a rigid-only `Matrix4` → `Isometry`;
     - the memo store over `Model::retain`;
     - a per-shape index table for faces, edges and vertices;
     - `KernelError` from `OpError`.
  3. `NameMap` built from `Provenance`:
     - `Side`, `CapStart` and `CapEnd` from `Role`;
     - `FromA`, `FromB` and `Split(k)` from `modified_from` in the split
       order;
     - `Instance` from `transform`;
     - `FilletBlend` and `ChamferBlend` from `generated_from(edge)`.

     The naming goldens (`tests/naming/*`) pass. A deliberate change goes
     in its own commit with a reason (open question 3).
  4. The facade's test suite runs against `ArrisKernel`:
     - every `#[ignore]`d twin runs and passes (flush union, through
       hole, blind hole, enclosed cavity, transversal cut, revolve
       touching the axis, second fillet, a vertical and a cap edge in one
       call);
     - the soft-failure tests they replaced are deleted;
     - the "profile crossing the axis fails" test stays.
  5. `ArrisKernel` becomes the application's kernel. Also:
     - its document acceptance tests and its `wasm32` build pass;
     - its headless snapshots are refreshed only with their diffs read;
     - an app smoke run through its `visual-debug` skill.
  6. The old backend goes: `TruckKernel`, the name matcher, the backend
     probe file and every truck-lineage crate in the manifest.
     `cargo tree` and the lockfile show none of them.

## Acceptance

- In Arris: `cargo nextest run --workspace` is green, including the corpus
  lint. Steps 2–4's and 6's fixtures pass every stage against the oracle
  with nothing unchecked, and step 2's property is green at its
  configured case count. `cargo build --workspace --target
  wasm32-unknown-unknown` passes.
- In the consumer: its full gate (fmt, clippy `-D warnings`, its size and
  wasm lints, `cargo test --all`, `--all-features`, the `wasm32` build)
  passes with no truck-lineage crate in `Cargo.lock`. There is no
  `#[ignore]` whose reason names the old backend, and its naming goldens
  pass with the matcher deleted.
- Together, these are the C2 **Accept** line, item by item.

## Docs to update on completion

- `docs/ROADMAP.md` §C2: mark the gate done with the date and ADR-0014,
  drop the "at another scale" caveat, and hand off to `/close-cycle`.
- `docs/ARCHITECTURE.md` §How a consumer's kernel facade maps on: the
  rows under design deltas. §Crates: `arris-geom`'s "lines and arcs"
  becomes lines, arcs and elliptic arcs.
- `docs/DATA-MODEL.md` §Profiles, §Surfaces: the elliptic segment and the
  surface ADR-0014 chose.
- `docs/adr/README.md`: ADR-0014.
- `docs/BACKLOG.md`: elliptic-faced booleans (C3/C4). If step 3 refused
  it, the elliptic revolve. A consumer-requested non-rigid transform, if
  that ever comes up.
- `AGENTS.md` current state: C2's gate passed, the facade swapped, next
  is `/close-cycle` into C3.

## Open questions

- `⚠ OPEN:` **How the consumer reaches Arris** (human, before step 7).
  Arris has no git remote. The consumer has one and runs CI, so a sibling
  `path =` dependency breaks its CI and every fresh clone. The options:
  - push Arris to a remote and use a git dependency pinned to a revision
    (recommended: no release needed, and the pin moves deliberately);
  - publish a 0.1 over the crates.io reservation;
  - a path dependency, accepting that the consumer's CI can't build it
    until one of the above happens.
- `⚠ OPEN:` **The consumer's kernel-of-record decision** (human, before
  step 7). The consumer's recorded decision keeps its current backend and
  parks an own kernel. The swap supersedes that in its repository, and
  that ADR is the human's to accept there.
- `⚠ OPEN:` **Naming goldens for full-circle holes** (agent, in step 7's
  consumer ADR). The old backend split a full circle into several side
  faces. Arris makes one. The consumer's `Side { instance }` names for a
  hole loop change unless the adapter pins them. The goldens move only by
  a deliberate commit that says why.
- **Elliptic revolve: build or refuse.** Answered in step 1 (ADR-0014):
  refused. The consumer's ellipse entities sit behind a feature that is
  off by default, no sketch tool draws one, and no document or test
  revolves one, so the refusal is no regression. Step 3 is the
  refusal-by-fixture.
- **Finding (step 1): the consumer's extruded ellipses change shape.**
  Its old backend builds every elliptic piece as a circular arc through
  three points of the ellipse. Arris builds the exact ellipse, so the
  consumer ADR in step 7 names this as a correction.
- **Finding (step 2): the area of an elliptic cylinder is no polynomial
  quadrature.** Its area element `√(a² sin²u + b² cos²u)` has complex
  singularities `atanh(b/a)` off `u = 0` and `π`; the region integral's
  inner split now aligns to the surface's own grid and the elliptic
  cylinder's `inner_step` is a sixteenth of a turn (1e-11 at an aspect
  of eighteen, where a quarter turn left 1e-7 and an unaligned split
  1e-6). The oracle had the same problem: Open CASCADE's fixed-order
  `SurfaceProperties` is 2e-5 off on a `Geom_SurfaceOfLinearExtrusion`
  and its adaptive overload 3e-3 off, so `measure.py` takes such a
  face's area as its basis arc's length (`GCPnts_AbscissaPoint`, to
  1e-13) times its height — a rectangle in (u, v) for every face an
  extrude makes — and keeps the fixed order over the whole shape, and
  every committed number bit for bit, when every face is elementary.
- **Finding (step 2): parallel-axis rulings are ordered by parameter.**
  ADR-0014 says the elliptic pairs follow the parallel cylinders'
  ordering rule (offset along `Z × ŵ`), but an elliptic section pair
  has up to four rulings and a coaxial pair (four crossings of an
  ellipse and a circle between its radii) has no `ŵ`. The rulings come
  ascending by the first operand's section parameter instead, which is
  deterministic in every case; `docs/DATA-MODEL.md` §Curves records it.
- **Finding (step 2): a section pair that touches and crosses is
  `Unsupported`.** `SurfaceIntersection` is `Tangent` or `Transversal`,
  never both, and the meridian arm already refuses a mixed result; the
  elliptic pair does the same. No extrude makes such a pair (its faces
  share their rulings), and booleans on elliptic faces are refused by
  the guard, so nothing reaches it at `Full`.
