# ADR-0012 — The render buffer beside the watertight one: an optional face-local corner block on the same `TriMesh`

- Status: accepted (2026-09-16)
- Plan: `queries-and-export` step 1
- Follows: ADR-0003 (tessellation by CDT through the pcurves), ADR-0011
  (the tessellation boundary is `f64`)

## Context

`arris_mesh::tessellate` returns a `TriMesh` that is watertight by
construction (ADR-0003): a topo vertex is one mesh vertex, an edge is
discretised once and its index run is shared by every face that uses it,
so a solid's mesh is closed and its signed volume is a number the corpus
holds to the oracle. That sharing is the whole point, and it is also
exactly what a renderer cannot use for anything but positions:

- A **sharp edge** is one index run shared by two faces at an angle. One
  normal per mesh vertex there is either a lie (the average, which rounds
  a box's edges into a shaded fillet) or a choice of one of the two.
- A **seam** — a cylinder's `u = 0`, a torus's two — is one index run
  used twice by the same face, once at `u = 0` and once at `u = 2π`. One
  `u` per mesh vertex there smears the whole texture backwards across the
  last column of triangles.
- A **pole or apex** — a sphere's `v = ±π/2`, a cone's tip — is one mesh
  vertex standing under a whole fan of triangles, each of which meets it
  from a different `u`. On a cone, every one of them wants a different
  normal.

So the mesh a renderer uploads and the mesh the kernel measures disagree
about how many vertices there are, and they disagree for reasons that are
properties of the B-Rep, not of the tessellation. ADR-0011 said this
would be settled by *adding fields*, in `f64`, and left the shape of the
addition open. `docs/ROADMAP.md` §C2's queries-and-export line is where
it comes due: the first consumer's renderer asks for per-corner normals
and (u, v), and the OBJ writer needs the same two arrays to have `vt` and
`vn` to write at all.

## Decision

**One `TriMesh`, which optionally carries a second, face-local index
space beside the shared one.** The watertight buffer keeps every field
and every guarantee it has; the corner block is asked for, is `None`
when it was not, and never changes the positions, the triangles, the
`FaceRange`s or the `EdgeRange`s.

```rust
pub struct MeshRequest { pub chord: f64, pub corners: bool }
pub fn tessellate_with(m: &Model, body: Body, request: &MeshRequest)
    -> Result<TriMesh, MeshError>;

impl TriMesh { pub fn corners(&self) -> Option<&Corners>; }

pub struct Corners { /* face-local vertices, in face iteration order */ }
impl Corners {
    pub fn positions(&self) -> &[u32];       // shared position index per face-local vertex
    pub fn normals(&self) -> &[[f64; 3]];    // outward
    pub fn uvs(&self) -> &[[f64; 2]];        // the surface's own parameters
    pub fn triangles(&self) -> &[[u32; 3]];  // parallel to TriMesh::triangles
    pub fn faces(&self) -> &[CornerFace];    // parallel to TriMesh::faces
}
pub struct CornerFace { pub face: FaceId, pub vertices: Range<usize>, pub uv_box: [Interval; 2] }
```

Four things are decided here.

### The block is face-local, not welded and not per-corner

A face-local vertex is one input point of one face's triangulation: a
loop sample, or an interior lattice point. Within a face it is shared by
every triangle that uses it, exactly as the CDT produced it; across faces
nothing is shared at all. `Corners::positions` maps each one back to its
shared position, so the renderer gets its watertight neighbour for free
and the two index spaces are never out of step.

That is the granularity the *geometry* has. Welding across faces cannot
carry two normals at a sharp edge; a per-triangle-corner block (three
vertices per triangle, no index buffer) can, but it triples the vertex
count for a case — a smooth face's interior — where the CDT already knows
the corners agree, and it throws away the index buffer a GPU wants. Being
face-local also makes the seam and the pole fall out rather than needing
a rule: the seam is two coedges of the same loop, so its samples are
already two runs of face-local vertices, and they differ by exactly one
period in `u`; the pole's fan already has one face-local vertex per
triangle that touches it, each with its own `u`.

The cost is one extra vertex per face per shared position, in three
arrays, on a request. It is not a second triangulation: the CDT runs
once, and the corner block is the input it already had, recorded instead
of discarded.

### (u, v) is the surface's own, never normalised

A cylinder's `u` is an angle in radians and its `v` a length; a NURBS
face's are its knot ranges. Those are the numbers the kernel computed
with, the numbers the oracle and the corpus can check —
`surface.point(u, v)` is the corner's position, to the face's tolerance,
on every fixture — and the numbers a `project_to_plane` or a `frame_at`
speaks in. Normalising to `[0, 1]` would throw away the period, so a
seam's two copies would stop differing by one period and start differing
by one; would throw away the aspect ratio, so a texture on a long thin
face would be stretched by an amount nothing records; and would not even
be stable, since a face's (u, v) box is a property of its trim and every
boolean changes it.

A consumer that wants `[0, 1]` divides by `CornerFace::uv_box`, which the
block carries for exactly that. That is a two-line map it controls, in
the same spirit as ADR-0011's `f32` cast.

### A singular corner takes the limit from inside its own face's domain

A corner may sit where `Surface::normal` is `None`: a sphere's pole, a
cone's apex, a NURBS's degenerate corner. The rule is the **limit of the
normal approached along the parameter in which the surface still moves,
from inside the face's own (u, v) box**. Where `∂P/∂u` vanishes, the
first-order term of `∂P/∂u × ∂P/∂v` is `ε (∂²P/∂u∂v × ∂P/∂v)`, with `ε`
signed by which side of the singular parameter the face's domain lies on,
and the limit is that cross product normalised; symmetrically where
`∂P/∂v` vanishes. Nothing is divided by zero and no surface kind is
matched on: `Surface::eval` already returns the second derivatives.

It gives the two answers a renderer needs:

- **A sphere's pole is `±Z`.** The limit does not depend on the corner's
  `u`, so every corner of the fan carries the same axis direction,
  outward — `+Z` at `v = π/2` of a forward-used sphere.
- **A cone's apex is a ring.** A cone's normal does not depend on `v` at
  all, so the limit at the apex is `(cos α cos u, cos α sin u, −sin α)`
  in the frame: one normal per corner's own `u`. Shading a cone's tip
  from a single normal — the axis, or an average — leaves the dark spot
  that every welded mesh of a cone has, and face-local vertices are what
  make avoiding it free.

If the first-order term vanishes too — a NURBS whose two derivatives stay
parallel to second order — the corner takes the normal of the nearest
non-singular face-local vertex of the same face, by (u, v) distance and
then by index, so the result is deterministic. A mesh never carries a
zero, a non-finite or an absent normal; a face with no non-singular
corner at all is `MeshError::Internal`, since the checker does not admit
one.

### Normals are `f64`, and so is everything else in the block

ADR-0011's reasoning applies unchanged and for the same reason: the
corner block is *checked*, not only drawn. The corpus runner asks for
corners on every fixture and asserts that each face-local vertex
evaluates back to its shared position and carries the outward normal
there; an `f32` block would answer at `1e-7` relative and could not. The
consumer's cast is the same one line at the same boundary, and the one
place a narrowing is real — binary STL's `f32` facets — is the *format's*
own, written at the writer and computed in nothing but `f64`.

## Consequences

- `TriMesh` stays one type with one meaning. `tessellate` keeps its
  signature and every caller, the measurements keep measuring the same
  buffer, and every committed fixture dump is unchanged. `corners()`
  returning `Option` is the whole of the new state a consumer has to
  reason about.
- A renderer uploads `Corners::positions`-indexed attribute arrays and
  `Corners::triangles` as its index buffer, and gets flat shading across
  sharp edges, a correct seam and a shaded cone tip without a weld pass,
  a normal-averaging pass or a duplicate-detection pass of its own.
- The corpus gains a real invariant on every fixture rather than one
  focused test: `surface.point(u, v)` is the position and the normal is
  the outward one, on every face-local vertex of every fixture. That also
  pins the `Reversed`-use flip, which nothing else checks pointwise.
- `arris_io::obj` has `vt` and `vn` to write, and writes them with
  face-local indices, which is what OBJ's `f v/vt/vn` was for.
- The cost is paid only when asked: three arrays over the face-local
  vertices, and the CDT's input kept instead of dropped. Step 2 measures
  it with `tools/test-timings.sh`; a measurable slowdown of the corpus is
  a backlog line, not a reason to make the block unconditional or absent.
- A future smooth-shading request — averaging across a tangent edge — is
  a *consumer's* pass over this block, or a later kernel option that adds
  a field. It is not this decision, and face-local vertices are the input
  it would need either way.

## Alternatives considered

- **A second mesh type, `RenderMesh`, built beside `TriMesh`.** The two
  describe the same triangles of the same faces, so a consumer would have
  to correlate them by `FaceRange` anyway, and either the CDT runs twice
  or the type is a view wearing a struct. It also doubles the surface the
  corpus, the writers and `arris-debug` have to know about, to avoid one
  `Option`.
- **Normals and (u, v) welded onto the existing mesh vertices.** It is
  the smallest diff and it is wrong at every interesting place: a box's
  edges shade as fillets, a cylinder's last column of triangles carries a
  reversed texture, a cone's tip is black. Those are the three cases the
  consumer's renderer reported.
- **Per-triangle-corner arrays, three per triangle, no index buffer.**
  Correct, and the simplest thing to generate, but it triples the vertex
  count over a smooth face where the CDT already established that the
  corners agree, and discards an index buffer that the same CDT already
  produced.
- **(u, v) normalised to `[0, 1]` over the face's box.** Convenient for a
  texture, but it is a lossy view of the number the kernel actually has,
  it is not stable under a boolean that re-trims the face, and it makes
  the seam's one-period difference unrecoverable. The `uv_box` in the
  block gives a consumer the same view in two lines.
- **Corners always, no `MeshRequest`.** Every measurement path, every
  STEP export and the whole fixture corpus would carry the arrays for
  nothing. A request flag is one bool, and `tessellate(m, body, chord)`
  stays for the callers that only want the watertight buffer.
