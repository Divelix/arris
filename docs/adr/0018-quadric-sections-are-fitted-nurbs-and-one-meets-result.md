# ADR-0018 — Quadric sections are fitted NURBS under the faces' tolerance, traced by ruling families in a region; one `Meets` result

- Status: accepted (2026-09-19)
- Plan: `quadric-intersection-curves` step 3 (the decision and the result
  type); the tracer and the fit it records landed in steps 1 and 2, the
  region and the fitted curves in the intersector land in step 4
- Closes: the quadric-curve `⚠ OPEN` of `docs/DATA-MODEL.md` §Curves
  (`SEED.md` §10.7, the first kickoff question); the mixed-kind result
  type ADR-0008 deferred
- Follows: ADR-0008 (the meridian arm, the quadric guard), ADR-0004 and
  ADR-0016 (the pave model the curves feed)

## Context

C3 puts every quadric pair in the intersector. Most of the new pairs
meet in curves no `Curve` variant carries: two cylinders on crossing
axes of unequal radii or skew within their radii, a cylinder against a
cone or a sphere off its axis, two cones on different axes — quartic
space curves — and a plane meets a cone in a parabola or a hyperbola in
some poses, conics with no variant. Until the representation is decided
none of them can be returned, S5 lists them unchecked, and the pave
model's quadric guard stays up.

Three representations were weighed (the idea, absorbed by the plan):
an exact `Curve::QuadricSection` over the two quadrics (A); a
`Curve::Nurbs` fitted to exact samples, held under the faces' tolerance
(B); a procedural `Curve::Intersection` over the two surfaces with B's
spline as its chart, evaluation converging onto both (C). Whatever is
stored, the costly part is *finding* the section — every branch, its
ends, and the singular points where branches meet — and that is common
to all three.

A second question rides with it. ADR-0008 refused a result that mixes
kinds — a circle beside a point on the axis, a crossing beside a touch
— and deferred its type here. In general position mixing is the rule:
two quadrics tangent at a point and crossing elsewhere meet in a
quartic with a double point.

Read in the reference tree: Open CASCADE's `IntAna_QuadQuadGeo` (the
closed-form case analysis, as for ADR-0008), `IntPatch_ImpImpIntersection`
with `IntPatch_ALine` and `IntPatch_ALineToWLine` (the analytic section
of two quadric patches, walked into a line of points — and its
`IntStatus_InfiniteSectionCurve`, the same unbounded branch the region
below answers), and `GeomInt_WLApprox` over `ApproxInt` (the walked line
approximated by a B-spline, the edge's tolerance then set from the
fit's error). Nothing was taken but the shape of the pipeline; the
tracer's algebra and the fit's tolerance rule below are Arris's own.

## Decision

**A section that is no conic is a fitted `Curve::Nurbs` (B).** The
tracer's branches are exact callables; each is fitted by `fit_curve`,
or `fit_curve_periodic` when closed, at the branch's own parameter, the
deviation the farther of the two surfaces from the fitted point. No new
`Curve` variant; STEP writes it as any B-spline; C4's marcher produces
the same thing. A **conic is never fitted**: an ellipse stays
`Curve::Ellipse`, and a plane's parabola or hyperbola on a cone is an
exact rational quadratic `Curve::Nurbs` over the region. Only the
quartics are approximated.

**The fit target is a named fraction of the pair's tolerance, below the
fit's own margin**, because each face's pcurve is fitted afterwards to
the 3D curve and can do no better than the 3D curve's distance from that
face, and it has to land within the same tolerance. So a fitted section
is *not* a reason an edge's tolerance grows (`docs/DATA-MODEL.md`
§Tolerances): the edge carries its faces' tolerance, as a closed-form
edge does. The constant, with that reason in its comment, lands with
the intersector's use of it (plan step 4).

**A closed branch is a periodic B-spline.** The pave model cuts a closed
section curve at its paves only and wraps the last block round; a
clamped fit whose ends meet would put a joint vertex where the oracle
has none. `fit_curve_periodic` wraps its knots and control points, so
the loop is closed and smooth across the seam by construction.

**The section is traced by ruling families, in a region.** One operand
is ruled — a cylinder, an elliptic cylinder or a cone — and is walked
by its own angle `s`; each ruling meets the other quadric in the roots
of `a(s)w² + b(s)w + c(s)`. With a cone's rulings taken from its apex,
the discriminant is a trigonometric polynomial of degree two in `s` for
every ruled kind, so the turning points are the roots of one quartic in
`tan(s/2)` (`arris_math::roots`): the branch topology is algebraic, not
sampled, and no loop can fall between samples. What steps 1 and 2
proved, recorded here as the decision's grounds:

- *The walked operand is a rule on the surfaces*: parallel rulings
  before a cone's, then the smaller radius or the narrower cone, then a
  circular section before an elliptic one, then the frames coordinate
  by coordinate. Swapping the arguments changes nothing, bit for bit.
- *Singular or not is a closed bound.* A critical point of the
  discriminant is a singular point when `|D| / 4|a||∇q|` — the distance
  the other surface would have to move for the ruling to touch it — is
  within `tol.linear`, or `|D|` is within its own rounding. The value is
  taken out as a bump, not clamped, so branches through the point end at
  it exactly (clamping leaves two roots `√(tol·R)` apart, far more than
  the tolerance). No pose is guessed at; the ones refused are named
  (`SectionFault`) and of measure zero.
- *Every branch is clipped to the region* along the walked rulings, at
  the roots of a quartic again — the unbounded branches of two cones or
  a plane's hyperbola, and a near-asymptotic loop a thousand radii long,
  which is as useless to a fit as an unbounded one. So
  `intersect_surfaces` takes a region (`within: &Aabb`), and a boolean
  passes one for the whole operation — the overlap of the operands'
  boxes, grown generously — so every face pair on the same two surfaces
  gets the same curve bit for bit. The closed-form arms ignore it.
- *The fit meets its bound between samples.* `fit_curve` keeps
  `fit_curve2`'s contract — checked at `4p + 4` parameters per span,
  accepted at half the tolerance — and the property tests hold it at
  2000 parameters per curve. Viviani's curve at `1e-7` takes 256, 128
  and 64 spans at degree 3, 4 and 5; the degree is chosen in step 4 on
  the metre-scale probes.

**One result shape for every meeting (i).** `SurfaceIntersection` is
`Empty`, `Coincident`, or `Meets { curves: Vec<MeetCurve>, points:
Vec<MeetPoint> }`, each entry carrying a `MeetKind` — `Crossing` or
`Touch`. `Transversal`, `Tangent` and `Points` are gone. A `Meets` has
at least one entry; the curves come in the order the arm that found them
documents, the two kinds interleaved in it, and the points ascend along
the shared axis. A point where traced branches end — a singular point,
where the surfaces are tangent — is among the points, and the branches'
ends there are that point. The two results ADR-0008 refused are now
returned: a sphere through a cone's apex, centred on its axis, is a
circle and the apex; parallel elliptic cylinders that touch and cross
are both kinds of ruling, ascending by the section's parameter. A
coincident section beside a meeting off it stays `Unsupported`: that is
a partial coincidence, not a mixed meeting.

**The tracer is the family method; the marcher stays C4's.** Exact
samples from closed forms are what make the fit trustworthy.

## Consequences

- `arris_geom::SurfaceIntersection` changes shape, a breaking change:
  every exhaustive match over it — S5, the pave model and its display,
  a blend's end section, the fixtures' runners — reads `Meets` by kind.
  S5 holds every curve and every point to its one rule whatever the
  kind, so it decides the two formerly refused positions instead of
  listing them unchecked.
- The pave model reads the crossing curves as section curves and the
  touching ones as contact rulings, by their index among the `Meets`
  curves. A pair past the quadric guard never meets in points or in both
  kinds; the pave model refuses one that does as an invariant until
  step 8 reads them — a singular point becoming the section vertex that
  ends its branches.
- A fitted edge lies within the fit's fraction of the tolerance of the
  true section, not on it. Invisible while that fraction is under the
  faces' tolerance; visible to a consumer re-evaluating a model at a
  finer precision than it was built at.
- `Curve::Nurbs` against every analytic surface and against the conics
  has to stop being `Unsupported` (plan steps 6 and 7), since the next
  boolean meets the fitted edge. That root isolation on a curve
  substituted into an implicit surface is C4's first need too.
- **Tripwires, kept as assertions and fixtures:** a fixture whose
  tolerances grow through chained booleans across a fitted edge (plan
  step 9 asserts every edge at its faces' tolerance), or a consumer
  re-evaluating at a finer precision than the model was built at.
  Either makes C the next step.

## Alternatives considered

- **A — an exact `Curve::QuadricSection`.** Exact at any precision, but
  it covers the quadric pairs only: the spiric section and every torus
  pair are no quadric sections and need B anyway, so the kernel would
  carry two representations. Its square-root parametrisation has branch
  points in evaluation, derivatives, projection and bounds; its pcurves
  are fitted regardless; STEP has no entity for it. About twice B's
  cost, still needing B.
- **C — a procedural `Curve::Intersection { surfaces, chart }`**,
  Parasolid's and ACIS's form. Exact to rounding at any precision, but
  every evaluation is an iteration — tessellation, sampling and
  mass-property quadrature all pay it — and it is ill-conditioned along
  a touching curve, where the two normals are parallel, which is where
  C3's tangent-face corpus lives. Its chart is B's spline, so B first
  forecloses nothing: **C is the upgrade path**, taken when a tripwire
  above fires. A backlog line records it.
- **A `Mixed { curves, points }` variant beside the three (ii).** A
  smaller diff, but every caller handles two shapes of the same thing,
  and in general position the mixed one is the common one.
- **Keep refusing mixed kinds (iii).** Incompatible with C3's accept
  line: the refused positions are ordinary in general position.
- **A general surface–surface marcher now.** It would decide every pair
  at once, but its samples are only as exact as its corrector, and it
  is C4's with NURBS operands; the family tracer's samples are exact to
  rounding on the walked surface.
