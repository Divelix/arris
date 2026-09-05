//! Typed generational ids into the model arena.

use core::fmt;

macro_rules! ids {
    ($( $(#[$doc:meta])* $name:ident => $prefix:literal ),* $(,)?) => {$(
        $(#[$doc])*
        ///
        /// A `u32` slot index and a `u32` generation. Ids order by
        /// `(index, generation)`, which is creation order for a model that
        /// has never been compacted, and compare equal only when both agree —
        /// a stale handle into a compacted slot never aliases the new
        /// occupant (`docs/01-architecture.md` §The model).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub struct $name {
            index: u32,
            generation: u32,
        }

        impl $name {
            /// An id for `index` at `generation`. Only the arena mints ids
            /// that resolve; a test that needs a dangling one builds it here.
            pub const fn new(index: u32, generation: u32) -> Self {
                Self { index, generation }
            }

            /// The slot index.
            pub const fn index(self) -> u32 {
                self.index
            }

            /// The slot's generation when this id was minted.
            pub const fn generation(self) -> u32 {
                self.generation
            }
        }

        impl fmt::Display for $name {
            /// The text-dump form: the prefix and the index, with `g<n>`
            /// appended only for a generation above zero.
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}{}", $prefix, self.index)?;
                if self.generation != 0 {
                    write!(f, "g{}", self.generation)?;
                }
                Ok(())
            }
        }
    )*};
}

ids! {
    /// Id of a vertex entity.
    VertexId => "v",
    /// Id of an edge entity.
    EdgeId => "e",
    /// Id of a face entity.
    FaceId => "f",
    /// Id of a shell entity.
    ShellId => "s",
    /// Id of a body entity.
    BodyId => "b",
    /// Id of a 3D curve in the geometry arena.
    CurveId => "c",
    /// Id of a surface in the geometry arena.
    SurfaceId => "S",
    /// Id of a pcurve (a 2D curve in a surface's parameter plane).
    Curve2Id => "p",
}

/// The kind of a topological entity, without its id: what errors and
/// dispatch tables name when the entity itself is not at hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EntityKind {
    /// A vertex.
    Vertex,
    /// An edge.
    Edge,
    /// A face.
    Face,
    /// A shell.
    Shell,
    /// A body.
    Body,
}

impl fmt::Display for EntityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            EntityKind::Vertex => "vertex",
            EntityKind::Edge => "edge",
            EntityKind::Face => "face",
            EntityKind::Shell => "shell",
            EntityKind::Body => "body",
        })
    }
}

/// The id of any topological entity: what a [`Shape`](crate::Shape) wraps
/// and what provenance, iteration and errors name uniformly.
///
/// Orders by kind (vertex < edge < face < shell < body), then by id, so a
/// sorted list of entities is grouped by kind in creation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EntityId {
    /// A vertex.
    Vertex(VertexId),
    /// An edge.
    Edge(EdgeId),
    /// A face.
    Face(FaceId),
    /// A shell.
    Shell(ShellId),
    /// A body.
    Body(BodyId),
}

impl EntityId {
    /// The entity's kind.
    pub const fn kind(self) -> EntityKind {
        match self {
            EntityId::Vertex(_) => EntityKind::Vertex,
            EntityId::Edge(_) => EntityKind::Edge,
            EntityId::Face(_) => EntityKind::Face,
            EntityId::Shell(_) => EntityKind::Shell,
            EntityId::Body(_) => EntityKind::Body,
        }
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EntityId::Vertex(id) => id.fmt(f),
            EntityId::Edge(id) => id.fmt(f),
            EntityId::Face(id) => id.fmt(f),
            EntityId::Shell(id) => id.fmt(f),
            EntityId::Body(id) => id.fmt(f),
        }
    }
}

/// The id of any geometry value in the arena: what an entity references
/// and what invariant M1 reports when a reference does not resolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GeometryId {
    /// A 3D curve.
    Curve(CurveId),
    /// A surface.
    Surface(SurfaceId),
    /// A pcurve.
    Curve2(Curve2Id),
}

impl fmt::Display for GeometryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeometryId::Curve(id) => id.fmt(f),
            GeometryId::Surface(id) => id.fmt(f),
            GeometryId::Curve2(id) => id.fmt(f),
        }
    }
}

macro_rules! into_entity_id {
    ($($id:ident => $variant:ident),* $(,)?) => {$(
        impl From<$id> for EntityId {
            fn from(id: $id) -> Self {
                EntityId::$variant(id)
            }
        }
    )*};
}
into_entity_id!(VertexId => Vertex, EdgeId => Edge, FaceId => Face, ShellId => Shell, BodyId => Body);

macro_rules! into_geometry_id {
    ($($id:ident => $variant:ident),* $(,)?) => {$(
        impl From<$id> for GeometryId {
            fn from(id: $id) -> Self {
                GeometryId::$variant(id)
            }
        }
    )*};
}
into_geometry_id!(CurveId => Curve, SurfaceId => Surface, Curve2Id => Curve2);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_order_by_index_then_generation() {
        let a = FaceId::new(1, 5);
        let b = FaceId::new(2, 0);
        let c = FaceId::new(2, 1);
        assert!(a < b, "a lower index orders first whatever the generation");
        assert!(b < c, "same index: the older generation orders first");
        assert_ne!(b, c, "a stale id never equals the slot's new occupant");
        let mut v = vec![c, b, a];
        v.sort();
        assert_eq!(v, [a, b, c]);
    }

    #[test]
    fn entity_ids_group_by_kind_then_creation_order() {
        let mut v = vec![
            EntityId::Body(BodyId::new(0, 0)),
            EntityId::Vertex(VertexId::new(9, 0)),
            EntityId::Face(FaceId::new(1, 0)),
            EntityId::Vertex(VertexId::new(2, 0)),
        ];
        v.sort();
        assert_eq!(
            v,
            [
                EntityId::Vertex(VertexId::new(2, 0)),
                EntityId::Vertex(VertexId::new(9, 0)),
                EntityId::Face(FaceId::new(1, 0)),
                EntityId::Body(BodyId::new(0, 0)),
            ]
        );
    }

    #[test]
    fn display_is_the_dump_form() {
        assert_eq!(VertexId::new(12, 0).to_string(), "v12");
        assert_eq!(EdgeId::new(3, 2).to_string(), "e3g2");
        assert_eq!(EntityId::from(FaceId::new(7, 0)).to_string(), "f7");
        assert_eq!(GeometryId::from(SurfaceId::new(4, 0)).to_string(), "S4");
        assert_eq!(EntityKind::Shell.to_string(), "shell");
    }
}
