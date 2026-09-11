//! The sweeps: a planar [`Profile`] becomes a solid by `revolve` (and, in
//! a later step of the plan, `extrude`). A sweep's faces are known
//! outright, so both enter the builder through `Builder::assemble`
//! (ADR-0004's entry, as `transform` uses it) and record every entity
//! `Generated` from a [`SweepPart`] naming the part of the sketch it came
//! from (`docs/ARCHITECTURE.md` §Operations, `docs/DATA-MODEL.md`
//! §Provenance).

use core::f64::consts::TAU;

use arris_check::arris_topo::arris_geom::{
    Curve, Curve2, GeomError, GeomKind, Profile, ProfileEdge, Surface, pcurve_on,
};
use arris_check::arris_topo::arris_math::nalgebra::UnitQuaternion;
use arris_check::arris_topo::arris_math::{
    Axis, Frame, Interval, Isometry, Point2, Point3, Tolerance, UnitVec3, Vec2, Vec3, wrap_angle,
};
use arris_check::arris_topo::builder::{
    Assembly, Builder, EdgeKey, EdgeSpec, FaceSpec, UseSpec, VertexKey, VertexSpec,
};
use arris_check::arris_topo::entity::{BodyKind, EdgeGeometry};
use arris_check::arris_topo::provenance::SweepPart;
use arris_check::arris_topo::{Body, EntityId, Model, Orientation, Provenance, Role, Shape};

use crate::error::{Fault, OpError, Reason};
use crate::verify;

fn degenerate(reason: Reason) -> OpError {
    OpError::Degenerate {
        entities: Vec::new(),
        reason,
    }
}

/// The revolve axis seen in the profile plane's own (u, v), once it has
/// been found to lie in that plane: a point of it, its direction, and the
/// unit in-plane direction from the axis towards the material — the side
/// of the axis the whole profile lies on.
#[derive(Debug, Clone, Copy)]
struct AxisInPlane {
    origin: Point2,
    along: Vec2,
    radial: Vec2,
}

impl AxisInPlane {
    /// The position along the axis of a (u, v) point.
    fn t(&self, p: Point2) -> f64 {
        (p - self.origin).dot(&self.along)
    }

    /// The signed distance of a (u, v) point from the axis, positive on
    /// the material side.
    fn rho(&self, p: Point2) -> f64 {
        (p - self.origin).dot(&self.radial)
    }
}

/// `true` when the angle `t` of a periodic parameter lies within `range`,
/// whose length is at most one turn.
fn turn_contains(range: Interval, t: f64) -> bool {
    wrap_angle(t - range.lo()) <= range.length()
}

/// The least and the greatest signed distance from the axis over one
/// edge: its ends for a line; its ends and, where the arc passes them,
/// the two points of its circle nearest to and farthest from the axis.
fn rho_range(edge: &ProfileEdge, axis: &AxisInPlane) -> (f64, f64) {
    let mut candidates = vec![edge.range.lo(), edge.range.hi()];
    if let Curve2::Circle { frame, .. } = &edge.pcurve {
        // `ρ(θ) = ρ(c) + r (a cos θ + b sin θ)`: extreme at `atan2(b, a)`
        // and half a turn on.
        let a = axis.radial.dot(&frame.x());
        let b = axis.radial.dot(&frame.y());
        let extreme = b.atan2(a);
        for t in [extreme, extreme + core::f64::consts::PI] {
            if turn_contains(edge.range, t) {
                candidates.push(edge.range.lo() + wrap_angle(t - edge.range.lo()));
            }
        }
    }
    candidates
        .into_iter()
        .map(|t| axis.rho(edge.pcurve.point(t)))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), r| {
            (lo.min(r), hi.max(r))
        })
}

/// The axis validated against the profile's plane: in it within the
/// tolerances ([`Reason::AxisNotInProfilePlane`] otherwise), then
/// projected exactly into it, so every surface of revolution is placed
/// by an axis the profile's curves lie in a plane through. Returns the
/// projected axis and its (u, v) view with `radial` the *left* normal of
/// its direction; which side the material is on is decided by
/// [`orient`].
fn axis_in_plane(
    profile: &Profile,
    axis: Axis,
    tol: Tolerance,
) -> Result<(Axis, AxisInPlane), OpError> {
    if !axis.origin.coords.iter().all(|c| c.is_finite()) {
        return Err(degenerate(Reason::NonFinite {
            what: "axis origin",
        }));
    }
    let plane = &profile.plane;
    let n = plane.z().into_inner();
    let d = axis.direction.into_inner();
    let dn = d.dot(&n);
    // The angle between the direction and the plane, and the origin's
    // height above it.
    let off_plane = dn.abs().atan2((1.0 - dn * dn).max(0.0).sqrt());
    let height = (axis.origin - plane.origin()).dot(&n);
    if !(off_plane <= tol.angular && height.abs() <= tol.linear) {
        return Err(degenerate(Reason::AxisNotInProfilePlane));
    }
    let origin = axis.origin - height * n;
    let direction =
        UnitVec3::try_new(d - dn * n, 0.0).ok_or(degenerate(Reason::AxisNotInProfilePlane))?;
    let projected = Axis { origin, direction };
    let o = plane.to_local(origin);
    let a = plane.vec_to_local(direction.into_inner());
    let along = Vec2::new(a.x, a.y);
    let along = along / along.norm();
    Ok((
        projected,
        AxisInPlane {
            origin: Point2::new(o.x, o.y),
            along,
            radial: Vec2::new(-along.y, along.x),
        },
    ))
}

/// The profile held to one side of the axis: every point of every edge at
/// a positive distance above `tol.linear` on the side `radial` is turned
/// to face ([`Reason::ProfileCrossesAxis`] for points on both sides,
/// [`Reason::ProfileTouchesAxis`] for one within the tolerance), and no
/// arc's circle crossing the axis ([`Reason::SpindleTorus`]).
fn orient(
    loops: &[Vec<ProfileEdge>],
    mut axis: AxisInPlane,
    tol: Tolerance,
) -> Result<AxisInPlane, OpError> {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for edge in loops.iter().flatten() {
        let (a, b) = rho_range(edge, &axis);
        lo = lo.min(a);
        hi = hi.max(b);
    }
    if lo < -tol.linear && hi > tol.linear {
        return Err(degenerate(Reason::ProfileCrossesAxis));
    }
    if hi <= tol.linear {
        axis.radial = -axis.radial;
        (lo, hi) = (-hi, -lo);
    }
    if lo <= tol.linear || !hi.is_finite() {
        return Err(degenerate(Reason::ProfileTouchesAxis));
    }
    for edge in loops.iter().flatten() {
        if let Curve2::Circle { frame, radius } = &edge.pcurve {
            let centre = axis.rho(frame.origin());
            if centre.abs() > tol.linear && centre - radius <= tol.linear {
                return Err(degenerate(Reason::SpindleTorus));
            }
        }
    }
    Ok(axis)
}

/// The surface one edge sweeps about the axis, in the one frame convention
/// of every surface of revolution here: origin on the axis, `X` the unit
/// radial from the axis into the profile's plane (so `u = 0` *is* the
/// profile plane and every seam lies in it), `Z` the axis direction —
/// except a cone whose radius shrinks along the axis, which takes
/// `Z = −axis`, since the data model's cone grows along `+Z`. A line
/// parallel to the axis sweeps a cylinder, perpendicular a plane (an
/// annulus, `annulus` set), oblique a cone with its apex on the axis; an
/// arc centred on the axis a sphere, elsewhere a torus of `R` its centre's
/// distance and `r` its radius.
fn swept_surface(
    edge: &ProfileEdge,
    axis: &AxisInPlane,
    base: &Frame,
    axis_point: impl Fn(f64) -> Point3,
    tol: Tolerance,
) -> Result<(Surface, bool), OpError> {
    match &edge.curve {
        Curve::Line { .. } => {
            let chord = edge.end - edge.start;
            let g = chord / chord.norm();
            let along = g.dot(&axis.along);
            let across = g.dot(&axis.radial);
            let (rho_s, rho_e) = (axis.rho(edge.start), axis.rho(edge.end));
            let (t_s, t_e) = (axis.t(edge.start), axis.t(edge.end));
            if across.abs() <= tol.angular {
                Ok((
                    Surface::Cylinder {
                        frame: *base,
                        radius: rho_s,
                    },
                    false,
                ))
            } else if along.abs() <= tol.angular {
                Ok((
                    Surface::Plane {
                        frame: base.with_origin(axis_point(t_s)),
                    },
                    true,
                ))
            } else {
                let (d_rho, d_t) = (rho_e - rho_s, t_e - t_s);
                let origin = axis_point(t_s);
                let frame = if d_rho * d_t > 0.0 {
                    base.with_origin(origin)
                } else {
                    Frame::from_orthonormal(
                        origin,
                        base.x().into_inner(),
                        -base.y().into_inner(),
                        -base.z().into_inner(),
                    )?
                };
                Ok((
                    Surface::Cone {
                        frame,
                        radius: rho_s,
                        half_angle: d_rho.abs().atan2(d_t.abs()),
                    },
                    false,
                ))
            }
        }
        &Curve::Circle { ref frame, radius } => {
            // The centre in the profile's (u, v): the arc's pcurve is the
            // same circle there, so its frame's origin is exact.
            let Curve2::Circle {
                frame: in_plane, ..
            } = &edge.pcurve
            else {
                return Err(profile_curve_fault(edge));
            };
            let _ = frame;
            let centre = in_plane.origin();
            let origin = axis_point(axis.t(centre));
            let major = axis.rho(centre);
            if major.abs() <= tol.linear {
                Ok((
                    Surface::Sphere {
                        frame: base.with_origin(origin),
                        radius,
                    },
                    false,
                ))
            } else {
                Ok((
                    Surface::Torus {
                        frame: base.with_origin(origin),
                        major_radius: major,
                        minor_radius: radius,
                    },
                    false,
                ))
            }
        }
        // `Profile::edges` makes lines and circles and nothing else.
        Curve::Ellipse { .. } | Curve::Nurbs(_) => Err(profile_curve_fault(edge)),
    }
}

/// A profile edge whose curve is not one `Profile::edges` makes: a
/// kernel bug, never a property of the sketch.
fn profile_curve_fault(edge: &ProfileEdge) -> OpError {
    OpError::Internal(Fault::Geometry(GeomError::Unsupported {
        a: GeomKind::Curve(edge.curve.kind()),
        b: GeomKind::Curve2(edge.pcurve.kind()),
    }))
}

/// One use of an edge in a side face's loop before its pcurve is placed:
/// the edge's slot, the direction it is walked in, its exact pcurve as
/// `pcurve_on` gave it, and the edge's range.
struct SideUse {
    edge: usize,
    orientation: Orientation,
    pcurve: Curve2,
    range: Interval,
}

/// Every pcurve of a loop translated by whole periods into the copy of
/// the surface's domain the loop is written in: the first use nearest
/// `u = 0`, each later use continuous with the end of the one before it.
/// `pcurve_on` reports a periodic parameter in `[0, 2π)`, so a curve in
/// the profile plane — at `u = 0` by the frame convention — can come back
/// at `2π` less a rounding, and a seam's second use is the first
/// translated by the period (`docs/DATA-MODEL.md` §Seams).
fn place_in_domain(surface: &Surface, uses: &mut [SideUse]) {
    let periods = surface.period();
    let mut previous_end: Option<Point2> = None;
    for u in uses.iter_mut() {
        let (t0, t1) = if u.orientation.is_reversed() {
            (u.range.hi(), u.range.lo())
        } else {
            (u.range.lo(), u.range.hi())
        };
        let start = u.pcurve.point(t0);
        let target = previous_end.unwrap_or(Point2::new(0.0, start.y));
        let mut by = Vec2::zeros();
        for (dir, period) in periods.iter().enumerate() {
            if let Some(period) = period {
                by[dir] = ((target[dir] - start[dir]) / period).round() * period;
            }
        }
        if by != Vec2::zeros() {
            u.pcurve = u.pcurve.translated(by);
        }
        previous_end = Some(u.pcurve.point(t1));
    }
}

/// Whether the swept face's surface normal is the outward one, read at
/// the segment's midpoint: outward is the segment's in-plane normal on
/// the right of the walk (the material is on the loop's left), and the
/// face is `Forward` when the surface normal there agrees with it.
fn side_orientation(
    edge: &ProfileEdge,
    normal: Vec3,
    surface: &Surface,
    pcurve: &Curve2,
) -> Result<Orientation, OpError> {
    let t = edge.range.midpoint();
    let tangent = edge.curve.eval(t).d1;
    let outward = tangent.cross(&normal);
    let uv = pcurve.point(t);
    let surface_normal = surface.normal(uv.x, uv.y).ok_or_else(|| {
        OpError::Internal(Fault::Geometry(GeomError::Degenerate {
            kind: GeomKind::Surface(surface.kind()),
            reason: "the swept surface has no normal at the segment's midpoint".into(),
        }))
    })?;
    Ok(if surface_normal.dot(&outward) > 0.0 {
        Orientation::Forward
    } else {
        Orientation::Reversed
    })
}

/// The consumer's index of the vertex an oriented edge starts at: the
/// segment's own when the loop was not turned round, the next segment's
/// when it was, since then the walk runs the segment backwards.
fn vertex_index(edge: &ProfileEdge, segments: usize) -> usize {
    if edge.reversed {
        (edge.segment + 1) % segments
    } else {
        edge.segment
    }
}

/// Revolves `profile` about `axis` by `angle` into a solid: a partial
/// turn with two flat ends, or a full turn with seams when `angle` is
/// within `angular_tolerance` of `2π`. Every segment of the profile
/// sweeps one face — a segment parallel to the axis a cylinder,
/// perpendicular a plane (an annulus, or a sector of one), oblique a cone;
/// an arc centred on the axis a sphere, elsewhere a torus — every vertex a
/// circular *rise* about the axis, and every pcurve is exact through
/// `pcurve_on`. The surfaces of revolution share one frame: origin on the
/// axis, `X` the radial into the profile's plane, so `u = 0` is the
/// profile plane and every seam lies in it, `Z` the axis (`−axis` for a
/// cone narrowing along it). The flat ends of a partial turn are the
/// profile face, its outward normal against the turn, and its copy
/// rotated by `angle`. Entities are appended in one fixed order —
/// vertices per loop in walking order (the start ring, then the end
/// ring), edges (start, end, rises), faces (start cap, end cap, sides per
/// loop per segment) — so the ids are a function of the profile alone;
/// every tolerance is `default_tolerance`. Provenance is one `Generated`
/// per entity from a [`Role::Revolve`] naming the part of the sketch it
/// came from: a segment perpendicular to the axis in a full turn sweeps
/// an annulus of two closed rises and has no `StartEdge`.
///
/// Errors, the model untouched: [`OpError::Profile`] when
/// `Profile::edges` refuses the sketch; [`OpError::Degenerate`] with
/// [`Reason::NonFinite`] for a non-finite angle or axis origin,
/// [`Reason::NotPositive`] for an angle at or below zero,
/// [`Reason::AngleAboveTurn`] above `2π`, [`Reason::AxisNotInProfilePlane`]
/// when the axis is off the plane by more than the tolerances,
/// [`Reason::ProfileCrossesAxis`] when the profile has points on both
/// sides of the axis, [`Reason::ProfileTouchesAxis`] when a vertex or a
/// segment comes within `default_tolerance` of it,
/// [`Reason::SpindleTorus`] when an arc's circle crosses it, and
/// [`Reason::MultiShell`] for a full turn of a profile with holes, whose
/// every hole closes into a cavity — a shell of its own, which the
/// one-shell `Solid` of cycle 1 does not hold (a partial turn's holes
/// open onto its flat ends and are one shell with the rest).
///
/// ```
/// use arris_ops::revolve;
/// use arris_ops::arris_check::arris_topo::Model;
/// use arris_ops::arris_check::arris_topo::arris_geom::{Profile, ProfileLoop, ProfileSegment};
/// use arris_ops::arris_check::arris_topo::arris_math::{Axis, Frame, Point2, Point3, Vec3};
/// use arris_ops::arris_check::arris_topo::provenance::{Role, SweepPart};
/// use core::f64::consts::TAU;
///
/// // A rectangle x ∈ [1, 2], z ∈ [−1, 1] in the xz plane, revolved about z: a tube.
/// let plane = Frame::new(Point3::origin(), -Vec3::y(), Vec3::x()).unwrap();
/// let p = |u, v| Point2::new(u, v);
/// let profile = Profile {
///     plane,
///     outer: ProfileLoop::Path {
///         start: p(1.0, -1.0),
///         segments: vec![
///             ProfileSegment::LineTo(p(2.0, -1.0)),
///             ProfileSegment::LineTo(p(2.0, 1.0)),
///             ProfileSegment::LineTo(p(1.0, 1.0)),
///             ProfileSegment::LineTo(p(1.0, -1.0)),
///         ],
///     },
///     holes: Vec::new(),
/// };
/// let mut m = Model::default();
/// let (body, provenance) = revolve(&mut m, &profile, Axis::z_at(Point3::origin()), TAU).unwrap();
/// assert_eq!(m.faces(body).unwrap().len(), 4, "two annuli and two walls");
/// assert_eq!(m.edges(body).unwrap().len(), 6, "four rises and two seams");
/// let wall = Role::Revolve(SweepPart::Side { loop_index: 0, segment: 1 });
/// assert_eq!(provenance.generated_from(wall).len(), 1);
/// ```
pub fn revolve(
    m: &mut Model,
    profile: &Profile,
    axis: Axis,
    angle: f64,
) -> Result<(Body, Provenance), OpError> {
    let precision = m.precision();
    let tol = precision.tolerance();
    if !angle.is_finite() {
        return Err(degenerate(Reason::NonFinite { what: "angle" }));
    }
    if angle <= 0.0 {
        return Err(degenerate(Reason::NotPositive {
            what: "angle",
            value: angle,
        }));
    }
    if angle > TAU + tol.angular {
        return Err(degenerate(Reason::AngleAboveTurn));
    }
    let full = (angle - TAU).abs() <= tol.angular;
    let angle = if full { TAU } else { angle };
    let (axis, in_plane) = axis_in_plane(profile, axis, tol)?;
    let loops = profile.edges(tol)?;
    // A full turn closes every hole into a cavity: a shell of its own,
    // which the one-shell `Solid` of cycle 1 does not hold.
    if full && loops.len() > 1 {
        return Err(degenerate(Reason::MultiShell {
            shells: loops.len(),
        }));
    }
    let in_plane = orient(&loops, in_plane, tol)?;

    let plane = &profile.plane;
    let normal = plane.z().into_inner();
    let radial = plane.vec_to_world(Vec3::new(in_plane.radial.x, in_plane.radial.y, 0.0));
    let base = Frame::new(axis.origin, axis.direction.into_inner(), radial)?;
    let axis_point = |t: f64| axis.at(t);
    // The material sweeps along `Z × X` at the start; whether that is the
    // profile's normal or its opposite decides the caps' and the sides'
    // walks below.
    let turn_along_normal = base.y().dot(&normal) > 0.0;
    let rotation = UnitQuaternion::from_axis_angle(&axis.direction, angle);
    let about_axis = Isometry::new(
        rotation,
        axis.origin.coords - Isometry::from_rotation(rotation).apply_vec(axis.origin.coords),
    );

    // Every segment's surface, before anything is written.
    let mut surfaces: Vec<Vec<(Surface, bool)>> = Vec::with_capacity(loops.len());
    for edges in &loops {
        let mut row = Vec::with_capacity(edges.len());
        for edge in edges {
            row.push(swept_surface(edge, &in_plane, &base, axis_point, tol)?);
        }
        surfaces.push(row);
    }

    let tolerance = precision.default_tolerance;
    let rise_range = if full {
        Interval::TURN
    } else {
        Interval::new(0.0, angle).map_err(|_| {
            degenerate(Reason::NotPositive {
                what: "angle",
                value: angle,
            })
        })?
    };
    let part = |p: SweepPart| Role::Revolve(p);

    m.transaction(|m| {
        let mut vertices: Vec<VertexSpec> = Vec::new();
        let mut vertex_roles: Vec<Role> = Vec::new();
        let mut edges: Vec<EdgeSpec> = Vec::new();
        let mut edge_roles: Vec<Role> = Vec::new();
        let mut faces: Vec<FaceSpec> = Vec::new();
        let mut face_roles: Vec<Role> = Vec::new();

        // The start ring, then the end ring; slot (loop, walk index).
        let mut start_vertex: Vec<Vec<usize>> = Vec::with_capacity(loops.len());
        let mut end_vertex: Vec<Vec<usize>> = Vec::with_capacity(loops.len());
        let mut points: Vec<Vec<Point3>> = Vec::with_capacity(loops.len());
        for edges_of in &loops {
            let n = edges_of.len();
            let mut ring = Vec::with_capacity(n);
            let mut ps = Vec::with_capacity(n);
            for edge in edges_of {
                let p = edge.curve.point(edge.range.lo());
                ring.push(vertices.len());
                vertices.push(VertexSpec::New {
                    point: p,
                    tolerance,
                });
                vertex_roles.push(part(SweepPart::StartVertex {
                    loop_index: edge.loop_index,
                    vertex: vertex_index(edge, n),
                }));
                ps.push(p);
            }
            start_vertex.push(ring);
            points.push(ps);
        }
        if full {
            end_vertex.clone_from(&start_vertex);
        } else {
            for (li, edges_of) in loops.iter().enumerate() {
                let n = edges_of.len();
                let mut ring = Vec::with_capacity(n);
                for (j, edge) in edges_of.iter().enumerate() {
                    ring.push(vertices.len());
                    vertices.push(VertexSpec::New {
                        point: about_axis.apply(points[li][j]),
                        tolerance,
                    });
                    vertex_roles.push(part(SweepPart::EndVertex {
                        loop_index: edge.loop_index,
                        vertex: vertex_index(edge, n),
                    }));
                }
                end_vertex.push(ring);
            }
        }

        // The start edges, the end edges, then the rises.
        let mut start_edge: Vec<Vec<Option<usize>>> = Vec::with_capacity(loops.len());
        let mut end_edge: Vec<Vec<Option<usize>>> = Vec::with_capacity(loops.len());
        let mut rise_edge: Vec<Vec<usize>> = Vec::with_capacity(loops.len());
        for (li, edges_of) in loops.iter().enumerate() {
            let n = edges_of.len();
            let mut row = Vec::with_capacity(n);
            for (j, edge) in edges_of.iter().enumerate() {
                let annulus = surfaces[li][j].1;
                if full && annulus {
                    row.push(None);
                    continue;
                }
                row.push(Some(edges.len()));
                edges.push(EdgeSpec::New {
                    geometry: EdgeGeometry::Curve {
                        curve: m.add_curve(edge.curve.clone()),
                        range: edge.range,
                    },
                    start: VertexKey::New(start_vertex[li][j]),
                    end: VertexKey::New(start_vertex[li][(j + 1) % n]),
                    tolerance,
                });
                edge_roles.push(part(SweepPart::StartEdge {
                    loop_index: edge.loop_index,
                    segment: edge.segment,
                }));
            }
            start_edge.push(row);
        }
        let mut end_curves: Vec<Vec<Option<Curve>>> = Vec::with_capacity(loops.len());
        for (li, edges_of) in loops.iter().enumerate() {
            let n = edges_of.len();
            let mut row = Vec::with_capacity(n);
            let mut curves = Vec::with_capacity(n);
            for (j, edge) in edges_of.iter().enumerate() {
                if full {
                    row.push(None);
                    curves.push(None);
                    continue;
                }
                let curve = edge.curve.transformed(&about_axis);
                row.push(Some(edges.len()));
                edges.push(EdgeSpec::New {
                    geometry: EdgeGeometry::Curve {
                        curve: m.add_curve(curve.clone()),
                        range: edge.range,
                    },
                    start: VertexKey::New(end_vertex[li][j]),
                    end: VertexKey::New(end_vertex[li][(j + 1) % n]),
                    tolerance,
                });
                edge_roles.push(part(SweepPart::EndEdge {
                    loop_index: edge.loop_index,
                    segment: edge.segment,
                }));
                curves.push(Some(curve));
            }
            end_edge.push(row);
            end_curves.push(curves);
        }
        let mut rises: Vec<Vec<Curve>> = Vec::with_capacity(loops.len());
        for (li, edges_of) in loops.iter().enumerate() {
            let n = edges_of.len();
            let mut row = Vec::with_capacity(n);
            let mut curves = Vec::with_capacity(n);
            for (j, edge) in edges_of.iter().enumerate() {
                let rise = Curve::Circle {
                    frame: base.with_origin(axis_point(in_plane.t(edge.start))),
                    radius: in_plane.rho(edge.start),
                };
                row.push(edges.len());
                edges.push(EdgeSpec::New {
                    geometry: EdgeGeometry::Curve {
                        curve: m.add_curve(rise.clone()),
                        range: rise_range,
                    },
                    start: VertexKey::New(start_vertex[li][j]),
                    end: VertexKey::New(end_vertex[li][j]),
                    tolerance,
                });
                edge_roles.push(part(SweepPart::Rise {
                    loop_index: edge.loop_index,
                    vertex: vertex_index(edge, n),
                }));
                curves.push(rise);
            }
            rise_edge.push(row);
            rises.push(curves);
        }

        // The two flat ends of a partial turn: the profile face, its
        // outward normal against the turn, and its rotated copy. A rigid
        // motion carries the parametrisation, so the rotated edges have
        // the profile's own pcurves on the rotated plane.
        if !full {
            let cap = |m: &mut Model,
                       faces: &mut Vec<FaceSpec>,
                       face_roles: &mut Vec<Role>,
                       frame: Frame,
                       orientation: Orientation,
                       slots: &[Vec<Option<usize>>],
                       role: SweepPart| {
                let surface = m.add_surface(Surface::Plane { frame });
                let mut cap_loops = Vec::with_capacity(loops.len());
                for (li, edges_of) in loops.iter().enumerate() {
                    let mut uses: Vec<UseSpec> = edges_of
                        .iter()
                        .enumerate()
                        .filter_map(|(j, edge)| {
                            Some(UseSpec {
                                edge: EdgeKey::New(slots[li][j]?),
                                orientation: Orientation::Forward,
                                pcurve: m.add_curve2(edge.pcurve.clone()),
                            })
                        })
                        .collect();
                    if orientation.is_reversed() {
                        uses.reverse();
                        for u in &mut uses {
                            u.orientation = u.orientation.flipped();
                        }
                    }
                    cap_loops.push(uses);
                }
                faces.push(FaceSpec::New {
                    surface,
                    orientation,
                    loops: cap_loops,
                    tolerance,
                });
                face_roles.push(part(role));
            };
            let (start_use, end_use) = if turn_along_normal {
                (Orientation::Reversed, Orientation::Forward)
            } else {
                (Orientation::Forward, Orientation::Reversed)
            };
            cap(
                m,
                &mut faces,
                &mut face_roles,
                *plane,
                start_use,
                &start_edge,
                SweepPart::StartCap,
            );
            cap(
                m,
                &mut faces,
                &mut face_roles,
                plane.transformed(&about_axis),
                end_use,
                &end_edge,
                SweepPart::EndCap,
            );
        }

        // The sides: start edge, rise up, end edge back, rise down — the
        // end edge the start edge's second use across the seam in a full
        // turn — walked that way when the material sweeps along the
        // profile's normal and the other way otherwise, so the material
        // is on the walk's left seen from outside; an annulus of a full
        // turn keeps only its two closed rises, one loop each.
        for (li, edges_of) in loops.iter().enumerate() {
            let n = edges_of.len();
            for (j, edge) in edges_of.iter().enumerate() {
                let (surface, annulus) = &surfaces[li][j];
                let next = (j + 1) % n;
                let on = |curve: &Curve, range: Interval| -> Result<Curve2, OpError> {
                    pcurve_on(curve, range, surface, tol)
                        .map_err(|e| OpError::Internal(Fault::Geometry(e)))
                };
                let start_pcurve = on(&edge.curve, edge.range)?;
                let orientation = side_orientation(edge, normal, surface, &start_pcurve)?;
                let mut cycle: Vec<SideUse> = Vec::with_capacity(4);
                if let Some(slot) = start_edge[li][j] {
                    cycle.push(SideUse {
                        edge: slot,
                        orientation: Orientation::Forward,
                        pcurve: start_pcurve.clone(),
                        range: edge.range,
                    });
                }
                cycle.push(SideUse {
                    edge: rise_edge[li][next],
                    orientation: Orientation::Forward,
                    pcurve: on(&rises[li][next], rise_range)?,
                    range: rise_range,
                });
                match (end_edge[li][j], &end_curves[li][j], start_edge[li][j]) {
                    (Some(slot), Some(curve), _) => cycle.push(SideUse {
                        edge: slot,
                        orientation: Orientation::Reversed,
                        pcurve: on(curve, edge.range)?,
                        range: edge.range,
                    }),
                    (None, _, Some(seam)) => cycle.push(SideUse {
                        edge: seam,
                        orientation: Orientation::Reversed,
                        pcurve: start_pcurve,
                        range: edge.range,
                    }),
                    (None, _, None) => {}
                    (Some(_), None, _) => {
                        return Err(OpError::Internal(Fault::Geometry(GeomError::Degenerate {
                            kind: GeomKind::Surface(surface.kind()),
                            reason: "an end edge with no curve".into(),
                        })));
                    }
                }
                cycle.push(SideUse {
                    edge: rise_edge[li][j],
                    orientation: Orientation::Reversed,
                    pcurve: on(&rises[li][j], rise_range)?,
                    range: rise_range,
                });
                if !turn_along_normal {
                    cycle.reverse();
                    for u in &mut cycle {
                        u.orientation = u.orientation.flipped();
                    }
                }
                let mut side_loops: Vec<Vec<SideUse>> = if full && *annulus {
                    cycle.into_iter().map(|u| vec![u]).collect()
                } else {
                    vec![cycle]
                };
                let surface_id = m.add_surface(surface.clone());
                let mut spec_loops = Vec::with_capacity(side_loops.len());
                for uses in &mut side_loops {
                    place_in_domain(surface, uses);
                    spec_loops.push(
                        uses.iter()
                            .map(|u| UseSpec {
                                edge: EdgeKey::New(u.edge),
                                orientation: u.orientation,
                                pcurve: m.add_curve2(u.pcurve.clone()),
                            })
                            .collect::<Vec<_>>(),
                    );
                }
                faces.push(FaceSpec::New {
                    surface: surface_id,
                    orientation,
                    loops: spec_loops,
                    tolerance,
                });
                face_roles.push(part(SweepPart::Side {
                    loop_index: edge.loop_index,
                    segment: edge.segment,
                }));
            }
        }

        let assembly = Assembly {
            vertices,
            edges,
            faces,
        };
        let b = Builder::assemble(m, tolerance, assembly)?;
        let built = b.finish(m, BodyKind::Solid)?;
        verify(m, built.body)?;
        // `assemble` makes one slot per spec in spec order and the built
        // maps are ordered by slot, so they run parallel to the role lists.
        let mut provenance = Provenance::new();
        let forward = |id: EntityId| Shape::new(id, Orientation::Forward);
        for (&id, &role) in built.vertices.values().zip(&vertex_roles) {
            provenance.add_generated(role, forward(id.into()));
        }
        for (&id, &role) in built.edges.values().zip(&edge_roles) {
            provenance.add_generated(role, forward(id.into()));
        }
        for (&id, &role) in built.faces.values().zip(&face_roles) {
            provenance.add_generated(role, forward(id.into()));
        }
        provenance.add_generated(part(SweepPart::Shell), forward(built.shell.into()));
        provenance.add_generated(part(SweepPart::Body), built.body);
        Ok((built.body, provenance))
    })
}
