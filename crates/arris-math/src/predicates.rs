//! Exact 2D orientation and in-circle predicates over `robust`.
//!
//! These decide combinatorial questions — which side of a segment a point
//! lies on, whether four points are cocircular — *exactly* on the stored
//! coordinates, with no tolerance: a predicate is never softened by a
//! tolerance and a tolerance comparison never pretends to be exact
//! (`docs/DATA-MODEL.md` §Tolerances). The arithmetic is Shewchuk's
//! adaptive-precision scheme as implemented by the `robust` crate.
//!
//! ```
//! use arris_math::Point2;
//! use arris_math::predicates::{Sign, incircle, orient2d};
//!
//! let a = Point2::new(0.0, 0.0);
//! let b = Point2::new(1.0, 0.0);
//! let c = Point2::new(0.0, 1.0);
//! assert_eq!(orient2d(a, b, c), Sign::Positive); // counter-clockwise
//! assert_eq!(orient2d(a, b, Point2::new(2.0, 0.0)), Sign::Zero); // collinear
//! assert_eq!(incircle(a, b, c, Point2::new(0.25, 0.25)), Sign::Positive); // inside
//! assert_eq!(incircle(a, b, c, Point2::new(1.0, 1.0)), Sign::Zero); // on the circle
//! ```

use core::cmp::Ordering;

use robust::Coord;

use crate::Point2;

/// The exact sign of a predicate's determinant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Sign {
    /// Below zero.
    Negative,
    /// Exactly zero: the degenerate configuration.
    Zero,
    /// Above zero.
    Positive,
}

impl Sign {
    /// The sign of `x`; NaN counts as zero, since it has no sign.
    pub fn of(x: f64) -> Sign {
        match x.partial_cmp(&0.0) {
            Some(Ordering::Less) => Sign::Negative,
            Some(Ordering::Greater) => Sign::Positive,
            Some(Ordering::Equal) | None => Sign::Zero,
        }
    }

    /// The opposite sign; zero stays zero.
    pub fn flipped(self) -> Sign {
        match self {
            Sign::Negative => Sign::Positive,
            Sign::Zero => Sign::Zero,
            Sign::Positive => Sign::Negative,
        }
    }
}

/// The exact sign of twice the signed area of the triangle `a b c`:
/// `Positive` when the points turn counter-clockwise (`c` is left of the
/// directed line `a → b`), `Negative` when clockwise, `Zero` when
/// collinear.
pub fn orient2d(a: Point2, b: Point2, c: Point2) -> Sign {
    Sign::of(robust::orient2d(coord(a), coord(b), coord(c)))
}

/// The exact position of `d` against the circle through `a`, `b`, `c`,
/// which must be counter-clockwise: `Positive` inside, `Negative` outside,
/// `Zero` on the circle. For a clockwise `a b c` the signs swap; for a
/// collinear one the "circle" is the line and the result is `Zero` for
/// every `d` on it.
pub fn incircle(a: Point2, b: Point2, c: Point2, d: Point2) -> Sign {
    Sign::of(robust::incircle(coord(a), coord(b), coord(c), coord(d)))
}

fn coord(p: Point2) -> Coord<f64> {
    Coord { x: p.x, y: p.y }
}
