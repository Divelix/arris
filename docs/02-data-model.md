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
the apex, the poles, a zero radius, a NURBS point whose two derivatives
are parallel or vanish — decided to rounding (`arris_math::is_negligible`),
never a direction made of noise. `domain()`
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

`pcurve_on(curve, range, surface, tol)` builds the pcurve, exhaustively
over (curve, surface), and its image under the surface is the curve *at
the same parameter*. On a plane every 3D curve has an exact pcurve: a
line is a `Line`, a circle a `Circle` and an ellipse an `Ellipse` whose
`Frame2` is right-handed when the curve's `Z` is along the plane's normal
and left-handed when it opposes it, a NURBS a `Nurbs` with its control
points projected (an affine map, so knots and weights carry over). On a
cylinder, a circle around the axis is a `Line` at constant v whose `u`
starts at the offset of the circle's `X` from the cylinder's and runs in
the sense of the circle's `Z` against the cylinder's, a line along the
axis is a `Line` at constant u, and an oblique plane section (a 3D
ellipse), or a NURBS, is a sinusoid in (u, v) — not a `Curve2` variant, so
it is a `Nurbs` fitted by `fit_curve2` (below) over the cylinder's
projection of the curve with `u` unwrapped along `t`, so a seam crossing
stays continuous and `u` may leave `[0, 2π)`. The rule: exact where a
variant exists, fitted otherwise, and in both cases the checker verifies
the pcurve against the 3D curve (§Invariants E4). A curve farther than
`tol.linear` from the surface at any of `PCURVE_SAMPLES` parameters over
the range is `GeomError::NotOnSurface` naming the parameter and the
distance; cone, sphere, torus and NURBS surfaces are `Unsupported` arms
until revolve needs them (M5).

**The (u, v) toolkit** is what every algorithm that reasons about a
face's domain shares — the checker's loop, face and body rows,
tessellation, mass properties and classification — and it lives in
`arris-geom` below all of them (decided in M2). `region2`: a loop is a
sequence of `Piece { curve: &Curve2, range, reversed }` walked in order,
and `discretise(pieces, chord_tolerance)` is its `Polygon2` — each piece
sampled at the segment count its second derivative bounds the chord
deviation by (`|d2| h² / 8`; a line is one segment, a conic never fewer
than `MIN_SEGMENTS_PER_TURN` per turn, a NURBS never fewer than
`MIN_SEGMENTS_PER_SPAN` per knot span, and never more than
`MAX_SEGMENTS_PER_PIECE`, the deviation achieved reported by
`chord_deviation()`; `f64::INFINITY` asks for the minimum counts, enough
for a sign) with the pieces taken as written, so a seam-crossing loop's
`u` runs past the period and is never wrapped. Over the polygon:
`signed_area()` (shoelace), `winding_number(p)` by `orient2d` crossings
(zero outside, `±1` inside by the turn), `contains(p)` exactly on a
segment, `gaps()` between consecutive pieces (L2), and
`self_intersections()` / `intersections(&other)` over
`segments_intersect`, exact through `orient2d` with touching counted
(L5, S5). `integrate::region_integral(pieces, f)` is `∬ f du dv` over
the region by Green's theorem — `∮ G dv` with `G = ∫_{u₀}^{u} f ds` —
with Gauss–Legendre quadrature of `GAUSS_ORDER` points per interval, a
conic piece split at quarter turns and a NURBS at its knots, signed by
the loop's turn so holes subtract themselves: `f = |∂P/∂u × ∂P/∂v|` is
an area, `f = P · (∂P/∂u × ∂P/∂v) / 3` summed over a solid's faces with
their use orientation is Gauss's volume (B2, `measure`).

`project_to_plane(curve, plane)` is the orthogonal projection onto a plane
for a consumer's sketch (01-architecture §Facade): a point-set projection
whose parameter is the variant's own — a line stays a `Line`, a circle
becomes a `Circle` when parallel and an `Ellipse` otherwise (its
semi-axes the singular values of the projected axes, its parameter the
circle's shifted by a phase), an ellipse an `Ellipse`, a NURBS a `Nurbs`
with its control points projected. A projection that collapses to a
point or a segment is `Degenerate`.

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
    geometry: EdgeGeometry,                 // Curve { curve: CurveId, range: Interval } | Degenerate { range: Interval }
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

pub struct Shell { faces: Vec<Face> }        // the handle: FaceId + Orientation

pub struct Body {
    kind: BodyKind,                          // Solid | Sheet | Wire | General
    shells: Vec<Shell>,                      // handles
    free_edges: Vec<Edge>,                   // handles; wire and general bodies
    free_vertices: Vec<VertexId>,            // general bodies
}
```

A reference that carries an orientation — a shell's face use, a body's
shell or free-edge use — is stored as the handle of that kind, since a
handle *is* an id and an orientation. The fields are private: an entity is
built by its constructor (`Vertex::new`, `Edge::new`, …, which check
nothing) and read through getters, and once in the arena it is never
written. The arena appends it through the raw insert (`Model::raw()`,
test scaffolding that stores a dangling reference as given), the builder
(§Euler operators) or `import`.

- A **vertex** is a point and a tolerance.
- An **edge** is a bounded piece of a 3D curve between two vertices, oriented
  by its curve's parameter direction. `range` is a sub-interval of the
  curve's domain; on a periodic curve it may cross the period (`[3π/2,
  5π/2]`). A **degenerate edge** has no 3D curve: both vertices are the same
  vertex at a surface singularity (a sphere's pole, a cone's apex) and it
  exists only to give the face's loop a pcurve across the singularity; it
  carries the parameter range of its pcurves itself, since there is no
  curve to take one from (`Edge::range()` is the range of either kind).
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

### Euler operators

Entities are immutable and Euler operators mutate; the two meet in
`arris_topo::Builder`. A builder holds one body under construction as a
staging area of vertices, edges and faces in tombstoned slots, edited by
Mäntylä's ten operators (*An Introduction to Solid Modeling*, ch. 9,
adapted to coedges and seams; ADR-0002) and frozen into the arena by
`finish(&mut Model, BodyKind) -> Result<Built, BuildError>`, which
appends every live slot in order inside a transaction and returns the
body with the slot → id maps an operation's provenance is built from. The
builder is the only way an operation makes topology; the raw insert and
`import` are the other two paths into the arena, and neither is an
operation's.

| Operator | Makes / kills | Inverse |
|---|---|---|
| `mvfs(Seed)` | the first vertex, a face with one loop of no coedges at it, the shell | `kvfs()` |
| `mev(at, Strut)` | a vertex and the edge to it, used twice in a row by the loop at `at` — `Forward` away, `Reversed` back | `kev(edge)` |
| `mef(from, to, Split)` | an edge between two junctions of one loop and the face on its left: the edge `Forward` then the coedges from `to` around to `from`; the old loop keeps the rest after the edge `Reversed` | `kef(edge)` |
| `mekr(from, to, Join)` | an edge between two loops of one face, joining them | `kemr(edge)` |
| `kfmrh(kill, into)` | removes a one-loop face on `into`'s surface with the opposite orientation; its loop becomes a ring of `into`; genus + 1 | `mfkrh(face, ring)` |

Every operator keeps `V − E + F − (L − F) − 2(S − G) = 0` (`counts()`;
`G` is the builder's own count of handles), and every operator followed by
its inverse restores the builder byte for byte (`dump()`): a kill leaves a
tombstone the next make of that kind fills, most recently freed first,
and loops are kept canonical — rotated to start at their lowest `(edge,
orientation)` use, ordered within a face by that key, a loop without
coedges first by its vertex — so the state is a function of the content
alone. A kill returns the record its make takes, so undoing is a call.

Positions, not vertices: an operator's place in a loop is `Position {
face, loop_index, coedge_index }`, the junction before that coedge and
the effective start vertex of it, because a vertex may stand at several
junctions of one loop (a seam's, a closed edge's) and only the junction
says which. `coedge_index` runs `0..=len`, `len` being the junction after
the last coedge — the same vertex as `0`, the same insertion point for
`mev`, and for `mef` the split that moves every coedge (`from = to +
len`, from any junction) as opposed to none (`from = to`), which is how a
closed edge splits off a cap on either side. `find_position(face, loop,
vertex)` answers the unambiguous case and names every junction otherwise.

Orientations inside the builder are *effective*: a use is walked as seen
from outside the material with the face's outward normal up, and each
face carries the orientation the shell will use it with (`Seed`,
`Split`). `finish` stores a `Reversed` face's loop backwards with every
use flipped, so every stored loop is counter-clockwise about its
surface's normal (§Orientation) — the cylinder's bottom cap, used
`Reversed`, stores its circle `Forward`. `kfmrh` needs the two faces on
one `SurfaceId` with opposite orientations: two coplanar faces with
opposing outward normals and nothing between them, which is what the
floor of a pocket reaching the bottom face is, and the loop moves with
its pcurves.

Geometry is explicit and the builder never computes any of it: `mev`
takes the new vertex's point, the edge's `CurveId` and range and the
*two* pcurves of its two uses on the current face — a seam is exactly a
strut whose two pcurves differ by the period, and only the caller knows
which use it is drawing; `mef` takes the curve, the range, the new face's
`SurfaceId` and one pcurve per side; every pcurve is `Option` and
`set_pcurve(position, id)` gives or replaces one, since a coedge `mef`
moves to a face on another surface keeps a pcurve id that is no longer
its own, and a strut in a face it will leave has none worth giving.
`finish` refuses a coedge without a pcurve, a loop without coedges, an
edge not used exactly twice or twice the same way, a geometry id that
does not resolve, and any kind but `Solid` — each a typed `BuildError`,
and the model exactly as it was — and never evaluates geometry: the
checker, above this crate, is where the finished body is proven.

### Adjacency and iteration

The arena keeps derived indices, maintained on every append because
entities are immutable: edge → coedges (`CoedgeRef { face, loop_index,
coedge_index }`, the address of a coedge, since loops and coedges are not
entities), vertex → edges (each edge once, a closed edge included), face →
shells. The queries — `Model::edge_uses(edge)`, `vertex_edges(vertex)`,
`face_shells(face)` — are model-wide, in creation order of the referencing
entity, and `NotFound` for an id that does not resolve; an entity shared
by two bodies lists both bodies' uses, and a per-body question filters
through the closure. A reference that does not resolve when its entity is
appended (a raw insert with a dangling id) is not indexed: M1 is where it
is reported. The indices are one value shared by every clone of a model
and copied whole on the first append after a clone, so `Model::clone`
stays O(chunks) and the copy is paid once, by the clone that diverges.

`Model::shells(body)`, `faces(body)`, `edges(body)`, `vertices(body)`
iterate in a deterministic order — depth-first over the body's shells,
faces, loops and coedges in stored order, then the free edges, then the
free vertices; each entity once at first visit, so a seam edge appears
once and a face used by two shells appears under the first. Every handle
yielded carries its *effective* orientation, composed by XOR from the
body handle down the path it was first reached by (§Orientation); a
vertex's is the orientation of the edge use that reached it — its
effective start first, then its end — which means nothing geometrically
and is there so every handle composes alike. A reference that does not
resolve is skipped; only the body handle itself is `NotFound`.
`closure(body)` is the same reach as sorted, duplicate-free id lists per
kind, geometry included: what the checker, `import`, `retain` and the
text dump walk. That order is the order tessellation numbers `FaceRange`s
in and the order provenance lists entities in.

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
| M2 | Every reference a parent in the body makes is in the adjacency index of the entity it names — a coedge in `edge_uses` of its edge, an edge in `vertex_edges` of its vertices, a shell in `face_shells` of its faces. A reference that did not resolve when its parent was appended (a raw insert naming a later id) is never indexed; every other row reads adjacency off the closure itself, so this is the only row about the indices | Fast |
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
| E2 | `start == end` exactly when the curve returns to its start over the range within the edge's tolerance: a closed curve names one vertex, an open one two. The geometric match of each end to its vertex is V2's | Fast |
| E3 | Every edge in a body is used by at least one coedge, or is a free edge of a wire/general body | Fast |
| E4 | For every coedge, the surface evaluated along the pcurve is within the edge's tolerance of the 3D curve at the same parameter, at `Precision::check_samples` parameters including both ends — the pcurve and the curve share the edge's parameter (same-parameter, same-range, always) | Fast |
| E5 | `edge.tolerance ≥ face.tolerance` for every face it bounds; `≤ vertex.tolerance` of both vertices | Fast |
| E6 | A degenerate edge has `start == end` and lies on faces whose surface is singular along its pcurve over its range (the image at `check_samples` parameters spans at most the vertex's tolerance) | Fast |
| E7 | A seam edge (used twice by one loop) has its two coedges in opposite orientation and pcurves that differ by exactly the surface's period in the periodic parameter | Fast |
| E8 | The edge does not self-intersect within its range. An analytic curve over a range E1 accepted cannot; a NURBS is tested as a polyline of `check_samples` points per knot span, two non-adjacent segments closer than the edge's tolerance being the crossing | Full |

**Loop and face**

| # | Invariant | Level |
|---|---|---|
| L1 | A loop has at least one coedge and is closed: coedge *i*'s effective end vertex is coedge *i+1*'s effective start vertex, cyclically. Reported once per loop, at the first junction that breaks | Fast |
| L2 | The pcurves are continuous in (u, v) at every coedge junction within `parametric_tolerance` scaled to the surface's speed, or jump by exactly one period in a periodic parameter — across a seam edge, and where a closed edge's pcurve wraps the parameter once. Every junction that is neither is reported | Fast |
| L3 | No edge is used twice in one loop except as a seam (E7); no edge is used by two loops of the same face except as a seam | Fast |
| L4 | Each loop's signed area in (u, v) is non-zero — its mean width, the area over half its perimeter, is above `parametric_tolerance` — and the loops of a face have exactly one outer loop (positive winding) per connected component of the domain, holes with negative winding inside one of them. Two outer loops are one component exactly when one contains the other; disjoint ones are two | Fast |
| L5 | The loops of a face do not intersect each other or themselves in (u, v), as polygons within `parametric_tolerance` of the pcurves | Full |
| F1 | The face has a surface and at least one loop; every pcurve lies within the surface's non-periodic domain bounds, to `parametric_tolerance`. Reported once per face, at the first coedge that leaves them | Fast |
| F2 | `face.tolerance ≥ Precision::min_tolerance` and ≤ every incident edge's | Fast |

**Shell and body**

| # | Invariant | Level |
|---|---|---|
| S1 | Every face use in a shell resolves and no face is used twice by one shell | Fast |
| S2 | In a `Solid` body every edge of the shell is used by exactly two coedges, with opposite effective orientation (the two faces agree on which side the material is); in a `Sheet` by one or two; in `General` by any number. The orientations pair up in every kind: as many forward uses as reversed, but for an odd count, where exactly one is left over. A `Wire` body's shell is not judged here — B3 says it should have none | Fast |
| S3 | A shell is connected through its edges | Fast |
| S4 | A shell of a `Solid` is closed: no edge with one coedge | Fast |
| S5 | The faces of a shell intersect only along their shared edges and vertices: their surfaces' intersection is empty, or every point of it that is interior to both faces is within the tolerance of an edge or vertex they share; two coincident surfaces must not carry faces whose interiors overlap. A pair the intersector has no closed form for is **unchecked** — listed by `Report::unchecked`, never passed and never a violation | Full |
| B1 | A `Solid` body has at least one shell; exactly one shell encloses positive volume and is the outer one, and every other encloses negative volume — its effective normals turned into the void — and lies inside it, by the parity of a ray cast from one of its vertices. A shell no ray could be classified against is **unchecked**, as S5's undecided pairs are | Full |
| B2 | A `Solid` body encloses positive volume: `∬ p · (r_u × r_v) / 3` over each face's region in (u, v), summed with the sign of each face use. The value is reported with the violation | Full |
| B3 | A `Wire` body has no shells; `free_edges` form chains (each vertex used by at most two free edges) — `General` bodies exempt | Fast |

**Euler–Poincaré** (every level, reported as one line, never a violation on
its own): `V − E + F − (L − F) − 2(S − G) = 0` with `L` the number of loops
and `S` the number of shells. The genus `G` is *derived* from the counts,
as the oracle derives it, so the line cannot fail on its genus; what it
checks is its parity — a count set that leaves a residual of one cannot
come from any closed orientable surface, whatever its genus. `Report::euler`
carries the line, `arris_debug::dump_text` prints it, and every fixture
asserts it. The line is linear in the closure, so it is taken at `Fast` too.

## Provenance

Every operation returns a `Provenance`: which output entities came from
which origins, and how. Three relations, in Open CASCADE's
`BRepTools_History` vocabulary (read in the reference tree), because they
are the three a parametric history needs; and an origin is an input
entity *or a role* — what an entity is to the operation that made it from
nothing — so that every chain has a root (ADR-0002):

```rust
pub enum Relation { Generated, Modified, Deleted }
pub enum Origin   { Entity(Shape), Role(Role) }
pub enum Role     { Box(BoxPart), Cylinder(CylinderPart) }   // exhaustive; M5 adds the profile roles

pub struct Provenance {
    generated: BTreeMap<Origin, Vec<Shape>>,   // origin → outputs generated from it, sorted
    modified:  BTreeMap<Origin, Vec<Shape>>,   // origin → outputs that are pieces of it
    deleted:   BTreeSet<Shape>,
}
```

- **Generated**: the output is a new entity of a *different* kind or role
  built from the origin — the wall of a hole from the tool's cylindrical
  face, an intersection edge from a pair of faces (one record per face;
  `generated_pair(a, b)` is their intersection), the side faces of an
  extrude from the profile's edges, every entity of a primitive from its
  role (`Role::Box(BoxPart::Face(Coord::Z, Side::Max))` is a box's top;
  `BoxPart::Edge { along, sides }` and `BoxPart::Vertex([Side; 3])` name
  the rest; `CylinderPart::{Wall, BottomCap, TopCap, BottomRim, TopRim,
  Seam, BottomVertex, TopVertex}` a cylinder's; both have `Shell` and
  `Body`).
- **Modified**: the output is a trimmed, split or re-tolerated piece of the
  input, same kind — the box's top face with a circle cut out of it, each
  half of a face split by an intersection curve (one input, several
  outputs), a transformed face.
- **Deleted**: the input has no image of its own kind in the output — the
  part of the tool inside the target, a face swallowed by a fuse.
- **Kept** is not recorded: an entity untouched by the operation keeps its
  id and is simply present in the output body. `Provenance::is_kept(input,
  &model, output_body)` is a query, not a relation.

Every entity of every input body is accounted for: it is kept, or it is
recorded — `Modified` into pieces, `Generated` from, `Deleted`, or both
`Deleted` and `Generated` from (the tool face that is gone and whose
image is the hole's wall); never both `Deleted` and `Modified`, since a
piece is an image. The ops tests assert that accounting on every fixture,
and that the relations are the same on every run.

Queries: `generated_from(origin) -> &[Shape]`, `modified_from(origin)`,
`is_deleted(input)`, `origins(output) -> Vec<(Relation, Origin)>` (the
inverse), `outputs()`, `origins_recorded()`, and `Provenance::then(&self,
&next) -> Provenance`, which composes two records so that a chain of
operations (eight cuts of a bolt pattern) reports against the original
inputs: an output of the first that the second modifies is replaced by
its pieces and one it deletes is dropped, with the relations chained
(`Modified` then `Modified` is `Modified`; anything through `Generated`
is `Generated`); one the second generates from stays and gains the
children; an input modified into pieces that are all gone is deleted;
intermediate entities appear nowhere. Composition is associative over
well-formed chains — an output is a new entity, and a record names only
what exists when it runs — and the tests check it at a thousand random
chains. `Provenance::mapped(&IdMap)` translates a record through the id
map `import` returns, leaving ids the map does not hold (origins in
bodies that were not imported) as they are.

**Stability** is what the record is for. Rebuilding the same feature tree
with a changed parameter produces, for each output entity, the same
`origins` chain in terms of the *inputs' roles* (the third hole's tool
face, the top face of the base plate) — because the record is built inside
the algorithm from the entity ids it actually split, not recovered afterwards
by geometric matching, and because the chain ends at a `Role`. A consumer's
persistent name is therefore a function of the origins chain, and the
roadmap's acceptance corpus asserts that function is constant across
parameter changes.

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

The text dump (`arris_debug::dump_text(&model, body)`) is a different
thing: a human-readable, deterministic listing that fixtures store as
`dump.txt` and tests diff. It has no reader and is never a format. Its
lines are: the model's `Precision`; the body with its kind; its shells,
faces, loops and coedges depth-first in iteration order (§Adjacency and
iteration), each handle with its effective orientation (`+f0`, `-e1` —
a seam edge appears twice in its loop, once with each sign), each face
with its surface written out and each coedge with its pcurve; the free
edges and vertices; then `edges` (each once, with its vertices, curve,
range, tolerance and the curve written out) and `vertices` (point,
tolerance) in iteration order; then the Euler line `euler V/E/F/L/S
g<G> = <r>` where `G` is the genus the counts imply and `r` the residual
of §Euler–Poincaré's identity once `G` is rounded down, `0` for a line
that closes. Every number is rounded to `DUMP_DECIMALS` (12) places with
trailing zeros trimmed, so an ulp never shows and a real change does; a
reference that does not resolve is written as its id and `?`, never
skipped, so the dump of an invalid body says where.

## Open questions

- `⚠ OPEN:` quadric–quadric intersection curves, exact variant or fitted
  NURBS (§Curves).
- `⚠ OPEN:` consumer references onto provenance (§Provenance,
  01-architecture §Facade).
