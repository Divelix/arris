# Docs lifecycle: backlog → idea → plan → docs

Full rules in `docs/README.md` ("Document lifecycle"). The short version:

| Artefact | Where | Is | Lifetime |
|---|---|---|---|
| Backlog line | `docs/BACKLOG.md` | one sentence | until picked up or rejected |
| Idea | `docs/ideas/<slug>.md` | a **brainstorm**: problem, options, trade-offs, recommendation, decision for the human. No checkboxes. | until decided: accepted → absorbed by a plan and deleted; rejected → one line under "Rejected" in the backlog with the reason, file deleted; parked → kept with `Status: Parked` |
| Plan | `docs/plans/<slug>.md` | a **todo**: commit-sized steps with checkboxes, acceptance test, docs-to-update list | until retired: durable parts moved to design docs / ADRs, then deleted |
| Design doc | `docs/0N-*.md` | the system as it is now, present tense | living |
| ADR | `docs/adr/` | a decision and its reasons | append-only |

- Not every backlog line becomes an idea, and not every idea becomes a plan.
  A small, obvious task skips the idea and goes straight to a plan; a large
  or contested one gets an idea first.
- At most **two active plans**. `docs/plans/` holding a third means retire
  one first.
- An idea never contains implementation steps; a plan never re-argues the
  decision its idea already made — it links the ADR if one was needed.
- Skills: `/idea`, `/plan`, `/work`, `/retire-plan` walk this pipeline;
  `/close-cycle` closes a roadmap cycle once its last plan is retired —
  drift review, the finished section compressed to its status line, the
  next section opened. One roadmap file, always; never an `04-roadmap.md`.
