#!/usr/bin/env python3
"""expected.py [--own] <fixture-dir>...  — build each recipe in Open
CASCADE and write its expected.json (all variants), printing one summary
line per result. `--own` also records each solid result's tolerance,
edge length and reach under `"own"`, what the differential bounds its
comparison by (`oracle.measure.own_measures`); a corpus fixture is never
written with it. A directory whose recipe the oracle cannot build prints
`<dir>: ERROR <why>` on one line of stderr and leaves no expected.json;
the rest are still built, and the exit status is 1. Run as
`uv run --project tools/oracle tools/oracle/expected.py`.
"""

import sys
from pathlib import Path

from oracle import require_ocp

require_ocp()

from oracle.fixture import EXPECTED, compute_expected, dump_expected, load_fixture, summary_lines  # noqa: E402


def main(argv: list[str]) -> int:
    if not argv:
        print(__doc__)
        return 2
    own = argv[0] == "--own"
    if own:
        argv = argv[1:]
    failed = 0
    for arg in argv:
        directory = Path(arg)
        try:
            fixture = load_fixture(directory)
            expected = compute_expected(fixture, own)
        except Exception as e:  # OracleError, or Open CASCADE's own on a recipe it cannot build
            # One line per refused directory, and on to the next: a batch
            # (the differential's, arris_debug::oracle::expected_batch)
            # reads each directory's answer apart.
            message = " ".join(str(e).split()) or type(e).__name__
            print(f"{directory}: ERROR {message}", file=sys.stderr)
            failed += 1
            continue
        dump_expected(expected, directory / EXPECTED)
        for line in summary_lines(str(directory), expected):
            print(line)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
