"""Reading an STL file through Open CASCADE's `RWStl`
(`docs/ARCHITECTURE.md` §Formats and tools): triangle count, area and the
signed volume by the divergence theorem, `Σ a · (b × c) / 6` over the
facets in file order — the same formula `arris_mesh::TriMesh::signed_volume`
and `area` use — so a Rust test can hold its own mesh's numbers to an
independent reader of the bytes `arris_io::stl` wrote, rather than to
itself.
"""

from pathlib import Path

from OCP.RWStl import RWStl

from . import OracleError


def measure_stl(path: Path) -> dict:
    """Triangle count, area and signed volume of the STL at `path`, read
    by OCCT's `RWStl`. Raises `OracleError` if the file does not parse."""
    triangulation = RWStl.ReadFile_s(str(path))
    if triangulation is None:
        raise OracleError(f"{path}: RWStl could not read this file")
    area = 0.0
    volume = 0.0
    for i in range(1, triangulation.NbTriangles() + 1):
        triangle = triangulation.Triangle(i)
        a = triangulation.Node(triangle.Value(1))
        b = triangulation.Node(triangle.Value(2))
        c = triangulation.Node(triangle.Value(3))
        ax, ay, az = a.X(), a.Y(), a.Z()
        bx, by, bz = b.X(), b.Y(), b.Z()
        cx, cy, cz = c.X(), c.Y(), c.Z()
        abx, aby, abz = bx - ax, by - ay, bz - az
        acx, acy, acz = cx - ax, cy - ay, cz - az
        cross_x = aby * acz - abz * acy
        cross_y = abz * acx - abx * acz
        cross_z = abx * acy - aby * acx
        area += 0.5 * (cross_x**2 + cross_y**2 + cross_z**2) ** 0.5
        bcx = by * cz - bz * cy
        bcy = bz * cx - bx * cz
        bcz = bx * cy - by * cx
        volume += (ax * bcx + ay * bcy + az * bcz) / 6.0
    return {
        "triangles": triangulation.NbTriangles(),
        "area": area,
        "volume": volume,
    }
