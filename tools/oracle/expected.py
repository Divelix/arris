#!/usr/bin/env python3
"""expected.py <fixture-dir>...  — build each recipe in Open CASCADE and
write its expected.json (all variants), printing one summary line per
result. Run as `uv run --project tools/oracle tools/oracle/expected.py`.
"""

import sys
from pathlib import Path

from oracle import OracleError, require_ocp

require_ocp()

from oracle.fixture import EXPECTED, compute_expected, dump_expected, load_fixture, summary_line  # noqa: E402


def main(argv: list[str]) -> int:
    if not argv:
        print(__doc__)
        return 2
    for arg in argv:
        directory = Path(arg)
        try:
            fixture = load_fixture(directory)
            expected = compute_expected(fixture)
        except OracleError as e:
            print(f"{directory}: ERROR {e}", file=sys.stderr)
            return 1
        dump_expected(expected, directory / EXPECTED)
        for variant, result in expected["results"].items():
            print(summary_line(f"{directory}[{variant}]", result))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
