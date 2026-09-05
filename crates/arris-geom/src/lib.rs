//! Geometry of the Arris kernel: analytic and NURBS surfaces and curves with
//! the parametrisations of `docs/02-data-model.md` §Geometry, their
//! evaluation and derivatives, point projection, and the curve/surface and
//! surface/surface intersections.
//!
//! Guarantees: `Surface`, `Curve` and `Curve2` are exhaustive enums, so a
//! new variant fails every dispatch to compile until it is handled; a pair
//! without a closed form is an explicit unsupported arm, never a wildcard.
//! Evaluation never panics and never allocates. Every query that can fail
//! returns a [`GeomError`] naming its operands; a projection with no
//! unique answer is [`GeomError::Ambiguous`], never a guessed parameter.
//! Depends only on `arris-math`.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod curve;
mod error;
mod intersect;
mod intersect_curve;
mod project;
mod surface;

pub use curve::{Curve, CurveEval, CurveKind};
pub use error::{AmbiguousLocus, GeomError, GeomKind};
pub use intersect::{SurfaceIntersection, intersect_surfaces};
pub use intersect_curve::{CurveSurfaceHit, CurveSurfaceIntersection, intersect_curve_surface};
pub use project::{CurveProjection, SurfaceProjection};
pub use surface::{Surface, SurfaceEval, SurfaceKind};
