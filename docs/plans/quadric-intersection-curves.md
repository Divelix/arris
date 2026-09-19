# Plan: quadric-intersection-curves

- Started: 2026-09-19
- Milestone: C3, every quadric pair (docs/ROADMAP.md §C3, the first
  bullet and its accept line; the first of C3's plans)
- Idea (verbatim from the human): "read
  @docs/ideas/quadric-intersection-curves.md, review it and if it is
  correct, choose option and decide on open questions and /plan it"
- Idea: docs/ideas/quadric-intersection-curves.md (absorbed)

## Goal

Two quadrics in any pose meet in `intersect_surfaces`, as long as one of
them is ruled (a cylinder, a cone, an elliptic cylinder — every quadric
pair there is but sphere–sphere, which is a circle already). The three
decisions of the idea are taken, on the human's delegation, as it
recommended: **a fitted `Curve::Nurbs` (B)**, **one `Meets { curves,
points }` result for every meeting (i)**, **the family tracer over closed
forms**, with the procedural curve (C) recorded as the upgrade path on
B's charts. The review of the idea added four things it left out, all
part of the decision:

- **A fit needs a bounded curve, and a quadric section need not be one.**
  A plane cuts a cone in a hyperbola, and two cones on different axes meet
  in unbounded branches in an open set of poses. `intersect_surfaces`
  therefore takes a region (`within: &Aabb`): every branch is clipped to
  the region's extent along the walked rulings, and a loop inside it stays
  closed. A boolean passes one region for the whole operation (the overlap
  of the operands' boxes, grown generously — it only has to bound what
  runs to infinity), so every face pair on the same two surfaces gets the
  same curve bit for bit.
- **A closed branch is a periodic B-spline.** The pave model cuts a closed
  section curve at its paves only and wraps the last block round; a
  clamped closed fit would put a joint vertex where the oracle has none.
- **A conic is never fitted.** A plane's parabola or hyperbola on a cone
  over the region is an exact rational quadratic `Curve::Nurbs`; an
  ellipse stays `Curve::Ellipse`. Only the quartics are fitted.
- **The fit target is a named fraction of the pair's tolerance, below
  `FIT_MARGIN`'s half**, because the pcurve fitted afterwards on each face
  can do no better than the 3D curve's own distance from that face, and
  has to land within the same tolerance. That is what keeps a section
  edge's tolerance at its faces' and never above.

When the plan is done: the quartic pairs are property-tested at random
poses, points on both surfaces within the tolerance; a boolean of two
cylinders on crossing axes of unequal radii, or on skew axes, passes every
corpus stage against Open CASCADE; a second boolean cuts through the
fitted edge the first one left, with no tolerance grown; S5 and B1 decide
those pairs instead of listing them unchecked; and `docs/DATA-MODEL.md`
§Curves has no `⚠ OPEN`.

## Non-goals

- Every pair with a torus in it, the spiric section included: degree
  eight, no algebraic discriminant, and it needs the conic–torus hits the
  roadmap lists on their own. C3's next plan, on this plan's tracer
  interface and fit.
- Lifting the pave model's quadric guard, booleans with cone, sphere,
  torus or elliptic-cylinder operand faces, and the fitted pcurve
  fallback on cones, spheres and tori they need. The boolean fixtures
  here have cylinder and plane faces only, which the guard already lets
  through. C3's next plan.
- Features a tolerance apart: section vertices clustered by closure,
  `regression/seam-a-tolerance-from-crossing-fuse`, the touching and
  coincident corpus. Here a near-singular pose is decided only as far as
  "one singular point or none, within `tol.linear`".
- `Curve::Nurbs` against `Curve::Nurbs` in `intersect_curves`: stays
  `Unsupported`, a backlog line for C4, whose marcher needs it first.
- A `Curve::Intersection` procedural variant (C), a
  `Curve::QuadricSection` (A), `Curve::Parabola`/`Hyperbola` variants.
  The first is a backlog line with the two tripwires the idea names.
- NURBS surface operands (C4).

## Design deltas

- **ADR-0018** (step 3): quadric intersection curves are fitted NURBS
  under the faces' tolerance, conics exact, traced by ruling families
  inside a region; one `Meets` result. Closes the `⚠ OPEN` in
  `docs/DATA-MODEL.md` §Curves and §Open questions (`SEED.md` §10.7) and
  settles the result type ADR-0008 deferred. Names C as the upgrade path
  and the tripwires: a fixture whose tolerances grow through chained
  booleans, a consumer re-evaluating at a finer precision than the model
  was built at. Written at step 3, not step 1, so it records what steps 1
  and 2 proved — the tracer's completeness argument and the fit's bound —
  rather than what the idea hoped. Modules read in the reference trees
  are named there (`IntAna_QuadQuadGeo`, `IntPatch`'s analytic–analytic
  walking lines, and the approximation of a walking line).
- **`arris_geom::SurfaceIntersection`, a breaking change** (step 3):
  `Transversal`, `Tangent` and `Points` are replaced by
  `Meets { curves: Vec<MeetCurve>, points: Vec<MeetPoint> }`, each entry
  carrying a `MeetKind { Crossing, Touch }`. `Empty` and `Coincident`
  stay. A point where traced branches end (a singular point of the
  section, where the surfaces are tangent) is among `points`, and the
  branches' ends there are that point. Ordering inside both lists is
  specified and deterministic. The names avoid `arris_ops::boolean`'s
  existing `Contact` and `SectionCurve`.
- **`arris_geom::intersect_surfaces`, a signature change** (step 4):
  `intersect_surfaces(a, b, within: &Aabb, tol)`. The closed-form arms
  ignore the region and return their unbounded lines as today. Callers:
  `boolean/pave.rs`, `blend.rs`, `arris-check`'s S5 and B1, the geometry
  fixtures' runner.
- **New in `arris-geom`:** `trace_quadrics` with `SectionTrace`,
  `SectionBranch`, `SectionPoint`, `BranchEnd` and `SectionFault`, and
  `GeomError::DegenerateSection` (step 1 — public, not the private module
  first planned: the crate's property tests are integration tests because
  of the dev-dependency cycle through `arris-debug`, and the exact section
  is what the procedural curve of the upgrade path evaluates); public
  `fit_curve` and its periodic form beside `fit_curve2` (step 2); a named
  constant for the section fit's fraction of the tolerance, with the
  reason above in its comment (step 4).
- **`intersect_curve_surface` and `intersect_curves`** (steps 6–7): the
  `Curve::Nurbs` arms against every analytic surface and against a line,
  a circle and an ellipse stop being `Unsupported`; `curves_coincide`
  answers for a `Nurbs` operand. No wildcard arm is added.
- **The pave model** (step 8): section crossings of a traced pair come
  from the result's `points`, not from `intersect_curves` on two NURBS;
  one region per boolean. `Interferences`' public shape follows `Meets`
  where it names the old variants (`SectionCurve::index`'s doc, the
  `Display`).
- **DATA-MODEL:** §Curves (the table of pairs, the traced curves'
  deterministic parametrisation and orientation, the region, the `⚠
  OPEN` removed), §NURBS (`fit_curve`, periodic fits), §Tolerances (a
  fitted section is *not* a reason an edge's tolerance grows, and why),
  §Invariants S5 and B1 rows, §Open questions.
- **Fixtures.** Step 3 changes no geometry; a blessed dump that prints a
  pair's intersection by variant name is restaged there as `fixtures:`,
  oracle values untouched.

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness or
bound has to be established here.

- [x] Step 1 **[3]** — The tracer, a ruled quadric against any quadric,
  as `arris_geom::trace_quadrics`, wired to nothing. Walk the rulings of
  the ruled operand by their angle `s`; each ruling meets the other
  quadric in the roots of `a(s)v² + b(s)v + c(s)`. With a cone's rulings
  taken from its apex, the discriminant is a trigonometric polynomial of
  degree two in `s` for every ruled kind, so the turning points are the
  roots of one `roots::quartic` and the branch topology is algebraic, not
  sampled: no loop can fall between samples. `a(s) = 0` marks where a
  branch leaves for infinity, clipped by the region. Pieces between
  turning points are joined through `s = mid − half·cos θ`, which removes
  the square-root singularity, so a branch is one smooth exact callable
  `θ ↦ point`, closed or open. A double root of the discriminant within
  `tol.linear`, measured in space and not in the discriminant's units, is
  a singular point: branches split there. Which operand is walked is a
  rule on the geometry, never on argument order. Tests: a property test
  per pair kind at random poses — every sample on both surfaces to a
  rounding-scaled bound, operand swap giving the same point sets, and a
  dense brute-force sweep of rulings finding no hit away from every
  branch; hand cases of known topology (crossing axes `r < R`: two loops;
  skew and partly inside: one loop; axes `R − r` apart: a figure eight,
  one singular point; clear: none; a cone pair with unbounded branches
  clipped to a box); Viviani's curve against its closed form.
- [x] Step 2 **[2]** — `fit_curve` in 3D beside `fit_curve2`, same
  contract (caller's parametrisation, caller's deviation, refinement,
  `Diverged`), and a periodic form whose result's `period()` is `Some`
  and whose image is closed to rounding. Tests mirror `fit_curve2`'s: a
  helix, Viviani's curve as a periodic fit held to its closed form
  between samples, the error arms, determinism.
- [x] Step 3 **[2]** — ADR-0018 and `SurfaceIntersection::Meets`. Every
  caller ported (`meridian.rs`, the closed-form table, S5, B1, the pave
  model, `result.rs`, `blend.rs`, the debug renderer, the geometry
  fixtures' runner) with no change of behaviour; the two results
  ADR-0008 refused for mixing kinds — the meridian arm's, and parallel
  elliptic cylinders that both touch and cross — are returned, each with
  a test, and the pave model keeps refusing them by name until step 8
  reads them. Commit body names the changed type.
- [x] Step 4 **[2]** — Two cylinders in a quartic pose. The `within`
  region on `intersect_surfaces`, every caller passing one; the tracer's
  branches fitted (periodic when closed) with deviation "the farther of
  the two surfaces" and the named fraction of `tol.linear`; singular
  points into `points`. Fixture `geom/c3-cylinder-pairs` with the
  oracle's sampled points on Arris's curves; the roadmap's property test
  for the pair at random poses. `docs/DATA-MODEL.md` §Curves loses its
  `⚠ OPEN` in this commit. Commit body names the changed signature.
- [ ] Step 5 **[2]** — The other ruled pairs: cylinder–cone, cone–cone,
  sphere against a cylinder or a cone off its axis, the elliptic cylinder
  against each. A plane oblique to a cone, or parallel to its axis and
  off it: `Curve::Ellipse`, or the parabola or hyperbola branches as
  exact rational quadratics over the region, split by knot insertion
  where one arc's weight would run away. Fixture `geom/c3-quadric-pairs`
  against the oracle; the property test extended to every pair. After
  this step no quadric pair without a torus is `Unsupported`.
- [ ] Step 6 **[3]** — `Curve::Nurbs` against every analytic surface in
  `intersect_curve_surface`. Each surface has an implicit polynomial; a
  rational span substituted into it is a polynomial in Bernstein form,
  its roots isolated by subdivision on sign variations and polished in
  the bracket. `tangent` decided as the other arms decide it — the
  signed *distance* (the implicit value over its gradient) with an
  extremum within `tol.linear` of zero, two closing crossings being one
  touch — and `Coincident` by the same measure. Tests: a fitted ellipse
  against each surface agrees with the exact `Curve::Ellipse` arm hit
  for hit within the fit's tolerance, at random poses; touches; a curve
  lying on the surface; a geometry fixture against the oracle.
- [ ] Step 7 **[2]** — `Curve::Nurbs` against a line, a circle and an
  ellipse in `intersect_curves`, through step 6 and the conic's plane as
  the existing conic arms go, and `curves_coincide` with a `Nurbs`
  operand. `Nurbs`–`Nurbs` stays an explicit `Unsupported` arm. Tests
  and a geometry fixture as step 6's.
- [ ] Step 8 **[2]** — The pave model over traced sections: one region
  per boolean, `Meets` read by kind, a singular point becoming the
  section vertex that ends its branches, closed periodic section curves
  through the existing wrap. Each section edge's tolerance asserted equal
  to its faces'. Fixtures against Open CASCADE, every corpus stage:
  `boolean/tee-unequal-fuse`, `-cut` and `-common` (crossing axes,
  `r < R`), `boolean/skew-hole-cut` (skew axes, the drill partly outside:
  one loop), `boolean/skew-bore-cut` (skew and fully inside: two loops).
- [ ] Step 9 **[2]** — A boolean through a fitted edge, which is what
  steps 6 and 7 are for: `boolean/tee-unequal-slot-cut` (a box cut
  across the junction curve of the fused tee) and
  `boolean/tee-unequal-drill-cut` (a cylinder through it). The test
  holds every edge of the result to its faces' tolerance: the idea's
  first tripwire, kept as an assertion. M4's identities (volume
  additivity, cut-then-fuse, commutativity) over unequal cylinders on
  crossing and skew axes at random poses in `boolean_prop.rs`.
- [ ] Step 10 **[2]** — S5 and B1 decide every pair steps 4 and 5 added:
  the region from the two faces' boxes, curves and points held to the
  existing interior-to-both rule. `blend_prop.rs` with nothing unchecked
  for two blends' cylinders on skew axes and a corner's sphere against a
  blend cylinder; the S5 and B1 rows of `docs/DATA-MODEL.md` §Invariants
  restated in the same commit.

## Acceptance

- `cargo nextest run --workspace` green at CI's case count, including the
  property tests of steps 1, 4, 5, 6 and 9: every intersection point on
  both surfaces within the tolerance, each curve's image matching both
  surfaces at its samples and between them.
- `geom/c3-cylinder-pairs` and `geom/c3-quadric-pairs` matching the
  oracle; the seven `boolean/*` fixtures of steps 8 and 9 passing every
  corpus stage with nothing unchecked, their section edges at their
  faces' tolerance.
- `crates/arris-ops/tests/blend_prop.rs` with no pair left unchecked
  that has no torus in it.
- `grep '⚠ OPEN' docs/DATA-MODEL.md` finds nothing.

## Docs to update on completion

- `docs/DATA-MODEL.md` §Curves, §NURBS, §Tolerances, §Invariants (S5,
  B1), §Open questions — most of it lands with steps 4, 5 and 10;
  retirement checks the present tense and that no sentence still says
  "C3's" of a pair that is decided.
- `docs/ARCHITECTURE.md` — the intersector's description (a closed-form
  table *and* a tracer with a fit; still no marcher), the pave model's
  region, §Threading if the fit changes what a pair's step reads.
- `docs/ROADMAP.md` §C3 — the first bullet marked done with what is left
  of it (the torus pairs), the curve-hit bullet narrowed to what remains.
- `docs/BACKLOG.md` — new lines: `Nurbs`–`Nurbs` curve intersection
  (C4); the procedural `Curve::Intersection` on the fitted charts, with
  its two tripwires; a fit that reuses the tracer's samples across face
  pairs if profiling asks.
- `AGENTS.md` current state — ADR-0018, and what C3's next plan is.

## Open questions

- Closed by step 1, for ADR-0018 to record: **the walked operand** is
  the one with parallel rulings before a cone, then the smaller radius or
  the narrower cone, then a circular section before an elliptic one, then
  the frames coordinate by coordinate — a rule on the surfaces, so a swap
  changes nothing bit for bit (property-tested). **Singular or not is a
  closed bound**: a critical point of the discriminant is singular when
  `|D| / 4|a||∇q|`, the distance the other surface would have to move for
  the ruling to touch it, is within `tol.linear`, or `|D|` is within its
  own rounding. No pose is left undecided; the refusals are named poses
  of measure zero (`SectionFault`). What step 1 also found: a branch must
  end at its singular point *exactly*, which the discriminant's own value
  subtracted as a bump gives and clamping does not (a near miss leaves
  the two roots `√(tol·R)` apart, far more than the tolerance); and the
  clip applies to every branch, not only the unbounded ones, because a
  near-asymptotic loop a thousand radii long is as useless to a fit as an
  unbounded one.
- Closed by step 3: **no fixture was restaged.** No blessed dump prints
  a pair's intersection (the `Interferences` display is in none), so the
  corpus is untouched. The one behaviour that moved is the one the step
  asked for: the two mixed positions are returned, and S5, which holds
  curves and points to one rule whatever their kind, decides them
  instead of listing them unchecked. The pave model refuses a pair past
  the quadric guard that meets in points or in both kinds as an
  invariant — none can reach it — until step 8.
- `⚠ OPEN:` Open CASCADE's counts on a closed section loop. One vertex
  per loop is expected on both sides; if its seam placement splits a loop
  differently, the fixture says so in `analytic.counts_differ` with the
  reason. Agent, step 8.
- Closed by step 4: **the degree of the 3D fit is 5**
  (`SECTION_FIT_DEGREE`). On metre-scale cylinder pairs (radii 1 to 2,
  crossing, skew, tilted) at the default tolerance a loop takes 200 to
  390 control points at degree 3, 70 to 160 at 4 and 60 to 120 at 5.
- Found by step 4, and done there:
  - **The tracer took a root in the wrong form** at a turning point on
    the walked ruling's base circle, where `b`, `√D` and `c` all vanish:
    `2c / (−b ∓ √D)` divided `c`'s rounding by them, and
    `c2-cylinder-pairs`' skew loop was half a unit off at its seam. The
    form is now chosen by its error bound (`trace.rs`, `Pencil::root`),
    with the pose as a hand case in `tests/trace.rs`.
  - **The fit's deviation** is each surface's distance from the fitted
    point *beyond the exact branch's own*, which is rounding except
    within a singular point's reach, where the tracer lets a branch be
    `tol.linear` off the other surface; measured from the surfaces alone
    a near-singular pose could never fit.
  - **The oracle walks what it cannot solve.** `IntAna_QuadQuadGeo` has
    no quartic, so an `unsolved` pair carries `GeomAPI_IntSS`'s lines,
    each sample polished onto both surfaces by Newton steps (the walk
    is only within 1e-8); `c2-cylinder-pairs` and `c2-quadric-pairs`
    were regenerated for it, their other answers unchanged. Its points
    lie within 1.1e-8 of Arris's loops, against a bound of 2.5e-8.
  - **The pave model refuses a traced section** — a fitted curve or a
    singular point — as `OpError::Unsupported` naming the pair, as it
    did while the intersector had none, and its one region per boolean
    (the operands' boxes' overlap grown by its diagonal) landed here,
    since every caller passes a region. Step 8 lifts the refusal.
  - **S5 and B1 already decide two cylinders in a quartic pose**, over
    the overlap of the two faces' boxes, since the intersector answers
    them; `docs/DATA-MODEL.md`'s S5 row says so. Step 10 is left the
    pairs step 5 adds, the blend property tests with nothing unchecked
    and the rows restated.
