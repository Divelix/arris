# Idea: c3-torus-pairs

- Status: Open
- Raised: 2026-09-19
- Prompt (verbatim from the human): "c3-torus-pairs"

## Problem

A torus face meets nothing it does not share an axis with. `intersect_surfaces`
sends every torus pair to the meridian arm (ADR-0008), and `off_axis` answers
each one it cannot decide there `Unsupported` — plane, cylinder, elliptic
cylinder, cone, sphere and torus alike (`crates/arris-geom/src/intersect.rs`,
`off_axis`). Tori are not rare in this kernel: every hole-rim fillet
(`blend/hole-rim-fillet`, ADR-0007) and every revolve of an arc
(`sweep/revolve-ring`) makes one. So once C3 lifts the quadric guard, the first
boolean after a fillet that is not coaxial with it — a cross drill through a
rounded rim, an oblique cut through a revolved ring — is refused. C3's goal
line names the torus, and its accept line asks for property tests of every pair
at random poses. S5 and B1 also list a posed blend's rim torus as unchecked
against anything off its axis.

What makes this hard is what ADR-0018 relied on: its tracer walks the
*rulings* of a cylinder, elliptic cylinder or cone, and a ruling meets a
quadric in a **quadratic**. That makes the discriminant a degree-two
trigonometric polynomial and puts the whole topology in the roots of one
quartic. A torus has no rulings, and a line meets it in a **quartic**. So the
torus pairs cannot be one more `match` arm on the tracer. Their sections have
degree 8 against a quadric and 16 against another torus (the spiric sections
among them), and they are always bounded, because the torus is.

## Constraints it runs into

- **ADR-0018**: "the tracer is the family method; the marcher stays C4's", and
  "the branch topology is algebraic, not sampled". Any new tracer must keep
  that guarantee: no loop can fall between samples. A second tracing method
  bends the first clause, so it needs an ADR of its own (ADR-0018 is
  append-only).
- **ADR-0018's storage decision holds unchanged**: a fitted `Curve::Nurbs`
  within `SECTION_FIT_FRACTION` of `tol.linear`, a closed branch periodic,
  singular points returned as `MeetPoint`s in one `Meets`. Option A of that ADR
  was rejected partly *because* the torus would need B anyway. This idea
  inherits that.
- **Kernel rules** (`.agents/rules/kernel.md`): exhaustive dispatch with no
  wildcard; tolerances taken from the model (the singular-point bound is in
  length, as the tracer's is); deterministic and symmetric under swapping the
  operands, bit for bit.
- **Layering**: all of this lives in `arris-geom`, with its Bernstein
  arithmetic (`crates/arris-geom/src/bernstein.rs`, univariate today). Nothing
  crosses a crate line.
- **Scope**: `SEED.md` §4 rules out nothing here. A spindle torus stays out
  (C3 "Out", `R > r` in `docs/DATA-MODEL.md` §Surfaces). The NURBS marcher is
  C4's.

## Options

### A — Tube circles as the family, for plane and sphere

Walk the torus's own tube circles (fixed `u`) instead of rulings. In the plane
of each tube circle, a plane leaves a line and a sphere leaves a circle; the
sphere's circle meets the tube circle along their radical line. Either way the
tube circle meets the other surface in the roots of a **quadratic**, and the
discriminant is again a degree-two trigonometric polynomial in `u` (checked for
both). That makes it ADR-0018's algebra with a circle family: the turning
points are one quartic, the singular-point bump and the branch joining are
reused, and no region clip is needed. This covers the spiric sections, an
oblique plane on a torus, and a sphere off a torus's axis. Cost: **3–4
steps**. What it leaves out: cylinder, elliptic cylinder and cone against a
torus, and torus against torus. For every member of those families the
per-member equation is quartic. The pair it misses (a drill through a filleted
rim) is the likeliest torus boolean in practice.

### B — Implicit–parametric tracing in the torus's (u, v)

Put the torus's parametrisation into the other surface's implicit polynomial.
The result is `f(u, v) = 0`, a trigonometric polynomial of bidegree (2, 2)
against a plane or quadric, or (4, 4) against a torus. Per quarter-turn patch
with half-angles it becomes a bivariate Bernstein polynomial with the
denominators cleared, and positive. That is the same substitution
`intersect_spline.rs` already does for a curve, one dimension up. The topology
comes from the solutions of `f = f_v = 0` (turning points in `u`) and
`f = f_u = f_v = 0` (singular points), isolated by bivariate Bernstein
subdivision, which excludes a box by its coefficients' signs and certifies it
by Newton.

- **Completeness**: every component either has a turning point or winds
  monotonically round `u` and so crosses `u = 0`. The roots of `f(0, v)` seed
  the second kind. So the guarantee "nothing falls between samples" still
  holds, proven by subdivision rather than a closed-form quartic.
- **Tracing**: between turning points, each `u` solves `f(u, ·) = 0` with the
  univariate code that already exists. Every sample lies on the torus exactly
  and on the other surface to rounding.
- **Pcurve for free**: the torus's pcurve *is* the traced (u, v) curve, so it
  needs no second fit on the torus side. That also covers the torus part of
  C3's "fitted pcurve fallback" line.

It covers all six torus pairs with one method. `within` is ignored, because
the domain is compact. The bivariate isolation it adds is also the core of C4:
a NURBS patch put into an analytic implicit is the same bivariate Bernstein
polynomial. Cost: **7–9 steps** (the bivariate arithmetic and isolation; the
tracer and its singular points; the fit and the `Meets`; the pairs in
dispatch, S5 and B1; property tests per pair; oracle fixtures). Risks:

- near-singular poses are worse conditioned in 2D than on ADR-0018's quartic,
  and the tangency bump does not carry over as is;
- subdivision depth at the default tolerance on metre-scale tori is not yet
  measured;
- a new ADR is needed.

### C — A + B, each where it is simplest

A for plane and sphere, B for the rest. This gives exact algebra where it is
cheap, but it means two new methods for two pairs that B already decides. The
extra algebra also buys no accuracy, because both fit to the same fraction.
Cost: A plus B, about **10–12 steps**.

### D — The ruled tracer grown to quartic members

Walk the cylinder's, elliptic cylinder's or cone's rulings against the torus,
four roots per ruling. The turning points are then the discriminant of a
quartic whose coefficients are trigonometric in `s`: degree about 16 in
`tan(s/2)`, ill-conditioned in the power basis, and in practice solved as the
2D system `f = f_w = 0` anyway, which is B's machinery without B's
uniformity. Torus against torus is still missing. Loses to B.

### Do nothing (defer tori to C4)

Tori stay `Unsupported` off their axis, and C4's marcher decides them along
with NURBS. It costs nothing now. But C3's goal and accept lines change,
booleans after a fillet or an arc revolve stay refused until C4, and S5 keeps
listing posed rim tori as unchecked. A general marcher is also a weaker answer
for a torus than exact samples.

## Recommendation

**B.** It is the only option that reaches every torus pair with one method,
including the cylinder–torus pair a real part hits first. It keeps ADR-0018's
completeness guarantee (proved by subdivision, not sampled), it gives exact
samples and the torus pcurve directly, and the bivariate isolation it builds is
the thing C4 needs first, so it is not throwaway. A covers too little on its
own. C is more code for no accuracy gain. D is B's machinery without B's reach.

**What would change my mind:** if the first step shows the bivariate isolation
cannot certify turning and singular points at the default tolerance on
metre-scale tori, at a reasonable depth. In that case, fall back to A for plane
and sphere, and move the other four pairs to C4 with the roadmap amended.

**Adjacent, not in scope:** a conic against a cone, sphere or torus in
`intersect_curve_surface`, which the lifted guard also needs. It can probably
go through the existing rational-span Bernstein arm (a circle as rational
quadratic spans), which already takes the torus's degree-4 implicit. That
belongs to the same plan's guard lifting, not to this idea.

## Decision for the human

1. **Trace torus pairs by implicit–parametric substitution in the torus's
   (u, v), with bivariate Bernstein isolation (B)?** Preferred: yes. The
   alternatives: A only, with the other four pairs deferred to C4; or do
   nothing and move tori to C4.
2. **Does B also serve a torus against an elliptic cylinder and against
   another torus in this cycle?** Preferred: yes, all six pairs, since the
   method costs the same for each. The alternative is torus–torus to C4.
3. **An ADR (0019), "torus sections are traced in the torus's parameter
   plane"?** It would record the second tracer beside ADR-0018's family
   method, the completeness argument, the singular-point bound in 2D, and the
   pcurve taken from the trace. Preferred: yes, written with the plan's first
   step. It amends ADR-0018's "the tracer is the family method" without
   reopening its storage decision.
4. **One plan with the conic hits and the lifted quadric guard (C3's second
   plan as `AGENTS.md` names it), or torus pairs as a plan of their own
   first?** Preferred: a plan of its own. The guard lift needs its own corpus
   of quadric-operand booleans, and B's first-step risk should be known before
   that corpus is written against it.
