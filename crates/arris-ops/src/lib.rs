//! Operations of the Arris kernel: primitives, planar profiles, extrude,
//! revolve, transform, the booleans, and `measure` for mass properties.
//!
//! Guarantees: every operation has the shape `op(&mut Model, inputs…) ->
//! Result<(Body, Provenance), OpError>` (`docs/01-architecture.md`
//! §Operations); it never mutates its inputs, never panics on geometry,
//! returns provenance for every entity it touched, and leaves the model as
//! it was on `Err`. In debug builds its output passes `arris-check` before
//! it is returned. The `parallel` feature reserves `rayon` inside an
//! operation; `paranoid` reserves the release-build checker run.
#![forbid(unsafe_code)]
#![warn(missing_docs)]
