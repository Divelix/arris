# ADR-0010 — `Model::retain` keeps slots sparse: a live id never moves, a dead one never aliases

- Status: accepted (2026-09-16)
- Plan: `c2-facade-decisions` step 4
- Closes: the kickoff `⚠ OPEN` on whether compaction renumbers slots
  (`docs/ARCHITECTURE.md` §The model, §Open questions)

## Context

`Model::retain(keep)` is the only operation that invalidates handles: it
frees every entity and geometry value unreachable from `keep`, dropping
the slot's value and bumping its generation, and rebuilds the adjacency
indices. A freed slot keeps its index and is filled by a later append,
lowest index first, at the bumped generation.

What was left open at kickoff is whether it should also **renumber**: move
the survivors down into a dense arena and hand the consumer an `IdMap`
for everything that moved. A dense arena serialises smaller and iterates
without holes; a sparse one keeps every surviving id exactly as it was.

A downstream project holds ids across an evaluation: in a document, in an
undo stack, in a selection the user made before the edit, in a name a
consumer derived from `Provenance`. Those ids outlive the call that
compacts. The question is therefore not about the arena — it is about
what the kernel promises a consumer holding an id while the model changes
underneath it.

## Decision

**`Model::retain` never renumbers a slot.** Every entity reachable from
`keep` keeps its id, its generation and its place; nothing that survives
moves, and `retain` returns a count, not a map. A handle to a freed
entity is `NotFound` for ever: the slot's generation is bumped before it
can be refilled, so a later append into that slot hands out a *different*
id, and the stale handle never resolves to the entity that took its
place. **A live id never moves and a dead id never aliases** — the two
halves of the same promise.

**A dense copy is `Model::import` into a fresh model**, which returns the
new body and the `IdMap` from old ids to new. A consumer that wants a
compact arena — to serialise, to hand a subtree to another thread, to
snapshot — asks for one explicitly and gets the map with it, rather than
having every id it holds moved by a call whose stated job was to free
memory. The native format is the other dense copy: it writes the arenas
slot by slot, freed slots included, so a model read back has the same ids
and mints the same next one.

## Consequences

- A consumer's ids survive `retain`. It may compact a long-lived model on
  its own schedule without touching a selection, an undo entry or a
  persistent name; nothing it stored has to be rewritten, and there is no
  map it can forget to apply.
- A stale handle is a typed `NotFound`, never a wrong answer. This is what
  makes the generation counter worth its word in every id: a bug that
  keeps a handle past a `retain` is reported at the first use, not
  silently redirected to whatever filled the slot.
- The arena stays sparse until appends refill it. A model that frees a
  great deal and then builds little keeps the holes; the cost is index
  space, not correctness, and the native format is where a consumer that
  cares pays it off.
- `retain`'s signature stays `Result<usize, NotFound>`. Renumbering would
  have made an `IdMap` part of it, and applying that map correctly would
  have become a requirement on every consumer.
- Determinism is unaffected: ids are a function of the calls made, so two
  models built by the same sequence — a `retain` among the calls —  agree
  on every id, which `crates/arris-topo/tests/import_retain.rs` holds
  them to.

## Alternatives considered

- **Renumber into a dense arena, returning an `IdMap`.** Cheaper
  serialisation and no holes, but every surviving id changes, so applying
  the map becomes mandatory for every consumer on every compaction — and
  a consumer that stored an id in a document, a selection or a name has
  to find and rewrite all of them. It also makes `retain` a
  handle-invalidating operation for *live* entities, not only dead ones,
  which is a much larger promise to break.
- **Renumber only when the arena is mostly holes**, on a load factor.
  The worst of both: the consumer must handle renumbering anyway, and now
  cannot predict when it happens.
- **No compaction at all**, the model growing without bound. A
  long-running consumer that builds and drops bodies all day needs the
  slots back; `retain` is how it gets them without invalidating what it
  kept.
