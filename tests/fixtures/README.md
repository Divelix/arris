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
| `dump.txt` | Arris, through the corpus runner under `ARRIS_BLESS=1`, once the fixture passes | the text dump of the result, the regression guard for ids and provenance (absent while the fixture is `#[ignore]`d); `dump.<variant>.txt` for a variant other than `default` |

The Rust reading of both files is `arris_debug::fixtures`; the Python one
is `tools/oracle/oracle/recipe.py` and `fixture.py`. The corpus lint
(`crates/arris/tests/corpus_lint.rs`, run by `cargo test`) checks every
directory: both files present and parseable, `expected.json` not stale, the
Euler line zero, every `analytic` value matching the oracle to 1e-6
relative, counts and probe expectations exactly. The oracle's own self-test
(`uv run --project tools/oracle tools/oracle/selftest.py`) reproduces every
committed `expected.json` and round-trips each result through STEP.

The corpus **runner** (`arris_debug::corpus::run(dir, variant)`, one
`#[test]` per fixture in `crates/arris/tests/corpus.rs`) is the fixture
test itself: it builds the recipe in Arris, runs the checker at `Full`
(nothing violated, nothing undecided), compares counts and genus against
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
ends the run there, nothing later compared, no `dump.txt`. A fixture
whose recipe needs an operation of a later step or milestone is
`#[ignore = "M4: needs ops::fuse (step 8)"]` and fails naming the op
under `--include-ignored`, so the day the operation lands the test says
so. `ARRIS_BLESS=1 cargo test -p arris --test corpus
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
  `pi`, and `sin cos tan sqrt radians degrees abs`. Both sides evaluate the
  same grammar.
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

- **`analytic`** is the author's closed form, the cross-check that catches a
  convention mismatch on either side (profile orientation, seam counting,
  which faces a fuse keeps). Every field is optional; `degenerate: true`
  says the result has no volume (`boolean/flush-common`,
  `boolean/swallow-cut`, `boolean/disjoint-common`), and then nothing
  else is compared.
  `expect_error: "multi-shell" | "tangent-contact"` says Open CASCADE
  builds a result Arris refuses by design (`boolean/split-cut`: two
  solids, `Reason::MultiShell`; the tangent cases, `Reason::TangentContact`
  — `docs/plans/m4-booleans.md` `⚠ OPEN` 1 and 2): the oracle's numbers
  are recorded and the lint still cross-checks them against the other
  `analytic` values, but the runner asserts the typed error and compares
  nothing. `genus` is what the Euler line is checked with:
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
- The two fixtures here are written by `geom/generate.py` (closed forms at
  full precision in committed poses) — edit and rerun it, then
  `expected.py`, rather than the numbers.
- The corpus lint checks presence, hash and shape for this kind (every
  name resolves, every spec builds, one result per sample and pair with
  the counts asked for); the values are the oracle test's to compare.
- What the oracle cannot answer stably is recorded as it is: the
  conic–quadric intersector has no tolerance, so a curve built exactly
  tangent to a cylinder comes back as none, one or two points by rounding,
  and Arris's single `tangent` hit is compared against whatever it
  reported to 1e-6 (a touch is conditioned as the square root of the
  rounding); a line parallel to the axis yields a hit at `t ≈ 1e16` that
  the oracle drops as off both operands and counts under `dropped`.

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
it. proptest's own `proptest-regressions/` files are gitignored and never
committed: the regression is a fixture under this directory (or a
hand-picked test beside the property) with the *desired* assertion,
`#[ignore]`d until it passes, and the seed and case count in the commit
body — so the original case stays reproducible after the shrinker or the
strategy changes (`.agents/rules/kernel.md` §Testing, `inspect` skill).

## Conventions the numbers assume

Open CASCADE's, which Arris matches (`docs/DATA-MODEL.md`
§Conventions): a full revolve or a cylinder has one seam edge, counted
once; a fuse of flush boxes drops the shared face and does not merge the
coplanar neighbours; a common with no volume is degenerate.
