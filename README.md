# arris

A B-Rep geometric kernel written from scratch in Rust.

An *arris* is the sharp edge where two surfaces meet — the thing a B-Rep kernel exists to compute.

This is a placeholder release reserving the crate name. Work in progress; no usable API yet.

## Workspace

Arris is a Cargo workspace. `crates/arris` is the facade a consumer depends
on; it re-exports the layered crates beneath it — `arris-math`, `arris-geom`,
`arris-topo` (the representation), `arris-check` (the invariant checker),
`arris-ops`, `arris-mesh`, `arris-io` (the algorithms) and `arris-debug`
(dev-facing: rasteriser, fixtures, property-test strategies). A crate never
depends on one above it; `tools/check-layers.sh` enforces that in CI and in
the pre-commit hook. The layout and the reasons are in
`docs/01-architecture.md`; `tools/oracle/` is the Open CASCADE test oracle,
run through Python and never linked.
