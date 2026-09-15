# Idea: c2-facade-decisions

- Status: Accepted, next `/plan c2-facade-decisions`. Decided by the agent
  on the human's delegation, 2026-09-15.
- Raised: 2026-09-15
- Prompt (verbatim from the human): "decide yourself on options in facade
  idea, I just remind you our goal is a parasolid-grade CAD kernel that
  can be used by any downstream project (e.g. robocad and/or python
  binding lib)"

## Problem

C2 carries three `⚠ OPEN`s that `SEED.md` §10.7 named at kickoff and
`docs/ROADMAP.md` §C2 closes "each an ADR with the consumer's adapter as
the test". All three shape public API every downstream project compiles
against:

1. **Compaction** (ARCHITECTURE §The model): does `Model::retain` renumber
   slots, or keep them sparse as it does today?
2. **The tessellation boundary** (ARCHITECTURE §Threading): does `TriMesh`
   offer `f32` positions, or leave the cast to the consumer?
3. **Origin names** (ARCHITECTURE §How a consumer's kernel facade maps on,
   DATA-MODEL §Provenance): does Arris ship the origin-name grammar as a
   helper, or leave it to the consumer?

The first consumer is the test, not the specification. The decisions are
taken for any downstream project: a parametric CAD behind a facade, or a
language binding whose objects hold kernel handles. How the first consumer
works today, read from its facade:

- **One model per app session**, per evaluator thread. Shapes are held by
  a content hash; after each plan it calls `retain` with the keep set,
  explicitly. A plan in flight must not invalidate the previous plan's
  handles. **No kernel id is ever persisted**: saved files and undo hold
  name strings.
- **Every mesh consumer wants `f64`.** Its render mesh is `f64` positions
  **and per-vertex normals**, `u32` indices, per-face and per-edge ranges,
  and **faces that share no vertex**, since picking ids are per vertex.
  The cast to `f32` happens once, at GPU upload. Mass properties and STL
  export re-tessellate at their own tolerance, in `f64`.
- **Its names are its own vocabulary**: feature keys, a `Pattern` op the
  kernel does not have, persistent sketch-segment keys, `FromA`/`FromB`
  with `.Split(k)`, `Instance(k)`, and edge and vertex names derived from
  face pairs. The strings are stored in saved documents under a schema
  version. About 1,600 lines. Provenance replaces its geometric matcher
  and backend face-order code; the grammar and parser (~700 lines) stay
  its own.

## Constraints it runs into

- `SEED.md` §9: shapes immutable, ids generational, provenance a contract;
  §4 and §7: pure Rust, wasm-capable, a library any CAD builds on.
- `.agents/rules/kernel.md`: "f64 everywhere inside; f32 only where a
  consumer asks for it"; deterministic ids.
- Layers: a naming helper in `arris-topo` may speak only of `Origin`,
  `Role` and `Relation`.
- The native format (DATA-MODEL §Native format) stores freed slots, so a
  model read back mints the same ids.
- `docs/BACKLOG.md`: "Per-vertex normals and (u, v) in `TriMesh`, … once a
  consumer's renderer asks for either". The first renderer asks for
  normals.

## Options

### 1 — Compaction

**1A — Sparse, as today.** Freed slots are refilled lowest first at a bumped
generation, so a stale handle fails to resolve instead of aliasing. The
arena never exceeds its peak live size. A dense copy needs no new API:
`Model::import` into a fresh model returns the renumbered body with its
`IdMap`.

**1B — Renumber on `retain`, returning a mandatory `IdMap`.** Denser
storage, but every surviving handle changes under every holder: a facade's
shape store, a mesh cache's face tags, a handle in flight on another
thread, a Python object wrapping an id. Each holder must remap or silently
point at the wrong entity.

### 2 — The tessellation boundary

**2A — `f64` only.** The consumer casts where its GPU wants it.

**2B — An `f32` accessor on `TriMesh`.** A copy that neither the first
renderer nor a numpy-backed binding (whose default is `float64`, and which
can view the `f64` buffer without copying) wants.

**Adjacent: normals, (u, v) and face-local vertices.** Arris shares edge
positions between faces (`tessellate.rs`), which a watertight consumer
needs (STL, mesh volume), and has no normals or (u, v). A renderer needs a
normal per face corner, and texture mapping or CAM needs (u, v) per corner.
A consumer can only recover either by projecting every position back onto
its surface. The CDT already has each point's (u, v) and evaluates the
exact surface normal there for free.

### 3 — Origin names

**3A — No grammar in Arris; a guaranteed order instead.** Each consumer maps
`Provenance` onto its own names:
- `Role::Extrude(Side)` to a side name;
- `Modified` from operand A or B to its operand wrapper;
- several outputs from one origin to a split index;
- `transform`'s one-to-one record to an instance index;
- a blend face `Generated` from an edge to a blend name.

What only the kernel can give is **the order of the pieces modified from
one origin**, deterministic and unchanged under a parameter edit that does
not change the piece count. Today it is deterministic, but nothing states
or tests it as stable: `provenance/bolt-pattern-rebuild` holds each hole
wall's origins chain, not a split order.

**3B — `arris-topo::naming`: a rendered grammar with a parser.** Arris would
own a persisted string format, and application words (feature keys,
patterns, sketch ids) it cannot know. The first consumer's strings already
live in saved documents, so it would translate anyway.

**3C — A structured lineage value, no strings.** `Provenance::lineage(output)`
returns the chain as a typed tree down to `Role`s, which a consumer renders.
It is 3A's adapter logic moved into the kernel. It still needs 3A's order.

**The Parasolid-grade mechanism, for later:** a kernel that serves
consumers with no naming scheme of their own does it with **attributes**
that operations propagate by declared rules (kept on modify, copied or
dropped on split, dropped on delete), as Parasolid's attribute definitions
do. Provenance is the record those rules would be computed from. It is a
cycle of its own, not a C2 helper.

## Decision

Taken for any downstream project, not for the first adapter's convenience.

1. **1A: sparse, never renumbered.** Any handle any consumer holds either
   resolves to the entity it named or fails loudly; no other rule is safe
   when the holders include threads and foreign-language objects.
   Parasolid's tags likewise do not change while the session lives.
   Density is a storage concern, served by `Model::import` into a fresh
   model and by the native format.
2. **2A: `f64` only.** Precision is the kernel's; the cast is the
   renderer's, one line at upload, and a binding exposes the `f64` buffer
   without a copy. The `f32` half of the backlog line closes.
3. **3A: no grammar, a stable split order.** A name grammar is an
   application's persisted format, and a kernel for any downstream project
   owns none. The kernel owns what no consumer can reconstruct: the split
   order, defined by the origin face's own (u, v) rather than any centroid
   sort, and tested across variants. 3C waits for a second consumer asking
   for it. Attribute propagation is deferred as a backlog line, for the
   cycle that serves consumers without their own names.
4. **Normals and (u, v) per face corner: yes**, with the watertight shared
   position buffer kept. Both come from the surface at the CDT's own
   (u, v), which only the kernel has, and both serve every renderer, CAM
   tool or binding alike. They belong in the plan for C2's queries and
   export line, with STL and OBJ beside them. Whether they are stored per
   corner or as a split vertex view is that plan's design delta. The
   backlog line's remaining half closes with it.

**What would reopen these:**
- A consumer that must serialise kernel ids densely without an import
  would reopen 1.
- A wasm consumer handing meshes to JS as typed arrays, where an `f32`
  copy in Rust beats a JS-side loop, would reopen 2, as an accessor, never
  as the stored type.
- A split order that cannot be made stable by (u, v) for seam-crossing or
  curved faces would make 3 a per-piece key in the record instead of an
  index. The plan's first finding would show it.

ADRs: one each for 1, 2 and 3, since each closes a kickoff `⚠ OPEN`. The
plan is about four steps: the three ADRs, then the split-order guarantee
with its `provenance/` fixture. Decision 4 is a delta for the queries and
export plan.
