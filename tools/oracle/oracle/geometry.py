"""The geometry fixture kind: Open CASCADE's evaluations, projections and
intersections of named analytic surfaces and curves, for
`tests/fixtures/geom/`. The recipe grammar (also in
`tests/fixtures/README.md`):

    {
      "kind": "geometry",
      "params":   {...},                                # optional numbers
      "surfaces": {"<name>": {"type": "plane",    "origin", "z", "x"},
                   "<name>": {"type": "cylinder", "origin", "z", "x", "radius"},
                   "<name>": {"type": "cone",     "origin", "z", "x", "radius", "half_angle_deg"},
                   "<name>": {"type": "sphere",   "origin", "z", "x", "radius"},
                   "<name>": {"type": "torus",    "origin", "z", "x", "major_radius", "minor_radius"}},
      "curves":   {"<name>": {"type": "line",     "origin", "direction"},
                   "<name>": {"type": "circle",   "origin", "z", "x", "radius"},
                   "<name>": {"type": "ellipse",  "origin", "z", "x", "major_radius", "minor_radius"}},
      "samples":  [{"of": "<name>", "params": [[u, v], ...] | [t, ...], "points": [[x, y, z], ...]}],
      "pairs":    [{"a": "<name>", "b": "<name>"}]
    }

Frames are `gp_Ax3(origin, z, x)`: `x` is projected perpendicular to `z`
and both are normalised, `y = z × x`. A pair is two surfaces or a curve
`a` against a surface `b`. Every number may be an expression over
`params` as in the solid kind.

The result per sample: `Geom_*::D2` at every parameter and
`GeomAPI_ProjectPointOnSurf` / `OnCurve` for every point (nearest point,
its parameters, the distance). Per pair: `IntAna_QuadQuadGeo` for two
surfaces — the type (`empty`, `coincident`, `point`, `line`, `circle`,
`ellipse`, or `unsolved` where it finds no conic,
`IntAna_NoGeometricSolution`) and, for every curve it returns, its type
and sampled points, or for a `point` result its points — or
`IntAna_IntConicQuad` for a curve against a surface: `coincident`, or
`points` with every hit's point and conic parameter, duplicates within
`Precision::Confusion` reported once (a tangent touch comes back twice).
"""

import math
from typing import Any

from OCP.Geom import (
    Geom_Circle,
    Geom_ConicalSurface,
    Geom_CylindricalSurface,
    Geom_Ellipse,
    Geom_Line,
    Geom_Plane,
    Geom_SphericalSurface,
    Geom_ToroidalSurface,
)
from OCP.GeomAPI import GeomAPI_ProjectPointOnCurve, GeomAPI_ProjectPointOnSurf
from OCP.gp import gp_Ax2, gp_Ax3, gp_Dir, gp_Pnt, gp_Vec
from OCP.IntAna import IntAna_IntConicQuad, IntAna_QuadQuadGeo, IntAna_Quadric, IntAna_ResultType
from OCP.Precision import Precision

from . import OracleError
from .recipe import number, resolve_params, vector

SURFACE_TYPES = ("plane", "cylinder", "cone", "sphere", "torus")
CURVE_TYPES = ("line", "circle", "ellipse")

# Where a result curve is sampled: lines at these parameters from the
# curve's own origin, closed curves at this many even steps of a turn.
LINE_SAMPLES = (-3.0, -1.0, 0.0, 1.0, 3.0)
TURN_SAMPLES = 8


def _pnt(v: list[float]) -> gp_Pnt:
    return gp_Pnt(v[0], v[1], v[2])


def _dir(v: list[float], what: str) -> gp_Dir:
    if math.hypot(*v) == 0.0:
        raise OracleError(f"{what}: zero direction")
    return gp_Dir(v[0], v[1], v[2])


def _ax3(spec: dict, params: dict[str, float], what: str) -> gp_Ax3:
    return gp_Ax3(
        _pnt(vector(spec["origin"], params)),
        _dir(vector(spec["z"], params), f"{what} z"),
        _dir(vector(spec["x"], params), f"{what} x"),
    )


def _positive(spec: dict, key: str, params: dict[str, float], what: str) -> float:
    v = number(spec[key], params)
    if not v > 0.0:
        raise OracleError(f"{what}: {key} must be positive")
    return v


def build_surface(name: str, spec: dict, params: dict[str, float]):
    """The `Geom_Surface` of a surface spec."""
    kind = spec.get("type")
    ax = _ax3(spec, params, name)
    if kind == "plane":
        return Geom_Plane(ax)
    if kind == "cylinder":
        return Geom_CylindricalSurface(ax, _positive(spec, "radius", params, name))
    if kind == "cone":
        angle = math.radians(number(spec["half_angle_deg"], params))
        if not 0.0 < angle < math.pi / 2:
            raise OracleError(f"{name}: half_angle_deg must be inside (0, 90)")
        return Geom_ConicalSurface(ax, angle, _positive(spec, "radius", params, name))
    if kind == "sphere":
        return Geom_SphericalSurface(ax, _positive(spec, "radius", params, name))
    if kind == "torus":
        big = _positive(spec, "major_radius", params, name)
        small = _positive(spec, "minor_radius", params, name)
        if small >= big:
            raise OracleError(f"{name}: minor_radius must be below major_radius")
        return Geom_ToroidalSurface(ax, big, small)
    raise OracleError(f"{name}: unknown surface type {kind!r}")


def build_curve(name: str, spec: dict, params: dict[str, float]):
    """The `Geom_Curve` of a curve spec."""
    kind = spec.get("type")
    if kind == "line":
        return Geom_Line(_pnt(vector(spec["origin"], params)), _dir(vector(spec["direction"], params), name))
    ax = _ax3(spec, params, name)
    ax2 = gp_Ax2(ax.Location(), ax.Direction(), ax.XDirection())
    if kind == "circle":
        return Geom_Circle(ax2, _positive(spec, "radius", params, name))
    if kind == "ellipse":
        big = _positive(spec, "major_radius", params, name)
        small = _positive(spec, "minor_radius", params, name)
        if small > big:
            raise OracleError(f"{name}: minor_radius must not exceed major_radius")
        return Geom_Ellipse(ax2, big, small)
    raise OracleError(f"{name}: unknown curve type {kind!r}")


def _xyz(p) -> list[float]:
    return [p.X(), p.Y(), p.Z()]


def evaluate_surface(surface, u: float, v: float) -> dict:
    p, du, dv, duu, dvv, duv = gp_Pnt(), gp_Vec(), gp_Vec(), gp_Vec(), gp_Vec(), gp_Vec()
    surface.D2(u, v, p, du, dv, duu, dvv, duv)
    return {"at": [u, v], "point": _xyz(p), "du": _xyz(du), "dv": _xyz(dv), "duu": _xyz(duu), "duv": _xyz(duv), "dvv": _xyz(dvv)}


def evaluate_curve(curve, t: float) -> dict:
    p, d1, d2 = gp_Pnt(), gp_Vec(), gp_Vec()
    curve.D2(t, p, d1, d2)
    return {"at": t, "point": _xyz(p), "d1": _xyz(d1), "d2": _xyz(d2)}


def project_surface(surface, point: list[float]) -> dict:
    proj = GeomAPI_ProjectPointOnSurf(_pnt(point), surface)
    if proj.NbPoints() == 0:
        raise OracleError(f"projection of {point} found no point")
    u, v = proj.LowerDistanceParameters()
    return {"point": point, "uv": [u, v], "nearest": _xyz(proj.NearestPoint()), "distance": proj.LowerDistance()}


def project_curve(curve, point: list[float]) -> dict:
    proj = GeomAPI_ProjectPointOnCurve(_pnt(point), curve)
    if proj.NbPoints() == 0:
        raise OracleError(f"projection of {point} found no point")
    return {"point": point, "t": proj.LowerDistanceParameter(), "nearest": _xyz(proj.NearestPoint()), "distance": proj.LowerDistance()}


def _sample_line(line) -> dict:
    loc, d = line.Location(), line.Direction()
    return {"type": "line", "points": [[loc.X() + t * d.X(), loc.Y() + t * d.Y(), loc.Z() + t * d.Z()] for t in LINE_SAMPLES]}


def _sample_closed(kind: str, conic) -> dict:
    curve = Geom_Circle(conic) if kind == "circle" else Geom_Ellipse(conic)
    return {"type": kind, "points": [_xyz(curve.Value(2.0 * math.pi * i / TURN_SAMPLES)) for i in range(TURN_SAMPLES)]}


_QUADRIC_OF = {
    "plane": lambda s: s.Pln(),
    "cylinder": lambda s: s.Cylinder(),
    "cone": lambda s: s.Cone(),
    "sphere": lambda s: s.Sphere(),
    "torus": lambda s: s.Torus(),
}

# The overloads of `IntAna_QuadQuadGeo`, by the kinds in the order it
# takes them and the tolerances each takes: an angle and a distance
# (`angular`), a distance alone (`linear`), or none. A pair the other way
# round is swapped before the call; the result does not depend on the order.
_OVERLOADS = {
    ("plane", "plane"): "angular",
    ("plane", "cylinder"): "angular",
    ("plane", "cone"): "angular",
    ("plane", "sphere"): "none",
    ("plane", "torus"): "linear",
    ("cylinder", "cylinder"): "linear",
    ("cylinder", "cone"): "linear",
    ("cylinder", "sphere"): "linear",
    ("cylinder", "torus"): "linear",
    ("cone", "cone"): "linear",
    ("cone", "torus"): "linear",
    ("sphere", "cone"): "linear",
    ("sphere", "sphere"): "linear",
    ("sphere", "torus"): "linear",
    ("torus", "torus"): "linear",
}


def intersect_surfaces(name_a: str, kind_a: str, a, name_b: str, kind_b: str, b) -> dict:
    """`IntAna_QuadQuadGeo` on the two `gp` quadrics."""
    qa, qb = _QUADRIC_OF[kind_a](a), _QUADRIC_OF[kind_b](b)
    if (kind_a, kind_b) in _OVERLOADS:
        tolerances = _OVERLOADS[(kind_a, kind_b)]
    elif (kind_b, kind_a) in _OVERLOADS:
        tolerances = _OVERLOADS[(kind_b, kind_a)]
        qa, qb = qb, qa
    else:
        raise OracleError(f"{name_a} vs {name_b}: no IntAna_QuadQuadGeo for {kind_a}–{kind_b}")
    try:
        if tolerances == "angular":
            r = IntAna_QuadQuadGeo(qa, qb, Precision.Angular_s(), Precision.Confusion_s())
        elif tolerances == "linear":
            r = IntAna_QuadQuadGeo(qa, qb, Precision.Confusion_s())
        else:
            r = IntAna_QuadQuadGeo(qa, qb)
    except TypeError as e:
        raise OracleError(f"{name_a} vs {name_b}: no IntAna_QuadQuadGeo for {kind_a}–{kind_b}: {e}") from e
    if not r.IsDone():
        raise OracleError(f"{name_a} vs {name_b}: IntAna_QuadQuadGeo not done")
    t = r.TypeInter()
    out: dict[str, Any] = {"a": name_a, "b": name_b}
    if t == IntAna_ResultType.IntAna_Empty:
        out["type"] = "empty"
    elif t == IntAna_ResultType.IntAna_Same:
        out["type"] = "coincident"
    elif t == IntAna_ResultType.IntAna_Point:
        out["type"] = "point"
        out["points"] = [_xyz(r.Point(i + 1)) for i in range(r.NbSolutions())]
    elif t == IntAna_ResultType.IntAna_Line:
        out["type"] = "line"
        out["curves"] = [_sample_line(r.Line(i + 1)) for i in range(r.NbSolutions())]
    elif t == IntAna_ResultType.IntAna_Circle:
        out["type"] = "circle"
        out["curves"] = [_sample_closed("circle", r.Circle(i + 1)) for i in range(r.NbSolutions())]
    elif t == IntAna_ResultType.IntAna_Ellipse:
        out["type"] = "ellipse"
        out["curves"] = [_sample_closed("ellipse", r.Ellipse(i + 1)) for i in range(r.NbSolutions())]
    elif t == IntAna_ResultType.IntAna_NoGeometricSolution:
        out["type"] = "unsolved"
    else:
        raise OracleError(f"{name_a} vs {name_b}: unexpected IntAna result {t}")
    return out


def intersect_curve_surface(name_c: str, kind_c: str, curve, name_s: str, kind_s: str, surface) -> dict:
    """`IntAna_IntConicQuad` of the `gp` conic against the plane or the
    quadric."""
    conic = {"line": lambda c: c.Lin(), "circle": lambda c: c.Circ(), "ellipse": lambda c: c.Elips()}[kind_c](curve)
    try:
        if kind_s == "plane":
            r = IntAna_IntConicQuad(conic, surface.Pln(), Precision.Angular_s(), Precision.Confusion_s())
        else:
            r = IntAna_IntConicQuad(conic, IntAna_Quadric(_QUADRIC_OF[kind_s](surface)))
    except TypeError as e:
        raise OracleError(f"{name_c} vs {name_s}: no IntAna_IntConicQuad for {kind_c}–{kind_s}: {e}") from e
    if not r.IsDone():
        raise OracleError(f"{name_c} vs {name_s}: IntAna_IntConicQuad not done")
    out: dict[str, Any] = {"a": name_c, "b": name_s}
    if r.IsInQuadric():
        out["type"] = "coincident"
        return out
    out["type"] = "points"
    hits: list[dict] = []
    dropped = 0
    if not r.IsParallel():
        for i in range(r.NbPoints()):
            p = _xyz(r.Point(i + 1))
            # The line–quadric intersector has no angular tolerance: a
            # line parallel to a cylinder's axis to rounding solves a
            # quadratic with a vanishing leading coefficient and reports a
            # root at 1e16. A hit that is not on both operands within
            # Precision::Confusion is that artefact, dropped and counted.
            on_curve = GeomAPI_ProjectPointOnCurve(_pnt(p), curve)
            on_surface = GeomAPI_ProjectPointOnSurf(_pnt(p), surface)
            if (
                on_curve.NbPoints() == 0
                or on_surface.NbPoints() == 0
                or on_curve.LowerDistance() > Precision.Confusion_s()
                or on_surface.LowerDistance() > Precision.Confusion_s()
            ):
                dropped += 1
                continue
            if any(math.dist(p, h["point"]) <= Precision.Confusion_s() for h in hits):
                continue
            hits.append({"point": p, "t": r.ParamOnConic(i + 1)})
    hits.sort(key=lambda h: h["t"])
    out["hits"] = hits
    if dropped:
        out["dropped"] = dropped
    return out


def compute_geometry(fixture: dict) -> dict:
    """The `samples` and `pairs` results of a geometry recipe."""
    params = resolve_params(fixture, "default")
    surfaces = fixture.get("surfaces", {})
    curves = fixture.get("curves", {})
    if set(surfaces) & set(curves):
        raise OracleError(f"names used for both a surface and a curve: {sorted(set(surfaces) & set(curves))}")
    built_s = {n: (spec["type"], build_surface(n, spec, params)) for n, spec in surfaces.items()}
    built_c = {n: (spec["type"], build_curve(n, spec, params)) for n, spec in curves.items()}

    samples = []
    for i, sample in enumerate(fixture.get("samples", [])):
        name = sample.get("of")
        result: dict[str, Any] = {"of": name, "evaluations": [], "projections": []}
        if name in built_s:
            _, s = built_s[name]
            for at in sample.get("params", []):
                u, v = vector(at, params, 2)
                result["evaluations"].append(evaluate_surface(s, u, v))
            for p in sample.get("points", []):
                result["projections"].append(project_surface(s, vector(p, params)))
        elif name in built_c:
            _, c = built_c[name]
            for at in sample.get("params", []):
                result["evaluations"].append(evaluate_curve(c, number(at, params)))
            for p in sample.get("points", []):
                result["projections"].append(project_curve(c, vector(p, params)))
        else:
            raise OracleError(f"sample {i}: {name!r} is neither a surface nor a curve")
        samples.append(result)

    pairs = []
    for i, pair in enumerate(fixture.get("pairs", [])):
        a, b = pair.get("a"), pair.get("b")
        if b not in built_s:
            raise OracleError(f"pair {i}: b {b!r} is not a surface")
        kb, sb = built_s[b]
        if a in built_s:
            ka, sa = built_s[a]
            pairs.append(intersect_surfaces(a, ka, sa, b, kb, sb))
        elif a in built_c:
            ka, ca = built_c[a]
            pairs.append(intersect_curve_surface(a, ka, ca, b, kb, sb))
        else:
            raise OracleError(f"pair {i}: a {a!r} is neither a surface nor a curve")
    return {"samples": samples, "pairs": pairs}
