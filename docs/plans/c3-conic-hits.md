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
  commit body says which.
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
- [ ] Step 3 **[2]** — A conic against a torus, two coplanar conics, and
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
- [ ] Step 4 **[2]** — The guard lifted for the ruled kinds: a cone and an
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
- [ ] Step 5 **[3]** — The guard gone: a sphere and a torus as operand
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
- [ ] Step 6 **[3]** — A section through an apex or a pole, and ADR-0021.
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
- Every existing fixture unchanged but `boolean/elliptic-operand-cut`.
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
- ⚠ OPEN 2 — **a section through an apex or a pole: built, or refused by
  name?** (agent, at step 6.) Planned as built. The fallback, if the
  result's degenerate edges cannot be made robust in one step, is a named
  refusal with the regression fixtures above and the pose handed to the
  tolerance plan with the tracers' singular points.
- For step 6, from step 1: a section that passes an apex or a pole
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
  runs through the apex and a meridian's over a pole, as before. Whether
  ADR-0021's "never" covers them is step 6's to say when it writes it.
- ⚠ OPEN 3 — **does the guard's removal slow the existing corpus?**
  (agent, at step 8; the human if it does.) Plane and cylinder operands
  pay nothing new; the question is a filleted body's many blend faces,
  each pair now traced and fitted at up to 100 ms (ADR-0019). If a
  fixture goes past seconds, the shared-fit backlog line becomes a step
  here or a plan of its own — the human's call.
