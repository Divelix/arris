# Plan: cylinder-cylinder-booleans

- Started: 2026-09-14
- Milestone: C2, the application gate (docs/ROADMAP.md §C2, the
  cylinder–cylinder line)
- Idea (verbatim from the human): "/plan cylinder-cylinder-booleans" —
  the idea's four decisions as recommended, under the human's "decide
  yourself" of 2026-09-13 recorded in `plans/fillet-and-chamfer` §Open
  questions, which assumes option B
- Idea: docs/ideas/cylinder-cylinder-booleans.md (absorbed)

## Goal

Two cylinders meet by closed form in every pose whose curve is a line or
a conic. Parallel axes give two `Transversal` rulings, one `Tangent`
ruling (externally at `d = R₁ + R₂`, internally at `d = |R₁ − R₂|`), or
`Empty`. Equal radii with crossing axes give two `Transversal` ellipses
in the planes that bisect the axes. A skew pair whose axes are further
apart than `R₁ + R₂` is `Empty`. Every other pose stays `Unsupported`:
crossing axes of unequal radii, and a skew pair within `R₁ + R₂`, are
quartics and C3's. `fuse`, `common` and `cut` build over those arms:
- the consumer's transversal probe passes in its own units (2.2079e-5);
- two equal cylinders crossing at any angle pass, the two ellipses
  meeting at a section vertex no operand edge made;
- a tangent contact between two cylinders is decided by one curvature
  rule, which replaces the plane–cylinder match.
S5 and B1 check every cylinder pair an extrude or a fillet miter makes.
`regression/fillet-miter` passes every corpus stage as
`blend/fillet-miter`, and the extrude and fillet tests allow no unchecked
row.

## Non-goals

- The quartic: crossing axes of unequal radii, skew axes within
  `R₁ + R₂`, the point touch of two skew cylinders at `d = R₁ + R₂`.
  Each stays `GeomError::Unsupported`. The quadric-curve `⚠ OPEN` stays
  open, as C3's (DATA-MODEL §Curves already says so).
- Cone, sphere and torus pairs (the quadric checker arms' plan). The
  curvature rule is written for every surface, but it is only exercised
  on planes and cylinders here.
- `plans/fillet-and-chamfer` step 7's ruling arm (the D and C chord
  edges). This plan makes their S5 row checkable and builds no blend.
- The `Operand` / `Selection` enums of the boolean (backlog). No step
  here touches `Op::select`.
- An ADR. Both arms are closed forms inside ADR-0004, and the curvature
  rule is a §Operations change (the idea's decision 4).

## Design deltas

- **`arris-geom` `intersect_surfaces`**, cylinder–cylinder arm. The
  signature is unchanged; the table grows.
  - Parallel axes within `tol.angular`, `d` the distance between the
    axes:
    - `d ≤ tol.linear`: coaxial, as today.
    - `|d − (R₁ + R₂)| ≤ tol.linear` or `|d − |R₁ − R₂|| ≤ tol.linear`:
      `Tangent([ruling])`.
    - Strictly between the two: `Transversal([two rulings])`.
    - Otherwise: `Empty`.
    Each ruling runs along the first cylinder's `Z`, from its point
    nearest the first cylinder's origin. The two are ordered by the sign
    of their offset across the line of centres (the agent fixes the sign
    and writes it in DATA-MODEL).
  - Coplanar crossing axes (their nearest approach within `tol.linear`,
    not parallel), radii equal within `tol.linear`: `Transversal`, two
    ellipses centred at the crossing point. With `b` flipped so that
    `a · b ≥ 0` and `ψ` the angle between the axes:
    - one ellipse has `Z` along `a − b`, `X` along `a + b` and major
      radius `R / sin(ψ/2)`;
    - the other has `Z` along `a + b`, `X` along `a − b` and major radius
      `R / cos(ψ/2)`;
    - both have minor radius `R`, along `a × b`.
    Swapping the operands gives the same point sets. The two ellipses
    cross at `±R·(a × b)/|a × b|`, off the plane of the axes. The idea's
    "on the plane of the axes" is wrong.
  - Crossing axes of unequal radii: `Unsupported`, as today.
  - Skew axes (nearest approach over `tol.linear`): `Empty` when
    `d > R₁ + R₂ + tol.linear` (triangle inequality), `Unsupported`
    otherwise.
- **`arris-geom` public API (addition):**
  `Surface::normal_curvature(&self, u: f64, v: f64, direction: Vec3) -> Option<f64>`.
  - It is the second fundamental form over the first, taken along the
    tangent `direction`, from `Surface::eval`'s second derivatives.
  - The sign is with respect to `Surface::normal`: positive where the
    surface bends toward it.
  - It returns `None` where `normal` is `None` or `direction` is not
    tangent within `tol.angular`.
- **The curvature rule** in `boolean/result.rs`: `tangent_side` stops
  matching on plane–cylinder. At a contact point it takes:
  - `n`, the other face's effective outward normal;
  - `w`, the direction across the contact curve in the shared tangent
    plane;
  - `κ_f` and `κ_g`, each face's normal curvature along `w`, signed
    against `n`.
  A face's piece lies inside the other body exactly when `κ_f < κ_g`.
  Equal curvatures, compared exactly with no literal, are a higher-order
  touch: the rule returns `None`, which is today's
  `OpError::Unsupported` naming the pair. For a plane and a cylinder it reduces to the rule
  ARCHITECTURE states today, and the step that lands it proves this on
  the existing corpus.
- **The pave model: section crossings.** Two section curves of one
  `Transversal` pair that cross (`intersect_curves`) at a point inside
  both faces become one section vertex. It is merged with any hit or
  crossing already there, by `merge_point`, and paves both curves. A
  touch of an operand edge on the other face that lands on such a vertex
  paves that edge (the tee, step 3). `Interferences` records where each
  vertex came from. The new source is public through the boolean's
  interference types and is named in its commit.
- **The oracle:** `tools/oracle/oracle/geometry.py` maps
  `IntAna_NoGeometricSolution` to `"type": "unsolved"`. `oracle.rs`
  holds Arris to `Unsupported`, or to its closed-form `Empty`, against
  it. What Open CASCADE answers for a skew pair is recorded as it is,
  under tests/fixtures/README §Geometry fixtures' "cannot answer stably"
  paragraph.
- **No new `Reason`, no new `Role`.** A tangent fuse or cut between two
  cylinders is the existing `Reason::TangentContact`; a common that
  selects nothing is `Reason::Empty`.
- **`plans/fillet-and-chamfer` step 7:** its second bullet (moving
  `regression/fillet-miter` into `blend/`) is done here, because the
  commit that fixes a fixture moves it (kernel rules). Step 1's arm
  turned out to be that fix — the miter's two cylinders cross at equal
  radii — so step 1 moves it and strikes that plan's bullet, not step 8.

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness or
bound has to be established here.

- [x] Step 1 **[2]** — **The intersector arms.**
  - `cylinder_cylinder` as §Design deltas: parallel, equal-radius
    crossing, skew and apart. `Unsupported` for the rest, written out.
  - Geometry fixture `geom/c2-cylinder-pairs`, written by
    `generate.py` in the `tilt` pose: two rulings, an external and an
    internal tangent, apart, nested, crossing at 90° and at 50°, crossing
    with unequal radii, skew apart, skew close. Include the `unsolved`
    oracle type.
  - `intersect_surfaces.rs`: properties for random parallel pairs (the
    case by `d` against the radii, each curve on both surfaces, swap
    symmetry), random equal-radius crossing pairs at `ψ ∈ [10°, 90°]`
    (both ellipses on both cylinders, crossing at `±R·(a × b)`), and skew
    pairs apart. `cylinders_that_are_not_coaxial_are_unsupported_naming_the_pair`
    narrows to the quartic poses.
  - DATA-MODEL §Curves: the table and the ellipse frames. ARCHITECTURE
    §Geometry dispatch: the cylinder pair's partial support.
  - *Found at the step:* S5 now decides the miter's two blend cylinders,
    so the tests that pinned exactly one unchecked row per miter
    (`fillet.rs`'s two miter tests, `blend_prop.rs` at rest) now pin none,
    and `regression/fillet-miter`, failing only on S5, passes every stage:
    it moves to `blend/fillet-miter` with its dump here (§Design deltas).
    Step 8 keeps the rest. Open CASCADE reports the inside touch of
    `c2-cylinder-pairs` as two rulings 1e-7 apart; `oracle.rs` holds a
    touch's rulings to 1e-6, as for a curve's touch.
- [x] Step 2 **[3]** — **Two crossing cylinders: section curves that meet
  each other.**
  - The pave model's section crossings (§Design deltas). The (u, v)
    arrangement at the kink where two half-ellipses meet is ordered by
    the tangent angle as today. A tie is `TangentContact` as today and is
    not expected.
  - Fixtures, `R = 1`, both cylinders long enough to pass through each
    other:
    - `boolean/cross-cylinders-common`, the Steinmetz solid, `16R³/3`,
      area `16R²`;
    - `cross-cylinders-fuse`, `2πR²L − 16R³/3`;
    - `cross-cylinders-cut`, `πR²L − 16R³/3`: two lumps, since the tool
      is as wide as the target;
    - `oblique-cross-common` at `ψ = 60°`, `16R³/(3 sin ψ)`.
  - **Gate:** if two crossing ellipses need anything beyond one merged
    vertex paving both, such as a pcurve fit that misses the corpus at
    the kink or an arrangement tie, stop and return to the human before
    step 3.
  - *Found at the step:* the gate holds — one vertex per crossing, made
    by `intersect_curves` on the pair's two ellipses and merged by
    `merge_point`, paves both, and the four half-edges at the kink are
    a quarter turn apart. Three findings changed the fixtures:
    - The cut is `Reason::NonManifold` in every pose, not two lumps: the
      walls are tangent to each other at the crossing vertices, so the
      closures of the two lumps touch there, which ADR-0006 refuses.
      `cross-cylinders-cut` is `expect_error: "non-manifold"` with the
      oracle's compound recorded, as `edge-touching-fuse`.
    - Open CASCADE cuts the section arc that runs through an ellipse's
      parameter origin at that origin, one more vertex and edge than the
      paves make; Arris's edge crosses the period once (E1). The three
      built fixtures state it in `counts_differ`.
    - A cylinder's default seam (`Frame::from_z`) lands on the other
      seam's hit or on a crossing vertex, so every fixture turns the tool
      about its own axis (`turn`) for a generic seam. The variant with the
      seam through a crossing vertex is not a merge of a hit and a
      crossing: a ruling through a crossing vertex is tangent to the
      other wall there (the vertex's normal is the axes' common
      perpendicular, which both axes are perpendicular to), so the seam's
      hit is a *touch*, and a touch paves nothing today. That is exactly
      step 3's rule, so the variant moves to step 3.
- [ ] Step 3 **[2]** — **The tee, and a touch on a section vertex.** A
  touch of an operand edge on the other face that lands on a section
  vertex paves that edge there (§Design deltas). Two fixtures:
  - `cross-cylinders-common`'s variant `seam-through-crossing`, the tool
    turned so its seam runs through `(0, R, 0)`: the seam's touch on the
    target's wall lands on the crossing vertex and cuts the seam there,
    and the lens on the tool's wall is bounded by the seam pieces at the
    vertex (step 2's finding).
  - The tee: a branch of equal radius ending on the main cylinder's
    axis. Its rim circle touches the main wall exactly at the two
    crossing vertices, the same rule. Fixture `boolean/tee-fuse`,
    `πR²L + πR²H − 8R³/3`. If more than that paving is needed, the fixture
    waits under `regression/tee-fuse` with the cause named, and a backlog
    line records it for C3's tangent corpus.
- [ ] Step 4 **[2]** — **Parallel cylinders, and the consumer's probe.**
  - Fixtures:
    - `boolean/parallel-cylinders-cut`: the probe in its own units,
      `r = 0.03`, axes 0.04 apart, a target 0.01 tall and a tool
      clearing both caps, `(πr² − lens)·h = 2.2079e-5`;
    - `parallel-cylinders-common`: the lens prism;
    - `parallel-cylinders-fuse`, with a variant whose caps are flush
      (rim circles crossing on a coincident plane pair at the ruling's
      end);
    - `parallel-cylinders-seam`: a ruling on one operand's seam edge,
      the block that is an operand edge.
  - Every section curve here is a ruling, and no step-2 machinery is
    needed. A finding otherwise is recorded in this plan.
- [ ] Step 5 **[2]** — **The curvature rule, generalised.**
  - `Surface::normal_curvature` with rustdoc and an example. Tests
    against the closed forms on every analytic kind (plane 0, cylinder
    `±1/R` across its rulings and 0 along them, sphere `1/R`, torus and
    cone at a few points) and on the hand-built NURBS saddle.
  - `tangent_side` over it (§Design deltas). `boolean/tangent-hole`,
    `tangent-outside-cut` and `boolean_prop.rs`'s tangent pair stay
    green unchanged: the reduction holds. ARCHITECTURE §Operations: the
    curvature rule's paragraph.
- [ ] Step 6 **[2]** — **Tangent cylinders.**
  - External, two `R = 1` cylinders with axes 2 apart:
    `boolean/tangent-cylinders-fuse` is `expect_error:
    "tangent-contact"`; `tangent-cylinders-cut` is the target, with the
    ruling no edge (the `tangent-outside-cut` convention).
  - Internal, a pin `r = 1` inside a bore `R = 2` with axes 1 apart,
    both clearing the caps: `pin-in-bore-fuse` is the outer cylinder, and
    `pin-in-bore-cut` is `"tangent-contact"`. Open CASCADE builds both
    refusals, so each gets `expect_error`.
- [ ] Step 7 **[2]** — **Property tests** (seeded, `prop_shards!`).
  - `arris_debug::prop::body::parallel_pair` and `crossing_pair`, both
    under one random motion:
    - parallel: `d` drawn between the tangent distances, clear of each
      by a margin; a quarter of cases put a seam on a ruling and a
      quarter make the caps flush;
    - crossing: equal radii, `ψ ∈ [30°, 90°]`, each operand through the
      other, a quarter with a seam through a crossing vertex.
  - Held to `boolean_prop.rs`'s identities: additivity, the cut identity
    whatever the lumps, commutativity, `Full` with nothing unchecked,
    `audit`, and two runs dumping identically. The crossing common is
    also held to `16R³/(3 sin ψ)`.
  - A failure is shrunk to a `regression/` fixture in this step.
- [ ] Step 8 **[2]** — **The S5 rows gone.** (The miter's move into
  `blend/`, `fillet.rs`'s two miter tests and "nothing unchecked at rest"
  landed with step 1.)
  - `extrude.rs` drops `non_coaxial_cylinders` and allows nothing
    unchecked.
  - `blend_prop.rs`'s posed allowance narrows: only S5 between two blend
    cylinders whose axes are skew within `2r` (the closed form from the
    case), and never between parallel or crossing ones.
  - DATA-MODEL §Curves' "Until then S5 reports…" sentence narrows to the
    quartic poses.

## Acceptance

- `cargo nextest run -p arris --test corpus --run-ignored all`: every new
  `boolean/` fixture (cross-cylinders ×3, `oblique-cross-common`,
  `tee-fuse`, parallel-cylinders ×4, tangent-cylinders ×2, pin-in-bore
  ×2) and `blend/fillet-miter` passing every stage against Open CASCADE.
  `regression/` holds no fillet-miter.
- `cargo nextest run -p arris-geom --test oracle`: `geom/c2-cylinder-pairs`
  matched.
- The consumer's number in its own units: cylinder − cylinder transversal
  2.2079e-5.
- Step 7's properties green at the configured case count. The extrude,
  fillet and blend property tests green with the narrowed allowances of
  step 8.

## Docs to update on completion

- `docs/DATA-MODEL.md` §Curves: the cylinder–cylinder table, ellipse
  frames and the S5 sentence (steps 1, 8 write them; the retirement
  checks them).
- `docs/ARCHITECTURE.md` §Operations: section crossings in the pave model
  and the curvature rule; §Geometry dispatch: the partial cylinder pair.
- `docs/ROADMAP.md` §C2: the cylinder–cylinder line **Done** with its
  fixtures; the blend line's "waiting on the cylinder–cylinder line"
  clauses (the miter, the posed S5 rows) rewritten to what remains, the
  skew pairs.
- `tests/fixtures/README.md` §Geometry fixtures: `c2-cylinder-pairs`, the
  `unsolved` type.
- `tools/oracle/README.md`: the `unsolved` type.
- `docs/BACKLOG.md`: the tee's line if step 3 leaves it in `regression/`.
- `AGENTS.md` current state: cylinder–cylinder booleans landed.

## Open questions

None for the human; the idea's four decisions are taken as recommended
(header). Two are the agent's, each at its step:

- Whether a tie of normal curvatures is ever reached by a cylinder pair
  (step 5). By the table it is not: equal curvatures at a tangent ruling
  means equal radii internally tangent, which is coaxial and
  `Coincident`. A test states it.
- Whether the tee is one paving rule or a C3 case (step 3), decided by
  the fixture with the fallback written there.
