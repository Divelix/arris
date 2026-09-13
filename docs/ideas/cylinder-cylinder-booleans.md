# Idea: cylinder-cylinder-booleans

- Status: Open
- Raised: 2026-09-13
- Prompt (verbatim from the human): "good, /idea next thing"

## Problem

C2's cylinder–cylinder line (`docs/ROADMAP.md` §C2). The consumer's probe
cuts one extruded circle from another: two parallel `z` cylinders of
radius 0.03 with axes 0.04 apart. The tool clears both caps, and the
expected volume is `(πr² − lens)·h` = 2.2079e-5. Arris refuses it today:
`cylinder_cylinder` (`crates/arris-geom/src/intersect.rs`) handles only
coaxial pairs and returns `GeomError::Unsupported` for every other pose.
The boolean passes that on as `OpError::Unsupported`. The gap is not
limited to booleans. `docs/DATA-MODEL.md` §Curves says S5 reports every
non-coaxial cylinder pair unchecked, so an extrude of a profile with two
arcs whose boxes overlap already carries an unchecked row.

Two cylinders meet in one of three ways (Open CASCADE's
`IntAna_QuadQuadGeo::Perform(gp_Cylinder, gp_Cylinder)`, read in the
reference tree):

- **Parallel axes, distance `d`:** two rulings when `|R₁−R₂| < d < R₁+R₂`,
  one tangent ruling at either end of that range, and nothing outside it.
  Everything is exact lines.
- **Equal radii with intersecting axes:** two ellipses in the planes that
  bisect the axes. They are closed-form conics, a T-pipe or an elbow.
  Open CASCADE builds this arm for any angle.
- **Anything else:** a quartic space curve with no `Curve` variant. This
  is the `⚠ OPEN` in DATA-MODEL §Curves and `SEED.md` §10's first kickoff
  question.

## Constraints it runs into

- `SEED.md` §9: exact intersections for analytic pairs, no marcher, no
  fitted 3D curve standing in for a closed form. Exhaustive dispatch, with
  no wildcard arm.
- ADR-0004 decides these points:
  - Tangent pairs are named cases: a slit interior to two result faces is
    `Reason::TangentContact`, and a touch from outside in a `cut` passes.
  - The *curvature rule* that decides which side a tangent piece lies on
    is written for a plane and a cylinder only (`tangent_side` in
    `crates/arris-ops/src/boolean/result.rs`, which returns `None` for any
    other pair).
- The roadmap's own line: crossing axes "close the quadric-curve
  `⚠ OPEN` with an ADR; no probe forces them". The fillet idea
  (`docs/ideas/fillet-and-chamfer.md`, still Open) needs the equal-radius
  ellipse arm for its miter and proposes moving the general pairs to C3.
- The accept line of C2 holds corpus results to `Full` with nothing
  unchecked. A cylinder pair's S5 row is only as checked as the
  intersector arm behind it.

## Options

### A — Parallel axes only

Add the parallel arm: `Transversal` for two rulings, `Tangent` for one
(externally tangent at `d = R₁+R₂`, internally at `d = |R₁−R₂|`), and
`Empty` otherwise. Each ruling gets a deterministic origin on the first
cylinder's frame, symmetric under swapping operands up to orientation, as
the table already promises. The pcurves exist (a ruling is a `Line` at
constant `u`). The edge-on-face hits exist (line–cylinder, conic–cylinder).
Two pieces are new:

- **The curvature rule, generalised.** Compare the two surfaces' normal
  curvatures across the ruling instead of matching on plane–cylinder. A
  plane has curvature 0, and a cylinder `±1/R` by which side its axis
  lies. That gives one rule for every ruling contact. A boss touching a
  boss is `TangentContact` in a `fuse`; a pin tangent inside a bore works
  out by the same rule.
- **A section ruling that lands on a seam, or passes through a rim
  vertex.** The pave model shares the point, but no fixture has put a
  section curve exactly on an existing edge of a curved face. That needs
  a property test with a random rotation about the axes.

Crossing axes stay `Unsupported` and the `⚠ OPEN` stays open. As a side
effect, every cylinder pair an extrude makes has parallel axes, so that
S5 row becomes checked. Cost: ~5 steps.
1. The intersector arm, with the oracle's geometry fixture.
2. The curvature rule generalised.
3. Fixtures, the probe at scale and in its own units:
   `cut`/`fuse`/`common` of overlapping cylinders, an external tangent
   `fuse` refused, an internal tangent.
4. A property strategy for parallel cylinder pairs (additivity,
   cut-then-fuse).
5. The S5 row and extrude's unchecked pair gone.

### B — A, plus the equal-radius ellipse arm

As A, and also two `Transversal` ellipses for equal radii whose axes
intersect, and one `Tangent`-style refusal where they touch at a point.
Each ellipse's pcurve on both cylinders is a fitted `Nurbs` (the existing
rule for an oblique section), exactly as `boolean/oblique-hole` already
does on one side. It needs no new `Curve` variant, so it does not decide
the `⚠ OPEN`. It covers a T-pipe and an elbow fuse, and it is the arm the
fillet idea's miter needs, built once. The risk: the two ellipses cross
each other at two points on the plane of the axes, so a boolean gets
section curves that meet at a section vertex no face edge made. The pave
model allows this, but nothing in the corpus exercises it. Cost: A plus
~2 steps (the arm with its geometry fixture, a T-pipe fixture set).

### C — Every cylinder pair, closing the `⚠ OPEN`

As B, plus the quartic. This means an ADR choosing between an exact
`Curve::QuadricSection` (a new geometry variant: a breaking change, every
curve `match` fails to compile, and STEP writes a fitted B-spline) and a
`Curve::Nurbs` fitted to the edge tolerance. It also needs a root-finding
parametrisation robust through the figure-eight case, where the two
lobes meet. That is C3's line ("every quadric pair in the intersector")
in all but name. No probe forces it and nothing in C2 consumes it.
Cost: ~10 steps and an ADR.

### Do nothing

The transversal probe stays red, so C2 does not close. Extrudes with
overlapping arc cylinders keep an unchecked S5 row.

## Recommendation

**B.** A alone passes the probe, but it leaves the fillet plan to add the
ellipse arm later, from inside a much larger change. That arm is a closed
form, is cheap here, and gets its own fixtures before any blend depends on
it. C is C3's work pulled forward with no consumer. It would also decide
the `⚠ OPEN` without the one piece of evidence that should decide it: which
representation the first boolean over a quartic actually needs.

The generalised curvature rule belongs in either A or B. A second
hard-coded pair would be the start of a table the next tangent pair (cone,
sphere, torus) has to grow again.

What would change my mind: if the fillet idea is rejected or loses its
miter, B's extra arm has no consumer in C2, and A is enough.

## Decision for the human

1. Scope: **B, parallel axes plus the equal-radius ellipse arm** (A,
   parallel only; C, every pair)?
2. Crossing axes with unequal radii: **stay `Unsupported` in C2, with the
   roadmap's C2 line narrowed and the quadric-curve `⚠ OPEN` moved to C3**
   (or kept in C2 as the line reads today)?
3. The tangent side of a contact: **one curvature rule over normal
   curvatures across the ruling, replacing the plane–cylinder match** (or
   a second match arm for cylinder–cylinder)?
4. ADR: **none**. Both arms are closed forms inside ADR-0004's design, and
   the curvature rule is a design-doc change to §Operations. An ADR is due
   only if option C is chosen.
