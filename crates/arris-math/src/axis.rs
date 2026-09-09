//! An axis: a point and a unit direction, the plain value a primitive or a
//! revolve is placed by (`docs/ARCHITECTURE.md` §Operations).

use crate::frame::FrameError;
use crate::{Point3, UnitVec3, Vec3};

/// A point and a unit direction. A plain value, never a handle: a
/// cylinder is built from an `Axis` and numbers, and only the result
/// lives in the model. The direction is unit by construction —
/// [`Axis::new`] normalises — and that is the only invariant it carries.
///
/// ```
/// use arris_math::{Axis, Point3, Vec3};
///
/// let a = Axis::new(Point3::new(1.0, 2.0, 3.0), Vec3::new(0.0, 0.0, 5.0)).unwrap();
/// assert_eq!(a, Axis::z_at(Point3::new(1.0, 2.0, 3.0)));
/// assert!(Axis::new(Point3::origin(), Vec3::zeros()).is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Axis {
    /// A point on the axis.
    pub origin: Point3,
    /// The direction, unit.
    pub direction: UnitVec3,
}

impl Axis {
    /// The axis through `origin` along `direction`, normalised. Errors: a
    /// non-finite input or a zero direction.
    pub fn new(origin: Point3, direction: Vec3) -> Result<Self, FrameError> {
        if !(origin.coords.iter().all(|c| c.is_finite()) && direction.iter().all(|c| c.is_finite()))
        {
            return Err(FrameError::NonFinite);
        }
        let direction = UnitVec3::try_new(direction, 0.0).ok_or(FrameError::ZeroAxis)?;
        Ok(Axis { origin, direction })
    }

    /// The axis through `origin` along `+z`.
    pub fn z_at(origin: Point3) -> Self {
        Axis {
            origin,
            direction: Vec3::z_axis(),
        }
    }

    /// The point `t` along the axis from its origin.
    pub fn at(&self, t: f64) -> Point3 {
        self.origin + t * self.direction.into_inner()
    }
}
