# The fixture corpus

The unit of acceptance (`docs/ROADMAP.md` §Fixtures). One directory per
fixture, `<area>/<slug>/`, of one of two kinds — a **solid** (the default:
a recipe built and measured) or **geometry** (`"kind": "geometry"`, under
`geom/`: analytic surfaces and curves evaluated, projected onto and
intersected; §Geometry fixtures below) — holding:

| File | Written by | Holds |
|---|---|---|
| `fixture.json` | a person or the agent | the **recipe**: operands and operations both sides evaluate, probe points, tolerances, and the closed-form `analytic` values |
| `expected.json` | `tools/oracle/expected.py`, never by hand | the **oracle's answer** per variant: volume, area, centroid, the inertia tensor about the centroid, counts, Euler characteristic and genus, probe classifications, plus the OCCT version and the recipe hash |
| `dump.txt` | Arris, through the corpus runner under `ARRIS_BLESS=1`, once the fixture passes | the text dump of the result, the regression guard for ids and provenance, committed once the fixture passes and required by the lint of every fixture the runner compares; `dump.<variant>.txt` for a variant other than `default` |

The Rust reading of both files is `arris_debug::fixtures`; the Python one
is `tools/oracle/oracle/recipe.py` and `fixture.py`. The corpus lint
(`crates/arris/tests/corpus_lint.rs`, run by `cargo test`) checks every
directory: both files present and parseable, `expected.json` not stale, the
Euler line zero, every `analytic` value matching the oracle to 1e-6
relative, counts and probe expectations exactly — and every solid under
`primitive/`, `transform/`, `boolean/`, `sweep/`, `provenance/` or
`blend/` that the
runner compares (the oracle built a solid, the recipe expects no refusal)
carrying its committed dump per variant, which a fixture only has once it
passed and was blessed. An `#[ignore]`d fixture in those areas therefore
fails the lint: they hold zero ignored fixtures by test, and CI runs the
corpus's ignored tests as well, all but the `regression_*` ones. A
fixture under `regression/` — a failure waiting for its fix, below — needs
no dump and must have none: one with a dump passes, belongs in its area,
and fails the lint until it moves. The oracle's own self-test
(`uv run --project tools/oracle tools/oracle/selftest.py`) reproduces every
committed `expected.json` and round-trips each result through STEP.

The corpus **runner** (`arris_debug::corpus::run(dir, variant)`, one
`#[test]` per fixture in `crates/arris/tests/corpus.rs`) is the fixture
test itself: it builds the recipe in Arris, runs the checker at `Full`
(nothing violated, nothing undecided), compares counts — `solids` as the
result's lumps, `arris_check::lumps` — and genus against
`expected.json`, writes STEP under `target/inspect/` and has the oracle
read it back (`compare.py`), measures it over the B-Rep
(`ops::measure::mass_properties`) and holds its volume, area, centroid
and inertia tensor to the oracle's within `volume_rel`, `area_rel`,
`centroid_abs` and `inertia_rel`, tessellates the result at `mesh_chord` and
holds the mesh closed with a positive signed volume within
`mesh_volume_rel` of the oracle's, classifies every probe point against
the result (`arris_check::classify::classify_point`) and holds it to the
oracle's class *exactly* — both sides have their own tolerance for "on",
the fixture's `probe` for the oracle and the entities' own for Arris, and
a probe is placed so the two agree, so a disagreement is a finding and
never something a band is widened to cover — asserts every step's
provenance accounting, and diffs the dump against `dump.txt`. A result
the oracle recorded no solid for (`degenerate` in `expected.json`) must
fail with `OpError::Degenerate` at its result step, and one whose recipe
says `analytic.expect_error` must fail with that typed refusal; either
ends the run there, nothing later compared, no `dump.txt`. Every op of
the recipe grammar has its operation; a fixture that does not pass yet —
a failure shrunk to one — lives under `regression/<slug>/`, outside the
lint's areas, with its test `regression_<slug>` `#[ignore = "why"]`d in
`corpus.rs`; it fails at the stage that differs under `--include-ignored`,
so the day it passes the test says so, and the commit that fixes it moves
the directory into its area, blesses its dump and renames the test
`<area>_<slug>`. `ARRIS_BLESS=1 cargo test -p arris --test corpus
<name>` writes the dump instead of diffing it; commit the file as part
of the step that made the fixture pass, and a later change to it is a
`fixtures:` commit that says why the ids or the geometry moved.

## `fixture.json`

```json
{
  "description": "what the fixture is for",
  "params":   {"R": 35, "r": 3, "t": 10},
  "variants": {"thicker": {"t": 12}},
  "steps": [
    {"name": "plate", "op": "box", "min": [0, 0, 0], "max": [100, 100, "t"]},
    {"name": "h0", "op": "cylinder", "base": ["50 + R * cos(radians(0))", 50, -1],
     "axis": [0, 0, 1], "radius": "r", "height": "t + 2"},
    {"name": "result", "op": "cut", "target": "plate", "tool": "h0"}
  ],
  "result": "result",
  "probes": [{"label": "inside", "point": [50, 50, 5], "expect": "in"}],
  "tolerances": {"volume_rel": 1e-9, "area_rel": 1e-9, "centroid_abs": 1e-7, "probe": 1e-7},
  "analytic": {
    "volume": "100 * 100 * t - pi * r * r * t",
    "area": "...",
    "centroid": [50, 50, 5],
    "counts": {"vertices": 10, "edges": 15, "faces": 7, "loops": 9, "shells": 1},
    "genus": 1
  }
}
```

- **Numbers** anywhere in `steps`, `probes` and `analytic` may be a JSON
  number or a string expression over `params`: `+ - * / ^`, parentheses,
  `pi`, and `sin cos tan sqrt radians degrees abs`. `^` is power,
  binding tighter than `* /` and a unary minus (`d^2 / 4` is `d²/4`,
  `-2 ^ 2` is `−4`) and right-associative (`2 ^ 3 ^ 2` is `512`); `**` is
  rejected, not read as power, on both sides. Both sides evaluate the same grammar: proven by
  `expr-cases.json` (in this directory), which
  `arris_debug::fixtures::expr`'s own test and the oracle's
  `selftest.py` both evaluate.
- **`variants`** override params; every variant gets its own result in
  `expected.json`, and `default` (the base params) always exists. A probe's
  `expect` is checked in every variant, so leave it out (`null`) when the
  answer changes between variants.
- **Steps** are chained by `name`; `result` names the fixture's result.
  Angles are degrees; directions and plane axes are normalised by the
  interpreter.

| `op` | Fields |
|---|---|
| `box` | `min`, `max` |
| `cylinder` | `base` (centre of the base cap), `axis`, `radius`, `height` |
| `profile` | `plane` `{origin, x, y}`; `outer` and `holes` as loops: `{"circle": {"center": [u, v], "radius": r}}` or `{"start": [u, v], "segments": [{"line_to": [u, v]}, {"arc_to": [u, v], "via": [u, v]}, …]}` (the last segment ends at `start`; loop orientation is irrelevant) |
| `extrude` | `profile`, `direction`, `length` |
| `revolve` | `profile`, `axis` `{origin, direction}`, `angle_deg` |
| `transform` | `of`, optional `translate`, optional `rotate` `{axis, origin, angle_deg}`; rotation first |
| `fuse`, `common` | `a`, `b` |
| `cut` | `target`, `tool` |
| `fillet` | `of`, `edges` (a list of points, one on each edge to blend), `radius` |
| `chamfer` | `of`, `edges` (as a `fillet`'s), `distance` (one, measured on both faces from the edge) |

- **A `fillet`'s or a `chamfer`'s edges are named by a point each**, so a selection
  survives a transform and a second blend, which a role does not: Arris
  takes the edge `classify_point` answers `On(Edge)` for, the oracle the
  nearest edge by `BRepExtrema`, and both refuse a point that is within
  `probe` of two edges or on none — a vertex, a face, the inside or the
  outside (`CorpusError::EdgePoint`).
- **A `profile` plane's `x` and `y`** must be orthogonal (each normalised
  first): refused on both sides, by the same named tolerance
  (`arris_math::Precision::DEFAULT.angular_tolerance`, Open CASCADE's
  `Precision::Angular`, mirrored as a literal in `recipe.py` with a
  comment pointing back here).
- **`analytic`** is the author's closed form, the cross-check that catches a
  convention mismatch on either side (profile orientation, seam counting,
  which faces a fuse keeps). Every field is optional; `degenerate: true`
  says the result has no volume (`boolean/flush-common`,
  `boolean/swallow-cut`, `boolean/disjoint-common`), and then nothing
  else is compared.
  `expect_error: "tangent-contact" | "non-manifold" | "blend-too-large" |
  "tangent-chain" | "vertex-blend"` says Open CASCADE
  builds a result Arris refuses by design (the tangent cases,
  `Reason::TangentContact` — ADR-0004, whose contact Open CASCADE carries
  as an edge of four faces, so where that makes the Euler characteristic
  odd, as in `boolean/tangent-hole`, the oracle records no genus
  and the recipe states none; `boolean/edge-touching-fuse`, two
  solids sharing an edge, `Reason::NonManifold` — ADR-0006, whose
  compound has an odd Euler characteristic, so the oracle records no
  genus for it and the recipe states none; a blend it builds and Arris
  refuses as `Reason::BlendTooLarge`, `TangentChain` or `VertexBlend` —
  ADR-0007): the oracle's numbers are
  recorded and the lint still
  cross-checks them against the other `analytic` values, but the runner asserts the typed error and compares
  nothing — and the oracle's self-test records the result without
  round-tripping it through STEP, since nothing ever reads it back
  (`boolean/tangent-hole`'s slit carries the tangent ruling as an edge of
  four faces, which Open CASCADE's own reader does not give back as a
  closed surface). `counts_differ: "why"` says Arris builds the result with
  counts that differ from Open CASCADE's by a stated convention
  (`boolean/tangent-outside-cut`: Open CASCADE imprints the tangent
  ruling on the touched face, Arris keeps the face whole); `counts` is
  then required and is Arris's, the runner and `compare.py` hold the
  result to it, the oracle's counts stay in `expected.json` as the
  record, and the lint holds both to the Euler line. `genus` is what the
  Euler line is checked with:
  `V − E + F − (L − F) − 2(S − G) = 0` with the oracle's counts.
- **Tolerances** are the fixture's; absent ones take the defaults shown.
  Counts and classifications are always exact. Three keys are Arris's
  alone and never reach the oracle: `mesh_chord` (default `1e-3`), the
  chord tolerance the runner tessellates the result at, and
  `mesh_volume_rel` (default `2e-3`), how far the mesh's signed volume may
  be from the oracle's volume — sized by the closed form of an inscribed
  prism, `4δ / (3r)` at that chord on the corpus's smallest radius
  (ADR-0003); and `inertia_rel` (default `1e-9`), read by the runner for
  the `measure` stage's inertia tensor.

## Geometry fixtures (`"kind": "geometry"`)

`"kind"` absent or `"solid"` is the recipe above; anything else that is
not `"geometry"` is an error on both sides (`fixture_kind` in the
oracle, `arris_debug::fixtures::kind_of` in Rust) — never silently read
as a solid.

The M1 oracle: no solid, no STEP. `fixture.json` names analytic surfaces
and curves by their frames and radii, the parameters to evaluate them at,
the points to project onto them, and the pairs to intersect; the oracle
writes what Open CASCADE's `Geom_*::D2`, `GeomAPI_ProjectPointOn*`,
`IntAna_QuadQuadGeo` and `IntAna_IntConicQuad` say, and
`crates/arris-geom/tests/oracle.rs` holds Arris to it: evaluations and
projected parameters to 1e-9 relative, intersection types exactly, the
oracle's sampled points on Arris's curves to 1e-9.

```json
{
  "kind": "geometry",
  "description": "...",
  "params": {"R": 2},
  "surfaces": {
    "wall": {"type": "cylinder", "origin": [0, 0, 0], "z": [0, 0, 1], "x": [1, 0, 0], "radius": "R"},
    "cap":  {"type": "plane", "origin": [0, 0, 5], "z": [0, 0, 1], "x": [1, 0, 0]}
  },
  "curves": {
    "ring": {"type": "circle", "origin": [0, 0, 0], "z": [0, 1, 0], "x": [1, 0, 0], "radius": 3}
  },
  "samples": [
    {"of": "wall", "params": [[0, 0], ["pi / 2", 3]], "points": [[0, -5, 3]]},
    {"of": "ring", "params": ["pi"], "points": [[-4, 0, 0]]}
  ],
  "pairs": [{"a": "cap", "b": "wall"}, {"a": "ring", "b": "wall"}]
}
```

| `type` | Fields |
|---|---|
| `plane` | `origin`, `z`, `x` |
| `cylinder`, `sphere` | `origin`, `z`, `x`, `radius` |
| `cone` | `origin`, `z`, `x`, `radius` (at `v = 0`), `half_angle_deg` |
| `torus` | `origin`, `z`, `x`, `major_radius`, `minor_radius` |
| `line` | `origin`, `direction` |
| `circle` | `origin`, `z`, `x`, `radius` |
| `ellipse` | `origin`, `z`, `x` (the major axis), `major_radius`, `minor_radius` |

- A frame is `origin`, `z`, `x` as `Frame::new` and `gp_Ax3` build it: `x`
  made perpendicular to `z`, both normalised, `y = z × x`. Numbers may be
  expressions as in a solid recipe.
- A sample's `params` are `[u, v]` pairs for a surface and `t` values for
  a curve; `points` are projected. A pair is two surfaces, or a curve `a`
  against a surface `b`.
- The fixtures here are written by `geom/generate.py` (closed forms at
  full precision in committed poses) — edit and rerun it, then
  `expected.py`, rather than the numbers: `analytic-eval`,
  `c1-intersections` (the plane and plane–cylinder table),
  `c2-cylinder-pairs` (every pose of the cylinder–cylinder table) and
  `c2-quadric-pairs` (the coaxial pairs with a cone, a sphere or a torus
  in them, and the pairs any sphere makes: crossings, touches, a plane
  through an apex, two spheres touching).
- A surface pair Open CASCADE finds no conic for is `"type": "unsolved"`
  (`IntAna_NoGeometricSolution`). The oracle test holds Arris to
  `Unsupported` or to a closed-form `Empty` against it — or, where Arris
  has a closed form the oracle's own case analysis let go, holds Arris's
  curves to both surfaces — and `oracle.rs` pins which one each pair is
  by name. A pair meeting in isolated points is `"type": "point"` with
  its `points`; Arris's `Points` are held to them exactly.
- The corpus lint checks presence, hash and shape for this kind (every
  name resolves, every spec builds, one result per sample and pair with
  the counts asked for); the values are the oracle test's to compare.
- What the oracle cannot answer stably is recorded as it is: the
  conic–quadric intersector has no tolerance, so a curve built exactly
  tangent to a cylinder comes back as none, one or two points by rounding,
  and Arris's single `tangent` hit is compared against whatever it
  reported to 1e-6 (a touch is conditioned as the square root of the
  rounding); a line parallel to the axis yields a hit at `t ≈ 1e16` that
  the oracle drops as off both operands and counts under `dropped`. Two
  parallel cylinders touching come back as one ruling or as two about
  1e-7 apart, by the same rounding (`c2-cylinder-pairs`' inside touch is
  two), and each is compared against Arris's single `Tangent` ruling to
  1e-6. The
  cylinder–cylinder intersector has no case for skew axes: it answers
  `unsolved` for unequal radii whether or not the cylinders can meet,
  and two ellipses for equal radii whatever the gap between the axes. So
  `c2-cylinder-pairs` gives its skew pairs unequal radii, and Arris's
  `Empty` for the pair further apart than the radii stands against
  `unsolved`. The plane–sphere case decides a touch at machine epsilon,
  so a plane built tangent at a pole comes back `empty` (or a circle of
  rounding radius) where Arris says `Points`, which the oracle test
  accepts within 1e-6 of Arris's point; the cylinder–sphere case wants
  the sphere's own axis on the cylinder's exactly, so `c2-quadric-pairs`
  has the same sphere twice — its frame turned across the axis, which
  comes back `unsolved` and holds Arris's circles to both surfaces, and
  along the axis, which is answered.

## `expected.json`

Layout in `tools/oracle/README.md`. Regenerate with

```sh
uv run --project tools/oracle tools/oracle/expected.py tests/fixtures/<area>/<slug>
```

and commit the change as `fixtures: …` saying why the numbers moved
(`.agents/rules/git.md`). The hash covers `params`, `variants`, `steps`,
`result` and `probes` of a solid, and `kind`, `params`, `surfaces`,
`curves`, `samples` and `pairs` of a geometry fixture; a change to any of
them makes the old `expected.json` stale and the lint says so. Both sides
parse a coordinate to the same `f64` (serde_json's `float_roundtrip`), so
a full-precision number hashes and evaluates identically.

## Property-test failures

Property tests run through `arris_debug::prop::check`: a seeded
`proptest` runner, `ARRIS_PROPTEST_CASES` cases (default 256) from
`ARRIS_PROPTEST_SEED` (default fixed, so CI is deterministic). A failure
prints the shrunk input and the `ARRIS_PROPTEST_SEED=…` that reproduces
it.

An expensive property is split across shards by `prop_shards!` and runs as
`property::shard_3`. Its failure names the shard, but the seed it prints is
still the *base* seed and the total case count — that recipe reproduces the
whole run, and the shard alone is re-run by its test name. The fixture a
sharded failure becomes records all three in its commit body: the base
seed, the case count and the shard. proptest's own `proptest-regressions/` files are gitignored and never
committed: the regression is a fixture under `regression/` (or a
hand-picked test beside the property) with the *desired* assertion,
`#[ignore]`d until it passes, and the seed and case count in the commit
body — so the original case stays reproducible after the shrinker or the
strategy changes (`.agents/rules/kernel.md` §Testing, `inspect` skill).

## Conventions the numbers assume

Open CASCADE's, which Arris matches (`docs/DATA-MODEL.md`
§Conventions): a full revolve or a cylinder has one seam edge, counted
once; a degenerate edge (a cone's apex, a sphere's pole) is not counted
at all, on either side, so the Euler line closes on its genus; a fuse of flush boxes drops the shared face and does not merge the
coplanar neighbours; a common with no volume is degenerate.
