//! The fixture corpus, one test per fixture and every variant of it
//! (`docs/ROADMAP.md` §Fixtures): the recipe built in Arris, the checker
//! at `Full`, counts and genus against the oracle, STEP read back by the
//! oracle, provenance accounting, the dump diffed against `dump.txt`.
//! Every fixture is live.

use arris_debug::corpus::{self, CorpusError};
use arris_debug::fixtures;

fn run(name: &str) {
    let dir = fixtures::corpus_root().join(name);
    let fixture = fixtures::load(&dir).unwrap();
    for variant in fixture.recipe.variant_names() {
        if let Err(e) = corpus::run(&dir, &variant) {
            panic!("{name} [{variant}]: {e}");
        }
    }
}

/// One variant of a fixture, for a recipe whose variants are worth
/// failing apart.
fn run_variant(name: &str, variant: &str) {
    let dir = fixtures::corpus_root().join(name);
    if let Err(e) = corpus::run(&dir, variant) {
        panic!("{name} [{variant}]: {e}");
    }
}

#[test]
fn primitive_box() {
    run("primitive/box");
}

#[test]
fn primitive_cylinder() {
    run("primitive/cylinder");
}

#[test]
fn transform_posed_cylinder() {
    run("transform/posed-cylinder");
}

/// The hollow ring moved: two shells carried whole, the cavity still the
/// one lump's void.
#[test]
fn transform_moved_hollow_ring() {
    run("transform/moved-hollow-ring");
}

#[test]
fn boolean_through_hole() {
    run("boolean/through-hole");
}

#[test]
fn boolean_blind_hole() {
    run("boolean/blind-hole");
}

/// The consumer's through-hole probe in its own units: a 0.1 m plate less
/// a r 0.02 cylinder, in a model whose default tolerance is a micrometre
/// — `w²t − πr²t = 8.7434e-5`.
#[test]
fn boolean_probe_through_hole_m() {
    run("boolean/probe-through-hole-m");
}

/// The consumer's blind-hole probe in the same units: the floor kept,
/// `w²t − πr²d = 1.8743e-4`.
#[test]
fn boolean_probe_blind_hole_m() {
    run("boolean/probe-blind-hole-m");
}

/// The consumer's flush-union probe in the same units: two 1 m cubes
/// sharing a face, volume 2.0.
#[test]
fn boolean_probe_flush_union_m() {
    run("boolean/probe-flush-union-m");
}

#[test]
fn boolean_bolt_pattern_8() {
    run("boolean/bolt-pattern-8");
}

/// Two boxes sharing a face: the flush case. The shared face vanishes
/// and the four edges around it are held once, from the first operand.
#[test]
fn boolean_flush_union() {
    run("boolean/flush-union");
}

#[test]
fn boolean_corner_union() {
    run("boolean/corner-union");
}

#[test]
fn boolean_corner_common() {
    run("boolean/corner-common");
}

#[test]
fn boolean_corner_cut() {
    run("boolean/corner-cut");
}

/// The two flush boxes' common is the shared face alone: nothing with
/// thickness, the runner's degenerate path.
#[test]
fn boolean_flush_common() {
    run("boolean/flush-common");
}

#[test]
fn boolean_disjoint_cut() {
    run("boolean/disjoint-cut");
}

#[test]
fn boolean_frame_cut() {
    run("boolean/frame-cut");
}

/// The target inside the tool: nothing survives, and the oracle records
/// no solid — the runner's degenerate path.
#[test]
fn boolean_swallow_cut() {
    run("boolean/swallow-cut");
}

/// A slab through the plate: two boxes, two lumps of one solid — Open
/// CASCADE's two solids (ADR-0006).
#[test]
fn boolean_split_cut() {
    run("boolean/split-cut");
}

/// The consumer's hollow box in its own units: one lump, its cavity a
/// void shell made of the tool's faces turned inward.
#[test]
fn boolean_enclosed_cavity() {
    run("boolean/enclosed-cavity");
}

/// Operands apart: two lumps, every face of both kept by id.
#[test]
fn boolean_disjoint_fuse() {
    run("boolean/disjoint-fuse");
}

/// A cylinder wholly inside a box: a void of three faces, the seam
/// carried with them.
#[test]
fn boolean_cavity_cylinder() {
    run("boolean/cavity-cylinder");
}

/// A box inside the cavity of a hollow box: three shells, two lumps, one
/// of them in the other's void.
#[test]
fn boolean_lump_in_cavity() {
    run("boolean/lump-in-cavity");
}

/// Two boxes touching along an edge: Open CASCADE builds two solids that
/// share it, Arris refuses with `Reason::NonManifold` — the runner's
/// expected-error path.
#[test]
fn boolean_edge_touching_fuse() {
    run("boolean/edge-touching-fuse");
}

#[test]
fn boolean_boss() {
    run("boolean/boss");
}

/// The boss's bottom cap coincident with the plate's top: the cap
/// vanishes, the plate's top is split by the cap's rim and keeps the
/// outside, and the rim is the wall's own edge.
#[test]
fn boolean_boss_flush() {
    run("boolean/boss-flush");
}

/// A rod filling a tube's bore: the coincident cylinder walls vanish,
/// the rod's discs sit beside the tube's annuli sharing the inner
/// circles — the cylinder–cylinder coincident arm, and the common
/// blocks of a periodic edge.
#[test]
fn boolean_coaxial_fuse() {
    run("boolean/coaxial-fuse");
}

/// A hole drilled at 30°: two ellipse sections, NURBS pcurves on the
/// wall, and a wall whose (u, v) region is a strip oblique to the
/// ruling — the case the tessellator flattens the ruled direction for
/// (ADR-0005).
#[test]
fn boolean_oblique_hole() {
    run("boolean/oblique-hole");
}

/// A cylinder touching the plate's side face from outside: the tangent
/// pair contributes no section edge and no split, and the result is
/// the plate with every id kept. Open CASCADE imprints the ruling; the
/// fixture states that convention in `analytic.counts_differ`.
#[test]
fn boolean_tangent_outside_cut() {
    run("boolean/tangent-outside-cut");
}

/// A blind hole whose wall touches a side face from inside along a
/// ruling interior to both: the slit no manifold `Solid` can carry,
/// `Reason::TangentContact` through the runner's expected-error path
/// (plan m4-booleans `⚠ OPEN` 1).
#[test]
fn boolean_tangent_hole() {
    run("boolean/tangent-hole");
}

/// The same solid as `through-hole` in another pose: both operands moved
/// by one rigid motion before the cut.
#[test]
fn boolean_posed_through_hole() {
    run("boolean/posed-through-hole");
}

/// Two coaxial cylinders: the pair has no section curve, and the bore is
/// the tool's wall reversed.
#[test]
fn boolean_coaxial_cut() {
    run("boolean/coaxial-cut");
}

/// Operands that share no material: the runner's degenerate path through
/// `common`.
#[test]
fn boolean_disjoint_common() {
    run("boolean/disjoint-common");
}

/// A sliver: the top and bottom faces are a D of one straight edge and
/// one arc under an eighth of a turn, which the checker's minimum
/// discretisation once flattened to a chord (plan step 9).
#[test]
fn boolean_sliver_common() {
    run("boolean/sliver-common");
}

/// Two equal cylinders crossing at 90°: the Steinmetz solid. The two
/// section ellipses cross each other at (0, ±R, 0), a section vertex no
/// operand edge made, and each wall's lens through the seam is two
/// faces. Open CASCADE also cuts an arc at the ellipse's parameter
/// origin; the fixture states that convention in `analytic.counts_differ`.
#[test]
fn boolean_cross_cylinders_common() {
    run("boolean/cross-cylinders-common");
}

/// The same cylinders fused: each wall keeps its two pieces outside the
/// other, meeting at the crossing vertices.
#[test]
fn boolean_cross_cylinders_fuse() {
    run("boolean/cross-cylinders-fuse");
}

/// The target minus the tool that is as wide as it: two lumps whose
/// closures touch at the crossing vertices, `Reason::NonManifold`
/// through the runner's expected-error path (ADR-0006).
#[test]
fn boolean_cross_cylinders_cut() {
    run("boolean/cross-cylinders-cut");
}

/// The common at ψ = 60°: ellipses of two different major radii still
/// crossing at (0, ±R, 0), `16R³/(3 sin ψ)`.
#[test]
fn boolean_oblique_cross_common() {
    run("boolean/oblique-cross-common");
}

/// The Steinmetz solid with the tool's seam through a crossing vertex:
/// the seam touches the target's wall there, and the touch landing on
/// the section vertex cuts the seam. Open CASCADE's extra arc vertex is
/// stated in `analytic.counts_differ`.
#[test]
fn boolean_seam_through_crossing_common() {
    run("boolean/seam-through-crossing-common");
}

/// The crossing-cylinder fuse with the second cylinder's seam turned to
/// just beside a crossing vertex rather than through it: the same solid
/// as at any turn, so the result is held to cross-cylinders-fuse's
/// closed forms — which Open CASCADE misses in this band, stated in
/// `analytic.measure_differs` (ADR-0015). The seam's touch is resolved
/// into its crossings (ADR-0016) and the sliver between the seam and the
/// two ellipses is decided by the transversal rule.
#[test]
fn boolean_seam_beside_crossing_fuse() {
    run("boolean/seam-beside-crossing-fuse");
}

/// The same fuse with the seam between one and two tolerances from the
/// crossing vertex: its two crossings and that vertex are three points
/// too far apart to merge and too close for the blocks between them to
/// clear the seam's band. The desired body is the one at −90°, the three
/// one vertex.
#[test]
#[ignore = "Fault::Split, a section edge of f1 ends at a node nothing else reaches: the blocks between the seam's crossings and the crossing vertex are dropped as boundary (docs/BACKLOG.md, features a tolerance apart)"]
fn regression_seam_a_tolerance_from_crossing_fuse() {
    run("regression/seam-a-tolerance-from-crossing-fuse");
}

/// A drill touching the main wall from inside, at a singular point of the
/// traced section: the pave model makes the point one vertex that ends
/// both branches, and the main wall is left two pieces meeting only
/// there.
#[test]
#[ignore = "OpError::Internal, BuildError::NotClosed: the main wall's two pieces meet only at the singular vertex, pinched between the drill's exits, and the builder cannot close the shell there (docs/BACKLOG.md, features a tolerance apart)"]
fn regression_singular_bore_cut() {
    run("regression/singular-bore-cut");
}

/// A tee of equal radii: the branch's rim circle touches the main wall
/// exactly at the two crossing vertices, and each touch cuts the rim
/// there.
#[test]
fn boolean_tee_fuse() {
    run("boolean/tee-fuse");
}

/// A tee of unequal radii: the branch's wall meets the main wall in one
/// traced loop, one periodic fitted curve paved once where the branch's
/// seam pierces the main wall (ADR-0018).
#[test]
fn boolean_tee_unequal_fuse() {
    run("boolean/tee-unequal-fuse");
}

/// The same operands' cut: a pocket opening through the main wall along
/// the loop.
#[test]
fn boolean_tee_unequal_cut() {
    run("boolean/tee-unequal-cut");
}

/// The same operands' common: the branch inside the main.
#[test]
fn boolean_tee_unequal_common() {
    run("boolean/tee-unequal-common");
}

/// A drill on a skew axis breaking out of the main cylinder's side: one
/// traced loop, a notch.
#[test]
fn boolean_skew_hole_cut() {
    run("boolean/skew-hole-cut");
}

/// A drill on a skew axis inside the main cylinder: two traced loops, a
/// bore, genus 1.
#[test]
fn boolean_skew_bore_cut() {
    run("boolean/skew-bore-cut");
}

/// The consumer's cylinder − cylinder transversal probe in its own units:
/// two parallel walls meeting in two rulings, `(πr² − lens)·h =
/// 2.2079e-5`.
#[test]
fn boolean_parallel_cylinders_cut() {
    run("boolean/parallel-cylinders-cut");
}

/// The same operands' common: the lens prism.
#[test]
fn boolean_parallel_cylinders_common() {
    run("boolean/parallel-cylinders-common");
}

/// The same operands fused, and flush: the rim circles crossing on the
/// coincident caps at the rulings' ends.
#[test]
fn boolean_parallel_cylinders_fuse() {
    run("boolean/parallel-cylinders-fuse");
}

/// A ruling on the target's seam: that block is the seam edge, not a
/// section edge.
#[test]
fn boolean_parallel_cylinders_seam() {
    run("boolean/parallel-cylinders-seam");
}

/// Two walls touching from outside along a ruling: the curvature rule puts
/// each outside the other, and the cut is the target with every id kept.
/// Open CASCADE imprints the ruling (`analytic.counts_differ`).
#[test]
fn boolean_tangent_cylinders_cut() {
    run("boolean/tangent-cylinders-cut");
}

/// The same operands fused: both walls survive through the contact,
/// `Reason::TangentContact` through the runner's expected-error path.
#[test]
fn boolean_tangent_cylinders_fuse() {
    run("boolean/tangent-cylinders-fuse");
}

/// A pin touching a bore's wall from inside: the pin's wall is inside the
/// bore, and the fuse is the bore with every id kept.
#[test]
fn boolean_pin_in_bore_fuse() {
    run("boolean/pin-in-bore-fuse");
}

/// The pin cut from the bore: both walls survive through the contact,
/// `Reason::TangentContact`.
#[test]
fn boolean_pin_in_bore_cut() {
    run("boolean/pin-in-bore-cut");
}

/// Two short cylinders crossing at 30°: each of the tool's cap planes cuts
/// the target's wall in an ellipse coplanar with the tool's rim circle, and
/// whether that rim lies along the ellipse is decided without the quartic.
#[test]
fn boolean_short_cross_cylinders_fuse() {
    run("boolean/short-cross-cylinders-fuse");
}

/// A box with an elliptic cylinder cut through it: the pave model's
/// quadric guard refuses the elliptic-cylinder face before any
/// intersector runs, `OpError::Unsupported` naming it (ADR-0014).
#[test]
fn boolean_elliptic_operand_cut() {
    run("boolean/elliptic-operand-cut");
}

/// A rectangle with a circular hole extruded: `boolean/through-hole`'s
/// solid and numbers by the other path.
#[test]
fn sweep_extrude_plate_with_hole() {
    run("sweep/extrude-plate-with-hole");
}

/// A stadium with a hole: two half-cylinder faces with no seam beside the
/// seamed bore, their boxes apart so the checker decides every pair.
#[test]
fn sweep_extrude_slot() {
    run("sweep/extrude-slot");
}

/// The plate-with-hole profile on `z = 10` extruded down: the same solid
/// again, the profile face on top keeping its frame.
#[test]
fn sweep_extrude_downward() {
    run("sweep/extrude-downward");
}

/// A full ellipse with a turned major axis extruded: one seamed elliptic
/// cylinder between two planes, volume `π a b h` (ADR-0014).
#[test]
fn sweep_extrude_ellipse() {
    run("sweep/extrude-ellipse");
}

/// Two half-ellipses joined by lines: two elliptic-cylinder faces with no
/// seam, their full sections crossing off the faces.
#[test]
fn sweep_extrude_elliptic_slot() {
    run("sweep/extrude-elliptic-slot");
}

/// A rectangle with an elliptic hole: the seamed elliptic bore inside
/// four walls, `boolean/through-hole`'s counts.
#[test]
fn sweep_extrude_plate_elliptic_hole() {
    run("sweep/extrude-plate-elliptic-hole");
}

/// A rectangle revolved a full turn about z: two seamed walls and two
/// annuli of two closed rises each — `boolean/coaxial-cut`'s solid by the
/// other path.
#[test]
fn sweep_revolve_tube() {
    run("sweep/revolve-tube");
}

/// The same rectangle a quarter turn: two flat ends, every rise an arc.
#[test]
fn sweep_revolve_quarter() {
    run("sweep/revolve-quarter");
}

/// An L revolved 270°: the walls' (u, v) regions run past `π` with no
/// seam, two annular sectors of one notch face each other.
#[test]
fn sweep_revolve_l_profile() {
    run("sweep/revolve-l-profile");
}

/// A full turn of a rectangle with a rectangular hole: the hole closes
/// into a ring-shaped cavity, one lump of two shells.
#[test]
fn sweep_revolve_hollow_ring() {
    run("sweep/revolve-hollow-ring");
}

/// The consumer's rectangle with a side on the axis: a solid cylinder in
/// a full turn, and in a quarter and three-quarter turn a sector whose
/// flat ends share the edge on the axis — reflex there past half a turn.
#[test]
fn sweep_revolve_onto_axis() {
    run("sweep/revolve-onto-axis");
}

/// The same profile in the consumer's units: a full turn at a micrometre
/// default tolerance, volume 2π.
#[test]
fn sweep_probe_revolve_onto_axis_m() {
    run("sweep/probe-revolve-onto-axis-m");
}

/// A rectangle on the axis with a notch cut in from it: in a full turn
/// the notch closes into a void of the one lump, two shells from one
/// loop; a quarter turn opens it onto the flat ends.
#[test]
fn sweep_revolve_notch_to_axis() {
    run("sweep/revolve-notch-to-axis");
}

/// A trapezoid revolved about z: a frustum less its bore, its cone
/// narrowing along the axis and, in `widening`, widening; a quarter turn's
/// flat ends meet the cone along a ruling (ADR-0008).
#[test]
fn sweep_revolve_frustum() {
    run("sweep/revolve-frustum");
}

/// An arc about the origin revolved a full turn: a spherical zone less
/// its bore, S5 deciding the sphere against the annuli and the bore by the
/// meridian arm (ADR-0008).
#[test]
fn sweep_revolve_barrel() {
    run("sweep/revolve-barrel");
}

/// A circle revolved about z: `sample::torus` as a revolve builds it, one
/// face and one vertex; a quarter turn's discs cut the torus in its tube
/// circles (ADR-0008).
#[test]
fn sweep_revolve_ring() {
    run("sweep/revolve-ring");
}

/// An ellipse revolved about z: Open CASCADE builds the elliptic torus
/// and Arris refuses it, `Reason::EllipticRevolve` naming the segment,
/// since the surface it would sweep has no variant (ADR-0014).
#[test]
fn sweep_revolve_ellipse() {
    run("sweep/revolve-ellipse");
}

/// The consumer's 2-cube with one vertical edge filleted: a plane–plane
/// blend, a cylinder between two circle ends (ADR-0007), at `r = 0.2`
/// and at `r = 0.5`.
#[test]
fn blend_box_edge_fillet() {
    run("blend/box-edge-fillet");
}

/// The same solid rotated: a cap edge of the cube, the blend's ends on
/// two side faces.
#[test]
fn blend_box_cap_edge_fillet() {
    run("blend/box-cap-edge-fillet");
}

/// The cube rotated about (1, 1, 1) and moved before the fillet: the
/// same blend in an oblique pose, the edge and every probe named by the
/// rotation's closed form.
#[test]
fn blend_box_posed_edge_fillet() {
    run("blend/box-posed-edge-fillet");
}

/// An extruded parallelogram's slanted top edge: the end faces are
/// oblique to the edge, so each end trim is an ellipse arc with a fitted
/// pcurve on the blend (the oblique-section rule).
#[test]
fn blend_box_oblique_end() {
    run("blend/box-oblique-end");
}

/// An extruded L's inner vertical edge: a concave plane–plane blend that
/// adds material, its cylinder's axis in the notch and the blend face
/// reversed against it.
#[test]
fn blend_l_inner_edge() {
    run("blend/l-inner-edge");
}

/// The cube's four vertical edges in one call: four disjoint blends, each
/// cap edge cut at both its ends by two of them.
#[test]
fn blend_box_four_verticals() {
    run("blend/box-four-verticals");
}

/// The consumer's 2-cube with one vertical edge chamfered: a plane–plane
/// chamfer, a plane between two segments on the caps, at `d = 0.2` and
/// at `d = 0.5`.
#[test]
fn blend_box_edge_chamfer() {
    run("blend/box-edge-chamfer");
}

/// The cube's four vertical edges chamfered in one call.
#[test]
fn blend_box_four_vertical_chamfers() {
    run("blend/box-four-vertical-chamfers");
}

/// The miter's chamfer twin: a vertical and a cap edge at one corner, two
/// chamfer planes meeting in a line, every face pair checked at `Full`.
#[test]
fn blend_box_corner_chamfers() {
    run("blend/box-corner-chamfers");
}

/// A second fillet on a filleted body: the extruded square's side edge
/// 0–1, then 1–2 of that result, the two blends sharing the side face
/// the first modified.
#[test]
fn blend_second_fillet() {
    run("blend/second-fillet");
}

/// The consumer's second-blend probe in its own units: the same two
/// disjoint blends on a 0.1 m cube at a micrometre default tolerance.
#[test]
fn blend_probe_second_fillet_m() {
    run("blend/probe-second-fillet-m");
}

/// The consumer's probe of a vertical and a cap edge blended in one call,
/// in the same units: the two blends share the side face x = 0 without
/// meeting on it.
#[test]
fn blend_probe_cap_and_vertical_fillet_m() {
    run("blend/probe-cap-and-vertical-fillet-m");
}

/// The cap edge of a face a first blend trimmed, ending where that
/// blend's contact meets its arc: a tangent chain Open CASCADE follows,
/// `Reason::TangentChain` through the runner's expected-error path.
#[test]
fn blend_tangent_chain_cap_edge() {
    run("blend/tangent-chain-cap-edge");
}

/// A rise ending at a vertex of five edges, where a box stands on its
/// corner on another's top edge: `Reason::VertexBlend`.
#[test]
fn blend_five_edge_vertex() {
    run("blend/five-edge-vertex");
}

/// Three fillets at one box corner: the sphere corner, an octant about
/// the ball's centre tangent to the three cylinders, its pole a degenerate
/// edge (ADR-0007). S5 decides the sphere against each cylinder and each
/// plane by the meridian arm, so nothing is unchecked.
#[test]
fn blend_box_corner_three_fillets() {
    run("blend/box-corner-three-fillets");
}

/// Every edge of the cube filleted in one call: twelve cylinders and eight
/// sphere corners, no face keeping a vertex of the box.
#[test]
fn blend_box_all_edges_fillet() {
    run("blend/box-all-edges-fillet");
}

/// Three chamfers at one box corner, meeting in a triangle.
#[test]
fn blend_box_corner_three_chamfers() {
    run("blend/box-corner-three-chamfers");
}

/// The miter: the vertical and the cap edge at one corner blended in one
/// call, two cylinders meeting in the ellipse of their bisecting plane
/// (ADR-0007). S5 decides the two blend cylinders by the equal-radius
/// crossing arm, so the checker stage has nothing unchecked.
#[test]
fn blend_fillet_miter() {
    run("blend/fillet-miter");
}

/// A half disc's chord edge: a plane against a cylinder along a ruling,
/// convex, the blend cylinder tangent to the arc face along a ruling —
/// S5 decides the pair by the parallel-axis arm's inside tangency.
#[test]
fn blend_d_chord_edge() {
    run("blend/d-chord-edge");
}

/// A half-round rib's root on a plate: the ruling arm concave, the blend
/// adding material, tangent to the rib from outside.
#[test]
fn blend_rib_root_edge() {
    run("blend/rib-root-edge");
}

/// A hole's top rim: a plane against a cylinder along a circle, convex,
/// the blend a quarter of a torus coaxial with the hole with no ends
/// (ADR-0007), S5 deciding it against the top face and the wall by the
/// meridian arm (ADR-0008).
#[test]
fn blend_hole_rim_fillet() {
    run("blend/hole-rim-fillet");
}

/// The same rim chamfered: a 45° cone coaxial with the hole.
#[test]
fn blend_hole_rim_chamfer() {
    run("blend/hole-rim-chamfer");
}

/// A boss's base on a revolved disc: the circle arm concave, the torus
/// adding material, every face on the one axis.
#[test]
fn blend_boss_base_fillet() {
    run("blend/boss-base-fillet");
}

/// The same recipe under three parameter sets, one test each so a
/// variant that drifts says which: the eight bolt holes' chain is
/// `crates/arris/tests/provenance.rs`'s subject, and these hold each
/// variant's solid to the oracle and to its own `dump.<variant>.txt`.
#[test]
fn provenance_bolt_pattern_rebuild() {
    run_variant("provenance/bolt-pattern-rebuild", "default");
}

#[test]
fn provenance_bolt_pattern_rebuild_thicker_wider() {
    run_variant("provenance/bolt-pattern-rebuild", "thicker-wider");
}

#[test]
fn provenance_bolt_pattern_rebuild_tighter() {
    run_variant("provenance/bolt-pattern-rebuild", "tighter");
}

/// The split-order fixtures (ADR-0009), one test per variant as above:
/// `provenance.rs` holds piece `k` of every split face to the same
/// neighbours in every variant; these hold each variant's solid.
#[test]
fn provenance_split_bar_cut() {
    run_variant("provenance/split-bar-cut", "default");
}

#[test]
fn provenance_split_bar_cut_left() {
    run_variant("provenance/split-bar-cut", "left");
}

#[test]
fn provenance_split_bar_cut_right() {
    run_variant("provenance/split-bar-cut", "right");
}

#[test]
fn provenance_split_bar_cut_narrow() {
    run_variant("provenance/split-bar-cut", "narrow");
}

#[test]
fn provenance_split_frame_cut() {
    run_variant("provenance/split-frame-cut", "default");
}

#[test]
fn provenance_split_frame_cut_left() {
    run_variant("provenance/split-frame-cut", "left");
}

#[test]
fn provenance_split_frame_cut_right() {
    run_variant("provenance/split-frame-cut", "right");
}

#[test]
fn provenance_split_frame_cut_narrow() {
    run_variant("provenance/split-frame-cut", "narrow");
}

#[test]
fn provenance_split_cylinder_seam() {
    run_variant("provenance/split-cylinder-seam", "default");
}

#[test]
fn provenance_split_cylinder_seam_turned_back() {
    run_variant("provenance/split-cylinder-seam", "turned-back");
}

#[test]
fn provenance_split_cylinder_seam_turned_on() {
    run_variant("provenance/split-cylinder-seam", "turned-on");
}

#[test]
fn provenance_split_cylinder_seam_narrow() {
    run_variant("provenance/split-cylinder-seam", "narrow");
}

#[test]
fn provenance_split_cross_common() {
    run_variant("provenance/split-cross-common", "default");
}

#[test]
fn provenance_split_cross_common_larger() {
    run_variant("provenance/split-cross-common", "larger");
}

#[test]
fn provenance_split_cross_common_longer() {
    run_variant("provenance/split-cross-common", "longer");
}

#[test]
fn provenance_split_cross_common_turned() {
    run_variant("provenance/split-cross-common", "turned");
}

/// The edge split-order fixture (ADR-0009, step 2): two notches into one
/// box edge, the second splitting the piece the first left.
#[test]
fn provenance_split_edge_notch() {
    run_variant("provenance/split-edge-notch", "default");
}

#[test]
fn provenance_split_edge_notch_slid() {
    run_variant("provenance/split-edge-notch", "slid");
}

#[test]
fn provenance_split_edge_notch_apart() {
    run_variant("provenance/split-edge-notch", "apart");
}

#[test]
fn provenance_split_edge_notch_narrow() {
    run_variant("provenance/split-edge-notch", "narrow");
}

/// A variant the recipe does not have fails naming it.
#[test]
fn an_unknown_variant_fails_with_its_name() {
    let dir = fixtures::corpus_root().join("primitive/box");
    let err = corpus::run(&dir, "nope").unwrap_err();
    assert!(
        matches!(&err, CorpusError::Variant { variant, .. } if variant == "nope"),
        "{err}"
    );
}

/// A committed dump that differs by one id fails with the diff, and a
/// missing one says how to write it.
#[test]
fn a_dump_that_differs_by_one_id_fails_with_the_diff() {
    let scratch = tempdir("dump-diff");
    let from = fixtures::corpus_root().join("primitive/cylinder");
    for file in ["fixture.json", "expected.json"] {
        std::fs::copy(from.join(file), scratch.join(file)).unwrap();
    }
    let err = corpus::run(&scratch, "default").unwrap_err();
    assert!(
        matches!(&err, CorpusError::Dump { what, .. } if what.contains("not committed")),
        "{err}"
    );
    let dump = std::fs::read_to_string(from.join("dump.txt")).unwrap();
    assert!(dump.contains("coedge +e1 "), "the seam walked up");
    std::fs::write(
        scratch.join("dump.txt"),
        dump.replacen("coedge +e1 ", "coedge +e7 ", 1),
    )
    .unwrap();
    let err = corpus::run(&scratch, "default").unwrap_err();
    match &err {
        CorpusError::Dump { what, .. } => {
            let committed = what
                .lines()
                .any(|l| l.starts_with('-') && l.contains("coedge +e7"));
            let built = what
                .lines()
                .any(|l| l.starts_with('+') && l.contains("coedge +e1"));
            assert!(committed && built, "{what}");
        }
        other => panic!("{other}"),
    }
    std::fs::write(scratch.join("dump.txt"), &dump).unwrap();
    corpus::run(&scratch, "default").unwrap();
}

fn tempdir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("arris-corpus-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
