# Plan: c3-torus-pairs

- Started: 2026-09-19
- Milestone: C3, every quadric pair (docs/ROADMAP.md §C3: what the first
  bullet has left, the torus part of the S5 and B1 bullet, and the accept
  line's "every quadric pair"; the second of C3's plans)
- Idea (verbatim from the human): "c3-torus-pairs"
- Idea: docs/ideas/c3-torus-pairs.md (absorbed)

## Goal

A torus meets every analytic surface in every pose in
`intersect_surfaces`: a plane, a cylinder, an elliptic cylinder, a cone, a
sphere and another torus sharing no axis with it, the spiric sections and
the Villarceau circles among them. The idea's four decisions are taken, on
the human's delegation, as it recommended: **the section is traced in the
torus's own (u, v)** — the torus's parametrisation put into the other
surface's implicit polynomial, `f(u, v) = 0`, a bivariate Bernstein
polynomial per quarter-turn patch, its turning points (`f = f_v = 0`) and
singular points (`f = f_u = f_v = 0`) isolated by subdivision (B); **all
six pairs in this cycle**, torus–torus included; **ADR-0019** records the
second tracer beside ADR-0018's family method; and **this is a plan of its
own** — the conic hits and the lifted guard are C3's third plan.

ADR-0018's storage decision holds unchanged: each branch a fitted
`Curve::Nurbs` within `SECTION_FIT_FRACTION` of `tol.linear`, periodic when
closed, singular points as `MeetPoint`s of one `Meets`. So does its
guarantee: the branch topology is proven, not sampled. Every component of
the section either turns in `u` or winds round it and crosses `u = 0`, so
the isolated turning points and the roots of `f(0, ·)` seed all of them.
The torus is compact, so `within` is ignored and every face pair on the
same two surfaces gets the same curve bit for bit with no region at all.

When the plan is done: the six torus pairs are property-tested at random
poses, every point on both surfaces within the tolerance and a brute-force
sweep finding nothing off a branch; `geom/c3-torus-pairs` matches Open
CASCADE's walked lines; no pair of analytic surfaces is `Unsupported` in
`intersect_surfaces`; and S5 and B1 decide a torus face against anything,
with only the tracers' named refusals left unchecked.

## Non-goals

- Booleans with a torus operand face: the pave model's quadric guard
  stays up, and so do the conic hits on a cone, a sphere or a torus in
  `intersect_curve_surface`, the two coplanar conics in
  `intersect_curves`, and the fitted pcurve fallback on cones, spheres and
  tori (`pcurve_on` of a fitted curve on a torus stays `Unsupported`).
  C3's third plan, with its corpus of quadric-operand booleans. This plan
  leaves it the branch's exact (u, v) to fit the torus's pcurve from
  (design deltas), and changes nothing in `arris-ops`.
- Features a tolerance apart in a section: two singular points nearer
  each other than the tolerance, a near-tangency along a whole curve.
  Decided only as far as ADR-0018 decides them — one singular point or
  none within `tol.linear`, the rest a named `SectionFault` — and left to
  the plan that takes `regression/seam-a-tolerance-from-crossing-fuse`.
- A spindle torus (`R ≤ r`): C3 "Out"; the substitution assumes a ring
  torus and `Surface::Torus` holds no other.
- NURBS operands and the marcher (C4). `bernstein2` is written so C4 can
  put a NURBS patch into an implicit with it, and no further.
- Conic sections of a torus beyond the two that are named poses: a tube
  circle lying on the other surface (step 3) and the Villarceau circles
  (open question 2). Any other circle a posed surface happens to share
  with a torus — an oblique elliptic cylinder through a parallel circle —
  is returned fitted.
- A fit that shares the tracer's samples between face pairs, and S5's
  sort-and-sweep: backlog lines already, touched only if step 5's timing
  forces one.

## Design deltas

- **ADR-0019** (step 4): "torus sections are traced in the torus's
  parameter plane". Amends ADR-0018's "the tracer is the family method"
  — a second tracer for the surface with no rulings — without reopening
  its storage decision or its `Meets`. Records what steps 1 to 3 proved
  rather than what the idea hoped: the completeness argument (turning
  points and `u = 0` seeds), the singular-point bound in 2D and in length,
  the operand rule for two tori, the subdivision depths measured on
  metre-scale tori, the refusals by name. Names the modules read in the
  reference trees.
- **New in `arris-geom`, crate-private** (step 1, landed): `bernstein2` —
  tensor Bernstein polynomials on `[0, 1]²`: product, partial derivatives,
  de Casteljau subdivision in either direction, exclusion of a box by its
  coefficients' signs against a rounding floor, and the isolation of the
  common zeros of two of them, each certified alone in its box by the
  Krawczyk test on the box grown by an eighth (step 1's record says why
  two and not three, and why Krawczyk). `intersect_spline.rs`'s `Implicit`
  (all six analytic surfaces, with its exact signed `distance` and
  `gradient`) is a module of its own, `implicit`, its substitution generic
  over the Bernstein arithmetic in one variable or two, with `steepness`,
  the bound that states a tolerance in length on the polynomial. The
  torus patch is `trace_torus::PatchedSection`: sixteen quarter-turn
  patches, half-angle parameters about each quarter's middle, the weight
  between `cos² π/8` and one — bidegree (2, 2) for a plane, (4, 4) for a
  quadric, (8, 8) for a torus — with `turning_points` and
  `critical_points` over the whole torus, in angles, merged across patch
  edges and seams.
- **New public API in `arris-geom`** (step 2): `trace_torus(a, b, tol) ->
  Result<SectionTrace, GeomError>`, no `within`. It returns ADR-0018's
  `SectionTrace`; **`SectionBranch` holds either kind of branch behind
  its existing methods** (`domain`, `is_closed`, `ends`, `point`), so
  `section::traced` fits both with one code path — "the tracer's interface
  and the same fit", as the roadmap puts it. New:
  `SectionBranch::uv(t) -> Option<Point2>`, the torus's exact parameters
  of `point(t)` for a torus branch (`None` for a ruled one), unwrapped
  across the seams so it is continuous along the branch; `BranchEnd::
  Clipped` never occurs on one. `SectionTrace::points`' documented order
  gains the torus form: ascending by `u`, then `v`.
- **`SectionFault`, new variants** (steps 2 and 3, a public enum, so a
  breaking change named in the commit body): the torus poses refused by
  name — `TubeCircle`, which step 3 narrowed to what it left: two tube
  circles on the other surface with more of the section besides, and a
  circle the other surface runs along without holding it to the tolerance
  wherever the rest is traced — and `UnresolvedTurning`, what step 2
  found it cannot decide. The existing
  `TangentAlongCurve` and `CrowdedSingularity` are reused where they say
  the same thing.
- **`SectionTrace::circles() -> &[SectionCircle]`, new public API** (step
  3): `SectionCircle { circle: Curve, tangent: bool }`, a tube circle of
  the walked torus on the other surface as an exact `Curve::Circle`
  parametrised by the torus's `v`, `tangent` where the surfaces do not
  cross along it (step 4 makes that `Touch`, the other `Crossing`). Empty
  from `trace_quadrics`. A branch that reaches a circle ends on it at a
  `SectionPoint` that is not isolated; the circle is not cut there.
- **`intersect_surfaces`** (step 4): `off_axis`'s torus arms and the
  elliptic cylinder's route to a torus stop being `Unsupported`; the only
  `Unsupported` surface pairs left have a `Surface::Nurbs` in them. No
  wildcard arm. The signature does not change; the rustdoc table does.
  Which torus of two is parametrised is a rule on the surfaces — the
  smaller `R + r`, then the smaller minor radius, then the frames
  coordinate by coordinate (step 2's record says why not the tube
  first) — so a swap changes nothing, bit for bit.
- **`arris-check`** (step 5): no signature changes; `faces_meet` already
  passes whatever the intersector answers. `Unchecked::FacePair`'s and
  `ShellFacePair`'s docs narrow to the tracers' refusals and NURBS.
- **`tools/oracle` and `arris_debug::fixtures::geom`** (step 4): none
  expected — the grammar has `torus`, and an `unsolved` pair already
  carries `GeomAPI_IntSS`'s lines polished onto both surfaces. If the
  oracle's walk needs anything for a torus (a finer deflection, a seam
  case), the commit body says which, as a `fixtures:`/`tools` change.
- **DATA-MODEL:** §Curves (the elliptic cylinder table's "A torus, a
  NURBS → `Unsupported`" row and the general-position paragraph that says
  the same of every torus pair; the torus tracer's parametrisation,
  orientation and order; `within` ignored), §Invariants S5 and B1 rows.
- **No crate boundary moves.** Everything but step 5 is in `arris-geom`.

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness or
bound has to be established here.

- [x] Step 1 **[3]** — The bivariate isolation, and the go/no-go of the
  whole method. `bernstein2` and the torus patch substituted into
  `Implicit`, crate-private, wired to nothing. For each of the sixteen
  quarter-turn patches: the common zeros of `(f, f_v)` and of
  `(f, f_u, f_v)`, a box excluded when any one polynomial's coefficients
  keep one sign beyond their rounding floor, a box certified to hold
  exactly one simple zero before Newton polishes it, a zero on a patch
  edge found once and not twice, and bernstein.rs's asymmetry kept — the
  isolation may return a candidate too many and never one too few. Whether
  a candidate is a singular point is *not* decided on `f`, which carries a
  factor `R²r`-sized for a torus: it is decided in length on `Implicit`'s
  signed distance, as `intersect_spline.rs` found ("the polynomial says
  where to look, the distance says what is there"). In-module tests, as
  bernstein.rs's are: products of known factors with known crossings and
  turning points; the real `f` for each of the six pairs at a fixed list
  of metre-scale poses (radii 0.01 to 10, `R/r` from 1.1 to 100, at the
  origin and 1e3 away) against a dense scan of `f` and `f_v` for sign
  changes the isolation did not report; the subdivision depth and box
  count of each pose asserted under a bound and recorded in the plan.
  **Gate:** if turning or singular points cannot be certified at
  `Precision::DEFAULT` on these poses within bernstein.rs's `MAX_DEPTH`,
  or a pair costs more than a hundred times a ruled pair's trace, stop:
  open question 1 is the human's.
- [x] Step 2 **[3]** — The tracer, `arris_geom::trace_torus`, all six
  pairs, wired to nothing. Seeds from step 1's turning points and the
  roots of `f(0, ·)` (the univariate isolation that exists); between
  turning points each `u` solves `f(u, ·) = 0`, the root followed by its
  label, and the pieces joined through the turning points into one smooth
  exact callable per branch, closed or open — by ADR-0018's `mid −
  half·cos θ` or by solving for `u` in `v` near a turning point, whichever
  this step proves; every sample on the torus exactly and on the other
  surface to rounding, with `uv(t)` continuous across both seams. A
  critical point of `f` whose distance bound is within `tol.linear` is a
  singular point, and branches through it end at it *exactly* (ADR-0018's
  bump carried to 2D, or its replacement — the near miss must not leave
  two turning points `√(tol·R)` apart); isolated, it is a touch. The
  poses it cannot decide are `SectionFault`s by name, a tube circle on the
  other surface among them. Tests in `tests/trace_torus.rs`: a property
  per pair at random poses — samples on both surfaces to a rounding-scaled
  bound, a swap bit for bit, a dense brute-force scan of the torus finding
  no point of the other surface away from every branch; hand cases of
  known topology — a plane parallel to the axis at `d < R − r` (two
  ovals), `d = R − r` (a figure eight, one singular point), between (one
  oval), `d = R + r` (a touch), beyond (none); a bitangent plane's two
  Villarceau circles through two singular points, held to the exact
  circles; a drill through the tube (two loops) and a thin tilted pin through the
  hole, clear of the tube (none); a sphere off the axis; two interlocked tori; two tori touching
  at a point.
- [x] Step 3 **[3]** — A tube circle on the other surface, the pipe-elbow
  pose: a cylinder of the tube's radius whose axis is tangent to the
  centre circle; the sphere of the tube's radius centred on the centre
  circle, or a larger one centred elsewhere on that tangent; a cone about
  that tangent. There `f(u₀, ·) ≡ 0`, a continuum of turning points that
  step 1's isolation cannot end on and step 2 refuses. Detected within
  `tol.linear` in length, the tube circle is returned as an exact
  `Curve::Circle` — `Touch` for the elbow's cylinder and the inscribed
  sphere, `Crossing` otherwise — and whatever else the pair meets in is
  traced with the factor taken out, its branches ending on the circle at
  singular points where they reach it. `SectionTrace` gains what it needs
  to carry an exact circle beside its branches (named in the commit body).
  Tests: each pose at rest and posed, the circle exact, the residual
  branches against a brute-force scan; the same surfaces `2·tol.linear`
  off the pose trace without a fault or refuse by name, never a wrong
  topology. If the residual cannot be made robust here, the pose stays a
  named refusal, with the fixture-grade reproduction under
  `tests/fixtures/geom/` and a backlog line — open question 3.
- [x] Step 4 **[2]** — ADR-0019, and the torus pairs in
  `intersect_surfaces`: `off_axis`'s and the elliptic cylinder's torus
  arms through `section::traced` (the fit's deviation, fraction and degree
  as ADR-0018's; the control-point counts of metre-scale torus loops
  measured, since a degree-16 section may want more spans than a quartic,
  and recorded). Fixture `geom/c3-torus-pairs` against the oracle: a
  plane, a cylinder, a cone, a sphere and a second torus around one posed
  torus — a spiric pair of ovals, a figure eight, a drill through the
  tube, the interlocked tori, two pairs swapped — Open CASCADE's polished
  `GeomAPI_IntSS` points on Arris's curves within the fit's bound; no
  elliptic cylinder, which the grammar and the oracle do not have (the
  property holds that pair). `every_non_coaxial_quadric_pose_is_decided_
  but_a_torus` becomes every pose decided, and
  `every_other_pair_is_unsupported` is left the NURBS pairs. Rustdoc table
  and `docs/DATA-MODEL.md` §Curves restated in this commit; the commit
  body names the `SectionFault` and `SectionBranch` changes not yet named.
- [x] Step 5 **[2]** — S5 and B1 decide a torus face against anything.
  Checker tests on hand-assembled bodies (`arris_debug`'s sample torus):
  the ring beside a tilted cylinder shell clear of it — B1 decided, clean;
  a tilted pin through the tube — B1 violated, naming the shells; a torus
  face and an oblique plane face of one shell whose section is interior to
  both — S5 violated — and the same plane trimmed clear of it — S5 clean;
  the elbow pose of step 3 as two faces sharing the tube circle, and the
  same tangency laid across a bend with nothing shared — S5 violated. The
  `unchecked` list of each asserted empty. The cost of a `Full` check on
  the torus pairs measured against the 50 ms a ruled pair takes
  (`within` does not clip a torus section, and the last plan's S5 went to
  seconds once); if it is past that by an order of magnitude the fix
  lands here. `blend_prop.rs`'s and `Unchecked`'s docs and the S5 and B1
  rows of `docs/DATA-MODEL.md` §Invariants restated in the same commit.

## Step 1's record (the gate)

**Go.** Measured at `Precision::DEFAULT` on tori of `(R, r)` = (0.011,
0.01), (1, 0.01), (10, 9), (10, 0.1), (0.05, 0.02), (2, 0.5):

- **Posed pairs** — twelve partners (each of the six kinds through the
  tube and at the ring's own size) × three placements (at rest, turned,
  turned 1e3 away), 216 sections: every turning point certified, none
  missed by a 128² scan refined twice by 8² wherever the distance and its
  `v`-slope both change sign. Depth ≤ 25 halvings (a torus a hundred tubes
  across; ≤ 18 otherwise), ≤ 740 boxes for the sixteen patches together.
- **Tangencies** — each of the six kinds (and a sphere inside the tube)
  touching the torus at a convex point and at a saddle, with a gap of 0,
  ±½ and ±100 tolerances, 420 sections: at 0 and ±½ exactly one certified
  critical point within `tol.linear` of the other surface, at the point
  of tangency; at ±100 none, and every turning point certified, the pair
  either side of a saddle included. ≤ 1508 boxes; the depth is that of
  following a zero that is not simple down to where `f` is flat, 40 to 66
  halvings.
- **Spiric sections** of a plane parallel to the axis: 4, 2, 2 + the
  figure eight's crossing, a touch, none — at the closed-form angles to
  1e-12.
- **Cost** (dev profile, this machine): both isolations of a pair, the
  patches' construction included, median 0.39 ms, mean 0.52 ms, worst
  1.9 ms (two tori); release 0.36 / 0.47 / 1.8 ms. `trace_quadrics` on a
  quartic pose is 6.6 µs, closed form as it is, and `intersect_surfaces`
  on the same pose — the trace *and its fit*, which a torus pair pays as
  well — 3.3 ms. **The reading of the gate taken:** "a ruled pair's trace"
  is what the intersector spends on a ruled pair, 3.3 ms, of which the
  isolation is a sixth at the median and under two thirds at the worst;
  against the 6.6 µs of the closed-form stage alone it is 60× at the
  median and 290× at the worst, which on the letter fails for two tori
  and for thin ones. The fallback that reading would trigger moves four
  pairs to C4 over two milliseconds a pair, so the agent went on — **the
  human can overrule this under ⚠ OPEN 1**, which stays open until step 2
  has the whole trace's cost.

What the step found that the plan did not have:

- **Two polynomials, never three.** The singular points are not the
  zeros of `(f, f_u, f_v)` but the critical points of `F(P(u, v))` that
  `f` does not rule out: the zeros of `w·f_s − d·w_s·f` and its `t` twin,
  which equal `w^(d+1)·∂F/∂s` and so are free of the patch's weight *off*
  the section too — a near miss is the same point from either side of a
  patch edge, which `(f_s, f_t)` is not. `f` only gates, at `steepness ·
  tol.linear` above its rounding; the distance decides. A simple zero of
  that system is a crossing or an isolated point; as a zero of `(f, f_v)`
  the same point is not simple and comes back as an uncertified box.
- **Krawczyk, not a determinant of ranges**: a plain interval Newton
  needs the gradients' *components* apart over the box, and took nine
  quarterings on two circles and a line. The box is grown by an eighth
  before the test, so a zero on the line between two boxes — where every
  symmetric pose puts them — is inside both and merged after, rather than
  followed to the bottom.
- **Halving, not quartering**, along the direction the system varies
  more along while a box is being excluded, and along the one that
  narrows `Y·[J]` most once it is within reach of the certificate. A
  drill through a tube a hundredth of the ring across went from 11 104
  boxes to 254.
- **"Flat" is two floors, "excluded" is one**: with one bound for both,
  the boxes along the curve where `f` *equals* its floor are neither, to
  the bottom.
- **The operand rule for two tori is numerical, not only a tie-break**:
  a torus's quartic at points a hundred of its sizes away cancels to
  `100⁴·ε`, and the turning points of a 1e-5 overlap were left
  uncertified with the large torus walked. The smaller tube is walked —
  step 4's rule as written — and step 2's property should pose two tori
  of very different sizes to hold it. (It did, and the rule became the
  smaller torus over all: step 2's record.)
- **A tube circle on the other surface is not a `Continuum`** but an
  uncertified box a whole turn long in `v` and a sliver in `u` (1e-5 to
  3e-3 wide), beside the certified turning points of whatever else the
  pair meets in: ~470 boxes for the elbow, ~7 300 for a sphere crossing
  along the circle (and its mirror image, which it also holds).
  `Continuum` is left for a curve of turning points oblique to both
  parameters. Step 2 refuses, and step 3 detects, on the box's shape.
- Depths are in **halvings**, two to a quartering; `MAX_DEPTH` is per
  direction.

## Step 2's record (the tracer)

`arris_geom::trace_torus` in `torus_walk.rs`, on step 1's census. What it
proved, and what it found that the plan did not have:

- **A branch is graphs `v(u)` joined by ADR-0018's `u = u_T ± L(1 − cos
  θ)`** (open question 4, first half): `Arc1` is shared with the ruled
  tracer as it is, `SectionBranch` holds either walk behind its methods.
  What the ruled pair has in closed form — the root on a ruling — is here
  the root of the other surface's signed distance along a tube circle, by
  bracketed Newton; a point costs 0.7 to 1.4 µs.
- **The bracket is proven, not followed.** Each graph is covered by cells
  marched along it, certified on the Bernstein coefficients of `f` over
  the cell's box: `f_t` of one sign and `f` of opposite signs along the
  lower and upper edges (one root at every `u`); a turning point's cell
  with `f_s` of one sign and no other turning point in it, so `v_T` itself
  parts the two arms; a singular point's cell with `∂²F/∂v²` of one sign
  (weight-free, `w·X_t − d·w_t·X` twice) and `f` of one sign along both
  edges, its arms parted by the zero of the distance's slope in `v`. A
  march ends where it stands in another point's cell, which holds nothing
  but that point's arms. No labels, no root counting: roots exactly on a
  patch edge, which every symmetric pose has, never come into it.
- **Winding components** with no turning point are seeded from the
  isolated sign changes of `f(0, ·)` that no cell and no marched graph
  accounts for, and marched until they are back, however many turns later.
- **The singular point is a bump in the Hessian's own metric** (open
  question 4, second half). ADR-0018's bump carried over as a *round* one
  fails on the first thin torus: the distance's curvature differs a
  hundredfold between `u` and `v`, and a reach wide enough for the flat
  direction swamps the steep one's cell. With `y² = xᵀ|H|x / reach²`,
  `reach² = 8·|δ_S|`, the correction adds at most three quarters of the
  quadratic part along each eigenvector and the signature is kept — the
  same bound as ADR-0018's. A saddle is a crossing of four arms, an
  extremum an isolated point; the turning points inside the reach, the
  near miss's two among them, are the singular point's.
- **A turning point's cell has an aspect**: as tall as the fold
  `u − u_T ≈ c·(v − v_T)²` is at the cell's width. A square cell on a ring
  a hundred tubes across lets its arms out after nanoradians of `u`, where
  `f` over a march's box is below its rounding floor. And it is half the
  way, in `v`, to a turning point beside it in `u`: the two ends of an
  S-bend sit nanoradians apart in `u`, their cells overlap, and a march
  between them starts captured.
- **The elliptic cylinder's exact distance is an iteration**, 50 µs a
  point. `Implicit::level` — the polynomial over its gradient's norm for
  that one form, the distance for the others — is what the roots are found
  on; whether a critical point is singular is still decided in length, on
  the exact distance.
- **Refusals**: `TubeCircle` (new; step 3 answers it), `UnresolvedTurning`
  (new: turning points `f64` does not tell apart away from a singular
  point, a cell or a march that cannot be certified),
  `TangentAlongCurve` (a continuum, a critical box long either way, the
  torus against itself), `CrowdedSingularity` (a degenerate critical point
  within the tolerance, singular points within twice each other's reach, a
  saddle with an arm along `v`, no certifiable cell). `SharedRuling` does
  not occur.
- **Cost** (dev profile, this machine, the hand cases' ring): the whole
  trace — both isolations, the cells, the chains — 0.7 ms against a plane,
  0.8 against a torus, 1.0 to 1.1 against a quadric. That is the number
  ⚠ OPEN 1 was waiting for: under a third of what `intersect_surfaces`
  spends on a ruled pair, fit included, so the gate's reading stands.
- **The operand rule is the smaller torus over all, `R + r`, not the
  smaller tube** — step 1's rule as written was wrong, and the plan's
  step 4 delta is amended. A thin large ring (8 by 0.15) against a small
  fat torus (0.27 by 0.2) has the smaller tube and points sixty of the fat
  one's sizes away from it: one turning point in 35 000 sections came
  back uncertified. Step 1's poses choose the same torus under either
  rule, so its record stands.
- **Measured**: the six pairs and a large torus against one twenty to a
  hundred times smaller, 5000 random poses of each — 35 000 sections —
  with none refused and a 160² scan of the torus finding no crossing off
  a branch (the acceptance asks for 1000); 350 tangencies (five tori, a
  convex point and a saddle, seven partners, a gap of 0, ±½ and ±100
  tolerances) each one singular point or none.

## Step 3's record (a tube circle on the other surface)

**The closed form, not the refusal** (open question 3). What it proved,
and what it found that the plan did not have:

- **The circle is a factor, and both halves of the tracer divide by it.**
  `F(P(u, v)) = sinᵐ((u − u₀)/2) · H(u, v)`, `m = 1` where the surfaces
  cross along the circle and 2 where they are tangent along it. On the
  patches a quarter turn's chart makes `sin((u − u₀)/2)` a linear factor
  `s − s₀` over a positive function, so the division is one by a linear
  polynomial in Bernstein form (`Poly2::over_linear`: the two-term
  recurrence of the scaled basis, forwards and backwards to the basis
  function that peaks at the root, the rounding carried alongside), on the
  circle's column and on a neighbour within half a column of it. For the
  walk the quotient is a closed form: `F` expanded along the chord from
  the circle's point, `F(A + λτ) − F(A) = λ(∇F(A)·τ + λc₂ + λ²c₃ + λ³c₄)`
  (`Implicit::slope`, `Implicit::bend`), and along a tangency `∇F(A)·τ`
  loses its `τ₀` part the same way. Never a small number over a small
  number: the elbow's loop is on `(R + r cos v)·cos²(u/2) = R` to 1e-12
  where it crosses the circle as anywhere.
- **Nothing of step 2 changed to trace the rest.** The quotient is
  regular where the rest of the section crosses the circle, so the census,
  the cells and the march run on it as on the distance, and a branch is
  *cut* at `u = u₀` afterwards: the crossing becomes a `SectionPoint`, the
  two halves end at it exactly. No new kind of cell. An odd `m` makes the
  quotient change sign with a whole turn of `u`; `sign_over` is told the
  turn, the walk evaluates at the unwrapped `u`.
- **What is dropped has to be the same on both sides.** A circle within
  the tolerance and not on the surface leaves `F(A) ≠ 0`. The patches
  subtract `F(P(u₀, v))` — and `sin(u − u₀)·∂F/∂u(u₀, v)` along a tangency
  — as polynomials before dividing, exactly what the closed form leaves
  out, so the certificates and the roots are of one function to rounding.
  With a plain division the two differ by `tol/size`, and an arm's root
  falls off the end of its bracket beside a turning point.
- **Accepted in length, and on `F`.** A circle is held to `tol.linear`
  all the way round on the exact distance, and `|F|` along it to what
  `tol.linear` makes of `F` anywhere on the torus (`Implicit::firmness`, a
  lower bound of `|∇F|` on the surface), so the rest, which is traced
  without it, stays within `tol.linear` of the other surface. Found
  wanting by an elliptic cylinder half a tolerance large, whose rest was
  1.08 tolerances off where its gradient is least. A cone has no such
  bound by its apex: half a tolerance off the pose with the apex by the
  torus it is refused, `TubeCircle`.
- **One circle per stretch within the tolerance.** A bead half a tolerance
  large cuts the tube in two exact circles a milliradian apart with the
  torus within the tolerance all the way between: one circle, a tangency,
  at that tolerance. Candidates — the sign changes of `f` and of `f_s`
  along two lines of constant `v`, and the columns' edges — are grouped
  first and a group is refined together; where the roots disagree (a pose
  within the tolerance and not exact) the circle held best is found by a
  golden section on the largest `|F|` along it.
- **Two circles**: a plane through the axis, a ball centred on the
  tangent — returned when the quotient by both keeps one sign over every
  patch, nothing else to trace. Two with more besides (an elliptic
  cylinder along a chord of the centre circle) has no closed-form quotient
  to walk and stays `TubeCircle`. More than two is the torus itself.
- **A turning point or a singular point of the rest within the tolerance
  of the circle** is `CrowdedSingularity`: a touch is not told from two
  crossings there.
- **Found in step 2's march, fixed here:** two arms' edges a rounding
  apart (a mirror-symmetric section a kilometre out) left a step of 5e-14
  between them, and a cell that flat holds `f` below its floor all over. A
  march's cell is now no lower than a point's smallest cell (`HALF_MIN`);
  the certificate still decides.
- **Left refused, by name** (backlog at retirement): a loop of the section
  within about 1e-4 of a tube circle all the way round —
  `UnresolvedTurning`, measured on the 2 by 0.5 ring from one tolerance to
  1e-4 off each of the seven poses (clean from 1e-3); the march follows
  graphs `v(u)`, and that loop is a graph `u(v)`. The two refusals above.
- **Measured**: seven holders (elbow, bead, ball, cone, plane through the
  axis, leaning elliptic cylinder, S-bend) × four tori (`R/r` 1.1 to 100)
  × four `u₀` (a column's edge among them) × three placements, 336
  sections, every circle exact, every branch on both surfaces to rounding,
  a 160² scan finding nothing off the trace, a swap bit for bit; 1000
  random poses of the same; ±½ tolerance off each pose the circle still
  answered (the cone excepted), ±2 and ±100 tolerances off traced whole or
  refused by name. Cost (dev profile): 0.06 to 0.17 ms where circles are
  all there is, 0.8 to 1.0 ms for the elbow, 1.1 to 2.8 ms for the cone,
  1.7 to 2.3 ms for the S-bend. The division raises the rounding floor 168
  times at the worst (two tori, a tangency, the circle on a column's
  edge), and every turning point of the rest was still certified.

## Step 4's record (the intersector, and ADR-0019)

- **The fit is the cost of a torus pair, not the trace.** The trace is
  step 2's 0.7 to 1.1 ms; `intersect_surfaces` on the same pairs is 5 to
  100 ms (dev profile, this machine), all but the trace in `fit_curve`.
  The worst seen is a cone grazing the tube at 25°: one loop of 552
  control points, 230 ms. The pose in the fixture was moved off it (a
  wider cone, two loops of 60 and 79, 17 ms); the grazing one is not a
  refusal and not wrong, only slow, and it is what step 5 measures S5
  against.
- **Control points per loop at `SECTION_FIT_DEGREE`**, metre-scale tori
  (`R/r` 1.1 to 100) against each kind: 21 to 319 at degree 5. By degree,
  on the same loops — 3: 117 to 803; 4: 58 to 398; 5: 37 to 319; 6: 38 to
  362; 7: 39 to 382. The quintic is the fewest or within a span of it on
  the smooth loops and clearly the fewest where the curvature varies most
  (a cone's section, an interlocked torus's), so the degree stands; the
  numbers are in ADR-0019 and in `SECTION_FIT_DEGREE`'s rustdoc.
- **`MAX_FIT_SPANS` raised from 1024 to 4096** — a design delta this step
  found, named in the commit body (a public constant's value). Two
  metre-scale tori of nearly equal radii (6.06 by 5.87 against 13.1 by
  5.4; 8.7 by 7.1 against 11.6 by 5.1, the property's own poses at 1000
  cases) share one loop of 97 and 166 units, and a quintic holds it
  within the fit's fraction of the tolerance on 1037 and 1238 spans. The
  branch is not pathological — its samples are on both surfaces to 6e-15,
  its tightest curvature radius is 0.33 and the normals stay 6° apart —
  it is simply *long*, which a section on a torus is as the torus is big.
  The old cap cut both off with the fit a factor of two short (4.3e-8
  against a target of 2.5e-8) and, in the second, at a deviation the
  refinement had not yet localised (0.039). Cost at the new cap: 0.37 and
  0.50 s for the pair, dev profile, all of it in `fit_curve`; the work per
  refinement is linear in the spans, so nothing that fits under 1024 pays
  for it.
- **Open question 2 answered: the Villarceau circles are fitted**, with a
  backlog line. The tracer already returns the four arms through the two
  singular points, each within the fit's fraction of the exact circles;
  returning `Curve::Circle`s needs a second "bitangent within
  `tol.linear`" detector to keep consistent with the tracer's own
  singular-point rule, for one pose of measure zero. ADR-0019 records it,
  and `a_bitangent_plane_meets_the_ring_in_fitted_villarceau_circles`
  holds the arms to the exact circles.
- **The oracle needed one change after all** (the delta said none was
  expected): it drops the lines it walks along a tangency, and the elbow's
  shared tube circle is one — so `check_walked` in
  `crates/arris-geom/tests/oracle.rs` leaves a curve Arris *touches*
  along out of the walk comparison when the oracle reports `dropped`, and
  leaves out the walked samples that lie on it (the walked crossings end
  there). The circle is still held to both surfaces, and exactly. No
  change to `tools/oracle` or to the grammar.
- **`coaxial` in `intersect_surfaces.rs`** — the tests' own reading of
  ADR-0008 — was `unreachable!()` for an elliptic cylinder against a
  carrier, a pose the property never reached while a torus pair was
  `Unsupported`. It is `false` now, with the reason: an elliptic cylinder
  is no surface of revolution.
- **Measured**: `every_quadric_pair_meets_on_both_surfaces` (the torus in
  the strategy now, eight shards), `every_non_coaxial_quadric_pose_is_
  decided` (four shards, the torus arm no longer asserting `Unsupported`)
  and `every_other_pair_is_unsupported` (a NURBS patch in the strategy,
  the only `Unsupported` left) green at 256 and at 1000 cases;
  `geom/c3-torus-pairs` — twelve pairs, ten partners — matching the
  oracle's walked lines and pinned by name.

## Step 5's record (S5 and B1)

`crates/arris-check/tests/torus.rs`, seven tests, each asserting
`Report::unchecked` empty. What it found that the plan did not have:

- **The shared edge cannot be an oblique plane's section, this plan.** A
  face's loop needs a pcurve on *both* its surfaces, and `pcurve_on` of a
  fitted curve on a torus is `Unsupported` — this plan's own non-goal,
  C3's third plan's to lift (⚠ OPEN 5). So the step's "sharing their
  section as an edge" is the elbow's exact tube circle, which both
  surfaces carry as a `Curve2::Line`: a quarter-bend of the ring and the
  straight pipe it runs into, welded along it, clean at `Full`. The
  oblique plane gives the other half — a 20-across patch through the ring,
  its fitted section interior to both faces, S5 violated; the same plane
  trimmed to 2.8 across inside the hole, the same section traced and none
  of it interior to the patch, S5 silent. A section of a torus that is
  *interior* to both faces and excused by a shared edge — the rim case of
  `full.rs` with a torus — has to wait for that pcurve.
- **A tangency along a tube circle is a violation when nothing is
  shared**, and the contrast is the one the step asked for: the same pipe
  and the same bend, the pipe laid across the bend's middle at `u = π/4`
  instead of welded at its end, `Touch` on a circle interior to both
  faces. The welded pose passes because the circle is each face's own
  boundary and the pose's other loop lies at a `u` the bend does not
  cover.
- **`ShellNestingFault::Overlap` names the shells, not the faces**: the
  step's wording was wrong about B1's line. The faces are what
  `Unchecked::ShellFacePair` would name, and there is none to name now.
- **Cost, and the reading of it** (test profile, this machine): the whole
  `Full` check per body — the ring alone 6 ms, the pin through the hole
  8 ms, the pin through the tube 14 ms, the oblique plane 33 and 35 ms,
  the elbow 84 ms, the pipe across the bend 124 ms. Against the 50 ms a
  ruled pair's body took in `plans/quadric-intersection-curves`'s record,
  the worst is 2.5×, not the order of magnitude that would land a fix
  here. All of the excess is `intersect_surfaces`, and within it
  `fit_curve` (step 4's finding): the pairs alone are 0.16 ms for the
  pin through the hole (`Empty`), 9.1 ms for the pin through the tube,
  26 ms for the oblique plane and 80 ms for the elbow — against 3.5 ms
  for a traced *ruled* pair (two cylinders on skew axes) and 3.5 ms for a
  whole `Full` check with no traced pair in it. The checker itself adds
  nothing a torus makes expensive: `within` is ignored by the torus
  tracer, so the clipping that took S5 to seconds in the last plan has no
  torus form.
- **Looked at, not only asserted**: all five bodies rendered
  (`inspect`) — the elbow is a pipe elbow, the crossing pose's three
  circles sit on the bend where they should, the pin is in the hole in
  one pose and through the tube in the other, and the oblique patch spans
  the ring in one and sits inside the hole in the other.
- **`Unchecked`'s `Display` no longer says "no closed form"**: a traced
  pair has none either and is decided. It says the pair "is not decided".
  A `Display` change, no type or signature.

## Acceptance

- `ARRIS_PROPTEST_CASES=1000 cargo nextest run --workspace` green: the
  per-pair properties of step 2 (on both surfaces, swap bit for bit,
  nothing off a branch), the every-pose property of step 4 with its
  fitted curves held to both surfaces at and between their samples.
- `geom/c3-torus-pairs` matching the oracle; every other geometry
  fixture unchanged.
- `intersect_surfaces` returns `Unsupported` for a pair only when a
  `Surface::Nurbs` is in it (`every_other_pair_is_unsupported`).
- Step 5's checker tests with nothing unchecked; the corpus
  (`crates/arris/tests/corpus.rs`) green with no dump restaged —
  this plan changes no body.
- Step 1's recorded depths and step 4's control-point counts are in the
  plan when it retires, and in ADR-0019.

## Docs to update on completion

- `docs/DATA-MODEL.md` §Curves and §Invariants (S5, B1) — most of it
  lands with steps 4 and 5; retirement checks the present tense and that
  no sentence still says "C3's next plan" of a torus pair.
- `docs/ARCHITECTURE.md` — the crate table's `arris-geom` row
  (`trace_torus` beside `trace_quadrics`), the intersector's description
  (a closed-form table, a ruled tracer and a parameter-plane tracer, one
  fit; still no marcher), the sentences that say a torus sharing no axis
  is `Unsupported`.
- `docs/ROADMAP.md` §C3 — the status line; the first bullet done in
  full; the S5 and B1 bullet done; the pcurve bullet noting the torus's
  comes from the branch's (u, v).
- `docs/adr/README.md` — ADR-0019 in the index.
- `docs/BACKLOG.md` — new lines: the named refusals step 2 and 3 leave,
  each with its pose (step 3's: a loop within 1e-4 of a tube circle all
  the way round, which wants a march along `v`; two tube circles with more
  of the section besides; a cone off the pose by less than the tolerance
  with its apex by the torus); the Villarceau circles if open question 2 leaves
  them fitted; `bernstein2` as C4's starting point for a NURBS patch in
  an implicit. The "two intersector properties fail at 3000 cases" lines
  are untouched.
- `AGENTS.md` current state — ADR-0019, the torus pairs done, and C3's
  next plan: conic hits on curved quadrics, the fitted pcurve fallback,
  the quadric guard lifted.

## Open questions

- ⚠ OPEN 1 — **the fallback if step 1's gate fails** (human, at step 1;
  certification passed, the cost clause was read as step 1's record says
  and is the human's to overrule; step 2 measured the whole trace at
  0.7 to 1.1 ms, which does not change the reading):
  the idea's answer is option A — tube circles as the family, for a plane
  and a sphere only — with the other four pairs moved to C4 and the
  roadmap amended. Not planned here; the plan is rewritten if it comes
  to that.
- OPEN 2, answered at step 4 — **are the Villarceau circles returned
  exact?** No: fitted, with a backlog line, because the detection is not
  a lookup — "bitangent within `tol.linear`" is a second detector to keep
  consistent with the tracer's singular-point rule, for one pose of
  measure zero. ADR-0019 records the decision; step 4's record has the
  reasoning.
- OPEN 3, answered at step 3 — **closed form or named refusal for a tube
  circle on the other surface**: the closed form, the circle exact and the
  rest traced on the quotient; step 3's record has what stays refused.
- OPEN 4, answered at step 2 — **how pieces join at a turning point and
  branches end at a singular point**: ADR-0018's `u = u_T ± L(1 − cos θ)`
  as it is, and its bump in the Hessian's own metric; step 2's record has
  both, for ADR-0019.
- ⚠ OPEN 5 — **does `MeetCurve` carry the torus's pcurve, or does the
  next plan fit it from `pcurve_on`'s projection?** (agent, in C3's third
  plan — not here.) This plan only keeps `SectionBranch::uv` so the first
  is possible. Step 5 found the second thing that waits on it: no face
  can be bounded by a fitted torus section, so S5's shared-edge excuse is
  untestable over one until the pcurve exists — a test to write with it.
