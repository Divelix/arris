#!/usr/bin/env python3
"""occt_step.py <fixture-dir> <out.step> [--variant NAME] [--nurbs]  —
build the fixture's recipe in Open CASCADE and write the result as Open
CASCADE's own STEP: the file the STEP reader is held to (ADR-0025).
`--nurbs` first passes the result through
`BRepBuilderAPI_NurbsConvert`, so every face is a B-spline surface and
every edge a B-spline curve. Exits 0 with the file written, 2 on a usage
or environment error or a recipe the oracle cannot build. Run as
`uv run --project tools/oracle tools/oracle/occt_step.py`.
"""

import sys
from pathlib import Path

from oracle import OracleError, require_ocp

require_ocp()

from oracle import step  # noqa: E402
from oracle.fixture import load_fixture  # noqa: E402
from oracle.recipe import build, fixture_kind  # noqa: E402


def write(directory: Path, out: Path, variant: str = "default", nurbs: bool = False) -> None:
    """Writes the result of `directory`'s recipe under `variant` to `out`."""
    fixture = load_fixture(directory)
    if fixture_kind(fixture) in ("geometry", "part"):
        raise OracleError(f"{directory} is a {fixture_kind(fixture)} fixture: it has no recipe result to write")
    shape, _ = build(fixture, variant)
    if nurbs:
        shape = step.nurbs(shape)
    out.parent.mkdir(parents=True, exist_ok=True)
    step.write(shape, out)


def main(argv: list[str]) -> int:
    variant = "default"
    if "--variant" in argv:
        i = argv.index("--variant")
        variant = argv[i + 1]
        argv = argv[:i] + argv[i + 2 :]
    nurbs = "--nurbs" in argv
    argv = [a for a in argv if a != "--nurbs"]
    if len(argv) != 2:
        print(__doc__)
        return 2
    try:
        write(Path(argv[0]), Path(argv[1]), variant, nurbs)
    except Exception as e:  # OracleError, or Open CASCADE's own on a recipe it cannot build
        message = " ".join(str(e).split()) or type(e).__name__
        print(f"occt_step: ERROR {message}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
