---
name: close-cycle
description: Close a finished release cycle or milestone — verify nothing is still open, run the mandated drift review of every design doc against the code, compress the finished section of docs/03-roadmap.md to its status line, open the next cycle's section, update the spine and AGENTS.md, and hand the tag to the human. Use when the human says "close the cycle", "v0.2 is done", "milestone is done", "what's next after this release", or when the last line of a roadmap cycle has been retired. Never tags and never pushes.
argument-hint: <cycle or milestone, e.g. v0.2 — and optionally the next cycle's theme>
---

# /close-cycle — one roadmap, one section per cycle

`docs/03-roadmap.md` is a **living design doc**, not a log. It is never
forked into `04-roadmap.md` and never accumulates plans: a finished cycle
shrinks to its status line, and the next cycle is appended below it. The
numbered docs are *topics* — 01 architecture, 02 data model, 03 roadmap — so
a second roadmap file would only ever raise "which one is current?".

## Do

1. **Check it is actually finished.** Every line of the cycle's section
   either done or explicitly moved to `docs/BACKLOG.md`; `docs/plans/`
   holding nothing but `TEMPLATE.md`; `cargo fmt --check`, `cargo clippy
   --workspace --all-targets -- -D warnings` and `cargo test --workspace`
   green. If not, stop and report exactly what is open — do not close
   around it.
2. **Drift review** (`docs/README.md` mandates it at every boundary, and
   this is the only place it happens): read **every** design doc —
   `01-…`, `02-…`, `03-…` — against the code and fix each sentence that is
   no longer true. The cycle is not closed until the list is empty. List
   what you fixed in the reply; if you fixed nothing, say why you believe
   nothing had drifted.
3. **Compress the finished section** to the standard shape:
   goal, one `**Status: done <date>, tag `vN`.**` line naming the risk that
   was retired and the ADRs that were taken, then the in / out / accept
   lists. Narrative goes; `/plan` reads in/out/accept and `/retire-plan`
   writes the status line, so those four survive. Before deleting any
   sentence, grep for the fact in `docs/`, `crates/` and `README.md` — a
   measurement or a rationale that lives *only* here is relocated to the
   doc or the code comment that wants it, never dropped.
4. **Open the next section.** `## vN — <theme>` with goal, in, out and
   accept, drawn from `docs/BACKLOG.md`. The theme and the in/out split are
   the **human's call**: propose, do not decide. If the answer is not
   obvious from the backlog, stop and ask.
5. **`Spine:`** at the top of the roadmap gains the new cycle.
6. **`AGENTS.md` "Current state"**: one sentence for the closed cycle, a
   new `**Next:**`, block still under ~15 lines.
7. Commit as `docs: close <cycle>` with a body listing the docs updated and
   the drift fixed. Then **tell the human to tag** `vN.N.0` — tags and
   pushes are theirs (`.agents/rules/git.md`), never yours.

## Don't

- Don't create `04-roadmap.md`, an `ARCHIVE.md`, or a CHANGELOG. Git holds
  the history; that is the whole reason the section compresses.
- Don't leave the finished section at full length "because it is useful" —
  a cycle that keeps 40 lines is what makes the file look unmaintainable
  after three of them.
- Don't invent the next cycle's scope. A roadmap section is a commitment
  the human makes, not one the agent proposes into existence.
- Don't tag, push, or open plans for the new cycle here. `/idea` and
  `/plan` come after, one line at a time.

`$ARGUMENTS`
