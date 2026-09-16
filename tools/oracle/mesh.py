#!/usr/bin/env python3
"""mesh.py <file.stl>  — read an STL file through Open CASCADE's `RWStl`
and print its triangle count, area and signed volume as JSON, so a Rust
test can hold `arris_mesh::TriMesh`'s own numbers to an independent
reader of the bytes `arris_io::stl` wrote. Prints nothing else to
stdout; exits 2 on a usage or read error. Run as
`uv run --project tools/oracle tools/oracle/mesh.py`.
"""

import json
import sys
from pathlib import Path

from oracle import OracleError, require_ocp

require_ocp()

from oracle.mesh import measure_stl  # noqa: E402


def main(argv: list[str]) -> int:
    if len(argv) != 1:
        print(__doc__)
        return 2
    try:
        result = measure_stl(Path(argv[0]))
    except OracleError as e:
        print(f"mesh: ERROR {e}", file=sys.stderr)
        return 2
    print(json.dumps(result))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
