# The Open CASCADE oracle

Ground truth for the fixture corpus (`SEED.md` §7, `docs/ROADMAP.md`
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
| `expected.py <fixture-dir>...` | Builds each recipe (every variant), measures it, writes `expected.json`, prints one summary line per result; for a geometry fixture, evaluates, projects and intersects instead |
| `compare.py <fixture-dir> <file.step> [--variant NAME]` | Reads a STEP file (Arris's output), measures it, compares against `expected.json` within the fixture's tolerances, prints a table; exit 1 on mismatch, 2 on a stale `expected.json` or an environment error. Solid fixtures only: a geometry fixture is compared by `crates/arris-geom/tests/oracle.rs`. `arris_debug::oracle::compare` is the Rust seam to it, and the corpus runner (`arris_debug::corpus::run`) calls it on every fixture |
| `selftest.py [fixture-dir...]` | `tests/fixtures/expr-cases.json`'s expression grammar cases (the same ones `arris_debug::fixtures::expr`'s own test evaluates); inline smoke recipes covering every op and the geometry kind against closed forms; then for each fixture: a fresh `expected` must equal the committed one, and for a solid OCCT's own STEP of the result must compare clean |

## Package

- `oracle/recipe.py` — the recipe interpreter: `box`, `cylinder`, `profile`
  (lines, three-point arcs, circles, holes), `extrude`, `revolve`,
  `transform`, `fuse`, `common`, `cut`, `fillet` and `chamfer`
  (`BRepFilletAPI_MakeFillet`, `MakeChamfer` with one distance, each edge
  the nearest to a recipe point by `BRepExtrema`, which must be
  the only edge within the fixture's `probe`), chained by step name; `params`
  with string expressions and `variants` overriding them. The grammar is
  the module docstring and `tests/fixtures/README.md`.
- `oracle/measure.py` — volume, area, centroid and the inertia tensor
  (`GProp`), counts by unique
  sub-shape (a seam edge once, an edge `BRep_Tool::Degenerated` names not
  at all), loops, shells, solids — of the boundary only: an edge oriented
  INTERNAL or EXTERNAL, a wire of nothing else and a vertex on nothing
  else are left out, as a STEP round trip leaves them — the Euler
  characteristic `V − E + 2F − L` and the genus it implies, in/out/on
  classification of probe points (`BRepClass3d`), and the comparison with
  its tolerances.
- `oracle/geometry.py` — the geometry kind: `Geom_Plane`,
  `Geom_CylindricalSurface`, `Geom_ConicalSurface`,
  `Geom_SphericalSurface`, `Geom_ToroidalSurface`, `Geom_Line`,
  `Geom_Circle` and `Geom_Ellipse` from named specs; `D2` at every
  parameter; `GeomAPI_ProjectPointOnSurf` / `OnCurve` for every point;
  `IntAna_QuadQuadGeo` for surface pairs (`unsolved` where it reports
  `NoGeometricSolution`) and `IntAna_IntConicQuad` for a
  curve against a surface, hits deduplicated within `Precision::Confusion`
  and dropped (counted) when off either operand.
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
      "inertia": [[993800.5904969159, -1.3969838619232178e-09, -4.656612873077393e-10],
                  [-1.3969838619232178e-09, 1693800.5904969191, -2.3283064365386963e-10],
                  [-4.656612873077393e-10, -2.3283064365386963e-10, 2495978.761403409]],
      "euler_characteristic": 0, "genus": 1,
      "probes": [{"label": "inside", "point": [5, 5, 5], "class": "in"}]
    }
  }
}
```

`inertia` is OCCT's `GProp_GProps::MatrixOfInertia` of the volume
properties: the 3×3 tensor about the centre of mass at unit density, in
the physical convention — the diagonal holds the moments of inertia and
the off-diagonal the *negated* products — which is what
`arris_ops::measure::MassProperties` states and the runner's `measure`
stage compares within the fixture's `inertia_rel`.

The hash covers only what the oracle evaluates, so editing a fixture's
`analytic` or description does not stale it; editing a step does, and
`compare.py` refuses to run until `expected.py` is rerun. A degenerate
result (no solid — the common of two flush boxes) has counts only.

A geometry fixture's `expected.json` has `"kind": "geometry"` and, instead
of `results`, one entry per recipe sample and pair:

```json
{
  "occt": "8.0.1.0.0", "recipe_sha256": "…", "kind": "geometry",
  "samples": [
    {"of": "wall",
     "evaluations": [{"at": [0.0, 0.0], "point": [2.0, 0.0, 0.0], "du": […], "dv": […], "duu": […], "duv": […], "dvv": […]}],
     "projections": [{"point": [0.0, -5.0, 3.0], "uv": [4.71238898038469, 3.0], "nearest": [0.0, -2.0, 3.0], "distance": 3.0}]},
    {"of": "ring",
     "evaluations": [{"at": 3.141592653589793, "point": […], "d1": […], "d2": […]}],
     "projections": [{"point": [-4.0, 0.0, 0.0], "t": 3.141592653589793, "nearest": […], "distance": 1.0}]}
  ],
  "pairs": [
    {"a": "cap", "b": "wall", "type": "circle", "curves": [{"type": "circle", "points": [[…], …]}]},
    {"a": "ring", "b": "wall", "type": "points", "hits": [{"point": […], "t": 0.8410686705679302}, …]}
  ]
}
```

A surface pair's `type` is `empty`, `coincident`, `line`, `circle` or
`ellipse`, with every result curve sampled (five points along a line,
eight around a closed curve); a curve–surface pair's is `coincident` or
`points`, hits ascending by the conic parameter, plus `dropped` when the
intersector reported a point that lies on neither operand.

## Conventions the interpreter mirrors

Open CASCADE's, which Arris follows too (`docs/DATA-MODEL.md`
§Conventions): a full revolve and a cylinder have one seam edge; a fuse of
flush boxes drops the shared face and keeps coplanar neighbours unmerged;
a common with no volume is an empty result. A fixture's `analytic` block
is the cross-check that catches a convention mismatch on either side.
