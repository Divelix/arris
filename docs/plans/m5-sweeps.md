# Plan: m5-sweeps

- Started: 2026-09-11
- Milestone: M5 (cycle C1, docs/ROADMAP.md)
- Idea (verbatim from the human): "/plan m5-sweeps" — the roadmap's M5
  section is the brief; no idea file.

## Goal

A sketched profile becomes a solid. `geom::Profile` is a plane and loops
of lines and arcs — an outer loop and holes, in the plane's own (u, v),
orientation-free — validated once (closed, non-crossing, holes inside the
outer and outside each other) and turned into curves with exact pcurves.
`ops::extrude(m, &profile, direction, length)` sweeps it along its plane's
normal, either way, onto planes and cylinders; `ops::revolve(m, &profile,
axis, angle)` sweeps it about an axis in its plane, a partial turn with
two flat ends or a full turn with seams, onto planes, cylinders and — where
an oblique segment or an arc demands it — cones, spheres and tori *as
surfaces*, their booleans being C3's. Both enter the builder through
`assemble`, since a sweep's faces are known outright (ADR-0004's entry, as
`transform` already uses it), and both record every entity `Generated`
from a role that names the profile part it came from: caps from the
profile face, sides and their edges from the profile's segments, rises and
their vertices from the profile's vertices. Every `sweep/*` fixture
passes every corpus stage, the cycle's corpus runs in CI with nothing
ignored, and Pappus's theorems hold at a thousand random profiles.

## Non-goals

No sweep along a path, loft or draft (C5). No profile touching or crossing
its revolve axis — both are typed refusals here, the apex and its
degenerate edges being C2's. No oblique extrusion (`⚠ OPEN` 3). No arc
whose full circle crosses the axis (a spindle torus; `R > r` stays the
data model's rule). No NURBS segment in a profile, no NURBS surface from
a sweep. No sheet body (`⚠ OPEN` 1). No new arm in `intersect_surfaces` or
`intersect_curve_surface`: the checker's S5 and B1 stay **unchecked** on
a cone, sphere or torus face, never wrong and never quietly passed, and
an oracle fixture on those surfaces waits for C2's intersector (`⚠ OPEN`
2). No boolean over a swept quadric face (C3). No merging of coplanar
neighbours, no fitted pcurve fallback on the surfaces of revolution — the
sweeps' curves are all exact there.

## Design deltas

- **`arris-geom` — `Profile`** (01 §Operations "plain values from
  `arris-math`/`arris-geom`"; 02 gains a §Profiles under §Pcurves):
  `pub mod profile` with `Profile { plane: Frame, outer: ProfileLoop,
  holes: Vec<ProfileLoop> }`, `ProfileLoop::{Circle { center: Point2,
  radius }, Path { start: Point2, segments: Vec<ProfileSegment> }}`,
  `ProfileSegment::{LineTo(Point2), ArcTo { to: Point2, via: Point2 }}` —
  one-to-one with the recipe grammar of `tests/fixtures/README.md`, the
  consumer's sketch as it is drawn, loop orientation irrelevant.
  `Profile::edges(&self, tol: Tolerance) -> Result<Vec<Vec<ProfileEdge>>,
  ProfileError>` validates and orients: every path closes within
  `tol.linear` (the last segment's end *is* the start), has at least two
  segments, and no segment is shorter than `tol.linear`; an arc's `via`
  is off the chord by more than `tol.linear` (else `DegenerateArc`); each
  loop, discretised by `region2` at the minimum counts, has a mean width
  above `tol.linear` (`ZeroArea`), crosses neither itself
  (`SelfIntersecting`) nor another loop (`Crossing`); every hole lies
  inside the outer (`HoleOutside`) and outside every other hole
  (`NestedHoles`), by winding. The outer is returned counter-clockwise
  about the plane's normal and every hole clockwise, reversed from the
  consumer's order where needed; a `ProfileEdge` is the segment's 3D
  `Curve` (a `Line`, or a `Circle` in the plane whose `Z` is `±` the
  normal so the parameter runs from the segment's start through `via`),
  its `range`, its exact in-plane `Curve2` (by `pcurve_on`), its two
  endpoints, and the `(loop, segment)` indices *as the consumer wrote
  them*, with `reversed` saying whether orientation flipped the loop. A
  circle loop is one closed edge whose one vertex sits at `center +
  radius · plane.x`, where the oracle's `gp_Circ` on the plane's `Ax2`
  puts it. Errors name loop and segment indices: `ProfileError::{
  NotClosed { loop, gap }, TooFewSegments { loop }, ShortSegment { loop,
  segment }, DegenerateArc { loop, segment }, ZeroArea { loop },
  SelfIntersecting { loop, segments: [usize; 2] }, Crossing { loops:
  [usize; 2] }, HoleOutside { hole }, NestedHoles { holes: [usize; 2] }}`.
  `Profile::area_and_centroid(tol)` — the plane region's area and (u, v)
  centroid by `integrate::region_integral` over the oriented edges — is
  what the Pappus tests read and what the extrude and revolve use to
  refuse a zero-thickness result and to pick which side the material is
  on.
- **`arris-geom` — `pcurve_on` on the surfaces of revolution** (02
  §Pcurves): the exact arms a revolve needs, each a `Curve2::Line` at the
  curve's own parameter, verified by `check_on` like the cylinder's —
  on a **cone**, a ruling (a line through the apex) at constant `u`, and
  a circle about the axis at constant `v`; on a **sphere**, a circle about
  the axis at constant `v` (a latitude) and a circle through both poles
  at constant `u` (a meridian; `v` running with or against `t` as the
  circle's `Z` lies in the equatorial plane one way or the other); on a
  **torus**, a circle about the axis at constant `v` and a circle of the
  tube at constant `u`. A `u` origin is the offset of the curve's `X`
  from the surface's, in `[0, 2π)`, running in the sense of the curve's
  `Z` against the surface's, as on the cylinder. Every other (curve,
  surface) pair on these three stays `GeomError::Unsupported` by name —
  an oblique section of a cone, a NURBS — until an operation makes one
  (a fitted fallback over `Surface::project` is a backlog line, not a
  step).
- **`arris-topo` — `Role`** (02 §Provenance; a representation-crate
  delta): `Role::{Extrude(SweepPart), Revolve(SweepPart)}` beside `Box`
  and `Cylinder`, closing the "M5 adds the profile's roles" note.
  `SweepPart::{Body, Shell, StartCap, EndCap, Side { loop, segment },
  StartEdge { loop, segment }, EndEdge { loop, segment }, Rise { loop,
  vertex }, StartVertex { loop, vertex }, EndVertex { loop, vertex }}`,
  indices the consumer's own (`loop` 0 is the outer, holes from 1;
  `vertex` is the index of the segment that *starts* there — a circle
  loop has segment 0 and vertex 0). In a full revolve there is no
  `EndCap`, no `EndEdge`, no `EndVertex`: the start edges are the seams
  and the start vertices are the only ones. `Display` as `extrude:…` /
  `revolve:…`; `serde` with the feature.
- **`arris-ops` — the operations** (01 §Operations, §Errors, §Facade):
  - `extrude(m, profile: &Profile, direction: Vec3, length: f64) ->
    Result<(Body, Provenance), OpError>`: `direction` normalised and held
    to `±` the plane's normal within `angular_tolerance`
    (`Reason::DirectionNotNormal` otherwise, `⚠ OPEN` 3), `length`
    finite and positive (`NotPositive`). The profile face keeps its
    plane's frame whichever way the sweep goes — the facade's "a planar
    face's frame *is* the answer" — and is the cap whose outward normal
    opposes the sweep, the other cap the same loops on the translated
    plane. A line segment sweeps a plane, an arc a cylinder whose frame is
    `Frame::new(center, ±normal, plane.x)` so a circle loop's seam stands
    at its vertex's rise, as the oracle's does; every side face's loop is
    start edge, rise up, end edge back, rise down (a seam used twice on a
    circle loop), every pcurve exact through `pcurve_on`.
  - `revolve(m, profile: &Profile, axis: Axis, angle: f64) -> Result<
    (Body, Provenance), OpError>`: the axis lies in the profile's plane
    within the tolerances (`Reason::AxisNotInProfilePlane`); `angle` in
    `(0, 2π]` (`NotPositive`, `Reason::AngleAboveTurn`), a full turn when
    within `angular_tolerance` of `2π`; the profile lies wholly on one
    side of the axis — every vertex and every arc's nearest approach at a
    positive distance above `tol.linear` — else `Reason::ProfileTouchesAxis`
    (within it) or `Reason::ProfileCrossesAxis` (across it); an arc whose
    centre is nearer the axis than its radius is `Reason::SpindleTorus`.
    The surfaces of revolution all share one frame convention: origin on
    the axis, `X` the unit radial from the axis into the profile's plane
    (so `u = 0` *is* the profile plane and every seam lies in it), `Z`
    the axis direction — except a cone whose radius shrinks along the
    axis, which takes `Z = −axis`, since the data model's cone grows
    along `+Z` with `α ∈ (0, π/2)`. A segment parallel to the axis
    sweeps a cylinder, perpendicular a plane (an annulus or a sector of
    one: two loops of one closed circle each in a full turn, one loop of
    two arcs and two rises otherwise), oblique a cone with its apex on
    the axis; an arc centred on the axis a sphere, elsewhere a torus of
    `R` its centre's distance and `r` its radius. Every vertex sweeps a
    `Curve::Circle` with `Z = +axis` and `X` the radial, over `[0,
    angle]`; each swept face's loop is start edge, rise, end edge, rise,
    with the start and end edges one seam in a full turn and the end
    edges the profile's curves rotated by `angle` otherwise; the two flat
    ends of a partial turn are the profile face (outward normal against
    the turn) and its rotated copy. Every pcurve is exact through
    `pcurve_on`, a seam's second use translated by the period. The face
    use orientation is decided per face by the surface normal against
    the swept segment's outward in-plane normal (material on the loop's
    left), uniform over a face by construction.
  - Both build an `Assembly` of all-`New` specs in one fixed order —
    vertices per loop in oriented order (start ring, then end ring),
    edges (start, end, rises), faces (start cap, end cap, sides per loop
    per segment) — and `finish` inside one transaction, the checker on
    the output as every operation; ids are a function of the profile
    alone, guarded by the dumps. Every tolerance is `default_tolerance`.
    Provenance is one `Generated` per entity from its `SweepPart`,
    exactly as the primitives' `roles` writes it.
  - `Reason` gains `DirectionNotNormal`, `AxisNotInProfilePlane`,
    `ProfileTouchesAxis`, `AngleAboveTurn`, `SpindleTorus`
    (`ProfileCrossesAxis` and `ZeroThickness` already exist and are used
    here for the first time). `OpError` gains `Profile(ProfileError)` — an
    invalid profile has no entities to name, so it is neither
    `InvalidInput` nor `Degenerate`; a new row in 01 §Errors.
  - `ops::planar_face` as the roadmap names it does **not** become a
    public operation in this plan (`⚠ OPEN` 1): the cap construction the
    two sweeps share is `Profile::edges` plus a private face builder in
    `ops::sweep`, and 01 §Operations' sweep paragraph is rewritten at
    retirement to say so. No ADR: the value-not-topology decision is
    already 01's, and `assemble` as the entry is ADR-0004's.
- **`arris-debug`** (01 §Formats and tools): the corpus runner builds a
  `profile` step into a `geom::Profile` kept beside the bodies
  (`Chain::profiles`; `Made` is unchanged and a profile step makes no
  body and needs no accounting), fills the `extrude` and `revolve` arms,
  and its `CorpusError::Unsupported` is retired with the last op that
  raised it, together with `crates/arris/tests/corpus.rs`'s
  unsupported-op test. `prop::profile::{rectilinear, general}` — random
  profiles in a random plane pose: a staircase polygon whose segments
  are parallel or perpendicular to a given axis (planes and cylinders
  only), and a star polygon with random arcs and a random hole (every
  surface kind), each with the axis at a positive distance and the
  extrude and revolve parameters to go with it; `prop::sweep` is the
  Pappus oracle: `A·L` and `2A + P·L` for an extrude, `θ·ρ̄·A` and `θ·Σ
  ρ̄ᵢℓᵢ (+ 2A)` for a revolve, with `A`, `ρ̄` and the per-edge centroid
  distances integrated in the plane by `region_integral` — an
  independent path from `measure`'s flux over the swept faces. The
  `inspect` skill gains the profile: `render_png` of the profile's loops
  in their plane through `polyline_of`, for a profile the validator
  refuses.
- **Fixtures** (`tests/fixtures/README.md`; each with `expected.json`
  from `expected.py`, the corpus lint green, a row in roadmap §C1
  acceptance corpus in the same commit): the three existing `sweep/*`
  un-ignored; `sweep/revolve-l-profile` (an L of six segments, `x ∈ [1,
  3]`, `z ∈ [−1, 1]` less the notch `[2, 3] × [0, 1]`, revolved 270°:
  every face a plane or cylinder, a `u` range past `π` with no seam —
  8.25π = 25.9181, area 22.5π + 6 = 76.6858, 12/18/8/8, genus 0);
  `sweep/extrude-slot` (a stadium of two lines and two semicircular arcs,
  straight length 20, radius 5, with a circular hole r 2 at its centre,
  extruded 8: partial cylinder faces with no seam beside a seamed one,
  the three cylinders' boxes apart so S5 decides every pair without a
  cylinder–cylinder arm — 2127.7876, area 1203.8053, 10/15/7/9, genus
  1); `sweep/extrude-downward`
  (the plate-with-hole profile on the plane `z = 10`, extruded 10 along
  `−z`: `through-hole`'s solid and numbers by the third path, the profile
  face on top keeping its frame). Every property-test failure becomes one
  more. The quadric-faced revolves (a trapezoid's frustum, an arc's
  barrel, a circle's ring) are closed-form unit tests, not fixtures, in
  this plan (`⚠ OPEN` 2).
- **CI and the lint** (roadmap §M5's third bullet): `ci.yml`'s suite runs
  with ignored tests included, and `corpus_lint` holds every solid fixture
  under `primitive/`, `boolean/`, `sweep/` and `provenance/` that the
  runner would compare — not `degenerate`, not `expect_error` — to a
  committed `dump.txt` per variant, which a fixture only has once it
  passed and was blessed: zero ignored fixtures as a test, not a grep.

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness or
bound has to be established here.

- [ ] Step 1 **[2]** — `geom::profile`. The types, `Profile::edges`,
  `area_and_centroid`, `ProfileError`; the recipe grammar's `Loop` and
  `Segment` convert to it in `arris-debug`. Tests (1000 cases, random
  plane poses): a random star polygon with arcs and holes, given in either
  orientation, comes back outer counter-clockwise and holes clockwise with
  the consumer's indices intact; every edge's `Curve2` image is its
  `Curve` at the same parameter to 1e-12·scale and consecutive edges meet
  at their vertices; `area_and_centroid` of a rectangle, a disc, a
  stadium and a rectangle with a hole match the closed forms to 1e-12
  relative; every `ProfileError` variant from a hand-built profile — a
  gap, one segment, a collinear `via`, a bow-tie, two crossing loops, a
  hole outside, a hole inside a hole — named with the right indices; the
  three committed `sweep/*` recipes and the three new ones load into
  valid profiles.
- [ ] Step 2 **[2]** — `pcurve_on` on cone, sphere and torus: the six
  exact arms of the deltas. Tests (1000 cases, `prop::geom` poses): the
  image of every arm's pcurve is the curve at the same parameter to
  1e-12·scale over its range, including circles offset from the
  surface's `X` and running against its `Z`; a curve off the surface is
  `NotOnSurface`; an oblique line on a cone and every NURBS are
  `Unsupported` naming the pair; `crates/arris-geom/tests/pcurve.rs`'s
  existing plane and cylinder tests unchanged.
- [ ] Step 3 **[3]** — `ops::revolve` over planes and cylinders, and the
  runner's `profile` and `revolve` arms. The `Role` variants, the new
  `Reason`s and `OpError::Profile`, the sweep's private cap builder and
  assembly order, the seam layout of a full turn, the annulus of a
  perpendicular segment, the flat ends of a partial turn, the face use
  orientation rule, provenance. Fixtures: `revolve-tube` and
  `revolve-quarter` un-ignored and blessed; `revolve-l-profile` authored,
  its oracle generated, passing and blessed. Tests: `prop::profile::
  rectilinear` at 1000 cases — the checker at `Full` with nothing
  violated *and nothing unchecked* (planes and cylinders only), volume
  and area to the Pappus oracle at 1e-9 relative, the mesh closed within
  the inscribed bound, one `Generated` per entity and every `SweepPart`
  of the profile present, the dump identical on two runs; the tube's
  numbers equal to `boolean/coaxial-cut`'s; a tube profile given
  clockwise gives the same dump as counter-clockwise; an angle within
  `angular_tolerance` of `2π` is the full turn and `2π + 1e-3` is
  `AngleAboveTurn`; a profile touching the axis at a vertex and along a
  segment is `ProfileTouchesAxis`, one straddling it `ProfileCrossesAxis`,
  an axis tilted out of the plane `AxisNotInProfilePlane` — the model
  untouched after each.
- [ ] Step 4 **[3]** — `ops::revolve` over cones, spheres and tori: the
  oblique segment (both cone orientations, the `Z = −axis` case), the arc
  centred on the axis and off it, `SpindleTorus`. Tests: `prop::profile::
  general` at 1000 cases — the checker at `Fast` green and at `Full` with
  no violation and every unchecked row a pair or a cast on a cone,
  sphere or torus face and nothing else, volume and area to the Pappus
  oracle at 1e-9, the mesh closed within the inscribed bound, the dump
  identical on two runs; the closed forms: a trapezoid's frustum
  (`πh(R² + Rr + r²)/3` less its bore), an arc's barrel (a spherical
  zone, `πh(3a² + 3b² + h²)/6` less its bore), a circle's ring
  (`2π²Rr²`, `4π²Rr`, 1/2/1/1, genus 1, the same counts and Euler line as
  `sample::torus`); each written to STEP and read back by the oracle's
  reader with its volume (`oracle::compare` over a scratch fixture); a
  circle loop whose circle crosses the axis is `SpindleTorus`.
- [ ] Step 5 **[2]** — `ops::extrude`, its fixtures, the runner's
  `extrude` arm, the unsupported-op test and `CorpusError::Unsupported`
  retired. Fixtures: `extrude-plate-with-hole` un-ignored and blessed;
  `extrude-slot` and `extrude-downward` authored, oracles generated,
  passing and blessed. Tests: `prop::profile::general` at 1000 cases
  along `+n` and `−n` — the checker at `Full` with no violation and every
  unchecked row a cylinder–cylinder pair that is not coaxial (two arcs
  whose boxes overlap: the quadric-curve `⚠ OPEN`, C2's arm) and nothing
  else, `A·L` and `2A + P·L` at 1e-9, the mesh
  closed, provenance complete, the dump identical on two runs, the profile
  face's frame equal to the profile's plane either way; the cross-check:
  for a line-only outer with one circular hole, `extrude(profile)` and
  `cut(extrude(outer), primitive_cylinder(hole))` agree in volume, area
  and counts at 200 poses; `DirectionNotNormal` for a direction 1° off
  the normal, `NotPositive` for a zero length, every `ProfileError`
  surfacing as `OpError::Profile` with the model untouched.
- [ ] Step 6 **[1]** — The corpus in CI with nothing ignored. `ci.yml`
  runs the suite with ignored tests included; `corpus_lint` holds every
  comparable solid fixture in the four areas to a committed dump per
  variant, with a scratch fixture missing its dump failing the lint;
  `crates/arris/tests/corpus.rs` carries no `#[ignore]`; the roadmap's
  §C1 acceptance corpus table complete with every `sweep/*` row.

## Acceptance

`ARRIS_PROPTEST_CASES=1000 cargo nextest run --workspace` with ignored
tests included, green, and the same with `-p arris-ops --features
parallel` and `-p arris --features parallel`, byte-identical dumps: every
`sweep/*` fixture — `extrude-plate-with-hole`, `extrude-slot`,
`extrude-downward`, `revolve-tube`, `revolve-quarter`, `revolve-l-profile`
— passing every corpus stage (the checker at `Full` with nothing
unchecked, counts and genus, the oracle's reading of the STEP, `measure`
to 1e-9, the mesh closed within `mesh_volume_rel`, the probes classified
as the oracle classifies them, the provenance accounting, the dump); zero
ignored fixtures under `primitive/`, `boolean/`, `sweep/`, `provenance/`
by the lint of step 6; the Pappus properties of steps 3, 4 and 5 at 1000
cases; `uv run --project tools/oracle tools/oracle/selftest.py` green over
the new fixtures; the layer check and the wasm build pass; CI green on
`main`. Then `/close-cycle`: the drift review empty, tags `m5` and `c1`
(the human's).

## Docs to update on completion

- `docs/ROADMAP.md` §M5 — status line: date, what was retired (a sketch
  as a value, sweeps assembled outright, the surfaces of revolution with
  their frame convention, the seam in the profile plane, Pappus as the
  oracle), the numbers, what stayed unchecked and why; §Fixtures — the
  `profile` step now built, the lint's dump rule; §C1 acceptance corpus
  — the three new rows (written at their steps, checked here).
- `docs/ARCHITECTURE.md` §Operations — the sweep paragraph rewritten:
  `Profile` as the value, `extrude`'s and `revolve`'s rules, the frame
  convention, `assemble` as the entry, no `planar_face` operation
  (`⚠ OPEN` 1's answer); §Errors — `OpError::Profile`, the new
  `Reason`s; §Formats and tools — the runner's profile steps,
  `prop::profile`, `prop::sweep`, the lint's dump rule, `Unsupported`
  retired; §Facade — the extrude/revolve row now real.
- `docs/DATA-MODEL.md` §Pcurves — the cone, sphere and torus arms, the
  "until revolve needs them (M5)" clause gone; §Profiles (new, under
  §Pcurves) — the types, the validation rules, the orientation rule;
  §Provenance — `Role::{Extrude, Revolve}` and `SweepPart`, the "M5 adds
  the profile's roles" note closed; §Surfaces — the cone's `Z` rule for a
  narrowing sweep noted beside the table.
- `tests/fixtures/README.md` — the three new fixtures, the `profile` step
  built, the dump rule of the lint, the retired `Unsupported` sentence.
- `.agents/skills/inspect/SKILL.md` — rendering a refused profile.
- `docs/BACKLOG.md` — lines for: oracle fixtures on cone, sphere and
  torus faces once C2's intersector lets S5 and B1 decide them
  (`⚠ OPEN` 2); oblique extrusion (C5, `⚠ OPEN` 3); a fitted pcurve
  fallback on the surfaces of revolution when an operation needs one; a
  spindle torus (`R < r`) when an operation needs one; `planar_face` as
  a sheet body with C7's sheet bodies (`⚠ OPEN` 1). The `Builder::finish`
  Sheet line stays.
- `AGENTS.md` current state — M5 done, C1 closed, next `/close-cycle`
  then C2.

## Open questions

All three decided by the human on 2026-09-11 as recommended; kept here
so the reasoning stays with the plan until retirement.

- `⚠ OPEN 1:` **`planar_face`'s public surface.** *Decided: the
  recommendation.* The roadmap lists
  `ops::planar_face`; a public operation returning a body would be a
  `Sheet` of one face, which `Builder::finish` refuses today (the backlog's
  C7 line) and which nothing downstream consumes in C1 (`measure`,
  `step::write` and the booleans all refuse a non-solid). Recommendation:
  the sketch stays a `geom::Profile` value validated by `Profile::edges`,
  the cap builder is the sweeps' private helper, and a sheet-returning
  `planar_face` arrives with C7's sheet bodies — 01 §Operations rewritten
  to say so.
- `⚠ OPEN 2:` **Oracle fixtures on the quadric faces.** *Decided: the
  recommendation.* A revolve fixture
  with a cone, sphere or torus face cannot pass the corpus runner's `Full`
  stage — S5 has no plane–cone, plane–sphere, plane–torus or
  cylinder–torus arm and B1's ray has no closed form against them, so the
  rows are unchecked and the runner refuses unchecked rows by design —
  and an `#[ignore]`d fixture under `sweep/` would fail the milestone's
  own "zero ignored" line. Recommendation: in M5 the quadric-faced
  revolves are held to closed forms and to Pappus (step 4), with the
  STEP read back by the oracle's reader for the volume; the oracle
  fixtures (`revolve-frustum`, `revolve-barrel`, `revolve-ring`) are a
  backlog line for C2, whose intersector is what makes them runnable.
  Alternatives: a fifth fixture area kept ignored until C2; a recipe
  field that lets the runner tolerate named unchecked rows (weakens the
  corpus).
- `⚠ OPEN 3:` **Oblique extrusion.** *Decided: the recommendation.*
  Open CASCADE's prism takes any
  direction; an oblique extrusion of an arc is a cylinder of elliptical
  section, which no `Surface` variant holds, and of a line a plane, which
  is fine. Recommendation: refuse any direction off the plane's normal
  with `Reason::DirectionNotNormal` rather than support line-only
  profiles obliquely and arcs not; C5's sweep along a path is where the
  general case belongs.

## Findings

Recorded by the step that met them; each is a departure from the design
deltas above, not a new decision.
