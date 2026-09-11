//! `ops::revolve` over planes and cylinders (`docs/plans/m5-sweeps.md`
//! step 3): a thousand rectilinear staircases beside an axis in random
//! poses — the checker at `Full` with nothing violated and nothing
//! unchecked, volume and area to Pappus's theorems, the mesh closed and
//! within its chord of the exact volume, one `Generated` per entity and
//! every part of the sketch present, the dump identical on two runs;
//! the tube's numbers equal to `boolean/coaxial-cut`'s; a profile given
//! clockwise the same body as counter-clockwise; the angle's bounds; and
//! every typed refusal with the model untouched.

use std::collections::BTreeSet;

use arris_debug::prop::profile::Sweep;
use arris_debug::prop::sweep;
use arris_debug::{corpus, dump_text, fixtures, prop, prop_shards};
use arris_mesh::tessellate;
use arris_ops::arris_check::arris_topo::arris_geom::{
    Profile, ProfileError, ProfileLoop, ProfileSegment,
};
use arris_ops::arris_check::arris_topo::arris_math::{
    Axis, Frame, Point2, Point3, Tolerance, Vec3,
};
use arris_ops::arris_check::arris_topo::provenance::SweepPart;
use arris_ops::arris_check::arris_topo::{
    Body, Model, Orientation, Origin, Provenance, Relation, Role, Shape,
};
use arris_ops::arris_check::{Level, check};
use arris_ops::measure::mass_properties;
use arris_ops::{OpError, Reason, revolve};
use core::f64::consts::TAU;
use proptest::prelude::*;

/// The relative tolerance the Pappus identities hold to.
const REL: f64 = 1e-9;

/// The chord tolerance the property meshes at. The mesh's surface lies
/// within this of the true one, so its volume is within this times the
/// area of the exact volume.
const MESH_CHORD: f64 = 1e-2;

fn fail(what: impl core::fmt::Display) -> TestCaseError {
    TestCaseError::fail(what.to_string())
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= REL * a.abs().max(b.abs()).max(1.0)
}

/// The vertices, edges, faces, shells and the body itself, each once as a
/// `Forward` [`Shape`].
fn entities_of(m: &Model, body: Body) -> BTreeSet<Shape> {
    let c = m.closure(body).unwrap();
    let mut set: BTreeSet<Shape> = BTreeSet::new();
    set.extend(
        c.vertices
            .iter()
            .map(|&v| Shape::new(v, Orientation::Forward)),
    );
    set.extend(c.edges.iter().map(|&e| Shape::new(e, Orientation::Forward)));
    set.extend(c.faces.iter().map(|&f| Shape::new(f, Orientation::Forward)));
    set.extend(
        c.shells
            .iter()
            .map(|&s| Shape::new(s, Orientation::Forward)),
    );
    set.insert(Shape::from(body));
    set
}

/// Every entity of the body generated from exactly one `Role::Revolve`,
/// every role naming one entity, nothing modified or deleted: the set of
/// parts recorded.
fn recorded_parts(
    m: &Model,
    body: Body,
    p: &Provenance,
) -> Result<BTreeSet<SweepPart>, TestCaseError> {
    let entities = entities_of(m, body);
    let mut parts = BTreeSet::new();
    for &e in &entities {
        let origins = p.origins(e);
        prop_assert_eq!(origins.len(), 1, "{}: {:?}\n{}", e, origins, p);
        let (relation, origin) = origins[0];
        prop_assert_eq!(relation, Relation::Generated, "{}", e);
        let Origin::Role(Role::Revolve(part)) = origin else {
            return Err(fail(format!(
                "{e}: generated from {origin}, not a revolve part"
            )));
        };
        prop_assert!(parts.insert(part), "{:?} names two entities", part);
        prop_assert!(p.modified_from(origin).is_empty());
        prop_assert!(!p.is_deleted(e));
    }
    prop_assert_eq!(p.deleted().count(), 0);
    prop_assert_eq!(p.outputs().len(), entities.len(), "{}", p);
    Ok(parts)
}

/// The parts a revolve of `sweep` makes, from the sketch alone: the
/// vertex indices by the segment that starts there, no `StartEdge` for a
/// segment perpendicular to the axis in a full turn, no end parts in a
/// full turn.
fn expected_parts(sweep: &Sweep, tol: Tolerance) -> BTreeSet<SweepPart> {
    let full = (sweep.angle - TAU).abs() <= tol.angular;
    let a = sweep
        .profile
        .plane
        .vec_to_local(sweep.axis.direction.into_inner());
    let along = Point2::new(a.x, a.y).coords.normalize();
    let mut parts = BTreeSet::from([SweepPart::Body, SweepPart::Shell]);
    if !full {
        parts.insert(SweepPart::StartCap);
        parts.insert(SweepPart::EndCap);
    }
    for edges in sweep.profile.edges(tol).unwrap() {
        let n = edges.len();
        for e in &edges {
            let (loop_index, segment) = (e.loop_index, e.segment);
            let vertex = if e.reversed {
                (segment + 1) % n
            } else {
                segment
            };
            let g = (e.end - e.start).normalize();
            let perpendicular = g.dot(&along).abs() <= tol.angular;
            parts.insert(SweepPart::Side {
                loop_index,
                segment,
            });
            parts.insert(SweepPart::Rise { loop_index, vertex });
            parts.insert(SweepPart::StartVertex { loop_index, vertex });
            if !(full && perpendicular) {
                parts.insert(SweepPart::StartEdge {
                    loop_index,
                    segment,
                });
            }
            if !full {
                parts.insert(SweepPart::EndEdge {
                    loop_index,
                    segment,
                });
                parts.insert(SweepPart::EndVertex { loop_index, vertex });
            }
        }
    }
    parts
}

prop_shards! {
    /// A staircase of segments parallel and perpendicular to the axis,
    /// every face a plane or a cylinder the checker decides every pair
    /// of: clean at `Full` with nothing unchecked, Pappus's volume and
    /// area, a closed mesh, complete provenance, a deterministic dump.
    rectilinear_profiles_revolve_to_pappus [shard_0 shard_1 shard_2 shard_3]
        (sweep) = prop::profile::rectilinear() => {
            let mut m = Model::default();
            let tol = m.precision().tolerance();
            let full = (sweep.angle - TAU).abs() <= tol.angular;
            if full && !sweep.profile.holes.is_empty() {
                // The hole would close into a cavity: the designed refusal,
                // with the model as it was.
                match revolve(&mut m, &sweep.profile, sweep.axis, sweep.angle) {
                    Err(OpError::Degenerate {
                        reason: Reason::MultiShell { shells },
                        ..
                    }) => prop_assert_eq!(shells, 1 + sweep.profile.holes.len()),
                    Ok(_) => return Err(fail("a full turn with a hole is two shells")),
                    Err(e) => return Err(fail(format!("revolve: {e}"))),
                }
                let (body, _) = revolve(&mut m, &sweep.profile, sweep.axis, 1.0).map_err(fail)?;
                let mut fresh = Model::default();
                let (again, _) = revolve(&mut fresh, &sweep.profile, sweep.axis, 1.0).map_err(fail)?;
                prop_assert_eq!(
                    dump_text(&m, body).map_err(fail)?,
                    dump_text(&fresh, again).map_err(fail)?,
                    "the model is as it was"
                );
                return Ok(());
            }
            let (body, p) = revolve(&mut m, &sweep.profile, sweep.axis, sweep.angle)
                .map_err(|e| fail(format!("revolve: {e}")))?;
            let report = check(&m, body, Level::Full);
            prop_assert!(
                report.is_ok() && report.unchecked().is_empty(),
                "not clean at Full\n{}\n{}",
                report,
                dump_text(&m, body).map_err(fail)?
            );
            let props = mass_properties(&m, body).map_err(fail)?;
            let pappus = sweep::revolved(&sweep.profile, &sweep.axis, sweep.angle, tol)
                .map_err(fail)?;
            prop_assert!(
                close(props.volume, pappus.volume),
                "volume {} vs Pappus {}",
                props.volume,
                pappus.volume
            );
            prop_assert!(
                close(props.area, pappus.area),
                "area {} vs Pappus {}",
                props.area,
                pappus.area
            );
            let mesh = tessellate(&m, body, MESH_CHORD).map_err(fail)?;
            let Some(volume) = mesh.signed_volume() else {
                return Err(fail("the mesh is not closed"));
            };
            prop_assert!(
                (volume - props.volume).abs() <= MESH_CHORD * props.area,
                "mesh volume {} vs {} at chord {}",
                volume,
                props.volume,
                MESH_CHORD
            );
            let parts = recorded_parts(&m, body, &p)?;
            prop_assert_eq!(parts, expected_parts(&sweep, tol));
            let dump = dump_text(&m, body).map_err(fail)?;
            let mut again = Model::default();
            let (twice, _) = revolve(&mut again, &sweep.profile, sweep.axis, sweep.angle)
                .map_err(fail)?;
            prop_assert_eq!(dump_text(&again, twice).map_err(fail)?, dump);
            Ok(())
        }
}

/// The plane `y = 0` with `u` along `x` and `v` along `z`: the plane the
/// `sweep/revolve-*` fixtures draw in.
fn xz_plane() -> Frame {
    Frame::new(Point3::origin(), -Vec3::y(), Vec3::x()).unwrap()
}

fn rectangle(u0: f64, u1: f64, v0: f64, v1: f64, clockwise: bool) -> ProfileLoop {
    let p = |u, v| Point2::new(u, v);
    let corners = if clockwise {
        [p(u0, v1), p(u1, v1), p(u1, v0), p(u0, v0)]
    } else {
        [p(u1, v0), p(u1, v1), p(u0, v1), p(u0, v0)]
    };
    ProfileLoop::Path {
        start: p(u0, v0),
        segments: corners.into_iter().map(ProfileSegment::LineTo).collect(),
    }
}

fn tube_profile(clockwise: bool) -> Profile {
    Profile {
        plane: xz_plane(),
        outer: rectangle(1.0, 2.0, -1.0, 1.0, clockwise),
        holes: Vec::new(),
    }
}

fn counts(m: &Model, body: Body) -> (usize, usize, usize) {
    let c = m.closure(body).unwrap();
    (c.vertices.len(), c.edges.len(), c.faces.len())
}

/// The full-turn tube has the numbers of the coaxial cut that builds the
/// same solid the other way: the two recipes of the corpus, measured.
#[test]
fn the_tube_has_the_coaxial_cuts_numbers() {
    let tube = corpus::chain(
        &fixtures::corpus_root().join("sweep/revolve-tube"),
        "default",
    )
    .unwrap();
    let cut = corpus::chain(
        &fixtures::corpus_root().join("boolean/coaxial-cut"),
        "default",
    )
    .unwrap();
    let (a, b) = (tube.result().unwrap(), cut.result().unwrap());
    let (pa, pb) = (
        mass_properties(&tube.model, a).unwrap(),
        mass_properties(&cut.model, b).unwrap(),
    );
    assert!(
        close(pa.volume, pb.volume),
        "{} vs {}",
        pa.volume,
        pb.volume
    );
    assert!(close(pa.area, pb.area), "{} vs {}", pa.area, pb.area);
    assert!((pa.centroid - pb.centroid).norm() <= REL);
    assert_eq!(counts(&tube.model, a), counts(&cut.model, b));
    assert_eq!(counts(&tube.model, a), (4, 6, 4));
}

/// A profile carries no orientation: the tube written clockwise is the
/// same body, id for id, as written counter-clockwise.
#[test]
fn a_clockwise_profile_gives_the_counter_clockwise_dump() {
    let axis = Axis::z_at(Point3::origin());
    let mut m = Model::default();
    let (ccw, _) = revolve(&mut m, &tube_profile(false), axis, TAU).unwrap();
    let mut n = Model::default();
    let (cw, _) = revolve(&mut n, &tube_profile(true), axis, TAU).unwrap();
    assert_eq!(dump_text(&m, ccw).unwrap(), dump_text(&n, cw).unwrap());
}

/// An angle within the angular tolerance of `2π` is the full turn, one
/// above it is refused, and so are a zero and a non-finite one.
#[test]
fn the_angle_is_held_to_a_turn() {
    let axis = Axis::z_at(Point3::origin());
    let profile = tube_profile(false);
    let eps = Model::default().precision().angular_tolerance / 2.0;
    for angle in [TAU, TAU + eps, TAU - eps] {
        let mut m = Model::default();
        let (body, _) = revolve(&mut m, &profile, axis, angle).unwrap();
        assert_eq!(counts(&m, body), (4, 6, 4), "a full turn at {angle}");
    }
    let mut m = Model::default();
    let (body, _) = revolve(&mut m, &profile, axis, TAU - 1e-3).unwrap();
    assert_eq!(
        counts(&m, body),
        (8, 12, 6),
        "just short of a turn: two flat ends"
    );
    let refused = |angle: f64| revolve(&mut Model::default(), &profile, axis, angle).unwrap_err();
    assert!(matches!(
        refused(TAU + 1e-3),
        OpError::Degenerate {
            reason: Reason::AngleAboveTurn,
            ..
        }
    ));
    assert!(matches!(
        refused(0.0),
        OpError::Degenerate {
            reason: Reason::NotPositive { what: "angle", .. },
            ..
        }
    ));
    assert!(matches!(
        refused(f64::NAN),
        OpError::Degenerate {
            reason: Reason::NonFinite { what: "angle" },
            ..
        }
    ));
}

/// Every refusal of the profile against the axis, and the model as it
/// was after each: the tube revolved afterwards has the ids of one
/// revolved in a fresh model.
#[test]
fn the_profile_is_held_clear_of_the_axis_and_the_axis_to_the_plane() {
    let z = Axis::z_at(Point3::origin());
    let sketch = |outer: ProfileLoop| Profile {
        plane: xz_plane(),
        outer,
        holes: Vec::new(),
    };
    let p = |u, v| Point2::new(u, v);
    let mut m = Model::default();
    let reason = |m: &mut Model, profile: &Profile, axis: Axis| match revolve(m, profile, axis, 1.0)
    {
        Err(OpError::Degenerate { reason, .. }) => reason,
        other => panic!("{other:?}"),
    };
    // A vertex on the axis.
    let apex = sketch(ProfileLoop::Path {
        start: p(0.0, 0.0),
        segments: vec![
            ProfileSegment::LineTo(p(1.0, -1.0)),
            ProfileSegment::LineTo(p(1.0, 1.0)),
            ProfileSegment::LineTo(p(0.0, 0.0)),
        ],
    });
    assert_eq!(reason(&mut m, &apex, z), Reason::ProfileTouchesAxis);
    // A segment along the axis.
    let flush = sketch(rectangle(0.0, 1.0, -1.0, 1.0, false));
    assert_eq!(reason(&mut m, &flush, z), Reason::ProfileTouchesAxis);
    // Straddling it.
    let across = sketch(rectangle(-1.0, 1.0, -1.0, 1.0, false));
    assert_eq!(reason(&mut m, &across, z), Reason::ProfileCrossesAxis);
    // An axis tilted out of the plane, and one lifted off it.
    let tube = tube_profile(false);
    let tilted = Axis::new(Point3::origin(), Vec3::new(0.0, 0.1, 1.0)).unwrap();
    assert_eq!(reason(&mut m, &tube, tilted), Reason::AxisNotInProfilePlane);
    let lifted = Axis::z_at(Point3::new(0.0, 1.0, 0.0));
    assert_eq!(reason(&mut m, &tube, lifted), Reason::AxisNotInProfilePlane);
    // A full turn of a profile with a hole would close the hole into a
    // cavity: a second shell, refused by name.
    let holed = Profile {
        holes: vec![rectangle(1.25, 1.75, -0.5, 0.5, false)],
        ..tube_profile(false)
    };
    assert!(matches!(
        revolve(&mut m, &holed, z, TAU),
        Err(OpError::Degenerate {
            reason: Reason::MultiShell { shells: 2 },
            ..
        })
    ));
    assert!(
        revolve(&mut m, &holed, z, 1.0).is_ok(),
        "a partial turn's hole opens onto the ends"
    );
    // An invalid sketch reaches the caller as the profile's own error.
    let bowtie = sketch(ProfileLoop::Path {
        start: p(1.0, 0.0),
        segments: vec![
            ProfileSegment::LineTo(p(11.0, 0.0)),
            ProfileSegment::LineTo(p(3.0, 8.0)),
            ProfileSegment::LineTo(p(9.0, 10.0)),
            ProfileSegment::LineTo(p(1.0, 0.0)),
        ],
    });
    assert!(matches!(
        revolve(&mut m, &bowtie, z, 1.0),
        Err(OpError::Profile(ProfileError::SelfIntersecting {
            loop_index: 0,
            ..
        }))
    ));
    // Nothing of any refusal stayed behind — nor of the partial turn
    // that succeeded, which a fresh model is given too.
    let mut fresh = Model::default();
    revolve(&mut fresh, &holed, z, 1.0).unwrap();
    let (body, _) = revolve(&mut m, &tube, z, TAU).unwrap();
    let (again, _) = revolve(&mut fresh, &tube, z, TAU).unwrap();
    assert_eq!(
        dump_text(&m, body).unwrap(),
        dump_text(&fresh, again).unwrap(),
        "the model is as it was"
    );
}
