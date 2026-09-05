# Plan: m1-geometry

- Started: 2026-09-05
- Milestone: M1 (cycle C1, docs/03-roadmap.md)
- Idea (verbatim from the human): "/plan m1-geometry" — the roadmap's M1
  section is the brief; no idea file.

## Goal

Every curve and surface C1 needs exists as a value and is proven: the
`Surface`, `Curve` and `Curve2` enums of 02-data-model with the Open CASCADE
parametrisations, evaluated with first and second derivatives; a point
projects onto every analytic variant and onto NURBS curves; every analytic
curve on a plane or a cylinder has a pcurve, exact where a `Curve2` variant
exists and a NURBS fitted to a stated tolerance otherwise; the C1
intersection table — plane–plane, plane–cylinder, line–plane,
line–cylinder, circle–plane, circle–cylinder — is an exhaustive dispatch
whose every other pair is `Unsupported`; `arris-math` carries the points,
frames, intervals, tolerance types, exact predicates and root finders the
rest of the cycle calls. It is proven by property tests at 1000 cases and by
a geometry oracle that evaluates the same surfaces, projections and
intersections in Open CASCADE. At the end, `cargo test --workspace` is
green on a workspace that still has no entity and no operation.

## Non-goals

No topology, arena, checker logic or operation (M2). No intersection
involving a cone, sphere, torus or NURBS surface, and no cylinder–cylinder
— every such pair is an explicit `Unsupported` arm (their evaluation and
projection *are* in, so the enums are complete). No NURBS surface beyond
evaluation and derivatives: no projection onto one, no fitting of one. No
NURBS degree elevation (backlog). No pcurve on a cone, sphere or torus
(M5 adds the arms revolve needs). No 2D curve–curve intersection and no
2D point classification (M4). No bounding boxes: the backlog's `Aabb`
line stays until an algorithm asks. No tessellation of curves beyond the
debug sampler of step 2.

## Design deltas

- **`arris-math` public API** (02-data-model §Conventions gains the new
  names): `Point3`, `Vec3`, `UnitVec3`, `Point2`, `Vec2`, `UnitVec2` as
  aliases of `nalgebra` types with `nalgebra` re-exported (ADR-0001,
  step 1); `Frame` (right-handed, `x × y = z`, constructed only through
  validating constructors), `Frame2` (a 2D origin and orthonormal `x`,
  `y` of *either* handedness — see the `Curve2` delta), `Isometry` (a
  rigid motion; `Frame::transformed`, so that "a transform is a frame
  change and nothing else" holds); `Interval` (closed, `lo ≤ hi`, may
  exceed a period); `Tolerance { linear, angular }` with
  `Precision::tolerance()`; `predicates::{orient2d, incircle}` over
  `robust`; `roots::{quadratic, cubic, quartic, newton_in_interval}`. The
  test-only constant `SCALE_EPS` does not exist: test tolerances are
  literals in tests, algorithm tolerances are `Tolerance` fields.
- **`arris-geom` public API**: `Surface`, `Curve` (the enums of 02 with the
  `Nurbs` variants arriving in step 8, so steps 2–7 dispatch over the
  analytic variants only — every dispatch gains its NURBS arm in step 8,
  which is the exhaustiveness rule in action); `SurfaceKind`, `CurveKind`,
  `Curve2Kind`; `Curve2` (step 9); `NurbsCurve`, `NurbsCurve2`,
  `NurbsSurface`; evaluation types `SurfaceEval { point, du, dv, duu,
  duv, dvv }`, `CurveEval { point, d1, d2 }`; projection results
  `SurfaceProjection { uv, point, distance }`, `CurveProjection { t,
  point, distance }`; intersection results `SurfaceIntersection::{Empty,
  Coincident, Transversal(Vec<Curve>), Tangent(Vec<Curve>)}` and
  `CurveSurfaceIntersection::{Points(Vec<CurveSurfaceHit>), Coincident}`
  with `CurveSurfaceHit { t, uv, point, tangent: bool }` sorted by `t`;
  `GeomError::{Unsupported { a, b }, Degenerate { .. }, Ambiguous { .. },
  NotOnSurface { .. }, Fit(FitError) }` over `thiserror` (`arris-geom`
  gains `thiserror`, the same crate `mesh` already uses; 01-architecture
  §Crates row updated). Free functions `intersect_surfaces`,
  `intersect_curve_surface`, `pcurve_on`, `project_to_plane`,
  `fit_curve2`.
- **02-data-model §Pcurves, `Curve2`**: `Circle { center, radius }` and
  `Ellipse { center, x, … }` cannot represent a circle traversed clockwise
  in (u, v), and a 3D circle shared by a cap and a wall *is* clockwise on
  one of the two planes whose normal opposes the circle's `Z`. Both
  variants become `{ frame: Frame2, radius… }` where `Frame2`'s
  handedness carries the direction of traversal. Lands in step 9; the doc
  changes in the same commit (decided, §Open questions).
- **02-data-model §Conventions**: `Tolerance`, `Isometry`, `Frame2` added;
  §Curves gains the intersection result types and the rule that an
  intersection curve's frame is Arris's own deterministic choice (the
  circle's `X` is the cylinder's, the ellipse's `X` the major axis in the
  direction of increasing `v`), matching Open CASCADE only where the
  *surface's* parametrisation is concerned.
- **`arris-debug::prop`** (01-architecture §Formats and tools): the M0
  strategies `unit_vec3` and `rotation` return `UnitVec3` and
  `UnitQuaternion` instead of arrays (a breaking change to a dev-facing
  API, named in step 1's commit); new `frame`, `pose`, `radius`,
  `point_in_box`, and `geom::{plane, cylinder, cone, sphere, torus, line,
  circle, ellipse, surface, curve}` strategies. Because `arris-debug`
  depends on `arris-geom` and `arris-math`, their property tests live in
  `crates/<crate>/tests/*.rs` (integration tests, where the types unify),
  never in `#[cfg(test)]` modules — the dev-dependency cycle the layer
  check allows produces two copies of the crate in a unit test.
- **`arris-debug`** gains `polyline_of(&Curve, Interval, n)` and
  `wireframe_of(&Surface, [Interval; 2], n)` so a failing intersection
  renders through the M0 rasteriser (the `inspect` skill's PNG row).
- **The oracle** (01-architecture §Formats and tools, `tests/fixtures/
  README.md`): a second fixture kind, `"kind": "geometry"`, under
  `tests/fixtures/geom/`; `expected.py` dispatches on it and writes OCCT's
  evaluations, projections and intersections; the corpus lint checks
  presence and hash for this kind and skips the body checks (decided,
  §Open questions).
- **CI**: the `test` job runs with `ARRIS_PROPTEST_CASES=1000`, the
  roadmap's acceptance count; the default stays 256 for the pre-commit
  hook.
- **ADR-0001** — `arris-math` exposes `nalgebra`'s point/vector/unit types
  by alias and re-exports the crate, so a `nalgebra` major bump is an Arris
  API change. Alternatives: newtypes (boilerplate on every operator, no
  solver access without unwrapping), own types (re-implementing what
  `SEED.md` §9 chose `nalgebra` for). Step 1.
- `docs/BACKLOG.md` gains "NURBS degree elevation" now (02 §NURBS names
  it as a primitive edit; no C1 step needs it).

## Steps

- [x] Step 1 — Math foundation and ADR-0001. `arris-math`: the point,
  vector and unit aliases with `nalgebra` re-exported; `Frame::new(origin,
  z, x_hint) -> Result<Frame, FrameError>` (Gram–Schmidt, degenerate hint
  is the error), `Frame::from_z` picking `x` by the same rule as
  `gp_Ax3(P, N)` in the reference tree so an axis-built cylinder seams
  where Open CASCADE's does, `to_local`/`to_world` for points and
  vectors, `Frame2` with either handedness and `is_right_handed`;
  `Isometry` from a rotation and a translation, `apply` to points,
  vectors, frames, composition and inverse; `Interval` (`contains`,
  `clamp`, `length`, `midpoint`, `lerp`, `overlaps`); `Tolerance` and
  `Precision::tolerance()`; `predicates::{orient2d, incircle}` returning
  `Ordering`-like signs. `arris-debug::prop`: `unit_vec3`/`rotation` return
  math types; `frame()`, `pose()`, `radius(range)`, `point_in_box(scale)`.
  CI's test job gets `ARRIS_PROPTEST_CASES=1000`. Tests (integration):
  a frame from any random `z` and hint is orthonormal to 1e-15 and
  right-handed; `to_world ∘ to_local` is the identity to 1e-12·scale
  under random poses; `Isometry` composition equals sequential
  application; `orient2d` and `incircle` on integer-grid points agree
  with the exact `i128` determinant at every case, including collinear
  and cocircular ones; `Interval` clamps and overlaps. ADR-0001 written.
- [x] Step 2 — Analytic surfaces and curves evaluate. `Surface::{Plane,
  Cylinder, Cone, Sphere, Torus}` and `Curve::{Line, Circle, Ellipse}`
  with the parametrisations of 02 §Geometry (read `Geom_CylindricalSurface`,
  `Geom_ConicalSurface`, `Geom_SphericalSurface`, `Geom_ToroidalSurface`,
  `Geom_Ellipse` in the reference tree for the seam and apex conventions,
  reimplemented); `eval(u, v) -> SurfaceEval`, `normal(u, v) ->
  Option<UnitVec3>` (`None` at a singularity), `eval(t) -> CurveEval`,
  `domain`, `period`, `kind`; `SurfaceKind`, `CurveKind`. `arris-debug`:
  `polyline_of`, `wireframe_of`, and the `prop::geom` strategies for every
  analytic variant in a random pose. Tests: evaluation in a posed frame
  equals the local closed form moved by the pose to 1e-12·scale; `du`,
  `dv`, `d1` and the second derivatives match central differences to
  1e-6·scale; the normal is unit and orthogonal to both derivatives;
  periodic variants repeat after one period exactly in the sense of
  1e-12·scale; the singular loci (apex, poles) return `None`; a PNG of a
  cylinder wireframe with a circle and an ellipse on it, rendered and read
  by the agent, recorded in the commit body.
- [ ] Step 3 — Projection onto every analytic variant. `Surface::project
  (Point3) -> Result<SurfaceProjection, GeomError>` for the five analytic
  surfaces (closed forms; a point on the cylinder's or cone's axis, the
  sphere's centre, the torus's axis or its centre circle is `Ambiguous`,
  never a silently chosen `u`); `Curve::project` for `Line` and `Circle`
  (`Ellipse` waits for step 5's roots); the cylinder's `u` lands in
  `[0, 2π)` and the sphere's `v` in `[−π/2, π/2]`. Tests: projection is
  idempotent and its point lies on the surface (implicit-form distance) to
  1e-12·scale; a point built at `(u, v)` and displaced along the normal
  projects back to `(u, v)` modulo the period to 1e-12; `distance` equals
  the closed-form distance; the `Ambiguous` loci are reported, not
  guessed — all at 1000 random poses per variant.
- [ ] Step 4 — Plane–cylinder and plane–plane. `intersect_surfaces(a, b,
  tol)`: plane–plane → `Empty` (parallel, distinct beyond `tol.linear`),
  `Coincident`, or one `Transversal` line; plane–cylinder by the case
  table — normal parallel to the axis within `tol.angular` → circle;
  oblique → ellipse with `b = R`, `a = R / |n · Z|`, centre at the axis
  piercing point, `X` the major axis; normal perpendicular to the axis →
  axis-to-plane distance `d` against `R` within `tol.linear`: two
  `Transversal` lines, one `Tangent` line, or `Empty`. Every other
  surface pair is an explicit `GeomError::Unsupported` arm (no wildcard;
  read `IntAna_QuadQuadGeo` in the reference tree for the case analysis,
  reimplemented on our frames). Results are symmetric under argument
  swap up to curve orientation, and bit-identical across two runs. Tests
  (1000 cases): a strategy that picks the case first, then a random pose
  and a plane that realises it — the returned variant is the case's;
  sampled points of every result curve lie on both surfaces to
  1e-12·scale; the ellipse's axes match the closed form; the tangent line
  is the cylinder's ruling at the nearest point; swapping the arguments
  gives the same curves. Hand-picked cases beside the property: the
  box-minus-cylinder faces of `boolean/through-hole` (four parallel
  planes clear of the hole, two perpendicular ones giving the circles).
- [ ] Step 5 — Roots, guarded Newton, ellipse projection. `roots::
  quadratic`, `cubic`, `quartic` returning the real roots sorted with
  multiplicity, and `newton_in_interval(f, df, Interval, tol)` that never
  leaves its bracket (bisection when a Newton step would); the method for
  degree 3 and 4 is `/work`'s choice (closed form with Newton polish, or a
  companion matrix) and the test is the same either way. `Curve::project`
  for `Ellipse` through the quartic, `Ambiguous` at the centre. Tests
  (1000 cases): polynomials built from random real roots separated by at
  least 1e-3·scale are recovered to 1e-12·scale; polynomials built from
  complex pairs report no real root; a double root is found once with
  multiplicity two; Newton on a random cubic with a sign-changing bracket
  converges inside it; ellipse projection is idempotent and the residual
  `(p − C(t)) · C′(t)` is zero to 1e-12·scale².
- [ ] Step 6 — Curve–surface intersections. `intersect_curve_surface(c, s,
  tol)`: line–plane (one hit, `Coincident`, or none), line–cylinder
  (quadratic: two hits, one `tangent` hit within `tol.linear`, none, or
  `Coincident` for a ruling), circle–plane (0, 1 tangent, 2, or
  `Coincident`), circle–cylinder (the quartic of step 5 in the half-angle
  substitution: up to four hits, `Coincident` for a circle around the
  axis at radius `R`); every other (curve, surface) pair an explicit
  `Unsupported` arm. Hits sorted by `t`, `uv` from step 3's projection,
  and `t` inside the curve's domain. Tests (1000 cases): hit count by
  constructed case; every hit lies on both operands to 1e-12·scale;
  `tangent` when the case was built tangent; `Coincident` when the curve
  was built on the surface; two runs agree bit for bit.
- [ ] Step 7 — The geometry oracle. `tools/oracle`: `expected.py` accepts
  `"kind": "geometry"` recipes — named surfaces and curves with frames and
  radii, `samples` of parameters and of points to project, and `pairs` to
  intersect — and writes OCCT's `Geom_*::D2` evaluations, `GeomAPI_
  ProjectPointOnSurf`/`OnCurve` results with parameters, and the type and
  sampled points of `IntAna_QuadQuadGeo` (surface pairs) and
  `IntAna_IntConicQuad` (curve–surface) results; `selftest.py` covers the
  new kind. Two fixtures: `geom/analytic-eval` (every analytic variant in
  three poses each, parameters on and off the seam, projections from both
  sides) and `geom/c1-intersections` (every case of steps 4 and 6 in a
  random but committed pose). `arris_debug::fixtures` loads the kind, the
  corpus lint checks its hash; `crates/arris-geom/tests/oracle.rs`
  compares Arris's evaluations and projected `(u, v)` against the oracle
  to 1e-9 relative and the intersection types exactly, with the oracle's
  sampled points on Arris's curves to 1e-9. `tools/oracle/README.md` and
  `tests/fixtures/README.md` describe the kind. A convention mismatch here
  is fixed in Arris in this step: the oracle is the parametrisation's
  ground truth (02 §Conventions).
- [ ] Step 8 — NURBS types and evaluation. `NurbsCurve`, `NurbsCurve2`,
  `NurbsSurface` per 02 §NURBS (degree, knots, control points, positive
  weights; validating constructors returning `GeomError::Degenerate`
  for a bad knot vector); de Boor evaluation with derivatives to order 2,
  `domain`, `period` for unclamped periodic knots, knot insertion for
  curves; `Surface::Nurbs` and `Curve::Nurbs` added, and every dispatch
  of steps 2–7 gains its arm: evaluation, `project` for NURBS curves by
  coarse sampling then `newton_in_interval` on the squared distance
  (documented as nearest local minimum from the best sample),
  `Ambiguous`-free, `Unsupported` for projection onto a NURBS surface and
  for every intersection pair involving one. Read `truck-geometry`'s
  B-spline module and *The NURBS Book* ch. 2–5 for what a correct
  implementation looks like; nothing copied. Tests: a full circle as a
  rational quadratic NURBS evaluates to the analytic circle to
  1e-12·scale at 1000 parameters; knot insertion leaves the curve
  unchanged at 1000 parameters; derivatives match central differences;
  a bilinear patch evaluates as its plane; a periodic knot vector wraps;
  projection onto the NURBS circle agrees with the analytic circle's.
- [ ] Step 9 — `Curve2` and NURBS fitting. `Curve2::{Line, Circle,
  Ellipse, Nurbs}` with the `Frame2` delta (decided, §Open questions),
  `eval(t) -> Curve2Eval`, `project(Point2)`, `domain`, `period`, `kind`;
  `fit_curve2(f: impl Fn(f64) -> Point2, range, degree, deviation: impl
  Fn(f64, Point2) -> f64, tol) -> Result<NurbsCurve2, FitError>`: global
  least-squares approximation at the *given* parameter (*The NURBS Book*
  9.4.1) with knots refined where the caller's deviation exceeds `tol`,
  bounded by a named maximum knot count (`FitError::Diverged` beyond it,
  never a loop); the parametrisation is the caller's, so the result is
  same-parameter by construction. Tests (1000 cases): fitting a random
  sinusoid `v = A + B cos t + C sin t, u = t` — the ellipse-on-cylinder
  form — meets `tol` at 1000 samples including both ends with a knot
  count under the bound; a tolerance below the sampling noise returns
  `Diverged`; `Curve2` circles of both handedness evaluate and project
  correctly.
- [ ] Step 10 — Pcurves and `project_to_plane`. `pcurve_on(curve, range,
  surface, tol) -> Result<Curve2, GeomError>`, exhaustive over (curve,
  surface): on a plane every variant is exact (line, circle with the
  handedness of `Z` against the plane's normal, ellipse, NURBS by
  projecting control points); on a cylinder a ruling line is a `Line` at
  constant `u`, a circle around the axis a `Line` at constant `v` with
  the parameter offset of the circle's `X` against the cylinder's, an
  ellipse or a NURBS is `fit_curve2` over the step 3 projection with `u`
  unwrapped along `t` so a seam crossing stays continuous and may leave
  `[0, 2π)` (02 §Pcurves); a curve not on the surface within `tol` is
  `NotOnSurface`; cone, sphere, torus and NURBS surfaces are `Unsupported`
  arms. `project_to_plane(curve, plane)` is the plane arm without the
  on-surface check (01 §Facade). Tests (1000 cases): for every analytic
  curve on a plane and on a cylinder in random poses, `S(pcurve(t))`
  matches `C(t)` at 1000 parameters including both ends — to 1e-12·scale
  on the exact arms, to `tol` on the fitted one (invariant E4 before the
  checker exists); a fitted pcurve across the seam is continuous; a
  clockwise circle on a plane round-trips through `Frame2`; a circle
  projected to an oblique plane is the expected ellipse.

## Acceptance

`ARRIS_PROPTEST_CASES=1000 cargo test --workspace` green: every property
of steps 1–10 at 1000 cases (projection idempotent and on-surface to
1e-12·scale; every intersection point on both operands to 1e-12·scale;
pcurve image matching the 3D curve to the fitting tolerance; the
plane–cylinder case table agreeing with the closed forms in random
poses), the two `geom/*` fixtures matching the oracle, the M0 corpus lint
still green with the new kind; `uv run --project tools/oracle
tools/oracle/selftest.py` green including `geom/`; the layer check and the
wasm build pass; CI green on `main`. Then tag `m1` (the human's).

## Docs to update on completion

- `docs/03-roadmap.md` §M1 — status line: date, what was retired (the
  plane–cylinder table and the parametrisation agreement with OCCT),
  ADR-0001, and the `Curve2` delta.
- `docs/02-data-model.md` §Conventions — `Tolerance`, `Isometry`,
  `Frame2`; §Curves — intersection result types and the curve-frame
  rule; §Pcurves — the `Curve2` variants as decided in step 9; §NURBS —
  "knot insertion" only, degree elevation to the backlog.
- `docs/01-architecture.md` §Crates — `arris-geom` external deps
  (`thiserror`); §Geometry dispatch — the result enums; §Formats and
  tools — the geometry fixture kind and the integration-test placement
  rule for crates below `arris-debug`.
- `docs/adr/README.md` — ADR-0001 in the table.
- `tests/fixtures/README.md`, `tools/oracle/README.md` — the `geometry`
  kind (written in step 7; confirmed here).
- `.agents/skills/inspect/SKILL.md` — the PNG row mentions `polyline_of`
  and `wireframe_of` for curves and surfaces without a body.
- `AGENTS.md` current state — "M1 done <date>; next `/plan m2-topology`".
- `docs/BACKLOG.md` — the degree-elevation line stays; nothing else.

## Open questions

All four decided by the human on 2026-09-05 in favour of the agent's
recommendation; kept here so `/work` does not re-ask.

- **Decided:** `nalgebra` types by alias with the crate re-exported
  (ADR-0001, step 1). A `nalgebra` major bump is an Arris API change,
  named in the ADR's consequences.
- **Decided:** the geometry oracle is in (step 7); `expected.py` learns
  the `geometry` recipe kind and the corpus lint tolerates it.
- **Decided:** `Curve2::Circle` and `Curve2::Ellipse` carry a `Frame2`
  whose handedness is the direction of traversal (step 9); 02-data-model
  §Pcurves changes in that commit.
- **Decided:** geometry property tests are integration tests under
  `crates/<crate>/tests/`; the `proptest`-feature fallback is taken only
  if step 2 finds the cycle unworkable, recorded under "Findings".
