//! The booleans at random poses (ADR-0004): a
//! box and a cylinder from `prop::body`, both operand orders — volume
//! and area additivity, the cut identity, commutativity of `fuse` and
//! `common`, results of several lumps held to the same identities
//! (ADR-0006), and every result clean at `Full` with nothing unchecked. A
//! failure
//! prints the shrunk pair and the seed, and becomes a fixture under
//! `tests/fixtures/boolean/` (`tests/fixtures/README.md` §Property-test
//! failures).

use arris_debug::prop::body::{Boxed, Cylindrical, OverlappingPair, TangentPair};
use arris_debug::testing::{REL, close_to, fail};
use arris_debug::{dump_text, prop, prop_shards};
use arris_ops::arris_check::arris_topo::arris_math::nalgebra::{Quaternion, UnitQuaternion};
use arris_ops::arris_check::arris_topo::arris_math::{Axis, Isometry, Point3, Vec3};
use arris_ops::arris_check::arris_topo::provenance::audit;
use arris_ops::arris_check::arris_topo::{Body, Model, Provenance};
use arris_ops::arris_check::{Level, check};
use arris_ops::measure::{MassProperties, mass_properties};
use arris_ops::{OpError, Reason, common, cut, fuse};
use proptest::prelude::*;

/// A boolean of two bodies: `fuse`, `common` or `cut`.
type Boolean = fn(&mut Model, Body, Body) -> Result<(Body, Provenance), OpError>;

/// [`arris_debug::testing::close_to`] at [`REL`].
fn close(a: f64, b: f64, floor: f64) -> bool {
    close_to(a, b, floor, REL)
}

/// The relative bound an identity holds to when its two sides are built
/// from different fittings of the same curve: `REL` plus what the
/// model's own tolerance permits over the body's size. A pcurve is
/// fitted to within its edge's tolerance, so a face's (u, v) region is
/// bounded to within `tol` in 3D and every mass property — each an
/// integral over that boundary — carries a relative error of order
/// `tol` over a length of the body, taken as `√A`. Measured on the case
/// below: the difference scales linearly with the model's tolerance,
/// 7.4e-9 in a volume of 7.2 at `tol` 1e-7 and 8.3e-11 at 1e-9. `REL`
/// alone is a literal, and the kernel's rule is that the tolerance is
/// the model's.
fn fitted_rel(m: &Model, p: &MassProperties) -> f64 {
    REL + m.precision().default_tolerance / p.area.sqrt()
}

/// `op(a, b)`, its result clean at `Full` with nothing unchecked, and its
/// mass properties.
fn run(
    m: &mut Model,
    name: &str,
    op: Boolean,
    a: Body,
    b: Body,
) -> Result<(Body, MassProperties), TestCaseError> {
    let (body, provenance) = op(m, a, b).map_err(|e| fail(format!("{name}: {e}")))?;
    let report = check(m, body, Level::Full);
    if !report.is_ok() || !report.unchecked().is_empty() {
        return Err(fail(format!("{name}: not clean at Full\n{report}")));
    }
    audit(m, &[a, b], body, &provenance).map_err(|e| fail(format!("{name}: provenance: {e}")))?;
    let props = mass_properties(m, body).map_err(|e| fail(format!("{name}: measure: {e}")))?;
    Ok((body, props))
}

/// The operands of `pair` in a fresh model, with their mass properties.
fn operands(
    pair: &OverlappingPair,
) -> Result<(Model, Body, Body, MassProperties, MassProperties), TestCaseError> {
    let mut m = Model::default();
    let (a, b) = pair.build(&mut m).map_err(fail)?;
    let pa = mass_properties(&m, a).map_err(fail)?;
    let pb = mass_properties(&m, b).map_err(fail)?;
    Ok((m, a, b, pa, pb))
}

/// `V(A ∪ B) + V(A ∩ B) = V(A) + V(B)`, and the same for the areas: the
/// boundary of the union and the boundary of the common partition the
/// two operands' boundaries between them.
fn assert_additive(
    union: &MassProperties,
    inter: &MassProperties,
    pa: &MassProperties,
    pb: &MassProperties,
) -> Result<(), TestCaseError> {
    let (v, s) = (pa.volume + pb.volume, pa.area + pb.area);
    prop_assert!(
        close(union.volume + inter.volume, v, v),
        "V(A ∪ B) + V(A ∩ B) = {} + {}, V(A) + V(B) = {} + {}",
        union.volume,
        inter.volume,
        pa.volume,
        pb.volume
    );
    prop_assert!(
        close(union.area + inter.area, s, s),
        "A(A ∪ B) + A(A ∩ B) = {} + {}, A(A) + A(B) = {} + {}",
        union.area,
        inter.area,
        pa.area,
        pb.area
    );
    Ok(())
}

/// `V(A − B) + V(A ∩ B) = V(A)`.
fn assert_cut_identity(
    diff: &MassProperties,
    inter: &MassProperties,
    pa: &MassProperties,
    pb: &MassProperties,
) -> Result<(), TestCaseError> {
    prop_assert!(
        close(diff.volume + inter.volume, pa.volume, pa.volume + pb.volume),
        "V(A − B) + V(A ∩ B) = {} + {}, V(A) = {}",
        diff.volume,
        inter.volume,
        pa.volume
    );
    Ok(())
}

prop_shards! {
    /// A cylinder that clears every edge of the box: every outcome is known
    /// by construction. `fuse`, `common` and `box − cylinder` are one clean
    /// shell each, `cylinder − box` two lumps of one solid — the cylinder's
    /// two ends — and all of them obey the identities.
    piercing_pairs_obey_every_identity_whatever_their_lumps
        [shard_0 shard_1 shard_2 shard_3]
        (pair) = prop::body::piercing_pair() => {
            let (mut m, a, b, pa, pb) = operands(&pair)?;
            let (_, union) = run(&mut m, "fuse(a, b)", fuse, a, b)?;
            let (_, inter) = run(&mut m, "common(a, b)", common, a, b)?;
            let (_, diff) = run(&mut m, "cut(a, b)", cut, a, b)?;
            assert_additive(&union, &inter, &pa, &pb)?;
            assert_cut_identity(&diff, &inter, &pa, &pb)?;
            let (ends, ends_props) = run(&mut m, "cut(b, a)", cut, b, a)?;
            prop_assert_eq!(
                m.shells(ends).map_err(fail)?.len(),
                2,
                "cut(b, a): the cylinder's two ends"
            );
            assert_cut_identity(&ends_props, &inter, &pb, &pa)?;
            Ok(())
        }
}

prop_shards! {
    /// Any overlapping pair, the wall free to cross the box's edges: `fuse`
    /// and `common` are clean and additive; `box − cylinder` is clean and
    /// obeys the identity whether it is one lump or several (a corner
    /// sliced off).
    overlapping_pairs_fuse_and_common_additively
        [shard_0 shard_1 shard_2 shard_3 shard_4 shard_5 shard_6 shard_7
         shard_8 shard_9 shard_10 shard_11]
        (pair) = prop::body::overlapping_pair() => {
            let (mut m, a, b, pa, pb) = operands(&pair)?;
            let (_, union) = run(&mut m, "fuse(a, b)", fuse, a, b)?;
            let (_, inter) = run(&mut m, "common(a, b)", common, a, b)?;
            assert_additive(&union, &inter, &pa, &pb)?;
            let (_, diff) = run(&mut m, "cut(a, b)", cut, a, b)?;
            assert_cut_identity(&diff, &inter, &pa, &pb)?;
            Ok(())
        }
}

/// A cylinder touching a random face of the box from outside along a
/// ruling through the face's interior (plan step 11): `box − cylinder`
/// is the box — the same faces, edges and vertices by id, the same mass
/// properties — `common` is the designed `Empty`, and `fuse`, which
/// would keep the face and the wall touching along the contact, is the
/// designed `TangentContact`; the model is as it was after each
/// refusal. The touches land in both places — a box edge on the wall
/// where the ruling crosses the face's rim, a rim circle on the face
/// where the cylinder ends inside it — and neither paves anything.
#[test]
fn a_cylinder_tangent_to_a_box_face_leaves_the_box_and_shares_nothing() {
    prop::check(prop::body::tangent_pair(), |pair: TangentPair| {
        let mut m = Model::default();
        let (a, b) = pair.build(&mut m).map_err(fail)?;
        let pa = mass_properties(&m, a).map_err(fail)?;
        let before = dump_text(&m, a).map_err(fail)?;
        let (body, _) = run(&mut m, "cut(box, cylinder)", cut, a, b)?;
        let faces = |body: Body| -> Result<Vec<_>, TestCaseError> {
            Ok(m.faces(body).map_err(fail)?.iter().map(|f| f.id).collect())
        };
        prop_assert_eq!(faces(body)?, faces(a)?, "every face of the box kept by id");
        let after = mass_properties(&m, body).map_err(fail)?;
        prop_assert!(close(after.volume, pa.volume, pa.volume));
        prop_assert!(close(after.area, pa.area, pa.area));
        prop_assert!((after.centroid - pa.centroid).norm() <= REL * pa.area.sqrt());
        match common(&mut m, a, b) {
            Err(OpError::Degenerate {
                reason: Reason::Empty,
                ..
            }) => {}
            Ok(_) => return Err(fail("common(box, cylinder): a touch shares no material")),
            Err(e) => return Err(fail(format!("common(box, cylinder): {e}"))),
        }
        match fuse(&mut m, a, b) {
            Err(OpError::Degenerate {
                reason: Reason::TangentContact,
                ..
            }) => {}
            Ok(_) => {
                return Err(fail(
                    "fuse(box, cylinder): the face and the wall would share a slit",
                ));
            }
            Err(e) => return Err(fail(format!("fuse(box, cylinder): {e}"))),
        }
        prop_assert_eq!(
            dump_text(&m, a).map_err(fail)?,
            before,
            "the model is as it was"
        );
        Ok(())
    });
}

/// The dump with every number, and the sign in front of every id,
/// replaced by `#`, its lines sorted: two results that differ only in
/// which entity got which id, the order the faces were assembled in and
/// which way a section edge runs have the same text.
fn up_to_ids(dump: &str) -> Vec<String> {
    let mut lines: Vec<String> = dump
        .lines()
        .map(|line| {
            let mut out = String::with_capacity(line.len());
            let c: Vec<char> = line.chars().collect();
            let mut i = 0;
            while i < c.len() {
                let id_sign = (c[i] == '+' || c[i] == '-')
                    && c.get(i + 1).is_some_and(|x| "bsfevp".contains(*x))
                    && c.get(i + 2).is_some_and(char::is_ascii_digit);
                if id_sign {
                    i += 1;
                    continue;
                }
                let number = c[i].is_ascii_digit()
                    || (c[i] == '-' && c.get(i + 1).is_some_and(char::is_ascii_digit));
                if !number {
                    out.push(c[i]);
                    i += 1;
                    continue;
                }
                out.push('#');
                i += usize::from(c[i] == '-');
                while c.get(i).is_some_and(|x| x.is_ascii_digit() || *x == '.') {
                    i += 1;
                }
                if c.get(i) == Some(&'e') {
                    let mut j = i + 1;
                    j += usize::from(c.get(j).is_some_and(|x| *x == '-' || *x == '+'));
                    if c.get(j).is_some_and(char::is_ascii_digit) {
                        i = j;
                        while c.get(i).is_some_and(char::is_ascii_digit) {
                            i += 1;
                        }
                    }
                }
            }
            out
        })
        .collect();
    lines.sort();
    lines
}

/// Volume, area, centroid and inertia equal to `REL`.
fn assert_same_properties(
    x: &MassProperties,
    y: &MassProperties,
    what: &str,
) -> Result<(), TestCaseError> {
    assert_same_properties_to(x, y, what, REL)
}

/// The same to `rel`: [`fitted_rel`] where the two bodies were built
/// from different fittings of the same curves.
fn assert_same_properties_to(
    x: &MassProperties,
    y: &MassProperties,
    what: &str,
    rel: f64,
) -> Result<(), TestCaseError> {
    prop_assert!(
        close_to(x.volume, y.volume, 1.0, rel),
        "{what}: volumes {} and {}",
        x.volume,
        y.volume
    );
    prop_assert!(
        close_to(x.area, y.area, 1.0, rel),
        "{what}: areas {} and {}",
        x.area,
        y.area
    );
    let scale = x.centroid.coords.abs().max().max(1.0);
    prop_assert!(
        (x.centroid - y.centroid).norm() <= rel * scale,
        "{what}: centroids {} and {}",
        x.centroid,
        y.centroid
    );
    let scale = x.inertia.abs().max().max(1.0);
    prop_assert!(
        (x.inertia - y.inertia).abs().max() <= rel * scale,
        "{what}: inertia {} and {}",
        x.inertia,
        y.inertia
    );
    Ok(())
}

prop_shards! {
    /// `fuse(a, b)` and `fuse(b, a)`, `common(a, b)` and `common(b, a)`: the
    /// same mass properties, the same counts, the same dump up to ids.
    fuse_and_common_commute_at_random_poses
        [shard_0 shard_1 shard_2 shard_3 shard_4 shard_5 shard_6 shard_7
         shard_8 shard_9 shard_10 shard_11 shard_12 shard_13 shard_14
         shard_15 shard_16 shard_17]
        (pair) = prop::body::overlapping_pair() => {
            for (name, op) in [("fuse", fuse as Boolean), ("common", common as Boolean)] {
                let (mut m, a, b, _, _) = operands(&pair)?;
                let (ab, pab) = run(&mut m, &format!("{name}(a, b)"), op, a, b)?;
                let (ba, pba) = run(&mut m, &format!("{name}(b, a)"), op, b, a)?;
                assert_same_properties(&pab, &pba, name)?;
                let (da, db) = (
                    dump_text(&m, ab).map_err(fail)?,
                    dump_text(&m, ba).map_err(fail)?,
                );
                prop_assert_eq!(
                    arris_debug::dump::euler_line(&m, ab).map_err(fail)?,
                    arris_debug::dump::euler_line(&m, ba).map_err(fail)?,
                    "{}: counts",
                    name
                );
                prop_assert_eq!(
                    up_to_ids(&da),
                    up_to_ids(&db),
                    "{}: dumps\n{}\n{}",
                    name,
                    da,
                    db
                );
            }
            Ok(())
        }
}

// -- coincident faces (plan step 10, `⚠ OPEN` 4) -----------------------

prop_shards! {
    /// `V((A − B) ∪ B) = V(A ∪ B)` and `V((A − B) ∪ (A ∩ B)) = V(A)`: every
    /// face of `A − B` that came from the tool is coincident with a face of
    /// `B` with the normals opposed, every face of `A ∩ B` is coincident
    /// with one of `A − B`, and the section edges of the first cut are
    /// common blocks of the fuse — the flush case at every pose the cut
    /// succeeds at. Both fuses are clean at `Full` and hold the union's
    /// counts where they are known.
    cut_then_fuse_restores_the_union_at_random_poses
        [shard_0 shard_1 shard_2 shard_3 shard_4 shard_5 shard_6 shard_7
         shard_8 shard_9 shard_10 shard_11 shard_12 shard_13 shard_14
         shard_15 shard_16 shard_17 shard_18 shard_19 shard_20 shard_21
         shard_22 shard_23 shard_24 shard_25 shard_26 shard_27 shard_28
         shard_29]
        (pair) = prop::body::overlapping_pair() => {
            let (mut m, a, b, pa, _) = operands(&pair)?;
            let (diff, _) = cut(&mut m, a, b).map_err(|e| fail(format!("cut(a, b): {e}")))?;
            let (union, punion) = run(&mut m, "fuse(a, b)", fuse, a, b)?;
            let (inter, _) = run(&mut m, "common(a, b)", common, a, b)?;
            let (restored, prestored) = run(&mut m, "fuse(a − b, b)", fuse, diff, b)?;
            let rel = fitted_rel(&m, &punion);
            assert_same_properties_to(&prestored, &punion, "(a − b) ∪ b against a ∪ b", rel)?;
            prop_assert_eq!(
                arris_debug::dump::euler_line(&m, restored).map_err(fail)?,
                arris_debug::dump::euler_line(&m, union).map_err(fail)?,
                "(a − b) ∪ b: counts"
            );
            let (_, pwhole) = run(&mut m, "fuse(a − b, a ∩ b)", fuse, diff, inter)?;
            let rel = fitted_rel(&m, &pa);
            prop_assert!(
                close_to(pwhole.volume, pa.volume, pa.volume, rel),
                "V((a − b) ∪ (a ∩ b)) = {}, V(a) = {}",
                pwhole.volume,
                pa.volume
            );
            prop_assert!(
                close_to(pwhole.area, pa.area, pa.area, rel),
                "A((a − b) ∪ (a ∩ b)) = {}, A(a) = {}",
                pwhole.area,
                pa.area
            );
            Ok(())
        }
}

/// The first shrunk failure of the test above (seed and count in the
/// commit body): an oblique cylinder through an axis-aligned box. The
/// section ellipse of the cut is an edge of both `A − B` and `A ∩ B`,
/// and the two faces of `A − B` and `A ∩ B` on the box's face meet along
/// it as a common block; the fitted pcurves of that ellipse on the
/// cylinder differ between the two operands by more than the polygon
/// band, so the coincidence has to be decided by the curves.
#[test]
fn cut_then_fuse_of_an_oblique_cylinder_through_a_box() {
    let pair = OverlappingPair {
        cuboid: Boxed {
            min: Point3::new(-5.111718902382138, -9.737641865616778, -8.27137327424838),
            max: Point3::new(5.111718902382138, 9.737641865616778, 8.27137327424838),
            pose: Isometry::identity(),
        },
        cylinder: Cylindrical {
            axis: Axis::new(
                Point3::new(19.799757631166585, 37.59109796609268, 13.164435039577995),
                Vec3::new(
                    -0.5661663057675169,
                    -0.7883157354119573,
                    -0.2408609879483756,
                ),
            )
            .unwrap(),
            radius: 5.84425050754312,
            height: 82.56639984248808,
            pose: Isometry::identity(),
        },
    };
    let (mut m, a, b, pa, _) = operands(&pair).unwrap();
    let (diff, _) = cut(&mut m, a, b).unwrap();
    let (union, punion) = run(&mut m, "fuse(a, b)", fuse, a, b).unwrap();
    let (inter, _) = run(&mut m, "common(a, b)", common, a, b).unwrap();
    let (restored, prestored) = run(&mut m, "fuse(a − b, b)", fuse, diff, b).unwrap();
    let rel = fitted_rel(&m, &punion);
    assert_same_properties_to(&prestored, &punion, "(a − b) ∪ b against a ∪ b", rel).unwrap();
    assert_eq!(
        arris_debug::dump::euler_line(&m, restored).unwrap(),
        arris_debug::dump::euler_line(&m, union).unwrap()
    );
    let (_, pwhole) = run(&mut m, "fuse(a − b, a ∩ b)", fuse, diff, inter).unwrap();
    let rel = fitted_rel(&m, &pa);
    assert!(
        close_to(pwhole.volume, pa.volume, pa.volume, rel),
        "{} vs {}",
        pwhole.volume,
        pa.volume
    );
    assert!(
        close_to(pwhole.area, pa.area, pa.area, rel),
        "{} vs {}",
        pwhole.area,
        pa.area
    );
}

/// The second shrunk failure of the property above, the one that fixed
/// its bound (seed and count in the commit body): a cylinder across a
/// box a tenth of its length, at a pose 38 units from the origin. Every
/// count matches and the additivity identities hold to 1e-15, but
/// `V((A − B) ∪ B)` and `V(A ∪ B)` differ by 1.02e-9 relative — the
/// section curve's pcurve on the cylinder is a NURBS fitted once for
/// the union and again for the cut it is restored from, each within the
/// edge's tolerance, so the two (u, v) regions differ by that. The
/// difference scales linearly with the model's tolerance, which is what
/// [`fitted_rel`] states.
#[test]
fn cut_then_fuse_of_a_cylinder_across_a_small_box() {
    let pose = Isometry::new(
        UnitQuaternion::from_quaternion(Quaternion::new(0.0, 0.0, 1.0, 0.0)),
        Vec3::new(0.0, 0.0, -38.41144480853464),
    );
    let pair = OverlappingPair {
        cuboid: Boxed {
            min: Point3::new(-0.7606671317062929, -0.5, -0.5),
            max: Point3::new(0.7606671317062929, 0.5, 0.5),
            pose,
        },
        cylinder: Cylindrical {
            axis: Axis::new(
                Point3::new(2.4979061392527404, -1.3037770040355285, 0.959678290952288),
                Vec3::new(
                    -0.7701744682418742,
                    0.46517622691348676,
                    -0.4363970283845648,
                ),
            )
            .unwrap(),
            radius: 0.5775556717543766,
            height: 6.231381987111529,
            pose,
        },
    };
    let (mut m, a, b, _, _) = operands(&pair).unwrap();
    let (diff, _) = cut(&mut m, a, b).unwrap();
    let (union, punion) = run(&mut m, "fuse(a, b)", fuse, a, b).unwrap();
    let (restored, prestored) = run(&mut m, "fuse(a − b, b)", fuse, diff, b).unwrap();
    assert_eq!(
        arris_debug::dump::euler_line(&m, restored).unwrap(),
        arris_debug::dump::euler_line(&m, union).unwrap()
    );
    assert!(
        !close(prestored.volume, punion.volume, 1.0),
        "the case no longer needs the fitted bound: {} vs {}",
        prestored.volume,
        punion.volume
    );
    let rel = fitted_rel(&m, &punion);
    assert_same_properties_to(&prestored, &punion, "(a − b) ∪ b against a ∪ b", rel).unwrap();
}
