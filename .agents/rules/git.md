# Git: trunk-based, always green

- `main` is the trunk and is always green: `cargo fmt --check`, `cargo clippy
  --workspace --all-targets -- -D warnings`, `cargo test --workspace` and
  `cargo doc --workspace --no-deps` with `-D warnings` pass on every commit.
  The `.githooks/pre-commit` hook enforces it; never bypass it with
  `--no-verify`.
- Commit **directly to `main`**. A plan step is the unit of work and the unit
  of commit: finish the step, run the checks, tick the box, commit.
- Branch only when a plan is experimental enough that throwing it away is a
  real outcome, or the human asked to review before it lands. Then:
  `plan/<slug>`, rebased onto `main`, fast-forwarded in (`git merge --ff-only`),
  deleted after. No merge commits, no long-lived branches.
- Never rewrite `main`: no `--amend` of a pushed commit, no force-push, no
  rebase of anything already on `main`.
- Never push unless asked. Commits are the agent's; pushes are the human's.
- Stage deliberately: read `git status` and `git diff`; no blind `git add -A`.
  A changed fixture expectation (`tests/fixtures/**/*.json`) is only staged
  with a matching intentional geometry change, and the commit message says
  `fixtures:` and why. Oracle values do not drift; if they changed, the
  fixture or the oracle script changed, and the body says which.

## Message format

```
type(scope): imperative summary, lower case, no period  (plans/<slug> step N)

Why this change, not what — the diff already says what. Reference ADRs
(ADR-0004) and docs (docs/DATA-MODEL.md §Tolerances) that justify or were
updated by it. A change to a public type or signature names it here.
```

- `type`: `feat`, `fix`, `refactor`, `perf`, `test`, `docs`, `chore`, `build`,
  `ci`, `fixtures`.
- `scope`: crate short name(s) — `math`, `geom`, `topo`, `check`, `ops`,
  `mesh`, `io`, `debug`, `arris` — or `docs`, `plan`, `adr`, `tools`.
  Several: `feat(geom,ops): …`.
- The `(plans/<slug> step N)` suffix is present on every commit that executes
  a plan step. Plan retirement commits are `docs: retire plan <slug>`.
- Docs change in the **same commit** as the code that changes behaviour.
  A commit that only updates docs to match existing code is `docs(sync): …`.
## Tags

- Milestones: `m0`, `m1`, … on the commit that retires the milestone's last
  plan and passes its acceptance corpus.
- Releases: `v0.1.0` SemVer, on `main`, created by the human. Pre-1.0 a
  minor bump may break the public API; the tag's commit body lists what.

## What the agent does without asking

While executing a plan: run checks, commit each step, tick boxes, update the
plan file. Anything else that touches history — branching, tagging, pushing,
resetting, rewriting, publishing to crates.io — is asked first.
