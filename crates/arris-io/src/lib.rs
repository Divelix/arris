//! Formats of the Arris kernel: the STEP AP214 Part 21 writer (later reader)
//! and the native format, which is `serde` of the model.
//!
//! Guarantees: the native format round-trips a model to an identical text
//! dump; STEP carries the B-Rep entity subset with pcurves written out, so a
//! reader does not recompute them (`docs/ARCHITECTURE.md` §Formats and
//! tools). The `serde` feature (on by default) enables the native format.
//! Depends on `arris-check` and below, re-exported here so a crate above
//! reaches the checker and the representation through this one; never on
//! `arris-ops` or `arris-mesh`.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "serde")]
pub mod native;
pub mod step;

pub use arris_check;
