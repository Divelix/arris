# Backlog

One line per raw idea. Picking one up means `/idea` (needs thinking) or
`/plan` (obvious); the line is removed then — as is a line a roadmap cycle
has committed to, which now lives in `docs/03-roadmap.md` instead. Rejected
ideas keep one line below with the reason, so the same idea is not
re-brainstormed.

- `cargo-semver-checks` in CI once the first non-placeholder version is published
- Benchmarks (`divan` or `criterion`) for tessellation and the boolean corpus, so a robustness fix that costs 10× shows up
- `cargo-fuzz` targets for the STEP reader and the intersectors, seeded from the fixture corpus
- A `no_std`-friendly `arris-math`, if an embedded or wasm consumer ever wants it
- IGES read/write (SEED §6, later cycles)
- Publish the workspace crates to crates.io over the 0.0.1 `arris` reservation once cycle 1's vertical slice passes its corpus

## Rejected
