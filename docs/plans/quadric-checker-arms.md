# Plan: quadric-checker-arms

- Started: 2026-09-15
- Milestone: C2, the application gate (docs/ROADMAP.md §C2, the lines
  "Cone, sphere and torus in the intersector…" and "The checker's S5 and
  B1 arms, and `classify_point`…")
- Idea (verbatim from the human): "ok, /plan it then" — after
  `/work fillet-and-chamfer step 8` stopped on this dependency, the
  sequencing `plans/fillet-and-chamfer` §Open questions already decided
- No idea file: the scope is the roadmap's two lines as ADR-0007 narrowed
  them

## Goal

Every pair of faces a revolve or a blend makes on a cone, sphere or
torus can be checked by closed form, because every such pair shares an
axis. Two surfaces of revolution about one axis meet where their
meridians meet in a plane through that axis:
- a plane perpendicular to the axis, a cylinder, a cone, a sphere and a
  torus each cut that plane in lines or circles;
- two lines or circles meet in 2D by closed form;
- each point where they meet sweeps a circle about the axis, or is a
  single point when it lies on the axis.

A sphere is a surface of revolution about any line through its centre.
So this one arm also covers every plane–sphere and sphere–sphere pair,
and every quadric whose axis passes through a sphere's centre. A plane
through the axis cuts a cone in two rulings and a torus in two circles:
these are the caps of a partial revolve. A line meets a cone, sphere or
torus by closed form, so B1's containment ray and `classify_point` work
on those faces.

When this plan is done:
- M5's quadric-faced revolves are corpus fixtures, `sweep/revolve-frustum`,
  `revolve-barrel` and `revolve-ring`, passing every stage with nothing
  unchecked;
- random general revolve profiles check at `Full` with nothing
  unchecked;
- `plans/fillet-and-chamfer` steps 8 and 9 can hold a torus, cone or
  sphere blend to `Full`;
- the boolean refuses exactly what it refused before.

## Non-goals

- Quadric pairs in general position, C3's: a plane oblique to a cone's or
  torus's axis, a plane parallel to that axis and off it, and two cones
  or tori on different axes. Each stays `GeomError::Unsupported`, as an
  explicit arm. The quadric-curve `⚠ OPEN` in DATA-MODEL §Curves stays
  open: every curve here is a line or a circle.
- Conics against a cone, sphere or torus in `intersect_curve_surface`.
  Only lines are needed: the checker casts lines, and the boolean's
  edge–face hits stay guarded (below).
- Booleans with cone, sphere or torus operand faces. The guard keeps
  today's refusal. A corpus for them is C3's, beside the general pairs.
- A result that mixes kinds — a transversal curve beside a tangent one,
  or a curve beside a point on the axis. It is `Unsupported` here
  (§Design deltas); the result type that carries it is decided with
  C3's ADR.
- The blends themselves (`plans/fillet-and-chamfer` steps 8–9), NURBS,
  a spindle torus.

## Design deltas

- **ADR-0008** (step 1): coaxial surfaces of revolution meet through
  their meridians.
  - The decision:
    - One arm replaces a table of pairwise closed forms.
    - A sphere is taken about the line to the other operand's axis, or
      along the plane's normal.
    - `tol.linear` carries over exactly: distances in the plane through
      the axis are 3D distances between points at one angle, and a
      point's distance to a surface of revolution is its distance to the
      full symmetric meridian section in that plane.
    - Tangency and coincidence are decided in 2D.
    - A meeting on the axis becomes a point.
    - A result that mixes kinds is refused.
  - It names the reference-tree module read, Open CASCADE's
    `IntAna_QuadQuadGeo` (its coaxial branches). It records the rejected
    alternative, one closed form per pair (fifteen arms, the sphere's
    free axis handled three times).
- **`arris-geom` public type `SurfaceIntersection`** gains
  `Points(Vec<Point3>)`, a breaking change named in step 1's commit. It
  means the surfaces meet only at isolated points: a touch (a plane
  tangent to a sphere, two spheres touching) or a crossing through a
  singular point (a plane perpendicular to a cone through its apex).
  The points come ascending along the shared axis. Every exhaustive
  match gains an arm: S5, the boolean's display and pave, and blend's
  `face_end`.
- **`intersect_surfaces`, the meridian arm** (DATA-MODEL §Curves gains
  its table):
  - Meridian sections in the plane through the axis, with `ρ` signed
    across the axis and `z` along it:
    - plane ⊥ axis: the line `z = h`;
    - cylinder: `ρ = ±R`;
    - cone: two lines crossing at the apex;
    - sphere: a circle centred on the axis;
    - torus: two circles centred at `ρ = ±R`, radius `r`.
  - Coaxial means axes parallel within `tol.angular` and apart by at most
    `tol.linear`. A sphere is coaxial with anything whose axis passes
    within `tol.linear` of its centre, and with every plane and sphere.
  - Each 2D meeting at `|ρ| > tol.linear` is a circle of radius `|ρ|`,
    and its `±ρ` mirror is the same circle, kept once. A meeting at
    `|ρ| ≤ tol.linear` is a point on the axis. A 2D touch within
    `tol.linear` is `Tangent`, and equal sections are `Coincident`.
  - The result is `Transversal`, `Tangent`, `Points`, `Coincident` or
    `Empty`. Anything that mixes kinds is `Unsupported` naming the pair
    (§Non-goals).
  - A circle's frame is centred on the axis, with `Z` and `X` from the
    first operand whose frame carries the axis, so its seam shares a
    plane with that surface's. The agent fixes the sphere–sphere and
    plane–sphere frames deterministically and writes them in DATA-MODEL,
    as the cylinder plan did for its ellipses.
  - Swapping the operands gives the same point sets. The existing
    plane–plane, plane–cylinder and cylinder–cylinder arms are unchanged:
    coaxial cylinders already have a closed form, and their dumps must
    not move.
- **`intersect_surfaces`, a plane through the axis** (step 2): a cone
  gives two `Transversal` rulings through its apex, and a torus its two
  meridian circles. A sphere is already the meridian arm's. The cylinder
  arm is unchanged.
- **`intersect_curve_surface`, lines against the quadrics** (step 3):
  - Line–sphere is a quadratic.
  - Line–cone is a quadratic over both nappes. A ruling is `Coincident`;
    a line parallel to a ruling gives one hit.
  - Line–torus is the quartic in `t` through `arris_math::roots`, with
    each crossing polished. A touch is found where an extremum of the
    distance along the line is within `tol.linear`.
  - A hit at a singular point, a cone's apex or a sphere's pole, takes
    `u = 0` and the singular `v` rather than `Surface::project`'s
    `Ambiguous`. That is the one stated exception to "`uv` is the
    surface's own projection".
- **The checker.**
  - S5 on `Points`: a point inside both faces that is not within
    tolerance of a vertex both faces reach, or of an edge they share, is
    `FacesIntersect`. The vertex clause covers two cones closing on one
    apex, which share a vertex but no edge.
  - `Classifier::contains` abandons a direction whose hit lands on a
    degenerate edge, which `Side::Boundary` already answers.
  - `ClassifyError::Geometry`'s docs narrow to NURBS (DATA-MODEL
    §Invariants S5 row, ARCHITECTURE §The checker).
- **The boolean guard** (step 1). In `boolean::pave`, a face pair or an
  edge–face pair whose boxes meet and that has a face on a cone, sphere
  or torus is refused before the intersector, as the same
  `OpError::Unsupported` naming the pair it gets today. The new arms
  widen no boolean silently (ARCHITECTURE §Operations).
- **The oracle:** `tools/oracle/oracle/geometry.py` intersects coaxial
  quadric and torus pairs through `IntAna_QuadQuadGeo`, and a line
  against a torus through `IntAna_IntLinTorus`. A point result is
  written as `"type": "point"`. What it cannot answer stably is recorded
  under tests/fixtures/README §Geometry fixtures, as before.
- **`plans/fillet-and-chamfer`** §Dependencies item 2 names this plan,
  and steps 8–9 wait on it (edited with this plan).

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness or
bound has to be established here.

- [x] Step 1 **[3]** — **The meridian arm, `Points`, and the boolean
  guard.**
  - ADR-0008. `SurfaceIntersection::Points`, with the new arm in every
    match that uses the enum. S5's `Points` rule: a raw-insert test builds
    a sphere face touching a plane face inside both and sees
    `FacesIntersect`; the same touch at a shared vertex passes.
  - The meridian arm as §Design deltas, and the boolean guard with a test
    that a revolve with a cone face against a box through it is still
    `Unsupported` naming the pair.
  - `intersect_surfaces.rs` properties over random coaxial pairs of every
    kind, in random poses:
    - the case matches the 2D closed form;
    - each circle lies on both surfaces;
    - swapping the operands gives the same points;
    - constructed touches are `Tangent` (a torus on a plane at `z0 ± r`,
      a torus inside a cylinder of radius `R ± r`, a sphere on a plane);
    - constructed apexes and poles on the axis are `Points`.
  - Non-coaxial poses: every one named in §Non-goals is still
    `Unsupported`.
  - Geometry fixture `geom/c2-quadric-pairs`, written by `generate.py` in
    the `tilt` pose from the blend and revolve poses:
    - a plane ⊥ axis on a cone, a sphere and a torus (crossing and
      touching);
    - a cylinder on a coaxial cone, sphere and torus;
    - a cone on a cone and a sphere on a torus;
    - a plane on a sphere in general position, and two spheres;
    - a plane through a cone's apex, as a point.
  - DATA-MODEL §Curves: the meridian table and frames. ARCHITECTURE
    §Geometry dispatch: partial support of the quadric pairs and
    `Points`. The existing corpus dumps are unchanged.
  - **Gate:** if 2D tangency or coincidence does not decide the 3D case
    within `tol.linear` somewhere — near a cone's apex, at a torus's inner
    equator, or on a sphere taken about a borrowed axis — stop and return
    to the human before step 2.
- [ ] Step 2 **[2]** — **A plane through the axis.**
  - Plane–cone gives two rulings and plane–torus two meridian circles.
    Properties at random poses, and `c2-quadric-pairs` gains both pairs.
  - These are the caps of M5's partial revolves against their cone and
    torus faces.
- [ ] Step 3 **[2]** — **Lines against the cone, sphere and torus.**
  - `intersect_curve_surface` as §Design deltas: the singular-point
    `uv`, and the torus quartic polished with a touch decided by the
    distance's extrema.
  - `intersect_curve_surface.rs` properties: every hit on both operands,
    and hits ascending. The constructed cases:
    - a tangent line to each surface, a ruling of the cone and a line
      through its apex;
    - a line through a sphere's pole and the torus's axis (no hit);
    - a line grazing the torus's inner and outer equators, as touches.
  - `c2-quadric-pairs` gains line pairs against the oracle, the torus's
    through `IntAna_IntLinTorus`.
- [ ] Step 4 **[2]** — **The checker and the classifier over the arms;
  the revolve tests' allowances gone.**
  - `Classifier::contains` over the line arms, abandoning a hit on a
    degenerate edge. `ClassifyError`'s docs narrow.
  - `revolve.rs`: `on_a_quadric` is deleted. The general-profile shards
    run with nothing unchecked. The frustum, zone and ring test and the
    cone, ball and pinch test assert `report.unchecked()` is empty.
  - A property failure is shrunk to a `regression/` fixture in this step.
    A mixed-kind result the shards reach is a finding: recorded in this
    plan, and returned to the human before the enum grows.
- [ ] Step 5 **[1]** — **M5's quadric revolves into the corpus.**
  - Fixtures, their oracle values from `expected.py`, each blessed:
    - `sweep/revolve-frustum`, a trapezoid's frustum less its bore
      (`2π(16 + 8 + 4)/3 − 2π`), with variants for the widening cone and
      for a quarter turn, whose caps are step 2's;
    - `sweep/revolve-barrel`, the arc's spherical zone less its bore
      (`π·2·(3·8 + 3·8 + 4)/6 − 2π`);
    - `sweep/revolve-ring`, the ring torus (`2π²·5·4`), with a variant
      for a quarter turn.
  - Every probe goes through `classify_point` on cone, sphere and torus
    faces.
  - `revolve.rs` drops its scratch-fixture comparison for the three
    cases it now leaves to the corpus. It keeps the closed-form and
    `sample::torus` assertions.

## Acceptance

- `cargo nextest run -p arris --test corpus --run-ignored all`:
  - `sweep/revolve-frustum`, `revolve-barrel` and `revolve-ring`, every
    variant, pass every stage against Open CASCADE with nothing
    unchecked;
  - every existing fixture's dump is unchanged.
- `cargo nextest run -p arris-geom --test oracle`: `geom/c2-quadric-pairs`
  matched.
- Steps 1–3's intersector properties and `revolve.rs`'s general-profile
  shards are green at the configured case count, with no unchecked
  allowance.
- The boolean refuses a pair with a cone, sphere or torus face exactly as
  before, and `boolean_prop.rs` is green.

## Docs to update on completion

- `docs/adr/0008-…`: written at step 1; the retirement checks it against
  the code.
- `docs/DATA-MODEL.md`:
  - §Curves: the meridian table, circle frames and `Points`; the
    `intersect_curve_surface` paragraph's line–quadric arms; the
    quadric-curve `⚠ OPEN`'s "Until then S5 reports…" sentence narrowed
    to the general positions.
  - §Invariants: the S5 row's point rule, and B1's no longer unchecked
    against the three kinds.
- `docs/ARCHITECTURE.md`:
  - §Geometry dispatch: the partial quadric support.
  - §The checker: the classifier on the quadrics.
  - §Operations: the boolean's quadric guard.
- `docs/ROADMAP.md` §C2:
  - the intersector and checker lines **Done** with their fixtures;
  - the intersector line's positions gain the plane through the axis
    (M5's partial revolves);
  - the accept line's three revolves no longer "instead of closed-form
    tests".
- `tests/fixtures/README.md` §Geometry fixtures: `c2-quadric-pairs`, the
  `point` type, what the oracle answers unstably for the new pairs.
- `tools/oracle/README.md`: the torus pairs, the line–torus hits, the
  `point` type.
- `docs/BACKLOG.md`:
  - a mixed-kind surface intersection result, with C3's ADR;
  - conic–quadric curve–surface arms, for the boolean's edge hits;
  - booleans with quadric operand faces behind the guard, with C3's
    corpus.
- `AGENTS.md` current state: the quadric checker arms landed, ADR-0008;
  **Next:** `/work fillet-and-chamfer` step 8.

## Open questions

None for the human. The agent decides two, each at its step:

- ~~The sphere–sphere and plane–sphere circle frames (step 1), fixed
  deterministically and written in DATA-MODEL.~~ Decided at step 1: a
  plane on a sphere takes the plane's frame at the sphere's centre, two
  spheres the line from the first centre to the second with `X` the
  first sphere's `X` or `Y`, whichever has the larger component across
  it; a sphere whose own `Z` is along the axis carries the axis itself
  (DATA-MODEL §Curves). Findings of step 1: the gate held — 2D tangency
  and coincidence decided every constructed and random case within
  `tol.linear`; the oracle's own branches do not — its plane–sphere touch
  is decided at machine epsilon (`empty` at a pole) and its
  cylinder–sphere test wants the sphere's axis on the cylinder's exactly
  (`unsolved` for a turned sphere) — both recorded in
  `tests/fixtures/README.md`, and the oracle test holds Arris's curves
  to the surfaces where the oracle is silent.
- Whether any mixed-kind result is reached by a revolve or a blend in
  C2's positions (step 4). By construction a hole-rim torus touches its
  cylinder and plane without crossing either, and the random profiles
  have measure zero on a touch, so none is expected. If the shards find
  one, it goes to the human with the fixture, not into a wider enum.
