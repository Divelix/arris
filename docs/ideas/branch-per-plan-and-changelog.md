# Idea: branch-per-plan-and-changelog

- Status: Open
- Raised: 2026-09-26
- Prompt (verbatim from the human, said in the plugin CAD's repository for both): "Until now we worked without branching at all, just committing and pushing to main in both [the plugin CAD] and arris, which is bad practice. I suggest change this workflow to trunk-based where each plan is separate feature branch, where retire plan skil finishes by creating mr to main. The only thing we need to think through is versioning: how to connect plans with versions properly. One option that I applied in other projects: we have CHANGELOG.md with changes in all versions and all new features from implemented plans go to Unreleased section in that file and human in arbitrary moment can command AI agent to release - then agent looks at all unreleased changes and derives bump version values and does release commit directly to main without branching."

## Problem

Every plan step lands on `main` as it is written (`.agents/rules/git.md`).
Nobody reviews a plan as a whole, and nothing on GitHub stops a red commit
from reaching `main`. CI's `oracle`, `wasm` and `parallel` jobs don't run
in the pre-commit hook, so today they find a break only after it has
landed. `ci.yml` already runs on `pull_request`, but nothing uses it.

Versioning works, but late. `/release` reads commit bodies since the last
tag and writes the notes then, into the reply and the GitHub Release. The
author of those notes is the agent at release time, not the one who knew
the plan. The notes survive only on GitHub. The crates published so far
(`v0.1.0` to `v0.2.0`) ship no changelog in the repository, and the plugin CAD has
to read GitHub Releases to learn what broke.

## Constraints it runs into

- `.agents/rules/git.md`: "Commit directly to `main`", "Never push unless
  asked", and branches only for experimental plans. This idea reverses the
  first and narrows the second.
- git.md §Tags, §The version: this stays unchanged. The workspace is
  released in lockstep, `main` carries `X.Y.Z-dev`, a release is two
  commits (`release X.Y.Z` and `open …-dev`), the tag goes on the first,
  and the human tags and approves the `crates-io` environment.
- `/release` step 3 and its Don't list say "no `CHANGELOG.md`: a
  hand-maintained fifth place is the one with no owner". This idea
  answers that argument; it doesn't ignore it.
- Commit per plan step with the `(plans/<slug> step N)` suffix: a squash
  merge would destroy it, and the version derivation depends on commit
  bodies that name public API changes.
- At most two active plans: two branches can be open at once, and both
  retire commits edit `docs/ROADMAP.md` and `AGENTS.md`.
- Backlog: "`cargo-semver-checks` in CI once the first non-placeholder
  version is published". That condition is now met, and this idea would
  absorb the line.

## Options

### A — Branch per plan, PR at retire; `/release` otherwise unchanged

Each plan runs on `plan/<slug>`. `/work` branches at step 1. The last
commit is `docs: retire plan <slug>`, followed by a push of the branch and
`gh pr create`, and the human merges. Merges are rebase-only so the step
commits and their bodies survive. A ruleset on `main` requires the CI
checks, including `oracle`, `wasm` and `parallel`, and forbids direct
pushes. `/release`'s two commits then go through a `release/vX.Y.Z` PR,
and the human tags the merged release commit. Tag after the merge, because
GitHub's rebase-merge rewrites commit SHAs. `/close-cycle`'s
`docs: close <cycle>` goes in the same PR. Cost: about 3 plan steps (ADR,
git.md, `/work` + `/retire-plan` + `/release` + `/close-cycle`), plus the
human's GitHub settings.

### B — A, plus per-plan changelog fragments and `CHANGELOG.md`

`/retire-plan` writes `changes/<plan-slug>.md` in `/release` step 3's
reader-facing style: what a consumer can now do, the refusals they will
hit, and a **Breaking** list with the one-line fix, plus the plan's semver
effect (patch or minor, pre-1.0). The author is the agent that just did
the work, and the notes get reviewed in the plan's PR. `/release` merges
the fragments into `CHANGELOG.md` and the GitHub Release, deletes them,
and derives the number as the largest declared effect. It cross-checks
that number against the commit-body lookup it runs today and against
`cargo-semver-checks`. If they disagree, it stops and asks.

This is still not a fifth place with no owner. Every fragment has an
owner, the retiring plan, and `/release` is the only thing that writes
`CHANGELOG.md`. The file ships inside each crate's tarball, so it is where
a crates.io reader looks. Cost: A + about 2 steps.

### C — A, plus fragments that go only to the GitHub Release

Same as B, but `/release` pastes the fragments into the GitHub Release and
deletes them, and no `CHANGELOG.md` exists. This keeps `/release`'s
current rule, and the notes are still written at plan time. The history
lives only on GitHub, not in the published crate.

### D — release-plz

It reads the conventional commits, opens a release PR with bumps, a
changelog and semver checks, and publishes. It would replace most of
`/release` and `release.yml`. Its changelog is one line per commit, which
is noisy with one commit per step and can't produce the notes `/release`
step 3 asks for. It also doesn't know about the `-dev` guard or the
environment reviewer.

### Do nothing

`main`'s greenness depends on local hooks, which skip three CI jobs, and
nobody reviews a plan as a whole.

## Recommendation

**B.** A alone fixes the branching problem but leaves the notes to be
written late. C is the cautious alternative if the no-changelog rule
should hold, and it costs the same. D fights the existing `-dev` and
reviewer design. The plugin CAD's idea with the same slug proposes A now and B at
its first release, so both repositories end up on one process. What would
change my mind: if the GitHub Releases already feed what the plugin CAD reads when
it bumps the pinned minor, C is enough.

## Decision for the human

1. Adopt branch per plan with a PR at retire, rebase-merge only, and a
   protected `main` requiring every CI job? *Yes.* It needs an ADR that
   supersedes git.md's "commit directly to `main`".
2. Should fragments be written at plan retirement? *Yes.*
3. Where do the fragments end up: `CHANGELOG.md` and the GitHub Release
   (B), or only the GitHub Release (C)? *B*. It needs the same ADR, since
   it overturns `/release`'s "no `CHANGELOG.md`".
4. Should the release land as a `release/*` PR, with the human tagging the
   merged release commit? *Yes.*
5. Should `cargo-semver-checks` join CI now, as a cross-check on the
   declared effect? *Yes*, which retires its backlog line.
6. May the agent push `plan/*` and `release/*` branches without asking
   each time? *Yes, since invoking the skill is the ask.* `main`, tags and
   the crates.io approval stay the human's.
