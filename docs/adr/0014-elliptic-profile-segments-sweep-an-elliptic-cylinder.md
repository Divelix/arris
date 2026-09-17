# ADR-0014 — Elliptic profile segments sweep an elliptic cylinder; a revolve refuses them

- Status: accepted (2026-09-17)
- Plan: `facade-swap` step 1
- Follows: ADR-0008 (the quadric guard), ADR-0013

## Context

The consumer's sketch-to-profile conversion emits elliptic segments
beside lines and arcs. Each one is an elliptic arc in the sketch plane,
given by:

- a centre;
- the end of its major axis, which fixes the major radius and the
  rotation;
- a minor radius;
- a start and a sweep in the eccentric anomaly (`a cos τ, b sin τ` in
  the axes' frame), with the sweep's sign giving the direction.

A full ellipse is a one-segment loop with a sweep of `±2π`. Two more facts
about the consumer shape this decision:

- **Its backend never builds a real ellipse.** It builds each elliptic
  piece as a circular arc through three points of the ellipse, so an
  extruded ellipse has the wrong shape between those points.
- **Nothing it ships reaches the kernel with an ellipse today.** The
  sketch entities sit behind a feature that is off by default, no sketch
  tool draws one, and no document or test extrudes, revolves or cuts
  with one. Its sketch tests do find elliptic holes in plates.

`geom::Profile` has lines, arcs and circles only (`docs/DATA-MODEL.md`
§Profiles). A facade swap that drops the segment would be a regression,
and one that approximates it would repeat the old backend's mistake. The
segment has to be exact.

Four decisions follow from that. What surface an extruded ellipse sweeps,
what a revolve does with one, how STEP carries the surface, and how a
boolean treats the new faces. Projection needs no decision: the new edges
are ellipses and lines, and `project_to_plane` already maps both.

Read for this: Open CASCADE's `StepToGeom::MakeSurfaceOfLinearExtrusion`
and `GeomToStep_MakeSurfaceOfLinearExtrusion`. A STEP linear-extrusion
surface reads back as `C(u) + v·D`, with `D` the axis vector's direction
and its magnitude dropped. `Geom_Ellipse`'s `C(u)` is `O + a cos u·X +
b sin u·Y`.

## Decision

### The profile grammar

```rust
pub enum ProfileLoop {
    Circle  { center: Point2, radius: f64 },
    Ellipse { center: Point2, major: Vec2, minor_radius: f64 },
    Path    { start: Point2, segments: Vec<ProfileSegment> },
}

pub enum ProfileSegment {
    LineTo(Point2),
    ArcTo { to: Point2, via: Point2 },
    EllipseTo { to: Point2, center: Point2, major: Vec2, minor_radius: f64, ccw: bool },
}
```

- **`major`** runs from the centre to one end of the major axis, so its
  length is the major radius, as the consumer draws it. **`ccw`** is the
  turn in the plane's (u, v): counter-clockwise from the previous end to
  `to` when true.
- **An arc needs `via`; an ellipse does not.** An arc's centre is not
  given, so `via` has to fix it. An ellipse's centre and axes are given,
  so only the direction is left, and a flag states it with nothing to
  cross-check.
- **Ends, not angles.** The segment keeps the path grammar: it ends at a
  point, and the previous segment's end is its start. The consumer
  evaluates its eccentric anomaly at `start + sweep` to get `to`, and
  `ccw` is the sweep's sign. A `±2π` sweep in a one-segment loop becomes
  `ProfileLoop::Ellipse`, and the sign is dropped: the loop's role sets
  its orientation, as it does for a circle.

`Profile::edges` gets these rows beside the existing ones, each naming
`(loop_index, segment)`:

| Check | Error |
|---|---|
| `major` and `minor_radius` are finite and each above `tol.linear` | `DegenerateEllipse` |
| the segment's start and `to` each lie within `tol.linear` of the ellipse | `OffEllipse { distance }` |

A `to` equal to the start is `ShortSegment`, as it already is for an arc.
The full ellipse is the loop variant, never a path segment.

`Profile::edges` also normalises the ellipse:

- **A minor radius longer than `major`** gives the same point set. The
  edge's `Curve::Ellipse` swaps the axes and turns the frame a quarter
  turn, so `a ≥ b` holds as it does everywhere else.
- **Radii that agree within `tol.linear`** give a `Circle` edge instead,
  of their mean radius, through the same ends. It deviates from the
  ellipse by at most half that difference. A near-circular section is
  then always a cylinder, with every cylinder arm the booleans and
  blends already have.

An elliptic edge's `Curve::Ellipse` has these conventions:

- `Z` is `±` the plane's normal, so the parameter runs in the segment's
  turn;
- `X` lies along the (normalised) major axis;
- its pcurve on the plane is the exact `Curve2::Ellipse` from
  `pcurve_on`;
- a full ellipse's one vertex is at `center + major`, parameter zero,
  where `gp_Elips` on the plane's `Ax2` puts it.

### The surface: a new exhaustive variant

```rust
Surface::EllipticCylinder { frame: Frame, major_radius: f64, minor_radius: f64 }
```

| `P(u, v)` | Domain | Periodic | Seam |
|---|---|---|---|
| `O + a c u·X + b s u·Y + v·Z`, `a ≥ b > 0` | u ∈ [0, 2π), v ∈ ℝ | u, period 2π | the ruling through `O + a·X` |

- **Normal.** `∂P/∂u × ∂P/∂v = b c u·X + a s u·Y` points outward and is
  never singular.
- **Placement.** An extrude places the surface as it places a cylinder:
  origin at the ellipse's centre, `Z` along the extrusion, `X` the
  edge's major axis. The cap ellipse is then the section at constant
  `v`, and a ruling the line at constant `u`.
- **Pcurves.** Both are exact `Curve2::Line`s, with `u` running with or
  against the ellipse's turn by the sign of its `Z` against the
  surface's. Every other curve on this surface is `Unsupported` in
  `pcurve_on`, with no fitted fallback, as on the surfaces of
  revolution.
- **Surface queries.** They are the ellipse's, extended along `Z`:
  - `project` goes through the ellipse's quartic in the right section,
    and is `Ambiguous` on the axis and on the strip over the evolute
    segment;
  - `chord_steps` bounds `u` by the largest second derivative, `a`, and
    leaves `v` flat;
  - `inner_step` is a quarter period;
  - `bounds` is the ellipse's per-axis extrema swept along `Z`.
- **Intersection arms.** These are the arms step 2 needs, all in closed
  form:
  - **`intersect_surfaces`, plane against elliptic cylinder, in every
    pose.** A plane across the axis cuts an ellipse, the affine image
    of the section. A plane parallel to the axis gives two rulings, one
    tangent ruling, or nothing, from the line–ellipse quadratic in the
    right section.
  - **Parallel axes**, an elliptic cylinder against a cylinder or
    another elliptic cylinder. The pair reduces to two conics in the
    right section, through `arris_math::roots`' quartic. The result is
    `Coincident`, `Empty`, `Tangent` rulings or `Transversal` rulings,
    with the same ordering rules as parallel cylinders.
  - **`intersect_curve_surface`, line against elliptic cylinder.** This
    is the classifier's ray: a quadratic in the frame that maps the
    section to a circle. The same arm answers a conic lying in a
    section plane, which is `Coincident` or a 2D conic pair.
  - **Everything else is an explicit `Unsupported` arm:** crossing
    axes, and any pair with a cone, a sphere, a torus or a NURBS. The
    meridian arm does not take the new surface, since it is not a
    surface of revolution.

  An extruded profile's faces all have parallel rulings and caps across
  them. S5 and B1 therefore decide every pair an extrude makes, and
  nothing is left unchecked at `Full`.

### STEP

The surface is written as:

```
SURFACE_OF_LINEAR_EXTRUSION('', #ellipse, #axis)
```

- `#ellipse` is `ELLIPSE('', #placement, a, b)`, placed by the surface's
  frame.
- `#axis` is `VECTOR('', #z, 1.)`.

Open CASCADE reads this back as `Geom_SurfaceOfLinearExtrusion` with the
same `(u, v)`, so the PCURVEs the writer already emits per use carry over
unchanged. STEP has no elementary elliptic-cylinder entity, and a
B-spline surface would be an approximation of the parametrisation, not
of the shape. The native format takes the variant through `serde` like
every other variant.

### Revolve: refused

A revolve whose profile has an elliptic segment or loop is
`OpError::Degenerate` with

```rust
Reason::EllipticRevolve { loop_index: usize, segment: usize }
```

The reason names the first such segment in the consumer's order, and the
refusal comes before any entity is made. This follows the precedent of
`Reason::SpindleTorus`: the surface the sweep would need has no variant.

- **A spheroid or an elliptic torus** would need two things. One is a
  surface of revolution with an elliptic meridian, as a variant or a
  `Nurbs`. The other is an ellipse section in the meridian arm's table
  (ADR-0008), with line–ellipse and circle–ellipse meetings in the
  meridian plane.
- **Nothing asks for it.** No document of the consumer's revolves an
  ellipse, and its revolve code has only ever built circle-arc stand-ins.
  So the refusal is not a regression the consumer ADR has to carry.
- **The build is a backlog line.** It lands with the first consumer or
  corpus that revolves an ellipse.

### Booleans: the guard refuses the new faces

The pave model's quadric guard (ADR-0008) covers the elliptic cylinder
too. A face on one is refused as `OpError::Unsupported` naming the face
and its pair, before any intersector is asked.

The guard is needed. Plane against elliptic cylinder is now a closed form
in `intersect_surfaces`, so without the guard a boolean would widen
silently, with no corpus behind it. Booleans with elliptic operand faces
are C3's, beside the other quadric pairs. Blends are already safe:
`ops::fillet` and `ops::chamfer` refuse any edge whose faces are not
plane–plane or plane–cylinder, naming the pair, and that table gains the
variant in its explicit refusal arm.

## Consequences

- **A breaking change to `Surface` and `SurfaceKind`.** Every exhaustive
  `match` on `Surface` breaks: the geometry dispatch in `arris-geom`, the
  checker, the sweeps, blends, tessellation, STEP, and `arris-debug`'s
  dump, sample and property strategies. That breakage is the feature
  (`.agents/rules/kernel.md` §API). Each site gets a real arm or a named
  `Unsupported`, never a wildcard.
- **A breaking change to `ProfileLoop` and `ProfileSegment`,** plus
  three new variants: `ProfileError::DegenerateEllipse`,
  `ProfileError::OffEllipse` and `Reason::EllipticRevolve`. The fixture
  recipe grammar (`tests/fixtures/README.md`) gains the ellipse loop and
  segment in step 2.
- **The oracle builds the same shape.** Open CASCADE's prism over a
  `gp_Elips` edge is a linear-extrusion face with the same
  parametrisation, so volume πab·h and the counts compare one to one.
- **Oblique extrusion of arcs gets a target.** An oblique circular
  cylinder is a right elliptic cylinder in its own section frame, and so
  is an oblique elliptic one. The backlog line for oblique extrusion,
  refused as `Reason::DirectionNotNormal` today, now has a variant to
  build on. It needs no second surface decision.
- **`project_to_plane`, `face_frame` and `frame_at` need no change.**
  The new edges are ellipses and lines. `face_frame` is for planes, and
  `frame_at` reads `Surface::normal`, which the variant defines
  everywhere.
- **The facade's elliptic segments become exact.** Before, they were
  circle-arc stand-ins, so the consumer's extruded ellipses change shape
  when it swaps kernels. That is a correction, and the consumer's ADR
  says so.

## Alternatives considered

- **An exact rational `Surface::Nurbs` for the side face.** A degree-2
  rational curve holds the ellipse exactly, and its linear extrusion is
  a NURBS surface, so the `Surface` enum stays unchanged. Rejected for
  three reasons:
  - Every row that has to decide the side face would see a
    `Surface::Nurbs` with no closed form: S5 against its caps and
    neighbours, B1's ray cast, and `project` (which a NURBS does not
    have).
  - The only exact way through those rows is to recognise the NURBS as
    an extruded ellipse. That rebuilds this variant behind a type that
    hides it, and every other NURBS arm stays `Unsupported` at `Full`.
  - The rational parametrisation is not the ellipse's angle, so its
    pcurves could not be `Line`s. A `Curve::Ellipse` edge's parameter
    would no longer be the surface's `u`, which forces a
    reparametrisation or a fitted pcurve on every cap edge.
- **A general `Surface::Extrusion { curve, direction }`,** Open
  CASCADE's shape. It covers ellipses, NURBS curves and any later swept
  curve with one variant. Rejected because each arm would dispatch again
  on the curve it holds, and the only kind any operation makes is the
  ellipse. A general swept surface belongs to C5's sweep along a path,
  which will have its own corpus.
- **A second radius on `Surface::Cylinder`.** One variant fewer, but
  every existing cylinder arm assumes a round section and would need a
  guard: plane–cylinder sections, cylinder–cylinder, the meridian arm,
  blends, the fillet table and the boolean's cylinder cases. A separate
  variant leaves them exact and makes the compiler list the sites.
- **An elliptic arc approximated by circular arcs or a fitted
  B-spline,** as the old backend does. Rejected because it is exactly the
  wrong-shape result the swap exists to remove.
- **Build the elliptic revolve now.** It needs a new surface of
  revolution, ellipse sections in the meridian arm and their own corpus,
  and no consumer path reaches it. That is a milestone's work on a case
  nothing exercises, so it stays a backlog line.
- **Profile segments by angle (`start`, `sweep`), as the consumer
  writes them.** Rejected because it breaks the path grammar, where each
  segment starts where the last ended. A loop's closure would then
  depend on two angle evaluations agreeing, rather than on points the
  validator checks.
