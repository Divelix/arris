//! Meshes of the Arris kernel: `TriMesh` and `Polyline`, with signed volume,
//! area and closedness, and the tessellation of bodies into them with
//! per-face and per-edge ranges.
//!
//! Guarantees (`docs/01-architecture.md` §Tessellation, ADR-0003):
//! positions are `f64` and exact evaluations of the geometry — a topo
//! vertex's point, an edge's curve at a sampled parameter, later a
//! surface at an interior grid point; a topo vertex is one mesh vertex
//! and an edge's samples are one index run shared by every face that
//! uses the edge, so a mesh of a solid is closed by construction; a seam
//! edge is discretised once and its indices appear in the wall's
//! triangles twice, a degenerate edge's (u, v) segment maps to one index
//! and the triangles that collapse are dropped; triangles are
//! counter-clockwise seen from outside, by the face use's effective
//! orientation against the surface normal; `FaceRange`s and `EdgeRange`s
//! are in the body's iteration order; same body, same chord, same mesh
//! on every platform, with the `parallel` feature on or off. The
//! `parallel` feature reserves `rayon` over faces. Depends on
//! `arris-check` and below; never on `arris-ops` or `arris-io`.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod aabb;
pub mod cdt;
mod polyline;
mod tessellate;
mod trimesh;

pub use aabb::Aabb;
pub use polyline::Polyline;
pub use tessellate::tessellate;
pub use trimesh::{EdgeRange, FaceRange, MeshError, TriMesh};
