# ADR-0008 — Coaxial surfaces of revolution meet through their meridians

- Status: accepted (2026-09-15)
- Plan: `quadric-checker-arms` step 1 (the arm, `SurfaceIntersection::
  Points`, S5's point rule and the boolean's quadric guard); the plane
  through the axis, the line arms and the checker's dispatch over them
  land in its later steps under this decision

## Context

C2's intersector and checker lines (`docs/ROADMAP.md` §C2) ask for cone,
sphere and torus pairs in exactly the positions a blend (ADR-0007) and
M5's revolves put them: a plane perpendicular to a quadric's axis, a
cylinder coaxial with one, a sphere on a cylinder's axis, plane–sphere,
so that S5 and B1 can hold a revolved frustum, barrel or ring and a
hole-rim torus or a corner sphere to `Full` with nothing unchecked. The
general pairs — a plane oblique to a cone, two tori on different axes —
are C3's, and their curves are not conics (the quadric-curve `⚠ OPEN` in
`docs/DATA-MODEL.md` §Curves). Every pair C2 needs shares an axis, and
every pair that shares an axis meets in circles about it or in points on
it.

Read in the reference tree: Open CASCADE's `IntAna_QuadQuadGeo`
(`ModelingData/TKGeomBase/IntAna`), whose `Perform` overloads on the
cylinder–cone, cylinder–sphere, cone–cone, sphere–cone, sphere–sphere,
plane–torus, cylinder–torus, cone–torus and sphere–torus pairs are one
closed form each for the coaxial case: the circles of a cylinder on a
cone at `R / tan α` either side of the apex, of a sphere on a cylinder at
`±√(R² − r²)` from the centre, of two spheres on their radical plane, of
a sphere on a torus by the two circles of the tube's section, and
`IntAna_Point` for two spheres touching, a plane on a sphere at a pole and
two cones closing on one apex. Each is a distinct branch with its own
tolerance choices — the plane–sphere touch is decided at machine epsilon,
the cylinder–sphere axis test is exact — which the oracle fixture
records. `IntAna_ResultType` carries a `PointAndCircle` for the mixed
case.

## Decision

**One arm replaces the table.** Every pair with a cone, a sphere or a
torus in it, when the two share an axis, is decided in the plane through
that axis: each surface is its *meridian sections* there — a plane
perpendicular to the axis a line, a cylinder two lines, a cone the two
lines through its apex, a sphere a circle on the axis, a torus two
circles either side of it — and the sections meet by the closed forms of
two lines, a line and a circle, and two circles. A meeting off the axis
sweeps a circle about it; one on the axis is a point. The section sets
are symmetric across the axis and the mirror pairs meet in the mirror
points bit for bit, so only the half-plane `ρ ≥ 0` is read, and no
circle is found twice.

**A sphere is taken about the line to the other operand's axis, or
along the plane's normal.** Any line through its centre is an axis of
revolution, so a sphere is coaxial with every cylinder, cone or torus
whose axis passes within `tol.linear` of its centre, with every plane,
and with every sphere — the one arm covers plane–sphere and
sphere–sphere in general position, the two pairs C2's corner sphere
needs, and no branch is written three times for the sphere's free axis.

**`tol.linear` carries over exactly.** A distance in the plane through
the axis is the 3D distance between points at one angle, and a point's
distance to a surface of revolution is its distance to the full symmetric
section in that plane. So tangency and coincidence are decided in 2D at
`tol.linear` — a line within `tol.linear` of a circle's radius from its
centre, two circles within `tol.linear` of the sum or difference of their
radii, two lines parallel within `tol.angular` and apart within
`tol.linear` — and mean exactly what they mean in 3D.

**A meeting on the axis becomes a point.** Within `tol.linear` of the
axis a meeting sweeps no circle: it is one of `SurfaceIntersection::
Points`, the enum's new variant — a plane tangent to a sphere at its
pole, two spheres touching, a plane perpendicular to a cone through its
apex, two cones closing on one apex. The points come ascending along the
axis. S5 holds them to its rule point by point, with a vertex both faces
reach excusing a point as an edge they share does, since two cones on one
apex and a blend sphere on a plane share a vertex but no edge.

**A result that mixes kinds is refused.** A circle beside a point on the
axis — a sphere centred on a cone's axis through its apex — or a crossing
beside a touch is `GeomError::Unsupported` naming the pair; the type
that would carry it is decided with C3's ADR on the general pairs, not
grown here for a position no revolve or blend of C2 makes.

**The boolean keeps refusing what it refused.** A face pair or an
edge–face pair with a cone, a sphere or a torus in it is refused in the
pave model before any intersector is asked, as the same
`OpError::Unsupported` naming the pair; the arm widens no boolean
silently. Booleans with quadric operand faces are C3's, with a corpus of
their own.

## Consequences

- One function, one tolerance story and one set of properties (each
  circle on both surfaces and a genuine crossing of the sampled
  meridians, no sampled crossing missed, constructed touches `Tangent`,
  constructed apexes and poles `Points`) cover the fifteen pairs the
  table would have had, and every pair the table would have had to list
  is reached through the same 2D code.
- `SurfaceIntersection` gains `Points(Vec<Point3>)`, a breaking change:
  every exhaustive match over it — S5, the boolean's display and pave,
  a blend's end section — gains an arm.
- The frame of a circle is the first operand's whose frame carries the
  axis, so the seam is shared as plane–cylinder shares it; a plane on a
  sphere takes the plane's frame at the sphere's centre, two spheres the
  line of centres (`docs/DATA-MODEL.md` §Curves has the rule).
- The oracle answers the coaxial pairs through its own branches, and its
  branches' tolerances are not this arm's: a pole touch it calls `empty`,
  a turned sphere on a cylinder `unsolved`. The fixture records both and
  the oracle test holds Arris to the surfaces where the oracle is silent.
- A plane *through* the axis — a cone's two rulings, a torus's two
  meridian circles — is not a meridian meeting and stays `Unsupported`
  until the plan's next step; a line against the quadrics, and the
  checker's classifier over it, is the step after.

## Alternatives considered

- **One closed form per pair**, the reference tree's table: fifteen
  arms, the sphere's free axis handled three times, each with its own
  tolerance decisions to prove and its own oracle quirks to record. The
  meridian arm is the same mathematics factored once.
- **A general quadric–quadric intersector now** (`IntAna_IntQuadQuad`,
  the quartic space curves): decides the quadric-curve `⚠ OPEN` for every
  pair at once, which no C2 probe forces, and would carry a curve type no
  blend or revolve here produces. It stays C3's, with its ADR.
- **Growing the result type for a mixed meeting** (`PointAndCircle`): a
  variant for a position no C2 operation makes, decided before the
  general pairs that would populate it. A refusal now, the type with C3.
