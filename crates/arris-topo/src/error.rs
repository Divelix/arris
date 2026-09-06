//! The typed errors of the topology crate.

use core::fmt;

use arris_math::Precision;

use crate::id::{
    BodyId, Curve2Id, CurveId, EdgeId, EntityId, FaceId, GeometryId, ShellId, SurfaceId, VertexId,
};

/// The id of anything the arena stores: a topological entity or a geometry
/// value. What an error names when it does not matter which.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AnyId {
    /// A vertex, edge, face, shell or body.
    Entity(EntityId),
    /// A curve, surface or pcurve.
    Geometry(GeometryId),
}

impl fmt::Display for AnyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnyId::Entity(id) => id.fmt(f),
            AnyId::Geometry(id) => id.fmt(f),
        }
    }
}

impl From<EntityId> for AnyId {
    fn from(id: EntityId) -> Self {
        AnyId::Entity(id)
    }
}

impl From<GeometryId> for AnyId {
    fn from(id: GeometryId) -> Self {
        AnyId::Geometry(id)
    }
}

macro_rules! into_any_id {
    ($($id:ident => $via:ident),* $(,)?) => {$(
        impl From<$id> for AnyId {
            fn from(id: $id) -> Self {
                AnyId::from($via::from(id))
            }
        }
    )*};
}
into_any_id!(
    VertexId => EntityId, EdgeId => EntityId, FaceId => EntityId, ShellId => EntityId,
    BodyId => EntityId, CurveId => GeometryId, SurfaceId => GeometryId, Curve2Id => GeometryId,
);

/// An id that does not resolve in this model: its index is past the arena,
/// its slot was freed, or its generation is older than the slot's. Never
/// an alias of whatever occupies the slot now.
///
/// ```
/// use arris_topo::{Model, NotFound, VertexId};
///
/// let m = Model::default();
/// let err = m.vertex(VertexId::new(0, 0)).unwrap_err();
/// assert_eq!(err, NotFound { id: VertexId::new(0, 0).into() });
/// assert_eq!(err.to_string(), "v0 does not resolve in this model");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
#[error("{id} does not resolve in this model")]
pub struct NotFound {
    /// The id that was looked up.
    pub id: AnyId,
}

impl NotFound {
    /// A `NotFound` for `id`.
    pub fn new(id: impl Into<AnyId>) -> Self {
        NotFound { id: id.into() }
    }
}

/// Why a topological operation on a model failed. Every variant names what
/// it is about; none is a panic.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TopoError {
    /// A handle does not resolve in this model.
    #[error(transparent)]
    NotFound(#[from] NotFound),
    /// The `Precision` given to `Model::new` is not consistent
    /// (`Precision::is_consistent`): a field is non-finite or non-positive,
    /// the bounds are out of order, or there are no samples.
    #[error("precision is inconsistent: {0:?}")]
    Precision(Precision),
}
