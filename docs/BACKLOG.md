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
- S5's coincident-surface arm tests face overlap on a grid of interior points carried through 3D, so an overlap thinner than the grid spacing passes; an exact (u, v) region intersection replaces it once a boolean can produce such faces (M4 finding candidate). It is also the checker's one super-linear cost that shows: each of the grid's 529 points is a `point_side` over the other face's polygon, tens of thousands of points for a wall piece bounded by oblique sections, so a `Full` check of a fuse whose two wall pieces' boxes overlap spends up to a second in that pair at `opt-level = 1` and forty at 0 (M4 step 9 finding); the exact test, or a bucketed `point_side`, retires it. Reviewed at M4 step 10: the boolean's exact overlap test is the (u, v) arrangement in `arris-ops`, above `arris-check`, so S5 cannot call it; the exact test for the checker is a polygon–polygon overlap in `region2` (segment crossings plus one containment), still to write
- `Builder::finish` makes `Solid` only; `Sheet` needs an operator that leaves an edge with one use (a `mev` strut not closed by a `mef`), which no C1 operation asks for — with sheet bodies in C7
- STEP cannot carry a left-handed pcurve conic (`AXIS2_PLACEMENT_2D` is direct) or a degenerate edge's coedge; both are dropped on write (architecture §Formats and tools). A reader (C7) must rebuild them from the 3D curve and the surface's singularity
- The Euler line counts degenerate edges, so a sphere derives genus 1 (`sample::sphere` prints `2/3/1/1/1 g1 = 0`); excluding them would change every fixture's printed counts and the oracle's own derivation, so it waits for a cycle that can regenerate both (M3 finding)
- `TriMesh::push_position` casts the position count to `u32` with no bound, so a mesh past four billion vertices wraps silently instead of returning a typed error (M3 finding)
- Two coplanar conics that are not the same conic (a circle and an ellipse, two distinct ellipses) are `GeomError::Unsupported` in `intersect_curves`: the pair is a quartic and no C1 recipe makes one, but a boolean over cones, spheres or tori will (C3, beside the quadric-pair intersector)
- A NURBS surface has no `project` (cycle 1, by design), so a NURBS face's mesh deviation can only be measured against a closed form; the tessellation property test covers the analytic kinds and one hand-built saddle (M3 finding)
- The boolean's remaining hot passes are sequential: the edge-on-face hits of the pave model and the ray cast that classifies every piece (`classify_point`), plus the checker that runs after every operation in debug builds. `parallel` covers the face-pair intersections and the per-face splitting and buys 1.4× on the eight-cut bolt pattern; the hits are a candidate for the same treatment, and the classifier needs its ray casts to stay in a fixed order (M4 step 13 finding)
- Per-vertex normals and (u, v) in `TriMesh`, alongside the `f32` position boundary (architecture §Threading `⚠ OPEN`), once a consumer's renderer asks for either (M3 non-goal)
- Adaptive or curvature-driven mesh refinement beyond a chord tolerance, if a consumer's mesh sizes come out too coarse or too dense against `arris-mesh`'s uniform (u, v) grid (M3 non-goal)

## Rejected
