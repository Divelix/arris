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
- **`arris_debug::fixtures::geom`** (step 6): `CurveSpec::Nurbs` — the
  geometry grammar's `nurbs` curve, `NurbsCurve::new`'s arguments and
  the oracle's `Geom_BSplineCurve` — and `BuildError::Geometry` for one
  that is no curve; the oracle meets it with a surface through
  `GeomAPI_IntCS`.
- **`intersect_curve_surface` and `intersect_curves`** (steps 6–7): the
  `Curve::Nurbs` arms against every analytic surface and against a line,
  a circle and an ellipse stop being `Unsupported`; `curves_coincide`
  answers for a `Nurbs` operand. No wildcard arm is added.
- **`arris_debug::fixtures::geom`** (step 7): `Hit::tb`, the second
  curve's parameter of a curve–curve pair, and a `Pair` of two curves;
  the oracle meets them through `GeomAPI_ExtremaCurveCurve`.
- **The pave model** (step 8): section crossings of a traced pair come
  from the result's `points`, not from `intersect_curves` on two NURBS;
  one region per boolean. `Interferences`' public shape follows `Meets`
  where it names the old variants (`SectionCurve::index`'s doc, the
  `Display`).
- **`NurbsCurve::segment`** (step 8, new public API in `arris-geom`):
  the curve over a range, clamped, in the same parameter — for the STEP
  writer, which writes an edge on a periodic NURBS on its own piece.
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
- [x] Step 5 **[2]** — The other ruled pairs: cylinder–cone, cone–cone,
  sphere against a cylinder or a cone off its axis, the elliptic cylinder
  against each. A plane oblique to a cone, or parallel to its axis and
  off it: `Curve::Ellipse`, or the parabola or hyperbola branches as
  exact rational quadratics over the region, split by knot insertion
  where one arc's weight would run away. Fixture `geom/c3-quadric-pairs`
  against the oracle; the property test extended to every pair. After
  this step no quadric pair without a torus is `Unsupported`.
- [x] Step 6 **[3]** — `Curve::Nurbs` against every analytic surface in
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
- [x] Step 7 **[2]** — `Curve::Nurbs` against a line, a circle and an
  ellipse in `intersect_curves`, through step 6 and the conic's plane as
  the existing conic arms go, and `curves_coincide` with a `Nurbs`
  operand. `Nurbs`–`Nurbs` stays an explicit `Unsupported` arm. Tests
  and a geometry fixture as step 6's.
- [x] Step 8 **[2]** — The pave model over traced sections: one region
  per boolean, `Meets` read by kind, a singular point becoming the
  section vertex that ends its branches, closed periodic section curves
  through the existing wrap. Each section edge's tolerance asserted equal
  to its faces'. Fixtures against Open CASCADE, every corpus stage:
  `boolean/tee-unequal-fuse`, `-cut` and `-common` (crossing axes,
  `r < R`), `boolean/skew-hole-cut` (skew axes, the drill partly outside:
  one loop), `boolean/skew-bore-cut` (skew and fully inside: two loops).
- [x] Step 9 **[2]** — A boolean through a fitted edge, which is what
  steps 6 and 7 are for: `boolean/tee-unequal-slot-cut` (a box cut
  across the junction curve of the fused tee) and
  `boolean/tee-unequal-drill-cut` (a cylinder through it). The test
  holds every edge of the result to its faces' tolerance: the idea's
  first tripwire, kept as an assertion. M4's identities (volume
  additivity, cut-then-fuse, commutativity) over unequal cylinders on
  crossing and skew axes at random poses in `boolean_prop.rs`.
- [x] Step 10 **[2]** — S5 and B1 decide every pair steps 4 and 5 added:
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
  pairs if profiling asks; a closed NURBS curve that is not periodic
  met at its seam, reported at both ends (C7, with the STEP reader).
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
- Closed by step 8: **Open CASCADE's counts on a closed section loop
  are Arris's.** Both put one vertex on a loop where the other operand's
  seam pierces it and split a face at a seam the same way, so no fixture
  of step 8 needs `counts_differ`.
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
- Found by step 5, and done there:
  - **A plane off a cone's axis is exact in every pose**, including
    through the apex: the apex alone, one touching ruling or two
    crossing ones, by the angle between the plane and the axis against
    the half-angle (`cone_section.rs`). A hyperbola's arcs keep their
    middle weight under `cosh HYPERBOLA_HALF_SPAN`, built split at double
    knots rather than split afterwards by knot insertion — the same
    curve, without the step.
  - **No elliptic cylinder in the oracle fixture.** The geometry grammar
    has none and Open CASCADE carries one only as a surface of
    extrusion, which `IntAna_QuadQuadGeo` does not take; the elliptic
    cylinder's traced pairs are held by the every-pair property test.
  - **The tracer's refusals stand**: `SectionFault` poses of measure
    zero stay `GeomError::DegenerateSection`, not `Unsupported` — among
    them two cones sharing an apex, whose section is up to four rulings
    through it (a backlog line).
  - **S5 decides every pair step 5 added** over the overlap of the face
    boxes, as step 4's pairs; `docs/DATA-MODEL.md`'s S5 row says so.
  - Two property tests of the closed-form table fail at 3000 cases and
    did before this plan; a backlog line records them.
- Found by step 6, and done there:
  - **The polynomial says where to look, the distance says what is
    there.** A span in the implicit form is `g = φ·δ`, `φ > 0` near the
    surface: the sign changes of `g′` (Bernstein subdivision, polished
    in the bracket) and the knots split the curve into stretches where
    `g` crosses zero at most once, and every verdict — touch,
    `Coincident`, the crossing's Newton — is taken on the exact signed
    distance `δ`, so a touch is `tol.linear` of *length* as in every
    other arm and a crossing is on the surface to rounding. Deciding a
    touch on `g` itself would have put `φ`, which is `R`-sized for a
    quadric and `R²r`-sized for a torus, into the tolerance.
  - **The robustness rests on one asymmetry**: a parameter that is no
    extremum only splits a monotone stretch, so the isolation may
    return too many and never too few. A stretch whose coefficients are
    all within `64ε` of their terms' magnitude is looked at in its
    middle, a cluster 48 halvings do not separate likewise, a halving
    that lands on a zero is remembered. Held by: the line arm's hits
    for a straight NURBS of degree 1, rational degree 2 and degree 25
    (against a torus, a polynomial of degree 100) on all six surfaces;
    the ellipse's own sign changes for an exact rational and a fitted
    periodic ellipse; a random NURBS's sign changes; 5000 cases each,
    and again under a second seed.
  - **An open curve's end within `tol.linear` of the surface is a hit
    and not `tangent`**, unless the run of stops it belongs to holds an
    extremum inside the curve. A clamped curve has no parameter past
    its end for the crossing to be found at, and the pave model makes
    no vertex of a touch: step 8 reads an open branch ending on a face
    as a vertex. A closed curve that is *not* periodic has its two ends
    for two hits at one point; no curve the kernel makes is one (a
    closed fit is periodic), a STEP reader's would be — a backlog line.
  - **The `Curve::Ellipse` arm has a closed form against a plane and a
    cylinder only**, so "agrees with the exact arm hit for hit" is held
    there, and against the ellipse's own sampled distance on all six.
  - **`geom/c3-nurbs-hits`**: an ellipse as four rational arcs, a
    quintic of three spans and a rational cubic with a double knot
    against a plane, a cylinder, a cone, a sphere and a torus — 15
    pairs, 37 hits, points and parameters matching `GeomAPI_IntCS` to
    1e-9 relative, and the curves' evaluations and projections with
    them. No elliptic cylinder, as step 5 found of the grammar.
- Found by step 7, and done there:
  - **A NURBS curve against a line goes through two planes**, both
    holding the line and square to each other: a crossing of the line
    is transversal to one of them unless the curve runs along the line,
    so it is found to rounding; candidates within `tol.linear` of each
    other are one hit, a plane's crossing before the other's touch. **In
    a conic's plane it goes through the conic's cylinder**, circular or
    elliptic, so the coplanar NURBS–ellipse pair the closed-form table
    still refuses for two conics is answered, and a touch is decided in
    length by step 6's arm.
  - **Two NURBS curves coincide** when they are the same spline (control
    points within `tol.linear` over the same degree, knots and weights —
    the same section made twice), are apart when a point of either at
    an end, a knot or a span's middle has no hit of the other within
    `tol.linear` on the plane square to it there, and are `Unsupported`
    otherwise (one curve over two knot vectors). The plane and not
    `project_parameter` decides "apart": the projection is a sampled
    local search, and on a sharp random curve it settles on a local
    minimum metres from a point that lies on the curve.
  - **`project_parameter` lost a minimum beside a C⁰ knot**: its Newton
    bracket ending on the knot was read on the next piece, where the
    distance's derivative has jumped and the sign change is gone. Each
    bracket is now read on the one polynomial piece it lies in, and one
    across a knot is left to its halves (`nurbs/spline.rs`, with a
    polyline's corner as a test in `tests/nurbs.rs`); no fixture moved.
  - **The oracle is `GeomAPI_ExtremaCurveCurve` span by span**: Open
    CASCADE has no 3D curve–curve intersector, and its extrema over a
    whole B-spline's domain missed the second crossing of a chord. The
    fixture is `geom/c3-nurbs-crossings`, 15 pairs, every hit matched in
    its point and both parameters to 1e-9 relative, the touch pinned by
    name in `oracle.rs`.
- Found by step 8, and done there:
  - **A traced loop's period is its own length**, not a turn: the pave
    model's periodic handling (paves wrapped into the curve's domain,
    the seed vertex of an unpaved loop at the domain's start, its middle
    tested for "inside both") reads the curve's domain instead of
    `[0, 2π)`.
  - **An open section curve is paved at each end a vertex lies on.** A
    singular point's branches leave it and come back; projecting the
    vertex found one end, and the block to the other was lost.
  - **The checker's E8 flagged a wrap-around block** whose range runs a
    few units in the last place past the loop's knots: the tiny span
    there, sampled like any other, put non-adjacent segments within the
    tolerance of each other. E8 now needs more than the tolerance of
    polyline between two segments before their nearness is a crossing
    (`docs/DATA-MODEL.md` §Invariants).
  - **STEP lost the wrap-around block**: an `EDGE_CURVE` has no range,
    and on a periodic curve written as a plain B-spline a reader's
    projection of the vertices picks the other side, collapsing the
    block onto its twin (`skew-hole-cut` read back at 18.87 against
    18.34). An edge on a periodic NURBS is now written on its own piece,
    `NurbsCurve::segment`.
  - **Open CASCADE's measurement, not its body, was 1e-6 off.** Its
    `GProp` fixed-order integration over the many-span pcurves a walked
    section trims its faces with is off by 6e-7 to 1.7e-6 in volume and
    area; the adaptive overloads bring its own bodies to within 1e-10 of
    a quadrature of the exact section, where Arris's already are. The
    oracle measures a shape with a B-spline edge adaptively (`measure.py`
    `SPLINE_EPS`); the one committed fixture with such an edge,
    `elliptic-operand-cut`, moved 1000× nearer its closed form (8.1e-8 to
    6.5e-11) and was regenerated.
  - **`tee-unequal-cut` and `-common` hold volume, area and inertia at
    1e-7 relative**, the model's tolerance: both sides' loop edges lie
    within the tolerance of the exact section, not on it, and on bodies
    this small the trim that moves reaches past 1e-9 — both stand within
    5e-9 of the quadrature, on opposite sides. The fuse and the skew cuts
    hold the default 1e-9.
  - **Open CASCADE's solid classifier misreads a point on the ruling
    through the middle of a loop piece** — it casts its ray along the
    wall there — so `skew-bore-cut`'s wall probes stand off that ruling.
  - **A boolean through a singular point stops in the builder**: the
    pave model makes the point one vertex ending both branches, but a
    drill touching the main wall from inside leaves the wall two pieces
    meeting only there, pinched, and the builder refuses to close the
    shell (`OpError::Internal`). `regression/singular-bore-cut`, ignored,
    with a backlog line: features a tolerance apart are C3's next plan's.
- Found by step 9, and done there:
  - **Both booleans through the tee's fitted edge build as they are**:
    `tee-unequal-slot-cut` (the box's planes cross the loop's three
    pieces in four points, the slot a window through the branch, genus
    1) and `tee-unequal-drill-cut` (the drill's wall meets both walls in
    traced loops crossing the tee's loop in two points) pass every
    corpus stage, every edge at its faces' tolerance. The drill cut
    holds volume, area and inertia at 1e-7 relative as the tee's cut
    does: built at a model tolerance of 1e-10 Arris measures area
    53.6887165506, Open CASCADE's body 4e-10 below it, Arris's at the
    default tolerance 9e-10 above.
  - **A crossing merged into an edge's own end paved it again.** In
    `fuse(a − b, b)` of a posed notch the loop edge crossed the tool's
    seam 1.04e-7 from its end vertex, a hair past that vertex's
    tolerance; the crossing joined the section vertex holding the end,
    which then paved the edge beside its own end — a 2e-8 sliver block
    the tool's wall could not be split along (`SplitFault::Turn`).
    `pave_edges` now skips a crossing whose vertex holds one of the
    edge's ends, as `pave_coincident_edges` already did
    (`docs/ARCHITECTURE.md` §Operations); the shrunk case is a test
    beside the property.
  - **S5 took seconds on a traced branch clipped by the faces' boxes**:
    `curve_range` projected every point of both faces' polygons onto
    the open NURBS, 4.4 s a pair where the check is otherwise 50 ms. A
    curve with a bounded domain — periodic, or a NURBS the region
    already bounded — is sampled over its domain; only a line needs the
    hull. Same verdicts, the checker's tests unchanged.
  - **The quartic property measures in the pair's own frame**: the
    boundary integral over fitted pcurves grows with the distance from
    the origin (`docs/BACKLOG.md`, mass properties that do not depend on
    where the body is), 1.2e-9 relative in a common 25 away where the
    same bodies at the origin agree to 2.6e-11; the fuse and the common
    carry the same pcurves, so it is the measurement, not the boolean.
    It runs in 16 shards: seven booleans and their `Full` checks a case.
  - **Shallow crossings of a loop edge and a seam, at 1000 cases.**
    Three more posed notches failed `fuse(a − b, b)`: where the notch's
    fitted loop crosses the tool's seam at a shallow angle the two stay
    within the tolerance over a stretch longer than it. The NURBS–line
    arm's two planes each found the crossing, 1.1e-7 apart, two hits of
    one; it now joins candidates the curve runs between within the
    tolerance of the line (`intersect_curves.rs`, `docs/DATA-MODEL.md`
    §Curves). And the one hit, where it was one, lay a hair past the
    tolerance of the vertex the cut had made there; the pave model now
    puts a crossing at an edge's end vertex when that end is on the other
    edge and the stretch between is too (`at_shared_end`,
    `docs/ARCHITECTURE.md` §Operations). Both are the features a
    tolerance apart the next plan takes on, in the one form this plan's
    identities reach; the shrunk cases are a test beside the property,
    which passes at 1000 cases with every other property of `arris-geom`,
    `arris-ops` and `arris`.
- Found by step 10, and done there:
  - **Nothing was left for S5 and B1 to learn**: both run `faces_meet`,
    which has passed the overlap of the two faces' boxes as the region
    since step 4, and step 9 made the traced branch it clips affordable.
    The step is `blend_prop.rs` holding every result to nothing
    unchecked, at rest and posed — fillets and chamfers, 1000 cases each
    — with the exemption for skew blend cylinders and a corner's sphere
    against a blend cylinder deleted, and the S5 and B1 rows and
    `Unchecked::FacePair`'s doc restated to what is left: a torus in a
    pose sharing no axis with the other surface, and the tracer's
    refusals.
