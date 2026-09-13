//! Dev-facing tools of the Arris kernel: the deterministic text dump, the
//! software rasteriser that renders a mesh to a PNG the agent can read, the
//! samplers that turn a curve or a surface into polylines for it, the
//! fixture loader, the corpus runner and the seam to the Open CASCADE oracle, and the property-test configuration
//! and strategies.
//!
//! Guarantees: this is the only crate in the workspace that writes files,
//! and it is a dependency of the workspace's tests, never of a consumer
//! (`docs/ARCHITECTURE.md` §Formats and tools). The `rerun` feature
//! streams a body to a Rerun viewer for the human.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod body;
pub mod corpus;
pub mod dump;
pub mod fixtures;
pub mod geom;
pub mod oracle;
#[cfg(not(target_arch = "wasm32"))]
pub mod prop;
pub mod render;
#[cfg(feature = "rerun")]
pub mod rerun;
pub mod sample;
#[cfg(not(target_arch = "wasm32"))]
pub mod testing;

pub use body::{
    DebugMeshError, DomainError, RENDER_CHORD_FRACTION, RenderBodyError, mesh_of, render_body,
    render_domain,
};
pub use dump::{dump_text, euler_line};
pub use geom::{polyline_of, wireframe_of};
pub use render::{Highlight, Raster, RenderError, View, render, render_png};
