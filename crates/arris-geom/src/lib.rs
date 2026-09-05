//! Geometry of the Arris kernel: analytic and NURBS surfaces and curves with
//! the parametrisations of `docs/02-data-model.md` §Geometry, their
//! evaluation and derivatives, point projection, the curve/surface and
//! surface/surface intersections, and the pcurves of curves on planes and
//! cylinders.
//!
//! Guarantees: `Surface`, `Curve` and `Curve2` are exhaustive enums, so a
//! new variant fails every dispatch to compile until it is handled; a pair
//! without a closed form is an explicit unsupported arm, never a wildcard.
//! Evaluation never panics and never allocates (a NURBS degree is bounded
//! by [`MAX_DEGREE`] so its buffers live on the stack). Every query that can fail
//! returns a [`GeomError`] naming its operands; a projection with no
//! unique answer is [`GeomError::Ambiguous`], never a guessed parameter.
//! Depends only on `arris-math`.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod curve;
mod curve2;
mod error;
mod intersect;
mod intersect_curve;
mod nurbs;
mod pcurve;
mod project;
mod surface;

pub use curve::{Curve, CurveEval, CurveKind};
pub use curve2::{Curve2, Curve2Eval, Curve2Kind, Curve2Projection};
pub use error::{AmbiguousLocus, GeomError, GeomKind};
pub use intersect::{SurfaceIntersection, intersect_surfaces};
pub use intersect_curve::{CurveSurfaceHit, CurveSurfaceIntersection, intersect_curve_surface};
pub use nurbs::{
    FitError, MAX_DEGREE, MAX_FIT_SPANS, NurbsCurve, NurbsCurve2, NurbsSurface, fit_curve2,
};
pub use pcurve::{PCURVE_FIT_DEGREE, PCURVE_SAMPLES, pcurve_on, project_to_plane};
pub use project::{CurveProjection, SurfaceProjection};
pub use surface::{Surface, SurfaceEval, SurfaceKind};
