"""Fixture directories on disk."""

import json
from pathlib import Path

from . import OracleError, occt_version
from .measure import DEFAULT_TOLERANCES, measure, own_measures
from .geometry import compute_geometry
from .recipe import build, fixture_kind, probes, recipe_hash, variant_names

FIXTURE = "fixture.json"
EXPECTED = "expected.json"


def repo_root() -> Path:
    return Path(__file__).resolve().parents[3]


def corpus_root() -> Path:
    return repo_root() / "tests" / "fixtures"


def load_fixture(directory: Path) -> dict:
    path = directory / FIXTURE
    if not path.exists():
        raise OracleError(f"no {FIXTURE} in {directory}")
    with path.open() as f:
        return json.load(f)


def load_expected(directory: Path) -> dict | None:
    path = directory / EXPECTED
    if not path.exists():
        return None
    with path.open() as f:
        return json.load(f)


def fixture_dirs(root: Path | None = None) -> list[Path]:
    root = root or corpus_root()
    return sorted(p.parent for p in root.glob(f"**/{FIXTURE}"))


def compute_expected(fixture: dict, own: bool = False) -> dict:
    """The expected.json content for a recipe: one result per variant for
    the solid kind, the samples and pairs for the geometry kind. `own`
    adds each solid result's `own_measures` under `"own"`: what the
    differential bounds its comparison by, never written for a corpus
    fixture."""
    if fixture_kind(fixture) == "geometry":
        return {"occt": occt_version(), "recipe_sha256": recipe_hash(fixture), "kind": "geometry", **compute_geometry(fixture)}
    tol = {**DEFAULT_TOLERANCES, **fixture.get("tolerances", {})}
    # A result Arris refuses as non-manifold is Open CASCADE's compound of
    # solids sharing an edge or a vertex, and one it refuses as a tangent
    # contact carries the contact as an edge of four faces; either may have
    # an odd Euler characteristic.
    manifold = fixture.get("analytic", {}).get("expect_error") not in ("non-manifold", "tangent-contact")
    results = {}
    for variant in variant_names(fixture):
        shape, _ = build(fixture, variant)
        results[variant] = measure(shape, probes(fixture, variant), tol["probe"], manifold)
        if own and (measures := own_measures(shape, results[variant])) is not None:
            results[variant]["own"] = measures
    return {"occt": occt_version(), "recipe_sha256": recipe_hash(fixture), "results": results}


def dump_expected(expected: dict, path: Path) -> None:
    # sort_keys and a trailing newline: the file diffs cleanly; floats are
    # repr'd by json, full precision.
    path.write_text(json.dumps(expected, indent=2, sort_keys=True) + "\n")


def summary_lines(name: str, expected: dict) -> list[str]:
    """One line per result: per variant for a solid, one for a geometry."""
    if expected.get("kind") == "geometry":
        evals = sum(len(s["evaluations"]) for s in expected["samples"])
        projs = sum(len(s["projections"]) for s in expected["samples"])
        return [f"{name}: geometry, {evals} evaluations, {projs} projections, {len(expected['pairs'])} pairs"]
    return [summary_line(f"{name}[{variant}]", result) for variant, result in expected["results"].items()]


def summary_line(name: str, result: dict) -> str:
    c = result["counts"]
    if result["degenerate"]:
        return f"{name}: degenerate (no solid), counts V/E/F/L={c['vertices']}/{c['edges']}/{c['faces']}/{c['loops']}"
    return (
        f"{name}: volume {result['volume']:.10g}, area {result['area']:.10g}, "
        f"V/E/F/L/S={c['vertices']}/{c['edges']}/{c['faces']}/{c['loops']}/{c['shells']}, "
        f"genus {result['genus']}, "
        + ", ".join(f"{p['label']}={p['class']}" for p in result["probes"])
    )
