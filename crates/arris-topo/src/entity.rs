//! The entity structs of `docs/02-data-model.md` §Entities: what a
//! `Model` stores under each id. Immutable once inserted — a field is read
//! through its getter and never written — and built through the
//! constructors below, which check nothing: the checker (`arris-check`)
//! is where a bad entity is reported.
//!
//! The handles of the same five names (`Body`, `Shell`, `Face`, `Edge`,
//! `Vertex`: id plus orientation) live at the crate root; a reference from
//! one entity to another that carries an orientation is stored as that
//! handle.

use core::fmt;

use arris_math::{Interval, Point3};

use crate::handle;
use crate::id::{Curve2Id, CurveId, EdgeId, SurfaceId, VertexId};
use crate::orientation::Orientation;

/// A point and a tolerance: the true point lies within `tolerance` of
/// `point`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vertex {
    point: Point3,
    tolerance: f64,
}

impl Vertex {
    /// A vertex at `point` with `tolerance`.
    pub const fn new(point: Point3, tolerance: f64) -> Self {
        Vertex { point, tolerance }
    }

    /// The stored point.
    pub const fn point(&self) -> Point3 {
        self.point
    }

    /// The radius of the ball the true point lies in.
    pub const fn tolerance(&self) -> f64 {
        self.tolerance
    }
}

/// What an edge's 3D geometry is: a piece of a curve, or nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EdgeGeometry {
    /// A bounded piece of a curve, oriented by the curve's parameter.
    Curve {
        /// The curve.
        curve: CurveId,
        /// The parameter range, a sub-interval of the curve's domain; on a
        /// periodic curve it may cross the period.
        range: Interval,
    },
    /// No 3D curve: both vertices are one vertex at a surface singularity,
    /// and the edge exists only to give a loop a pcurve across it.
    Degenerate,
}

/// A bounded piece of a 3D curve between two vertices, or a degenerate
/// edge at a singularity (`docs/02-data-model.md` §Entities).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Edge {
    geometry: EdgeGeometry,
    start: VertexId,
    end: VertexId,
    tolerance: f64,
}

impl Edge {
    /// An edge over `geometry` from `start` to `end` with `tolerance`.
    pub const fn new(
        geometry: EdgeGeometry,
        start: VertexId,
        end: VertexId,
        tolerance: f64,
    ) -> Self {
        Edge {
            geometry,
            start,
            end,
            tolerance,
        }
    }

    /// The 3D geometry.
    pub const fn geometry(&self) -> EdgeGeometry {
        self.geometry
    }

    /// The curve and range of a non-degenerate edge.
    pub const fn curve(&self) -> Option<(CurveId, Interval)> {
        match self.geometry {
            EdgeGeometry::Curve { curve, range } => Some((curve, range)),
            EdgeGeometry::Degenerate => None,
        }
    }

    /// The vertex at the lower end of the range.
    pub const fn start(&self) -> VertexId {
        self.start
    }

    /// The vertex at the upper end of the range.
    pub const fn end(&self) -> VertexId {
        self.end
    }

    /// The radius of the tube the true curve lies in.
    pub const fn tolerance(&self) -> f64 {
        self.tolerance
    }

    /// `true` when both ends are the same vertex: a closed edge, or a
    /// degenerate one.
    pub fn is_closed(&self) -> bool {
        self.start == self.end
    }

    /// `true` for [`EdgeGeometry::Degenerate`].
    pub const fn is_degenerate(&self) -> bool {
        matches!(self.geometry, EdgeGeometry::Degenerate)
    }
}

/// One use of an edge by one loop: the edge, the direction it is walked
/// in relative to the edge's own, and the pcurve for that use.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Coedge {
    edge: EdgeId,
    orientation: Orientation,
    pcurve: Curve2Id,
}

impl Coedge {
    /// A use of `edge` walked with `orientation`, drawn on the face by
    /// `pcurve`.
    pub const fn new(edge: EdgeId, orientation: Orientation, pcurve: Curve2Id) -> Self {
        Coedge {
            edge,
            orientation,
            pcurve,
        }
    }

    /// The edge used.
    pub const fn edge(&self) -> EdgeId {
        self.edge
    }

    /// Along (`Forward`) or against (`Reversed`) the edge's curve.
    pub const fn orientation(&self) -> Orientation {
        self.orientation
    }

    /// The pcurve of this use, same-parameter with the edge's curve.
    pub const fn pcurve(&self) -> Curve2Id {
        self.pcurve
    }

    /// The edge and this use's orientation as one handle.
    pub const fn edge_use(&self) -> handle::Edge {
        handle::Edge::new(self.edge, self.orientation)
    }
}

/// A closed ring of coedges. Walked in order with the face's natural
/// normal up, the face's material is on the left: an outer loop is
/// counter-clockwise in (u, v), a hole clockwise.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Loop {
    coedges: Vec<Coedge>,
}

impl Loop {
    /// A loop of `coedges`, in walking order.
    pub const fn new(coedges: Vec<Coedge>) -> Self {
        Loop { coedges }
    }

    /// The coedges in walking order.
    pub fn coedges(&self) -> &[Coedge] {
        &self.coedges
    }
}

/// A surface trimmed by loops; its natural normal is the surface's.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Face {
    surface: SurfaceId,
    loops: Vec<Loop>,
    tolerance: f64,
}

impl Face {
    /// A face on `surface` bounded by `loops` with `tolerance`.
    pub const fn new(surface: SurfaceId, loops: Vec<Loop>, tolerance: f64) -> Self {
        Face {
            surface,
            loops,
            tolerance,
        }
    }

    /// The surface.
    pub const fn surface(&self) -> SurfaceId {
        self.surface
    }

    /// The loops, in stored order.
    pub fn loops(&self) -> &[Loop] {
        &self.loops
    }

    /// The half-thickness of the slab the true surface lies in.
    pub const fn tolerance(&self) -> f64 {
        self.tolerance
    }
}

/// A set of face uses. A shell of a solid is closed and its effective face
/// normals point out of the material.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Shell {
    faces: Vec<handle::Face>,
}

impl Shell {
    /// A shell of `faces`, each with the orientation it is used in.
    pub const fn new(faces: Vec<handle::Face>) -> Self {
        Shell { faces }
    }

    /// The face uses in stored order.
    pub fn faces(&self) -> &[handle::Face] {
        &self.faces
    }
}

/// What structure a body has (`docs/02-data-model.md` §Entities).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BodyKind {
    /// Every shell closed, every edge used by exactly two coedges.
    Solid,
    /// Open shells allowed; every edge used by one or two coedges; a
    /// face's effective normal is the sheet's front.
    Sheet,
    /// No faces, only free edges.
    Wire,
    /// Any mix, including a face used by two shells and an edge used by
    /// more than two coedges.
    General,
}

impl fmt::Display for BodyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            BodyKind::Solid => "solid",
            BodyKind::Sheet => "sheet",
            BodyKind::Wire => "wire",
            BodyKind::General => "general",
        })
    }
}

/// What operations take and return: a kind, shell uses, and the free
/// edges and vertices a wire or general body carries outside any shell.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Body {
    kind: BodyKind,
    shells: Vec<handle::Shell>,
    free_edges: Vec<handle::Edge>,
    free_vertices: Vec<VertexId>,
}

impl Body {
    /// A body of `kind` over `shells`, `free_edges` and `free_vertices`.
    pub const fn new(
        kind: BodyKind,
        shells: Vec<handle::Shell>,
        free_edges: Vec<handle::Edge>,
        free_vertices: Vec<VertexId>,
    ) -> Self {
        Body {
            kind,
            shells,
            free_edges,
            free_vertices,
        }
    }

    /// A `Solid` over `shells` with nothing free.
    pub const fn solid(shells: Vec<handle::Shell>) -> Self {
        Body::new(BodyKind::Solid, shells, Vec::new(), Vec::new())
    }

    /// The kind.
    pub const fn kind(&self) -> BodyKind {
        self.kind
    }

    /// The shell uses in stored order.
    pub fn shells(&self) -> &[handle::Shell] {
        &self.shells
    }

    /// The edge uses outside any shell (wire and general bodies).
    pub fn free_edges(&self) -> &[handle::Edge] {
        &self.free_edges
    }

    /// The vertices outside any edge (general bodies).
    pub fn free_vertices(&self) -> &[VertexId] {
        &self.free_vertices
    }
}
