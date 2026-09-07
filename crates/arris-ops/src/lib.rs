//! Operations of the Arris kernel: primitives, planar profiles, extrude,
//! revolve, transform, the booleans, and `measure` for mass properties.
//!
//! Guarantees: every operation has the shape `op(&mut Model, inputs…) ->
//! Result<(Body, Provenance), OpError>` (`docs/01-architecture.md`
//! §Operations); it never mutates its inputs, never panics on geometry,
//! returns provenance for every entity it touched, and leaves the model as
//! it was on `Err`. In debug builds its output passes `arris-check` at
//! `Level::Fast` before it is returned, and a failure there panics with
//! the report: a kernel bug, the one place a panic is allowed. The
//! `paranoid` feature runs the same check in release builds and returns
//! [`OpError::Internal`] instead. The `parallel` feature reserves `rayon`
//! inside an operation. Depends on `arris-check` and below, re-exported
//! here.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;
pub mod measure;
mod primitive;
mod transform;

pub use arris_check;

pub use error::{Fault, OpError, Reason};
pub use primitive::{primitive_box, primitive_cylinder};
pub use transform::transform;

use arris_check::arris_topo::{Body, Model};

/// The debug-build guard every operation runs on its output before
/// returning `Ok`: `Level::Fast`, a panic with the report on a failure.
/// With the `paranoid` feature a release build runs it too and returns
/// [`OpError::Internal`]. In a plain release build, nothing runs.
#[cfg(any(debug_assertions, feature = "paranoid"))]
fn verify(m: &Model, body: Body) -> Result<(), OpError> {
    let report = arris_check::check(m, body, arris_check::Level::Fast);
    if report.is_ok() {
        return Ok(());
    }
    #[cfg(debug_assertions)]
    panic!("kernel bug: an operation's output fails the checker\n{report}");
    #[cfg(not(debug_assertions))]
    Err(OpError::Internal(Fault::Checker(Box::new(report))))
}

#[cfg(not(any(debug_assertions, feature = "paranoid")))]
fn verify(_: &Model, _: Body) -> Result<(), OpError> {
    Ok(())
}
