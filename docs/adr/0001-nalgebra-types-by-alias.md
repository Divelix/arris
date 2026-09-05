# ADR-0001 — `arris-math` exposes `nalgebra`'s types by alias

- Status: accepted (2026-09-05)
- Plan: `m1-geometry` step 1

## Context

`SEED.md` §9 chose `nalgebra` for linear algebra: the point/vector/unit
distinction and the small solvers. What was left open is how the kernel's
public API names those types. Every crate above `arris-math`, and every
consumer, handles `Point3`, `Vec3` and `UnitVec3` on every call; the choice
decides whether they get `nalgebra`'s operators and solvers for free or
through a wrapper.

## Decision

`arris-math` defines `Point3`, `Vec3`, `UnitVec3`, `Point2`, `Vec2` and
`UnitVec2` as **type aliases** of `nalgebra::Point3<f64>`,
`Vector3<f64>`, `Unit<Vector3<f64>>` and their 2D twins, and **re-exports
the `nalgebra` crate** as `arris_math::nalgebra`. Rotations are
`nalgebra::UnitQuaternion<f64>` by that path.

Types with an Arris invariant that `nalgebra` does not carry are Arris's
own: `Frame` (right-handed, orthonormal, only validating constructors),
`Frame2` (either handedness), `Isometry` (wraps `Isometry3`, exposes only
rigid-motion operations), `Interval`, `Tolerance`, `Precision`.

## Consequences

- A `nalgebra` major version bump changes Arris's public API and is
  therefore an Arris minor bump pre-1.0 (a major one after), named in the
  tag's commit body (`.agents/rules/git.md` §Tags). `nalgebra` is pinned
  at the workspace level for that reason.
- Callers get every `nalgebra` operator, `norm`, `cross`, `dot`, the
  decompositions and `serde` support (when the feature is on) without a
  conversion, and `glam` interop at a consumer boundary is `nalgebra`'s
  problem, not Arris's.
- `Unit<_>` is the only guarantee a `UnitVec3` carries; the rest of the
  kernel's invariants live in `Frame` and the geometry types, which is
  where the checker looks for them.

## Alternatives considered

- **Newtypes** over `nalgebra`: every operator re-implemented or derived,
  every solver reached by unwrapping, and no invariant gained that a bare
  vector could carry. Boilerplate without a guarantee.
- **Own types**: re-implementing what `SEED.md` §9 chose `nalgebra` for,
  and a second numeric library for a consumer that already uses one.
