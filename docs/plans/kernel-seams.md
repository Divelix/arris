# Plan: kernel-seams

- Started: 2026-09-13
- Milestone: C2, the application gate (docs/ROADMAP.md §C2). This plan
  adds no roadmap line: it prepares the seams the fillet line builds on.
- Idea (verbatim from the human): "I think we should do refactoring and
  only then start fillet-and-chamfer implementation" — "yes, do it"
- Idea: none. It comes from a whole-workspace review at `a4049d7`, and
  the findings it acts on are restated here. `docs/ideas/fillet-and-chamfer.md`
  is parked until this plan retires.

## Goal

The code a blend will be built on is written once, and the contracts it
relies on are enforced by tests rather than left in comments:

- **One face domain.** The checker, the classifier, the boolean and
  tessellation all ask "where is this (u, v) point on a face" through one
  `FaceDomain`. That fixes the checker's copy, which never tries a
  period shift, so S5 and B1 can miss an overlap on a periodic face
  whose loops sit in another translate.
- **One enclosed-volume integral** and **one Euler line**, each used by
  every caller.
- **Provenance through slots, not zips.** An operation that assembles a
  body matches output ids to its specs through the slots `assemble`
  hands back. No caller zips `BTreeMap` iteration order against its own
  lists any more.
- **The assembly and provenance machinery is shared.** The effective
  loop walk, the body → `Assembly` description, and the boolean's
  keep/regenerate assembly with its provenance writer each live in one
  place a blend can call.
- **Rule fixes.** The rule breaks the review found are fixed, each with a
  test: a wildcard arm, an unbounded grid, errors swallowed or naming the
  wrong entity, model-default tolerances where the entity's belong, the
  Rust/Python expression grammar, serde features that cannot be turned
  off.
- **No plan references in code,** and a lint that keeps it so.

Every corpus fixture's blessed dump stays byte-identical unless a step
says `fixtures:` and why.

## Non-goals

- **The cylinder–cylinder prerequisites.** The trig root solver in
  `arris-math`, a conic view of `Curve`, the angle helpers, the
  intersection arm signatures and periodic-aware tangent contact all
  belong to `plans/cylinder-cylinder-booleans` when that is planned, as
  its first steps.
- **File splits for their own sake.** `topo/builder.rs`, `ops/sweep.rs`,
  `boolean/pave.rs`, `debug/corpus.rs`, `mesh/cdt.rs` and `check/check.rs`
  stay whole. `boolean/result.rs` shrinks only by what step 9 extracts.
- **Public-API redesigns that no blend needs:**
  - a `DegenerateReason` enum for `GeomError`
  - nesting `BuildError`
  - `Operand` and `Selection` enums in the boolean
  - narrowing `pub` internals
  - replacing re-export chains with direct dependencies

  Each is a backlog line.
- **Splitting `arris-debug` into crates.** That is an idea, not a step.
- **Test-suite reorganisation** beyond the helpers step 12 moves: the
  `fast_part1`/`fast_part2` split, `tests/boolean.rs` split, and the
  corpus test macro.
- **Sheet bodies in `finish` validation** (C7), and O(1) kill operators.
- **A chord sampled over the whole face** rather than at one point.
  Backlog. Blend surfaces are analytic, and a torus's speed varies by
  (R + r)/(R − r).

## Design deltas

- **`arris-check` gains `pub mod domain` (public addition).**
  - `FaceDomain::of(model, face, tolerance)` holds a face's loops as
    polygons, its (u, v) box padded by chord deviation, and its 3D box
    grown by the face's own tolerance. (Step 2 found both callers grew
    the box by the face's tolerance, the checker too; `tolerance` sets
    the chord and the side band only.)
  - Free functions beside it: `shifts`, `bands` (per direction, which
    E7 and the loop rows read) and `band`, `chord` (the arrangement's),
    and `boundary_entity` over an edge set, vertices before edges in id
    order, which the classifier asks of a whole body.
  - `side(uv) -> (Side, Vec2)` tries every period translate.
    `winds_around(uv)` and `boundary_entity(model, point)` resolve a
    point to a vertex, else an edge, trying periods on closed edges.
  - `band(surface, uv, tolerance) -> f64` converts a 3D length to a
    (u, v) step at `uv`.
  - The checker passes the model's parametric tolerance; the boolean
    passes the face's own. It replaces:
    - `Checker::face_side` and `faces_fine`, `Classifier::face_side`
      and its polygons, `check::uv_bounds`
    - `ops::boolean::faces::{band, shifts, FaceInfo}`'s domain part
    - mesh's padded (u, v) box
  - It lives in `check` because the classifier and B1, which ADR-0004
    holds to one code path with the boolean, are already there, and ops
    and mesh already depend on `check`. No layer edge changes.
  - It is public and documented, not `#[doc(hidden)]`: ops and mesh
    cross the crate boundary to reach it, and a consumer classifying
    (u, v) points on a face has the same need.
- **`arris_check::classify::Classifier` becomes public.**
  - `Classifier::of_body(model, body)` is built once and answers many
    points.
  - `classify_point` wraps it.
  - `Checker::shell_contains` builds one per shell once rather than once
    per call, and the boolean builds one per operand rather than one per
    piece.
- **`arris_check::flux` (public addition).** `face_flux(model, face,
  integrand)` is the one region integral over a face.
  - B2 (`face_volume`), `lumps::shell_volumes` and
    `ops::measure::integrate_faces` call it.
  - `face_area` stays in ops: it is a surface integral, not a flux.
- **Euler line in `arris-topo` (public addition).**
  - `arris_topo::euler::EulerLine` moves down from `arris-check`, and
    `arris_check::EulerLine` stays as a re-export. `EulerLine::of(model,
    closure)` counts with one rule for degenerate edges.
  - It is used by `Builder::counts`, `assembled_genus`,
    `check::topology::euler_line`, `arris_debug::dump::euler_line` and
    `fixtures::lint`.
  - `Counts` keeps its fields and delegates the arithmetic.
  - The rule is the checker's, as DATA-MODEL §Invariants already states
    and the oracle counts: every degenerate edge is left out.
  - `finish` and `assemble` refuse a degenerate edge not used exactly
    once, as `BuildError::EdgeUses` with its message naming the
    degenerate case. No surface closes on one singular point twice, and
    the builder accepts a degenerate edge used twice today only because
    it passes the two-uses test.
- **`Builder::assemble` returns its slots (signature change).**
  - It becomes `Builder::assemble(model, tolerance, assembly) ->
    Result<(Builder, AssemblySlots), BuildError>`, with `AssemblySlots {
    vertices: Vec<VertexRef>, edges: Vec<EdgeRef>, faces:
    Vec<Vec<FaceRef>> }` one per spec in spec order.
  - Callers look outputs up as `built.vertices[&slot]` through `get`.
  - This is the internal `Assembled::vertex_of`/`edge_of` table, made
    public with a face table added.
  - Named in the commit. DATA-MODEL §Euler operators.
- **The effective walk in `arris-topo` (public addition).**
  - `builder::effective_uses(face_orientation, &Loop) -> Vec<(EdgeId,
    Orientation, Curve2Id)>` and `FaceSpec::from_face(model, face_use,
    edge_key)`.
  - They replace the five hand copies: `finish`, `keep_face`,
    `transform`, the boolean's `assembly`, and `tests/assemble.rs`.
- **`Assembly::of_body` (public addition).**
  - `Assembly::of_body(model, body, remap: &mut impl GeometryRemap) ->
    Result<(Assembly, BodyIndex), NotFound>` describes a body shell by
    shell with every entity `New`.
  - `remap` maps curve and surface ids; the identity keeps them.
  - `BodyIndex` maps each vertex, edge, face and shell to its spec
    index, which is what provenance is built from.
  - `transform` becomes a remap plus this. The fillet's `keep` variant is
    that plan's to add.
- **`arris-ops` gains a private `rebuild` module.**
  - It holds the keep/regenerate assembly (`Kept`, `Policy`, `Plan`) and
    the provenance writer, taken from `boolean/result.rs` without
    behaviour change.
  - The boolean becomes one producer of pieces; a blend will be another.
- **`OpError::NotFound` carries `AnyId` (signature change).**
  - `OpError::NotFound(AnyId)`, with `From<NotFound>`. Geometry ids are
    reported as themselves, not as the face that holds them.
  - `Fault` gains `Invariant { what: &'static str }`, `Unmade { segment:
    usize }`, `NoNormal { face: FaceId }` and `ProfileCurve(GeomError)`.
    They replace `not_found(0)` on internal lookups and the
    `GeomError::Degenerate` strings ops builds for its own faults.
  - Named in the commit. ARCHITECTURE §Errors.
- **`Violation::level` is exhaustive**, as is the CDT's `classify`
  (`Fail::Internal` on three zero signs).
- **`MeshError::GridTooLarge { face, points }`** (public addition): the
  interior lattice is capped by its total, not per direction.
  `MeshError::Internal` stops tessellation faults being blamed on the
  CDT. ARCHITECTURE §Tessellation.
- **`StepError::NotFound`** is returned where the writer now drops an
  edge use or a control point.
- **Features.** Internal workspace dependencies set `default-features =
  false`, with `serde` forwarded explicitly. CI builds `arris` with
  `--no-default-features`.
- **Fixture grammar.**
  - `^` is power on both sides and `**` is rejected on both.
  - An unknown `kind` is an error in Rust as it is in Python.
  - A profile plane whose `x` and `y` are not orthogonal is refused in
    Rust as it is in Python, by one named tolerance.
  - `tests/fixtures/expr-cases.json` holds cases both sides' tests
    evaluate.
  - `tests/fixtures/README.md` and `tools/oracle/README.md`.
- **Provenance audit (public addition).**
  - `arris_topo::provenance::audit(model, inputs, output, &Provenance)
    -> Result<(), AuditError>` is the accounting rule the corpus
    runner's private `account` holds today: every output entity kept or
    with an origin, every input kept or recorded, nothing both deleted
    and modified.
  - The runner and the ops property tests call it. DATA-MODEL
    §Provenance.
- **`arris_debug::testing` (dev-only addition).** It holds the helpers
  copied across test files: `close`, `fail`, `entities_of`,
  `recorded_parts`, the finite-difference derivatives, and the
  period-difference compare.
- **Plan-reference lint.** `crates/arris/tests/docs_refs.rs` fails when a
  file under `crates/`, `tools/`, `.githooks/` or `Cargo.toml` names
  `docs/plans/<slug>` or `plans/<slug>` for a plan that does not exist.
  ARCHITECTURE §Formats and tools.

No ADR. Each delta above removes a duplicate or states an existing
contract; none changes a representation decision. ADR-0004's
one-classifier claim becomes true of the checker too.

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness or
bound has to be established here.

- [x] Step 1 **[2]** — **The periodic blind spot in S5 and B1, reproduced
  and fixed.**
  - Build the smallest body where the checker's `face_side` answers
    `Outside` for a point inside a periodic face: a cylinder face whose
    loops are written in `[π, 3π)`, or a partial revolve's wall in
    `[−π/2, π/2]`, overlapping another face. Also a closed edge on
    `[π, 3π]` that `on_shared_boundary` reads as off the boundary.
  - Each goes in as a test in `check/tests/full.rs` asserting the row
    that should fire, or should not fire.
  - Fix in place: period shifts in `Checker::face_side`, and period
    shifts in place of the clamp in `on_shared_boundary`.
  - The C1 builders may always write a periodic face's loops in the
    first translate, so the blind spot may be latent. The fix lands
    either way, guarded by the raw-insert tests, and the commit says
    which it was.
  - The corpus runs green with dumps unchanged. A dump that changes is a
    `fixtures:` commit naming the fixture the checker had passed wrongly.
- [x] Step 2 **[2]** — **One `FaceDomain`.**
  - Add `arris_check::domain` per the delta. The checker's `Full` rows,
    the classifier, ops `FaceInfo` (which keeps its edge list and
    `EdgeInfo`) and mesh's padded box all move onto it.
  - Delete `uv_bounds`, `band`, both `shifts`, the four chord blocks,
    the three padded-box blocks, the two 3D face boxes and the three
    vertex-else-edge tests.
  - Property test (seeded, `prop_shards!`): for a random face on a
    periodic surface and a random interior point, `side` answers the
    same for the face and for the same face with its loops translated
    by a period.
  - Corpus dumps unchanged. Checker, classifier and boolean tests
    unchanged.
- [x] Step 3 **[2]** — **The entity's tolerance, not the model's, in
  `Full` rows and the classifier.**
  - `full.rs` (the on-surface distance in `regions_overlap` and
    `curve_is_interior_to_both`) and `classify.rs` (the ray's
    on-surface start and its intersection tolerance) take the face's
    tolerance, the larger of the two faces' for a pair.
  - Test: two faces at a tolerance ten times the default that meet
    within theirs. S5 and `classify_point` answer by the faces'
    tolerance where they answered by the default before.
  - Corpus dumps unchanged, since every C1 face is at the default.
  - Finding (step 3): the intersector in `faces_meet` takes the pair's
    tolerance as well, or two faces within theirs never reach the
    overlap test. The classifier's ray cast cannot change a
    `classify_point` answer on a body that passes E5 and V1–V3: a point
    its ray-start or grazing test now judges differently is already
    `On` an entity by that entity's tolerance, and only which
    directions are abandoned moves. The observable tests are S5 and B1
    by the faces' tolerance, with `classify_point` asserted to decide
    the same loose body.
- [x] Step 4 **[1]** — **`Classifier` built once; `face_flux`.**
  - `Classifier::of_body` becomes public. `shell_contains`/`nesting`
    reuse one classifier per shell, and the boolean's piece selection
    reuses one per operand.
  - `arris_check::flux::face_flux` replaces `face_volume` and
    `integrate_faces`'s loop.
  - Tests: `classify_point` and `Classifier::of_body(..).classify`
    agree on every point of the existing classify tests. The measure
    and B2 tests unchanged. `tools/test-timings.sh` before and after is
    in the commit body.
- [ ] Step 5 **[2]** — **One Euler line.**
  - `arris_topo::euler::EulerLine` per the delta, used by the five
    callers.
  - Every degenerate edge is left out of the count, and `finish` and
    `assemble` refuse one not used exactly once (the delta). DATA-MODEL
    §Invariants and §Euler operators say so in this commit.
  - Tests:
    - a builder with a degenerate edge used twice (`mef` over
      `Degenerate` geometry with `from == to`) is refused by `finish` as
      `EdgeUses`;
    - on the revolve sphere, cone and notch-to-axis bodies,
      `Builder::counts`, the checker's line and the dump print the same
      line.
  - Corpus dumps unchanged, since no C1 or C2 fixture has a degenerate
    edge used twice. If one does, stop: the rule is wrong for that body,
    and the step returns to the human.
- [ ] Step 6 **[1]** — **`assemble` hands back its slots.**
  - `AssemblySlots` per the delta.
  - `sweep::record`, `transform` and the boolean's output-id tables
    look ids up by slot; the three zips and their "one slot per spec in
    spec order" comments go.
  - Test in `topo/tests/assemble.rs`: an assembly interleaving `Keep`
    and `New` vertex, edge and face specs, where every `built.*[&slot]`
    is the entity its spec names. The same assembly through a zip would
    misalign, which the test shows by asserting the orders differ.
  - Provenance tests and corpus dumps unchanged.
- [ ] Step 7 **[1]** — **The effective walk, once.**
  - `effective_uses` and `FaceSpec::from_face` per the delta. `finish`,
    `keep_face`, `transform`, the boolean's `assembly` and
    `tests/assemble.rs` call them.
  - Test: for every face of every `arris_debug::sample` body under both
    orientations, `effective_uses` followed by `keep_face`'s inverse
    returns the stored loop.
- [ ] Step 8 **[1]** — **`Assembly::of_body`, and `transform` over it.**
  - Per the delta. `transform` becomes the curve and surface remap plus
    `of_body`, and its provenance comes from `BodyIndex` and step 6's
    slots.
  - `topo/tests/assemble.rs::describe(keep = false)` calls it.
  - Test: `of_body` with the identity remap, assembled and finished,
    dumps equal to the input up to ids, for every sample body and a
    multi-shell boolean result. The transform tests and property tests
    unchanged.
- [ ] Step 9 **[2]** — **`ops::rebuild`, out of the boolean.**
  - `Kept`, `Policy`, `Plan`, `assembly()` and the provenance writer
    move from `boolean/result.rs` to `src/rebuild.rs`, taking pieces and
    a per-operand policy. `boolean()` calls it.
  - `lump_order` keeps its scratch assembly into a cloned model.
    - This step moves code and does not change behaviour.
    - The clone shares the arena's chunks, so it costs one extra
      assemble and finish, and only for multi-shell results.
    - Re-ordering a finished body's shells would need a mutation the
      immutable entities do not offer.
    - Assembling once is a backlog line.
  - The `unwrap_or(0.0)` edge-tolerance fallback becomes a `Fault`.
  - Test: every `boolean/` and `provenance/` fixture dump and provenance
    record byte-identical; the boolean property tests green.
- [ ] Step 10 **[1]** — **Errors that name what failed.**
  - `OpError::NotFound(AnyId)` and the new `Fault` variants per the
    delta. The 36 `map_err(|_| …)` sites carry the id.
  - Silent fallbacks become faults: `Builder::position_after`'s
    `unwrap_or((0, 0))`, `primitive`'s role lookups (`map_or(0)`,
    `map_or(min)`, the edge given `BoxPart::Body`), and `pave`'s
    `period().unwrap_or(0.0)`.
  - Tests: a measure over a body whose surface id was compacted away
    names the surface; a boolean over a body that does not resolve still
    names the body.
- [ ] Step 11 **[1]** — **Rule fixes in check, mesh and io; features.**
  - `Violation::level` exhaustive; `cdt::classify`'s wildcard →
    `Fail::Internal`.
  - `interior_grid` capped by total points →
    `MeshError::GridTooLarge`; `MeshError::Internal`.
  - `step.rs`'s swallowed lookups → `StepError::NotFound`.
  - `default-features = false` on internal workspace dependencies, and
    a CI job `cargo check -p arris --no-default-features` that also
    asserts `cargo tree` has no `serde_json`.
  - Tests:
    - a torus face with a tiny minor radius at a fine chord returns
      `GridTooLarge`, not an allocation;
    - a model with a face removed under a written edge returns
      `NotFound` from the STEP writer;
    - the CI job.
- [ ] Step 12 **[1]** — **Fixture grammar parity, the provenance audit,
  shared test helpers.**
  - Python maps `BitXor` to power and rejects `Pow`.
  - Rust rejects an unknown `kind` and a non-orthogonal profile plane,
    by one named tolerance mirrored in `recipe.py`.
  - `tests/fixtures/expr-cases.json` is evaluated by a Rust test and by
    `selftest.py`.
  - `arris_topo::provenance::audit` replaces the corpus runner's
    `account`, and the ops provenance tests call it.
  - `arris_debug::testing` holds the copied helpers; ops, geom and mesh
    tests import them.
  - The whole corpus runs green with `recipe_sha256` unchanged, since no
    recipe text changes.
- [ ] Step 13 **[1]** — **No plan references in code, and the lint.**
  - Every `docs/plans/…` and `plans/…` citation in `crates/`, `tools/`,
    `Cargo.toml` and test headers is replaced by the ADR or design-doc
    section it stands for.
  - The "cycle 1" and "M0"/"M4" wording that is no longer true goes:
    `builder.rs`, `step.rs`, `classify.rs`, `intersect*.rs`, ops
    `error.rs`/`sweep.rs`/`result.rs`, debug `geom.rs`/`sample.rs`,
    `mesh/lib.rs` and the geom test names.
  - The dead `let _ = …` lines go.
  - Add `crates/arris/tests/docs_refs.rs`, which fails on a reference to
    a plan file that does not exist. It is proven by a scratch file with
    a bad reference, as `corpus_lint.rs` proves its own rules.

## Acceptance

- `cargo nextest run --workspace` is green, and so is the corpus run
  `cargo nextest run -p arris --test corpus --run-ignored all`.
- Every blessed dump under `tests/fixtures/{primitive,transform,boolean,
  sweep,provenance}/` is byte-identical to `a4049d7`, except fixtures a
  step named in a `fixtures:` commit.
- Every `expected.json` is unchanged, since oracle values do not drift.
- The new tests are green:
  - the periodic S5/B1 guards (step 1)
  - the `FaceDomain` translate property (step 2)
  - the loose-tolerance contact (step 3)
  - the degenerate-used-twice Euler line (step 5)
  - the interleaved-spec slots (step 6)
  - the effective-walk round trip (step 7)
  - the `of_body` round trip (step 8)
  - `GridTooLarge` and the STEP `NotFound` (step 11)
  - the expression cases on both sides (step 12)
  - the docs-refs lint (step 13)
- `cargo check -p arris --no-default-features` builds without serde.
- `tools/check-layers.sh` passes.
- There are no zips of `built.{vertices,edges,faces}.values()` in
  `crates/arris-ops/src` (`rg 'built\.(vertices|edges|faces)\.values\(\)'`
  finds nothing).
- There is one definition each of the period-shift side test, the (u, v)
  band, the face flux and the Euler line
  (`rg 'fn shifts|fn uv_bounds|fn band|fn face_volume|fn euler_line'`
  finds only `domain`, `flux` and `euler`).

## Docs to update on completion

- `docs/ARCHITECTURE.md`:
  - §The checker: `FaceDomain` and `Classifier` as the shared domain
    ADR-0004 relies on, the entity tolerances in S5/B1, `flux`.
  - §Operations: `rebuild` as the assembly seam the boolean uses,
    `transform` over `Assembly::of_body`.
  - §Errors: `OpError::NotFound(AnyId)`, the new `Fault` variants,
    `MeshError::GridTooLarge`/`Internal`.
  - §Tessellation: the lattice cap.
  - §Formats and tools: the docs-refs lint, `arris_debug::testing`, the
    expression cases.
- `docs/DATA-MODEL.md`:
  - §Euler operators: `AssemblySlots`, `effective_uses`,
    `FaceSpec::from_face`, `Assembly::of_body`.
  - §Invariants: the Euler line's degenerate-edge rule, and
    `EulerLine` in topo.
  - §Provenance: `audit`.
- `tests/fixtures/README.md`: the grammar (`^`, `kind`, the plane
  check) and `expr-cases.json`.
- `tools/oracle/README.md`: the grammar and the self-test's expression
  cases.
- `docs/BACKLOG.md`: the deferred review items listed under Non-goals
  are there (added with this plan); none is closed here.
- `docs/ideas/fillet-and-chamfer.md`: Status line says the plan can be
  written.
- `AGENTS.md` current state: the seams landed; next is `/plan
  fillet-and-chamfer`.

## Open questions

None. The four raised at planning were decided on 2026-09-13 by the
agent, at the human's request ("Decide yourself on open questions"),
and folded into the deltas and steps:

- **Euler line and degenerate edges:** every degenerate edge is left
  out, and `finish` refuses one not used exactly once (step 5).
- **`FaceDomain`:** public and documented (step 2).
- **`lump_order`:** keeps its scratch clone; assembling once is backlog
  (step 9).
- **Step 1 not reproducing:** the fix lands regardless, guarded by
  raw-insert tests.
