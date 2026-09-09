# ADR-0002 — Euler operators over a staging builder; explicit pcurves; provenance rooted in roles

- Status: accepted (2026-09-06)
- Plan: `m2-topology` steps 9 and 10

## Context

architecture makes entities immutable: an operation appends and
returns a handle to a new body that shares every untouched entity. Euler
operators — the classical way a solid modeller makes topology one valid
step at a time — mutate: `mev` inserts two coedges into a loop that
already exists. The two had to meet somewhere, and the roadmap's M2
section names Euler operators as the only way an operation builds
topology.

Three questions were open in the plan. Where the mutable state lives.
Whether the operators compute pcurves from the curve and the faces'
surfaces or take them from the caller (the plan's first `⚠ OPEN`). And
what a primitive, which has no input body, writes into its provenance
record so that a persistent name has a root (the second `⚠ OPEN`).

Read in the reference trees: Open CASCADE's `BRep_Builder` (`ModelingData/
TKBRep/BRep`), which is not Euler-based — it makes bare vertices, edges,
faces and adds sub-shapes to shapes with no invariant kept between calls,
leaving `BRepCheck` to find out afterwards — and `BRepTools_History`
(`ModelingData/TKBRep/BRepTools`), whose three relations Generated /
Modified / Removed data-model's provenance took its vocabulary from,
and whose records are keyed by input shapes only, so a shape with no
input has no record. Mäntylä, *An Introduction to Solid Modeling*, ch. 9,
for the operator set and its half-edge formulation.

## Decision

**A staging builder.** `arris_topo::Builder` is a value holding one body
under construction — vertices, edges and faces in tombstoned slots, loops
as sequences of effective uses — edited by Mäntylä's ten operators
(`mvfs`/`kvfs`, `mev`/`kev`, `mef`/`kef`, `mekr`/`kemr`,
`kfmrh`/`mfkrh`) and frozen into the arena by `finish(&mut Model,
BodyKind)`, which appends every live slot in order inside a transaction
and returns the body with the slot → id maps. The arena stays
append-only and immutable; the mutation is confined to the builder, and
nothing outside the builder, the raw insert and `import` appends
topology. Two properties are the builder's contract and its tests: every
operator keeps the Euler–Poincaré line at zero, and every operator
followed by its inverse restores the builder byte for byte — a kill
leaves a tombstone the next make fills (most recently freed first), loops
are kept in a canonical rotation and order, and a kill returns the record
its make takes. Positions in a loop are `(face, loop, coedge index)`,
never bare vertices, because a seam's vertex stands at two junctions of
one loop.

**Explicit pcurves.** `mev` takes the two pcurves of the strut's two uses
and `mef` one per side of the new edge; `set_pcurve` gives or replaces
any; every pcurve is `Option` and `finish` refuses a missing one. The
builder never computes a pcurve and never evaluates geometry. A seam is a
strut whose two pcurves differ by the period, and `pcurve_on` cannot know
which of the two uses it is asked for; the caller — a primitive now, the
boolean in M4 — always can. The cost, a coedge that `mef` moves to a face
on another surface keeps a pcurve id that is no longer its own until the
caller replaces it, is paid in exchange for a builder that never guesses;
the checker's E4 is where an unreplaced one is reported.

**Provenance rooted in roles.** The `generated` and `modified` keys of a
`Provenance` are `Origin::{Entity(Shape), Role(Role)}`. A primitive
records every entity it makes as `Generated` from a `Role` —
`Role::Box(BoxPart)`, `Role::Cylinder(CylinderPart)`, an exhaustive enum
extended per operation kind — so `origins(output)` of any entity, followed
through `Provenance::then` across a chain of operations, ends at a role,
which is what a consumer's persistent name is a function of.

## Consequences

- An operation's algorithm is written against the builder's operators,
  and a wrong sequence fails at the operator (a typed `BuildError`) or at
  `finish`, not as a corrupt arena. The checker still runs on the output:
  the builder proves combinatorics, the checker proves geometry.
- The builder makes one shell and finishes a `Solid` only. Voids
  (multi-shell solids), sheets, wires and general bodies are the raw
  insert's until an operation needs them, which cycle 1 does not.
- `kfmrh` requires its two faces on one `SurfaceId` with opposite
  orientations — coplanar with opposing outward normals — so the moved
  loop keeps its pcurves. A face's surface is not changed after creation;
  a primitive creates a face on the surface it will end on and moves the
  coedges to it.
- Roles make a fourth relation unnecessary: nothing is "created from
  nothing", and the bolt-pattern rebuild fixture names a hole's wall
  after the tool cylinder's wall role through the cut's `Generated` chain.

## Alternatives considered

- **Operators directly on the arena** (a mutable model): every entity
  written in place, undo and structural sharing gone, exactly what
  architecture rejected.
- **`BRep_Builder`'s approach** — bare makes and adds, validity found
  afterwards: the checker becomes the only line of defence and a boolean
  can assemble anything. Rejected for the same reason Fornjot's
  post-mortems give: a representation that can be wrong mid-construction
  is wrong in ways the checker names too late.
- **Pcurves computed by the builder** from the curve and the surface:
  right on a plane, ambiguous at a seam, `Unsupported` on the surfaces
  M5 brings; the builder would grow a geometry dependency and a wildcard.
- **A fourth relation `Created`** with no origin: the naming chain ends in
  nothing, and the consumer is back to matching by geometry.
