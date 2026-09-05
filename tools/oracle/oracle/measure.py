"""Measure a shape: the numbers expected.json holds and compare.py checks.

    volume, area, centroid       GProp mass properties
    counts                       unique vertices, edges (a seam once),
                                 faces, loops (wires), shells, solids
    euler_characteristic         V − E + 2F − L: 2(S − G) for closed shells
    genus                        S − χ/2, what the fixture's analytic genus
                                 is checked against (the "Euler line")
    degenerate                   no solid in the result
    probes                       in / out / on for each probe point
"""

import math

from OCP.BRep import BRep_Tool
from OCP.BRepClass3d import BRepClass3d_SolidClassifier
from OCP.BRepGProp import BRepGProp
from OCP.GProp import GProp_GProps
from OCP.gp import gp_Pnt
from OCP.TopAbs import (
    TopAbs_EDGE,
    TopAbs_FACE,
    TopAbs_IN,
    TopAbs_ON,
    TopAbs_OUT,
    TopAbs_SHELL,
    TopAbs_SOLID,
    TopAbs_VERTEX,
    TopAbs_WIRE,
)
from OCP.TopExp import TopExp, TopExp_Explorer
from OCP.TopoDS import TopoDS, TopoDS_Shape
from OCP.collections import IndexedMap_TopoDS_Shape_TopTools_ShapeMapHasher as IndexedMapOfShape

from . import OracleError

DEFAULT_TOLERANCES = {
    "volume_rel": 1e-9,
    "area_rel": 1e-9,
    "centroid_abs": 1e-7,
    "probe": 1e-7,
}


def _count(shape: TopoDS_Shape, kind) -> int:
    m = IndexedMapOfShape()
    TopExp.MapShapes_s(shape, kind, m)
    return m.Extent()


def solids(shape: TopoDS_Shape) -> list:
    out = []
    ex = TopExp_Explorer(shape, TopAbs_SOLID)
    while ex.More():
        out.append(TopoDS.Solid(ex.Current()))
        ex.Next()
    return out


def classify(shape: TopoDS_Shape, point: list[float], tolerance: float) -> str:
    """"in", "out" or "on" against the solids of `shape`: on if on any, in
    if in any, else out."""
    states = []
    for solid in solids(shape):
        c = BRepClass3d_SolidClassifier(solid)
        c.Perform(gp_Pnt(*point), tolerance)
        states.append(c.State())
    if TopAbs_ON in states:
        return "on"
    if TopAbs_IN in states:
        return "in"
    if all(s == TopAbs_OUT for s in states):
        return "out"
    raise OracleError(f"probe {point}: classifier returned UNKNOWN")


def measure(shape: TopoDS_Shape, probes: list[dict], probe_tolerance: float) -> dict:
    """Everything expected.json records for one result."""
    counts = {
        "vertices": _count(shape, TopAbs_VERTEX),
        "edges": _count(shape, TopAbs_EDGE),
        "faces": _count(shape, TopAbs_FACE),
        "loops": _count(shape, TopAbs_WIRE),
        "shells": _count(shape, TopAbs_SHELL),
        "solids": _count(shape, TopAbs_SOLID),
    }
    degenerate = counts["solids"] == 0
    out: dict = {"degenerate": degenerate, "counts": counts}
    if degenerate:
        return out

    vp = GProp_GProps()
    BRepGProp.VolumeProperties_s(shape, vp)
    sp = GProp_GProps()
    BRepGProp.SurfaceProperties_s(shape, sp)
    c = vp.CentreOfMass()
    chi = counts["vertices"] - counts["edges"] + 2 * counts["faces"] - counts["loops"]
    if chi % 2 != 0:
        raise OracleError(f"Euler characteristic {chi} is odd: the shape is not a closed surface")
    out.update(
        {
            "volume": vp.Mass(),
            "area": sp.Mass(),
            "centroid": [c.X(), c.Y(), c.Z()],
            "euler_characteristic": chi,
            "genus": counts["shells"] - chi // 2,
            "probes": [
                {
                    "label": p.get("label", str(i)),
                    "point": [float(x) for x in p["point"]],
                    "class": classify(shape, [float(x) for x in p["point"]], probe_tolerance),
                }
                for i, p in enumerate(probes)
            ],
        }
    )
    for k in ("volume", "area"):
        if not math.isfinite(out[k]):
            raise OracleError(f"{k} is not finite")
    return out


def compare(expected: dict, actual: dict, tolerances: dict) -> list[tuple[str, str, str, bool]]:
    """Rows of (quantity, expected, actual, ok). Counts and classes exact,
    volume and area relative, centroid absolute."""
    tol = {**DEFAULT_TOLERANCES, **tolerances}
    rows: list[tuple[str, str, str, bool]] = []

    def row(name, e, a, ok):
        rows.append((name, str(e), str(a), bool(ok)))

    row("degenerate", expected["degenerate"], actual["degenerate"], expected["degenerate"] == actual["degenerate"])
    for k in ("vertices", "edges", "faces", "loops", "shells", "solids"):
        e, a = expected["counts"][k], actual["counts"][k]
        row(f"counts.{k}", e, a, e == a)
    if expected["degenerate"] or actual["degenerate"]:
        return rows
    for k, t in (("volume", tol["volume_rel"]), ("area", tol["area_rel"])):
        e, a = expected[k], actual[k]
        row(k, repr(e), repr(a), abs(a - e) <= t * max(abs(e), abs(a), 1e-300))
    e, a = expected["centroid"], actual["centroid"]
    dist = math.dist(e, a)
    row("centroid", [f"{x:.9g}" for x in e], [f"{x:.9g}" for x in a], dist <= tol["centroid_abs"])
    row("genus", expected["genus"], actual["genus"], expected["genus"] == actual["genus"])
    ea = {p["label"]: p["class"] for p in expected.get("probes", [])}
    aa = {p["label"]: p["class"] for p in actual.get("probes", [])}
    for label in sorted(set(ea) | set(aa)):
        row(f"probe.{label}", ea.get(label), aa.get(label), ea.get(label) == aa.get(label))
    return rows


def format_table(rows: list[tuple[str, str, str, bool]]) -> str:
    w0 = max(len(r[0]) for r in rows) if rows else 8
    w1 = max(len(r[1]) for r in rows) if rows else 8
    w2 = max(len(r[2]) for r in rows) if rows else 8
    lines = [f"{'quantity':<{w0}}  {'expected':<{w1}}  {'actual':<{w2}}  ok"]
    for name, e, a, ok in rows:
        lines.append(f"{name:<{w0}}  {e:<{w1}}  {a:<{w2}}  {'ok' if ok else 'MISMATCH'}")
    return "\n".join(lines)


def tolerance_of(shape: TopoDS_Shape) -> float:
    """The largest vertex tolerance: what "on" means for this shape."""
    t = 0.0
    ex = TopExp_Explorer(shape, TopAbs_VERTEX)
    while ex.More():
        t = max(t, BRep_Tool.Tolerance_s(TopoDS.Vertex(ex.Current())))
        ex.Next()
    return t
