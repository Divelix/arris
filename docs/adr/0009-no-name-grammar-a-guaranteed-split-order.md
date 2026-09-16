# ADR-0009 — Arris owns no name grammar: a consumer names from `Provenance`, and the kernel guarantees the split order

- Status: accepted (2026-09-15)
- Plan: `c2-facade-decisions` steps 1 and 2 (the face rule, the edge rule
  and their proofs); the property test lands in step 3 under this decision
- Closes: the kickoff `⚠ OPEN` on the origin-name helper (`SEED.md`
  §10.7, `docs/ARCHITECTURE.md` §How a consumer's kernel facade maps on,
  `docs/DATA-MODEL.md` §Provenance)

## Context

Every downstream project needs persistent topological names: a fillet
selected on an edge must find the same edge after a dimension changes.
The kernel's answer since ADR-0002 is `Provenance`: every operation
records which output came from which input and how, chains compose with
`then`, and every chain ends at a `Role`. What was left open is whether
Arris also ships the *grammar* that turns a chain into a name.

Read in the reference trees: FreeCAD's element maps build names on a
history exactly as a consumer of `Provenance` would, and the words in
them are the application's (feature keys, sketch geometry ids); Open
CASCADE's `BRepTools_History` gives `Generated` and `Modified` lists with
no stated order; Parasolid serves consumers that have no naming scheme of
their own with attributes that operations propagate by declared rules.
The first consumer's names are its own vocabulary — feature keys, a
pattern operation the kernel does not have, `FromA`/`FromB` with
`Split(k)`, `Instance(k)`, edge and vertex names derived from face pairs
— stored in saved documents under a schema version. A second consumer, a
language binding, would want none of those words.

One thing in such a name no consumer can produce: **`Split(k)`**, the
index of a piece among the pieces of one origin. Its meaning is the
kernel's to define, because only the kernel knows the pieces before they
have ids. Before this decision the lists were ascending by id, and ids
follow the assembly — shell by shell, lump by lump, the pieces of a face
in the order the split's walk found them. On the plan's fixtures that
order held across every variant, because the walk and the lump order are
combinatorial, but nothing stated it, nothing tested it, and a change to
either would have moved every consumer's names silently.

## Decision

**Arris ships no name grammar, no parser and no lineage value.** A
consumer maps `Provenance` onto its own names — a `Role` to its part
name, `Modified` from an operand to its operand wrapper, several outputs
of one origin to `Split(k)`, `transform`'s one-to-one record to an
instance, a blend face `Generated` from an edge to a blend name.

**The order of an origin's outputs is a contract.** `generated_from` and
`modified_from` return the outputs in the order the operation added
them, deduplicated; every operation adds pieces in **split order**; two
records are equal only when every origin's outputs agree in order;
`mapped` keeps the order; `then` nests it — the outputs standing for
piece `i` of an origin (what the next operation generated from it, then
its pieces, then the piece itself when it stays) come before those of
piece `i + 1`, so `Split(k)` composes through a chain.

**A face's pieces ascend by their boundary key, then by (u, v).** The
boundary key of a piece is the sorted, deduplicated set of origins of its
boundary edges as the record names them: an operand edge for a piece of
one, both faces of the pair for a section edge. Every such origin is an
input of the operation, whose ids a rebuild of the same upstream recipe
repeats, so the key reads no output id and no geometry, and two pieces
bounded by different entities compare the same way under every edit that
keeps which entities bound which piece. Two pieces with equal keys — a
tie — are ordered by a point strictly inside each in the origin face's
own (u, v), `u` first, coordinates within the model's parametric
tolerance counting as equal, so that rounding in the point a symmetric
split makes alike never orders them. A `Generated` list of pieces (a cut
tool's face surviving in several pieces) follows the same order as a
`Modified` one; a consumer names both.

**An edge's pieces ascend along its curve**, a closed edge's from its
range's start; the section edges one face pair generates are ordered
along their own curve. Edges of one origin on different curves are not
compared: a face origin pairs with several faces of the other operand,
and the pairs' own order separates them.

**Attribute propagation is deferred**, as a backlog line: a kernel
serving consumers with no naming scheme of their own does it with
attributes that operations carry by declared rules, computed from the
record this decision fixes the order of. It is a cycle of its own.

## Evidence

Four `provenance/` fixtures, each the same recipe under four parameter
sets that move, resize and turn a split without changing which entities
bound which piece; `crates/arris/tests/provenance.rs` gives each piece a
signature — the roles, composed through the whole recipe, of the faces
it shares an edge with — and holds piece `k` of every split origin to the
same signature in every variant:

- `split-bar-cut`: a bar through a box, two lumps, four faces split in
  two, the bar moved to either side of the centre and narrowed.
- `split-frame-cut`: the same bar through a rectangular frame, so each of
  the bar's X faces survives in two pieces `Generated` from one tool
  face.
- `split-cylinder-seam`: a slab through a rod along its axis, turned
  about it within (0°, 180°) and narrowed.
- `split-cross-common`: the Steinmetz solid at three radii and lengths and
  two turns of the tool about its axis.

`split-edge-notch` (the plan's step 2) proves the edge rule: two notches
cut one after the other into one box edge, the first splitting it in two
and the second splitting one of the halves, so the two cuts composed list
three pieces of one edge — a list no single step makes and the nesting of
`then` produces. Its variants slide the notches along the edge without
swapping them and narrow them. Beside the signature test, which for an
edge reads its two end vertices' origins, the edge order is read
geometrically: edges of one origin that lie on one curve ascend by their
range on it, on this fixture and on the four face fixtures, whose tool
faces also generate section edges — the frame cut's pair of faces meeting
in a line the window's edges cut generates two.

A finding for the edge rule, the counterpart of step 1's. **Within one
operation the id order and the curve order coincide today**, over the
whole corpus: the boolean cuts an operand edge at its paves, which are
ascending by parameter, and assembles the pieces in an order that has so
far agreed. They part under composition, which is where the notch fixture
separates them — the piece the first cut left untouched has a lower id
than the two the second cut made from the other half and comes last,
because it lies last along the edge's curve. So no record and no dump
moved when the rule was stated; what moved is that it is now stated,
tested against the geometry, and free of the assembly.

Two findings shaped the face fixtures. **A piece never contains a seam.** The
split's arrangement keeps a periodic face's seam as a boundary, so a wall
cut into two regions with the seam inside one of them is three faces on
both sides of the oracle, and an edit that moves the split across the
seam changes the piece count — which no order survives, and which the
rule therefore need not survive. **The tie exists**, and only on a closed
face: the two pieces on either side of a seam are bounded by the same
origins (rims, seam, the same cut face), on both cylinder fixtures. The
test tells tied pieces apart by which side of the origin face's own frame
they lie on, the frame a transform carries with the face, and that side
holds in every variant. The corpus runs every variant against Open
CASCADE at every stage.

## Consequences

- **A change of public semantics** in `arris-topo::Provenance`:
  `generated_from` and `modified_from` are in split order, not ascending
  by id; `PartialEq` is order-sensitive. No id and no dump changes, since
  the assembly is untouched. Against the former id order, checked over
  every step of every corpus fixture: outputs of different kinds from one
  origin (a tool face's wall piece, then the section edges and vertices
  generated from it) now come in the operation's recording order rather
  than vertices first; among outputs of one kind, eight boolean records
  reorder — the pieces of a face split across two lumps (`split-cut`,
  the frame cut), where the id order was the lumps', and the three
  pieces of each Steinmetz wall — and every other record, the bolt
  pattern's chains included, keeps its order. A blend's `Generated` list
  from an edge is one-to-one by kind and now reads in the blend's own
  recording order.
- A consumer's `Split(k)` is stable under a parameter edit that keeps
  which entities bound which piece. What can move it: an edit that
  changes the piece count or which entities bound a piece — a cut moved
  across a seam, a hole reaching an edge — and, for two *tied* pieces, an
  edit that carries one piece's interior point past the other's in the
  face's own `u`, which on the fixtures means turning a symmetric split
  by half a turn. Both are the consumer's to detect through the
  signatures this ADR's test uses, and neither is something a name
  grammar in the kernel would have hidden.
- The boolean sorts each face's pieces once, by a key of a handful of
  handles; nothing in the assembly or the ids changes.
- `docs/DATA-MODEL.md` §Provenance states the split order; the
  `⚠ OPEN` on consumer references closes; the facade row in
  `docs/ARCHITECTURE.md` says the consumer names from `Provenance`.

## Alternatives considered

- **`arris-topo::naming`, a rendered grammar with a parser.** Arris would
  own a persisted string format and the application words it cannot know;
  the first consumer's strings already live in saved documents, so it
  would translate anyway, and a binding would want a different format.
- **A structured lineage value** (`Provenance::lineage(output)`, a typed
  tree down to `Role`s). It is the adapter's logic moved into the kernel
  and still needs this order; it waits for a second consumer asking for
  it, as a backlog line.
- **A (u, v) order alone.** Flips whenever a piece crosses its face's
  seam or a symmetric split's interior points swap by rounding; the
  boundary key takes every case it can, and (u, v) only the rest.
- **A per-piece key in the record** instead of an index, the idea's
  reopen condition. The boundary key is exactly that key; keeping it as
  the order rather than a stored value keeps `Provenance` as it is, and
  the gate did not fire: the key holds on every variant whose pieces are
  not tied.
- **Ascending by id, as today, stated as the rule.** It held across the
  variants of every fixture, but it is the assembly's order — shells,
  lumps, the walk — and a change to any of them for other reasons would
  move every consumer's names; the pieces of a face split across two
  lumps came in lump order, which nothing about the face determines.
