# Idea: c3-tolerance-apart

- Status: Open
- Raised: 2026-09-22
- Prompt (verbatim from the human): "c3-tolerance-apart"

## Problem

C3's last roadmap line: *features a tolerance apart — section vertices
clustered by closure rather than first-come merging, and a corpus of faces
touching and coincident within a tolerance*. C3 cannot close without it:
its accept line names `regression/seam-a-tolerance-from-crossing-fuse`,
and the first-party binding and the reader cycle both wait on the close
(ADR-0020).

Six failures were parked on this plan. Each is a body the kernel should
build and does not, met at random poses, not designed ones:

| Fixture / line | What is a tolerance apart | Today | Rate |
|---|---|---|---|
| `seam-a-tolerance-from-crossing-fuse` | three points 1.7e-7 and 2.4e-7 apart at 1e-7 | `Fault::Split` | a 5e-6° band |
| conic hit at `t = 0` (c3-conic-hits step 3) | a pave one rounding under `2π` and the edge's start | a hair-thin block | tests work round it |
| `pole-slice-beside-seam-cut` | a section and the seam, one curve over 1.8e-4 | `Fault::Seam`; L2 if forced | 1 in 8000 |
| `frustum-stub-cut-then-fuse` | two fits of one section, other knots | `Unsupported` (NURBS–NURBS) | 1 in 256, property spares the pair |
| `grazing-ball-bar-cut` | two fits of one section 1.06e-7 apart at 5° | S5 refuses a right body | 3 in 1400 |
| `singular-bore-cut` | nothing — an exact touch leaves a pinched vertex | `BuildError::NotClosed` | designed pose |

They share one root. ADR-0004 promised that "nothing downstream ever
compares two independently computed points", and that coincident and
tangent cases are named, "never a tolerance accident". The pave model
keeps the first promise *per interference* and then merges by first come
(`PaveBuild::vertex_near`, `merge_point` in `boolean/pave.rs`): a point
joins the first vertex whose ball reaches it. "Within a tolerance" is not
transitive, so between one and two tolerances the answer depends on
creation order — the very objection ADR-0004 raised against matching
pieces. The same non-equivalence shows one level up, between curves
(section against seam, fit against fit), and the second half of the
roadmap line — faces a fraction of a tolerance off flush or off tangent —
has no fixture at all, so how it fails is unknown.

## Constraints it runs into

- **ADR-0004**: shared paves; no fuzzy value (its rejected alternative);
  named coincident and tangent cases. **`SEED.md` §9**: tolerances are
  per entity and grow by a stated rule.
- **`docs/DATA-MODEL.md` §Tolerances**: growth needs a nameable reason —
  "a vertex merged from two points `t` apart" is one; `max_tolerance`
  turns runaway growth into `OpError::Tolerance`; "the true geometry lies
  within `t` of the stored" is what an edge's tolerance *says*.
- **ADR-0016**: the touch stays a verdict on depth, resolved through the
  section curves; it calls the seam fixture "about merging, not touches".
- **ADR-0018 / ADR-0019**: a fit is held to `SECTION_FIT_FRACTION` of the
  tolerance of its two *surfaces*, one region per boolean, and a fitted
  edge's tolerance does not grow. At a meeting angle θ that lets the
  curve lie up to `tol / 4 sin θ` from the true section along the
  surfaces — past the edge's tube from about 14° down, so the tolerance
  statement above is false for a grazing fitted edge today. ADR-0018's
  tripwires (growth through chained booleans) are not what fired.
- **ADR-0021**: beside a singular point is refused by name and its fixture
  kept in `regression/` with the desired body — the precedent for a named
  refusal that is not accepted behaviour.
- **ADR-0015**: Open CASCADE is wrong in parts of these bands (its 9/15
  counts beside the crossing); fixtures there are held to closed forms.
- **`.agents/rules/kernel.md`**: deterministic ids — closure is
  order-independent where first come is not, but blessed dumps must not
  renumber at generic poses; no literal tolerances; a new `Reason` or
  `VertexSource` variant is a design delta.
- **Data model, Solid**: Euler–Poincaré's parity and L5 both refuse a
  surface pinched at a vertex; `TangentContact` already refuses the slit,
  the curve analogue, "because the manifold `Solid` has no way to say it".

## Options

### A — the accept line and nothing else
Cluster section vertices by closure, snap the `t = 0` pave, move the one
named fixture, close C3. The other four fixtures stay ignored and their
lines return to the backlog under the measuring harness or a named cycle;
the C3 goal sentence about faces within a tolerance is struck, honestly.
About 3 steps. It forecloses nothing, and it ships a kernel whose
cut-then-fuse over a cone fails once in 256 poses and whose checker
refuses right bodies — into the binding and the consumer's side-by-side
run, where each is found again as a regression and ranked first
(ADR-0020).

### B — one relation, decided once per level
"The same within tolerance" made an equivalence at each level where the
pave model asks it, and the corpus that proves it:

1. **Points.** Section vertices are the connected components of "within
   tolerance" over every candidate point at once — hits, crossings,
   section crossings, singular points, resolved touches, operand vertices
   as anchors — with the existing `base + spread` tolerance. A component
   is numbered by its first member in today's creation order, so a
   generic pose keeps its dump. Paves in one component are one pave, in
   the turn metric on a closed edge, which is the `t = 0` line and the
   dropped 1.7e-7 blocks both. A chain is bounded by `max_tolerance`; a
   component holding two vertices of one operand would collapse that
   operand's edge and is a named refusal, not a merge.
2. **Curves.** A stretch of a section within tolerance of an operand edge
   between two paves is that edge's block (the seam beside the pole). An
   operand edge whose two faces lie on the pair's two surfaces *is* that
   pair's section, decided by surface identity — well-conditioned, exact,
   and no NURBS-against-NURBS comparison (the restoring fuse).
3. **Fits.** A fitted section is held to a fraction of the tolerance of
   the *exact branch* the tracer already samples, not only of the two
   surfaces. The edge's tube then contains the true section at every
   angle, S5's re-trace agrees with the edge without an excuse, and 2's
   identity argument is sound at a grazing pair. Cost: more spans where
   sections graze. Amends the fit target of ADR-0018/0019.
4. **Faces.** The missing corpus: each designed flush, coaxial and tangent
   fixture (`flush-union`, `boss-flush`, `pin-in-bore-*`, `coaxial-*`,
   `tangent-cylinders-*`, `pipe-elbow-fuse`, `edge-touching-fuse`) swept
   by an offset, a tilt and a radius change from a quarter of a tolerance
   to several, as a property: the result is the flush body, the generic
   body, or a named refusal — never `Internal`, never checker-red — and
   volume is continuous across the band to `area × tol`, an identity that
   needs no oracle where ADR-0015 says the oracle is wrong. The band's
   ends become fixtures with oracle values.
5. **The pinch.** Decided, not left as `NotClosed`: see the decision below.

About 10–12 steps, one ADR. What 4 finds is unknown; the plan bounds it —
a failure that is none of 1–3 becomes a regression fixture and a backlog
line, not a new step. Forecloses nothing; ADR-0018's option C stays the
upgrade path.

### C — a fuzzy value or a snap pre-pass
Widen every comparison, or snap operands onto each other before the pave
model runs. ADR-0004 rejected the first by name and `SEED.md` §9 the
idea behind both: the widening is recorded in no entity. Listed so it is
not re-brainstormed.

### D — refuse every such pose by name
Turn `Fault::Split`, `Fault::Seam` and `NotClosed` in these bands into
typed `Reason`s. 2–3 steps, and correct as far as it goes — an `Internal`
fault on valid input is the worse bug — but four of the six are bodies
with nothing degenerate about them, and kernel.md does not let a failure
become accepted behaviour. It is the right *fallback inside B* for what
step 4 turns up, not a plan.

### Do nothing
C3 stays open; the binding and the reader do not start.

## Recommendation

**B, as one plan, ordered so the accept-line fixture lands in its first
steps and the corpus sweep comes last.** A closes the cycle on paper and
moves the same work into the consumer's regression list, where it costs a
round trip each. B's three mechanisms are small because each reuses what
exists — the merge loop, common blocks, the tracer's exact samples — and
the charter's measure is a kernel a consumer does not have to work
around. For 3 the alternative, S5 reading a lateral slack of `tol / sin θ`,
is rejected: it softens the one component that must not soften, without
bound as θ → 0, and leaves the edge's tolerance statement false.

What would change my mind: components chaining past a few tolerances at
*generic* poses of the existing corpus (visible as grown tolerances in
blessed dumps) — then closure needs a diameter bound and its own idea; or
the branch-held fit multiplying span counts severalfold on the metre
probes — then 3 moves to the harness, with `grazing-ball-bar-cut` parked.

## Decision for the human

1. **Scope: B in one plan (preferred), or A now and the rest after the
   reader?** B delays the C3 close by one plan of c3-conic-hits' size.
2. **A pinched vertex in a `Solid`: a named refusal, `TangentContact`'s
   point analogue, with `singular-bore-cut` kept in `regression/` holding
   the desired body (preferred) — or admitted, amending Euler's parity
   and L5?** Open CASCADE builds it; admitting it is a data-model change
   every vertex-fan walk downstream inherits, and belongs with the
   `General` body if a consumer's regression asks.
3. **An ADR is needed** — "the same within tolerance is an equivalence,
   decided once per level", carrying the fit target's amendment and
   decision 2. Agreed?

This idea absorbed five backlog lines. If the answer to 1 is A, four of
them (grazing fits, one section in two booleans, the pole beside the
seam, the singular bore) go back to `docs/BACKLOG.md` re-pointed; the
fifth, the conic hit at `t = 0`, is done either way.
