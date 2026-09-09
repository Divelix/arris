//! Handles: an id plus the orientation it is viewed with.

use core::fmt;

use crate::id::{BodyId, EdgeId, EntityId, EntityKind, FaceId, ShellId, VertexId};
use crate::orientation::Orientation;

/// The uniform handle: any topological entity, with the orientation
/// composed down the path it was reached by (`docs/ARCHITECTURE.md`
/// §The model). Provenance records, iteration and errors speak in `Shape`;
/// the typed handles below are the same pair with the kind fixed and
/// convert to it for free.
///
/// Orders by id then orientation, so a `BTreeMap<Shape, _>` lists entities
/// grouped by kind in creation order.
///
/// ```
/// use arris_topo::{Face, FaceId, Orientation, Shape};
///
/// let face = Face::forward(FaceId::new(3, 0));
/// let shape: Shape = face.reversed().into();
/// assert_eq!(shape.orientation, Orientation::Reversed);
/// assert_eq!(Face::try_from(shape), Ok(face.reversed()));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Shape {
    /// The entity.
    pub id: EntityId,
    /// The orientation the entity is seen with through this handle.
    pub orientation: Orientation,
}

impl Shape {
    /// A handle to `id` with `orientation`.
    pub fn new(id: impl Into<EntityId>, orientation: Orientation) -> Self {
        Shape {
            id: id.into(),
            orientation,
        }
    }

    /// The entity's kind.
    pub const fn kind(self) -> EntityKind {
        self.id.kind()
    }

    /// The same entity with the opposite orientation.
    pub const fn reversed(self) -> Self {
        Shape {
            id: self.id,
            orientation: self.orientation.flipped(),
        }
    }

    /// This handle seen through a further reference of orientation `outer`:
    /// the id is unchanged, the orientations compose by XOR.
    pub const fn oriented_by(self, outer: Orientation) -> Self {
        Shape {
            id: self.id,
            orientation: outer.compose(self.orientation),
        }
    }
}

impl fmt::Display for Shape {
    /// The text-dump form: the orientation sign and the id, `+f3` or `-e7`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.orientation, self.id)
    }
}

/// A [`Shape`] carried a kind it cannot become: the typed conversion asked
/// for one kind and the handle holds another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WrongKind {
    /// The kind asked for.
    pub wanted: EntityKind,
    /// The handle that was not that kind.
    pub found: Shape,
}

impl fmt::Display for WrongKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "expected a {} handle, found {}", self.wanted, self.found)
    }
}

impl std::error::Error for WrongKind {}

macro_rules! handles {
    ($( $(#[$doc:meta])* $name:ident($id:ident) => $variant:ident ),* $(,)?) => {$(
        $(#[$doc])*
        ///
        /// A typed [`Shape`]: the same id-plus-orientation pair with the
        /// kind fixed at compile time. Copying it is copying two integers;
        /// it never carries geometry.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub struct $name {
            /// The entity.
            pub id: $id,
            /// The orientation the entity is seen with through this handle.
            pub orientation: Orientation,
        }

        impl $name {
            /// A handle to `id` with `orientation`.
            pub const fn new(id: $id, orientation: Orientation) -> Self {
                Self { id, orientation }
            }

            /// A `Forward` handle to `id`.
            pub const fn forward(id: $id) -> Self {
                Self::new(id, Orientation::Forward)
            }

            /// The same entity with the opposite orientation.
            pub const fn reversed(self) -> Self {
                Self::new(self.id, self.orientation.flipped())
            }

            /// This handle seen through a further reference of orientation
            /// `outer`; the orientations compose by XOR.
            pub const fn oriented_by(self, outer: Orientation) -> Self {
                Self::new(self.id, outer.compose(self.orientation))
            }

            /// The untyped handle.
            pub const fn shape(self) -> Shape {
                Shape { id: EntityId::$variant(self.id), orientation: self.orientation }
            }
        }

        impl From<$name> for Shape {
            fn from(h: $name) -> Shape {
                h.shape()
            }
        }

        impl From<$id> for $name {
            /// A `Forward` handle.
            fn from(id: $id) -> Self {
                Self::forward(id)
            }
        }

        impl TryFrom<Shape> for $name {
            type Error = WrongKind;

            fn try_from(s: Shape) -> Result<Self, WrongKind> {
                match s.id {
                    EntityId::$variant(id) => Ok(Self::new(id, s.orientation)),
                    _ => Err(WrongKind { wanted: EntityKind::$variant, found: s }),
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}{}", self.orientation, self.id)
            }
        }
    )*};
}

handles! {
    /// Handle to a vertex.
    Vertex(VertexId) => Vertex,
    /// Handle to an edge: the edge, and whether it is traversed along or
    /// against its curve.
    Edge(EdgeId) => Edge,
    /// Handle to a face: the face, and whether its normal is the surface's
    /// or the flipped one.
    Face(FaceId) => Face,
    /// Handle to a shell.
    Shell(ShellId) => Shell,
    /// Handle to a body: what operations take and return.
    Body(BodyId) => Body,
}

#[cfg(test)]
mod tests {
    use super::*;
    use Orientation::{Forward, Reversed};

    #[test]
    fn typed_and_untyped_are_the_same_pair() {
        let e = Edge::new(EdgeId::new(4, 1), Reversed);
        let s: Shape = e.into();
        assert_eq!(s, Shape::new(EdgeId::new(4, 1), Reversed));
        assert_eq!(Edge::try_from(s), Ok(e));
        assert_eq!(s.kind(), EntityKind::Edge);
        assert_eq!(s.to_string(), "-e4g1");
        assert_eq!(e.to_string(), s.to_string());
    }

    #[test]
    fn wrong_kind_is_an_error_that_names_both() {
        let s = Shape::new(FaceId::new(2, 0), Forward);
        let err = Body::try_from(s).unwrap_err();
        assert_eq!(
            err,
            WrongKind {
                wanted: EntityKind::Body,
                found: s
            }
        );
        assert_eq!(err.to_string(), "expected a body handle, found +f2");
    }

    #[test]
    fn orientation_composes_through_handles() {
        let f = Face::forward(FaceId::new(0, 0));
        assert_eq!(f.oriented_by(Reversed), f.reversed());
        assert_eq!(f.reversed().oriented_by(Reversed), f);
        assert_eq!(f.shape().oriented_by(Reversed), f.reversed().shape());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn shape_round_trips_through_serde() {
        let shapes = [
            Shape::new(VertexId::new(1, 0), Forward),
            Shape::new(FaceId::new(7, 3), Reversed),
            Body::forward(BodyId::new(0, 0)).into(),
        ];
        for s in shapes {
            let text = serde_json::to_string(&s).unwrap();
            let back: Shape = serde_json::from_str(&text).unwrap();
            assert_eq!(s, back, "{text}");
        }
        let text = serde_json::to_string(&shapes[1]).unwrap();
        assert_eq!(
            text,
            r#"{"id":{"Face":{"index":7,"generation":3}},"orientation":"Reversed"}"#
        );
    }
}
