# ADR-0019 — Torus sections are traced in the torus's parameter plane: a second tracer beside the ruling families, ADR-0018's storage unchanged

- Status: accepted (2026-09-20)
- Plan: `c3-torus-pairs` step 4 (the decision and the intersector's arms);
  the isolation, the tracer and the tube circle it records landed in
  steps 1 to 3
- Closes: the `Unsupported` arms of `docs/DATA-MODEL.md` §Curves for a
  torus sharing no axis with the other surface — the spiric section among
  them; the last pairs S5 and B1 listed unchecked that are not NURBS
- Follows: ADR-0018 (the ruled tracer, the fit, the one `Meets` result),
  ADR-0008 (the meridian arm, which keeps every coaxial pose)
- Amends: ADR-0018's "the tracer is the family method" — a second tracer
  for the surface with no rulings. Its storage decision, its fit rule and
  its result type are unchanged.

## Context

ADR-0018's tracer walks one operand's rulings: every ruling meets the
other quadric in the roots of a quadratic whose discriminant is a
trigonometric polynomial of degree two, so the branch topology comes out
of one quartic and is proven rather than sampled. **A torus has no
rulings.** Every pair with a torus in it that shares no axis with it was
left `Unsupported`: the spiric sections of a plane, a drill through the
tube, a pipe elbow, two interlocked tori — and with them the faces
`ops::fillet` and `ops::revolve` produce, which is why C3 exists at all
(ADR-0020).

Open CASCADE has no closed form for these either. `IntAna_QuadQuadGeo`'s
five torus overloads answer only the coaxial poses — two tori whose axes
are not parallel, or parallel and apart, are `IntAna_NoGeometricSolution`
at the top of `Perform(const gp_Torus&, const gp_Torus&, ...)` — and
`IntPatch_ImpImpIntersection`'s `IntPTo`, `IntCyTo`, `IntCoTo`, `IntSpTo`
and `IntToTo` pass the pair to that same closed form, so
`IntPatch_Intersection::GeomGeomPerfom` falls through to
`ParamParamPerfom`: the parametric marcher `IntPatch_PrmPrmIntersection`
over `IntWalk`, whose samples are as exact as its corrector and whose
topology comes from its start points. That is the walk `GeomAPI_IntSS`
performs, and what the oracle records as `unsolved` with walked lines.
Read for the case analysis and for where it stops; nothing taken.

Marching is C4's decision (ADR-0018), and taking it here for the torus
would make the kernel's hardest guarantee — *every part of the section is
on a branch* — a sampling argument for a quarter of its pairs.

## Decision

**A section with a torus in it is traced in the torus's own (u, v)**, by
putting the torus's parametrisation into the other surface's implicit
polynomial: `f(u, v) = 0` over sixteen quarter-turn patches, each a
tensor Bernstein polynomial in the half-angle charts of its quarter — the
rational weight between `cos² π/8` and one — of bidegree (2, 2) against a
plane, (4, 4) against a quadric and (8, 8) against a torus. The torus is
compact, so there is no region to clip to and `within` is no part of it.
What steps 1 to 3 proved, recorded here as the decision's grounds:

- **The branch topology is proven, not sampled.** Every component of the
  section either turns in `u` or winds round the torus and crosses
  `u = 0`, so the turning points — the common zeros of `(f, f_v)`,
  isolated by subdivision — and the isolated roots of `f(0, ·)` seed all
  of them. A box is excluded when one polynomial's coefficients keep a
  sign beyond their rounding floor, and a zero is certified alone in its
  box by the **Krawczyk test on the box grown by an eighth**: a plain
  interval Newton needs the gradients' components apart over the box and
  took nine quarterings on two circles and a line, and the growth puts a
  zero on the line between two boxes — where every symmetric pose puts
  one — inside both, to be merged after rather than followed down. The
  isolation may return a candidate too many and never one too few, as
  `bernstein.rs`'s does in one variable.
- **A branch is graphs `v(u)` joined through their turning points** by
  ADR-0018's `u = u_T ± L(1 − cos θ)`, so `Arc1` and `SectionBranch` are
  shared with the ruled tracer and `section::traced` fits both with one
  code path. What a ruled pair has in closed form — the root on a ruling
  — is here the root of the other surface's level along a tube circle, by
  bracketed Newton, and **the bracket is proven, not followed**: each
  graph is covered by cells marched along it, each certified on the
  Bernstein coefficients over the cell's box — `f_t` of one sign and `f`
  of opposite signs along the two edges, so there is one root at every
  `u`. No labels and no root counting, so the roots exactly on a patch
  edge that every symmetric pose has never come into it.
- **Singular or not is decided in length, on two polynomials and never
  three.** The singular points are not the zeros of `(f, f_u, f_v)` but
  the critical points of `F(P(u, v))`: the zeros of `w·f_s − d·w_s·f` and
  its `t` twin, which equal `w^(d+1)·∂F/∂s` and are free of the patch's
  weight off the section too, so a near miss is the same point from
  either side of a patch edge. `f` only gates, at `steepness · tol.linear`
  above its rounding floor; the *distance* decides, as ADR-0018 decides
  it on the discriminant's own value — `f` carries a factor `R²r`-sized
  for a torus and states nothing in length.
- **The value is taken out as a bump in the Hessian's own metric.**
  ADR-0018's round bump fails on the first thin torus: the distance's
  curvature differs a hundredfold between `u` and `v`, and a reach wide
  enough for the flat direction swamps the steep one's cell. With
  `y² = xᵀ|H|x / reach²` and `reach² = 8|δ_S|`, the correction adds at
  most three quarters of the quadratic part along each eigenvector and
  keeps the signature — ADR-0018's bound, in the metric the pose sets. A
  saddle is a crossing of four arms, an extremum an isolated point, and
  branches through a singular point end at it exactly.
- **A tube circle of the torus on the other surface is a conic, returned
  exactly.** There `F(P(u, v)) = sinᵐ((u − u₀)/2)·H(u, v)`, `m` being 1
  where the surfaces cross along the circle and 2 where they are tangent
  along it, and **both halves of the tracer divide by that factor**: the
  patches by a linear factor of a column's chart in Bernstein form, the
  walk by a closed form of `F` along the chord from the circle's point.
  The circle comes back as a `Curve::Circle` on the torus, parametrised
  by its `v`, `Touch` where the surfaces do not cross along it and
  `Crossing` where they do — a pipe elbow against its pipe, a sphere or a
  cone about the tangent to the centre circle, a plane through the axis —
  and the rest of the section is traced on the quotient, its branches cut
  at the circle into arms ending at `SectionPoint`s. What is dropped for
  the division to be exact is the same polynomial on both sides, so the
  certificates and the roots are of one function to rounding.
- **Which of two tori is walked is a rule on the surfaces**: the smaller
  over all (`R + r`), then the smaller tube, then the frames coordinate
  by coordinate. The rule is numerical before it is a tie-break — a
  torus's quartic at points a hundred of its own sizes away cancels to
  `100⁴·ε`, and with the large torus walked one turning point in 35 000
  sections came back uncertified. Swapping the operands changes nothing,
  bit for bit.

**Storage is ADR-0018's, unchanged.** Every branch is a `Curve::Nurbs`
fitted at the branch's own parameter within `SECTION_FIT_FRACTION` of
`tol.linear` of both surfaces, periodic when the branch is closed; every
singular point is a `MeetPoint` of the one `Meets`, `Touch` where
isolated and `Crossing` where branches end at it; the tube circles come
first in the result's curves, in the tracer's order, then the branches in
theirs. Measured here at `SECTION_FIT_DEGREE` (5) on metre-scale tori
(`R/r` from 1.1 to 100, radii 0.01 to 10): a loop takes **21 to 319
control points**, against 60 to 120 for the cylinder pairs the degree was
chosen on, and 800 at degree 3 where the quintic takes 245. Degree 6 is
fewer on the smooth loops (38 against 53) and *more* on the ones whose
curvature varies most (362 against 319 on a cone's section), so the
quintic stays — it is also `PCURVE_FIT_DEGREE`, and the two are fitted
one to the other.

**The Villarceau circles are fitted, not returned exact** (the plan's
open question 2). A bitangent plane's two circles come back as the four
arms the tracer finds through the pose's two singular points, each within
the fit's fraction of the tolerance of the exact circle. ADR-0018 says a
conic is never fitted, but that rule is about the poses a *closed form*
owns: here the pose is one of measure zero, and answering it exactly
takes a second "bitangent within `tol.linear`" detector beside the
tracer's own singular-point rule, to be kept consistent with it pose by
pose — the cost the tube circle's detector paid because a pipe elbow is
an ordinary shape and the factor it leaves is what makes the rest of that
section traceable at all. A backlog line records it; the pose is in the
corpus as the fitted arms.

**A tube circle is within `tol.linear` of the other surface**, not within
rounding: it is accepted in length on the exact distance all the way
round, and on `|F|` against what `tol.linear` makes of `F` anywhere on
the torus, so the rest, traced without the factor, stays within
`tol.linear` too. It is the one curve of a `Meets` held to the tolerance
rather than to rounding, exactly as a `MeetPoint` is.

**The poses the tracer does not resolve are refused by name**
(`SectionFault`), never guessed at, and each is of measure zero:
`TubeCircle` (two such circles with more of the section besides; a circle
not held to the tolerance wherever the rest is traced — a cone's, by its
apex, half a tolerance off the pose), `UnresolvedTurning` (turning points
`f64` does not tell apart away from a singular point, a loop within about
1e-4 of a tube circle all the way round among them — the march follows
graphs `v(u)`, and that loop is a graph `u(v)`), `TangentAlongCurve` (a
continuum of turning points, the torus against itself) and
`CrowdedSingularity` (a degenerate critical point within the tolerance,
singular points within twice each other's reach, a turning or singular
point within the tolerance of a tube circle).

## Consequences

- **`intersect_surfaces` returns `Unsupported` only for a pair with a
  `Surface::Nurbs` in it.** Every pose of every analytic pair is decided:
  the property that held that of the pairs without a torus now holds it
  of all of them, and `every_other_pair_is_unsupported` is left the NURBS
  pairs, which are C4's. S5 and B1 decide a torus face against anything,
  with only the tracers' named refusals unchecked.
- **The fit dominates a torus pair's cost.** The trace is 0.7 to 1.1 ms
  (dev profile, the plan's record); the fit of its branches is 3 to 90 ms
  — a 552-control-point loop on a grazing cone's section is the worst
  seen. The ruled pairs pay the same fit and are cheaper only because
  their loops are smoother. A cost that matters to S5 is a fit to share
  between face pairs, not a faster tracer; the backlog carries it.
- **`MAX_FIT_SPANS` is 4096, not 1024.** A torus section is as long as
  the two tori are big: the single loop two metre-scale tori of nearly
  equal radii share runs 100 to 170 units and takes 1000 to 1300 spans
  at degree 5, where a cylinder pair's loop takes under 200. The cap
  measured on the cylinder pairs cut those fits off a factor of two
  short, on branches that are not pathological — samples on both
  surfaces to 6e-15, a tightest curvature radius of 0.33, the normals
  never nearer than 6° of parallel. The cap is a budget that makes the
  refinement terminate, not a tolerance, and the work per refinement is
  linear in the spans, so raising it costs nothing where it is not
  reached.
- **The pcurve of a torus section is not fitted here.**
  `SectionBranch::uv` carries the branch's exact (u, v) on the walked
  torus, unwrapped across both seams, so C3's third plan can build that
  pcurve from the branch instead of projecting the fitted curve
  (`⚠ OPEN` in the plan). Until it does, `pcurve_on` of a fitted curve on
  a torus stays `Unsupported` and the pave model's quadric guard stays
  up.
- `bernstein2` — tensor Bernstein arithmetic, subdivision, exclusion and
  the certified isolation of two polynomials' common zeros — is
  crate-private in `arris-geom` and is what C4 puts a NURBS patch into an
  implicit with.
- **Tripwires:** a consumer whose tori are thin enough or large enough
  that `UnresolvedTurning` stops being a pose of measure zero, or a
  boolean corpus where the fit's cost dominates a whole operation. Either
  makes the marcher, or a shared fit, the next step.

## Alternatives considered

- **Tube circles as the family** (the idea's option A, the fallback if
  step 1's cost gate had failed): walk the torus's tube circles as the
  ruled tracer walks rulings, which is a closed form against a plane and
  a sphere — a circle meets a quadric in the roots of a quartic — and
  against nothing else. It answers two of the six pairs and leaves the
  other four to C4, which is the cycle's goal unmet; the cost it would
  have saved is a sixth of what the intersector already spends on a ruled
  pair.
- **The parametric marcher now** (Open CASCADE's route above, and
  truck's `truck-shapeops` SSI): one algorithm for every pair, including
  C4's NURBS. But its samples are only as exact as its corrector, its
  branch topology comes from its start points, and it is at its worst
  along a tangency — which is where C3's corpus lives. It stays C4's, for
  the operands that have no implicit form.
- **Eliminating one parameter algebraically** — the resultant of the two
  implicits over the torus, a degree-8 polynomial in `tan(u/2)` whose
  coefficients are polynomials in `v`. Exact in principle; in practice
  the resultant's coefficients cancel catastrophically at the sizes a
  metre-scale torus produces (the same `100⁴·ε` the operand rule above is
  about), and it answers a plane's spiric section only, where the closed
  form is already a quartic.
- **Subdividing the torus into NURBS patches** and using C4's
  surface–surface machinery on them: it makes the torus's exactness — the
  one operand whose samples are exact to rounding — into an
  approximation, and doubles the pose count the corpus has to cover.
