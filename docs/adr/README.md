# Architecture decision records

One file per decision, numbered, never edited after acceptance — a change of
mind is a new ADR that supersedes the old one. Format: Context, Decision,
Consequences, Alternatives considered. An ADR whose design was informed by a
reference implementation (Open CASCADE, truck, Fornjot) names the module read,
so provenance is auditable (`SEED.md` §8).

The decisions taken at kickoff live in `SEED.md` §9 and are not repeated
here; the first ADR is the first decision taken *after* the seed.

| # | Title | Status |
|---|---|---|
| [0001](0001-nalgebra-types-by-alias.md) | `arris-math` exposes `nalgebra`'s types by alias | accepted |
| [0002](0002-euler-operators-explicit-pcurves-role-provenance.md) | Euler operators over a staging builder; explicit pcurves; provenance rooted in roles | accepted |
| [0003](0003-tessellation-by-cdt-through-pcurves.md) | Tessellation: an own constrained Delaunay triangulation over `robust`, in (u, v), through the pcurves | accepted |
| [0004](0004-booleans-by-general-fuse-over-coedges.md) | Booleans by a General Fuse over coedges: shared paves, faces split in (u, v), the result assembled with kept ids | accepted |
| [0005](0005-ruled-direction-flattened-for-the-triangulation.md) | A ruled direction is flattened before the triangulation | accepted |
