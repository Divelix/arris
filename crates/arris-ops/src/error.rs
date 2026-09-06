//! The typed errors of the operations (`docs/01-architecture.md` §Errors):
//! every variant names the entities involved.

use arris_check::Report;
use arris_check::arris_topo::arris_geom::GeomKind;
use arris_check::arris_topo::arris_math::FrameError;
use arris_check::arris_topo::builder::BuildError;
use arris_check::arris_topo::{Body, Shape};

/// Why a requested result has no valid representation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reason {
    /// A parameter is NaN or infinite.
    NonFinite {
        /// Which parameter.
        what: &'static str,
    },
    /// A length that must be positive is not: a radius, a height, an
    /// extent of a box whose `min` is not below its `max`.
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
}

impl core::fmt::Display for Fault {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Fault::Checker(report) => write!(f, "the output fails the checker:\n{report}"),
            Fault::Builder(e) => write!(f, "the builder refused: {e}"),
            Fault::Frame(e) => write!(f, "a frame could not be placed: {e}"),
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
