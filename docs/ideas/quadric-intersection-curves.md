# Idea: quadric-intersection-curves

- Status: Open
- Raised: 2026-09-19
- Prompt (verbatim from the human): "yes, /idea that" — on the
  quadric-curve `⚠ OPEN`, the decision C3 opens with

## Problem

C3 (`docs/ROADMAP.md`) puts every quadric pair in the intersector, and
most of the new pairs meet in curves that are no `Curve` variant: two
cylinders on crossing axes of unequal radii or skew within the radii, a
cylinder against a cone or a sphere off its axis, two cones on different
axes (quadric ∩ quadric, a quartic space curve); a plane oblique to a
torus (a spiric section, a plane quartic); a torus against any quadric
(degree eight). Until the representation is decided, none of them can
be returned, S5 lists them as unchecked, and the quadric guard stays up.
`SEED.md` §10.7 named this the first kickoff question; `docs/DATA-MODEL.md`
§Curves carries it as the one `⚠ OPEN` left.

A second, smaller question rides with it. ADR-0008 refused a result
that mixes kinds — a circle beside a point, a crossing beside a touch —
and deferred its type to this ADR. In general position mixing is the
rule: two quadrics tangent at one point and crossing elsewhere meet in a
quartic with a double point, and equal cylinders crossing are already
two ellipses meeting at their tangency points.

Not every C3 pair needs this. A plane meets a cone in a conic and a
sphere in a circle in any pose; those are conics (an ellipse, or a
parabola or hyperbola, which are no variant either — option B below).

## Constraints it runs into

- `SEED.md` §9: analytic surfaces are exact variants; NURBS is one
  variant; pcurves exact for analytic pairs, fitted otherwise. Nothing
  there says intersection *curves* must be exact.
- `.agents/rules/kernel.md`: geometry enums are exhaustive, so a new
  `Curve` variant breaks every match (the intended cost); tolerances
  come from the entity or a named constant, so a fit target is one or
  the other.
- `docs/DATA-MODEL.md` §Tolerances: an edge's tolerance only grows, and
  only for a nameable reason. A fitted curve is such a reason; the
  question is whether it ever has to grow past the faces'.
- ADR-0004 (pave model) and ADR-0016 (touches resolved through the
  section curves): the next boolean will hit these edges, so whatever
  the curve is, `intersect_curve_surface` and `intersect_curves` need an
  arm for it. Today `Curve::Nurbs` against every surface is `Unsupported`
  (`crates/arris-geom/src/intersect_curve.rs`).
- ADR-0008: the meridian arm stays as is; mixed kinds were deferred to
  here.
- Reference practice: Open CASCADE's boolean approximates a quadric
  intersection line by a B-spline and sets the edge tolerance from the
  fit's error. Parasolid and ACIS store a procedural intersection curve:
  the two surfaces plus an approximating spline, with evaluation
  converging onto both surfaces.

## What every option needs first: a tracer

The costly part is not storing the curve but *finding* it: every branch,
its ends, and the singular points where branches meet (where the
surfaces are tangent). All three options below need the same tracer.
There is a family method that reuses closed forms the kernel already
has: every cylinder, cone, sphere and torus is swept by lines or circles
(rulings, parallels, tube circles). Walk one surface's family parameter,
intersect each member with the other surface by the existing conic–surface
arms (extended to cone, sphere and torus, which C3's roadmap already
lists), connect the hits into branches, and find turning and singular
points where the hit count changes, by bracketing. Each sample is exact
to rounding on both surfaces. Roughly 3–4 plan steps, whichever option
is chosen.

## Options

### A — an exact `Curve::QuadricSection`

The quadric ∩ quadric quartic, stored exactly (the two quadrics and a
parametrisation by square roots, Levin's pencil). Exact at any
precision, no tolerance growth. But it covers only quadric pairs: the
spiric section and torus pairs are not quadric sections, so they still
need B, and the kernel has two representations. The square-root
parametrisation has branch points to handle in evaluation, derivatives,
`project`, `bounds` and `chord_segments`. Its pcurves are not `Curve2`
variants and would be fitted anyway. STEP has no entity for it, so the
writer fits a B-spline at export. A new variant in every exhaustive
match. Cost: about 10–12 steps on top of the tracer, plus B for the
torus. Forecloses nothing, but pays twice.

### B — fitted `Curve::Nurbs`, held under the faces' tolerance

The tracer's exact samples are fitted by a 3D least-squares B-spline (a
3D twin of `fit_curve2`, which exists only in 2D today). Each branch is
split at singular points, which become vertices. The fit target is
below the smaller face tolerance by a named factor, so the edge's
tolerance is the faces' and never grows. Pcurves are fitted by
`fit_curve2`, as the oblique ellipse on a cylinder and the miter's
pcurves are today. A plane's parabola or hyperbola on a cone, the conics
with no variant, is fitted the same way. It covers every pair (quadric, torus, and later C4's
NURBS marcher, which produces the same thing). No new variant. STEP
writes it as it writes any B-spline. The cost moves to
`Curve::Nurbs` against analytic surfaces and against conics. Every
analytic surface has an implicit equation, so a NURBS curve substituted
into it becomes a polynomial per span, solved by bracketed root finding.
That is also C4's first requirement. Cost: about 8–10 steps with the
tracer. What it gives up is exactness: the stored curve lies within its
fit tolerance of the true curve, the way Open CASCADE's does. That is
invisible while the fit sits under the face tolerance, and visible only
if a model is re-evaluated at a tighter precision than the one it was
built at.

### C — a procedural `Curve::Intersection { surfaces, chart }`

Parasolid's form: the two surfaces plus B's spline as a chart, with
evaluation Newton-converging from the chart's point onto both surfaces.
Exact to rounding at any precision, and STEP's `INTERSECTION_CURVE` (a
`SURFACE_CURVE` subtype the writer already nearly emits) carries it.
But every evaluation is an iteration, so tessellation, E4's samples and
mass-property quadrature all slow down. It is ill-conditioned exactly
where C3's tangent-face corpus lives: along a `Tangent` curve the two
normals are parallel everywhere, and the convergence step degenerates.
It is a new variant with every dispatch arm (`project`, `bounds`,
native format). Cost: B's steps plus about 5. B's chart is C's chart,
so B first forecloses nothing.

### Do nothing

The quadric guard stays up and C3 stops at the conic pairs (a plane
against a cone or a sphere in any pose), with S5 keeping its unchecked
list. The roadmap's C3 accept line cannot be met.

## The mixed-kind result

- **(i) One shape for every meeting.** Keep `Empty` and `Coincident`,
  and replace `Transversal`, `Tangent` and `Points` with a `Meets {
  curves, points }` whose entries each carry a contact kind (crossing or
  touch). A curve's ends at a singular point are among the points. Every
  match over the result changes once (S5, the pave model, a blend's end
  section, the display). General position makes mixing normal, and one
  shape is what the callers already iterate over.
- **(ii) Add a `Mixed { curves, points }` variant beside the three.**
  This is a smaller diff, but every caller then handles two shapes of the
  same thing.
- **(iii) Keep refusing.** A refusal is incompatible with C3's accept line,
  since the refused positions are ordinary in general position.

## Recommendation

**B, with (i).** A doubles the work and still needs B for the torus. C
is the most exact option, but it degenerates on the tangent curves that
C3's tolerance corpus is about and costs an iteration per evaluation,
and its chart is B's spline, so choosing B now keeps C open. B covers
every pair with one representation, adds no variant, writes to STEP as
it stands, and its only new geometry work (NURBS-curve hits on analytic
surfaces) is work C4 needs anyway. Held under the face tolerance, a fit
changes nothing a tolerance can see.

What would change my mind: a fixture where fitted section edges push
tolerances up through chained booleans (a bolt pattern on a sphere, say);
or a consumer needing a model re-evaluated at a finer precision than
it was built at. Either would make C, built on B's charts, the next
step.

## Decision for the human

1. **Fitted `Curve::Nurbs` (B), or a procedural variant (C), or an exact
   quadric section (A)?** Preferred: B, fit below the face tolerance so
   no edge tolerance grows, with C recorded as the upgrade path.
2. **The mixed-kind result: one `Meets { curves, points }` shape (i), a
   `Mixed` variant beside the others (ii), or keep refusing (iii)?**
   Preferred: (i), a breaking change taken with C3's first plan.
3. **The tracer: the family method over the existing closed forms, or a
   general surface–surface marcher now?** Preferred: the family method.
   The marcher is C4's, and exact samples from closed forms are what
   make B's fit trustworthy.

An ADR is needed either way: it closes the `⚠ OPEN` in
`docs/DATA-MODEL.md` §Curves (as `SEED.md` §10.7 asks) and settles the
result type that ADR-0008 deferred.
