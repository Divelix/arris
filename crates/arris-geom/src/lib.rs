//! Geometry of the Arris kernel: analytic and NURBS surfaces and curves with
//! the parametrisations of `docs/02-data-model.md` §Geometry, their
//! evaluation and derivatives, point projection, and the curve/surface and
//! surface/surface intersections.
//!
//! Guarantees: `Surface`, `Curve` and `Curve2` are exhaustive enums, so a
//! new variant fails every dispatch to compile until it is handled; a pair
//! without a closed form is an explicit unsupported arm, never a wildcard.
//! Depends only on `arris-math`.
#![forbid(unsafe_code)]
#![warn(missing_docs)]
