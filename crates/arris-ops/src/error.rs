//! The typed errors of the operations (`docs/01-architecture.md` §Errors):
//! every variant names the entities involved.

use arris_check::arris_topo::arris_geom::{GeomError, GeomKind};
use arris_check::arris_topo::arris_math::FrameError;
use arris_check::arris_topo::builder::BuildError;
use arris_check::arris_topo::{Body, EdgeId, FaceId, Shape};
use arris_check::{ClassifyError, Report};

/// Why a requested result has no valid representation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reason {
    /// A parameter is NaN or infinite.
    NonFinite {
        /// Which parameter.
        what: &'static str,
    },
    /// A length or a measure that must be positive is not: a radius, a
    /// height, an extent of a box whose `min` is not below its `max`, the
    /// volume a body's faces enclose.
    NotPositive {
        /// Which length.
        what: &'static str,
        /// What was given.
        value: f64,
    },
    /// The result has no thickness: a common of flush bodies, a sweep of
    /// zero length.
    ZeroThickness,
    /// A revolve profile crosses its axis.
    ProfileCrossesAxis,
    /// The query needs an enclosed volume and the body is not a solid:
    /// a sheet, a wire, a general body.
    NotSolid,
    /// A boolean selected no material: a `common` of disjoint operands,
    /// a target swallowed by its tool.
    Empty,
    /// A boolean's surviving pieces make more than one shell — a
    /// disjoint `fuse`, a cut that splits its target, an enclosed cavity
    /// — and the `Solid` of cycle 1 holds one (ADR-0004, plan `⚠ OPEN`
    /// 2; a `General` body for it is cycle 2's).
    MultiShell {
        /// How many shells the pieces make.
        shells: usize,
    },
    /// Two faces touch along a curve that would be interior to both
    /// result faces — a hole wall tangent to a side face along a ruling,
    /// a section curve tangent to a loop edge at a vertex — which a
    /// manifold `Solid` cannot represent (ADR-0004, plan `⚠ OPEN` 1).
    TangentContact,
}

impl core::fmt::Display for Reason {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Reason::NonFinite { what } => write!(f, "{what} is not finite"),
            Reason::NotPositive { what, value } => {
                write!(f, "{what} must be positive, not {value}")
            }
            Reason::ZeroThickness => f.write_str("the result has no thickness"),
            Reason::ProfileCrossesAxis => f.write_str("the profile crosses the revolve axis"),
            Reason::NotSolid => f.write_str("the body is not a solid"),
            Reason::Empty => f.write_str("the result has no material"),
            Reason::MultiShell { shells } => {
                write!(f, "the result has {shells} shells, and a solid holds one")
            }
            Reason::TangentContact => {
                f.write_str("the faces touch along a curve interior to both result faces")
            }
        }
    }
}

/// A kernel bug an operation caught in its own output or its own
/// sequence, rather than a fault of the input. Returned only in release
/// builds with the `paranoid` feature on; a debug build panics with the
/// same content.
#[derive(Debug, Clone, PartialEq)]
pub enum Fault {
    /// The checker rejected the operation's output.
    Checker(Box<Report>),
    /// The builder refused a step of the operation's fixed sequence.
    Builder(BuildError),
    /// A frame the operation placed could not be built from inputs it had
    /// already validated.
    Frame(FrameError),
    /// A point the operation had to classify against a body could not be:
    /// every ray direction grazed it, a surface has no closed form
    /// against a ray, or an id did not resolve (`arris_check::classify`).
    Classify(ClassifyError),
    /// A geometry query on inputs the operation had already validated
    /// failed for a reason other than a missing closed form: a
    /// projection with no unique answer, a degenerate operand, a pcurve
    /// fit that would not converge.
    Geometry(GeomError),
    /// A section edge of a face pair crosses a seam of `face` inside
    /// itself. The seam's own hit on `other` is the pave that should
    /// have split it there (ADR-0004), so the two decisions disagreed.
    Seam {
        /// The face whose seam is crossed.
        face: FaceId,
        /// The other face of the pair.
        other: FaceId,
    },
    /// The (u, v) arrangement of a face's loops and section edges is not
    /// the planar subdivision the pave model promised (ADR-0004): see
    /// [`SplitFault`].
    Split(SplitFault),
    /// A piece of `edge`, of one face of a coincident pair, lies along
    /// the boundary of `face`, the other, on the same curve as one of
    /// its edges, but matches no piece of that edge between its paves:
    /// the two edges were paved differently by the vertices they share
    /// (ADR-0004).
    CommonBlock {
        /// The edge whose piece has no match.
        edge: EdgeId,
        /// The face whose boundary it lies along.
        face: FaceId,
    },
}

/// How the arrangement a boolean splits a face by failed to be a planar
/// subdivision. Each is a disagreement between the pave model and the
/// face's own (u, v): a kernel bug, never a property of the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitFault {
    /// A pave lies within the tolerance of an end of `edge` without
    /// being at its vertex, so the piece between them has no length.
    EmptySubEdge {
        /// The edge.
        edge: EdgeId,
    },
    /// A node of `face`'s arrangement has a single half-edge: a section
    /// edge ends inside the face without meeting anything.
    Dangling {
        /// The face.
        face: FaceId,
    },
    /// The walk from a half-edge of `face` re-entered a half-edge
    /// already walked before closing its cycle, or a cycle turned by
    /// something other than one full turn: two edges cross without a
    /// vertex there.
    Turn {
        /// The face.
        face: FaceId,
    },
    /// A clockwise cycle of `face` — a hole — lies inside no region.
    Hole {
        /// The face.
        face: FaceId,
    },
    /// A region of `face` holds no point at the clearance its polygons
    /// need: a sliver thinner than its own chord deviation.
    NoInterior {
        /// The face.
        face: FaceId,
    },
}

impl core::fmt::Display for SplitFault {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SplitFault::EmptySubEdge { edge } => {
                write!(f, "a pave on {edge} is at its end without being its vertex")
            }
            SplitFault::Dangling { face } => {
                write!(
                    f,
                    "a section edge of {face} ends at a node nothing else reaches"
                )
            }
            SplitFault::Turn { face } => {
                write!(f, "a cycle of {face}'s arrangement does not turn once")
            }
            SplitFault::Hole { face } => write!(f, "a hole of {face} lies inside no region"),
            SplitFault::NoInterior { face } => {
                write!(
                    f,
                    "a piece of {face} holds no interior point at its clearance"
                )
            }
        }
    }
}

impl core::fmt::Display for Fault {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Fault::Checker(report) => write!(f, "the output fails the checker:\n{report}"),
            Fault::Builder(e) => write!(f, "the builder refused: {e}"),
            Fault::Frame(e) => write!(f, "a frame could not be placed: {e}"),
            Fault::Classify(e) => write!(f, "a point could not be classified: {e}"),
            Fault::Geometry(e) => write!(f, "a geometry query failed: {e}"),
            Fault::Seam { face, other } => write!(
                f,
                "a section edge of {face} and {other} crosses a seam of {face} without a pave there"
            ),
            Fault::Split(e) => write!(f, "the face arrangement is not a subdivision: {e}"),
            Fault::CommonBlock { edge, face } => write!(
                f,
                "a piece of {edge} lies along the boundary of {face} but matches no piece of it"
            ),
        }
    }
}

/// Why an operation failed. Every variant names what it is about, so the
/// message a consumer shows says *which* face pair, *which* parameter,
/// not "boolean failed"; the model is as it was before the call.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum OpError {
    /// An input body fails the checker (checked in debug builds before the
    /// operation starts, and in release with the `paranoid` feature).
    #[error("{body} fails the checker:\n{report}")]
    InvalidInput {
        /// The body.
        body: Body,
        /// What it fails.
        report: Box<Report>,
    },
    /// The exhaustive dispatch reached a surface or curve pair the kernel
    /// has no formula for yet.
    #[error("no closed form for {} ({}) against {} ({})", .a.1, .a.0, .b.1, .b.0)]
    Unsupported {
        /// The first kind and entity.
        a: (GeomKind, Shape),
        /// The second.
        b: (GeomKind, Shape),
    },
    /// The requested result has no valid representation: a parameter that
    /// makes no geometry, a zero-thickness intersection, a profile
    /// crossing its revolve axis. Never a silently empty body.
    #[error("degenerate result: {reason}{}", entities_suffix(.entities))]
    Degenerate {
        /// The entities involved; none for a primitive, which has no
        /// input.
        entities: Vec<Shape>,
        /// Why.
        reason: Reason,
    },
    /// The result would need an entity tolerance above
    /// `Precision::max_tolerance`.
    #[error("{entity} would need tolerance {wanted}, above the model's maximum")]
    Tolerance {
        /// The entity.
        entity: Shape,
        /// The tolerance it wanted.
        wanted: f64,
    },
    /// A handle does not resolve in this model: the wrong model, or
    /// compacted away.
    #[error("{0} does not resolve in this model")]
    NotFound(Shape),
    /// A kernel bug, caught: see [`Fault`].
    #[error("kernel bug: {0}")]
    Internal(Fault),
}

fn entities_suffix(entities: &[Shape]) -> String {
    if entities.is_empty() {
        String::new()
    } else {
        let names: Vec<String> = entities.iter().map(|s| s.to_string()).collect();
        format!(" ({})", names.join(", "))
    }
}

impl From<BuildError> for OpError {
    fn from(e: BuildError) -> Self {
        OpError::Internal(Fault::Builder(e))
    }
}

impl From<FrameError> for OpError {
    fn from(e: FrameError) -> Self {
        OpError::Internal(Fault::Frame(e))
    }
}
