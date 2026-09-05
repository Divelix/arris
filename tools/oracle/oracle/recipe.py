"""Build a shape from a fixture recipe.

The recipe grammar (also in `tests/fixtures/README.md`):

    {
      "params":   {"t": 10, "r": 3},                 # optional numbers
      "variants": {"thicker": {"t": 12}},           # optional param overrides
      "steps": [  {"name": "...", "op": "...", ...}, ... ],
      "result":   "<name of the step whose shape is the fixture's result>",
      "probes":   [{"point": [x, y, z], "label": "inside"}, ...],
      "tolerances": {"volume_rel": 1e-9, "area_rel": 1e-9,
                     "centroid_abs": 1e-7, "probe": 1e-7}
    }

Every number in a step may be a string expression over the params
(`"50 + R * cos(radians(45))"`); a plain JSON number is used as is.

Operations, by `op`:

    box        min [x,y,z], max [x,y,z]
    cylinder   base [x,y,z], axis [x,y,z], radius, height
    profile    plane {origin, x, y}, outer <loop>, holes [<loop>, ...]
               <loop> = {"circle": {"center": [u,v], "radius": r}}
                      | {"start": [u,v], "segments": [
                            {"line_to": [u,v]},
                            {"arc_to": [u,v], "via": [u,v]}, ...]}
               (the last segment ends at `start`; loop orientation is
               irrelevant, holes are oriented by the interpreter)
    extrude    profile <name>, direction [x,y,z], length
    revolve    profile <name>, axis {origin, direction}, angle_deg
    transform  of <name>, translate [x,y,z] (optional),
               rotate {axis [x,y,z], origin [x,y,z], angle_deg} (optional)
    fuse       a <name>, b <name>
    common     a <name>, b <name>
    cut        target <name>, tool <name>

Conventions are Open CASCADE's: a full revolve (360°) has seam edges, a
fuse of flush boxes does not merge coplanar faces, a common with no volume
is an empty compound. Arris mirrors these; the fixtures' `analytic` values
catch a mismatch on either side.
"""

import ast
import hashlib
import json
import math
from typing import Any

from OCP.BRepAlgoAPI import BRepAlgoAPI_Common, BRepAlgoAPI_Cut, BRepAlgoAPI_Fuse
from OCP.BRepBuilderAPI import (
    BRepBuilderAPI_MakeEdge,
    BRepBuilderAPI_MakeFace,
    BRepBuilderAPI_MakeWire,
    BRepBuilderAPI_Transform,
)
from OCP.BRepPrimAPI import (
    BRepPrimAPI_MakeBox,
    BRepPrimAPI_MakeCylinder,
    BRepPrimAPI_MakePrism,
    BRepPrimAPI_MakeRevol,
)
from OCP.GC import GC_MakeArcOfCircle
from OCP.gp import gp_Ax1, gp_Ax2, gp_Ax3, gp_Circ, gp_Dir, gp_Pln, gp_Pnt, gp_Trsf, gp_Vec
from OCP.ShapeFix import ShapeFix_Face
from OCP.TopoDS import TopoDS, TopoDS_Shape

from . import OracleError

# --- parameters and expressions -------------------------------------------

_FUNCS = {
    "sin": math.sin,
    "cos": math.cos,
    "tan": math.tan,
    "sqrt": math.sqrt,
    "radians": math.radians,
    "degrees": math.degrees,
    "abs": abs,
    "pi": math.pi,
}


def _eval(node: ast.AST, params: dict[str, float]) -> float:
    if isinstance(node, ast.Expression):
        return _eval(node.body, params)
    if isinstance(node, ast.Constant) and isinstance(node.value, (int, float)):
        return float(node.value)
    if isinstance(node, ast.Name):
        if node.id in params:
            return float(params[node.id])
        if node.id in _FUNCS and not callable(_FUNCS[node.id]):
            return float(_FUNCS[node.id])
        raise OracleError(f"unknown name {node.id!r} in expression")
    if isinstance(node, ast.UnaryOp) and isinstance(node.op, (ast.USub, ast.UAdd)):
        v = _eval(node.operand, params)
        return -v if isinstance(node.op, ast.USub) else v
    if isinstance(node, ast.BinOp):
        a, b = _eval(node.left, params), _eval(node.right, params)
        if isinstance(node.op, ast.Add):
            return a + b
        if isinstance(node.op, ast.Sub):
            return a - b
        if isinstance(node.op, ast.Mult):
            return a * b
        if isinstance(node.op, ast.Div):
            return a / b
        if isinstance(node.op, ast.Pow):
            return a**b
    if isinstance(node, ast.Call) and isinstance(node.func, ast.Name):
        f = _FUNCS.get(node.func.id)
        if callable(f) and not node.keywords:
            return float(f(*[_eval(a, params) for a in node.args]))
    raise OracleError(f"unsupported expression element {ast.dump(node)}")


def number(value: Any, params: dict[str, float]) -> float:
    """A JSON number, or a string expression over `params`."""
    if isinstance(value, bool):
        raise OracleError(f"expected a number, got {value!r}")
    if isinstance(value, (int, float)):
        return float(value)
    if isinstance(value, str):
        try:
            tree = ast.parse(value, mode="eval")
        except SyntaxError as e:
            raise OracleError(f"bad expression {value!r}: {e}") from e
        return _eval(tree, params)
    raise OracleError(f"expected a number or expression, got {value!r}")


def vector(value: Any, params: dict[str, float], n: int = 3) -> list[float]:
    if not isinstance(value, list) or len(value) != n:
        raise OracleError(f"expected a {n}-vector, got {value!r}")
    return [number(v, params) for v in value]


def resolve_params(fixture: dict, variant: str) -> dict[str, float]:
    """The params of `variant` ("default" is the base set)."""
    params = {k: float(v) for k, v in fixture.get("params", {}).items()}
    if variant != "default":
        variants = fixture.get("variants", {})
        if variant not in variants:
            raise OracleError(f"no variant {variant!r} in the recipe")
        params.update({k: float(v) for k, v in variants[variant].items()})
    return params


def variant_names(fixture: dict) -> list[str]:
    return ["default"] + sorted(fixture.get("variants", {}).keys())


def recipe_hash(fixture: dict) -> str:
    """SHA-256 of the parts of the recipe the oracle evaluates, canonically
    encoded, so an edit to `analytic` or `description` does not stale the
    expected.json and an edit to a step does."""
    evaluated = {k: fixture.get(k) for k in ("params", "variants", "steps", "result", "probes")}
    text = json.dumps(evaluated, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(text.encode()).hexdigest()


# --- building ---------------------------------------------------------------


def _pnt(v: list[float]) -> gp_Pnt:
    return gp_Pnt(v[0], v[1], v[2])


def _dir(v: list[float]) -> gp_Dir:
    if math.hypot(*v) == 0.0:
        raise OracleError("zero direction vector")
    return gp_Dir(v[0], v[1], v[2])


def _checked(builder, what: str) -> TopoDS_Shape:
    # The MakeXxx builders are lazy: IsDone is meaningful only after Build.
    if hasattr(builder, "Build"):
        builder.Build()
    if not builder.IsDone():
        raise OracleError(f"{what}: Open CASCADE reports not done")
    return builder.Shape()


class _Plane:
    def __init__(self, spec: dict, params: dict[str, float]):
        self.origin = vector(spec["origin"], params)
        x = vector(spec["x"], params)
        y = vector(spec["y"], params)
        nx, ny = math.hypot(*x), math.hypot(*y)
        if nx == 0.0 or ny == 0.0:
            raise OracleError("profile plane axes must be non-zero")
        self.x = [c / nx for c in x]
        self.y = [c / ny for c in y]
        if abs(sum(a * b for a, b in zip(self.x, self.y))) > 1e-12:
            raise OracleError("profile plane axes must be orthogonal")
        self.z = [
            self.x[1] * self.y[2] - self.x[2] * self.y[1],
            self.x[2] * self.y[0] - self.x[0] * self.y[2],
            self.x[0] * self.y[1] - self.x[1] * self.y[0],
        ]

    def to3d(self, uv: list[float]) -> gp_Pnt:
        u, v = uv
        return gp_Pnt(*[self.origin[i] + u * self.x[i] + v * self.y[i] for i in range(3)])

    def ax3(self) -> gp_Ax3:
        return gp_Ax3(_pnt(self.origin), _dir(self.z), _dir(self.x))


def _wire(loop: dict, plane: _Plane, params: dict[str, float]):
    if "circle" in loop:
        c = loop["circle"]
        center = plane.to3d(vector(c["center"], params, 2))
        radius = number(c["radius"], params)
        if radius <= 0.0:
            raise OracleError("circle radius must be positive")
        circ = gp_Circ(gp_Ax2(center, _dir(plane.z), _dir(plane.x)), radius)
        edge = BRepBuilderAPI_MakeEdge(circ)
        wire = BRepBuilderAPI_MakeWire(TopoDS.Edge(_checked(edge, "circle edge")))
        return TopoDS.Wire(_checked(wire, "circle wire"))
    start = vector(loop["start"], params, 2)
    segments = loop.get("segments", [])
    if len(segments) < 2:
        raise OracleError("a loop needs at least two segments")
    wire = BRepBuilderAPI_MakeWire()
    current = start
    for i, seg in enumerate(segments):
        if "line_to" in seg:
            to = vector(seg["line_to"], params, 2)
            edge = BRepBuilderAPI_MakeEdge(plane.to3d(current), plane.to3d(to))
        elif "arc_to" in seg:
            to = vector(seg["arc_to"], params, 2)
            via = vector(seg["via"], params, 2)
            arc = GC_MakeArcOfCircle(plane.to3d(current), plane.to3d(via), plane.to3d(to))
            if not arc.IsDone():
                raise OracleError(f"segment {i}: three-point arc is degenerate")
            edge = BRepBuilderAPI_MakeEdge(arc.Value())
        else:
            raise OracleError(f"segment {i}: expected line_to or arc_to")
        wire.Add(TopoDS.Edge(_checked(edge, f"segment {i}")))
        current = to
    if any(abs(a - b) > 1e-12 for a, b in zip(current, start)):
        raise OracleError(f"loop does not close: ends at {current}, started at {start}")
    return TopoDS.Wire(_checked(wire, "loop wire"))


def _profile(step: dict, params: dict[str, float]) -> TopoDS_Shape:
    plane = _Plane(step["plane"], params)
    outer = _wire(step["outer"], plane, params)
    face = BRepBuilderAPI_MakeFace(gp_Pln(plane.ax3()), outer, True)
    for hole in step.get("holes", []):
        face.Add(_wire(hole, plane, params))
    shape = _checked(face, "profile face")
    fix = ShapeFix_Face(TopoDS.Face(shape))
    fix.FixOrientation()
    return fix.Face()


def build(fixture: dict, variant: str = "default") -> tuple[TopoDS_Shape, dict[str, TopoDS_Shape]]:
    """The result shape of `fixture` for `variant`, and every named step."""
    params = resolve_params(fixture, variant)
    shapes: dict[str, TopoDS_Shape] = {}

    def ref(name: Any) -> TopoDS_Shape:
        if name not in shapes:
            raise OracleError(f"step refers to {name!r}, which is not built yet")
        return shapes[name]

    for i, step in enumerate(fixture.get("steps", [])):
        name = step.get("name")
        op = step.get("op")
        if not name or not op:
            raise OracleError(f"step {i}: needs a name and an op")
        if name in shapes:
            raise OracleError(f"step {i}: name {name!r} is already used")
        try:
            shapes[name] = _build_step(step, op, params, ref)
        except KeyError as e:
            raise OracleError(f"step {name!r} ({op}): missing field {e}") from e
    result = fixture.get("result")
    if result not in shapes:
        raise OracleError(f"result {result!r} is not a step")
    return shapes[result], shapes


def _build_step(step: dict, op: str, params: dict[str, float], ref) -> TopoDS_Shape:
    if op == "box":
        lo, hi = vector(step["min"], params), vector(step["max"], params)
        if any(h <= l for l, h in zip(lo, hi)):
            raise OracleError("box max must exceed min on every axis")
        return _checked(BRepPrimAPI_MakeBox(_pnt(lo), _pnt(hi)), "box")
    if op == "cylinder":
        base, axis = vector(step["base"], params), vector(step["axis"], params)
        r, h = number(step["radius"], params), number(step["height"], params)
        if r <= 0.0 or h <= 0.0:
            raise OracleError("cylinder radius and height must be positive")
        return _checked(BRepPrimAPI_MakeCylinder(gp_Ax2(_pnt(base), _dir(axis)), r, h), "cylinder")
    if op == "profile":
        return _profile(step, params)
    if op == "extrude":
        d = vector(step["direction"], params)
        length = number(step["length"], params)
        n = math.hypot(*d)
        if n == 0.0 or length <= 0.0:
            raise OracleError("extrude needs a direction and a positive length")
        vec = gp_Vec(*[c / n * length for c in d])
        return _checked(BRepPrimAPI_MakePrism(ref(step["profile"]), vec), "extrude")
    if op == "revolve":
        ax = step["axis"]
        axis = gp_Ax1(_pnt(vector(ax["origin"], params)), _dir(vector(ax["direction"], params)))
        angle = math.radians(number(step["angle_deg"], params))
        if angle <= 0.0:
            raise OracleError("revolve angle must be positive")
        return _checked(BRepPrimAPI_MakeRevol(ref(step["profile"]), axis, angle), "revolve")
    if op == "transform":
        trsf = gp_Trsf()
        if "rotate" in step:
            rot = step["rotate"]
            r = gp_Trsf()
            r.SetRotation(
                gp_Ax1(
                    _pnt(vector(rot.get("origin", [0, 0, 0]), params)),
                    _dir(vector(rot["axis"], params)),
                ),
                math.radians(number(rot["angle_deg"], params)),
            )
            trsf = r
        if "translate" in step:
            t = gp_Trsf()
            t.SetTranslation(gp_Vec(*vector(step["translate"], params)))
            trsf = t.Multiplied(trsf)
        return _checked(BRepBuilderAPI_Transform(ref(step["of"]), trsf, True), "transform")
    if op == "fuse":
        return _checked(BRepAlgoAPI_Fuse(ref(step["a"]), ref(step["b"])), "fuse")
    if op == "common":
        return _checked(BRepAlgoAPI_Common(ref(step["a"]), ref(step["b"])), "common")
    if op == "cut":
        return _checked(BRepAlgoAPI_Cut(ref(step["target"]), ref(step["tool"])), "cut")
    raise OracleError(f"unknown op {op!r}")
