# ADR-0006 — Lumps in one `Solid`: several shells, nested by B1, derived and never stored

- Status: accepted (2026-09-13)
- Plan: `multi-shell` step 1
- Amends: ADR-0004, the consequence "Nothing here supports more than one
  shell … A `General` body for them is cycle 2's"

## Context

C2's application gate needs results of more than one shell
(`docs/ROADMAP.md` §C2): the consumer's enclosed-cavity probe — a box of
0.1 less a box of 0.04 inside it, 9.36e-4 and two shells — a `cut` that
splits its target, a `fuse` of operands that do not meet, and a full
revolve of a profile with holes, each hole closing into a cavity. M4 and
M5 refuse every one of them as `Reason::MultiShell`, because the `Solid`
of cycle 1 is one shell, and ADR-0004 left the representation of the
answer open with a `General` body as the guess.

A result of several shells can be carried three ways: as a `General`
body, as several bodies per operation, or as one `Solid` whose shells
nest into connected regions of material.

Read in the reference trees. Open CASCADE's `TopoDS_Solid` holds a list
of shells with no role on any of them: `BRepClass3d::OuterShell` finds
the outer one by classifying, and `BOPAlgo_BuilderSolid`
(`ModelingAlgorithms/TKBO`) sorts a boolean's shells into *growth* shells
and *holes* and gives each hole to the innermost growth shell containing
it — of two candidates, the one inside the other — so a result of several
growth shells is a compound of solids, each one outer shell with its
holes. The STEP side (`TopoDSToStep_MakeBrepWithVoids`,
`StepToTopoDS_Builder`) writes a solid with holes as a `BREP_WITH_VOIDS`
whose voids are the hole shells reversed under an
`ORIENTED_CLOSED_SHELL` of orientation false, and reverses them back on
reading. Parasolid's body is regions bounded by shells; ACIS's is lumps,
each a connected region, bounded by shells. Run through the oracle,
`BRepCheck_Analyzer` rejects a `TopoDS_Solid` of two outer shells whether
they are apart or touch at a vertex or along an edge, and accepts a
compound of the same two solids in all three cases: its solid is one
region of material, and its compound is what may touch. The consumer's
facade returns one shape per operation.

## Decision

**A `Solid` body holds one or more lumps.** A *lump* is an outer shell —
one enclosing positive volume by the checker's Gauss integral — and the
void shells, enclosing negative volume, whose innermost containing shell
it is. A cavity is a lump's void; the two halves of a split and two
disjoint operands of a fuse are two lumps; a lump may sit inside another
lump's cavity. The body stays what `docs/DATA-MODEL.md` §Entities says it
is — a kind and a list of shell uses — and every guarantee a `Solid` gives
holds shell by shell: each shell closed, every edge of it used by exactly
two of its coedges.

**Shells share nothing.** No edge and no vertex is used by two shells of
one body: `Builder::assemble` refuses both, and a boolean whose result
shells would touch refuses it by name before anything is assembled
(`Reason::NonManifold`). Two lumps meeting at a point or along a line are
a non-manifold body, C7's `General`, not a `Solid` with a special case —
the answer the reference trees give too, where touching is a property of a
compound and never of a solid.

**Lumps are derived, never stored.** Which shell is outer and which void
belongs to it is geometry, and the checker's B1 is where geometry is
proven: every shell of positive volume is outer and every shell of
negative volume a void; each void's innermost container is an outer shell;
each outer shell's is none or a void; no two shells of the body meet.
`arris_check::lumps(&Model, Body)` returns that nesting as a value for the
writer and the consumer, over the same ray cast B1 runs, so the row that
proves the nesting and the query that reads it cannot disagree. No entity,
no field of `Body` and nothing in the native format changes. An
operation stores a body's shells lump by lump, the outer shell first and
then its voids, so a dump reads as the lumps, but nothing relies on the
order.

**STEP writes a lump per solid entity.** A lump without voids is a
`MANIFOLD_SOLID_BREP`; one with voids a `BREP_WITH_VOIDS` over its outer
`CLOSED_SHELL` and an `ORIENTED_CLOSED_SHELL` of orientation false per
void, the void's closed shell written with every face turned, which is
how the reference reader gives it back as a hole. All of a body's lumps
go in the product's one shape representation; the oracle reads them as
that many solids.

## Consequences

- `Assembly` carries its faces as shells (`Assembly::shells`) and
  `Built` returns them (`Built::shells`). The Euler–Poincaré line is
  checked per shell, the builder's genus their sum; `finish` over the
  operators still makes one shell.
- B1 changes from "exactly one outer shell" to the nesting above. It costs
  a ray cast per pair of shells and a face-pair test between shells whose
  boxes overlap, nothing for a body of one shell. `lumps()` pays that on
  every STEP write and corpus run; if it shows up as a cost, storing the
  lump structure on `Body` is a new decision, with a native-format
  version bump.
- `Reason::MultiShell` goes: a boolean groups its surviving pieces into
  shells by their shared edges and orders them into lumps, and a full
  revolve of a profile with holes returns a void per hole. A result shell
  is `Modified` from every operand shell a piece of it came from, and a
  shell made only of a cut tool's pieces is `Generated` from the tool's
  shell; a revolve's cavity is `Generated` from `SweepPart::Cavity`.
- The corpus counts `solids` as lumps, which is what Open CASCADE reports
  for the same result.
- Queries that integrate over a body (`measure`, the mesh's signed volume)
  already sum over every shell use and need nothing.

## Alternatives considered

- **A `General` body**, as ADR-0004 foresaw: it is the kind whose
  guarantees are the weakest — a face used by two shells, an edge of any
  number of uses — so every consumer of a cavity would lose the `Solid`
  contract and every query that refuses a non-solid (`measure`) would
  refuse the consumer's hollow box. `General` is for what genuinely is not
  manifold.
- **Several bodies per operation**, Open CASCADE's compound of solids: the
  facade returns one shape per operation, the operation signature returns
  one `Body`, and provenance composes one record per call; a split result
  would need a second, list-shaped operation contract for a case that is
  one shape to the consumer.
- **Storing the lumps on `Body`** (a list of `(outer, voids)`): cheaper to
  read, but a second statement of what B1 proves, which the checker would
  then have to hold consistent with the geometry, and a native-format
  change. Deriving first, and storing only on a measured cost, keeps one
  source of truth.
