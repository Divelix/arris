//! The typed errors of the geometry crate.

use core::fmt;

use arris_math::{Point3, Tolerance};

use crate::{CurveKind, SurfaceKind};

/// The kind of a geometric operand, for errors that name a pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GeomKind {
    /// A [`crate::Surface`] variant.
    Surface(SurfaceKind),
    /// A [`crate::Curve`] variant.
    Curve(CurveKind),
    /// A point, the first operand of a projection.
    Point,
}

impl fmt::Display for GeomKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeomKind::Surface(k) => write!(f, "{k} surface"),
            GeomKind::Curve(k) => write!(f, "{k} curve"),
            GeomKind::Point => f.write_str("point"),
        }
    }
}

impl From<SurfaceKind> for GeomKind {
    fn from(k: SurfaceKind) -> Self {
        GeomKind::Surface(k)
    }
}

impl From<CurveKind> for GeomKind {
    fn from(k: CurveKind) -> Self {
        GeomKind::Curve(k)
    }
}

/// Where a projection has no unique answer: the locus a point was found
/// on. Every locus is a set of measure zero decided to rounding
/// ([`arris_math::is_negligible`]), never a tolerance, and it is reported
/// rather than resolved by a silent choice of parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AmbiguousLocus {
    /// The axis of a cylinder, cone or torus, or the axis of a circle
    /// through its centre: every point of a whole parallel is nearest.
    Axis,
    /// The centre of a sphere, or of a circle-like ellipse.
    Centre,
    /// The torus's centre circle, at the middle of the tube: every point
    /// of a whole meridian is nearest.
    CentreCircle,
    /// The plane through a cone's apex perpendicular to its axis: the two
    /// nappes are equally near.
    ApexPlane,
    /// The segment of an ellipse's major axis inside its evolute: two
    /// points, mirror images across the axis, are equally near.
    MajorAxis,
}

impl fmt::Display for AmbiguousLocus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            AmbiguousLocus::Axis => "the axis",
            AmbiguousLocus::Centre => "the centre",
            AmbiguousLocus::CentreCircle => "the centre circle",
            AmbiguousLocus::ApexPlane => "the plane through the apex",
            AmbiguousLocus::MajorAxis => "the major axis inside the evolute",
        })
    }
}

/// Why a geometric query has no answer. Every variant names the operands
/// involved, so the message says *which* pair or *which* point, never
/// "projection failed" (`.agents/rules/kernel.md`).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum GeomError {
    /// The exhaustive dispatch reached a pair the kernel has no closed
    /// form for yet. Never a fallback: a wildcard arm is forbidden.
    #[error("no closed form for {a} against {b}")]
    Unsupported {
        /// The first operand's kind.
        a: GeomKind,
        /// The second operand's kind.
        b: GeomKind,
    },
    /// An operand has no valid representation for the query: a frame
    /// with a non-finite axis, later a knot vector that is not one.
    #[error("degenerate {kind}: {reason}")]
    Degenerate {
        /// The operand.
        kind: GeomKind,
        /// What is wrong with it.
        reason: String,
    },
    /// The tolerance handed to the query is not finite and positive in
    /// both parts, so no decision it makes would mean anything.
    #[error("tolerance {0:?} is not finite and positive")]
    InvalidTolerance(Tolerance),
    /// The point has no unique nearest point on the target, or its
    /// nearest point has no unique parameter that the query could return
    /// without guessing.
    #[error("projection of {point} onto a {kind} is ambiguous: the point is on {locus}")]
    Ambiguous {
        /// What was projected onto.
        kind: GeomKind,
        /// The locus the point lies on.
        locus: AmbiguousLocus,
        /// The point.
        point: Point3,
    },
}
