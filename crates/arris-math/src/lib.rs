//! Numeric foundation of the Arris kernel: points, vectors and unit vectors
//! over `nalgebra`, frames, axes and rigid motions, intervals, exact orientation
//! predicates over `robust`, polynomial and interval-guarded root finding,
//! and `Precision`, the model-wide tolerance configuration.
//!
//! Guarantees: `f64` throughout, no allocation in evaluation, no panic on
//! any finite input, and no numeric literal standing in for a tolerance —
//! every tolerance is a `Precision` field, a [`Tolerance`] derived from it,
//! or a named constant with a comment (`.agents/rules/kernel.md`). Depends
//! on nothing in the workspace.
//!
//! The point and vector types are `nalgebra`'s by alias and `nalgebra` is
//! re-exported (`docs/adr/0001-nalgebra-types-by-alias.md`), so a caller
//! reaches every operator and solver `nalgebra` has and a `nalgebra` major
//! bump is an Arris API change.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod aabb;
mod axis;
mod frame;
mod interval;
mod isometry;
mod precision;
pub mod predicates;
pub mod roots;
mod tolerance;

pub use nalgebra;

pub use aabb::Aabb;
pub use axis::Axis;
pub use frame::{Frame, Frame2, FrameError, Handedness};
pub use interval::{Interval, IntervalError};
pub use isometry::Isometry;
pub use precision::Precision;
pub use tolerance::Tolerance;

/// Relative rounding slack: a magnitude at or below this fraction of its
/// natural scale is rounding noise, not a value. Eight ulps — what a
/// handful of multiplications and one trigonometric evaluation leave
/// behind, and orders of magnitude below any model tolerance. It is not a
/// geometric tolerance and never decides whether two things are *the
/// same*; it decides whether a computed quantity is zero *in floating
/// point*: the radius of a sphere's parallel at the pole, `cos(π/2)`
/// evaluated in `f64`, is `6e-17`, not `0`.
pub const RELATIVE_ROUNDING: f64 = 8.0 * f64::EPSILON;

/// `|x| ≤ RELATIVE_ROUNDING · |scale|`: `x` is zero to rounding at
/// `scale`. A zero `scale` makes only an exact zero negligible.
///
/// ```
/// use arris_math::is_negligible;
/// use core::f64::consts::FRAC_PI_2;
///
/// assert!(is_negligible(3.0 * FRAC_PI_2.cos(), 3.0));
/// assert!(!is_negligible(3.0 * (FRAC_PI_2 - 1e-9).cos(), 3.0));
/// ```
pub fn is_negligible(x: f64, scale: f64) -> bool {
    x.abs() <= RELATIVE_ROUNDING * scale.abs()
}

/// An angle moved into `[0, 2π)`: what a periodic curve's or surface's
/// parameter is reported in (`docs/DATA-MODEL.md` §Conventions). A
/// negative angle whose sum with `2π` rounds up to `2π` becomes `0` —
/// the same point on the circle, and inside the domain. A non-finite
/// angle comes back unchanged.
///
/// ```
/// use arris_math::wrap_angle;
/// use core::f64::consts::TAU;
///
/// assert_eq!(wrap_angle(0.0), 0.0);
/// assert_eq!(wrap_angle(-1.0), TAU - 1.0);
/// assert_eq!(wrap_angle(-1e-300), 0.0);
/// assert_eq!(wrap_angle(TAU + 1.0), 1.0);
/// ```
pub fn wrap_angle(t: f64) -> f64 {
    if !t.is_finite() {
        return t;
    }
    let t = if (0.0..core::f64::consts::TAU).contains(&t) {
        t
    } else {
        t.rem_euclid(core::f64::consts::TAU)
    };
    if t >= core::f64::consts::TAU { 0.0 } else { t }
}

/// The end of one whole period from `lo`: `lo + period`, stepped down to
/// the representable value below when that sum rounds up, so that
/// `end - lo <= period` holds exactly. A closed edge spans one period and
/// no more (`docs/DATA-MODEL.md` §Invariants, E1), and for a
/// `lo` that is not a small multiple of the period the sum can round to
/// one unit in the last place too far; this is the range's construction,
/// not a tolerance. A non-finite argument, or a `period` that is not
/// positive, comes back as `lo + period`.
///
/// ```
/// use arris_math::period_end;
/// use core::f64::consts::TAU;
///
/// assert_eq!(period_end(0.0, TAU), TAU);
/// // A pave one unit in the last place below a full turn: the sum
/// // rounds up, and the end is stepped back to keep the turn one turn.
/// let lo = f64::from_bits(TAU.to_bits() - 1);
/// assert!(lo + TAU - lo > TAU);
/// assert!(period_end(lo, TAU) - lo <= TAU);
/// ```
pub fn period_end(lo: f64, period: f64) -> f64 {
    let mut end = lo + period;
    if !(end.is_finite() && period > 0.0) {
        return end;
    }
    while end - lo > period {
        end = f64::from_bits(end.to_bits() - 1);
    }
    end
}

/// A position in 3D. `nalgebra::Point3<f64>` (ADR-0001).
pub type Point3 = nalgebra::Point3<f64>;
/// A displacement or direction in 3D, of any length.
/// `nalgebra::Vector3<f64>` (ADR-0001).
pub type Vec3 = nalgebra::Vector3<f64>;
/// A direction in 3D: a [`Vec3`] of unit length, guaranteed by construction.
/// `nalgebra::Unit<Vector3<f64>>` (ADR-0001).
pub type UnitVec3 = nalgebra::Unit<Vec3>;
/// A position in a surface's (u, v) plane. `nalgebra::Point2<f64>`
/// (ADR-0001).
pub type Point2 = nalgebra::Point2<f64>;
/// A displacement in the (u, v) plane. `nalgebra::Vector2<f64>` (ADR-0001).
pub type Vec2 = nalgebra::Vector2<f64>;
/// A direction in the (u, v) plane: a [`Vec2`] of unit length.
/// `nalgebra::Unit<Vector2<f64>>` (ADR-0001).
pub type UnitVec2 = nalgebra::Unit<Vec2>;
/// A 3x3 matrix, column-major: a rotation, or a tensor such as the
/// inertia of a body. `nalgebra::Matrix3<f64>` (ADR-0001).
pub type Matrix3 = nalgebra::Matrix3<f64>;

#[cfg(test)]
mod tests {
    use super::*;
    use core::f64::consts::{PI, TAU};

    #[test]
    fn wrap_angle_lands_in_the_half_open_turn() {
        assert_eq!(wrap_angle(0.0), 0.0);
        assert_eq!(wrap_angle(-1e-300), 0.0);
        assert_eq!(wrap_angle(-1.0), TAU - 1.0);
        assert_eq!(wrap_angle(PI), PI);
        assert!(wrap_angle(-f64::EPSILON) < TAU);
        assert_eq!(wrap_angle(3.0 * TAU + 1.0), 1.0);
        assert_eq!(wrap_angle(-3.0 * TAU - 1.0), TAU - 1.0);
        assert!(wrap_angle(f64::NAN).is_nan());
        assert_eq!(wrap_angle(f64::INFINITY), f64::INFINITY);
    }
}
