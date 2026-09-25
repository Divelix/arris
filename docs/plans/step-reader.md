# Plan: step-reader

- Started: 2026-09-25
- Milestone: C4 (docs/ROADMAP.md §C4, the reader and the real-part corpus)
- Idea (verbatim from the human): "step-reader"
- Idea: `docs/ideas/reader-cycle-scope.md` (absorbed). Its option B and
  its five decisions are taken as recommended. The charter delegates them,
  and ADR-0025 records them at step 1. The file was never committed, so
  step 1 deletes it only after the ADR holds everything it says.

## Goal

`arris_io::step::read` takes a Part 21 file of the AP203/214/242 B-Rep
subset and returns one result per solid in the file: a body that passes
the checker, with provenance naming the file entity each of its entities
came from, or a typed refusal naming the entity and a reason specific
enough to count. A parse error fails the whole file. Past parsing, one
refused solid does not hide the others.

Every entity is mapped exactly onto a variant Arris already has. The
analytic surfaces and curves stay themselves. B-splines, conics outside
the analytic kinds, and every extrusion and revolution that is not
analytic become `Nurbs`. Pcurves are rebuilt, never read. That includes
pcurves on NURBS surfaces, which is why `Surface::project` onto a NURBS
surface is built first. Degenerate edges the file left out are rebuilt at
the surface's singularity. Each entity's tolerance comes from the gaps
measured on it, not from the file's global uncertainty. Assemblies are
flattened to one body per solid instance, and lengths and angles are
converted to the caller's unit.

The reader is held three ways:

- Arris's own STEP of random shapes in random poses reads back as the
  same shape, to its entities' own tolerances (a property).
- Open CASCADE's STEP of every corpus fixture reads back to the counts,
  volume, area and centroid its `expected.json` records.
- Open CASCADE's STEP of every corpus fixture *converted to NURBS* reads
  back the same way. That conversion is the only source of free-form
  faces with seams and poles the tree has before the real-part corpus.

A fuzz target over the parser and the reader runs each night.

## Non-goals

- **The real-part corpus, the probe battery and the refusal histogram.**
  They are the next plan's (`real-part-corpus`, ADR-0025 §6): the NIST
  tier as `source: step` fixtures, the fetched tier, the battery of
  operations run on every part read, and the histogram printed. This plan
  builds the reader and the refusal type that histogram counts. It
  confirms no corpus licence and fetches nothing.
- **Healing.** No sewing, no gap closing past the cap, no open shells.
  Those are refused and counted; healing is its own cycle.
- **Booleans, blends or classification on NURBS faces.** They are the
  NURBS cycle's. A read NURBS face leaves its S5 face pairs `unchecked`,
  as the checker already reports, and nothing here closes that.
- **New geometry variants.** No `OFFSET_SURFACE` or `OFFSET_CURVE_3D`
  variant, and no surface of extrusion or revolution as a kind of its own.
  Offsets are refused and counted. If they rank high, a variant is the
  next cycle's decision.
- **Product structure in the model.** Arris has no assembly entity. An
  instance is a body, and its placement is baked into its geometry.
- **Writing anything new.** The writer is unchanged. Its known losses
  (degenerate edges, the handedness of left-handed pcurve conics) are
  what the reader rebuilds, not what it fixes.
- **Faceted B-reps, IGES, PMI, colours, layers and names.** A faceted
  B-rep is refused and counted. The rest is not read at all.

## Design deltas

- **ADR-0025 — the STEP reader: what it converts, refuses and flattens**
  (step 1). It records:
  - **§1 The subset and its exact conversions.** Every entity mapped, and
    the variant it becomes. A `SURFACE_OF_LINEAR_EXTRUSION` of a line
    becomes a `Plane`. Of a circle or an ellipse, in any direction, it
    becomes an `EllipticCylinder` or a `Cylinder`, with the section
    recomputed perpendicular to the direction. Of a spline it becomes a
    `Nurbs`. A `SURFACE_OF_REVOLUTION` of a line becomes a `Plane`,
    `Cylinder` or `Cone`. Of a circle in a plane through the axis it
    becomes a `Sphere` or a `Torus`. Anything else it becomes a rational
    `Nurbs`, exact. `PARABOLA`, `HYPERBOLA` and `POLYLINE` become
    `Nurbs`, exact. `TRIMMED_CURVE` and `RECTANGULAR_TRIMMED_SURFACE`
    become their basis. The edges and faces trim them anyway.
  - **§2 What is refused, by name.** Offsets, composite curves and
    surfaces, curve-bounded surfaces, degenerate tori, open shells and
    shell-based surface models, faceted B-reps, a torus Arris does not
    hold (`R ≤ r`), and a Part 21 edition-3 anchor or reference section.
  - **§3 Partial reads.** A parse error fails the file. Each solid is then
    `Ok` or its own refusal.
  - **§4 Tolerances.** Each entity is assigned the largest gap measured on
    it, raised to keep vertex ≥ edge ≥ face, floored at `min_tolerance`
    and capped relative to the part's size. Past the cap the solid is
    refused as a gap. It is never grown into a wrong solid.
  - **§5 Units, instances and the checker.** Lengths and angles convert to
    `ReadOptions::length_unit`, millimetres by default to match the
    writer. An assembly is flattened to one body per solid *instance*,
    with the placement path composed. The reader runs the checker at
    `Level::Fast` in **every** build, not only in debug builds. A
    violation there is the file's fault, not the kernel's, so it is a
    refusal (`Refusal::Invalid`) rather than a panic.
  - **§6 The battery and the histogram**, from the idea's option B, for
    the next plan: the fixed battery run on every checker-green part, and
    the table from each refusal reason (the reader's and the operations')
    to the named cycle it blocks.
- **`arris_io::step` becomes a module directory.** `step::write` keeps its
  path and signature. New public items:
  - `step::part21::{parse, Exchange, Header, Instance, Param, Part21Error}`
    (step 6). They are public so the fuzz target and a consumer that wants
    the header can reach them.
  - `step::read(&mut Model, &str, &ReadOptions) -> Result<Read,
    ReadError>`, with `ReadOptions`, `LengthUnit`, `Read`, `ReadSolid`,
    `ReadError`, `Refusal` and the fieldless `RefusalKind` the histogram
    counts (steps 8–10 and 16).
- **`arris-topo`: `Role::File(FileEntity)`** (step 10). `FileEntity {
  id: u64, instance: u32 }` names the `#id` an entity is `Generated` from
  and which placement of it. `Role` is exhaustive, so this is a breaking
  change to a public type, and step 10's commit names it.
  - Data-model §Provenance gains the rule: a solid's body, shells, faces,
    edges and vertices are `Generated` from their own file entities. A
    rebuilt degenerate edge is `Generated` from its face's entity, and an
    edge split at a seam from its `EDGE_CURVE`.
- **`arris-geom` gains:**
  - `NurbsCurve::{circle, ellipse, parabola, hyperbola}` over a range.
  - `NurbsSurface::{extrusion, revolution}`.
  - `Surface::to_nurbs(bounds)` for every analytic kind (step 2).
  - `Surface::project` onto `Nurbs` (step 3) and `pcurve_on` onto `Nurbs`
    (step 4). Both are `GeomError::Unsupported` today, by cycle-1 design.
    Data-model §Surfaces and §Pcurves change where they say so.
  - `NurbsSurface::closure` (step 4): a direction closed without
    periodic knots — a clamped full turn, what every exact revolution
    and a file's closed B-spline surface is — closes over its domain's
    length. Evaluation wraps a parameter outside the domain by it, and
    `Surface::period` reports it for a `Nurbs`, so the checker's seam
    rows, the pcurve unwrapping and a caller's placing of a seam treat
    it as a period. A behaviour change of two public methods, named in
    step 4's commit.
  - `NurbsSurface::project`, which `Surface::project` calls, and
    `AmbiguousLocus::MedialAxis` (step 3): a new variant of a public,
    exhaustive enum, so a breaking change, named in the commit. It is the
    locus of a tie between distinct points of a NURBS surface. The tie is
    decided to rounding like every other locus, not by a tolerance:
    `project` takes none.
- **A closed NURBS curve that is not periodic**, met by a surface at its
  closure, is one hit (step 5). Data-model §Curves says it is two today.
- **`arris-math`: the named constant for the gap cap** (step 12),
  commented with the evidence behind its value.
- **Oracle.**
  - `tools/oracle/occt_step.py` writes Open CASCADE's STEP of a fixture's
    result, plain or passed through `BRepBuilderAPI_NurbsConvert`. It is
    cached under ADR-0024's key.
  - `expected.py` records the converted shape's counts beside the
    original's, because conversion can add seams (step 15).
  - The geometry fixture kind gains a `nurbs` surface type
    (`SurfaceSpec::Nurbs` in `arris-debug`, unpublished) and its `samples`
    project onto it through `GeomAPI_ProjectPointOnSurf` (step 3).
- **The corpus runner gains a read-back stage**
  (`corpus::read_back_stage`, steps 14 and 15). `step_differs` fixtures
  skip it (ADR-0023). A fixture that fails it is a stage failure like any
  other.
- **`fuzz/` gains `step_read`**, seeded from every STEP file the corpus
  writes, and depends on `arris-io` (steps 7 and 17). The nightly job runs
  it for 30 minutes like the other targets.
- **The facade** re-exports `step::read`. ARCHITECTURE §How a consumer's
  kernel facade maps on gains the row (step 17).

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness or
bound has to be established here.

The free-form geometry comes first (steps 2–5). It is the one part nobody
has proven here, and every step after it leans on it. The parser comes
after, because it is routine.

- [x] Step 1 **[1]** — ADR-0025, from the idea's option B and its five
  decisions (§Design deltas), each argued once. Update `docs/ROADMAP.md`
  §C4: C4 is two plans, this one and then `real-part-corpus`, and its
  accept line skips `step_differs` fixtures. Delete
  `docs/ideas/reader-cycle-scope.md` once the ADR carries everything in it.
- [x] Step 2 **[2]** — Exact NURBS forms in `arris-geom`:
  - conic arcs as rational quadratics;
  - the extrusion of a NURBS curve;
  - its revolution through any angle, as rational quadratic arcs of at
    most 90°;
  - `Surface::to_nurbs(bounds)` for each analytic kind.

  Test: a property over random frames, radii and ranges that each form
  evaluates onto its analytic twin within rounding at every sampled
  (u, v). A full-turn revolution closes on itself, and a sphere's twin
  collapses its pole rows to one point.
- [x] Step 3 **[3]** — `Surface::project` onto a NURBS surface. It finds
  the global nearest point, not a local one: Bézier patches are bounded by
  their control hulls and pruned against the best distance found so far,
  and each surviving candidate is refined by Newton. Two candidates that
  tie within the tolerance at distinct parameters are
  `GeomError::Ambiguous`, never a silent choice. A pole returns the
  collapsed row's own `v` with `u` at its knot start, as the sphere's
  closed form does.

  Tests:
  - a property against step 2's twins, whose closed-form `project` is the
    oracle, over points near seams, near poles and far off the surface;
  - `tests/fixtures/geom/c4-nurbs-projections` against Open CASCADE's
    `GeomAPI_ProjectPointOnSurf` on free-form surfaces with no twin
    (a saddle, a bump, a surface folded back on itself).
- [x] Step 4 **[3]** — `pcurve_on` onto a NURBS surface. Projected samples
  at the surface's `chord_steps` are fitted to `Curve2::Nurbs`
  (`nurbs::fit`) and verified at E4's samples. The samples are unwrapped
  across a periodic seam, and across a *closed* seam that is not periodic
  (the knot ends meet in 3D), so the pcurve is continuous. A seam use
  takes the side its loop needs, which is decided by the caller and
  passed in, as the analytic seams already are. A curve running through a
  collapsed row is refused (ADR-0021). A curve ending there takes the
  row's `u` from its neighbour along the curve.

  Test: a property of curves lying on step 2's twins, each pcurve's image
  held to the curve within the linear tolerance:
  - meridians and parallels of a NURBS sphere, through the seam and to a
    pole;
  - a NURBS torus's section circles;
  - an oblique ellipse on a NURBS cylinder.
- [x] Step 5 **[2]** — A closed NURBS curve that is not periodic, met by a
  surface where its two ends join, is reported as one hit at its start
  parameter, not one at each end (ROADMAP §C4; data-model §Curves).

  Test: `tests/fixtures/geom/c4-closed-curve-hits`, a clamped closed
  B-spline crossing a plane and a cylinder at its join, held to Open
  CASCADE's hit count.
- [ ] Step 6 **[2]** — The Part 21 parser, `step::part21::parse(&str) ->
  Result<Exchange, Part21Error>`. It handles:
  - the header's three entities;
  - one or more `DATA` sections;
  - simple and complex (external-mapping) instances;
  - integers, and reals in every spelling the grammar allows (`1.`,
    `-1.E-5`);
  - strings with `''` and the `\X\`, `\X2\…\X0\`, `\X4\…\X0\` and `\S\`
    encodings;
  - enumerations, binaries, typed parameters, `$`, `*` and `/* comments */`.

  Instances are kept in a `BTreeMap` by id. A duplicate id, a malformed
  token or an unterminated string is a `Part21Error` carrying the line,
  the column and the instance id. Resolving references is the mapper's
  job, not the parser's.

  Tests:
  - a unit test per production;
  - every STEP file the corpus writes parses;
  - a hand-written file per error, each naming its line.
- [ ] Step 7 **[1]** — The fuzz target `fuzz/fuzz_targets/step_read.rs`,
  calling `part21::parse` for now. It is seeded by `fuzz/seed.rs` from
  every STEP file the corpus writes. The nightly job runs it for 30
  minutes. Proof: one local hour without a crash, or each crash fixed with
  a test that fails without the fix.
- [ ] Step 8 **[2]** — Units and the representation context.
  - The length unit comes from `SI_UNIT` with its prefix, or from a
    `CONVERSION_BASED_UNIT` (inch, foot), and converts to
    `ReadOptions::length_unit`.
  - The plane-angle unit is radians or a degree `CONVERSION_BASED_UNIT`,
    applied to every angle a geometric entity carries (a cone's
    semi-angle, a trimmed curve's parameters on a conic).
  - `UNCERTAINTY_MEASURE_WITH_UNIT` is read as the file's claim and kept
    beside the result, never as an entity's tolerance.
  - A context with no length unit is refused. Points, directions and
    `AXIS2_PLACEMENT_*` map through the conversion.

  Test: hand-written one-block files in millimetres, metres and inches,
  and a cone with its semi-angle in degrees, each read to the same
  geometry.
- [ ] Step 9 **[2]** — The geometry mapping. Every curve and surface of
  ADR-0025 §1 maps to its variant, through step 2's forms where it is not
  analytic. That covers:
  - every B-spline subtype (`UNIFORM`, `QUASI_UNIFORM`, `BEZIER`,
    `PIECEWISE_BEZIER`, the `_WITH_KNOTS` form, and the rational complex
    instances);
  - knot multiplicities expanded, and closed or periodic forms recognised.

  Everything ADR-0025 §2 names becomes a `Refusal` whose `RefusalKind`
  names the entity type. Refusals are introduced here.

  Tests:
  - the writer's own output for every geometry kind maps back equal;
  - one hand-written instance per conversion, held pointwise to its
    source's definition;
  - one per refusal.
- [ ] Step 10 **[3]** — One solid's topology. A `MANIFOLD_SOLID_BREP` or
  `BREP_WITH_VOIDS` becomes a `Builder::assemble` `Assembly`:
  - vertices from `VERTEX_POINT`;
  - edges from `EDGE_CURVE`, with the range found by projecting the
    vertices onto the curve, a closed edge taking the full period, and
    `same_sense` honoured;
  - faces with orientation from `same_sense`, loops in effective order from
    each bound's flag and `ORIENTED_EDGE` flags;
  - an edge used twice in one loop becomes a seam, its two pcurves on the
    period's two sides;
  - every pcurve rebuilt by `pcurve_on`.

  A void's `ORIENTED_CLOSED_SHELL` is turned back. Provenance comes from
  `Role::File`, which is added to `arris-topo` in this step (a breaking
  change, named in the commit body). The checker runs at `Fast`, and a
  violation is `Refusal::Invalid`. Tolerances are the model's default
  here; step 12 replaces them.

  Test: the Arris STEP of every corpus fixture reads back with the
  fixture's counts, and a volume, area and centroid within its tolerances.
  The fixtures with a degenerate edge wait for step 11, each under an
  exclusion that names it.
- [ ] Step 11 **[3]** — Degenerate edges rebuilt. A loop whose (u, v) walk
  jumps along a singular row — a sphere's pole, a cone's apex, a NURBS
  surface's collapsed row — gets the degenerate coedge the writer left
  out, on its own pcurve along the row, in the sense that closes the loop.
  A `VERTEX_LOOP` at a singularity is read as one. A jump that is not
  along a singular row is `Refusal::OpenLoop`, naming the face.

  Test: step 10's exclusions are lifted, and every corpus fixture with a
  pole or an apex reads back to its counts, the degenerate edges included.
- [ ] Step 12 **[2]** — Per-entity tolerances from measured gaps
  (ADR-0025 §4).
  - A vertex takes its distance to each edge's curve end.
  - An edge takes its curve's distance from each face's surface at E4's
    samples, and its pcurves' deviation.
  - A face takes the floor.

  Values are raised to keep vertex ≥ edge ≥ face. A gap past the cap is
  `Refusal::Gap { entity, gap }`. The cap's constant goes in `arris-math`,
  its value set from the gaps measured on Open CASCADE's files in steps 14
  and 15 and on the files of the literature, the evidence cited in its
  comment.

  Test: hand-perturbed files, with a vertex moved by δ below the cap and
  above it and an edge curve lifted off its face. Each reads to a
  checker-green solid whose tolerance is at least δ, or to the gap refusal
  naming the entity.
- [ ] Step 13 **[2]** — The write → read round trip as a property over
  `prop::body`'s shapes in random poses, sharded (`prop_shards!`). It
  asserts:
  - the checker at `Full` is green, with nothing unchecked;
  - the counts are equal;
  - volume, area, centroid and inertia are within the entities' own
    tolerance bound (`within_own_tolerance`'s first-order form, ADR-0023);
  - random probe points classify the same against the original and the
    read-back body.

  The hook runs it at 256 cases, CI at 1000 and the nightly at its tier.
- [ ] Step 14 **[2]** — Open CASCADE's STEP of every corpus fixture read
  back. `tools/oracle/occt_step.py` writes it, under ADR-0024's cache.
  `corpus::read_back_stage` holds the read result to `expected.json`'s
  counts, volume, area and centroid at the fixture's tolerances.
  `step_differs` fixtures skip the stage (ADR-0023). Every fixture passes,
  or its failure is shrunk to a `regression/` fixture under the kernel
  rule.
- [ ] Step 15 **[3]** — Free-form faces from a file. Every corpus fixture
  goes through `BRepBuilderAPI_NurbsConvert`, is written by Open CASCADE
  and is read back:
  - held to the same volume, area and centroid;
  - held to the converted shape's own counts, which `expected.py` records
    (`nurbs_counts`), since conversion can add seams;
  - S5 face pairs on NURBS faces are expected `unchecked`, and nothing
    else is.

  This is the proof of steps 3, 4 and 11 on files: closed and periodic
  NURBS surfaces, converted spheres' poles, converted tori's two seams.
  Failures are shrunk to `regression/` fixtures.
- [ ] Step 16 **[2]** — Assemblies and several solids (ADR-0025 §3 and §5).
  - The product structure is flattened (`SHAPE_DEFINITION_REPRESENTATION`,
    `NEXT_ASSEMBLY_USAGE_OCCURRENCE`,
    `CONTEXT_DEPENDENT_SHAPE_REPRESENTATION`,
    `REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION`,
    `ITEM_DEFINED_TRANSFORMATION`, `MAPPED_ITEM`), with the placement path
    composed and one body per instance. Instances are numbered in the
    deterministic order of their paths.
  - Each solid's result is independent of the others'.

  Tests:
  - an Open CASCADE XCAF assembly of two fixtures, one placed twice, reads
    to three bodies with the oracle's volumes and centroids;
  - a file with one solid spoiled by a hand-inserted `OFFSET_SURFACE`
    reads to the others plus that refusal;
  - two reads of one file give identical dumps.
- [ ] Step 17 **[1]** — Closing the surface.
  - `arris` re-exports `step::read`, and the facade table gets its row.
  - The fuzz target moves from `part21::parse` to `step::read` and is
    re-seeded.
  - `arris-debug` reads a STEP file for the `inspect` skill (dump, render,
    check).
  - A rustdoc example on `step::read`.
  - `RefusalKind::ALL`, so the next plan's histogram iterates every kind.

## Acceptance

- `cargo nextest run --workspace` passes:
  - the corpus with the read-back stage on Open CASCADE's STEP of every
    fixture (step 14) and of its NURBS conversion (step 15), `step_differs`
    skipped;
  - `geom/c4-nurbs-projections` and `geom/c4-closed-curve-hits` green;
  - the round-trip property green at the hook's 256 and CI's 1000 on the
    fixed seed.
- One nightly run with the round-trip property at the nightly tier green
  on its date seed, and the `step_read` fuzz target's 30 minutes finishing
  with every crash it found fixed or shrunk into a fixture.
- `step::read` on every file above returns no panic and no `Ok` body the
  checker rejects at `Full`. That is the "no wrong solid" half of C4's
  accept line. Its "real parts" half is `real-part-corpus`'s.

## Docs to update on completion

- `docs/ARCHITECTURE.md`:
  - §Crates and the layer rule: `arris-io`'s row says "writer and reader";
  - §Formats and tools: the reader's paragraph (subset, conversions,
    refusals, partial reads, units, instances, tolerances, the checker in
    every build);
  - §Errors: `ReadError` and `Refusal`;
  - §How a consumer's kernel facade maps on: the `step::read` row;
  - the fuzz line: `step_read`.
- `docs/DATA-MODEL.md`:
  - §Surfaces: `project` onto `Nurbs`, and `to_nurbs`;
  - §Pcurves: `pcurve_on` onto `Nurbs`, seam sides on closed NURBS;
  - §Curves: a closed non-periodic NURBS curve is one hit at its join;
  - §NURBS: the exact forms;
  - §Provenance: `Role::File`, and what a read entity is `Generated` from;
  - §Tolerances: tolerances assigned on read and the cap.
- `docs/ROADMAP.md`:
  - §C4: this plan's half done, with its numbers (fixtures read back, NURBS
    conversions read back, round-trip cases, fuzz hours);
  - §Beside the cycles: the STEP fuzz target in the harness line.
- `crates/arris-io/src/lib.rs` crate doc: "(later reader)" becomes the
  reader.
- `tests/fixtures/README.md`: the read-back stage and `nurbs_counts`.
- `tools/oracle/README.md`: `occt_step.py`.
- `fuzz/README.md`: `step_read`.
- `docs/BACKLOG.md`: a line per refused entity family (offsets, faceted
  B-reps, composite curves) until the histogram ranks them.
- `AGENTS.md` current state: the reader landed, and the corpus plan comes
  next.

## Open questions

- ⚠ OPEN: the gap cap's value. It is relative to the part's size, but the
  fraction is not yet known. **Agent**, by step 12, from gaps measured on
  Open CASCADE's files and on the files of the literature. It is revisited
  by `real-part-corpus` against real parts, and changing it there is a
  one-constant commit with its evidence.
- ⚠ OPEN: whether `BRepBuilderAPI_NurbsConvert` round-trips every fixture
  in Open CASCADE itself. If the oracle's own converted shape fails
  `BRepCheck_Analyzer` or its own round trip, the fixture's NURBS variant
  is recorded as `step_differs`-like. **Agent**, at step 15. If that holds
  for more than a handful of fixtures, it is an amendment to ADR-0023, not
  a new escape.
