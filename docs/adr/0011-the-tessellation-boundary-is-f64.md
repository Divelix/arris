# ADR-0011 — The tessellation boundary is `f64`: the cast to `f32` is the consumer's, at its own boundary

- Status: accepted (2026-09-16)
- Plan: `c2-facade-decisions` step 5
- Closes: the kickoff `⚠ OPEN` on `f32` at the tessellation boundary
  (`SEED.md` §10.7, `docs/ARCHITECTURE.md` §Threading, §Tessellation,
  §Open questions)

## Context

`arris_mesh::tessellate` returns a `TriMesh`: `Vec<[f64; 3]>` positions,
`Vec<[u32; 3]>` triangles, and per-face and per-edge ranges keyed by
`FaceId` and `EdgeId`. Everything inside the kernel is `f64`
(`.agents/rules/kernel.md`), and the kickoff left open whether the *output*
of tessellation should also offer `f32` positions, since that is what a
GPU buffer wants.

The pull is real: a renderer uploads `f32`, and a kernel that hands it
`f64` makes it walk the buffer once more. The push back is that `TriMesh`
is not only a render buffer. The corpus measures it — every fixture holds
the mesh closed with a signed volume within `mesh_volume_rel` of the
oracle's (ADR-0003) — the property tests measure it, `arris-debug`
rasterises it, and the coming STL and OBJ writers read it. A mesh whose
positions had been rounded to `f32` would answer those questions at
`1e-7` relative, not `1e-15`, and the chord tolerance a consumer asked
for would stop meaning what it says at any scale past a few metres in
millimetres.

And there is more than one consumer. The first is an application with a
renderer; the next is a language binding, whose array protocol is `f64`
and which would have to widen again whatever was narrowed.

## Decision

**`TriMesh` stays `f64`, and the kernel ships no `f32` accessor.** The
cast belongs at the consumer's own boundary, where it knows the buffer
layout it is filling, the precision its shaders need and whether to
subtract a local origin first — which a kernel casting blind cannot do,
and which matters far more to a renderer than the cast itself. It is one
line:

```rust
let gpu: Vec<[f32; 3]> = mesh.positions().iter()
    .map(|p| p.map(|c| c as f32))
    .collect();
```

`TriMesh::positions` returns `&[[f64; 3]]`, so that line neither copies
twice nor needs anything from the kernel. `TriMesh`'s rustdoc carries it
as a doc test, so the answer is where a consumer looks.

**This is not a statement that the mesh never grows.** Per-corner normals
and (u, v) with face-local vertices are a real request from a renderer
and are on `docs/ROADMAP.md` §C2's queries and export line, beside STL
and OBJ. They add *fields*, in `f64`, and change nothing here.

## Consequences

- One position buffer, one type, one meaning. No `f32`/`f64` pair to keep
  in step, no generic parameter through `arris-mesh`, and no second code
  path for the corpus to cover.
- The mesh stays measurable: the corpus's `mesh_volume_rel` and the
  tessellation property tests keep holding it to the oracle at the chord
  tolerance the fixture asked for, and STL and OBJ will write from the
  same numbers.
- A consumer uploading to a GPU walks the buffer once more per
  tessellation. That is a map over a `Vec` at the boundary it already
  owns, and it is where a local-origin subtraction belongs anyway.
- `.agents/rules/kernel.md`'s "`f32` only where a consumer asks for it"
  keeps its shape and loses its one anticipated case: nothing in the
  kernel is `f32` today.

## Alternatives considered

- **An `f32` position buffer beside the `f64` one**, or instead of it.
  Two buffers double the memory and can drift; one `f32` buffer loses the
  measurements the corpus is built on, and makes the chord tolerance a
  fiction at large coordinates.
- **`TriMesh<T: Float>`, generic over the scalar.** It pushes a type
  parameter through `arris-mesh`'s whole surface and every consumer's
  signature to save a `map` at one boundary, and the CDT is `f64`
  regardless — the generic would be a cast wearing a type parameter.
- **`fn positions_f32(&self) -> Vec<[f32; 3]>` as a convenience.** It is
  the consumer's one line with an allocation the consumer cannot control,
  and it would have to grow a local-origin variant the moment a renderer
  needed one. Where a convenience is that thin, the example in the
  rustdoc is the better shape.
