//! Rational B-splines: `NurbsCurve`, `NurbsCurve2` and `NurbsSurface` per
//! `docs/02-data-model.md` §NURBS — degree, knots, Cartesian control
//! points with positive weights kept apart, validating constructors,
//! evaluation with derivatives to second order into stack buffers (so it
//! never allocates), the period unclamped knots imply, and knot insertion
//! for curves. *The NURBS Book* ch. 2–5 is the reference for the
//! algorithms; the reference tree's `truck-geometry` B-spline module was
//! read for the shape of a Rust implementation and nothing was taken from
//! it: truck stores homogeneous points and builds derivative curves,
//! Arris keeps weights separate and evaluates the basis derivatives in
//! place.

mod basis;
mod curve;
mod spline;
mod surface;

pub use basis::MAX_DEGREE;
pub use curve::{NurbsCurve, NurbsCurve2};
pub use surface::NurbsSurface;
