# Backlog

One line per raw idea. Picking one up means `/idea` (needs thinking) or
`/plan` (obvious); the line is removed then — as is a line a roadmap cycle
has committed to, which now lives in `docs/ROADMAP.md` instead. Rejected
ideas keep one line below with the reason, so the same idea is not
re-brainstormed.

- `cargo-semver-checks` in CI once the first non-placeholder version is published
- Benchmarks (`divan` or `criterion`) for tessellation and the boolean corpus, so a robustness fix that costs 10× shows up
- `cargo-fuzz` targets for the STEP reader and the intersectors, seeded from the fixture corpus
- A `no_std`-friendly `arris-math`, if an embedded or wasm consumer ever wants it
- IGES read/write (SEED §6, later cycles)
- Publish the workspace crates to crates.io over the 0.0.1 `arris` reservation once cycle 1's vertical slice passes its corpus
- NURBS degree elevation as a primitive edit (data-model §NURBS names it; no C1 step needs it — knot insertion is enough for M1's fitting)
- Knot insertion on a periodic NURBS that keeps the wrap: today the result's knots no longer imply a period and it extrapolates outside its domain (data-model §NURBS); needed once a periodic curve from STEP is edited
- S5's coincident-surface arm tests face overlap on a grid of interior points carried through 3D, so an overlap thinner than the grid spacing passes; an exact (u, v) region intersection replaces it once a boolean can produce such faces (M4 finding candidate). Its cost — each of the grid's 529 points a walk over the other face's polygon, tens of thousands of points for a wall piece bounded by oblique sections (M4 step 9 finding) — is retired: `FaceDomain::side` reads a `SideIndex` (cylinder-cylinder-booleans step 9). Reviewed at M4 step 10: the boolean's exact overlap test is the (u, v) arrangement in `arris-ops`, above `arris-check`, so S5 cannot call it; the exact test for the checker is a polygon–polygon overlap in `region2` (segment crossings plus one containment), still to write
- `Builder::finish` makes `Solid` only; `Sheet` needs an operator that leaves an edge with one use (a `mev` strut not closed by a `mef`), which no C1 operation asks for — with sheet bodies in C7
- STEP cannot carry a left-handed pcurve conic (`AXIS2_PLACEMENT_2D` is direct) or a degenerate edge's coedge; both are dropped on write (architecture §Formats and tools). A reader (C7) must rebuild them from the 3D curve and the surface's singularity
- `TriMesh::push_position` casts the position count to `u32` with no bound, so a mesh past four billion vertices wraps silently instead of returning a typed error (M3 finding, confirmed by the kernel-seams review)
- Two coplanar conics that are not the same conic (a circle and an ellipse, two distinct ellipses) are `GeomError::Unsupported` in `intersect_curves`: the pair is a quartic. Two short cylinders crossing steeply make one (a rim circle beside the ellipse its cap plane cuts from the other wall, `boolean/short-cross-cylinders-fuse`), but only the coincidence is asked there, which `curves_coincide` answers; a boolean over cones, spheres or tori will need the points (C3, beside the quadric-pair intersector)
- A NURBS surface has no `project` (cycle 1, by design), so a NURBS face's mesh deviation can only be measured against a closed form; the tessellation property test covers the analytic kinds and one hand-built saddle (M3 finding)
- The boolean's remaining hot passes are sequential: the edge-on-face hits of the pave model and the ray cast that classifies every piece (`classify_point`), plus the checker that runs after every operation in debug builds. `parallel` covers the face-pair intersections and the per-face splitting and buys 1.4× on the eight-cut bolt pattern; the hits are a candidate for the same treatment, and the classifier needs its ray casts to stay in a fixed order (M4 step 13 finding)
- Per-vertex normals and (u, v) in `TriMesh`, alongside the `f32` position boundary (architecture §Threading `⚠ OPEN`), once a consumer's renderer asks for either (M3 non-goal)
- Adaptive or curvature-driven mesh refinement beyond a chord tolerance, if a consumer's mesh sizes come out too coarse or too dense against `arris-mesh`'s uniform (u, v) grid (M3 non-goal)
- Oblique extrusion, refused today as `Reason::DirectionNotNormal`: a line sweeps a plane at any angle, but an arc sweeps a cylinder of elliptical section that no `Surface` variant holds — with C5's sweep along a path (M5 `⚠ OPEN` 3)
- A fitted pcurve fallback over `Surface::project` on cones, spheres and tori (an oblique section of a cone, a small circle of a sphere about none of its axes, a Villarceau circle), `GeomError::Unsupported` today, for the first operation that makes such a curve on such a face (M5 non-goal)
- A spindle torus (`R < r`) as a surface, so a revolve of an arc whose circle crosses its axis is more than `Reason::SpindleTorus`, when an operation needs one (M5 non-goal)
- `ops::planar_face`, a `Profile` as a one-face sheet body, with C7's sheet bodies; the sweeps' cap builder is private until then (M5 `⚠ OPEN` 1)
- Lumps stored on the `Body` entity and in the native format rather than derived by `arris_check::lumps`: every STEP write and corpus count of a multi-shell body re-runs B1's face-pair meeting test and ray casts, so storing becomes worth it once that shows up as a cost (multi-shell `⚠ OPEN` 3)
- Split `arris-debug` into a light testkit (samples, dump, prop harness, geometry and profile strategies, expressions) above `arris-topo`, a corpus crate, and the renderer: today it pulls ops, mesh, io and `image` into every lower crate's test build (needs an idea and an ADR; kernel-seams review)
- `GeomError::Degenerate { reason: String }` → a typed `DegenerateReason` enum, with `basis::validate`, `Spline::new` and `FitError` returning it; tests match variants, not substrings (kernel-seams review)
- `BuildError`'s 39 variants nested by API (operators, `finish`, `assemble`); `primitive::line` given its own error instead of `BuildError::Empty` (kernel-seams review)
- The boolean's `side: usize` and `Option<bool>` selections → `enum Operand { A, B }` and `enum Selection { Drop, Keep, KeepReversed }`, including the public `EdgeImage::side`; with `plans/cylinder-cylinder-booleans` if it touches `Op::select` (kernel-seams review)
- ops and io reach math, geom and topo through `arris_check::arris_topo::…` re-export chains (122 paths in ops): declare the direct, layer-legal dependencies and hide the chain re-exports (kernel-seams review)
- Narrow `pub` internals: ops `boolean` interference types, topo `Staged*`/`RawInsert`/`CHUNK_SIZE`, geom tuning constants and `segments_intersect`/`interior_knots`, check `Report::with_*`; one export scheme per crate (kernel-seams review)
- Split the files that hold several concerns, each when a feature next touches it: `topo/builder.rs` (refs, staged, error, euler, finish, assemble, validate), `ops/sweep.rs` (one skeleton for extrude and revolve), `boolean/pave.rs` (hits, sections, coincident, each phase a typed output), `debug/corpus.rs` (one function per stage), `check/check.rs` (rows by entity), `mesh/cdt.rs` (locate/insert/legalize, constraint recovery, classification), `geom/pcurve.rs` (plane, revolution, fitted) (kernel-seams review)
- A miter of two blends with unequal dihedrals (a slanted prism's vertical edge and its cap edge): the two far contacts meet the sharp third edge at two points, so the corner is the cylinders' ellipse plus a second arc where the wider blend is trimmed by the face across, and the third face takes that arc — refused as `VertexBlend` since fillet-and-chamfer step 2; C6's vertex blends
- The discretisation chord from the surface's speed over the face's (u, v) box (`Surface::speed_bounds`), not at the first pcurve's first point, before NURBS faces or blends on tori with a large R/r ratio (kernel-seams review)
- ADR-0005's ribbon flattening detects a ruled direction only through an infinite `chord_steps`, so a ruled NURBS never gets it: a `Surface::is_ruled(direction)` query (kernel-seams review)
- Check tests by topic, not by the plan step that wrote them: `fast_part1`/`fast_part2` into row-family files over a shared `tests/common`; `ops/tests/boolean.rs` (two files joined) and `revolve.rs` split; a `corpus_tests!` macro checked against `fixtures::corpus()` instead of two hand lists (kernel-seams review)
- `oracle::scratch_fixture` re-runs OCCT on every test run although `recipe_hash` could skip an unchanged recipe; the workspace paths derived five ways into one `paths` module honouring `CARGO_TARGET_DIR` (kernel-seams review)
- S5 and B1 compare every face pair behind a box test: sort-and-sweep the boxes before the intersector (kernel-seams review)
- A multi-shell boolean assembles its result twice: `lump_order` finishes a scratch copy of the model to read `arris_check::lumps`, then the result is assembled again in lump order; `lumps` over the assembly's shells before `finish` would build it once (kernel-seams review)
- `Builder` kill operators and `Slots::len` are linear in the whole builder: a live slot count and an incremental edge-use index, once a blend runs local operators on large bodies (kernel-seams review)

## Rejected
