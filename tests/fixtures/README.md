# The fixture corpus

The unit of acceptance (`docs/03-roadmap.md` §Fixtures). One directory per
fixture, `<area>/<slug>/`, holding:

| File | Written by | Holds |
|---|---|---|
| `fixture.json` | a person or the agent | the **recipe**: operands and operations both sides evaluate, probe points, tolerances, and the closed-form `analytic` values |
| `expected.json` | `tools/oracle/expected.py`, never by hand | the **oracle's answer** per variant: volume, area, centroid, counts, Euler characteristic and genus, probe classifications, plus the OCCT version and the recipe hash |
| `dump.txt` | Arris, once the fixture passes | the text dump of the result, the regression guard for ids and provenance (absent while the fixture is `#[ignore]`d) |

The Rust reading of both files is `arris_debug::fixtures`; the Python one
is `tools/oracle/oracle/recipe.py` and `fixture.py`. The corpus lint
(`crates/arris/tests/corpus_lint.rs`, run by `cargo test`) checks every
directory: both files present and parseable, `expected.json` not stale, the
Euler line zero, every `analytic` value matching the oracle to 1e-6
relative, counts and probe expectations exactly. The oracle's own self-test
(`uv run --project tools/oracle tools/oracle/selftest.py`) reproduces every
committed `expected.json` and round-trips each result through STEP.

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
  says the result has no volume (`boolean/flush-common`), and then nothing
  else is compared. `genus` is what the Euler line is checked with:
  `V − E + F − (L − F) − 2(S − G) = 0` with the oracle's counts.
- **Tolerances** are the fixture's; absent ones take the defaults shown.
  Counts and classifications are always exact.

## `expected.json`

Layout in `tools/oracle/README.md`. Regenerate with

```sh
uv run --project tools/oracle tools/oracle/expected.py tests/fixtures/<area>/<slug>
```

and commit the change as `fixtures: …` saying why the numbers moved
(`.agents/rules/git.md`). The hash covers `params`, `variants`, `steps`,
`result` and `probes`; a change to any of them makes the old
`expected.json` stale and the lint says so.

## Conventions the numbers assume

Open CASCADE's, which Arris matches (`docs/02-data-model.md`
§Conventions): a full revolve or a cylinder has one seam edge, counted
once; a fuse of flush boxes drops the shared face and does not merge the
coplanar neighbours; a common with no volume is degenerate.
