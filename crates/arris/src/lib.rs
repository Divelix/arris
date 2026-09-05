//! `arris` — a B-Rep geometric kernel written from scratch in Rust.
//!
//! This crate is the facade: it re-exports the public API of the workspace
//! crates (`arris-math` up to `arris-io`) so a consumer depends on one name.
//! Nothing lives here that does not live in a lower crate.
//!
//! Placeholder release: the workspace exists, the geometry does not yet.
#![forbid(unsafe_code)]
#![warn(missing_docs)]
