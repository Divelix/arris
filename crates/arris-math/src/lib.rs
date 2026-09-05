//! Numeric foundation of the Arris kernel: points, vectors and unit vectors
//! over `nalgebra`, frames, intervals, exact orientation predicates over
//! `robust`, polynomial and interval-guarded root finding, and `Precision`,
//! the model-wide tolerance configuration.
//!
//! Guarantees: `f64` throughout, no allocation in evaluation, no panic on
//! any finite input, and no numeric literal standing in for a tolerance —
//! every tolerance is a `Precision` field or a named constant with a
//! comment (`.agents/rules/kernel.md`). Depends on nothing in the workspace.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod precision;

pub use precision::Precision;
