# Plan: queries-and-export

- Started: 2026-09-16
- Milestone: C2 — the application gate (`docs/ROADMAP.md` §C2, its last
  unstarted line)
- Idea (verbatim from the human): "/plan queries-and-export"

## Goal

The last line of C2 that is not the facade swap itself: the queries and the
export formats a consumer needs beside the B-Rep. A `TriMesh` can be asked
for **per-corner normals and (u, v) over face-local vertices**, beside the
watertight buffer it already carries and without changing it — a seam's two
copies get their own `u`, a pole's corners their own normal, and a renderer
uploads the corner arrays and the corner index buffer as they stand.
`arris_io` writes that mesh as **STL** (ASCII and binary) and **OBJ**
(`vt`, `vn`, a group per face), beside STEP, byte-identically on every run,
and Open CASCADE reads the STL back and agrees with it. `arris_ops::query`
answers the two questions the consumer's facade asks of a body that no
operation answers: the orthogonal **projection of edges and vertices onto a
plane**, each edge's parameter range carried through its projected curve's
own parameter, and a face's **outward-oriented frame**. `measure`'s inertia
tensor is held to an independent mesh integrator — the consumer's own path
— as well as to the oracle, so the convention is proven and not asserted.

## Non-goals

- No hidden-line removal, no silhouette: projection is of the edges and
  vertices the caller names, not of what a view of the body would show.
- No readers. STL and OBJ are written, never parsed by the kernel (a
  test's own parser is a test's).
- No glTF, no `.mtl`, no colours or materials, no texture-space
  normalisation of (u, v): the surface's own parameters are written and a
  consumer that wants `[0, 1]` normalises by the face's (u, v) box, which
  the corner block carries.
- No smooth shading across a sharp edge: a corner's normal is its own
  face's, never averaged with a neighbour's. Face-local is the point.
- No adaptive refinement, no second mesh path, no `f32` anywhere the
  kernel can be asked for it (ADR-0011 stands; binary STL's `f32` is the
  format's own, at the writer).
- No mesh-based `measure`: mass properties stay an exact flux integral over
  the B-Rep, and the mesh integrator is a *test*.
- Not the facade swap, which is C2's own closing plan.

## Design deltas

**`arris-mesh` — the corner block (ADR-0012, step 1).** `TriMesh` keeps
every field and guarantee it has: positions shared, a topo vertex one mesh
vertex, closed by construction, measured against the oracle. It gains one
optional block:

```rust
pub struct MeshRequest { pub chord: f64, pub corners: bool }   // MeshRequest::new(chord), .with_corners()
pub fn tessellate_with(m: &Model, body: Body, request: &MeshRequest) -> Result<TriMesh, MeshError>;
// `tessellate(m, body, chord)` stays, and is `tessellate_with` of `MeshRequest::new(chord)`.

impl TriMesh {
    pub fn corners(&self) -> Option<&Corners>;
    pub fn with_corners(self, corners: Corners) -> Result<Self, MeshError>;  // validated
}

pub struct Corners { /* face-local vertices, in face iteration order */ }
impl Corners {
    pub fn from_parts(positions, normals, uvs, triangles, faces) -> Result<Self, MeshError>;
    pub fn positions(&self) -> &[u32];        // shared position index per face-local vertex
    pub fn normals(&self) -> &[[f64; 3]];     // outward, f64
    pub fn uvs(&self) -> &[[f64; 2]];         // the surface's own parameters
    pub fn triangles(&self) -> &[[u32; 3]];   // parallel to TriMesh::triangles, indexing the arrays above
    pub fn faces(&self) -> &[CornerFace];     // parallel to TriMesh::faces
    pub fn face(&self, face: FaceId) -> Option<&CornerFace>;
}
pub struct CornerFace { pub face: FaceId, pub vertices: Range<usize>, pub uv_box: [Interval; 2] }
pub const NORMAL_UNIT_SLACK: f64;             // how far from unit `from_parts` accepts a normal
```

`FaceRange` and `EdgeRange` are untouched. `Interval` is re-exported from
`arris-mesh` as `Aabb` already is. **One new `MeshError` variant after
all** (step 2's finding, below): `MeshError::Corners(String)` — a block
whose arrays are not parallel, whose faces are not the mesh's in order,
whose triangles are not parallel to the mesh's, whose face-local vertex
does not stand on the mesh vertex its triangle does, or a face no point
of which has a surface normal. `MeshError::Internal` keeps covering a
corner that names no input point, as today. `CornerFace::uv_box` is the
tightest box over the face's own face-local vertices, not the face's
domain box, so a consumer's `[0, 1]` normalisation uses the whole range.

**`arris-io` — mesh formats, and the layer edge (ADR-0013, step 3).**
`ops`/`mesh`/`io` stop being flat siblings: `io` gains a normal dependency
on `mesh`, so the chain is `math ← geom ← topo ← check ← ops`/`mesh ← io ←
debug ← arris`. `tools/check-layers.sh`'s table moves `arris-io` to its own
layer and `arris-debug`/`arris` up one; the self-test proves it still
catches a forbidden edge. New public surface:

```rust
pub mod stl {
    pub fn write_ascii(mesh: &TriMesh, name: &str) -> Result<String, MeshWriteError>;
    pub fn write_binary(mesh: &TriMesh, name: &str) -> Result<Vec<u8>, MeshWriteError>;
}
pub mod obj { pub fn write(mesh: &TriMesh) -> Result<String, MeshWriteError>; }
pub enum MeshWriteError { TooManyTriangles { triangles: usize }, /* … */ }
```

Reals are the shortest round-trip decimal, as STEP's are, so two writes are
byte-identical. OBJ writes `v`/`f` and one `g` per face always, and
`vt`/`vn` with face-local indices when the mesh carries corners.

**`arris-ops` — `query` (steps 6, 7).** A second query module beside
`measure`, same shape: `&Model` in, no body, no provenance.

```rust
pub mod query {
    pub enum Projection {
        Vertex { vertex: VertexId, point: Point2 },
        Edge { edge: EdgeId, curve: Curve2, range: Interval },
    }
    pub fn project_to_plane(m: &Model, shapes: &[Shape], plane: &Frame)
        -> Result<Vec<Projection>, OpError>;
    pub fn face_frame(m: &Model, face: Face) -> Result<Frame, OpError>;
    pub fn frame_at(m: &Model, face: Face, uv: Point2) -> Result<Frame, OpError>;
}
```

`project_to_plane` is `geom::project_to_plane` per edge with the edge's
range carried into the projected curve's *own* parameter — a circle whose
projection is an ellipse is shifted by that ellipse's phase, so the range
must be shifted with it or the piece is the wrong arc. A degenerate edge,
and a curve whose projection collapses, is a typed refusal naming the
entity: `OpError::Degenerate` with one of **three new `Reason`
variants** (step 6's finding, below) — `NotProjectable` for a face,
shell or body, `DegenerateEdge`, `ProjectionCollapses`. `face_frame` is the face's surface frame with `Z` the **outward**
normal — flipped, right-handedness kept, when the face's use is `Reversed`
— and is a plane's only, refused otherwise (`Reason::NotPlanar`, step 7's
finding, below); `frame_at` is any face at a (u, v) its domain contains
(`check::domain::FaceDomain`), refused off it (`Reason::OutOfDomain`) or
at a singular point where there is no normal (`Reason::Singular`).

**`.agents/rules/kernel.md`** loses "nothing in the kernel is `f32`
today": binary STL's facets are `f32` because the format says so, at the
writer, and nothing the kernel computes with is.

No `⚠ OPEN:` in a design doc is closed by this plan; the two ADRs are new
decisions, not deferred ones.

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness or
bound has to be established here.

- [x] Step 1 **[2]** — **ADR-0012: the render buffer beside the watertight
      one.** Why the corner block is an *option* on the same `TriMesh` and
      not a second mesh type; why face-local and not per-triangle-corner or
      globally welded; why the surface's own (u, v) and not `[0, 1]`; the
      rule for a corner at a singularity (the normal of the surface at the
      nearest point of the face's own domain in the degenerate direction,
      so a sphere's pole gives ±Z and a cone's apex a ring of normals, one
      per corner's `u`); why normals are `f64` (ADR-0011). Docs only.
- [x] Step 2 **[2]** — **The corner block.** `MeshRequest`,
      `tessellate_with`, `Corners`/`CornerFace`, built from the (u, v) each
      face's CDT already has, oriented by the face use, the singular rule
      of ADR-0012 applied. Tests: for every face-local vertex,
      `surface.point(u, v)` is its shared position to the face's tolerance
      and its normal is the outward normal there; a seam's two copies differ
      by exactly one period in `u`; a sphere's pole corners carry ±Z and a
      cone's apex corners the cone's own ring; the watertight buffer is
      identical to `tessellate`'s, with `parallel` on and off; over
      `sample::{cuboid, cylinder, sphere, torus, patch}` and a filleted box.
      The corpus runner asks for corners and asserts the (u, v)-evaluates-to-
      position and outward-normal invariants on every fixture, so the whole
      corpus covers it.
- [x] Step 3 **[1]** — **ADR-0013: mesh formats live in `arris-io`**, which
      therefore depends on `arris-mesh`; the sibling rule becomes a chain,
      and a format's own narrowing (binary STL's `f32`) is the format's, not
      the kernel's. The ADR, the dependency edge, the layer table in
      `tools/check-layers.sh` and in architecture, `.agents/rules/kernel.md`'s
      `f32` line. Acceptance: `tools/check-layers.sh` and `--self-test`
      green, wasm build green, `cargo tree -p arris --no-default-features`
      still free of `serde`.
- [x] Step 4 **[1]** — **`io::stl`**, ASCII and binary, with
      `MeshWriteError`. Per-facet normal from the triangle's own winding, not
      from the corner block, as the format means it. Tests: two writes are
      byte-identical; the oracle reads both back (`tools/oracle/mesh.py` over
      `RWStl`, reached through a new `arris_debug::oracle::compare_stl`
      beside `compare`) and reports the same triangle count and area, and a
      signed volume within the mesh's own bound of `measure`'s, over the
      cube, the cylinder (a seam), the sphere (poles), the torus (genus 1)
      and a filleted box.
- [x] Step 5 **[1]** — **`io::obj`**: `v`, `vt`, `vn`, `f` with face-local
      indices, one `g` per face, positions shared and corners written when
      the mesh has them. Tests: a parser in the test reads the file back and
      reconstructs the corner block exactly; every `vt` evaluates on its
      face's surface to its `v`; every `vn` is that surface's outward normal
      there; the groups partition the triangles by face id in iteration
      order; a mesh without corners writes `v`/`f`/`g` and no `vt`/`vn`; two
      writes are byte-identical.
- [x] Step 6 **[2]** — **`query::project_to_plane`** over edges and
      vertices, the range carried through the projected parameter. Tests: a
      property over every curve kind in a random pose against a random plane
      — the projected curve at `t` is the 3D curve at `t` projected, to
      1e-12·scale, at both ends of the carried range and inside it, and the
      projected range covers exactly the edge's piece and no more (the
      circle-to-ellipse phase shift is the case this retires); a degenerate
      edge, a circle projecting onto a line and a `Shape` of the wrong kind
      are each a typed refusal naming the entity.
- [x] Step 7 **[1]** — **`query::face_frame` and `query::frame_at`**. Tests:
      over the sample bodies and a boolean result, the frame's `Z` at a
      planar face agrees with the face use's outward normal — a point
      offset along it by a multiple of the face's tolerance classifies
      `Out` and the opposite offset `In` (`check::classify_point`) — and the
      frame is right-handed and unchanged by a rebuild at other parameters;
      `frame_at` on a cylinder, a sphere and a torus agrees with
      `Surface::normal` composed with the use, is refused at a pole and off
      the domain.
- [ ] Step 8 **[1]** — **Inertia against the consumer's integrator.** A test
      that integrates `tessellate`'s `TriMesh` — volume, centroid and the
      full tensor over the tetrahedra of each triangle with the origin — and
      holds it to `measure::mass_properties` within the mesh's own error, in
      random poses, over the sample bodies and a filleted box, off-diagonal
      terms included, so the physical convention (`∫(|r|²I − rrᵀ)dV`, negated
      products) is proven against an independent path and not only against
      the oracle.

## Acceptance

- `cargo nextest run --workspace` green, and the pre-commit checks (`fmt`,
  `clippy -D warnings`, `doc -D warnings`).
- The whole fixture corpus green with the runner asking for corners: every
  face-local vertex of every fixture satisfies `surface.point(u, v) ==
  position` within the face's tolerance and carries the outward normal, and
  every committed `dump.txt` is unchanged.
- `tools/oracle/mesh.py` reads Arris's ASCII and binary STL of the cube,
  cylinder, sphere, torus and a filleted box in Open CASCADE and reports the
  same triangle count and area as the mesh, and a signed volume within the
  mesh's bound of `measure::mass_properties`.
- OBJ round-trips through the test's parser to the corner block exactly, and
  every `vt`/`vn` it wrote is the surface's own at that point.
- The projection property at the configured case count over every curve
  kind, plane and pose; the face-frame property over the sample bodies and a
  boolean result; the inertia property against the mesh integrator.
- `tools/check-layers.sh`, its self-test, and the wasm build.

## Docs to update on completion

- `docs/ARCHITECTURE.md` §Crates and the layer rule — `arris-io`'s row
  (depends on `arris-mesh`), the layer chain sentence, and the "`ops`,
  `mesh` and `io` are siblings" paragraph (ADR-0013).
- `docs/ARCHITECTURE.md` §Operations — `query::project_to_plane`,
  `face_frame`, `frame_at` beside `measure` in the query paragraph.
- `docs/ARCHITECTURE.md` §Tessellation — the corner block, the singular
  rule, and the removal of "no per-corner normals or (u, v) yet".
- `docs/ARCHITECTURE.md` §Formats and tools — `stl` and `obj` entries, the
  oracle's `mesh.py`, `arris_debug::oracle::compare_stl`.
- `docs/ARCHITECTURE.md` §How a consumer's kernel facade maps on — the
  projection, face-frame and tessellation rows point at the new API.
- `docs/DATA-MODEL.md` §Pcurves — `project_to_plane`'s paragraph gains the
  carried range and its phase shift.
- `docs/ROADMAP.md` §C2 — the queries-and-export line marked **Done**, with
  what it retired and the ADRs.
- `docs/adr/README.md` — ADR-0012 and ADR-0013 in the index.
- `.agents/rules/kernel.md` — the `f32` line (binary STL's facets).
- `tools/oracle/README.md` — `mesh.py` in the script table.
- `AGENTS.md` current state — C2's last non-facade line landed.

## Open questions

None. The three this plan opened were decided at planning time, by the
human, before step 1:

- **Mesh formats live in `arris-io`**, which therefore depends on
  `arris-mesh`: every format under the one name a consumer already looks
  in, at the price of the flat sibling rule between `ops`, `mesh` and
  `io`. That is ADR-0013 and step 3 writes it; the alternative (the
  writers in `arris-mesh`, the rule kept) is the ADR's rejected one.
- **`tessellate(m, body, chord)` stays** beside `tessellate_with`: every
  caller in the workspace and the corpus uses it, and a `MeshRequest` for
  a chord alone is noise. Step 2's commit body names it as the additive
  API delta it is.
- **The corpus runner asks for corners on every fixture**, so the whole
  corpus covers the invariants rather than one focused test — the cost is
  the corner block, not a second CDT. Step 2 checks that with
  `tools/test-timings.sh` and reports the number; a measurable slowdown is
  a finding for the backlog, not a reason to re-decide mid-plan.
  **Measured (step 2): none.** `cargo nextest run -p arris --test corpus`
  is 5.88 / 5.95 / 5.88 s with the block and 5.87 / 5.91 / 5.93 s without
  it, over the 104 fixtures — the run is dominated by the oracle and STEP
  stages, and the block is inside the noise. No backlog line.

## Findings

- **Step 2 needed one new `MeshError` variant**, against the design
  delta's "none expected": a corner block that does not fit its mesh is a
  caller's error, and neither `IndexOutOfRange` nor `Internal` names it
  honestly — `Internal` says "kernel bug", which a hand-built block is
  not. `MeshError::Corners(String)` is additive and pre-1.0; the design
  delta above now carries it.
- **Step 6 needed three new `Reason` variants**, which the design delta
  had not listed: the refusals it asks for name the shape through
  `OpError::Degenerate`, and none of the existing reasons says why.
  `Reason::{NotProjectable, DegenerateEdge, ProjectionCollapses}` are
  additive and pre-1.0; the design delta above now carries them. The
  range carry is computed in `ops` from the image's own frame, so
  `geom::project_to_plane` is unchanged: its conic phase is read from the
  projected point's and tangent's local `x`, both over the major radius,
  and the property holds at 1e-12 relative over 40 000 cases.
- **Step 7 needed three more `Reason` variants**, again against a design
  delta that named the refusals in prose but not their type:
  `Reason::NotPlanar` (`face_frame` on a curved face), `OutOfDomain` and
  `Singular` (`frame_at` off the face's domain, or at a pole or apex
  `Surface::normal` has none for). All three are additive and pre-1.0;
  the design delta above now carries them. `frame_at`'s frame is built
  from `Surface::eval`'s own `∂P/∂u` as the `X` hint and
  `Surface::normal` (composed with the face's use) as `Z`
  (`Frame::new`), never `Frame::from_z`, so its axes agree with
  `face_frame`'s on a plane at every `(u, v)` rather than only its `Z`;
  `face_frame`'s `Reversed` flip negates `X` and `Z`, keeping `Y`, which
  stays right-handed by construction (`Frame::from_orthonormal`, which
  cannot fail on axes already orthonormal, so it is `?`-propagated
  rather than unwrapped, per the kernel rule against assuming
  geometry-derived data cannot fail).
