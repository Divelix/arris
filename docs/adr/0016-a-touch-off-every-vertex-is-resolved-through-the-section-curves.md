# ADR-0016 — A touch off every vertex is resolved through the section curves; the intersector's touch stays a verdict on depth

- Status: accepted (2026-09-18)
- Plan: `seam-parametrisation-faults` step 4
- Follows: ADR-0004 (the pave model: hits, section vertices, paves)

## Context

`intersect_curve_surface` calls a hit `tangent` where the distance along
the curve has an extremum within `tol.linear` of zero, and reports the
crossings that extremum would split into as that one touch. The pave
model (ADR-0004) gives a touch no vertex of its own: one that lands on a
vertex the hits and crossings made joins it, and any other joins nothing.

That is a verdict on *depth*, and depth is quadratic in length. A line
`h` inside a cylinder of radius `R` crosses it `2√(2Rh)` apart: at
`R = 1` and the default tolerance of 1e-7, a touch can stand for two
crossings 9e-4 apart. Where the edge's own face is transversal to the
touched one this costs nothing — the section curve between the two
crossings stays within `h / sin θ` of the edge, inside its band, and no
block of it is kept. Where the two surfaces are *tangent to each other*
it is wrong: `sin θ → 0`, and the section curves leave the touch at any
distance.

Two equal cylinders with crossing axes are tangent at the two crossing
vertices of their ellipses, `(0, ±R, 0)`. Turn one about its own axis —
the identity on the solid — until its seam runs `R sin δ` beside a
crossing vertex: the seam is `R(1 − cos δ)` inside the other wall, under
the tolerance up to `δ` = 0.0256°, so it is one touch, `R sin δ` from
the vertex and joining nothing; both ellipses cross the seam, neither
finds a pave there, and the block over the seam is `Fault::Seam`. The
fuse failed so for every turn from about 6e-6° to 0.0256° either side
of ±90° (`regression/seam-beside-crossing-fuse`, found by
`crossing_cylinders_obey_every_identity` at 1000 cases).

## Decision

**The intersector's touch is left as it is, and the pave resolves a touch
that lands on no vertex through the section curves.** For such a touch of
edge `e` on face `f`, `e` is intersected (`intersect_curves`) with every
section curve of every `Transversal` pair of `f` and a face of `e`; each
crossing that is in `e`'s range, on `f`, not a touch itself and not
already a hit of `e` on `f` is an `EdgeFaceHit` like any other — merged
into a section vertex, paving `e` and the curves through it. The touch
stays in the list, with no vertex, as before. An edge `Coincident` with
the section curve is that curve's block and resolves nothing; a curve
pair with no closed form is the operation's `Unsupported`, naming `e`
and `f`.

The reason is conditioning. `e` against the surface of `f` near a
tangency is a square root: a depth known to `η` places the crossings to
`√(2Rη)`. `e` against the curve `f ∩ g`, `e` an edge of `g`, is two
curves of one surface crossing at an angle — at a crossing vertex of two
ellipses, 45° — and is exact to rounding: the resolved hits of the test
lie on the other wall to 1e-14.

## Consequences

- The seam's position no longer decides whether the pave can be built:
  at every turn the old band covered, `interferences` returns the pave
  of a generic turn — the seam cut twice, eight section edges.
- A touch that lands on a vertex is untouched, so every blessed dump and
  every designed tangency (`boolean/seam-through-crossing-common`,
  `boolean/tee-fuse`) is what it was.
- Resolved hits are merged after every other vertex, so a body whose seam
  is inside the band numbers its section vertices in a different order
  from one outside it. Deterministic for a given input; not the same
  across the band's edge, which no rule promised.
- Two vertices between one and two tolerances apart are still a fault
  (`Fault::Split`): 5.8e-6° to 1.1e-5° past ±90° here,
  `regression/seam-a-tolerance-from-crossing-fuse`, a backlog line. It
  failed before this decision as `Fault::Seam` and is about merging, not
  about touches.
- The piece the two crossings cut from the turned wall lies within the
  tolerance of the other wall throughout, and classifying it is the
  plan's step 5.

## Alternatives considered

- **Make the touch a verdict on reach: a touch only while the crossings
  it absorbs are within `tol.linear` of it.** Tried first, in every arm
  of `intersect_curve_surface`. It fixes the band and breaks the designed
  tangency: five of `arris-geom`'s intersector properties build an exact
  touch in a random pose at coordinates near 100, where rounding leaves
  the curve about 1e-14 inside the surface, and came back as two
  crossings 3.7e-7 to 4e-7 apart (`t` = ±1.83e-7, ±1.97e-7) — above the
  tolerance, so not a touch by reach either. No threshold separates a
  designed tangency with rounding from a seam 2e-7 beside a vertex: they
  are the same line against the same cylinder. The depth verdict is the
  well-conditioned one, and the information that tells the two apart is
  the section curve, which the intersector does not have.
- **Resolve every touch, not only one off every vertex.** Where a touch
  lands on a vertex the crossings are that vertex, so this would make the
  same vertices from a hit instead of a section crossing, earlier in the
  merge: the designed tangencies renumbered for no change in geometry.
- **Pave every section curve against every edge of its two faces, and
  drop the edge–face hits.** The well-conditioned intersection
  everywhere, and a different pave model from ADR-0004's: a hit also
  carries its landing on the other face's boundary, which the vertex
  merge reads. Nothing else in the corpus asks for it.
- **Read the chord within the tolerance as a piece of the edge lying in
  the face.** The chord is not in the face in any sense the result can
  use: the section curves cross it at its two ends and nowhere between,
  so an image of it there bounds nothing. Open CASCADE's counts in the
  band, 9/15 where a generic turn gives 11/17, look like this reading —
  its modules were not read for this — and its body there is the wrong
  one of ADR-0015.
