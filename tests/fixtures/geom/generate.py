#!/usr/bin/env python3
"""generate.py — write the geometry fixtures' `fixture.json` under this
directory: `analytic-eval` (every analytic variant in three poses,
parameters on and off the seam, projections from both sides),
`c1-intersections` (every case of the cycle-1 intersection table in one
committed general pose) and `c2-cylinder-pairs` (every pose of the
cylinder–cylinder table in that pose). Plain Python, no Open CASCADE: the coordinates
are the closed forms of `docs/DATA-MODEL.md` §Geometry written out in
world space at full precision, so both sides read identical numbers.
Rerun after editing, then `tools/oracle/expected.py` on each directory.

The poses: `world` (the identity), `swap` (`z` along +y, `x` along +z,
moved), and `tilt`, a rotation with rational entries (rows of the
orthogonal matrix `[[2, 3, 6], [3, −6, 2], [6, 2, −3]] / 7`) at an
off-centre origin, so nothing is aligned and every number is committed.
"""

import json
import math
from pathlib import Path

HERE = Path(__file__).resolve().parent
TAU = 2.0 * math.pi


# --- small vector helpers -----------------------------------------------------


def add(a, b):
    return [x + y for x, y in zip(a, b)]


def sub(a, b):
    return [x - y for x, y in zip(a, b)]


def mul(s, a):
    return [s * x for x in a]


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def cross(a, b):
    return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]


def norm(a):
    return math.sqrt(dot(a, a))


def unit(a):
    return mul(1.0 / norm(a), a)


class Frame:
    """origin, x, y, z with y = z × x; x is made perpendicular to z."""

    def __init__(self, origin, z, x):
        self.origin = list(map(float, origin))
        self.z = unit(z)
        x = sub(x, mul(dot(x, self.z), self.z))
        self.x = unit(x)
        self.y = cross(self.z, self.x)

    def to_world(self, p):
        return add(self.origin, add(mul(p[0], self.x), add(mul(p[1], self.y), mul(p[2], self.z))))

    def vec(self, v):
        return add(mul(v[0], self.x), add(mul(v[1], self.y), mul(v[2], self.z)))

    def spec(self, **rest):
        return {"origin": self.origin, "z": self.z, "x": self.x, **rest}

    def around(self, phase):
        """The direction at `phase` about z, and the one a quarter turn on."""
        a = add(mul(math.cos(phase), self.x), mul(math.sin(phase), self.y))
        return a, cross(self.z, a)


POSES = {
    "world": Frame([0, 0, 0], [0, 0, 1], [1, 0, 0]),
    "swap": Frame([1, 2, 3], [0, 1, 0], [0, 0, 1]),
    "tilt": Frame([-2.5, 1.75, 0.5], [2 / 7, 3 / 7, 6 / 7], [3 / 7, -6 / 7, 2 / 7]),
}


# --- closed forms (docs/DATA-MODEL.md §Geometry) -----------------------------


def surface_point(kind, f, dims, u, v):
    su, cu, sv, cv = math.sin(u), math.cos(u), math.sin(v), math.cos(v)
    if kind == "plane":
        local = [u, v, 0.0]
    elif kind == "cylinder":
        r = dims["radius"]
        local = [r * cu, r * su, v]
    elif kind == "cone":
        r, a = dims["radius"], math.radians(dims["half_angle_deg"])
        rho = r + v * math.sin(a)
        local = [rho * cu, rho * su, v * math.cos(a)]
    elif kind == "sphere":
        r = dims["radius"]
        local = [r * cv * cu, r * cv * su, r * sv]
    elif kind == "torus":
        big, small = dims["major_radius"], dims["minor_radius"]
        rho = big + small * cv
        local = [rho * cu, rho * su, small * sv]
    else:
        raise ValueError(kind)
    return f.to_world(local)


def surface_normal(kind, f, dims, u, v):
    su, cu, sv, cv = math.sin(u), math.cos(u), math.sin(v), math.cos(v)
    radial = [cu, su, 0.0]
    if kind == "plane":
        local = [0.0, 0.0, 1.0]
    elif kind == "cylinder":
        local = radial
    elif kind == "cone":
        a = math.radians(dims["half_angle_deg"])
        local = [math.cos(a) * cu, math.cos(a) * su, -math.sin(a)]
    elif kind == "sphere":
        local = [cv * cu, cv * su, sv]
    elif kind == "torus":
        local = [cv * cu, cv * su, sv]
    else:
        raise ValueError(kind)
    return f.vec(local)


def curve_point(kind, f, dims, t):
    st, ct = math.sin(t), math.cos(t)
    if kind == "circle":
        r = dims["radius"]
        return f.to_world([r * ct, r * st, 0.0])
    if kind == "ellipse":
        a, b = dims["major_radius"], dims["minor_radius"]
        return f.to_world([a * ct, b * st, 0.0])
    raise ValueError(kind)


def curve_in_plane_normal(kind, f, dims, t):
    """The outward in-plane normal of a circle or an ellipse at t."""
    st, ct = math.sin(t), math.cos(t)
    if kind == "circle":
        return f.vec([ct, st, 0.0])
    a, b = dims["major_radius"], dims["minor_radius"]
    return unit(f.vec([b * ct, a * st, 0.0]))


# --- analytic-eval --------------------------------------------------------------

SURFACE_DIMS = {
    "plane": {},
    "cylinder": {"radius": 2.0},
    "cone": {"radius": 2.0, "half_angle_deg": 30.0},
    "sphere": {"radius": 2.5},
    "torus": {"major_radius": 4.0, "minor_radius": 1.5},
}
CURVE_DIMS = {
    "line": {},
    "circle": {"radius": 2.0},
    "ellipse": {"major_radius": 3.0, "minor_radius": 2.0},
}
# Parameters: on the seam, just past it, mid-way, near the far end of the
# turn; v across the domain of every variant (the sphere's latitude for
# all, plus a pole for the sphere).
U_SAMPLES = [0.0, 0.7, 2.9, TAU - 0.3]
V_SAMPLES = {
    "plane": [-3.0, 0.0, 1.5, 4.0],
    "cylinder": [-3.0, 0.0, 1.5, 4.0],
    "cone": [-1.0, 0.0, 1.5, 4.0],
    "sphere": [-1.2, 0.0, 0.8, math.pi / 2],
    "torus": [0.0, 1.0, 2.9, TAU - 0.4],
}
# Projections: a point at (u, v) moved along the normal by ± this, both
# sides of the surface, at parameters off every ambiguous locus.
OFFSETS = [0.5, -0.5]
PROJECTION_PARAMS = {
    "plane": [(0.0, 0.0), (1.3, -2.1), (-4.0, 3.5)],
    "cylinder": [(0.0, 1.0), (1.3, -2.1), (5.2, 3.5)],
    "cone": [(0.0, 1.0), (1.3, 2.5), (5.2, 3.5)],
    "sphere": [(0.0, 0.3), (1.3, -0.6), (5.2, 1.1)],
    "torus": [(0.0, 0.6), (1.3, 2.2), (5.2, 4.4)],
}
T_SAMPLES = [0.0, 0.7, 2.9, TAU - 0.3]
CURVE_PROJECTION_PARAMS = [0.0, 1.1, 4.0]


def analytic_eval():
    surfaces, curves, samples = {}, {}, []
    for kind, dims in SURFACE_DIMS.items():
        for pose, f in POSES.items():
            name = f"{kind}_{pose}"
            surfaces[name] = {"type": kind, **f.spec(**dims)}
            params = [[u, v] for v in V_SAMPLES[kind] for u in U_SAMPLES]
            points = [
                add(surface_point(kind, f, dims, u, v), mul(d, surface_normal(kind, f, dims, u, v)))
                for (u, v) in PROJECTION_PARAMS[kind]
                for d in OFFSETS
            ]
            samples.append({"of": name, "params": params, "points": points})
    for kind, dims in CURVE_DIMS.items():
        for pose, f in POSES.items():
            name = f"{kind}_{pose}"
            if kind == "line":
                curves[name] = {"type": "line", "origin": f.origin, "direction": f.z}
                params = [-3.0, 0.0, 1.5, 4.0]
                points = [f.to_world([a, b, t]) for (a, b, t) in [(0.0, 0.0, 1.0), (1.5, -2.0, -3.0), (-0.5, 4.0, 2.5)]]
            else:
                curves[name] = {"type": kind, **f.spec(**dims)}
                params = T_SAMPLES
                points = [
                    add(curve_point(kind, f, dims, t), add(mul(d, curve_in_plane_normal(kind, f, dims, t)), mul(h, f.z)))
                    for t in CURVE_PROJECTION_PARAMS
                    for (d, h) in [(0.5, 1.0), (-0.5, -2.0)]
                ]
            samples.append({"of": name, "params": params, "points": points})
    return {
        "kind": "geometry",
        "description": "every analytic surface and curve in three poses (world, swap, tilt): D2 at parameters on and off the seam and across v, and projections of points displaced from the surface along its normal to both sides, off every ambiguous locus; written by generate.py",
        "surfaces": surfaces,
        "curves": curves,
        "samples": samples,
        "pairs": [],
    }


# --- c1-intersections --------------------------------------------------------------


def c1_intersections():
    f = POSES["tilt"]
    R = 2.0
    z = f.z
    surfaces = {"cyl": {"type": "cylinder", **f.spec(radius=R)}}
    curves = {}
    pairs = []

    def plane(name, origin, normal, x_hint):
        surfaces[name] = {"type": "plane", **Frame(origin, normal, x_hint).spec()}

    def line(name, origin, direction):
        curves[name] = {"type": "line", "origin": origin, "direction": direction}

    def circle(name, origin, normal, x_hint, radius):
        curves[name] = {"type": "circle", **Frame(origin, normal, x_hint).spec(radius=radius)}

    def ellipse(name, origin, normal, x_hint, major, minor):
        curves[name] = {
            "type": "ellipse",
            **Frame(origin, normal, x_hint).spec(major_radius=major, minor_radius=minor),
        }

    def pair(a, b):
        pairs.append({"a": a, "b": b})

    a, across = f.around(0.6)
    on_axis = f.to_world([0.0, 0.0, 1.5])
    # Plane–cylinder: the case table of step 4.
    plane("cap", add(on_axis, mul(0.7, a)), z, a)
    tilt = 0.5
    plane("oblique", add(on_axis, mul(0.3, across)), add(mul(math.cos(tilt), z), mul(math.sin(tilt), a)), across)
    plane("chordal", add(add(on_axis, mul(0.4 * R, a)), mul(2.0, across)), a, z)
    plane("touching", add(add(on_axis, mul(R, a)), mul(-1.5, across)), a, z)
    plane("clear", add(add(on_axis, mul(1.6 * R, a)), mul(0.5, across)), a, z)
    for name in ["cap", "oblique", "chordal", "touching", "clear"]:
        pair(name, "cyl")
    # Plane–plane: crossing, parallel apart, the same plane flipped.
    plane("cap_lifted", add(on_axis, mul(2.5, z)), z, across)
    plane("cap_flipped", add(on_axis, mul(-1.3, across)), mul(-1.0, z), a)
    pair("cap", "oblique")
    pair("cap", "cap_lifted")
    pair("cap", "cap_flipped")
    # Line–plane against the cap: crossing, parallel above, in the plane.
    cap_point = add(on_axis, mul(-0.8, across))
    d = unit(add(mul(math.cos(0.9), a), mul(math.sin(0.9), z)))
    line("skewer", sub(cap_point, mul(2.2, d)), d)
    line("hover", add(cap_point, mul(1.1, z)), unit(add(a, across)))
    line("flat", cap_point, unit(sub(a, mul(0.5, across))))
    for name in ["skewer", "hover", "flat"]:
        pair(name, "cap")
    # Line–cylinder: two hits, a touch, a miss, a ruling, parallel and off.
    b, bcross = f.around(2.3)
    d = unit(add(mul(math.cos(0.4), bcross), mul(math.sin(0.4), z)))
    line("chord", sub(add(on_axis, mul(0.45 * R, b)), mul(3.0, d)), d)
    line("grazing", sub(add(on_axis, mul(R, b)), mul(-1.7, d)), d)
    line("miss", sub(add(on_axis, mul(1.8 * R, b)), mul(0.6, d)), d)
    line("ruling", add(f.to_world([0.0, 0.0, -2.0]), mul(R, b)), z)
    line("offside", add(f.to_world([0.0, 0.0, -2.0]), mul(0.5 * R, b)), z)
    for name in ["chord", "grazing", "miss", "ruling", "offside"]:
        pair(name, "cyl")
    # Circle–plane against the cap: reach M = r sin(tilt); centre height h.
    r, tilt = 3.0, 0.8
    c_axis = add(mul(math.cos(tilt), z), mul(math.sin(tilt), a))
    reach = r * math.sin(tilt)
    cap_origin = surfaces["cap"]["origin"]
    for name, h in [("crossing_ring", 0.35 * reach), ("kissing_ring", -reach), ("hovering_ring", 1.4 * reach)]:
        circle(name, add(cap_origin, add(mul(h, z), mul(0.9, across))), c_axis, across, r)
        pair(name, "cap")
    circle("lying_ring", add(cap_origin, mul(-1.2, a)), z, across, r)
    circle("floating_ring", add(cap_origin, add(mul(0.9, z), mul(1.1, a))), z, across, r)
    pair("lying_ring", "cap")
    pair("floating_ring", "cap")
    # Circle–cylinder: a parallel, a meridional circle (four), and in the
    # plane across the axis two crossings, a touch from outside and from
    # inside, a miss outside and one inside.
    c, ccross = f.around(4.0)
    high = f.to_world([0.0, 0.0, 2.75])
    circle("parallel", high, z, c, R)
    circle("meridional", high, ccross, c, 1.6 * R)
    small = 1.2
    for name, delta in [
        ("two", abs(small - R) + 0.55 * (small + R - abs(small - R))),
        ("outside_touch", small + R),
        ("inside_touch", R - small),
        ("outside_miss", 1.3 * (small + R)),
        ("inside_miss", 0.4 * (R - small)),
    ]:
        circle(name, add(high, mul(delta, c)), z, c, small)
    for name in ["parallel", "meridional", "two", "outside_touch", "inside_touch", "outside_miss", "inside_miss"]:
        pair(name, "cyl")
    # Ellipse–plane and ellipse–cylinder, around the oblique section of
    # the cylinder — the ellipse a boolean's section edge on a cylinder
    # wall actually is. Its closed form is docs/DATA-MODEL.md §Curves:
    # centred at the piercing point of the axis, minor axis R across the
    # axis, major axis R / |n·Z| along the axis's projection on the plane.
    n = surfaces["oblique"]["z"]
    cos = abs(dot(n, z))
    piercing = add(f.origin, mul(dot(n, sub(surfaces["oblique"]["origin"], f.origin)) / dot(n, z), z))
    ellipse("section", piercing, n, z, R / cos, R)
    section = Frame(piercing, n, z)
    pair("section", "cyl")
    pair("section", "oblique")
    # Three planes normal to the section's major axis: through the centre
    # (the two ends of the minor axis), at the end of the major axis (one
    # touch), and clear of it.
    for name, along, x_hint in [
        ("cut_minor", 0.0, section.z),
        ("touch_major", R / cos, section.z),
        ("clear_major", 1.5 * R / cos, section.z),
    ]:
        plane(name, add(piercing, mul(along, section.x)), section.x, x_hint)
        pair("section", name)
    # An ellipse in a plane through the axis: four crossings when its
    # reach across the axis exceeds R, two touches when it equals R.
    e, ecross = f.around(5.2)
    mid = f.to_world([0.0, 0.0, -0.75])
    ellipse("meridional_ellipse", mid, ecross, e, 1.6 * R, 1.2 * R)
    ellipse("grazing_ellipse", mid, ecross, e, R, 0.6 * R)
    # And one across the axis, entirely inside the wall.
    ellipse("inner_ellipse", mid, z, e, 0.9 * R, 0.5 * R)
    for name in ["meridional_ellipse", "grazing_ellipse", "inner_ellipse"]:
        pair(name, "cyl")
    return {
        "kind": "geometry",
        "description": "every case of the cycle-1 intersection table (plane–plane, plane–cylinder, line–plane, line–cylinder, conic–plane, conic–cylinder) around one cylinder in the tilt pose: circle, ellipse, two rulings, a tangent ruling and empty; crossing, parallel and coincident planes; one hit, parallel and coincident lines; two hits, a touch, a miss, a ruling; two, four, touches, misses, a parallel; the oblique section ellipse against the cylinder and the plane it lies in, cut, touched and missed by three planes normal to its major axis, and ellipses across the axis with four crossings, two touches and none; written by generate.py",
        "surfaces": surfaces,
        "curves": curves,
        "samples": [],
        "pairs": pairs,
    }


# --- c2-cylinder-pairs --------------------------------------------------------------


def c2_cylinder_pairs():
    f = POSES["tilt"]
    R = 2.0
    z = f.z
    surfaces = {"cyl": {"type": "cylinder", **f.spec(radius=R)}}
    pairs = []

    def cylinder(name, origin, axis, x_hint, radius):
        surfaces[name] = {"type": "cylinder", **Frame(origin, axis, x_hint).spec(radius=radius)}

    def pair(a, b):
        pairs.append({"a": a, "b": b})

    a, across = f.around(0.6)
    on_axis = f.to_world([0.0, 0.0, 1.5])
    # Parallel axes, `d` apart along `a`, each origin slid along the axis
    # and one axis reversed: two rulings, a touch outside and inside
    # (`d = R + r` and `R − r`), apart, nested.
    r = 1.5
    for name, d, radius, flip in [
        ("two", 2.0, r, False),
        ("outside", R + r, r, True),
        ("inside", R - r, r, False),
        ("apart", R + r + 1.0, r, False),
        ("nested", 0.8, 0.5, True),
    ]:
        axis = mul(-1.0, z) if flip else z
        cylinder(name, add(add(on_axis, mul(d, a)), mul(-0.7, z)), axis, across, radius)
        pair("cyl", name)
    # Crossing axes through a point of the first, equal radii at 90° and
    # 50°: two ellipses each. Unequal radii: a quartic, unsolved.
    psi = math.radians(50.0)
    cylinder("cross_90", add(on_axis, mul(0.7, a)), a, z, R)
    cylinder("cross_50", add(on_axis, mul(-1.1, add(mul(math.cos(psi), z), mul(math.sin(psi), a)))), add(mul(math.cos(psi), z), mul(math.sin(psi), a)), across, R)
    cylinder("cross_unequal", add(on_axis, mul(0.4, across)), across, z, 1.2)
    for name in ["cross_90", "cross_50", "cross_unequal"]:
        pair("cyl", name)
    # Skew axes along `a`, their common perpendicular along `across`:
    # further apart than the radii (empty by the triangle inequality) and
    # within them (a quartic). Unequal radii: Open CASCADE reports two
    # ellipses for equal radii whatever the gap.
    cylinder("skew_apart", add(on_axis, mul(R + 1.2 + 0.8, across)), a, z, 1.2)
    cylinder("skew_close", add(on_axis, mul(1.5, across)), a, z, 1.2)
    for name in ["skew_apart", "skew_close"]:
        pair("cyl", name)
    # Two pairs the other way round: the rulings from the second operand's
    # origin, the ellipses with the axes swapped.
    pair("two", "cyl")
    pair("cross_50", "cyl")
    return {
        "kind": "geometry",
        "description": "every case of the cylinder–cylinder table around one cylinder in the tilt pose: parallel axes giving two rulings, a tangent ruling outside and inside, apart and nested; crossing axes of equal radii at 90° and 50° giving two ellipses; crossing axes of unequal radii and skew axes within the radii, which Open CASCADE leaves unsolved; skew axes apart, which it leaves unsolved and Arris decides empty; two pairs swapped; written by generate.py",
        "surfaces": surfaces,
        "curves": {},
        "samples": [],
        "pairs": pairs,
    }


def write(name, recipe):
    directory = HERE / name
    directory.mkdir(exist_ok=True)
    (directory / "fixture.json").write_text(json.dumps(recipe, indent=2) + "\n")
    print(f"{directory}: {len(recipe['surfaces'])} surfaces, {len(recipe['curves'])} curves, {len(recipe['samples'])} samples, {len(recipe['pairs'])} pairs")


if __name__ == "__main__":
    write("analytic-eval", analytic_eval())
    write("c1-intersections", c1_intersections())
    write("c2-cylinder-pairs", c2_cylinder_pairs())
