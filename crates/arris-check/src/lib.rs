//! The Arris invariant checker: one `Violation` per row of
//! `docs/02-data-model.md` §Invariants, a `Level` that says which rows run,
//! and a `Report` that lists every violation with the entity that violates
//! it.
//!
//! Guarantees: the checker never repairs, never panics, and reports in a
//! deterministic order. Its only workspace dependency is `arris-topo`, so no
//! algorithm crate can bypass it by accident (`docs/01-architecture.md` §The
//! checker).
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod report;
mod violation;

pub use report::Report;
pub use violation::{
    DegenerateFault, EdgeEnd, EdgeUseFault, EndMismatch, FaceFault, Level, LoopBreak, NestingFault,
    Quantity, Reference, SeamFault, ShellNestingFault, ToleranceBound, Violation, WireFault,
};
