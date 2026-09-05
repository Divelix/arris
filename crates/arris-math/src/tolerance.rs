//! The tolerance pair an algorithm takes.

use crate::Precision;

/// What a geometric algorithm needs to decide "the same" and "parallel":
/// a length and an angle. Derived from the model's [`Precision`] by
/// [`Precision::tolerance`] or from an entity's own tolerance by the
/// operation that owns it — never a literal in an algorithm
/// (`docs/02-data-model.md` §Tolerances).
///
/// ```
/// use arris_math::{Precision, Tolerance};
///
/// let tol: Tolerance = Precision::DEFAULT.tolerance();
/// assert_eq!(tol.linear, Precision::DEFAULT.default_tolerance);
/// assert_eq!(tol.angular, Precision::DEFAULT.angular_tolerance);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerance {
    /// Two points closer than this are the same point; a distance below
    /// it is zero.
    pub linear: f64,
    /// Two directions within this angle (radians) are parallel; two
    /// surfaces meeting at less than it are tangent.
    pub angular: f64,
}

impl Tolerance {
    /// A tolerance from its two parts.
    pub const fn new(linear: f64, angular: f64) -> Self {
        Tolerance { linear, angular }
    }

    /// `true` when both parts are finite and positive.
    pub fn is_consistent(&self) -> bool {
        self.linear.is_finite()
            && self.linear > 0.0
            && self.angular.is_finite()
            && self.angular > 0.0
    }
}

impl Precision {
    /// The tolerance an algorithm without an entity of its own uses:
    /// `default_tolerance` for lengths, `angular_tolerance` for angles.
    pub const fn tolerance(&self) -> Tolerance {
        Tolerance::new(self.default_tolerance, self.angular_tolerance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_precision_gives_a_consistent_tolerance() {
        assert!(Precision::DEFAULT.tolerance().is_consistent());
        assert!(!Tolerance::new(0.0, 1e-12).is_consistent());
        assert!(!Tolerance::new(1e-7, f64::NAN).is_consistent());
    }
}
