# Plan: c3-conic-hits

- Started: 2026-09-20
- Milestone: C3, every quadric pair (docs/ROADMAP.md §C3: the conic-hits
  bullet, the fitted-pcurve bullet, the quadric-guard bullet, and the
  accept line's boolean identities and `boolean/*` quadric fixtures; the
  third of C3's plans)
- Idea (verbatim from the human): "c3-conic-hits"

## Goal

A boolean takes a cone, a sphere, a torus or an elliptic-cylinder face as
an operand, so a body Arris built — a revolve, a fillet, a chamfer, an
extruded ellipse — is a body Arris takes (ADR-0020). Three things make it
so, and the plan lands them in the order the pave model needs them.
**Every curve on every analytic surface has a pcurve**: where no exact
`Curve2` exists, `pcurve_on` fits one over the surface's own projection,
unwrapped in every periodic direction and held to the 3D curve in length —
one fallback for the cylinder, the elliptic cylinder, the cone, the sphere
and the torus, where today only the cylinder has one. **Every edge an
operand can carry meets every analytic surface**: a circle or an ellipse
against a cone, a sphere, a torus, and an elliptic cylinder in any plane,
in `intersect_curve_surface`; two coplanar conics that are not the same
conic, in `intersect_curves`. **The quadric guard is gone**: the pave
model asks the intersector for every face pair and every edge–face pair,
reads a `Meets` that mixes kinds, puts a vertex where a section runs
through a cone's apex or a sphere's pole, and treats a contact along a
circle as it treats one along a ruling.

When the plan is done: `pcurve_on` and `intersect_curve_surface` return
`Unsupported` only with a `Surface::Nurbs` in the pair, and
`intersect_curves` only for two NURBS curves; the corpus holds a boolean
fixture against the oracle for each of the four kinds, for a fillet's
torus and sphere and a chamfer's cone cut after they were made, for a
section through an apex and through a pole, and for a pipe elbow; the M4
identities hold over a quadric operand against a box and a cylinder at
random poses; `fn quadric` no longer exists.

## Non-goals

- Features a tolerance apart: section vertices clustered by closure,
  `regression/seam-a-tolerance-from-crossing-fuse`, the corpus of faces
  touching and coincident within a tolerance, and
  `regression/singular-bore-cut` (a body pinched at a traced section's
  singular vertex). C3's last bullet and its own plan. A singular point
  here is only ever a surface's own — an apex, a pole — never a tracer's.
- NURBS operands, `Curve::Nurbs` against `Curve::Nurbs`, `project` on a
  NURBS surface (the NURBS cycle's; the reader cycle pulls the last
  forward). The three `Unsupported` arms named in the goal stay.
- New primitives (`primitive_sphere`, `_cone`, `_torus`): the fixtures
  build their operands from `Revolve`, `Fillet`, `Chamfer` and `Extrude`,
  which is the closure the cycle is about. A backlog line if a fixture
  wants one.
- Blends on quadric face pairs, a spindle torus, a revolve of an elliptic
  segment, the STEP reader: C3 "Out".
- A resolved tangent contact: a contact curve interior to both faces stays
  `Reason::TangentContact`, along a circle as along a ruling. Step 7 makes
  the circle *found*, not built through.
- The tracers' named refusals (`SectionFault`) and the fit's cost: a
  boolean that meets one returns it typed, naming the faces. Backlog lines
  already.

## Design deltas

- **ADR-0021** (step 6): "a pcurve never runs through a surface's
  singular point: fitted by projection, split at the apex and the pole".
  Records what steps 1 and 6 prove — the projection fallback and its
  unwrapping, the limit `u` at a singular end, the band about a singular
  point inside which a curve is *through* it in length, the vertex the
  pave model puts there — and answers the last plan's ⚠ OPEN 5 (⚠ OPEN 1
  here). Amends nothing; ADR-0018's and ADR-0019's storage stands.
- **`arris_geom::pcurve_on`** (step 1): the signature does not change; the
  table does. `fitted_on_cylinder` becomes one fallback over
  `Surface::project` for every analytic surface, `u` and — on a torus —
  `v` unwrapped along `t`, the deviation measured in 3D as it is today. A
  range with a singular point of the surface *inside* it is a new typed
  error naming the parameter — **`GeomError::ThroughSingularity { curve,
  surface, t }`**, a new variant of a public enum, named in the commit
  body — so the caller splits there; a range that *ends* on one is fitted,
  its `u` there the limit along the curve. `PCURVE_SAMPLES`' winding
  refusal stays, per periodic direction.
- **`arris_geom::intersect_curve_surface`** (steps 2 and 3): the
  `(Circle | Ellipse, Cone | Sphere | Torus)` arm and
  `conic_elliptic_cylinder`'s "any other plane" stop being `Unsupported`.
  `intersect_spline`'s verdict — stops, runs of touches, one crossing
  between two stops of opposite sign, all on `Implicit::distance` — is
  factored out of `spline_surface` so a conic is decided by the same code:
  its candidates from `conic2::trig2_roots` on a quadric (the implicit
  along a conic is a trigonometric polynomial of degree two, as
  `conic_cylinder` already has it) and from `bernstein` over four
  quarter-turn rational arcs on a torus (degree eight). No signature
  change; the rustdoc table restated.
- **`arris_geom::intersect_curves`** (step 3): `coplanar`'s
  ellipse-bearing arm answers through `conic2` instead of `Unsupported`.
  `curves_coincide`'s doc loses "the quartic `intersect_curves` refuses".
- **`arris-ops` `boolean::pave`** (steps 4 to 7): `fn quadric` and its two
  call sites deleted; the `mixed` invariant in `sections` deleted, the
  crossing curves read as sections and the touching ones as contacts
  whatever else the `Meets` holds; `contact_curve` takes a closed curve;
  `Build` reads each operand face's singular vertices (the degenerate
  edges `EdgeInfo` skips) and paves a section curve within the tolerance
  of one. `interferences`' rustdoc loses the guard. No public signature
  changes expected; any that a step finds is named in its commit body.
- **`arris_debug::fixtures::ExpectError::EllipticOperand` removed** (step
  4): `boolean/elliptic-operand-cut` becomes a fixture with a result, a
  `fixtures:` change whose body says the recipe is unchanged and the
  expectation is the oracle's solid instead of a refusal.
- **`tools/oracle`**: none expected — the recipe grammar's eleven
  operations build every operand here. If the oracle needs anything (a
  fuzzy value on a pole, a `geom/` generator case for conic hits), the
  commit body says which. *Step 3 needed three, none of which moves a
  number of an existing fixture:* **`arris_debug::fixtures::geom::
  SurfaceSpec::EllipticCylinder`**, a new variant of a public enum —
  Open CASCADE has no analytic elliptic cylinder, so the oracle builds
  the section ellipse extruded along the axis, trimmed to
  `EXTRUSION_REACH` since the general intersector finds nothing on an
  extrusion's unbounded parametric range; that general intersector for a
  conic against a torus or an elliptic cylinder, which `IntAna` has no
  form for; and a curve pair that is two conics, decided by the
  curve–curve extrema the NURBS pairs already use.
- **DATA-MODEL:** §Pcurves (the fallback on every surface, the singular
  rule), §Curves (the conic–surface and conic–conic tables), §Tolerances
  if step 6's band needs a sentence. **ARCHITECTURE:** §Operations (the
  guard paragraph, the revolve paragraph's "behind the quadric guard",
  the elliptic cylinder's), the pave model's description.
- **No crate boundary moves.**

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness or
bound has to be established here.

- [x] Step 1 **[3]** — The fitted pcurve on every analytic surface, and
  what happens by a singular point. `pcurve_on`'s fallback over
  `Surface::project` for the cone, the sphere, the torus and the elliptic
  cylinder, the cylinder's moved onto it unchanged in result. To
  establish: the unwrapping in two periodic directions on a torus; the
  `u` a curve arrives at a pole or an apex with (the limit, from the
  tangent), so a range ending there fits; how near a pole a curve may
  pass *without* ending on it before the fit in (u, v) — `u` swinging by
  π over a stretch as long as the miss — runs out of `MAX_FIT_SPANS`,
  measured from one tolerance to 1e-2 of the radius and recorded; and
  `ThroughSingularity` for a range with the point inside it, decided in
  length. Tests in `tests/pcurve.rs`: a property per surface — a random
  plane's, cylinder's and sphere's section from `intersect_surfaces`,
  exact conic or fitted, its pcurve's image within `tol.linear` of the
  curve at 2000 parameters, a seam crossing continuous; on a torus the
  projected pcurve against **`SectionBranch::uv`**, the exact reference
  the last plan kept for this (⚠ OPEN 1 is answered on that number); hand
  cases — a small circle of a sphere through a pole (split, both halves
  fit), one a tolerance clear of it, a cone's rulings-and-ellipse through
  and beside the apex, a Villarceau arm, a spiric oval across both seams.
  Rustdoc table and `docs/DATA-MODEL.md` §Pcurves restated in this commit.
  *Established:* the band is a quarter of `tol.linear`
  (`PCURVE_SINGULAR_BAND`), not the whole of it — a pcurve ending on the
  point is off the curve by the curve's own miss, the fit accepts half
  the tolerance, and at a whole tolerance the two sides of a split ran
  out of spans from a miss of 0.6; the unwrapping halves its sampling
  where `u` swings, so the near-miss limit is the fit's and not the
  table's, and the fit never reaches it: 917 control points just outside
  the band, 293 at `1e-2` of the radius, 10 to 110 ms (DATA-MODEL
  §Pcurves holds the series); the cylinder's fitted pcurves are the same
  to the bit, which took leaving its `u` unwrapped as `atan2` gives it.
- [x] Step 2 **[2]** — A conic against a cone, a sphere, and an elliptic
  cylinder in any plane. `spline_surface`'s verdict factored out and
  shared; the candidates from `trig2_roots` on the implicit's derivative
  along the conic, plus the distance's kinks — the cone's apex plane and
  its axis — as the line arm has them. `Coincident` for a cone's or a
  sphere's own circles and for an oblique section lying on the surface.
  Tests: a property per pair at random poses — every hit on both within
  the tolerance, ascending, a dense scan of the conic finding no crossing
  unreported, a touch one hit and not two; hand cases — a great circle
  through a pole, a circle through an apex, a parallel (`Coincident`), an
  ellipse tangent to a sphere from inside.
  *Established:* the kinks are the cone's alone, and the extrema of `ρ²`
  are added there and wherever the polynomial is **constant** along the
  conic (a conic concentric with and similar to the section — for an
  elliptic cylinder its distance still varies, and the derivative has
  nothing to say). Added everywhere they cost a hit: a conic through a
  sphere's pole passes the axis there, `ρ²` has a triple critical point,
  and the two roots a rounding apart straddling the crossing read as an
  extremum on the surface — a touch that absorbs the crossing. A
  sphere's own extrema are its polynomial's exactly, so nothing is lost.
  The random property moves the conic's centre onto the surface: drawn
  independently, 97% of the pairs miss.
- [x] Step 3 **[2]** — A conic against a torus, two coplanar conics, and
  the fixture. The torus by four rational quarter arcs through the
  Bernstein isolation `spline_surface` uses, degree eight, the hit's `t`
  the conic's own angle; up to eight hits, a Villarceau circle and the
  torus's own circles `Coincident`. `intersect_curves`' coplanar
  circle–ellipse and ellipse–ellipse through `conic2`, touches decided in
  length. Properties as step 2's. Fixture **`geom/c3-conic-hits`** against
  the oracle (`generate.py`, as `c3-nurbs-hits`): conics against each of
  the four surfaces and the coplanar pairs, crossings and a touch each.
  `every_other_pair_is_unsupported`'s curve–surface and curve–curve
  twin in `tests/intersect_curve_surface.rs` left the NURBS arms only,
  and `intersect_curves` given the same test for its one.
  *Established:* the quarter arc is now `geom`'s own `arc` module, moved
  out of the torus tracer unchanged, because a conic's quarters and a
  torus's patches are the same exact rational quadratic; a conic's
  quarter in a torus's form is degree eight, and the quarters' own ends
  go in beside the derivative's sign changes, an extremum at a join
  being a change neither side sees. The coplanar pair goes through
  `conic_pair` in the *second* conic's plane, and `curves_coincide` is
  now that verdict rather than a closed form of its own, so the two can
  no longer disagree. Three things the oracle needed, each in the
  commit body: Open CASCADE has no elliptic cylinder, so the geometry
  grammar's new `elliptic_cylinder` is the section ellipse extruded
  along the axis — the same point set with the same `(u, v)`, which its
  own samples in the fixture check — and the general intersector finds
  *nothing at all* on an extrusion's unbounded parametric range
  (±2e100) whatever the pose, so it is trimmed to `EXTRUSION_REACH`;
  a conic against a torus or an elliptic cylinder has no `IntAna` form
  and goes through `GeomAPI_IntCS`, which reports a conic *lying on*
  either as a cloud of hundreds of points rather than a segment, so the
  fixture asks it only where the two cross or touch and the coincident
  verdicts stay in the property tests; and two coplanar conics go
  through the curve–curve extrema the NURBS pairs already use. All
  fourteen pairs classified as built on the first run. One finding for
  step 6 and the tolerance plan: a root at `t = 0` comes back at one
  rounding *below* a whole turn — `trig2_roots` polishes it to a
  rounding-sized negative and `wrap_angle` brings that to just under
  `2π` — which is inside the `[0, 2π)` the guarantee states and is the
  same point, but a split there gives a hair-thin block at the end of a
  closed edge rather than none at its start. The periodic assertions are
  written in the turn metric because of it.
- [x] Step 4 **[2]** — The guard lifted for the ruled kinds: a cone and an
  elliptic cylinder as operand faces, clear of the apex. Fixtures against
  the oracle, every corpus stage: `boolean/frustum-oblique-cut` (an
  ellipse, its pcurve fitted on the cone), `boolean/frustum-slot-cut` (a
  plane parallel to the axis: the exact hyperbola of ADR-0018 as an
  edge), `boolean/frustum-cross-drill-cut` (a traced quartic),
  `boolean/chamfered-boss-slot-cut` (a chamfer's cone cut after it was
  made), `boolean/elliptic-operand-cut` turned from a refusal into its
  solid, `boolean/elliptic-cross-fuse`. `a_quadric_face_is_refused_before_
  the_intersector` becomes the same recipe building. A sphere or torus
  face is still refused, by the guard narrowed to those two.
  *Established:* the pave model needed **nothing** — narrowing `quadric`
  to the sphere and the torus is the whole kernel change, and every
  fixture passes every corpus stage on the recipe as first written. What
  the step cost was the **oracle**, in three places, because an operand
  face on a cone or an elliptic cylinder is the first face Open CASCADE
  cannot measure by the rules the corpus was calibrated on. (a)
  `_spline_bounded` read 3D edge curves; a section on a cone is an exact
  conic edge over a *fitted pcurve* (ADR-0021), which that test cannot
  see, and over such a pcurve the fixed-order integration is 1e-6 off.
  It now also reads the pcurves of a cone, sphere or torus face, and
  takes any shape with a surface-of-extrusion face — whose *plane* faces
  the fixed order is 2e-7 off on, against `boolean/elliptic-operand-cut`'s
  closed forms. (b) The area of an extrusion face was its basis arc's
  length times its height, which assumed "its (u, v) region is a
  rectangle for every face an extrude makes" — true until a boolean
  trims it. It is now the contour integral of the arc length against dv
  (Green), exact over a region bounded by rulings and arcs and giving
  back L·Δv bit for bit on a rectangle; a curved pcurve there is refused
  by name, since the corpus holds no such fixture — `elliptic-cross-fuse`
  was drawn as two crossing elliptic prisms first, and is the column
  through a plate instead because that pair is exactly the curved case:
  a backlog line, with the evidence that Open CASCADE's own measurement
  of it is 6e-4 to 1.2e-3 out. (c) The volume
  properties of a shape with an extrusion face go through
  `VolumePropertiesGK` with the span option: the plain adaptive
  integration is 9e-7 off in `elliptic-operand-cut`'s inertia tensor
  where GK is 1e-11 from the closed forms. **The oracle was the wrong
  side in every one of them** — Arris matched the closed forms to 1e-10
  before the change and after. Four committed `expected.json` move:
  `boolean/elliptic-operand-cut` (area 1.5e-7, inertia 2e-6 — the
  fixture's own correction) and the three `sweep/extrude-elliptic*` at
  1e-15, the GK path's rounding. One more finding, in
  `frustum-oblique-cut`'s own text: **Open CASCADE's STEP reader drops
  Arris's fitted pcurve** — degree five over thirty-six poles — and
  reprojects the cone's ellipse as a cubic over twenty-five, whose
  (u, v) region is 9.5e-7 of the cone face away, so that one fixture's
  *oracle comparison* is held to 1e-6 while Arris's own measure is
  4e-10 from the closed-form volume and area. The `mixed` invariant of
  `sections` is kept as a tripwire with its reason restated (a cone or
  an elliptic cylinder met at its apex or along a tangent circle is
  steps 6 and 7's), and no fixture here trips it.
- [x] Step 5 **[3]** — The guard gone: a sphere and a torus as operand
  faces, clear of the poles. What is unproven is the face, not the
  section: a whole torus face is periodic both ways with two seam edges,
  a sphere's closes on two degenerate edges, and `place`, `band` and the
  face splitter have met neither. Fixtures: `boolean/ball-corner-cut`
  (three small circles, fitted on the sphere), `boolean/ball-offset-
  drill-cut` (a Viviani-like loop), `boolean/ball-ball-common` (two
  revolved balls on different axes), `boolean/ring-slab-common` (spiric
  ovals across both seams), `boolean/ring-pin-cut` (a drill through the
  tube), `boolean/filleted-boss-drill-cut` and `boolean/filleted-corner-
  notch-cut` (a fillet's torus, and its cylinders and corner sphere, cut
  after they were made — the closure ADR-0020 names). Each result at
  `Full` with `Report::unchecked` empty, which is also S5's shared-edge
  excuse over a fitted torus section, untestable until now. `fn quadric`
  deleted; ARCHITECTURE §Operations restated in this commit.
  *Established:* the face needed **nothing** — `fn quadric` deleted is
  the whole of the guard's removal, and all seven fixtures pass every
  corpus stage on the recipe as first written, a torus band split at
  both seams into the four faces Open CASCADE makes of it, a sphere
  keeping or losing its degenerate edges with the pole. What the step
  found it found at random poses (a scratch run of a ball and a ring
  against a box and a cylinder, fourteen hundred poses, all four
  booleans, `Full` and the volume identities — step 8's property in
  outline), and neither fault was the splitter's. (a) **`place`'s seam
  check converted the tolerance once, at the block's midpoint**: a
  section loop round a pole ends on the seam where `R·cos v` is small
  and the same length is sixteen times the `u`, so a fit 7.1e-8 of `u`
  past the seam was `Fault::Seam`. It converts where the pcurve is, in
  the direction held (`check::domain::bands`);
  `boolean/ball-polar-drill-cut` is that pose. (b) **A wrong number
  with the checker green**: a plane's pcurve of a closed fitted section
  is exact — the same periodic NURBS — so a block that wraps is a range
  past the knots' end, and five places filtered the stored knots for
  their breaks and found none there: `region_integral` took the wrapped
  part in one Gauss interval, and the volume of a right body came out
  8e-4 off (the body itself Open CASCADE measured, from Arris's STEP,
  at its own volume to 2e-9). **`NurbsCurve::breaks_within` and
  `NurbsCurve2::breaks_within`**, new public methods, repeat a periodic
  curve's knots by whole periods, and the quadrature, both polygon
  segment counts, `Curve2::speed_bounds` and the checker's edge
  sampling read them; `boolean/ring-corner-common` is that pose, with
  a unit test of the integral in `integrate`. Two fixtures the plan did
  not list, then, each failing on the commit before. (c) **Out of
  scope, and a fixture:** `regression/grazing-ball-bar-cut` — where a
  ball and a bar meet at 5°, S5's own fit of the section and the
  edge's lie 1.06e-7 apart at a tolerance of 1e-7, and a right body is
  refused; OPEN 1's lateral slack met from the checker's side, a
  backlog line for the tolerance plan. One pose in five hundred.
  Three fixtures state oracle-comparison tolerances of their own, each
  with the evidence in its description that **Open CASCADE's own solid
  is the far side** — its section edges carry 1e-6 where Arris's carry
  1e-7, and 2.4e-5 on one edge of `ring-corner-common`:
  `ring-slab-common` (`inertia_rel` 3e-9, against a thirty-digit
  quadrature Arris is 7e-11 from), `ball-polar-drill-cut` (3e-9) and
  `ring-corner-common` (1e-6), the last two against the value Arris
  converges to as the model tolerance goes from 1e-7 to 1e-9, which
  Open CASCADE itself measures on Arris's STEP. For step 6: with a
  cylinder's wall through a ball's pole **Open CASCADE builds no solid
  at all** (`degenerate` in `expected.json`), so `ball-pole-drill-cut`
  will want `measure_differs` or another pose. For step 8: the scratch
  run is its ball and ring strategies; it took about three minutes per
  two hundred poses in release.
- [x] Step 6 **[3]** — A section through an apex or a pole, and ADR-0021.
  The operand's singular vertex paves every section curve within the
  tolerance of it, so no block has the point inside and step 1's
  `ThroughSingularity` is never met from a boolean; the result's faces
  keep or lose their degenerate edge by which side of the section
  survives. Fixtures: `boolean/cone-apex-slice-cut` (a plane through the
  apex: two rulings ending there), `boolean/ball-pole-slice-cut` (an
  oblique plane through a pole: a small circle through it),
  `boolean/ball-pole-drill-cut` (a cylinder's wall through a pole). A
  property: a plane at a random pose through a revolved cone's apex and a
  ball's pole, cut and common, additive in volume. If a pose cannot be
  made robust here it is refused by name and committed under
  `tests/fixtures/regression/` with its desired assertion — ⚠ OPEN 2.
  *Established:* **through is built, beside is refused by name**
  (ADR-0021, ⚠ OPEN 2 answered). The vertex was mostly there already —
  the seam ends on the apex or the pole and pierces the other face at it,
  an ordinary hit on a vertex — and what the faces lacked was the
  **node**: the vertex is a whole line of (u, v), both rulings reached
  the one corner of the box and read as a `TangentContact`, a traced
  loop as an arrangement that does not turn once. The degenerate edge is
  now paved at every `u` a section edge arrives with, and `result`,
  `rebuild` and the checker took its degenerate pieces as they were.
  `VertexSource::Singular` is for where the seam only *touches* the other
  surface at the point (`ball-pole-slice-cut`'s `along_seam`) or the face
  has no seam there. Two faults in `pcurve_on`, neither the fit's: a
  range that ends on the one pole **twice** — what a circle through it
  is, once paved — read the first end's `u` at both and wound by π at
  the last sample; and a sphere's **meridian** took its `v` from the
  circle's phase alone, a whole turn off the latitudes for the half
  circle by way of `t = π`, which nothing downstream can put back since
  `v` is no period of a sphere (the property's shrunk case: a face in
  the seam's plane). All three fixtures pass every stage on the recipes
  as drawn, the two slices against closed forms; `ball-pole-drill-cut`
  is the pose Open CASCADE *does* build, a wall through both poles, and
  states 3e-9 and 5e-9 because the oracle is the far side — single
  integrals by Gauss–Legendre, Arris 1.1e-11 and 2.3e-11 from them, Open
  CASCADE 2.3e-9 and 1.9e-9, its centroid 6.8e-10 off the symmetry plane.
  The property runs eight thousand poses clean. **What it and a sweep of
  misses found is the other half of step 1's open note**, and it is
  larger than the note thought: from the band (a quarter tolerance) out
  to a miss of about `1e-5` on a ball of radius 2 nothing builds — a
  `SplitFault::NoInterior` after tens of seconds, a traced loop not
  finishing at all, and by an apex an **invalid body in a release
  build** (a hyperbola doubling back within a tolerance, E8) or a false
  `TangentContact` (the intersector's two lines through the apex, in
  length, against a seam piercing the plane 3e-7 down its ruling). The
  fit is not the fault — step 1's 917 control points hold — the
  **polygon** is: evenly cut in the parameter by the largest second
  derivative on the piece, capped at 65536. So the pave model decides
  *through* by `pcurve_on`'s own band (and on a cone by the tangent:
  along a ruling, or across them), and refuses the rest by name,
  `Reason::BesideSingularity`, out to four polygon segments of the
  face's diagonal, before anything is fitted: milliseconds, typed, the
  model untouched. Two regression fixtures, each failing as its
  `#[ignore]` says: `regression/ball-beside-pole-slice-cut` (the
  refusal; ten times nearer, Open CASCADE's own cut returns the whole
  ball in two shells) and `regression/pole-slice-beside-seam-cut` (a
  circle leaving the pole 2e-4 of a radian off the seam's meridian,
  `Fault::Seam`, one pose in eight thousand — the strategy now keeps
  0.05 from the seam, and exactly along it is a variant that passes).
  Two backlog lines. For step 8: a ball or a cone at a random pose meets
  `BesideSingularity` rarely and not never, and the property takes it as
  it takes `TangentContact`. New variants of public enums, named in the
  commit body: `VertexSource::Singular`, `Reason::BesideSingularity`.
- [ ] Step 7 **[2]** — A `Meets` of both kinds, and a contact along a
  circle. The `mixed` invariant removed; `contact_curve` for a closed
  curve, its blocks wrapping as a section loop's do. Fixtures:
  `boolean/pipe-elbow-fuse` (a quarter bend and the straight pipe it runs
  into, tangent along the tube circle their caps share — an ordinary
  shape), `boolean/ball-in-bore-cut` (`ExpectError::TangentContact`: a
  ball touching a bore along a circle interior to both),
  `boolean/ball-on-apex-common` (a sphere through a cone's apex on its
  axis: a circle and a point in one `Meets`).
- [ ] Step 8 **[2]** — The identities over quadric operands.
  `boolean_prop.rs` strategies for a frustum, a ball, a ring and an
  elliptic prism in random poses against a box and a cylinder: volume
  additivity, cut-then-fuse, commutativity, each result checker-green at
  `Full`, sharded. Whatever counterexample it finds is shrunk into
  `tests/fixtures/regression/` or fixed here, never skipped. The corpus
  run's wall time before step 4 and after step 7 recorded in the plan,
  since every face pair now reaches the intersector (there is no bench
  yet — that is the measuring harness's — so this is a stopwatch, and
  ⚠ OPEN 3 reads it).

## Acceptance

- `ARRIS_PROPTEST_CASES=1000 cargo nextest run --workspace` green: step
  1's pcurve properties, steps 2 and 3's hit properties, step 6's and
  step 8's identities.
- `geom/c3-conic-hits` and every `boolean/*` fixture of steps 4 to 7
  passing every corpus stage against Open CASCADE — counts, volume, area,
  centroid, classifications, STEP, tessellation — each result's
  `Report::unchecked` empty at `Full`, every edge at its faces' tolerance
  (ADR-0018's tripwire).
- Every existing fixture unchanged but `boolean/elliptic-operand-cut` —
  *amended at step 4*: and the three `sweep/extrude-elliptic*`, whose
  numbers move by 1e-15 when a shape with an extrusion face takes the
  Gauss–Kronrod volume properties the oracle now needs.
- `grep -rn "quadric guard" crates docs/ARCHITECTURE.md docs/DATA-MODEL.md`
  finds nothing in the present tense; `pcurve_on`,
  `intersect_curve_surface` and `intersect_curves` are `Unsupported` only
  in the NURBS arms the goal names, held by a test each.

## Docs to update on completion

- `docs/DATA-MODEL.md` §Pcurves, §Curves, §Tolerances — most lands with
  steps 1 to 3 and 6; retirement checks the present tense and that no
  sentence still says "C3's" or "no fitted fallback" of these arms.
- `docs/ARCHITECTURE.md` — §Operations' guard and closure-gap sentences,
  the revolve and elliptic-cylinder paragraphs, the pave model's account
  of singular vertices and closed contacts, the crate table's
  `arris-geom` row.
- `docs/ROADMAP.md` §C3 — the status line; the conic-hits, pcurve and
  guard bullets done, the pcurve bullet's "rather than from a projection"
  restated as ⚠ OPEN 1 was answered; "Beside the cycles" — the binding's
  "until then a script that fillets and then cuts meets the quadric
  guard" is no longer the reason it waits.
- `docs/adr/README.md` — ADR-0021 in the index.
- `docs/BACKLOG.md` — new lines for whatever steps 5 to 8 refuse by name;
  the refactoring line "fitted pcurve fallback is cylinder-only" is in a
  gitignored note, not here. The fillet-corner line (a tilted great
  circle with a fitted pcurve on the sphere) loses its blocker and says
  so.
- `AGENTS.md` current state — the guard lifted, ADR-0021, and what C3 has
  left: features a tolerance apart.

## Open questions

- OPEN 1, answered at step 1: **projection only; `MeetCurve` does not
  change.** Over 1000 random torus sections the projected pcurve adds at
  most 0.82 of a tolerance to what the fitted 3D curve is off the exact
  branch already — and that is the finding: the fitted curve is held to
  its two surfaces, not to the branch, so along a grazing section it sits
  up to sixteen tolerances off the branch *within the torus*, and the
  branch's exact (u, v) would be that far off the curve the edge carries.
  `SectionBranch::uv` is no pcurve of the fitted curve at all; it stays
  as the test's reference. (The lateral slack of a section fit at a
  grazing crossing is ADR-0019's deviation measure doing what it says;
  whether an edge should be held to the branch instead is the tolerance
  plan's question, noted there when it opens.) The roadmap bullet's
  "rather than from a projection" is restated at retirement. The
  reasons it was the recommendation stand as well: a pcurve has to be
  rebuildable for a block of an edge a later boolean cuts, and for the
  reader's edges, when no branch exists, and a second source for the
  same pcurve is a second thing to keep consistent.
- OPEN 2, answered at step 6: **through is built, beside is refused by
  name** (ADR-0021). The degenerate edges were never the difficulty;
  the (u, v) polygons of a pcurve passing beside the point are, and they
  are a backlog line of their own, not the tolerance plan's.
- From step 1, answered at step 6 — none of the three, as written: the
  loop in (u, v) does not take it, and the pave model now reads *through*
  by `pcurve_on`'s band, so a block never ends "up to a tolerance short"
  of a vertex it was paved by. The note as it stood: a section that passes an apex or a pole
  *between* the band (a quarter tolerance) and the vertex's tolerance is
  not through it for `pcurve_on`. Paved at the singular vertex, each
  block ends up to a tolerance short of the point, and its pcurve is the
  honest projection — valid under E4, but its `u` hooks by up to a right
  angle over the last stretch, as long as the miss, and ends off the
  degenerate edge's `v` by the miss over the radius. Exact conics miss
  by rounding and never see this; a fitted section can (it is held to a
  quarter tolerance of its surfaces, not of the pole). Step 6 decides
  whether the loop in (u, v) takes that, or the section is refitted
  through the vertex, or the pose is ⚠ OPEN 2's refusal.
- The exact arms are untouched by the singular rule: a ruling's `Line`
  runs through the apex and a meridian's over a pole, as before.
  ADR-0021 says so: "never" is the fitted pcurve's, and the exact lines
  are outside the rule — though a boolean paves them at the vertex all
  the same, so no edge it makes runs through either.
- ⚠ OPEN 3 — **does the guard's removal slow the existing corpus?**
  (agent, at step 8; the human if it does.) Plane and cylinder operands
  pay nothing new; the question is a filleted body's many blend faces,
  each pair now traced and fitted at up to 100 ms (ADR-0019). If a
  fixture goes past seconds, the shared-fit backlog line becomes a step
  here or a plan of its own — the human's call.
