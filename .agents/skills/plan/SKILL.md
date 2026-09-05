---
name: plan
description: Write an executable plan under docs/plans/ from the template — commit-sized checkbox steps, acceptance corpus, design deltas including public-API changes, docs-to-update list — for a feature, refactor, or milestone. Use when the human says "plan", "let's do", "implement", names a milestone ("plan M0"), or accepts an idea's recommendation. Reads the idea file if one exists and absorbs it. Never starts executing.
argument-hint: <slug | milestone | accepted idea slug>
---

# /plan — from decision to todo

A plan is a todo the agent can execute step by step with a commit per step,
and the human can read in two minutes.

## Do

1. **Check the limit.** `ls docs/plans/` — two active plans (excluding
   `TEMPLATE.md`) means stop and ask which to retire first.
2. **Gather.** Read `docs/03-roadmap.md` for the milestone's in/out/accept
   lists, the design docs the work touches, the relevant ADRs,
   `.agents/rules/kernel.md`, and `docs/ideas/<slug>.md` if the plan comes
   from an idea. If it does, the plan's *Goal* and *Design deltas* absorb
   the idea's decision and recommendation, and the idea file is deleted in
   the same change (git keeps it; the plan header cites it as
   `Idea: docs/ideas/<slug>.md (absorbed)`).
3. **Write `docs/plans/<slug>.md`** from `docs/plans/TEMPLATE.md`:
   - *Goal* is one paragraph of what is true when done; *Non-goals* fence the
     scope.
   - *Design deltas* name each design doc section, public type and
     signature, and crate boundary that changes. A non-obvious decision
     becomes an ADR — list it as a step. A `⚠ OPEN:` in a design doc that
     this plan closes is listed here with the ADR that closes it.
   - *Steps* are commit-sized: each has an observable result and its own
     test, fixture or oracle comparison, and could be reverted alone. Order
     them so the riskiest unknown retires first — for kernel work that is
     almost always the geometric case nobody has proven yet, not the API
     around it. Numbering is the checkbox order.
   - *Acceptance* is an executable corpus run, ideally the milestone's own:
     which fixtures, which oracle values, which property tests.
   - *Docs to update* is written now, while the deltas are fresh — it is what
     `/retire-plan` executes. Rustdoc on new public items is part of the
     steps, not of this list.
   - *Open questions* carry `⚠ OPEN:` items, each with who decides and by
     which step.
4. Reply with the step list and the open questions, then **stop**. The
   human edits the plan before `/work` starts it.

## Don't

- Don't execute a step. Don't create branches. Don't write code.
- Don't re-argue the idea's decision in the plan; link the ADR instead.
- Don't plan past the milestone's "out" list; put the overflow in
  `docs/BACKLOG.md`.
- Don't plan an algorithm without its fixtures: a step that adds a boolean
  case and no fixture with an oracle value is two steps missing one.

`$ARGUMENTS`
