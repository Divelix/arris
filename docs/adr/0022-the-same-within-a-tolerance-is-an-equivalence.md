# ADR-0022 — The same within a tolerance is an equivalence, decided once per level

- Status: accepted (2026-09-23)
- Plan: `c3-tolerance-apart` (steps 2 to 7; this record is step 8)
- Closes: that plan's open questions 1 (the pinch), 2 (the closure's
  relation), 3 (what a fit is held to), 4 (a touch between two curves of
  one surface) and 6 (where a merged vertex's pcurves meet)
- Follows: ADR-0004 (the pave model), ADR-0006 (a `Solid` is manifold at
  every level), ADR-0016 (a touch off every vertex is resolved through the
  section curves), ADR-0021 (a pcurve never runs through a singular point)
- Amends: **the fit target of ADR-0018 and ADR-0019.** A fitted section is
  held to a fraction of the tolerance of its two surfaces *and of the
  exact branch its tracer samples*, at the branch's own parameter.
  ADR-0018's storage, its tripwires and its option C stand; ADR-0019's
  tracer stands, its branch now continuous through a turning point.

## Context

C3 made every pair of analytic surfaces a boolean operand, and with it
every way two features can be a tolerance apart: flush faces a quarter
tolerance off, a seam a tolerance from a crossing, a section grazing a
wall at a degree, one section fitted twice over two regions. The pave
model answered "is this the same?" at four levels — points, fits, curves,
faces — each by whichever comparison happened to be asked first, so the
answer depended on creation order, on which region a boolean traced in,
or on a closed form for two splines that does not exist.

Step 1 of the plan measured the band: every designed contact of the
corpus moved off by ¼ to 16 tolerances, 10 752 booleans at 256 poses.
4248 failed. By the mechanism each failure names, 157 were two points a
tolerance apart, 459 a curve crossing another with no vertex there, 56 a
fit off its branch, 1356 a section edge ending where nothing else does,
and 2284 — the larger part — none of these: no verdict for two surfaces
a hair off parallel, slivers the polygons do not resolve, the builder
refusing what near-tangent contacts leave it. The first four are this
record's; the rest are fixtures and backlog lines, the next plan's
subject.

A fuzzy value, a snap pre-pass moving the operands onto each other, and
S5 reading a lateral slack at a grazing angle were rejected before the
plan began: each widens a tolerance to make a case pass, which ADR-0004
and `SEED.md` §9 forbid.

## Decision

**1. Points: the section vertices are the connected components of the
candidate points.** Every hit, crossing, section crossing, singular point
and resolved touch is a candidate with a ball — the tolerance of the
entities that made it, never a grown one — and two candidates are one
point when their balls meet, `|p − q| ≤ tp + tq`, or they name one
operand vertex. The relation is fixed before any merge, so its components
are the closure and do not depend on the order the points were found in;
a component is numbered by its first member in creation order and carries
that member's source, so no generic pose's dump changed. Its tolerance is
`base + spread` as before. Two vertices of one operand in one component
would collapse that operand's edge and are `OpError::Tolerance`. It is
also the vertex–vertex interference Open CASCADE's builder uses. *Grounds (step 2):* every boolean the suite runs at 1000 cases,
72 710 of them, logged every section vertex's point, tolerance and
members before and after — identical, all of them: no chain formed
outside the band, so no diameter bound is needed. The fixture's three
points, 1.7e-7 and 2.4e-7 apart at 1e-7, are one vertex under every
order; neither "pairwise within the larger tolerance" nor a grown ball
merges them.

**2. A merged vertex's pcurves meet on its own (u, v).** A vertex whose
members lie further apart than a face's tolerance cannot be met by exact
pcurves: L2 holds a junction to the parametric tolerance and the mesh
evaluates it to the face's. A section vertex has one (u, v) per face per
side of a seam — an operand edge's pcurve where one is paved there, else
its point's projection — and a section edge's pcurve whose end lies
further from it than half of L2's band is moved there, its edge's
tolerance raised by what the move costs (the growth rule's own reason,
`docs/DATA-MODEL.md` §Tolerances). The vertex's point prefers a member on
an operand edge, and a section curve is paved at its section crossing's
own parameter where the vertex holds one. Widening L2 and the mesh to the
vertex's tolerance instead would leave loops open in (u, v), which every
polygon downstream assumes closed. *Grounds (step 2b):* 3802 ends moved
over the suite at 1000 cases, the largest 1.92e-7 and at most 0.66 of its
vertex's tolerance; no blessed dump changed but the fixture's own.

**3. A conic hit one rounding under a whole turn is the hit at 0**,
within `POLYNOMIAL_ROUNDING` — the budget the quartic's coefficients
already carry — and no tolerance.

**4. A touch between two curves of one surface is resolved, and a block
along an operand edge is that edge's.** Two conics in planes that are not
parallel meet on the line of the two planes; `conic_crossings` finds their
common points there, tangent only at a double root to rounding, wherever
`intersect_curves` gave a touch — ADR-0016's rule one level down. Then a
section block every check point of which lies within the tolerance of a
piece of an operand edge of either face, between the same two vertices,
is that piece and not a section edge: built as one it would bound a
sliver of zero area. It is asked before the block's midpoint is asked to
lie inside both faces, and the piece is imaged once on the pair's other
face. `pcurve_on`'s plane arm maps a line or conic exactly only when it
lies in the plane, and fits a tilted one's projection. *Grounds (step
4):* the seam and a small circle through a sphere's pole 2e-4 of a radian
off it cross again 1.8e-4 on, a touch 8e-10 deep; `conic_crossings` finds
the far crossing and the pole to 5e-12, where re-asking the intersector
at the model's smallest tolerance missed one 3.5e-7 out. The
singular-slice property draws its turn round the whole turn and a hair
off the seam and is green at 8000.

**5. Fits: a fitted section is held to the exact branch at the same
parameter.** Its deviation is `|fit(t) − branch(t)|` alone, which bounds
the old surface term (a projection's distance is 1-Lipschitz, and the
branch lies on both surfaces to rounding). So an edge's tube holds the
true section at every meeting angle, and two fits of one section over two
regions lie within half a tolerance of each other. This needed a branch
continuous through a torus section's turning points, where two arcs met
up to 4.3e-7 apart; inside a turning point's cell the branch is walked in
`v`, anchored at the turn, and the arcs meet to rounding. *Grounds (step
5b):* control points per loop, surfaces → same parameter → curve
distance: metre cylinder pairs 60–129 → 60–137 → 60–133; a cylinder
against a sphere and a crossing cylinder at 20°, 5°, 1°: 105/77, 129/133,
133/235 → 117/79, 133/143, 153/263 → 119/79, 133/140, 141/257; torus
sections 27–107 → at most +32% → within 10%, at five to eight times the
time. Neither doubles; the same parameter is the stricter bound and the
one a property checks directly.

**6. Curves: an edge whose two faces lie on the pair's two surfaces is on
the pair's section by the surfaces' identity.** An edge of a pair's face
whose other face in its own operand lies on a surface `Coincident` with
the pair's other face is on their section to its own tolerance (E4), and
along a branch exactly when its midpoint lies on it; the curves are not
compared. Two fits of one traced section over different regions are two
splines no closed form compares, and decision 5 makes the midpoint
sufficient. *Grounds (step 6):* `frustum-stub-cut-then-fuse` — the cut's
section edge against the restoring fuse's own section — builds with the
oracle's counts, and the quadric identities spare no pair at 1000 poses.

**7. Faces: one shell touching itself at a vertex is refused by name.**
A result vertex whose face uses close into more than one fan is
`Reason::NonManifold` naming it, found before anything is assembled. A
`Solid` is manifold at every level already — ADR-0006 refuses a vertex two
lumps share and ADR-0004 a slit two faces touch along, and one shell
touching itself at a point is the same statement; Euler's parity and L5
both read it as broken, and admitting it changes what every vertex-fan
walk downstream may assume, for exact tangency from inside that leaves a
wall of zero thickness. Open CASCADE builds the body; it is the `General`
body's, taken when a consumer's regression asks.

## Alternatives

- **A fuzzy value** carried by every entity and compared by interval
  arithmetic: every predicate three-valued, and nothing downstream written
  for the third.
- **A snap pre-pass** moving one operand onto the other's contact: an
  operation that changes its input, and a tolerance chosen per contact.
- **Pairwise-within-the-larger-tolerance, or a grown ball**, for the
  closure: neither merges three points each apart by more than one
  tolerance and less than two; the first also depends on order.
- **Curve distance to the branch** for the fit: the same counts at five
  to eight times the time, and a looser bound.
- **Admitting the pinched vertex:** decision 7.

## Consequences

- `seam-a-tolerance-from-crossing-fuse`, `grazing-ball-bar-cut`,
  `frustum-stub-cut-then-fuse` and `pole-slice-beside-seam-cut` pass every
  corpus stage from `boolean/`; `singular-bore-cut` is
  `Reason::NonManifold` naming its vertex, where it was
  `Internal(Builder(NotClosed))`, and stays in `regression/` holding the
  desired body.
- The torus fixtures' and the fitted sections' dumps were
  re-blessed at steps 5 and 5b: same topology, numbers within 4.4e-8
  and 1e-8, fits −4% to +35% control points.
- `arris_geom::conic_crossings` is new; `SECTION_FIT_FRACTION`'s and
  `pcurve_on`'s guarantees changed; `Reason::NonManifold` names one more
  case. No public type changed shape.
- The band's larger part — no verdict a hair off parallel or tangent,
  slivers the polygons do not resolve, the builder refusing near-tangent
  leftovers — is none of this and is `docs/BACKLOG.md`'s, with its
  fixtures under `regression/`.
