//! Topology of the Arris kernel: the `Model` arena, typed generational ids,
//! `Shape` handles as id plus orientation, the entities of
//! `docs/02-data-model.md` §Topology, pcurves, per-entity tolerances, Euler
//! operators, deterministic adjacency and iteration, and `Provenance`.
//!
//! Guarantees: entities are immutable and the arena is append-only; ids are
//! allocated in creation order and iteration order is the same on every
//! platform. The `serde` feature (on by default) derives the native format's
//! encoding. Depends on `arris-geom` and below, both re-exported here so a
//! crate above reaches geometry through this one alone.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod arena;
pub mod builder;
pub mod entity;
mod error;
mod handle;
mod id;
mod model;
mod orientation;
mod walk;

pub use arris_geom;
pub use arris_math;

pub use arena::CHUNK_SIZE;
pub use builder::Builder;
pub use error::{AnyId, NotFound, TopoError};
pub use handle::{Body, Edge, Face, Shape, Shell, Vertex, WrongKind};
pub use id::{
    BodyId, Curve2Id, CurveId, EdgeId, EntityId, EntityKind, FaceId, GeometryId, ShellId,
    SurfaceId, VertexId,
};
pub use model::{CoedgeRef, Model, RawInsert};
pub use orientation::Orientation;
pub use walk::Closure;
