---
name: inspect
description: See a shape, an intersection curve or a failed operation without a GUI — dump it as text, render it to a PNG you can read, run the invariant checker on it, compare it against the Open CASCADE oracle, or stream it to Rerun for the human. Use when working on anything geometric and you need to check the result rather than reason about it; when a fixture or property test fails; when asked what a shape looks like; and to turn a failure into a fixture.
---

# Seeing geometry without a GUI

You can look at a shape. Do that instead of reasoning about coordinates from
source. Never ask the human to describe what a shape looks like — the human
has no better tool than you do until Arris sits behind a CAD application.

**Status:** the harness below is a cycle-1 deliverable (`SEED.md` §6,
milestone M0 in `docs/03-roadmap.md`). Until the `arris-debug` crate and
`tools/oracle/` exist, this file is the contract they are built to; the plan
that lands them updates the commands here in the same step.

## The four questions, and which tool answers each

| Question | Tool | Output |
|---|---|---|
| Is it valid? | `arris-check` — `Model::check(shape)` | Every violated invariant with the entity that violates it. Read this first; most "wrong picture" bugs are a checker line. |
| What is it, exactly? | text dump — `arris_debug::dump_text(&model, shape)` | Entities, ids, orientations, geometry parameters, tolerances, pcurves; deterministic, diffable, what a fixture stores. |
| What does it look like? | PNG — `arris_debug::render_png(&mesh, &polylines, View::Iso, highlight, "name")` over a `TriMesh` and `Polyline`s (M3 adds the body → mesh step) | Orthographic 800×600 render to `target/inspect/<name>.png`, one flat colour per face id, edges black with hidden parts hidden, dots at edge ends, `Some(Highlight::Face(id) / Edge(id) / Point(p))` in red. `View::{Iso, Top, Front, Right}`; `Iso` looks from (+1, −1, +1) so +x, −y and +z faces are visible. **Read the PNG with the Read tool** — it renders as an image. `arris_debug::render` returns the pixel buffer for a test that counts colours. |
| Is it right? | oracle — `tools/oracle/compare.py <fixture>` | Arris's volume, area, centroid, counts and point classifications against Open CASCADE's, with the fixture's tolerance. |

For the human: `arris_debug::rerun(&model, shape)` streams the same scene to
a Rerun viewer, with intersection curves and per-entity colours as separate
entity paths so they can be toggled. Use it when the human asks to see
something; do not use it to convince yourself — the PNG is what you can read.

## Reading a picture

A picture says *something* is off; a number says what. After a PNG shows a
missing face or a spike:

1. `dump_text` the shape and grep the entity the picture points at — the
   face's surface parameters, its wire's edges, each edge's tolerance.
2. Render again with that entity highlighted, and the input shapes beside
   it, to see whether the entity is wrong or its neighbour is.
3. Run the checker on the *inputs*. A boolean on an invalid input produces
   an invalid output with an unhelpful picture.

## From a failure to a fixture

Every geometry failure becomes a fixture (`.agents/rules/kernel.md`):

1. **Reproduce** in a test with the operands built from primitives or
   loaded from STEP.
2. **Shrink**: fewer faces, rounder numbers, an axis-aligned pose if the
   failure survives it. Stop when one more simplification makes it pass —
   that boundary is the bug's description.
3. **Save** under `tests/fixtures/<area>/<slug>/`: the operands (STEP or a
   build script), the expected values (`expected.json`, from the oracle),
   and the test with the *desired* assertion, `#[ignore = "<what fails>"]`.
4. **Commit** it as `test(<scope>): fixture <slug>` with the failure's
   shape in the body. The fix is a plan step or a backlog line; the
   fixture is committed either way, and un-ignored when it passes.

## Property-test failures

`proptest` prints the shrunk case. Turn it into a fixture the same way; the
seed in the failure message goes into the fixture's body so the original
case is reproducible even after the shrinker changes.

## Limits

- The PNG renderer is a software rasteriser of the *tessellation*. A shape
  the tessellator cannot mesh renders as its edges only — that is itself a
  finding, not a rendering bug.
- The oracle needs `tools/oracle/.venv`; `tools/oracle/README.md` says how
  it is created. A missing venv makes `compare.py` fail loudly, never skip.
