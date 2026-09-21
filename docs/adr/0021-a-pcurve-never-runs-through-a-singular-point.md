# ADR-0021 — A pcurve never runs through a surface's singular point: fitted by projection, split at the apex and the pole, refused beside them

- Status: accepted (2026-09-21)
- Plan: `c3-conic-hits` step 6 (the decision, the pave model's singular
  vertices and the refusal); the projection fallback, its unwrapping and
  the band landed in step 1
- Closes: the last plan's ⚠ OPEN 5 (⚠ OPEN 1 of `c3-conic-hits`) —
  projection is the only source of a pcurve — and ⚠ OPEN 2: a section
  *through* an apex or a pole is built, one *beside* it is refused by name
- Follows: ADR-0002 (explicit pcurves, the degenerate edge), ADR-0004 (the
  pave model and the face arrangement), ADR-0018 and ADR-0019 (what a
  fitted section is held to)
- Amends: nothing. ADR-0018's and ADR-0019's storage stands.

## Context

A cone's apex and a sphere's poles are points where the parametrisation
collapses: every `u` names the one point. The data model already says
what a face does there — a degenerate edge, a whole line of (u, v) over
one vertex (ADR-0002) — but until C3 no curve a boolean made ever went
near one, because no boolean took a cone or a sphere face.

Two things break at such a point. **A pcurve through it is not a curve in
(u, v):** a circle through a pole arrives with one `u` and leaves with
that `u + π`, a jump no fit follows and no polygon closes. **A pcurve
beside it is a curve, and a violent one:** passing the point at a distance
`d` it turns `u` by up to `π` over a stretch `d` long. Step 1 measured
the fit of that: it never fails, 917 control points just outside the band
and 293 at a hundredth of the radius. Step 6 measured what reads the fit.

Open CASCADE was run, as the oracle, for where it stands; none of it was
read for this. Its cut builds the three fixtures below; with a plane
`3.7e-7` from a ball's pole it returns the whole ball in two shells, and
of the pose step 5 tried first, a cylinder's wall through one pole only,
it builds no solid at all.

## Decision

**1. Every pcurve comes from the surface's own projection**
(`Surface::project`), exact where a closed form exists and fitted
otherwise, unwrapped along `t` in each periodic direction. A tracer's
exact (u, v) is not the pcurve of the fitted 3D curve an edge carries
(1000 random torus sections: the projection adds at most 0.82 of a
tolerance, the branch's own (u, v) would be off by up to sixteen), a block
of an edge a later boolean cuts has no branch to ask, and neither has a
reader's edge. `MeetCurve` does not change.

**2. *Through* a singular point is a distance, and one band decides it
everywhere:** `PCURVE_SINGULAR_BAND`, a quarter of the linear tolerance.
`pcurve_on` refuses a range with the point inside it
(`GeomError::ThroughSingularity`, naming the parameter) and fits a range
that *ends* there, the pcurve ending on the point's own `v` with the `u`
the curve arrives with, read from its tangent. A range may end on the one
point **twice** — a closed curve through a pole, cut there — and each end
is read on its own side of the range. The exact arms are outside the
rule: a ruling is a line of one `u` through the apex onto the other
nappe, a meridian a line of one `u` over a pole, placed at the sphere's
own latitudes for the range asked (a sphere's `v` is no period of
anything, and nothing downstream could put a whole turn of it back).

**3. The pave model puts a vertex there, the operand's own.** A crossing
curve of a face pair within the band of a singular vertex of either face,
on the other face, is paved by a section vertex over that operand vertex —
the one the seam made by piercing the other face at its end, where it did,
and one of `VertexSource::Singular` where the seam only touches it or the
face has none. So no block has the point inside it, and
`ThroughSingularity` is never met from a boolean. On a cone *through* also
means **straight through**: a curve on a cone through its apex leaves it
along a ruling, and one that only comes within the band turns back there
with its tangent along `r₁ − r₂`, perpendicular to the axis whatever the
plane, which carrying the pcurve onto the point would misread. A pole is a
smooth point of the surface and needs no such test.

**4. The degenerate edge is paved where a section arrives.** The vertex is
a whole line of (u, v); a section edge ending on it arrives at one `u`,
and the face's arrangement needs a node there. The degenerate edge is
paved by that vertex at the parameter its own pcurve has that (u, v) at —
once per arrival, a circle through a pole twice, half a turn apart — and
is cut there like any edge: its pieces are degenerate edges on the same
vertex, and a result face keeps the pieces on its side of the section or
none. An arrival within the angular tolerance of an end of the edge is
the corner of the (u, v) box it already ends on.

**5. Beside the point is refused by name:**
`Reason::BesideSingularity`, the two faces and the vertex, before anything
is fitted, from the band out to `SINGULAR_CLEARANCE` (four) polygon
segments of the face's box diagonal — `diagonal × 4 / MAX_SEGMENTS_PER_PIECE`.
The reason is the reader, not the fit: a face's (u, v) polygons cut a
piece into at most 65536 segments *evenly in its parameter*, the bound on
their deviation from a pcurve that turns `π` over `d` exceeds the face
itself, and the split finds no interior point for the region beside it —
after tens of seconds. Measured on a ball of radius 2, the same for an
exact circle and for a traced loop: built from a miss of `1e-4`, not at
`1e-5`, where one segment is `1e-4`. By an apex the same refusal covers
what else lives there: the hyperbola that doubles back within a tolerance
of itself (the checker's E8), and the two lines the intersector gives, in
length, for a plane a hair off the apex while the seam pierces that plane
several tolerances down its ruling — a hit of either face's edge on the
other beside the vertex and not on it says the true section misses the
point, whatever the curves say.

## Consequences

- `boolean/cone-apex-slice-cut`, `boolean/ball-pole-slice-cut` (through
  the pole at a general turn, along the seam's meridian, and in the
  seam's plane) and `boolean/ball-pole-drill-cut` pass every corpus stage;
  a plane through an apex or a pole at eight thousand random poses cuts
  additively, checker-green at `Full`.
- A miss between a quarter tolerance and four polygon segments is a typed
  refusal where it was a hang, a `SplitFault`, or — by an apex in a release
  build — an invalid body. `regression/ball-beside-pole-slice-cut` holds
  the desired body. What lifts it is the polygon, not the pave model: a
  piece cut by each span's own curvature, and a chord converted to length
  where the pcurve is rather than once per face (`docs/BACKLOG.md`). The
  refused zone is `6e-5` of a face's diagonal about the point, so
  a random pose meets it rarely and not never; step 8's property takes it
  as a designed outcome, as it does a tangent contact.
- A section leaving a pole within `√(2 tol / R)` of the seam's meridian is
  the seam within the tolerance over a stretch beside the pole: one touch,
  no crossing, `Fault::Seam`. `regression/pole-slice-beside-seam-cut`; a
  feature a tolerance apart, with C3's last plan. Exactly along the seam is
  built.
- `VertexSource::Singular`, `Reason::BesideSingularity`: new variants of
  public enums.
- The checker's L2 reads a junction in (u, v), where a step of `u` at a
  pole is no length. Nothing here asks it to change; the seam regression's
  description records that a length-measured `place` alone moves the
  refusal there.
