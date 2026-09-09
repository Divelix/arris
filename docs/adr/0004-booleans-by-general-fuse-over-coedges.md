# ADR-0004 — Booleans by a General Fuse over coedges: shared paves, faces split in (u, v), the result assembled with kept ids

- Status: accepted (2026-09-07)
- Plan: `m4-booleans` — the assembly half at step 1 (2026-09-07); the
  split in (u, v), the classification and the assembled result with kept
  ids at step 7 (`ops::cut`, 2026-09-08)

## Context

M4 gives the kernel `fuse`, `common` and `cut` over bodies whose faces lie
on planes and cylinders (`docs/ROADMAP.md` §M4). Everything below the
operation exists: M1's intersection table, M2's tolerance-carrying
topology and Euler operators, the checker, `Provenance`. What M4 has to
decide is the *shape* of the algorithm on this representation — where the
pieces of a face come from, how the two operands come to share them, how a
piece is decided in or out, and how a result body is made without
renumbering the entities that did not change.

Read in the reference trees. Open CASCADE's General Fuse
(`ModelingAlgorithms/TKBO`): `BOPDS` is the data structure the whole
algorithm hangs on — a *pave* is a vertex sitting at a parameter on an
edge, a *pave block* the run of an edge between two consecutive paves, a
*common block* the pave blocks of two edges that coincide, and
`BOPDS_FaceInfo` the "in / on / out" lists per face; because the paves of
one interference are attached to *both* edges, the pieces the two
arguments are cut into are shared by construction rather than matched up
afterwards. `BOPAlgo_PaveFiller` fills that structure interference by
interference (vertex/vertex, vertex/edge, edge/edge, edge/face,
face/face), `IntTools` does each intersection with the shapes' own
tolerances, and `BOPAlgo_Builder` then assembles: `myImages` maps each
original sub-shape to its split images and a shape with no image *is* its
own image, so an untouched face comes out of a boolean as the same
`TShape` — the structural sharing that `BRepTools_History`'s
generated/modified/removed records are written against. `BOPAlgo_Builder_2`
rebuilds each face from its split edges through `BOPAlgo_BuilderFace`
(discard the edges to avoid, walk loops, group loops into areas), and
`BOPAlgo_BuilderSolid` does the same one level up for shells and solids.
`BRepClass3d_SClassifier` is the point classifier: a ray from the point,
the faces it hits, the parity of the crossings, with `SolidExplorer`
choosing a direction that does not graze. Mäntylä, *An Introduction to
Solid Modeling*, ch. 14, is the Euler-operator boolean: split each face
against the other solid with `mev`/`mef`, then glue — the path this ADR
does not take. Hoffmann, *Geometric and Solid Modeling*, ch. 4, for the
argument that classification must be consistent between neighbouring
pieces rather than merely correct per piece. `monstertruck`'s
`transversal/classic/` is the same decomposition in Rust without pcurves,
and its failures at seams are why §Faces split in their own (u, v) below
is not negotiable.

## Decision

**The decomposition is the General Fuse, rebuilt over coedges.** Both
operands are cut into pieces once, by the same data, and each operation is
a *selection* over those pieces; `fuse`, `common` and `cut` differ only in
the selection table. `ops::boolean::interferences` is that decomposition as
a value — per face pair its `SurfaceIntersection`, the edge-on-face hits,
the merged section vertices with their tolerances, the paves on every edge
and on every section curve, and the section edges with their two pcurves —
so the agent can print and draw the model the operation is about to build
on, and a boolean that goes wrong is debugged at the pave, not at the
result.

**Paves are shared, not matched.** A point where an edge of one operand
pierces a face of the other becomes one section vertex, tolerance grown by
the rule in `docs/DATA-MODEL.md` §Tolerances, and that one vertex is a
pave on the piercing edge *and* on the section curve of the face pair.
Nothing downstream ever compares two independently computed points to
decide whether they are the same point: they are the same vertex because
they were made once. This is `BOPDS`'s pave/pave-block idea in Arris's
entities, and it is what makes the two operands' pieces meet exactly.

**Faces are split in their own (u, v), through the pcurves.** A face's
loops are already written as pcurves on its surface (ADR-0002), and the
section curves get pcurves on both faces through `pcurve_on`; splitting is
therefore a plane arrangement in the face's parameter space — half-edges
ordered around each vertex by the pcurves' tangent direction, regions
walked, holes assigned by winding — and never a 3D wire-walking pass. A
seam is a pave like any other and a periodic parameter is translated by
whole periods into the fundamental domain, so the representation answers
the question that the tessellation ADR (0003) already refused to guess at.
The same choice as ADR-0003, for the same reason.

**The result is assembled, not operated.** `Builder::assemble(model,
tolerance, Assembly)` is the builder's second entry point: the caller
hands over the faces of the result, each `Keep` (an arena face, used
whole) or `New`, over edges and vertices that are each `Keep` or `New`.
A `Keep` slot *is* the arena's entity — `finish` appends nothing for it
and returns its id — so a face a boolean did not touch comes out of the
boolean as the same `FaceId` and the provenance records nothing about it,
which is `myImages`'s "a shape with no image is its own image" as one rule
in the builder rather than a convention every caller has to keep. An
operator applied to a kept slot drops the mark and `finish` appends that
slot, so the two entry points compose. `assemble` proves what an operator
sequence would have: every loop closes through effective vertices, every
edge is used exactly twice and in opposite directions, no arena entity is
kept twice, the faces are one edge-connected component, and the
Euler–Poincaré line closes at a whole genus, which becomes the builder's.

Mäntylä's split-and-glue is the path not taken. Reaching the same result
through `mev`/`mef`/`mekr` means finding, for every piece of every face, a
sequence of operators that arrives at it — the operators take *positions*
in loops, so the sequence depends on the rotation the previous operator
left behind — and the intermediate states are not bodies any invariant
holds for. It buys the Euler line, which `assemble` checks directly, and
costs the ability to say "this face is unchanged" at all, since every
operator on a face makes a new one. The operators stay the way a
*primitive* or a *sweep* is written, where the sequence is the shape's own
description.

**Classification is a ray cast with the entities' tolerances.**
`arris_check::classify` answers `Inside`, `Outside` or `On(Shape)` for a
point against a body: the eight fixed directions in order, each face's
hits by `intersect_curve_surface`, a hit's (u, v) decided by
`region2::point_side`, the parity of the crossings, a direction abandoned
on a boundary, tangent or coincident hit, and `Undecided` naming the body
and the point when all eight were — never a guess. It is the checker's B1
made public and complete, so the row that proves a shell nesting and the
predicate that decides a boolean piece are one piece of code, which is
Hoffmann's consistency argument taken literally. A piece is classified at
one point strictly inside it (`region2::interior_point`, carried to 3D),
not at its centroid: a sliver's centroid rounds onto its own boundary.

**Coincident and tangent faces are named cases, never a tolerance
accident.** A `Coincident` surface pair puts the other face's edges onto
this face through `pcurve_on` and the overlap is decided by the same (u, v)
arrangement, with the piece kept once, from the operand the selection
table names, by the two normals' agreement. A `Tangent` pair contributes
no section edge when the touch is from outside; a tangent ruling that
would end up interior to two result faces is `Degenerate {
TangentContact }` naming the pair, because the manifold `Solid` M4 returns
has no way to say "these two faces touch along a slit". Neither case is
reached by a comparison against a literal.

## Consequences

- An operation that leaves most of a body alone is cheap and its
  provenance is small: the untouched faces, edges and vertices are shared
  with the input by id, and a `Modified` record exists exactly for what
  changed. A face whose loop changed at all — even only by an edge that
  was split — is a new face, `Modified` from the old one; that is the
  boundary of "unchanged", and it is drawn by the builder, not by a
  heuristic.
- `assemble` makes the builder's kept/new distinction visible in
  `Builder::dump`, so two builders that describe the same shape but share
  different amounts of it are told apart in a test.
- A body the Euler operators cannot build, `assemble` cannot build either:
  a sphere's degenerate pole edges are used once each, which `finish` has
  always refused. M4's operands are planes and cylinders, so nothing in
  the corpus needs it; a one-use edge is a cycle-2 question about what a
  degenerate edge is, not a boolean question.
- The pave model is a value with a `Display`, which makes the failure of a
  boolean inspectable before the result exists — the `inspect` skill's
  `interferences`. The cost is that the model is built in full even when
  the operation is going to refuse.
- Splitting in (u, v) needs the arrangement to be robust where the
  intersection curve meets a loop at a shallow angle; the tie-break is the
  pcurves' curvature and an unresolved tie is a typed error, not a
  perturbation.
- Nothing here supports more than one shell: a disjoint fuse, a cut that
  splits its target and an enclosed cavity are each `Degenerate {
  MultiShell }` in M4 (plan `⚠ OPEN` 2). A `General` body for them is
  cycle 2's, and it is an addition to the assembly step, not a change to
  the decomposition.

## Alternatives considered

- **Mäntylä's split-and-glue through the Euler operators**: above. It is
  how a textbook writes the boolean when the representation has no
  pcurves and no arena; it would make every result body's ids new.
- **Matching the two operands' pieces after splitting them separately**,
  which is what a boolean without a shared pave model does: every meeting
  of two pieces becomes a tolerance comparison, and the comparisons are
  not transitive. `BOPDS`'s answer — compute the point once, attach it to
  both — is not an optimisation, it is the reason the result closes.
- **Splitting in 3D by walking wires**, as `truck-shapeops` does: the
  seam has to be guessed at, and a face on a cylinder is the case that
  breaks.
- **Classification by the piece's centroid**, or by a sample of its
  boundary: the centroid of a sliver is on its boundary, and a boundary
  sample classifies `On` and decides nothing.
- **A fuzzy tolerance**, OCCT's `BOPAlgo_Options::SetFuzzyValue`: it turns
  a failed intersection into a successful one by widening every
  comparison at once, and the widening is not recorded in the result's
  tolerances. Arris's tolerances are the model's and grow by a stated
  rule (`SEED.md` §9).
