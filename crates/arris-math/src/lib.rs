//! Numeric foundation of the Arris kernel: points, vectors and unit vectors
//! over `nalgebra`, frames and rigid motions, intervals, exact orientation
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

mod frame;
mod interval;
mod isometry;
mod precision;
pub mod predicates;
pub mod roots;
mod tolerance;

pub use nalgebra;

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
