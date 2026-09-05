---
name: retire-plan
description: Close a completed plan — verify every step is ticked and the acceptance test passes, execute its docs-to-update list, run a drift check on the design docs it touched, update the roadmap status line and AGENTS.md current state, delete the plan file, commit. Use when the human says "retire", "close the plan", "plan is done", or after the acceptance test of a plan's last step passed and the human confirmed.
argument-hint: <plan slug>
---

# /retire-plan — move the durable parts, delete the rest

Deletion is the "done" signal. Anything worth keeping was moved first.

## Do

1. Open `docs/plans/<slug>.md`. Every step ticked? Acceptance run and green
   (run it now)? If not, stop and report which.
2. Execute *Docs to update* line by line. Design docs stay present tense —
   describe the system as it now is; no "as of this plan" narrative.
   If an `⚠ OPEN:` was closed by a decision, write the ADR now and add it to
   `docs/adr/README.md`.
3. **Drift check** on every design doc the plan touched: read it against the
   code and fix every sentence that is no longer true. List what you fixed
   in the reply.
4. `docs/03-roadmap.md`: update the milestone's status line. If this plan
   completes a milestone, say so and remind the human to tag `mN`.
5. `AGENTS.md` "Current state": one milestone-level sentence, keep the block
   under ~15 lines.
6. Anything deferred from the plan goes to `docs/BACKLOG.md` as one line.
7. `git rm docs/plans/<slug>.md` and commit everything as
   `docs: retire plan <slug>` with a body listing the docs updated.

## Don't

- Don't summarise the plan into a design doc — design docs hold the design,
  not the history of how it got there.
- Don't keep the plan file "for reference"; git has it.
- Don't retire with unticked boxes by editing them to ticked.

`$ARGUMENTS`
