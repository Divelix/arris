# ADR-0013 — Mesh formats live in `arris-io`, which depends on `arris-mesh`

- Status: accepted (2026-09-16)
- Plan: `queries-and-export` step 3
- Follows: ADR-0012 (the render buffer beside the watertight one)

## Context

`docs/ROADMAP.md` §C2's queries-and-export line asks for STL and OBJ
writers beside the STEP writer `arris-io` already has. `ops`, `mesh` and
`io` are declared siblings (`docs/ARCHITECTURE.md` §Crates and the layer
rule): none depends on another, all three sit at layer 4, and
`tools/check-layers.sh` enforces that as a plain edge check. STL and OBJ
each write one thing — `arris_mesh::TriMesh` — so a writer for either
needs `arris-mesh`'s types in its signature, and that is exactly the edge
the sibling rule forbids.

The decision is where the writer goes, not what it writes: `TriMesh` is
public and the geometry it names is already frozen by the time a body
reaches it, so nothing here touches tessellation itself.

## Decision

**`arris-io` gains a normal dependency on `arris-mesh`.** The chain
`math` ← `geom` ← `topo` ← `check` ← `ops`/`mesh` ← `io` ← `debug` ←
`arris` replaces the old `ops`/`mesh`/`io` tier: `ops` and `mesh` stay
siblings — neither writes a format, so neither needs the other — and
`io` moves one layer above both.

```rust
pub mod stl { /* arris_mesh::TriMesh in, bytes or text out */ }
pub mod obj { /* arris_mesh::TriMesh in, text out */ }
```

`tools/check-layers.sh`'s table gets `arris-io` its own layer number
between `arris-mesh` and `arris-debug`, which itself moves up one, and
`arris` moves up one again; the script's self-test (a scratch copy with a
forbidden edge) still has to fail, so the change is proven, not just
declared. `docs/ARCHITECTURE.md`'s crate table, its layer-chain sentence
and its "`ops`, `mesh` and `io` are siblings" paragraph are edited in the
same commit as the `Cargo.toml` edge, per `.agents/rules/git.md`: a
dependency edge is exactly the kind of change the layer rule exists to
catch drifting from its doc.

Every format a consumer looks for `arris_io` for stays under that one
name — STEP, the native format, now STL and OBJ — rather than splitting
across two crates by what each format happens to read.

## Consequences

- One new Cargo edge, `arris-io` → `arris-mesh`, `default-features =
  false` at `[workspace.dependencies]` like every internal edge; `io`
  still forwards nothing of `mesh`'s own `parallel` feature, since a
  writer walks an already-built mesh and starts no triangulation of its
  own.
- `wasm32-unknown-unknown` still builds every crate with default
  features (`arris-mesh` has none that pull in `rayon` there already, so
  the edge changes nothing on that build), and `cargo tree -p arris
  --no-default-features` stays free of `serde` — `arris-mesh` reaches no
  `serde` type regardless of this edge.
- `arris-mesh`'s own rustdoc keeps saying nothing about `arris-io`: the
  edge is one-directional and `mesh` still never names an `io` type,
  which is the lower-crates-never-name-upper-crates' rule holding on the
  new tier same as the old one.
- A future format that reads or writes B-Rep geometry directly (a second
  STEP dialect, IGES) stays a sibling of STL and OBJ under `arris-io`; a
  future format that only ever reads or writes a `TriMesh` (glTF, were it
  ever added — `docs/plans/queries-and-export.md`'s non-goals rule it out
  for now) has the same edge already open for it.

## Alternatives considered

- **The writers live in `arris-mesh`, the sibling rule kept.** `TriMesh`
  already owns its measurements (`is_closed`, `signed_volume`, `area`);
  writing it to bytes is a similar kind of operation on the same type,
  and no new edge is needed at all. Rejected because it splits "every
  format" across two crates by what each format happens to read —
  `arris_debug`'s and a consumer's mental model, and the docs, would both
  have to say "STEP and the native format are in `io`, STL and OBJ are in
  `mesh`" for no reason a caller would predict, and a future glTF writer
  that wants a `Model` reference for its own reasons (scene names, say)
  would immediately need the edge this alternative avoids for one format
  and not the next.
- **A fourth crate, `arris-mesh-io`, siblings of `ops`/`mesh`/`io`.** Adds
  a name a consumer has to learn and a fourth entry in the layer table
  for two file formats, to keep a sibling rule whose only cost was ever
  going to be paid once. The one-name-per-kind-of-thing shape the crate
  layout already has (`math`/`geom`/`topo` for representation, `check`
  for the checker, `ops`/`mesh` for algorithms, `io` for every format) is
  worth more than the rule it would preserve.
