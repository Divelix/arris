//! Curves in a surface's (u, v) plane: the evaluation type and the kind
//! table. The `Curve2` enum itself lands with the pcurve work
//! (`docs/plans/m1-geometry.md` step 9).

use core::fmt;

use arris_math::{Point2, Vec2};

/// A 2D curve evaluated at one `t`: the point and its derivatives to
/// second order, with respect to `t` as stored, never normalised.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Curve2Eval {
    /// `P(t)`.
    pub point: Point2,
    /// `dP/dt`.
    pub d1: Vec2,
    /// `d²P/dt²`.
    pub d2: Vec2,
}

/// The fieldless twin of `Curve2` (`docs/02-data-model.md` §Pcurves), for
/// errors and dispatch tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Curve2Kind {
    /// A line in (u, v).
    Line,
    /// A circle in (u, v).
    Circle,
    /// An ellipse in (u, v).
    Ellipse,
    /// A [`crate::NurbsCurve2`].
    Nurbs,
}

impl fmt::Display for Curve2Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Curve2Kind::Line => "line",
            Curve2Kind::Circle => "circle",
            Curve2Kind::Ellipse => "ellipse",
            Curve2Kind::Nurbs => "NURBS",
        })
    }
}
