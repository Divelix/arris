//! Meshes of the Arris kernel: `TriMesh` and `Polyline`, with signed volume,
//! area and closedness, and the tessellation of bodies into them with
//! per-face and per-edge ranges.
//!
//! Guarantees: positions are `f64`; a mesh of a valid solid is closed and
//! its faces are numbered in the body's iteration order; edges are
//! discretised once and shared by the faces on both sides. The `parallel`
//! feature reserves `rayon` over faces. Depends on `arris-check` and below;
//! never on `arris-ops` or `arris-io`.
#![forbid(unsafe_code)]
#![warn(missing_docs)]
