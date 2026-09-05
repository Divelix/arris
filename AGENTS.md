# Arris

A B-Rep geometric kernel in pure Rust: analytic and NURBS geometry,
tolerance-carrying topology in an arena, booleans with provenance,
tessellation, STEP. A library: any CAD, CAM or simulation tool can build on it; the first
consumer swaps it in behind a kernel facade once it passes that
application's probe corpus.

Read in this order: `SEED.md` (charter, competition, stack, the decisions
taken at kickoff), `docs/01-architecture.md`, `docs/02-data-model.md`,
`docs/03-roadmap.md`, `docs/adr/`, then the rules in `.agents/rules/*.md`
(git, docs lifecycle, kernel) — Claude Code loads them automatically via
`.claude/rules`; any other agent reads them here. Skills for the idea →
plan → work → retire → close-cycle pipeline, and `inspect` for seeing
geometry, live in `.agents/skills/` (same symlink arrangement).

## Setup (once per clone)

```sh
git config core.hooksPath .githooks   # fmt, clippy -D warnings, test, doc before every commit
```

## Current state

**M0 done (2026-09-05).** The harness exists and no geometry does: the
layered workspace with the layer rule checked; `Precision`, ids, handles
and `Orientation`; the checker's `Violation` list drift-tested against
the data model; `TriMesh` and the PNG rasteriser; the Open CASCADE oracle
under `tools/oracle/`; the fifteen-fixture C1 corpus with oracle values
and a lint. `cargo test --workspace` and the oracle self-test are green.
**Next:** the human tags `m0`, then `/plan m1-geometry`.

## Rules that are not derivable from the code

- Lower crates never name upper crates' types: `math` ← `geom` ← `topo` ←
  `check` ← `ops`/`mesh`/`io` ← `debug` ← `arris`.
- No panics on geometry; typed errors naming entities. The checker runs
  after every operation in debug builds. Every operation returns
  provenance. Deterministic ids and iteration. Details:
  `.agents/rules/kernel.md`.
- Every failure becomes a fixture with an oracle value; acceptance is a
  corpus run, never a picture. The agent looks at geometry itself —
  `inspect` skill. Never ask the human to describe a shape.
- Decisions go in `docs/adr/`; `⚠ OPEN:` in a doc marks a deferred one.
  Kickoff decisions are in `SEED.md` §9 and are not re-litigated.
- Update this file's "Current state" when a milestone lands; keep it under
  ~15 lines — the roadmap holds the detail.
- Backlog line → `/idea` (brainstorm, `docs/ideas/`) → `/plan` (todo,
  `docs/plans/`) → `/work` (one step, one commit) → `/retire-plan` (docs
  updated, plan deleted); `/close-cycle` at a roadmap boundary. Not every
  idea becomes a plan. Details: `.agents/rules/docs-lifecycle.md`.
- Trunk-based git, `main` always green, commit per plan step, never push
  or publish unasked: `.agents/rules/git.md`.
- Reference trees (truck, monstertruck, Fornjot, Open CASCADE, FreeCAD,
  Rerun, and the application-side projects the requirements come from) are
  read-only and never copied, never `path =` deps. Where they live on this
  machine and what each is good for: `docs/notes/reference-trees.md` —
  gitignored and machine-local, so a fresh clone has to ask the human for
  it. Tracked docs say "the reference trees", never a path.
