# 02 — Data Model

What lives in a `Model`: the geometry enums and their parametrisations, the
topology entities and how orientation composes over them, what a pcurve and
a tolerance mean, the invariants the checker enforces, the provenance record
an operation returns, and the native format. The crate boundaries and the
operation contract are in [01-architecture](01-architecture.md).

## Conventions

- **Units** are the consumer's. Arris carries no unit; `Precision` (below)
  is what makes a model's numbers meaningful.
- **`f64`** everywhere. Angles in radians. Parameters are `f64`; parameter
  ranges are `Interval` from `arris-math`.
- **Points and vectors** are `nalgebra`'s by alias — `Point3`, `Vec3`,
  `UnitVec3`, `Point2`, `Vec2`, `UnitVec2` — with `nalgebra` re-exported
  from `arris-math` (ADR-0001). A `UnitVec3` is unit by construction and
  that is the only invariant it carries.
- **Frames** are right-handed: `Frame { origin, x, y, z }` with `x`, `y`, `z`
  orthonormal `UnitVec3` and `z = x × y`, built only through validating
  constructors (`Frame::new(origin, z, x_hint)`, `Frame::from_z`, which
  picks `x` by the rule of Open CASCADE's `gp_Ax3(P, N)` so an axis-built
  cylinder seams where the oracle's does, `Frame::from_rotation`). Every
  analytic surface and curve is placed by a frame, so a transform is a
  frame change and nothing else: an **`Isometry`** (a rotation then a
  translation) moves geometry through `Frame::transformed`. A **`Frame2`**
  is the (u, v)-plane analogue and may be of either handedness, which is
  how a pcurve records its direction of traversal.
- **Tolerances** reach an algorithm as `Tolerance { linear, angular }`,
  derived from the model by `Precision::tolerance()` or from an entity's
  own tolerance by the operation that owns it (§Tolerances). Exact
  predicates (`arris_math::predicates::{orient2d, incircle}`) return a
  `Sign` and take no tolerance.
- **Root finding** is `arris_math::roots`: `quadratic`, `cubic` and
  `quartic` return the real roots ascending with multiplicity, a multiple
  root reported once and decided to rounding (`POLYNOMIAL_ROUNDING`, a
  statement about `f64`, never a tolerance); `newton_in_interval` is the
  Newton that never leaves its bracket, for every iteration in the kernel
  that has one.
- **Orientation** is the two-valued `Orientation::{Forward, Reversed}` and
  composes by XOR: `Forward ∘ o = o`, `Reversed ∘ o = !o`.
- **Parametrisations match Open CASCADE's `Geom` classes** for the analytic
  types (read from the reference tree, `SEED.md` §8), so STEP round-trips
  without re-parametrising and the oracle's pcurves agree with ours.
- **Ids are typed and generational**: `VertexId`, `EdgeId`, `FaceId`,
  `ShellId`, `BodyId` for topology; `CurveId`, `SurfaceId`, `Curve2Id` for
  geometry; each `{ index: u32, generation: u32 }`, ordered by `(index,
  generation)`. `EntityId` is the enum over the five topological ids (what
  a `Shape` wraps) and `GeometryId` over the three geometric ones (what an
  entity references); each orders by kind, then id. In text dumps an id is
  its kind letter and index, `f3`, with `g<n>` appended for a generation
  above zero. Loops and coedges are not entities (§Topology).

## Geometry

Geometry is stored in the arena once and referenced by id; two faces may
share a `SurfaceId` (the two halves of a split face do). A geometry value is
never modified.

### Surfaces

```rust
pub enum Surface {
    Plane    { frame: Frame },
    Cylinder { frame: Frame, radius: f64 },
    Cone     { frame: Frame, radius: f64, half_angle: f64 },
    Sphere   { frame: Frame, radius: f64 },
    Torus    { frame: Frame, major_radius: f64, minor_radius: f64 },
    Nurbs    (NurbsSurface),
}
```

With `O, X, Y, Z` the frame and `c = cos`, `s = sin`:

| Variant | `P(u, v)` | Domain | Periodic | Seam / singularity |
|---|---|---|---|---|
| Plane | `O + u·X + v·Y` | ℝ² | — | none. Normal `Z` |
| Cylinder | `O + R(c u·X + s u·Y) + v·Z` | u ∈ [0, 2π), v ∈ ℝ | u, period 2π | seam at u = 0, the line through `O + R·X` along `Z` |
| Cone | `O + (R + v·s α)(c u·X + s u·Y) + v·c α·Z` | u ∈ [0, 2π), v ∈ ℝ | u | seam at u = 0; apex at v = −R / s α, a degenerate edge. `α` ∈ (0, π/2) is the half-angle; `R` the radius at v = 0 |
| Sphere | `O + R c v (c u·X + s u·Y) + R s v·Z` | u ∈ [0, 2π), v ∈ [−π/2, π/2] | u | seam at u = 0; poles at v = ±π/2, degenerate edges |
| Torus | `O + (R + r c v)(c u·X + s u·Y) + r s v·Z` | u, v ∈ [0, 2π) | u and v | seams at u = 0 and v = 0; `R > r` in cycle 1 (no self-intersecting tori until an operation needs them) |
| Nurbs | Piegl & Tiller, rational; clamped or not (§NURBS) | knot range | either, where the knots and net wrap | as the knots say |

The surface normal is `∂P/∂u × ∂P/∂v`, normalised. For the analytic types
that is: plane `Z`; cylinder, cone and sphere radially outward; torus
outward from the tube. It is the *surface's* normal; a face's normal is the
surface's composed with the face use's orientation (§Orientation).

A surface's parametric domain is unbounded where the table says ℝ; a face
trims it with loops. Periodic directions are stored as a period, and a
pcurve on a periodic surface may run outside `[0, 2π)` — a loop that crosses
the seam is written with a seam edge (§Seams), not by unwrapping.

`Surface::eval(u, v)` returns `SurfaceEval { point, du, dv, duu, duv, dvv }`
for every finite parameter, inside the domain or not (a periodic parameter
wraps); `normal(u, v)` is `None` where the parametrisation is singular —
the apex, the poles, a zero radius — decided to rounding
(`arris_math::is_negligible`), never a direction made of noise. `domain()`
gives the closed fundamental interval of a periodic direction, `[0, 2π]`,
and `Interval::REAL` where the table says ℝ; `period()` the period per
direction.

`Surface::project(p)` returns the nearest point of the whole parametric
surface (both nappes of a cone) as `SurfaceProjection { uv, point,
distance }` by the variant's closed form, with a periodic `u` in `[0, 2π)`
and the sphere's `v` in `[−π/2, π/2]`. Where the nearest point or its
parameter is not unique — the axis of a cylinder, cone or torus, the plane
through a cone's apex, a sphere's centre, a torus's centre circle — the
result is `GeomError::Ambiguous` naming the locus, decided to rounding and
never resolved by a silent choice of parameter. A point on a sphere's axis
off its centre projects to the pole with `u = 0`: the point is unique,
only the degenerate parameter is not.

`SurfaceKind` is the fieldless twin of the enum, used in errors and
dispatch tables. `Surface::frame()` is the placing frame of an analytic
variant and `None` for `Nurbs`, which is placed by its control points;
`project` onto a `Nurbs` is `GeomError::Unsupported` in cycle 1 (there is
no closed form, and no operation asks for it yet). `Surface` and `Curve`
are `Clone`, not `Copy`: the NURBS variants own their knots and control
points.

### Curves

```rust
pub enum Curve {
    Line    { origin: Point3, direction: UnitVec3 },
    Circle  { frame: Frame, radius: f64 },
    Ellipse { frame: Frame, major_radius: f64, minor_radius: f64 },
    Nurbs   (NurbsCurve),
}
```

| Variant | `P(t)` | Domain | Periodic |
|---|---|---|---|
| Line | `O + t·D` | ℝ | — |
| Circle | `O + R(c t·X + s t·Y)` | [0, 2π) | 2π |
| Ellipse | `O + a c t·X + b s t·Y`, `a ≥ b` | [0, 2π) | 2π |
| Nurbs | rational; clamped or not (§NURBS) | knot range | where the knots and control points wrap |

The tangent is `dP/dt`, never normalised in the enum's own evaluation; a
line's parameter is arc length because `D` is unit. `Curve::eval(t)` returns
`CurveEval { point, d1, d2 }`; `domain()`, `period()` and `kind()`
(`CurveKind`) follow the table as for surfaces. `Curve::project(p)` returns
the nearest point as `CurveProjection { t, point, distance }`, a periodic
`t` in `[0, 2π)`. A line and a circle project by closed form; an ellipse
through the quartic in `tan(t/2)` of `arris_math::roots`, every candidate
polished so the residual `(p − C(t)) · C′(t)` is zero to rounding. A point
on a circle's axis, at an ellipse's centre, or on the open segment of an
ellipse's major axis inside its evolute (two mirror-image nearest points)
is `GeomError::Ambiguous` naming the locus. A NURBS curve projects by
sampling every span (`2p + 2` parameters each) and bracketed Newton on
the derivative of the squared distance around the best sample, across
the seam of a closed or periodic curve: the nearest *local* minimum from
that sample, never `Ambiguous` and never a guarantee against a nearer
point the sampling missed.

`intersect_surfaces(a, b, tol)` returns `SurfaceIntersection::{Empty,
Coincident, Transversal(Vec<Curve>), Tangent(Vec<Curve>)}` for the pairs
with a closed form and `GeomError::Unsupported` naming the pair for every
other — in cycle 1, plane–plane (a line) and plane–cylinder (a circle, an
ellipse, two rulings, one tangent ruling, or nothing); every pair with a
`Nurbs` operand is an explicit `Unsupported` arm. `tol.angular`
decides parallel and perpendicular, `tol.linear` decides coincident,
tangent and empty. An intersection curve's frame is Arris's own
deterministic choice, matching Open CASCADE only where the *surface's*
parametrisation is concerned: a circle on a cylinder takes the cylinder's
`X` so the seam is shared; an ellipse's `Z` is the plane's normal and its
`X` the major axis in the direction of increasing `v`; a line's origin is
its point nearest the cylinder's origin (plane–cylinder) or the first
plane's origin (plane–plane), and a ruling's direction is the cylinder's
`Z`. Swapping the operands gives the same point sets, up to a line's
orientation.

`intersect_curve_surface(c, s, tol)` returns `CurveSurfaceIntersection::{
Points(Vec<CurveSurfaceHit>), Coincident}` for the pairs with a closed form
— in cycle 1, line–plane, line–cylinder, circle–plane and circle–cylinder
— and `GeomError::Unsupported` naming the pair for every other. A hit is
`CurveSurfaceHit { t, uv, point, tangent }`: `point` is the curve's point
at `t`, `uv` the surface's own projection of it, hits ascending by `t`
with a periodic `t` in `[0, 2π)`. A line is parallel to a plane or to a
cylinder's axis within `tol.angular`, and then coincident or clear within
`tol.linear`; a circle is `Coincident` when it lies within `tol.linear`
of the surface everywhere, which the extrema of its distance decide. A
hit is `tangent` where the distance along the curve has an extremum
within `tol.linear` of zero — the two crossings such an extremum would
split into are one touch — so a transversal hit is on both operands to
rounding and a tangent one within `tol.linear`. Circle–cylinder finds the
extrema of the radial distance through the quartic in `tan(t/2)` and the
crossings between them by bracketed Newton; the others are closed forms.

`⚠ OPEN:` the intersection curve of two cylinders (and of the other quadric
pairs whose curves are not conics) has an exact parametrisation that is not
a `Curve` variant. Either it becomes one (`Curve::QuadricSection`, exact,
with STEP export fitting a B-spline at write time) or the intersector fits
`Curve::Nurbs` to the edge's tolerance and the exact form is never stored.
`SEED.md` §10 lists this as the first kickoff question; it is decided by the
ADR that lands cylinder–cylinder intersection (cycle 2), and cycle 1's
plane–cylinder pairs produce only lines, circles and ellipses.

### Pcurves (`Curve2`)

```rust
pub enum Curve2 {
    Line    { origin: Point2, direction: UnitVec2 },
    Circle  { frame: Frame2, radius: f64 },
    Ellipse { frame: Frame2, major_radius: f64, minor_radius: f64 },
    Nurbs   (NurbsCurve2),
}
```

A pcurve is a curve in a surface's (u, v) plane, with the parametrisations
of the 3D table: `O + t·D`, `O + R(c t·X + s t·Y)`, `O + a c t·X + b s
t·Y`. A circle or an ellipse is placed by a `Frame2`, whose handedness is
the direction of traversal: right-handed is counter-clockwise in (u, v),
left-handed clockwise. That is what a `center`-and-radius circle could not
say, and a 3D circle shared by a cap and a wall *is* clockwise on the one
of the two planes whose normal opposes the circle's `Z` — the pcurve keeps
the edge's parameter and the frame records the turn, so the curve is never
reversed (§Orientation). `Curve2::eval(t)` returns `Curve2Eval { point,
d1, d2 }`; `domain()`, `period()` and `kind()` (`Curve2Kind`) follow the
table; `project(p)` returns `Curve2Projection { t, point, distance }` by
the closed forms of the 3D variants in the plane, `GeomError::AmbiguousUv`
at a circle's centre and on an ellipse's ambiguous loci, and by sampling
and bracketed Newton for a NURBS.

On a plane every analytic 3D curve has an analytic pcurve. On a cylinder, a
circle around the axis is a `Line` at constant v, a line along the axis is
a `Line` at constant u, and an oblique plane section (a 3D ellipse) is a
sinusoid in (u, v) — not a `Curve2` variant, so it is a `Nurbs` fitted to
the edge's tolerance. The rule: exact where a variant exists, fitted
otherwise, and in both cases the checker verifies the pcurve against the
3D curve (§Invariants E4).

**Fitting** is `fit_curve2(f, range, degree, deviation, tol)`: a global
least-squares B-spline approximation of `f: t ↦ (u, v)` at the *given*
parameter (*The NURBS Book* §9.4.1) — so the result is same-parameter by
construction and interpolates both ends — with the knot vector refined
where the caller's `deviation(t, fitted point)` exceeds `tol` (checked at
`4p + 4` parameters per span, accepted at half the tolerance so the
result meets it between the checks too). The deviation is the caller's
measure in the caller's units: for a pcurve, the 3D distance between the
surface at the fitted point and the true curve. Refinement is bounded by
`MAX_FIT_SPANS` (1024): beyond it the result is `FitError::Diverged`
(`GeomError::Fit`), never a loop.

### NURBS

`NurbsCurve`, `NurbsCurve2` and `NurbsSurface` follow *The NURBS Book*:
degree `p` in `1..=MAX_DEGREE` (25, Open CASCADE's bound, which also
sizes the evaluator's stack buffers so evaluation never allocates), a
non-decreasing knot vector of `n + p + 1` knots, `n` Cartesian control
points with their positive weights stored separately, de Boor evaluation
with derivatives to second order (the quotient rule over the homogeneous
sums), and knot insertion as the primitive edit (degree elevation is in
the backlog). The constructors validate and return
`GeomError::Degenerate` naming the fault: a knot value's multiplicity is
at most `p + 1`, and at most `p` strictly inside the domain
`[knots[p], knots[n]]`, which is non-empty and whose last span is not.
A knot vector need not be clamped. A curve or a surface direction is
**periodic** exactly when its structure wraps: the knots repeat `n − p`
places on shifted by the domain's length and the last `p` control points
(rows, for a surface) repeat the first `p`, both to rounding; `period()`
is then the domain's length and evaluation wraps the parameter into the
domain first. Any other parameter outside the domain evaluates the
nearest polynomial piece. Knot insertion (`insert_knot(t, times)`)
leaves the image over the domain unchanged; on a periodic curve it
breaks the wrap the knots implied, so the result's `period()` is `None`.

## Topology

### Entities

Five arena entity kinds. Loops and coedges live inside the face that owns
them, because nothing outside a face refers to them: provenance, iteration
and a consumer's topological references name vertices, edges and faces.
The entity structs below live in `arris_topo::entity`; the crate root
holds the *handles* of the same five names (`Body`, `Shell`, `Face`,
`Edge`, `Vertex`: id plus orientation, 01-architecture §The model), since
a consumer holds handles far more often than it reads an entity.

```rust
pub struct Vertex { point: Point3, tolerance: f64 }

pub struct Edge {
    geometry: EdgeGeometry,                 // Curve { curve: CurveId, range: Interval } | Degenerate
    start: VertexId, end: VertexId,         // equal on a closed or degenerate edge
    tolerance: f64,
}

pub struct Face {
    surface: SurfaceId,
    loops: Vec<Loop>,
    tolerance: f64,
}
pub struct Loop   { coedges: Vec<Coedge> }  // ordered, closed
pub struct Coedge { edge: EdgeId, orientation: Orientation, pcurve: Curve2Id }

pub struct Shell { faces: Vec<(FaceId, Orientation)> }

pub struct Body {
    kind: BodyKind,                          // Solid | Sheet | Wire | General
    shells: Vec<(ShellId, Orientation)>,
    free_edges: Vec<(EdgeId, Orientation)>,  // wire and general bodies
    free_vertices: Vec<VertexId>,            // general bodies
}
```

- A **vertex** is a point and a tolerance.
- An **edge** is a bounded piece of a 3D curve between two vertices, oriented
  by its curve's parameter direction. `range` is a sub-interval of the
  curve's domain; on a periodic curve it may cross the period (`[3π/2,
  5π/2]`). A **degenerate edge** has no 3D curve: both vertices are the same
  vertex at a surface singularity (a sphere's pole, a cone's apex) and it
  exists only to give the face's loop a pcurve across the singularity.
- A **face** is a surface trimmed by one or more loops. The face's natural
  normal is its surface's normal.
- A **loop** is a closed ring of coedges. A **coedge** is one use of an edge
  by one loop: the edge, the direction it is traversed in relative to the
  edge's own, and the pcurve for that use. A loop keeps the face's material
  on the *left* when walked in coedge order with the face's natural normal
  up — outer loops counter-clockwise in (u, v), holes clockwise. There is no
  outer/inner flag; that orientation rule and the winding it implies are the
  whole distinction, and on a periodic surface it is the winding number
  through the period that decides.
- A **shell** is a set of face uses. A shell of a solid body is closed and
  its effective face normals point out of the material.
- A **body** is what operations take and return. `Solid`: every shell
  closed, every edge used by exactly two coedges. `Sheet`: open shells
  allowed, every edge used by one or two coedges, a face's effective normal
  is the sheet's front. `Wire`: no faces, only free edges. `General`: any
  mix, including a face used by two shells (a face separating two regions
  of one body) and an edge used by more than two coedges.
  Non-manifold structure is thus representable from day one (`SEED.md`
  §9); cycle-1 operations produce and accept `Solid` only and return
  `OpError::Unsupported` for the rest.

### Orientation

A handle is an id and an orientation, and every reference from an entity to
a sub-entity carries one (a body's shell uses, a shell's face uses, a
coedge's edge use). Orientation composes by XOR along the path from the
handle down to the entity, and every rule in this document is stated in
terms of the *effective* orientation at the end of that path:

- effective face normal = surface normal, flipped if the composed
  orientation down to the face use is `Reversed`;
- effective edge direction = curve direction, flipped by the composed
  orientation down to the coedge;
- a loop of a `Reversed` face is walked backwards, which keeps the material
  on the left of the flipped normal — the convention survives composition.

Entities themselves are never oriented: a surface is never flipped to make
a face's normal point outward, a curve is never reversed to make a coedge
forward. A face used `Reversed` by a shell is the same face, in the same
arena slot, that another shell may use `Forward` from the other side.

### Seams and closed faces

A face on a periodic surface whose loop crosses the seam contains the seam
as an edge used twice by the same loop, once `Forward` and once `Reversed`,
with two pcurves that differ by the period in the periodic parameter (`u =
0` and `u = 2π` on a cylinder). A full cylinder wall is one face, one loop of
four coedges: bottom circle, seam up, top circle, seam down. The seam edge is
an ordinary edge with an ordinary 3D curve; only its two pcurves know it is
a seam. This is the representation truck lacks and every seam-crossing
algorithm quietly needs.

### Adjacency and iteration

The arena keeps derived indices, rebuilt incrementally on append because
entities are immutable: edge → coedges (face, loop index, coedge index),
vertex → edges, face → shells. `Model::faces(body)`, `edges(body)`,
`vertices(body)` iterate in a deterministic order — depth-first over the
body's shells, faces, loops and coedges in stored order, each entity once at
first visit. That order is the order tessellation numbers `FaceRange`s in
and the order provenance lists entities in.

## Tolerances

An entity's tolerance `t` says: the true geometry this entity stands for lies
within distance `t` of the stored geometry — a ball around a vertex's point,
a tube around an edge's curve, a slab around a face's surface. It is a
statement about *this* entity, not about the model, so two edges of one face
may carry different tolerances.

- **Ordering.** For every incidence, `vertex.tolerance ≥ edge.tolerance ≥
  face.tolerance`: an edge's tube contains its vertices' balls in the sense
  that the edge's ends are within the vertices' tolerances, and a face's slab
  is at least as tight as any edge in it. This is the Open CASCADE ordering
  and it is what makes "is this point on this edge" answerable with the
  edge's tolerance alone.
- **Growth.** An operation never emits an entity with a tolerance smaller
  than that of the input entity it was `Modified` from, and it raises a
  tolerance only for a reason it can name: an intersection whose curves
  agree only to `t`, a vertex merged from two points `t` apart. The record of
  why lives in the operation's tests, not in the entity.
- **`Precision`** is the model-wide configuration set at `Model::new`:
  `default_tolerance` (what primitives get), `min_tolerance` (the floor no
  entity goes below), `max_tolerance` (an operation that would exceed it
  returns `OpError::Tolerance`), `angular_tolerance` (radians, for
  parallel/tangent decisions), `parametric_tolerance` (how far a pcurve
  may deviate in (u, v), derived from `default_tolerance` and the surface's
  scale), and `check_samples` (how many parameters the checker samples
  along an edge). Default values are chosen for a model whose features are
  of order 1–1000 units.
- **No literals.** A tolerance in an algorithm is the entity's, or a field
  of `Precision`, or a named constant in `arris-math` with a comment. `1e-6`
  in an algorithm is a bug (`.agents/rules/kernel.md`).

Exact predicates (`robust`) decide combinatorial questions — which side of a
2D segment a point lies on, whether a triangle is oriented — on the stored
coordinates; tolerances decide whether two things are *the same*. The two
never mix: a predicate is never softened by a tolerance, and a tolerance
comparison never pretends to be exact.

## Invariants

The list `arris-check` enforces. Each item is a `Violation` variant carrying
the entity (and, where relevant, the parameter or the second entity) and has
a test that constructs the violation through the raw insert API and sees it
reported. The level says when it runs (01-architecture §The checker). The
list at least covers Open CASCADE's `BRepCheck` statuses (read in the
reference tree) mapped onto this representation.

**Model and references**

| # | Invariant | Level |
|---|---|---|
| M1 | Every id referenced by an entity of the body resolves in this model, with the stored generation | Fast |
| M2 | Every entity reachable from the body is reachable through a parent that lists it (no coedge names an edge whose face is not in the body's closure) | Fast |
| M3 | Every coordinate, parameter and tolerance is finite | Fast |

**Vertex**

| # | Invariant | Level |
|---|---|---|
| V1 | `Precision::min_tolerance ≤ tolerance ≤ max_tolerance` | Fast |
| V2 | For every incident edge, the edge's curve at the end of its range is within the vertex's tolerance of the vertex's point | Fast |
| V3 | For every face the vertex lies on (through any coedge), the surface at the pcurve's end is within the vertex's tolerance of the point | Fast |

**Edge**

| # | Invariant | Level |
|---|---|---|
| E1 | A non-degenerate edge has a curve and a non-empty range inside the curve's domain (crossing the period at most once) | Fast |
| E2 | `start`/`end` are the curve at the range's ends within the vertices' tolerances (V2 from the edge's side); a closed edge has `start == end` | Fast |
| E3 | Every edge in a body is used by at least one coedge, or is a free edge of a wire/general body | Fast |
| E4 | For every coedge, the surface evaluated along the pcurve is within the edge's tolerance of the 3D curve at the same parameter, at `Precision::check_samples` parameters including both ends — the pcurve and the curve share the edge's parameter (same-parameter, same-range, always) | Fast |
| E5 | `edge.tolerance ≥ face.tolerance` for every face it bounds; `≤ vertex.tolerance` of both vertices | Fast |
| E6 | A degenerate edge has `start == end`, no curve, and lies on a face whose surface is singular along its pcurve (its 3D image is one point within the vertex's tolerance) | Fast |
| E7 | A seam edge (used twice by one loop) has its two coedges in opposite orientation and pcurves that differ by exactly the surface's period in the periodic parameter | Fast |
| E8 | The edge does not self-intersect within its range | Full |

**Loop and face**

| # | Invariant | Level |
|---|---|---|
| L1 | A loop has at least one coedge and is closed: coedge *i*'s effective end vertex is coedge *i+1*'s effective start vertex, cyclically | Fast |
| L2 | The pcurves are continuous in (u, v) at every coedge junction within `parametric_tolerance`, except across a seam edge where they jump by the period | Fast |
| L3 | No edge is used twice in one loop except as a seam (E7); no edge is used by two loops of the same face except as a seam | Fast |
| L4 | Each loop's signed area in (u, v) is non-zero, and the loops of a face have exactly one outer loop (positive winding) per connected component of the face's domain, holes with negative winding inside it | Fast |
| L5 | The loops of a face do not intersect each other or themselves in (u, v) | Full |
| F1 | The face has a surface and at least one loop; every pcurve lies within the surface's non-periodic domain bounds | Fast |
| F2 | `face.tolerance ≥ Precision::min_tolerance` and ≤ every incident edge's | Fast |

**Shell and body**

| # | Invariant | Level |
|---|---|---|
| S1 | Every face use in a shell resolves and no face is used twice by one shell | Fast |
| S2 | In a `Solid` body every edge of the shell is used by exactly two coedges, with opposite effective orientation (the two faces agree on which side the material is); in a `Sheet` by one or two; in `General` by any number, with the orientations pairing up | Fast |
| S3 | A shell is connected through its edges | Fast |
| S4 | A shell of a `Solid` is closed: no edge with one coedge | Fast |
| S5 | The faces of a shell intersect only along their shared edges and vertices | Full |
| B1 | A `Solid` body has at least one shell; every shell is closed and oriented; exactly one shell is outer and the rest are voids inside it, each void's effective normals pointing into the void | Full |
| B2 | A `Solid` body encloses positive volume (Gauss over the faces) | Full |
| B3 | A `Wire` body has no shells; `free_edges` form chains (each vertex used by at most two free edges) — `General` bodies exempt | Fast |

**Euler–Poincaré** (`Full`, reported as one line, not a violation on its
own): for a `Solid`, `V − E + F − (L − F) − 2(S − G) = 0` with `L` the number
of loops, `S` the number of shells and `G` the genus computed from the
adjacency; the number is printed in every text dump and asserted in every
fixture.

## Provenance

Every operation returns a `Provenance`: which output entities came from
which input entities, and how. Three relations, in Open CASCADE's
`BRepTools_History` vocabulary (read in the reference tree), because they
are the three a parametric history needs:

```rust
pub enum Relation { Generated, Modified, Deleted }

pub struct Provenance {
    // (input entities, relation, output entity), stored sorted by input id
    generated: BTreeMap<Shape, Vec<Shape>>,      // input  → outputs generated from it
    generated_pair: BTreeMap<(Shape, Shape), Vec<Shape>>, // an intersection edge from two faces
    modified:  BTreeMap<Shape, Vec<Shape>>,      // input  → outputs that are pieces of it
    deleted:   BTreeSet<Shape>,
}
```

- **Generated**: the output is a new entity of a *different* kind or role
  built from the input — the wall of a hole from the tool's cylindrical
  face, an intersection edge from a pair of faces, the side faces of an
  extrude from the profile's edges, the cap faces from the profile face.
- **Modified**: the output is a trimmed, split or re-tolerated piece of the
  input, same kind — the box's top face with a circle cut out of it, each
  half of a face split by an intersection curve (one input, several
  outputs), a transformed face.
- **Deleted**: the input has no image in the output — the part of the tool
  inside the target, a face swallowed by a fuse.
- **Kept** is not recorded: an entity untouched by the operation keeps its
  id and is simply present in the output body. `Provenance::is_kept(input,
  &model, output_body)` is a query, not a relation.

Every entity of every input body is accounted for: it is kept, or it appears
in exactly one of the three relations (an entity can be both `Modified` into
pieces and have `Generated` children; it cannot be `Deleted` and anything
else). The ops tests assert that accounting on every fixture, and that the
relations are the same on every run.

Queries: `generated_from(input) -> &[Shape]`, `modified_from(input)`,
`is_deleted(input)`, `origins(output) -> Vec<(Relation, Shape)>` (the
inverse), and `Provenance::then(&self, &next) -> Provenance`, which composes
two records so that a chain of operations (eight cuts of a bolt pattern)
reports against the original inputs. Composition is associative and the
tests check it.

**Stability** is what the record is for. Rebuilding the same feature tree
with a changed parameter produces, for each output entity, the same
`origins` chain in terms of the *inputs' roles* (the third hole's tool face,
the top face of the base plate) — because the record is built inside the
algorithm from the entity ids it actually split, not recovered afterwards
by geometric matching. A consumer's persistent name is therefore a function
of the origins chain, and the roadmap's acceptance corpus asserts that
function is constant across parameter changes.

`⚠ OPEN:` how a consumer's persistent topological references map onto
provenance ids — 01-architecture §Facade.

## Native format

`arris-io::native` is `serde` of the `Model`: format version, `Precision`,
then every arena chunk in slot order with each entity's id as its integer
pair and its geometry ids as integers. Deterministic byte-for-byte for the
same model (`BTreeMap`s, fixed float formatting in the text encodings); a
model that round-trips through it dumps identically before and after, and
every fixture asserts so. The wire encoding is a `serde` choice per call
(`postcard` for size, JSON for diffs); the schema is the model. A version
bump is a design delta and comes with a migration or an explicit refusal.

The text dump (`arris-debug::dump_text`) is a different thing: a
human-readable, deterministic listing — entities in iteration order, ids,
effective orientations, surface and curve parameters at fixed precision,
tolerances, pcurves, the Euler line — that fixtures store and tests diff.
It has no reader and is never a format.

## Open questions

- `⚠ OPEN:` quadric–quadric intersection curves, exact variant or fitted
  NURBS (§Curves).
- `⚠ OPEN:` consumer references onto provenance (§Provenance,
  01-architecture §Facade).
