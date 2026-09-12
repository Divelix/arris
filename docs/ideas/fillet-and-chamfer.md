# Idea: fillet-and-chamfer

- Status: Open
- Raised: 2026-09-12
- Prompt (verbatim from the human): "do all 3" — accepting "`/idea` for
  fillet and chamfer, before any intersector work"

## Problem

C2's first line (`docs/ROADMAP.md` §C2): constant-radius fillet and flat
chamfer of named edges, several in one call (architecture §How a
consumer's kernel facade maps on, `ops::fillet`, `ops::chamfer`). The
consumer's facade takes one radius and a list of edges and names one
blend face per edge; its probes record where the truck lineage fails —
a second fillet on a filleted body (its blends are fitted curves it
cannot read back) and a vertical plus a cap edge of a box (declined
silently, while two cap edges meeting at the same kind of corner blend).
It is the largest unknown left in C2, and how the blend is built decides
how much of the intersector and checker lines C2 has to take on.

What Open CASCADE builds (8.0.1, `BRepFilletAPI`, r = 0.2 on a 2-cube;
a scratch run, not yet a fixture): one edge — a cylinder, two circle
ends; two edges at a corner whose third edge stays sharp — two cylinders
meeting in one ellipse, the *same* solid whichever two of the three
edges (so the declined probe is a miter, not a vertex blend); all three
— a sphere octant; a second fillet on an edge clear of the first — plain
cylinders, nothing special; a hole's rim — a torus, its chamfer a cone;
a boss's concave base — a torus. A second fillet on an edge that *runs
into* the first blend is rebuilt by Open CASCADE as the three-edge
corner, i.e. a blend over a blend.

## Constraints it runs into

- `SEED.md` §9: analytic surfaces first-class, exact intersections for
  analytic pairs; provenance part of the contract; no shortcuts. A blend
  approximated by NURBS is exactly the truck failure the probe records.
- `SEED.md` §6 and `docs/ROADMAP.md` C6: fillet networks, vertex blends,
  variable radius and blends over blends are a later cycle.
- ADR-0002 (Euler operators, explicit pcurves, role provenance) and
  ADR-0004 (`Builder::assemble` with kept ids; tangent contacts are a
  typed refusal, not an imprint).
- Architecture §The checker: the corpus holds every result to `Full`
  with nothing unchecked, and today S5 and B1 report cone, sphere and
  torus faces unchecked and `classify_point` refuses rays against them.
- Kernel rules: no tolerance literals, typed refusals naming entities,
  deterministic ids.

## Options

### A — Blend as a boolean tool

Build, per edge, the solid between the corner and the rolling ball —
an extrude of a corner-minus-quarter-disc profile for a straight edge, a
revolve of it for a circular one (both exist, M5) — and `cut` it from a
convex edge or `fuse` it onto a concave one (ADR-0004). Reuses the
General Fuse, its provenance and the sweeps.

Trade-offs: the tool's blend face is tangent to the body's faces along
exactly the edges that must survive, which is the `Tangent` pair
ADR-0004 refuses as `TangentContact` or leaves unimprinted — the risky
part, needing a spike. The tool has to stop at the edge's ends or it
gouges whatever lies beyond them (a step, a wall), so the end problem is
not avoided, only moved. Two blends at a corner become a general
cylinder–cylinder boolean. Provenance roots at `Role::Extrude` of a
private tool and has to be re-rooted to the edge. Nothing of it carries
into C6: a variable-radius or NURBS-face blend is no sweep. Cost: ~5
steps after a spike that may fail.

### B — Direct rolling-ball construction on analytic pairs

For each edge, the blend is decided by its two faces in closed form:
plane–plane → a cylinder on the offset planes' line (chamfer: a plane);
plane–cylinder along a circle → a torus (chamfer: a cone); plane–cylinder
along a ruling → a cylinder; convex or concave by the dihedral. The
contact curves are known from the construction (lines, circles), so the
two faces' loops are rewritten with them and the blend face is
assembled beside the kept faces (`Builder::assemble`, ADR-0004).
Provenance is direct: the blend face and its contact edges `Generated`
from the edge, the two faces `Modified`, the edge and the corners it
consumed `Deleted` (data-model §Provenance, no new `Role`).

At an end vertex the blend is trimmed by the face across the corner —
cylinder–plane, which the intersector has. Two blends meeting at a
vertex whose third edge stays sharp meet in a miter: equal-radius
cylinders whose axes cross, whose intersection factors into two planar
ellipses (a closed-form arm, not the quartic `⚠ OPEN`); two chamfers in a
line. Three blends at a vertex of three planes are a sphere through the
ball's one centre, tangent to all three cylinders along circles. A
closed edge (a hole's rim, a boss's base) has no ends. Everything else —
an edge adjacent to a blend face (a tangent chain), a vertex of more
than three edges, a pair outside the table, a radius the faces cannot
hold — is a typed refusal naming the edge, and each is C6's.

Trade-offs: more construction code than A, one arm per face pair and
per corner kind, and the corner cases are where the work is; but every
surface is exact, every result stays inside what the intersector and
checker can prove, and the second-fillet probe falls out (the input is
any body, not a primitive). It is the shape `ChFi3d` takes in Open
CASCADE (stripes per edge; `ChFi3d_Builder_C2.cxx` intersects two
stripes at a corner, `ChFi3d_Builder_CnCrn.cxx` fills n-corners) and
that monstertruck-fillet takes without exact surfaces; C6 extends it
rather than replacing it. Cost: ~8 steps — blend table and contacts,
one edge with trimmed ends, closed edges, the miter, the sphere corner,
the oracle's `fillet`/`chamfer` recipe ops and fixtures — plus the
checker arms below, likely their own plan.

What B needs from the other C2 lines, and no more: S5 face-pair arms and
B1 ray casts against torus, cone and sphere *in the positions a blend
puts them* — coaxial with a cylinder, perpendicular to a plane, a sphere
on a cylinder's axis — all closed forms; the ellipse miter arm in
`cylinder_cylinder`. A general plane–torus or crossing-cylinder quartic
is not on the path and stays C3's.

### Do nothing

The consumer cannot swap kernels: fillet and chamfer are in its facade
and its document model. C2 does not close.

## Recommendation

**B.** A buys reuse at the price of the one thing ADR-0004 deliberately
refuses (tangent contact along a surviving edge), still owns the end
problem, and is thrown away by C6. B keeps every surface analytic and
every row checkable, gives provenance the consumer names blend faces by
without a matcher, and scopes C2's intersector and checker lines down to
the coaxial arms a blend produces.

What would change my mind: the miter or the end trimming turning out to
need the general quadric intersector anyway — a spike on the box corner
settles that in one step, before the plan commits to the rest.

## Decision for the human

1. Construction: **B, direct rolling-ball on analytic pairs** (A,
   boolean tool, otherwise)?
2. Three blended edges at a vertex of three planes (the sphere corner —
   "fillet every edge of a box"): **in C2**, one step, since it is closed
   form and the first thing a user tries; or a typed refusal until C6?
3. A tangent chain, a vertex of more than three edges, a blend face as an
   input face: **typed refusals in C2**, each C6's?
4. **An ADR** records the construction (stripes per edge, closed-form
   corners, provenance rooted at the edge) when the plan's first step
   lands; and the C2 intersector and checker lines are narrowed to the
   coaxial arms B needs, with the general pairs moved to C3.
