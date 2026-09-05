---
name: idea
description: Turn a raw feature idea or design question from the human into a brainstorm document under docs/ideas/ — problem, options with trade-offs, cost, conflicts with existing decisions, a recommendation, and the decision the human has to make. Use when the human says "idea:", "what if", "should we", "think about", or hands over a backlog line; also when a plan request is too vague or contested to plan directly. Produces analysis only — never code, never a plan, never checkboxes.
argument-hint: <idea in one sentence, or a backlog line>
---

# /idea — brainstorm, don't plan

An idea is where thinking happens so a plan doesn't have to. Not every idea
becomes a plan; a clear "no, because…" is a successful outcome.

## Do

1. Pick a slug (`kebab-case`, noun phrase). If `docs/ideas/<slug>.md` exists,
   extend it rather than duplicating.
2. Read what constrains the idea before forming an opinion: `SEED.md`
   (non-goals, differentiators), the design doc(s) it touches, every ADR
   whose decision it might bend, `docs/03-roadmap.md` for which milestone it
   would belong to, and the code if it exists. Cite paths and ADR numbers.
3. Write `docs/ideas/<slug>.md` from `docs/ideas/TEMPLATE.md`. Options are
   real alternatives with honest trade-offs, including "do nothing". Cost is
   sized in plan steps, not hours. Name what the idea *conflicts* with — a
   non-goal in `SEED.md`, an ADR, a layer rule — explicitly.
4. End with one recommendation and the concrete decision(s) the human must
   make, phrased as questions with your preferred answer first.
5. If the idea came from `docs/BACKLOG.md`, remove that line.
6. Tell the human the recommendation and the decisions in the reply; the file
   is the record, the reply is the conversation.

## Don't

- No implementation steps, no checkboxes, no file lists — that is `/plan`.
- Don't create an ADR; an idea *proposes* that one may be needed.
- Don't touch code or design docs.
- Don't write more than the decision needs. Two pages is long.

`$ARGUMENTS`
