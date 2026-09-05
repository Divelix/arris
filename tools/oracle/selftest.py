#!/usr/bin/env python3
"""selftest.py [fixture-dir...]  — prove the oracle agrees with itself.

For an inline smoke recipe and then for every fixture directory (default:
all under tests/fixtures/): build the recipe, compute its expected values,
check they equal the committed expected.json (if any), write Open CASCADE's
own STEP of the result to a temporary file and run the comparison on it.
Exits 1 on the first disagreement. Run as
`uv run --project tools/oracle tools/oracle/selftest.py`.
"""

import json
import sys
import tempfile
from pathlib import Path

from oracle import OracleError, require_ocp

require_ocp()

from oracle import step  # noqa: E402
from oracle.fixture import compute_expected, fixture_dirs, load_expected, load_fixture  # noqa: E402
from oracle.measure import DEFAULT_TOLERANCES, compare, format_table, measure  # noqa: E402
from oracle.recipe import build, variant_names  # noqa: E402

PI = 3.141592653589793

# Inline recipes exercising every op the interpreter has, each with the
# closed form it must reproduce. The fixture corpus (tests/fixtures/) holds
# the real ones; these run even on a checkout with no corpus yet.
SMOKES = [
    {
        "name": "box − cylinder, params and a variant",
        "recipe": {
            "params": {"r": 4},
            "variants": {"wider": {"r": 5}},
            "steps": [
                {"name": "plate", "op": "box", "min": [0, 0, 0], "max": [40, 30, 10]},
                {"name": "hole", "op": "cylinder", "base": [20, 15, -1], "axis": [0, 0, 1], "radius": "r", "height": 12},
                {"name": "result", "op": "cut", "target": "plate", "tool": "hole"},
            ],
            "result": "result",
            "probes": [
                {"label": "inside", "point": [5, 5, 5]},
                {"label": "in_hole", "point": [20, 15, 5]},
                {"label": "on_top", "point": [5, 5, 10]},
            ],
        },
        "analytic": {
            "default": {"volume": 12000 - PI * 16 * 10, "area": 3800 - 2 * PI * 16 + 2 * PI * 4 * 10, "counts": (10, 15, 7, 9), "genus": 1},
            "wider": {"volume": 12000 - PI * 25 * 10, "genus": 1},
        },
        "probes": {"inside": "in", "in_hole": "out", "on_top": "on"},
    },
    {
        "name": "profile with a hole, extruded (same numbers as the cut)",
        "recipe": {
            "steps": [
                {
                    "name": "sketch",
                    "op": "profile",
                    "plane": {"origin": [0, 0, 0], "x": [1, 0, 0], "y": [0, 1, 0]},
                    "outer": {"start": [0, 0], "segments": [{"line_to": [40, 0]}, {"line_to": [40, 30]}, {"line_to": [0, 30]}, {"line_to": [0, 0]}]},
                    "holes": [{"circle": {"center": [20, 15], "radius": 4}}],
                },
                {"name": "result", "op": "extrude", "profile": "sketch", "direction": [0, 0, 2], "length": 10},
            ],
            "result": "result",
        },
        "analytic": {"default": {"volume": 12000 - PI * 16 * 10, "area": 3800 - 2 * PI * 16 + 2 * PI * 4 * 10, "counts": (10, 15, 7, 9), "genus": 1}},
    },
    {
        "name": "rectangle revolved into a tube, then a quarter",
        "recipe": {
            "params": {"angle": 360},
            "variants": {"quarter": {"angle": 90}},
            "steps": [
                {
                    "name": "rect",
                    "op": "profile",
                    "plane": {"origin": [0, 0, 0], "x": [1, 0, 0], "y": [0, 0, 1]},
                    "outer": {"start": [1, -1], "segments": [{"line_to": [2, -1]}, {"line_to": [2, 1]}, {"line_to": [1, 1]}, {"line_to": [1, -1]}]},
                },
                {"name": "result", "op": "revolve", "profile": "rect", "axis": {"origin": [0, 0, 0], "direction": [0, 0, 1]}, "angle_deg": "angle"},
            ],
            "result": "result",
        },
        "analytic": {
            "default": {"volume": 2 * PI * (4 - 1) / 2 * 2, "area": 2 * PI * (1 + 2) * 2 + 2 * PI * (4 - 1), "counts": (4, 6, 4, 6), "genus": 1},
            "quarter": {"volume": (2 * PI * (4 - 1) / 2 * 2) / 4, "area": (2 * PI * (1 + 2) * 2 + 2 * PI * (4 - 1)) / 4 + 2 * 2, "counts": (8, 12, 6, 6), "genus": 0},
        },
    },
    {
        "name": "arc profile: half disc extruded",
        "recipe": {
            "steps": [
                {
                    "name": "half",
                    "op": "profile",
                    "plane": {"origin": [0, 0, 0], "x": [1, 0, 0], "y": [0, 1, 0]},
                    "outer": {"start": [-2, 0], "segments": [{"line_to": [2, 0]}, {"arc_to": [-2, 0], "via": [0, 2]}]},
                },
                {"name": "result", "op": "extrude", "profile": "half", "direction": [0, 0, 1], "length": 3},
            ],
            "result": "result",
        },
        "analytic": {"default": {"volume": PI * 4 / 2 * 3, "area": PI * 4 + (PI * 2 + 4) * 3, "counts": (4, 6, 4, 4), "genus": 0}},
    },
    {
        "name": "transform then fuse, common and cut of two corner cubes",
        "recipe": {
            "params": {"which": 0},
            "steps": [
                {"name": "a", "op": "box", "min": [-1, -1, -1], "max": [1, 1, 1]},
                {"name": "b0", "op": "box", "min": [-1, -1, -1], "max": [1, 1, 1]},
                {"name": "b", "op": "transform", "of": "b0", "translate": [1, 1, 1], "rotate": {"axis": [0, 0, 1], "angle_deg": 90}},
                {"name": "result", "op": "fuse", "a": "a", "b": "b"},
                {"name": "inter", "op": "common", "a": "a", "b": "b"},
                {"name": "diff", "op": "cut", "target": "a", "tool": "b"},
            ],
            "result": "result",
        },
        "analytic": {"default": {"volume": 15.0, "area": 42.0, "counts": (20, 30, 12, 12), "genus": 0}},
        "others": {"inter": {"volume": 1.0, "area": 6.0, "counts": (8, 12, 6, 6)}, "diff": {"volume": 7.0, "area": 24.0, "counts": (14, 21, 9, 9)}},
    },
    {
        "name": "flush union keeps coplanar neighbours; flush common is degenerate",
        "recipe": {
            "steps": [
                {"name": "a", "op": "box", "min": [0, 0, 0], "max": [40, 30, 10]},
                {"name": "b", "op": "box", "min": [40, 0, 0], "max": [80, 30, 10]},
                {"name": "result", "op": "fuse", "a": "a", "b": "b"},
                {"name": "flush", "op": "common", "a": "a", "b": "b"},
            ],
            "result": "result",
        },
        "analytic": {"default": {"volume": 24000.0, "area": 7000.0, "counts": (12, 20, 10, 10), "genus": 0}},
        "degenerate": ["flush"],
    },
]


def check_analytic(name: str, result: dict, analytic: dict) -> bool:
    ok = True
    for key in ("volume", "area"):
        if key in analytic:
            e, a = analytic[key], result[key]
            if abs(a - e) > 1e-9 * abs(e):
                print(f"  {name}: {key} {a!r} differs from analytic {e!r}")
                ok = False
    if "counts" in analytic:
        c = result["counts"]
        got = (c["vertices"], c["edges"], c["faces"], c["loops"])
        if got != tuple(analytic["counts"]):
            print(f"  {name}: counts V/E/F/L {got} differ from analytic {analytic['counts']}")
            ok = False
    if "genus" in analytic and result["genus"] != analytic["genus"]:
        print(f"  {name}: genus {result['genus']} differs from analytic {analytic['genus']}")
        ok = False
    return ok


def run_smokes(tmp: Path) -> bool:
    ok = True
    for smoke in SMOKES:
        recipe = smoke["recipe"]
        print(f"smoke: {smoke['name']}")
        expected = compute_expected(recipe)
        for variant, analytic in smoke["analytic"].items():
            ok &= check_analytic(f"[{variant}]", expected["results"][variant], analytic)
        if "probes" in smoke:
            probes = {p["label"]: p["class"] for p in expected["results"]["default"]["probes"]}
            if probes != smoke["probes"]:
                print(f"  probes {probes} differ from {smoke['probes']}")
                ok = False
        _, shapes = build(recipe)
        for step_name, analytic in smoke.get("others", {}).items():
            ok &= check_analytic(step_name, measure(shapes[step_name], [], DEFAULT_TOLERANCES["probe"]), analytic)
        for step_name in smoke.get("degenerate", []):
            if not measure(shapes[step_name], [], DEFAULT_TOLERANCES["probe"])["degenerate"]:
                print(f"  {step_name}: expected a degenerate (no-solid) result")
                ok = False
        ok &= round_trip("smoke", recipe, expected, tmp)
    return ok


def round_trip(name: str, fixture: dict, expected: dict, tmp: Path) -> bool:
    tol = {**DEFAULT_TOLERANCES, **fixture.get("tolerances", {})}
    ok_all = True
    for variant in variant_names(fixture):
        shape, _ = build(fixture, variant)
        if expected["results"][variant]["degenerate"]:
            print(f"  {name}[{variant}]: degenerate, no STEP round trip")
            continue
        path = tmp / f"{name.replace('/', '_')}-{variant}.step"
        step.write(shape, path)
        back = step.read(path)
        actual = measure(back, fixture.get("probes", []), tol["probe"])
        rows = compare(expected["results"][variant], actual, tol)
        ok = all(r[3] for r in rows)
        ok_all &= ok
        print(f"  {name}[{variant}]: STEP round trip {'ok' if ok else 'MISMATCH'}")
        if not ok:
            print(format_table(rows))
    return ok_all


def check_committed(name: str, fresh: dict, committed: dict | None) -> bool:
    if committed is None:
        print(f"  {name}: no expected.json committed yet")
        return True
    a = json.dumps(fresh, sort_keys=True)
    b = json.dumps(committed, sort_keys=True)
    if a == b:
        print(f"  {name}: expected.json matches a fresh run")
        return True
    print(f"  {name}: expected.json DIFFERS from a fresh run (rerun expected.py, commit as fixtures:)")
    return False


def main(argv: list[str]) -> int:
    dirs = [Path(a) for a in argv] if argv else fixture_dirs()
    ok = True
    with tempfile.TemporaryDirectory(prefix="arris-oracle-") as tmpdir:
        tmp = Path(tmpdir)
        ok &= run_smokes(tmp)
        print(f"{len(dirs)} fixture director{'y' if len(dirs) == 1 else 'ies'}:")
        for directory in dirs:
            name = str(directory.relative_to(directory.parents[1])) if len(directory.parents) > 1 else str(directory)
            try:
                fixture = load_fixture(directory)
                fresh = compute_expected(fixture)
                ok &= check_committed(name, fresh, load_expected(directory))
                ok &= round_trip(name, fixture, fresh, tmp)
            except OracleError as e:
                print(f"  {name}: ERROR {e}")
                ok = False
    print("selftest: " + ("ok" if ok else "FAILED"))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
