# ADR-0017 — The application gate closes on the corpus; the swap is the consumer's

- Status: accepted (2026-09-18)
- Plan: `facade-swap` step 8
- Amends: `SEED.md` §6 "Cycle 2 — the application gate"

## Context

`SEED.md` §6 defined cycle 2 as Arris replacing the truck-derived kernel
behind the first consumer's facade. The C2 **Accept** line in
`docs/ROADMAP.md` had two halves:

- **The Arris half: the consumer's probe corpus as fixtures.** Every probe
  is a fixture in the consumer's units, matched against Open CASCADE (the
  `probe-*-m` fixtures, `boolean/enclosed-cavity`,
  `boolean/parallel-cylinders-cut`), and the revolve fixtures pass every
  stage. That half is green.
- **The consumer half: work in the consumer's repository.** Its naming
  fixtures pass through `Provenance` with no matcher, its facade compiles
  against Arris with the truck crates removed, and its twins are
  un-ignored.

Drafting the consumer's plan for the second half showed three things:

- **None of the remaining work is Arris work.** It is an adapter, a schema
  upgrade and snapshot refreshes, all under the consumer's own ADR, plan
  slots and priorities. Holding a kernel cycle open on it makes Arris's
  cadence the consumer's. The seed's own "the application is never
  blocked on Arris" asks for the opposite coupling.
- **"Covers everything the facade uses" was measured per method, not per
  case.** The consumer's backend blends edges between any faces it can
  represent. `ops::fillet` blends plane–plane and plane–cylinder pairs
  only, and refuses the rest by a typed error. The consumer's roadmap also
  reads STEP, which Arris does not yet do. Neither gap shows up in the
  probe corpus: they are cases the old backend handles, not failures it
  records. A swap would find them as regressions.
- **The kernel needs nothing from the consumer to know what the gate
  asked.** Arris names no consumer type, and the facade's requirements
  reached it as fixtures. The fixtures are the durable form.

## Decision

**C2 closes on Arris's own corpus.** Its **Accept** line keeps the Arris
half:

- the consumer's probe shapes as fixtures in its units, each against the
  oracle;
- the revolve fixtures passing every stage;
- every operation of the facade table (`docs/ARCHITECTURE.md` §How a
  consumer's kernel facade maps on) present, with its guarantees.

**The swap leaves the Arris roadmap.** The consumer takes Arris behind its
facade on its own schedule, under its own ADR, from a published release.
Its first step is to run its kernel suite against both backends. A case
the old backend builds and Arris refuses or gets wrong reaches Arris the
way every failure does: as a regression fixture with the desired
assertion (`.agents/rules/kernel.md` §Testing), then a plan.

## Consequences

- C2 can close with `/close-cycle` once `facade-swap` retires. The release
  that follows is what the consumer depends on.
- The consumer-side outline in `facade-swap` step 8 is not Arris's
  checklist any more. It is a starting point for the consumer's own plan,
  and nothing in this repository waits on it.
- Real-use surprises arrive later, as regression fixtures rather than as a
  blocking gate. The known ones go in the backlog now:
  - blends on face pairs outside ADR-0007's table;
  - a STEP reader, already a later cycle in `SEED.md` §6.
- `docs/ARCHITECTURE.md` §How a consumer's kernel facade maps on stays as
  written. It says how the facade maps, not when it swaps.

## Alternatives considered

- **Keep the swap as the gate.** It is the only test by real use, but it
  makes a kernel cycle wait on another project's plan slots, and every gap
  it would find still has to become an Arris fixture before it is fixed.
- **Drop the consumer half without recording why.** The seed's cycle 2
  would then say one thing and the roadmap another. An amendment belongs
  in an ADR.
