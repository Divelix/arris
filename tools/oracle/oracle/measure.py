"""Measure a shape: the numbers expected.json holds and compare.py checks.

    volume, area, centroid       GProp mass properties
    inertia                      the 3x3 inertia tensor about the centroid,
                                 unit density, physical convention (the
                                 products of inertia carried negated)
    counts                       unique vertices, edges (a seam once, a
                                 degenerate or an INTERNAL edge not at
                                 all), faces, loops (wires), shells, solids
    euler_characteristic         V − E + 2F − L: 2(S − G) for closed shells
    genus                        S − χ/2, what the fixture's analytic genus
                                 is checked against (the "Euler line");
                                 None for a non-manifold result, whose
                                 shared edge or vertex, or tangent contact
                                 carried as an edge of four faces, makes
                                 χ odd
    degenerate                   no solid in the result
    probes                       in / out / on for each probe point
"""

import math

from OCP.BRep import BRep_Tool
from OCP.BRepAdaptor import BRepAdaptor_Curve, BRepAdaptor_Surface
from OCP.BRepTools import BRepTools
from OCP.GCPnts import GCPnts_AbscissaPoint
from OCP.BRepClass3d import BRepClass3d_SolidClassifier
from OCP.BRepGProp import BRepGProp
from OCP.GeomAbs import GeomAbs_CurveType, GeomAbs_SurfaceType
from OCP.GProp import GProp_GProps
from OCP.gp import gp_Pnt
from OCP.TopAbs import (
    TopAbs_EDGE,
    TopAbs_FACE,
    TopAbs_FORWARD,
    TopAbs_IN,
    TopAbs_ON,
    TopAbs_OUT,
    TopAbs_REVERSED,
    TopAbs_SHELL,
    TopAbs_SOLID,
    TopAbs_VERTEX,
    TopAbs_WIRE,
)
from OCP.TopExp import TopExp, TopExp_Explorer
from OCP.TopoDS import TopoDS, TopoDS_Shape
from OCP.collections import IndexedMap_TopoDS_Shape_TopTools_ShapeMapHasher as IndexedMapOfShape

from . import OracleError

# The tolerance the length of an extrusion face's basis arc is asked to:
# three orders below the corpus's `area_rel` (1e-9).
LENGTH_TOL = 1e-13

# The relative error the adaptive integration is asked for on a shape
# bounded by a spline edge (`_spline_bounded`): three orders below the
# corpus's `volume_rel` and `area_rel` (1e-9), as LENGTH_TOL is.
SPLINE_EPS = 1e-12

# 3D edge curves the fixed-order integration is exact beside.
_CONIC_EDGES = (
    GeomAbs_CurveType.GeomAbs_Line,
    GeomAbs_CurveType.GeomAbs_Circle,
    GeomAbs_CurveType.GeomAbs_Ellipse,
)

_ELEMENTARY = (
    GeomAbs_SurfaceType.GeomAbs_Plane,
    GeomAbs_SurfaceType.GeomAbs_Cylinder,
    GeomAbs_SurfaceType.GeomAbs_Cone,
    GeomAbs_SurfaceType.GeomAbs_Sphere,
    GeomAbs_SurfaceType.GeomAbs_Torus,
)


def _faces(shape: TopoDS_Shape):
    explorer = TopExp_Explorer(shape, TopAbs_FACE)
    while explorer.More():
        yield TopoDS.Face(explorer.Current())
        explorer.Next()


def _spline_bounded(shape: TopoDS_Shape) -> bool:
    """Whether an edge of the shape has a 3D curve that is no line and no
    conic: a B-spline, the approximation of a section two cylinders on
    crossing or skew axes meet in (Open CASCADE's walked line, Arris's
    fitted one, ADR-0018). The faces it bounds are trimmed by B-spline
    pcurves of many spans, over which the fixed-order integration is 1e-6
    off in volume and area where the shape itself is right to 1e-10 — the
    adaptive overloads, asked for SPLINE_EPS, agree with a quadrature of
    the closed form to 1e-10. Such a shape is measured by those; every
    other keeps the fixed-order integration and its committed numbers."""
    explorer = TopExp_Explorer(shape, TopAbs_EDGE)
    while explorer.More():
        edge = TopoDS.Edge(explorer.Current())
        explorer.Next()
        if BRep_Tool.Degenerated_s(edge):
            continue
        if BRepAdaptor_Curve(edge).GetType() not in _CONIC_EDGES:
            return True
    return False


def _area(shape: TopoDS_Shape) -> float:
    """The total face area. Every face on a plane, cylinder, cone, sphere
    or torus: `BRepGProp::SurfaceProperties` over the whole shape, whose
    fixed-order integration is exact there, so the committed numbers
    stay bit for bit. A face on a `Geom_SurfaceOfLinearExtrusion` — an
    extruded elliptic profile segment, ADR-0014 — has an area element no
    polynomial in the parameters, which that integration is 2e-5 off on
    and the adaptive overload worse; its (u, v) region is a rectangle
    for every face an extrude makes, so its area is its basis arc's
    length over the face's `u` bounds, by `GCPnts_AbscissaPoint` to
    LENGTH_TOL, times its `v` extent. Any other face — a B-spline patch
    of a sample — takes the fixed-order integration face by face, as the
    whole-shape call gave it before."""
    faces = list(_faces(shape))
    spline = _spline_bounded(shape)
    if all(BRepAdaptor_Surface(f).GetType() in _ELEMENTARY for f in faces):
        sp = GProp_GProps()
        if spline:
            BRepGProp.SurfaceProperties_s(shape, sp, SPLINE_EPS)
        else:
            BRepGProp.SurfaceProperties_s(shape, sp)
        return sp.Mass()
    total = 0.0
    for face in faces:
        adaptor = BRepAdaptor_Surface(face)
        if adaptor.GetType() == GeomAbs_SurfaceType.GeomAbs_SurfaceOfExtrusion:
            u1, u2, v1, v2 = BRepTools.UVBounds_s(face)
            length = GCPnts_AbscissaPoint.Length_s(adaptor.BasisCurve(), u1, u2, LENGTH_TOL)
            total += length * (v2 - v1)
        else:
            sp = GProp_GProps()
            BRepGProp.SurfaceProperties_s(face, sp)
            total += sp.Mass()
    return total

DEFAULT_TOLERANCES = {
    "volume_rel": 1e-9,
    "area_rel": 1e-9,
    "centroid_abs": 1e-7,
    "probe": 1e-7,
    "inertia_rel": 1e-9,
}


def _count(shape: TopoDS_Shape, kind) -> int:
    m = IndexedMapOfShape()
    TopExp.MapShapes_s(shape, kind, m)
    return m.Extent()


def _count_boundary(shape: TopoDS_Shape) -> tuple[int, int, int]:
    """Unique vertices, edges and wires of the faces' boundaries. An edge
    oriented INTERNAL or EXTERNAL bounds nothing — Open CASCADE leaves a
    tangent contact it imprints as one, in a wire of its own, and its STEP
    writer drops it — so it, a wire of nothing else and a vertex on nothing
    else are left out, as a round trip would. A degenerate edge (a cone's
    apex, a sphere's pole) is a singular point of its surface, not a
    boundary between faces, so the Euler line leaves it out, as Arris's
    `Report::euler` does (docs/DATA-MODEL.md §Euler–Poincaré); its vertex
    is counted."""
    vertices, edges, wires = IndexedMapOfShape(), IndexedMapOfShape(), IndexedMapOfShape()
    wx = TopExp_Explorer(shape, TopAbs_WIRE)
    while wx.More():
        wire = wx.Current()
        bounds = False
        ex = TopExp_Explorer(wire, TopAbs_EDGE)
        while ex.More():
            edge = ex.Current()
            if edge.Orientation() in (TopAbs_FORWARD, TopAbs_REVERSED):
                bounds = True
                TopExp.MapShapes_s(edge, TopAbs_VERTEX, vertices)
                if not BRep_Tool.Degenerated_s(TopoDS.Edge(edge)):
                    edges.Add(edge)
            ex.Next()
        if bounds:
            wires.Add(wire)
        wx.Next()
    return vertices.Extent(), edges.Extent(), wires.Extent()


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


def inertia(props: GProp_GProps) -> list[list[float]]:
    """The 3x3 inertia tensor of `props` about the centre of mass, unit
    density, as rows. OCCT's `MatrixOfInertia` is already about the centre
    of mass and already in the physical convention -- the diagonal holds
    the moments and the off-diagonal the negated products -- which is the
    convention `arris_ops::measure::MassProperties` states."""
    m = props.MatrixOfInertia()
    return [[m.Value(i, j) for j in range(1, 4)] for i in range(1, 4)]


def measure(shape: TopoDS_Shape, probes: list[dict], probe_tolerance: float, manifold: bool = True) -> dict:
    """Everything expected.json records for one result. `manifold` false
    admits an odd Euler characteristic — solids of a compound sharing an
    edge or a vertex, what a recipe expecting `non-manifold` builds, or a
    tangent contact carried as an edge of four faces, what one expecting
    `tangent-contact` may build — and records no genus for it; otherwise
    an odd one is an error."""
    vertices, edges, loops = _count_boundary(shape)
    counts = {
        "vertices": vertices,
        "edges": edges,
        "faces": _count(shape, TopAbs_FACE),
        "loops": loops,
        "shells": _count(shape, TopAbs_SHELL),
        "solids": _count(shape, TopAbs_SOLID),
    }
    degenerate = counts["solids"] == 0
    out: dict = {"degenerate": degenerate, "counts": counts}
    if degenerate:
        return out

    vp = GProp_GProps()
    if _spline_bounded(shape):
        BRepGProp.VolumeProperties_s(shape, vp, SPLINE_EPS)
    else:
        BRepGProp.VolumeProperties_s(shape, vp)
    area = _area(shape)
    c = vp.CentreOfMass()
    chi = counts["vertices"] - counts["edges"] + 2 * counts["faces"] - counts["loops"]
    if chi % 2 != 0 and manifold:
        raise OracleError(f"Euler characteristic {chi} is odd: the shape is not a closed surface")
    out.update(
        {
            "volume": vp.Mass(),
            "area": area,
            "centroid": [c.X(), c.Y(), c.Z()],
            "inertia": inertia(vp),
            "euler_characteristic": chi,
            "genus": counts["shells"] - chi // 2 if chi % 2 == 0 else None,
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
    volume, area and each inertia component relative, centroid absolute."""
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
    ei, ai = expected.get("inertia"), actual.get("inertia")
    if ei is not None and ai is not None:
        # Relative to the largest component of the tensor: a product of
        # inertia that cancels to zero is not compared against itself.
        scale = max(abs(x) for row_ in ei for x in row_) or 1.0
        for i in range(3):
            for j in range(3):
                row(
                    f"inertia[{i}][{j}]",
                    repr(ei[i][j]),
                    repr(ai[i][j]),
                    abs(ai[i][j] - ei[i][j]) <= tol["inertia_rel"] * scale,
                )
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
