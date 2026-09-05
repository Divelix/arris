//! Dev-facing tools of the Arris kernel: the deterministic text dump, the
//! software rasteriser that renders a mesh to a PNG the agent can read, the
//! fixture loader and oracle helpers, and the property-test configuration
//! and strategies.
//!
//! Guarantees: this is the only crate in the workspace that writes files,
//! and it is a dependency of the workspace's tests, never of a consumer
//! (`docs/01-architecture.md` §Formats and tools). The `rerun` feature
//! reserves the Rerun stream for the human.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod fixtures;
pub mod render;

pub use render::{Highlight, Raster, RenderError, View, render, render_png};
