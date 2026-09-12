# Plan: revolve-touching-axis

- Started: 2026-09-13
- Milestone: C2, the application gate (docs/ROADMAP.md §C2, the line on
  the revolve profile touching its axis)
- Idea (verbatim from the human): "/plan revolve-touching-axis"

## Goal

A revolve whose profile touches its axis — along a segment, or at a
vertex — builds a solid instead of refusing. The consumer's probe, a
rectangle `[0, 1] × [−1, 1]` with one side on the axis turned `2π`, is a
solid cylinder of volume `2π` that passes every corpus stage against Open
CASCADE, as does its partial turn, whose two flat ends share the edge on
the axis. A segment on the axis sweeps nothing; a vertex on the axis sweeps
no rise, and a cone's apex or a sphere's pole there is a degenerate edge,
the first an operation makes. In a full turn a notch that reaches the axis
closes into a void of the same lump (ADR-0006), and a vertex on the axis
with no segment along it — a pinch — is a typed refusal.
`Reason::ProfileTouchesAxis` is gone.

## Non-goals

- A profile crossing its axis: a permanent `Reason::ProfileCrossesAxis`.
- A horn or spindle torus — an arc whose circle reaches the axis off its
  centre — stays `Reason::SpindleTorus` (backlog).
- Cone and sphere faces as corpus fixtures: the checker's S5 and B1 arms
  and `classify_point` against them are C2's own line. Here they are
  closed-form tests and the oracle's reading of a scratch STEP, as M5's
  frustum, barrel and ring were.
- Booleans with cone or sphere operands; sweep along a path; fillet.

## Design deltas

- **Measured against Open CASCADE before planning** (scratch recipes
  through `expected.py`, OCCT 8.0.1): the probe's full turn is
  `V/E/F/L/S = 2/3/3/3/1` — no vertex at either disc's centre, one seam —
  and its quarter turn `6/9/5/5/1`, the two axis vertices and the one edge
  on the axis shared by the flat ends. A 2×3 rectangle on the axis with a
  1×1 notch cut in from the axis is `4/6/6/6/2`, volume `11π`, one solid:
  the notch is a void. A cone with its apex on the axis and a pinch vertex
  both stop the oracle with an odd Euler characteristic, because Open
  CASCADE counts the degenerate edge at the apex (below).
- **The Euler line and the counts leave degenerate edges out** (~~⚠ OPEN
  1~~, decided), on both sides: `Report::euler`, `dump_text`'s line, the corpus runner's
  `edges` and `tools/oracle/oracle/measure.py`'s `edges`, which skips an
  edge `BRep_Tool::Degenerated` names. A degenerate edge is a singular
  point of a surface, not a boundary between faces — S2 already exempts it
  — and counting it is what makes a sphere derive genus 1 and a cone an
  odd characteristic. No committed fixture has a degenerate edge, so no
  `expected.json` or `dump.txt` moves; `sample::sphere`'s printed line
  becomes `2/1/1/1/1 g0 = 0` (the seam alone). Retires the backlog line on the Euler line
  counting degenerate edges.
- `arris-ops` revolve (`docs/ARCHITECTURE.md` §Operations): a vertex
  within `default_tolerance` of the axis is *on* it, and a line segment
  with both ends on it lies along it. `orient` refuses only a profile
  crossing the axis. A segment along the axis sweeps no face; in a partial
  turn it is one edge, the flat ends' shared edge, `Generated` from
  `StartEdge`, with no `EndEdge`; in a full turn it is nothing. A vertex on
  the axis is one vertex (no `EndVertex`) and sweeps no rise: a plane or a
  cylinder face passes through it; a cone's apex or a sphere's pole puts a
  degenerate edge in that face's loop, its pcurve the line at the
  singular `v` over the rise's range, `Generated` from `Rise { loop_index,
  vertex }` — one per face that closes there. A full-turn vertex on the
  axis that no face keeps (between a segment along the axis and a plane
  disc) is not built, as Open CASCADE builds none.
- In a full turn, a loop's shells are its *chains* — maximal runs of
  segments off the axis, each closing into its own shell across the axis
  — and a hole is a chain of its own. A simple profile on one side of the
  axis has exactly one chain whose ends span all the others along the
  axis. That chain is the lump's outer shell; every other chain is a void
  directly inside it, stored in loop order. A partial turn's flat ends
  join everything into one shell, as today.
- A full-turn vertex on the axis with no segment along it is two chains,
  or one chain and itself, touching at a point: `OpError::Degenerate` with
  `Reason::NonManifold` (~~⚠ OPEN 2~~, decided), no entities, as a sweep
  names none. A
  partial turn of the same profile is manifold (the flat ends close the
  vertex's link) and builds.
- `Reason::ProfileTouchesAxis` removed (**public enum change**, step 4,
  when its last case goes); `Reason::NonManifold`'s doc widens to sweeps.
- `docs/DATA-MODEL.md` §Provenance: the parts a touching profile does not
  make (no `Side` for a segment along the axis, no `Rise` or `EndVertex`
  for a vertex on it except the degenerate edges above); every void of a
  full turn is `SweepPart::Cavity { loop_index, segment }`, `segment` the
  lowest index the consumer wrote among its chain's segments, so two
  notches of one loop are two names and a hole's cavity is `segment: 0`
  (~~⚠ OPEN 3~~, decided; **public enum change**, step 4).
- `arris-debug` prop: `prop::profile::rectilinear` and `general` put the
  profile on the axis a share of the time (a bar's inner side along it; a
  polygon side along it, a sphere arc ending at a pole), so the Pappus
  properties reach every case above.

## Steps

Complexity grades the human uses to pick the agent for a step: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a geometric or numeric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness or
bound has to be established here.

- [x] Step 1 **[1]** — degenerate edges out of the Euler line and the
  counts, both sides. Tests: `sample::sphere` and
  a hand-built cone with an apex edge print a closing line at genus 0;
  the oracle's self-test builds Open CASCADE's own cone and sphere
  (`BRepPrimAPI`) and derives genus 0 with an even characteristic, and
  every committed `expected.json` reproduces unchanged.
- [ ] Step 2 **[2]** — a segment along the axis and a vertex on it, for
  faces on planes and cylinders; cones and spheres at the axis still
  refused as `ProfileTouchesAxis`. Fixture `sweep/revolve-onto-axis`: the
  consumer's rectangle, `default` a full turn (volume `2π`, `2/3/3/3/1`),
  variants `quarter` (`π/2`, `6/9/5/5/1`) and `three-quarter` (a reflex
  edge on the axis), each passing every corpus stage. Revolve tests: the
  provenance parts a touching profile does and does not make.
- [ ] Step 3 **[2]** — a cone's apex and a sphere's pole on the axis:
  degenerate edges in revolve, through the checker at `Full` (E6, L2 across
  the degenerate edge, V3), `measure`, `tessellate` (closed, triangles
  collapsing at the apex dropped) and STEP (the coedge left out, Open
  CASCADE's reader rebuilding it). Closed-form tests with the oracle's
  reading of a scratch STEP (`oracle::scratch_fixture`): a cone (a right
  triangle with a leg on the axis, full turn and quarter), a ball (a half
  disc on the axis, `4π/3`) and a partial turn of a pinch; the unchecked
  rows are the quadric pairs' and nothing else.
- [ ] Step 4 **[2]** — full-turn chains: a notch reaching the axis closes
  into a void, a pinch is `Reason::NonManifold` (Open CASCADE's
  `BRepCheck_Analyzer` of its own revolve of the pinch recorded in the
  commit body, as ADR-0006's vertex case was), `Cavity { loop_index,
  segment }`, and `Reason::ProfileTouchesAxis` removed. Fixture
  `sweep/revolve-notch-to-axis` (the 2×3 rectangle with the 1×1 notch:
  `default` `11π`, 2 shells and 1 solid; `quarter` one shell), every
  corpus stage. Revolve tests: a pinch full turn refused and its partial
  turn built; two notches on one loop are two voids with two names.
- [ ] Step 5 **[2]** — the property tests touch the axis: `rectilinear`
  and `general` draw touching profiles, and the revolve Pappus shards hold
  volume, area, the checker and the provenance accounting over them at
  the configured case count, full and partial turns alike.

## Acceptance

`cargo nextest run --workspace` with the corpus: `sweep/revolve-onto-axis`
(every variant) and `sweep/revolve-notch-to-axis` passing every stage —
checker `Full` with nothing unchecked, counts and genus, Open CASCADE's
volume, area, centroid and probes through STEP, mesh closed, mass
properties and inertia, classification, provenance accounting, dump; step
3's cone, ball and pinch tests matching their closed forms and the
oracle's reading; the revolve property shards green; the oracle's
self-test reproducing every committed `expected.json`; no
`ProfileTouchesAxis` left in the workspace.

## Docs to update on completion

- `docs/ARCHITECTURE.md` §Operations (the revolve paragraph: the profile on
  the axis, chains into shells, the pinch refusal; "the apex and its
  degenerate edges being cycle 2's" goes), §Errors (`ProfileTouchesAxis`
  out, `NonManifold` for sweeps).
- `docs/DATA-MODEL.md` §Topology (a degenerate edge is made by a revolve at
  an apex or a pole), §Euler–Poincaré (degenerate edges not counted),
  §Provenance (the parts a touching profile makes, the cavity's name).
- `tests/fixtures/README.md` §Conventions the numbers assume, and
  `tools/oracle/README.md` — `edges` counts no degenerate edge.
- `docs/BACKLOG.md` — the Euler-line-counts-degenerate-edges line removed.
- `docs/ROADMAP.md` §C2 — the axis line's status.
- `AGENTS.md` current state.

## Open questions

All three decided at planning; the human delegated them. No ADR: each
follows a rule already written down (S2's exemption, ADR-0006's
refusal, the consumer's segment indices).

- ~~`⚠ OPEN 1:`~~ degenerate edges in the counts — left out of `edges` and
  the Euler line on both sides, or kept with a separate `degenerate_edges`
  count every `expected.json` then carries, or genus left out for such a
  shape. **Decided: left out.** No schema change and no committed number
  moves, and it is S2's reading of a degenerate edge as a point of the
  surface; the other two either rewrite every oracle file or give up the
  genus check exactly where the new geometry is.
- ~~`⚠ OPEN 2:`~~ a full-turn pinch — `Reason::NonManifold` or a
  sweep-specific reason. **Decided: `NonManifold`.** It is shells (or one
  shell) touching at a vertex, the case ADR-0006 already refuses, and a
  consumer handles one reason for one fault. Step 4 still records what
  `BRepCheck_Analyzer` says of Open CASCADE's revolve of it; a verdict of
  valid would not make the vertex manifold, as ADR-0006 found for the
  compound.
- ~~`⚠ OPEN 3:`~~ the name of a void closed from a notch — `Cavity {
  loop_index, segment }` or a new `SweepPart` variant beside `Cavity {
  loop_index }`. **Decided: `Cavity { loop_index, segment }`**, `segment`
  the lowest consumer index among the chain's segments: the same under
  the loop's orientation, distinct between chains since chains share no
  segment, and one variant for every void a revolve makes.
