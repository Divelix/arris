//! Topology of the Arris kernel: the `Model` arena, typed generational ids,
//! `Shape` handles as id plus orientation, the entities of
//! `docs/02-data-model.md` §Topology, pcurves, per-entity tolerances, Euler
//! operators, deterministic adjacency and iteration, and `Provenance`.
//!
//! Guarantees: entities are immutable and the arena is append-only; ids are
//! allocated in creation order and iteration order is the same on every
//! platform. The `serde` feature (on by default) derives the native format's
//! encoding. Depends on `arris-geom` and below.
#![forbid(unsafe_code)]
#![warn(missing_docs)]
