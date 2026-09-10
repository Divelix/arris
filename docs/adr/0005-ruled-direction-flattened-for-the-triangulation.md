# ADR-0005 — A ruled direction is flattened before the triangulation

- Status: accepted (2026-09-10)
- Plan: `m4-booleans` step 14
- Amends: ADR-0003, "The triangulation is taken in a domain scaled to the
  surface" and "a plane, a cylinder and a cone are ruled and take none"

## Context

ADR-0003 scales a face's (u, v) by the surface's mean speeds before the
CDT, so that Delaunay's empty-circle criterion measures distance on the
surface, and gives a ruled direction no interior points, because the
chord bound there is carried by the loops' own samples: every edge is
sampled so that no step travels more than the surface's `chord_steps` in
either parameter.

M4 produced the first face that rule fails on. `boolean/oblique-hole` is
a plate with a hole drilled at 30°; the hole's wall is a cylinder face
whose (u, v) region is the strip between two ellipse sections — two
sinusoids `v = c ± r tan α · cos u`, the same shape a turn apart in `v`.
The chains run *oblique* to the ruling. In a domain scaled to the
surface, Delaunay joins each boundary point to the point nearest it on
the other chain, and on a sheared strip the nearest one is offset along
the ruling by roughly `s·H / (1 + s²)`, `s` the shear and `H` the strip's
thickness: 1.39 rad of the cylinder where the boundary is sampled every
0.048. Every such triangle is a chord across the hole. The mesh stayed
closed and its triangles stayed well shaped — it was the *right*
Delaunay triangulation of that point set — and the solid's mesh volume
came out 5·10⁻³ off where the inscribed prism bounds it at 4·10⁻⁴.

The rule as ADR-0003 wrote it — "a plane, a cylinder and a cone are ruled
and take none" — holds only while a region's boundary runs along the
ruling, which is every face M1 through M3 could build (a wall between two
rims, a cone between two circles) and stops being true the moment a
boolean cuts a face obliquely.

Read again: Open CASCADE's `BRepMesh_Delaun` and `BRepMesh_GeomTool`
insert internal nodes on the surface's iso-lines and so never rely on the
boundary alone; `BRepMesh_Deflection` sizes them.

## Decision

**A direction the surface is ruled along is flattened before the
triangulation.** Where exactly one parameter carries a finite
`chord_steps` — a cylinder, a cone — the other one's scale is set so the
region's whole extent along it is `RULED_RIBBON = 1/8` of one chord step
in the curved parameter. The domain becomes a thin ribbon, the
empty-circle criterion is left to the curved parameter alone, and the
triangulation is the one a ruled face wants: one quad per step of its
boundary, whatever the shear. It costs no interior points and no
triangles — `oblique-hole`'s wall meshes with the same 262 triangles it
did before, joining the boundary column by column instead of across the
hole, and the solid's mesh volume comes to 1·10⁻⁵ of the closed form.

The bound this rests on: a Delaunay triangle's circumcircle holds no
vertex; a circle that covers a ribbon of thickness `e` over a span `w` of
the curved parameter holds every boundary sample in that span, so `w` is
under the boundary's own step `d`; a circle of radius `r` centred in the
ribbon covers `2√(r² − e²)` of it, so `r² < d²/4 + e²`, and the triangle
the circle contains travels less than `√(d² + 4e²)` — at `e = d/8`, three
hundredths over `d`, against a deviation that goes as the square. A plane
is ruled both ways, takes no chord step at all and is left alone, since
every triangulation of a planar region is exact; a sphere, a torus and a
NURBS surface curve both ways and keep the isometric domain their
interior lattice is sized in.

**The guarantee is stated in the curved parameter and tested there.** No
triangle of a face on a cylinder travels more than one chord step of the
turn — asserted on `boolean/oblique-hole` and at random radii, tilts and
chords in `crates/arris-mesh/tests/tessellate.rs`, beside the volume the
inscribed prism bounds.

## Consequences

- The scaled domain is no longer isometric to the surface on a cylinder
  or a cone. Nothing outside `tessellate` sees it — the CDT returns
  indices, and a face's picture in (u, v) is still drawn in the
  parameters its pcurves are written in (`render_domain`).
- Triangles on a ruled face are long and thin along the ruling, which is
  what a ruled face is: their deviation is exactly the inscribed prism's,
  and a consumer that wants square triangles for shading asks for a
  smaller chord.
- A ruled face still takes no interior lattice, so a cylinder's mesh
  keeps its cost: this is the cheap half of the two fixes the failure
  admitted.
- `RULED_RIBBON` is a shape factor with the bound above behind it, not a
  tolerance; it never reaches a coordinate or a model value.

## Alternatives considered

- **Interior points on the ruled direction**, the lattice ADR-0003
  already describes, spaced by what keeps the shear's offset under a
  step: `d(1 + s²)/s`. Correct, in the existing idiom, and it costs
  ~35 rows for `oblique-hole` — 4 600 interior points and 9 000 triangles
  for one bolt hole's wall, where 262 triangles describe it exactly. A
  square lattice in the surface's own lengths, the simplest version of
  it, costs 80 rows and would charge every plain cylinder wall the same.
- **A strip-aware triangulation**: sweep the region in the curved
  parameter and emit a quad per column. It is the mesh this ADR
  produces, arrived at by a second algorithm to write and prove, and it
  needs vertices on the boundary at each column — which the shared edge
  discretisation forbids, since a vertex added to a face's boundary is
  not on its neighbour's.
- **Restating the guarantee** as "within the chord where the boundary
  runs along the ruling": that is not a guarantee a consumer can use, and
  the failure is not rare — every hole drilled at an angle has it.
- **Reading the deviation per triangle and splitting**: adaptive
  refinement, which ADR-0003 rejected and cycle 2 may revisit.
