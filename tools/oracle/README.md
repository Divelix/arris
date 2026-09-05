# The Open CASCADE oracle

Ground truth for the fixture corpus (`SEED.md` §7, `docs/03-roadmap.md`
§Fixtures). Open CASCADE is **run** here through the `cadquery-ocp` wheels;
no crate links it. Every number in a `tests/fixtures/**/expected.json` was
written by `expected.py` in this directory, never by hand.

## Environment

A [`uv`](https://docs.astral.sh/uv/) project pinned to Python 3.12 and
`cadquery-ocp==8.0.1.*` (`pyproject.toml`, `uv.lock`). The venv is created
on first use and lives in `tools/oracle/.venv` (gitignored):

```sh
uv sync --project tools/oracle          # once; downloads the OCCT wheel
uv run --project tools/oracle tools/oracle/selftest.py
```

Running a script outside the environment fails on the first line with the
command above, never with a silent skip. The OCCT version is recorded in
every `expected.json`; a pin change that moves a number is a `fixtures:`
commit that says so (`.agents/rules/git.md`).

## Scripts

| Script | Does |
|---|---|
| `expected.py <fixture-dir>...` | Builds each recipe (every variant), measures it, writes `expected.json`, prints one summary line per result |
| `compare.py <fixture-dir> <file.step> [--variant NAME]` | Reads a STEP file (Arris's output), measures it, compares against `expected.json` within the fixture's tolerances, prints a table; exit 1 on mismatch, 2 on a stale `expected.json` or an environment error |
| `selftest.py [fixture-dir...]` | Inline smoke recipes covering every op against closed forms, then for each fixture: a fresh `expected` must equal the committed one, and OCCT's own STEP of the result must compare clean |

## Package

- `oracle/recipe.py` — the recipe interpreter: `box`, `cylinder`, `profile`
  (lines, three-point arcs, circles, holes), `extrude`, `revolve`,
  `transform`, `fuse`, `common`, `cut`, chained by step name; `params`
  with string expressions and `variants` overriding them. The grammar is
  the module docstring and `tests/fixtures/README.md`.
- `oracle/measure.py` — volume, area, centroid (`GProp`), counts by unique
  sub-shape (a seam edge once), loops, shells, solids, the Euler
  characteristic `V − E + 2F − L` and the genus it implies, in/out/on
  classification of probe points (`BRepClass3d`), and the comparison with
  its tolerances.
- `oracle/step.py` — STEP AP214 write and read, with OCCT's transfer
  banner silenced.
- `oracle/fixture.py` — fixture directories, `expected.json` layout,
  the recipe hash.

## `expected.json`

```json
{
  "occt": "8.0.1.0.0",
  "recipe_sha256": "<hash of params, variants, steps, result, probes>",
  "results": {
    "default": {
      "degenerate": false,
      "counts": {"vertices": 10, "edges": 15, "faces": 7, "loops": 9, "shells": 1, "solids": 1},
      "volume": 11497.345175425633, "area": 3950.796447372311, "centroid": [20.0, 15.0, 5.0],
      "euler_characteristic": 0, "genus": 1,
      "probes": [{"label": "inside", "point": [5, 5, 5], "class": "in"}]
    }
  }
}
```

The hash covers only what the oracle evaluates, so editing a fixture's
`analytic` or description does not stale it; editing a step does, and
`compare.py` refuses to run until `expected.py` is rerun. A degenerate
result (no solid — the common of two flush boxes) has counts only.

## Conventions the interpreter mirrors

Open CASCADE's, which Arris follows too (`docs/02-data-model.md`
§Conventions): a full revolve and a cylinder have one seam edge; a fuse of
flush boxes drops the shared face and keeps coplanar neighbours unmerged;
a common with no volume is an empty result. A fixture's `analytic` block
is the cross-check that catches a convention mismatch on either side.
