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
  name — a tube circle of the torus on the other surface until step 3
  answers it, and whatever step 2 finds it cannot decide. The existing
  `TangentAlongCurve` and `CrowdedSingularity` are reused where they say
  the same thing.
- **`intersect_surfaces`** (step 4): `off_axis`'s torus arms and the
  elliptic cylinder's route to a torus stop being `Unsupported`; the only
  `Unsupported` surface pairs left have a `Surface::Nurbs` in them. No
  wildcard arm. The signature does not change; the rustdoc table does.
  Which torus of two is parametrised is a rule on the surfaces — the
  smaller minor radius, then the smaller major radius, then the frames
  coordinate by coordinate — so a swap changes nothing, bit for bit.
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
- [ ] Step 2 **[3]** — The tracer, `arris_geom::trace_torus`, all six
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
- [ ] Step 3 **[3]** — A tube circle on the other surface, the pipe-elbow
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
- [ ] Step 4 **[2]** — ADR-0019, and the torus pairs in
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
- [ ] Step 5 **[2]** — S5 and B1 decide a torus face against anything.
  Checker tests on hand-assembled bodies (`arris_debug`'s sample torus):
  the ring beside a tilted cylinder shell clear of it — B1 decided, clean;
  a tilted pin through the tube — B1 violated, naming the faces; a torus
  face and an oblique plane face of one shell sharing their section as an
  edge — S5 clean — and the same with the edge left out — S5 violated;
  the elbow pose of step 3 as two faces sharing the tube circle. The
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
  of very different sizes to hold it.
- **A tube circle on the other surface is not a `Continuum`** but an
  uncertified box a whole turn long in `v` and a sliver in `u` (1e-5 to
  3e-3 wide), beside the certified turning points of whatever else the
  pair meets in: ~470 boxes for the elbow, ~7 300 for a sphere crossing
  along the circle (and its mirror image, which it also holds).
  `Continuum` is left for a curve of turning points oblique to both
  parameters. Step 2 refuses, and step 3 detects, on the box's shape.
- Depths are in **halvings**, two to a quartering; `MAX_DEPTH` is per
  direction.

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
  each with its pose; the Villarceau circles if open question 2 leaves
  them fitted; `bernstein2` as C4's starting point for a NURBS patch in
  an implicit. The "two intersector properties fail at 3000 cases" lines
  are untouched.
- `AGENTS.md` current state — ADR-0019, the torus pairs done, and C3's
  next plan: conic hits on curved quadrics, the fitted pcurve fallback,
  the quadric guard lifted.

## Open questions

- ⚠ OPEN 1 — **the fallback if step 1's gate fails** (human, at step 1;
  certification passed, the cost clause was read as step 1's record says
  and is the human's to overrule):
  the idea's answer is option A — tube circles as the family, for a plane
  and a sphere only — with the other four pairs moved to C4 and the
  roadmap amended. Not planned here; the plan is rewritten if it comes
  to that.
- ⚠ OPEN 2 — **are the Villarceau circles returned exact?** (agent, by
  step 4; recorded in ADR-0019.) ADR-0018 says a conic is never fitted,
  and a bitangent plane is a pose with a name. But it is one pose of
  measure zero, the tracer already returns the two loops through their
  two singular points, and "bitangent within `tol.linear`" is a second
  detector to keep consistent with the first. Preferred: exact
  `Curve::Circle`s if step 2's singular points make the detection a
  lookup (two singular points on one plane section), fitted with a
  backlog line otherwise.
- ⚠ OPEN 3 — **closed form or named refusal for a tube circle on the
  other surface** (agent, at step 3). Preferred: the closed form, because
  a pipe elbow against its straight pipe is this pose and S5 meets it as
  soon as a body has both faces. The refusal is the fallback, not the
  plan.
- ⚠ OPEN 4 — **how step 2 joins pieces at a turning point and ends
  branches at a singular point** (agent, at step 2; recorded in
  ADR-0019): ADR-0018's two devices are one-dimensional and the idea
  already says the bump does not carry over as is.
- ⚠ OPEN 5 — **does `MeetCurve` carry the torus's pcurve, or does the
  next plan fit it from `pcurve_on`'s projection?** (agent, in C3's third
  plan — not here.) This plan only keeps `SectionBranch::uv` so the first
  is possible.
