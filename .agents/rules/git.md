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
- Releases: `vX.Y.Z` SemVer, on `main`, created by the human. Pushing the
  tag is what publishes: `.github/workflows/release.yml` runs `cargo
  publish --workspace` and opens a GitHub Release. CI runs on the tag as
  well, and the publish waits for the `crates-io` environment's reviewer,
  so the human approves it with that run's result in front of them. Every
  crate but `arris-debug` goes up; that one is `publish = false`.

### The version

- **`main` between releases carries the next version with `-dev`**:
  `[workspace.package].version = "0.2.0-dev"`, and each internal crate in
  `[workspace.dependencies]` pinned to exactly it, `version =
  "=0.2.0-dev"`. The eight crates are published in lockstep and only ever
  make sense as a set, so the pin is exact — and it is also the guard: a
  `[workspace.package]` bump that misses the requirements fails the next
  `cargo check`, and `release.yml` refuses to publish a pre-release
  version at all. A release is therefore impossible without the deliberate
  commit that drops `-dev` from all eight places at once — the workspace
  version and the seven requirements. `arris-debug` carries no version
  because it is never published.
- **Pre-1.0, Cargo reads `0.y.z` as `y` breaking, `z` compatible**, so:
  - **closing a roadmap cycle bumps the minor.** A cycle here always
    breaks the API — a new surface or curve kind makes every exhaustive
    `match` fail to compile, which is the point (`.agents/rules/kernel.md`
    §API). Cycle Cn releases `0.n.0` as long as that lines up; it is a
    convention, not a law, and the cycle's status line names the tag it
    actually got.
  - **a release between cycles bumps the patch** — the case that exists
    because a consumer is waiting on a fix. Unless a commit body since the
    last tag names a changed public type or signature: then it is a minor.
    That is a lookup, not a judgement, because every such change is named
    in its commit body by the rule above.
- The version lives in `Cargo.toml` and nowhere else. No doc, no README and
  no rustdoc line states it, so nothing can go stale.
- `/release` cuts one: it picks the number from the log, writes the release
  notes, bumps, proves the workspace still packages, and hands the human
  the exact tag command. `/close-cycle` ends by calling it.

## What the agent does without asking

While executing a plan: run checks, commit each step, tick boxes, update the
plan file. Anything else that touches history — branching, tagging, pushing,
resetting, rewriting, publishing to crates.io — is asked first.
