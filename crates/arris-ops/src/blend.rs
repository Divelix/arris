//! Blends: `fillet` rolls a ball of constant radius along named edges of
//! a solid (ADR-0007). Each blend is built directly from its edge's two
//! faces in closed form — two planes blend to a cylinder on the line
//! where their offset planes meet — with its contact curves read off the
//! construction, each end trimmed by the face across the corner, and the
//! result assembled through `rebuild::rewrite` with every untouched
//! entity kept by id (`docs/ARCHITECTURE.md` §Operations,
//! `docs/DATA-MODEL.md` §Provenance).

use core::f64::consts::TAU;
use std::collections::{BTreeMap, BTreeSet};

use arris_check::arris_topo::arris_geom::region2::Side;
use arris_check::arris_topo::arris_geom::{
    Curve, Curve2, GeomKind, Surface, SurfaceIntersection, SurfaceKind, intersect_surfaces,
    pcurve_on,
};
use arris_check::arris_topo::arris_math::{Frame, Interval, Point2, Point3, Tolerance, Vec2, Vec3};
use arris_check::arris_topo::builder::{EdgeKey, EdgeSpec, VertexKey, VertexSpec};
use arris_check::arris_topo::entity::EdgeGeometry;
use arris_check::arris_topo::{
    Body, Curve2Id, Edge, EdgeId, FaceId, Model, Orientation, Provenance, Shape, VertexId,
};
use arris_check::domain::FaceDomain;

use crate::error::{Fault, OpError, Reason};
use crate::rebuild::{self, AddedFace, Rewrite, StoredUse, forward};

fn degenerate(entities: Vec<Shape>, reason: Reason) -> OpError {
    OpError::Degenerate { entities, reason }
}

fn invariant(what: &'static str) -> OpError {
    OpError::Internal(Fault::Invariant { what })
}

/// One use of an edge by a loop of one of the body's faces, addressed.
#[derive(Debug, Clone, Copy)]
struct UseAt {
    face: FaceId,
    loop_index: usize,
    coedge_index: usize,
    /// The stored orientation of the coedge.
    orientation: Orientation,
    pcurve: Curve2Id,
}

/// The body read once: each face's effective orientation and the shell
/// it belongs to, every edge's uses and every vertex's edges, all in the
/// body's own order.
struct View {
    orientation: BTreeMap<FaceId, Orientation>,
    shell_of: BTreeMap<FaceId, usize>,
    faces: Vec<FaceId>,
    uses: BTreeMap<EdgeId, Vec<UseAt>>,
    vertex_edges: BTreeMap<VertexId, BTreeSet<EdgeId>>,
}

impl View {
    fn of(m: &Model, body: Body) -> Result<View, OpError> {
        let mut view = View {
            orientation: BTreeMap::new(),
            shell_of: BTreeMap::new(),
            faces: Vec::new(),
            uses: BTreeMap::new(),
            vertex_edges: BTreeMap::new(),
        };
        for (s, shell) in m.shells(body)?.into_iter().enumerate() {
            for face_use in m.shell(shell.id)?.faces() {
                let face = face_use.oriented_by(shell.orientation);
                if view.orientation.insert(face.id, face.orientation).is_some() {
                    return Err(invariant("a face used once"));
                }
                view.shell_of.insert(face.id, s);
                view.faces.push(face.id);
                let entity = m.face(face.id)?;
                for (loop_index, l) in entity.loops().iter().enumerate() {
                    for (coedge_index, c) in l.coedges().iter().enumerate() {
                        view.uses.entry(c.edge()).or_default().push(UseAt {
                            face: face.id,
                            loop_index,
                            coedge_index,
                            orientation: c.orientation(),
                            pcurve: c.pcurve(),
                        });
                        let edge = m.edge(c.edge())?;
                        for v in [edge.start(), edge.end()] {
                            view.vertex_edges.entry(v).or_default().insert(c.edge());
                        }
                    }
                }
            }
        }
        Ok(view)
    }

    /// The face's outward normal at `uv`, the surface's composed with the
    /// body's use of the face.
    fn outward(&self, m: &Model, face: FaceId, uv: Point2) -> Result<Vec3, OpError> {
        let surface = m.surface(m.face(face)?.surface())?;
        let n = surface
            .normal(uv.x, uv.y)
            .ok_or(OpError::Internal(Fault::NoNormal { face }))?;
        Ok(n.into_inner() * self.orientation[&face].sign())
    }
}

/// A corner edge cut short by a blend's end: where on its curve, and at
/// which of its ends.
#[derive(Debug, Clone, Copy)]
struct Trim {
    edge: EdgeId,
    t: f64,
    /// `true` when the cut is at the edge's `range.lo()` end.
    cuts_lo: bool,
}

/// The arc where the blend meets the face across a corner: a circle when
/// that plane is perpendicular to the edge, an ellipse when oblique.
struct Arc {
    curve: Curve,
    range: Interval,
    /// `true` when `range.lo()` is at the contact at `u = 0`.
    lo_first: bool,
    /// Its pcurve on the face across, exact.
    on_face: Curve2,
    /// Its pcurve on the blend, placed in the blend loop's translate.
    on_blend: Curve2,
    tolerance: f64,
}

/// One end of a blend: the corner vertex it consumes, the face across
/// it, the two trim points on the contact lines and the corner edges
/// they shorten, and the arc between them.
struct End {
    vertex: VertexId,
    face: FaceId,
    /// The trim points, one per contact (`u = 0` first).
    points: [Point3; 2],
    /// The contact lines' parameters there.
    t: [f64; 2],
    /// The corner edge each contact's trim point cuts.
    trims: [Trim; 2],
    arc: Arc,
    /// The tolerance of each trim vertex.
    vertex_tolerance: [f64; 2],
}

/// A contact of the blend with one of the edge's faces: the line at
/// distance `r tan(φ/2)` from the edge, its `u` on the blend, and its
/// pcurves.
struct Contact {
    face: FaceId,
    line: Curve,
    range: Interval,
    on_face: Curve2,
    on_blend: Curve2,
    tolerance: f64,
}

/// One edge's blend, decided and checked, before anything is written.
struct Blend {
    edge: EdgeId,
    surface: Surface,
    orientation: Orientation,
    /// The contacts, at `u = 0` then at `u = β`.
    contacts: [Contact; 2],
    /// At the edge's `range.lo()` end, then its `range.hi()` end.
    ends: [End; 2],
    tolerance: f64,
}

/// `t` moved by whole periods into `range`, when it can be; a parameter
/// of a non-periodic curve as it is.
fn into_range(range: Interval, t: f64, period: Option<f64>) -> Option<f64> {
    match period {
        Some(p) => {
            let k = ((range.lo() - t) / p).ceil();
            let shifted = t + k * p;
            (shifted <= range.hi()).then_some(shifted)
        }
        None => range.contains(t).then_some(t),
    }
}

/// `pcurve` translated by whole turns in `u` so that its point at `t`
/// has `u` nearest `target`: the blend loop is written in one translate
/// of the cylinder's domain, and `pcurve_on` reports `u` in `[0, 2π)`.
fn placed(pcurve: Curve2, t: f64, target: f64) -> Curve2 {
    let u = pcurve.point(t).x;
    let k = ((target - u) / TAU).round();
    if k == 0.0 {
        pcurve
    } else {
        pcurve.translated(Vec2::new(k * TAU, 0.0))
    }
}

/// `true` when `pcurve` over `range` lies strictly inside `face` at
/// `samples` interior parameters, by the face's own domain at its own
/// tolerance: the test a contact line and an end arc pass before a blend
/// is built (ADR-0007, the `BlendTooLarge` bound).
fn inside_face(
    m: &Model,
    face: FaceId,
    pcurve: &Curve2,
    range: Interval,
    samples: usize,
) -> Result<bool, OpError> {
    let tolerance = m.face(face)?.tolerance();
    let domain = FaceDomain::of(m, face, tolerance)?;
    let n = samples.max(1);
    for i in 1..=n {
        let t = range.lerp(i as f64 / (n + 1) as f64);
        if domain.side(pcurve.point(t)).0 != Side::Inside {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The range of the end curve between `t_a` and `t_b` that holds
/// `t_mid`, the two candidates being the two ways round a closed curve:
/// the interval and whether it starts at `t_a`.
fn arc_between(t_a: f64, t_b: f64, t_mid: f64) -> Result<(Interval, bool), OpError> {
    let up = |t: f64, from: f64| if t >= from { t } else { t + TAU };
    let (b, mid) = (up(t_b, t_a), up(t_mid, t_a));
    let (lo, hi, a_first) = if mid <= b {
        (t_a, b, true)
    } else {
        (t_b, up(t_a, t_b), false)
    };
    Interval::new(lo, hi)
        .map(|r| (r, a_first))
        .map_err(|_| invariant("an end arc of positive length"))
}

/// The blend of one edge under the plane–plane arm, every contact and
/// end decided and checked against the body, nothing written.
fn plan(
    m: &Model,
    view: &View,
    edge: EdgeId,
    radius: f64,
    tol: Tolerance,
    samples: usize,
) -> Result<Blend, OpError> {
    let e = forward(edge);
    let entity = *m.edge(edge)?;
    let Some((curve_id, range)) = entity.curve() else {
        return Err(degenerate(vec![e], Reason::VertexBlend));
    };
    if entity.start() == entity.end() {
        return Err(degenerate(vec![e], Reason::VertexBlend));
    }
    let uses = view
        .uses
        .get(&edge)
        .filter(|u| u.len() == 2)
        .ok_or(invariant("two uses of the blended edge"))?;
    let (ua, ub) = (uses[0], uses[1]);
    let faces = [ua.face, ub.face];
    let pcurves = [m.curve2(ua.pcurve)?, m.curve2(ub.pcurve)?];
    let mut normals = [Vec3::zeros(); 2];
    for i in 0..2 {
        normals[i] = view.outward(m, faces[i], pcurves[i].point(range.midpoint()))?;
    }
    let [n1, n2] = normals;
    let [f1, f2] = faces;
    // A tangent dihedral has no corner to roll a ball into, whatever the
    // surfaces are.
    if n1.cross(&n2).norm() <= tol.angular {
        return Err(degenerate(
            vec![e, forward(f1), forward(f2)],
            Reason::TangentChain,
        ));
    }
    let surfaces = [
        m.surface(m.face(f1)?.surface())?,
        m.surface(m.face(f2)?.surface())?,
    ];
    let unsupported = || OpError::Unsupported {
        a: (GeomKind::Surface(surfaces[0].kind()), forward(f1)),
        b: (GeomKind::Surface(surfaces[1].kind()), forward(f2)),
    };
    // The table: plane–plane in this arm; every other pair is named.
    match (surfaces[0], surfaces[1]) {
        (Surface::Plane { .. }, Surface::Plane { .. }) => {}
        (
            Surface::Plane { .. }
            | Surface::Cylinder { .. }
            | Surface::Cone { .. }
            | Surface::Sphere { .. }
            | Surface::Torus { .. }
            | Surface::Nurbs(_),
            Surface::Plane { .. }
            | Surface::Cylinder { .. }
            | Surface::Cone { .. }
            | Surface::Sphere { .. }
            | Surface::Torus { .. }
            | Surface::Nurbs(_),
        ) => return Err(unsupported()),
    }
    let curve = m.curve(curve_id)?;
    let &Curve::Line { origin, direction } = curve else {
        return Err(OpError::Unsupported {
            a: (GeomKind::Curve(curve.kind()), e),
            b: (GeomKind::Surface(SurfaceKind::Plane), forward(f1)),
        });
    };
    let d: Vec3 = direction.into_inner();
    // Convex or concave is read from the dihedral: the direction into
    // face 1 from the edge against face 2's outward normal.
    let t1 = d * view.orientation[&f1].compose(ua.orientation).sign();
    let convex = n1.cross(&t1).dot(&n2) < 0.0;
    let s = if convex { -1.0 } else { 1.0 };
    let c = n1.dot(&n2);
    // The ball's centre against the edge, on both offset planes.
    let offset: Vec3 = (n1 + n2) * (s * radius / (1.0 + c));
    let contact_offset = [offset - n1 * (s * radius), offset - n2 * (s * radius)];
    // The axis's `X` points at one contact, so `u` runs from `0` there to
    // `β = π − φ` at the other; `Z` is the edge's direction so `v` is its
    // parameter.
    let x = [-s * n1, -s * n2];
    let turn = |from: Vec3, to: Vec3| to.dot(&d.cross(&from)).atan2(to.dot(&from));
    let (lo, beta) = if turn(x[0], x[1]) > 0.0 {
        (0, turn(x[0], x[1]))
    } else {
        (1, turn(x[1], x[0]))
    };
    let hi = 1 - lo;
    let axis_origin = origin + offset;
    let frame = Frame::new(axis_origin, d, x[lo])?;
    let surface = Surface::Cylinder { frame, radius };
    let tolerance = m
        .precision()
        .default_tolerance
        .max(m.face(f1)?.tolerance())
        .max(m.face(f2)?.tolerance());
    let by_u = [lo, hi];
    let lines = by_u.map(|i| Curve::Line {
        origin: origin + contact_offset[i],
        direction,
    });
    let contact_faces = by_u.map(|i| faces[i]);
    let contact_uses = by_u.map(|i| uses[i]);

    let mut ends: Vec<End> = Vec::with_capacity(2);
    for (at_lo, vertex) in [(true, entity.start()), (false, entity.end())] {
        let v = forward(vertex);
        let vertex_blend = || degenerate(vec![e, v], Reason::VertexBlend);
        // The corner's other two edges: the neighbour of the blended
        // edge's coedge at this end in each face's loop.
        let mut corner_edges = [edge; 2];
        for (k, u) in contact_uses.iter().enumerate() {
            let l = &m.face(u.face)?.loops()[u.loop_index];
            let n = l.coedges().len();
            let before = (u.orientation == Orientation::Forward) == at_lo;
            let j = if before {
                (u.coedge_index + n - 1) % n
            } else {
                (u.coedge_index + 1) % n
            };
            corner_edges[k] = l.coedges()[j].edge();
        }
        let at_vertex = view
            .vertex_edges
            .get(&vertex)
            .ok_or(invariant("the corner vertex's edges"))?;
        let three: BTreeSet<EdgeId> = [edge, corner_edges[0], corner_edges[1]]
            .into_iter()
            .collect();
        if three.len() != 3 || *at_vertex != three {
            return Err(vertex_blend());
        }
        let other_face = |corner: EdgeId, own: FaceId| -> Result<FaceId, OpError> {
            let uses = view
                .uses
                .get(&corner)
                .ok_or(invariant("the corner edge's uses"))?;
            let others: Vec<FaceId> = uses.iter().map(|u| u.face).filter(|&f| f != own).collect();
            match others.as_slice() {
                [f] if *f != f1 && *f != f2 => Ok(*f),
                _ => Err(vertex_blend()),
            }
        };
        let face3 = other_face(corner_edges[0], contact_faces[0])?;
        if other_face(corner_edges[1], contact_faces[1])? != face3 {
            return Err(vertex_blend());
        }
        let surface3 = m.surface(m.face(face3)?.surface())?;
        let Surface::Plane { frame: plane3 } = surface3 else {
            return Err(OpError::Unsupported {
                a: (GeomKind::Surface(SurfaceKind::Cylinder), e),
                b: (GeomKind::Surface(surface3.kind()), forward(face3)),
            });
        };
        let n3: Vec3 = plane3.z().into_inner();
        let dn = d.dot(&n3);
        if dn.abs() <= tol.angular {
            return Err(vertex_blend());
        }
        let face3_tolerance = m.face(face3)?.tolerance();
        // Where each contact line pierces the face across.
        let mut points = [Point3::origin(); 2];
        let mut t = [0.0; 2];
        for k in 0..2 {
            let Curve::Line { origin: q, .. } = lines[k] else {
                return Err(invariant("a contact line"));
            };
            t[k] = (plane3.origin() - q).dot(&n3) / dn;
            points[k] = q + t[k] * d;
        }
        // The corner edges shortened to the trim points.
        let mut trims = [Trim {
            edge,
            t: 0.0,
            cuts_lo: true,
        }; 2];
        let mut vertex_tolerance = [0.0; 2];
        for k in 0..2 {
            let corner = corner_edges[k];
            let ce = *m.edge(corner)?;
            let Some((cid, crange)) = ce.curve() else {
                return Err(vertex_blend());
            };
            let ccurve = m.curve(cid)?;
            let projection = ccurve
                .project(points[k])
                .map_err(|g| OpError::Internal(Fault::Geometry(g)))?;
            if projection.distance > ce.tolerance() {
                return Err(invariant("the corner edge through the trim point"));
            }
            let too_large = || degenerate(vec![e, forward(corner)], Reason::BlendTooLarge);
            let Some(tc) = into_range(crange, projection.t, ccurve.period()) else {
                return Err(too_large());
            };
            let cuts_lo = ce.start() == vertex;
            let far = if cuts_lo { ce.end() } else { ce.start() };
            let far_vertex = m.vertex(far)?;
            let kept_length = if cuts_lo {
                crange.hi() - tc
            } else {
                tc - crange.lo()
            };
            if kept_length <= 0.0
                || (points[k] - far_vertex.point()).norm() <= far_vertex.tolerance()
            {
                return Err(too_large());
            }
            trims[k] = Trim {
                edge: corner,
                t: tc,
                cuts_lo,
            };
            vertex_tolerance[k] = tolerance.max(ce.tolerance()).max(face3_tolerance);
        }
        // The arc: the blend cut by the plane across, between the two
        // trim points, on the blend's side of its axis.
        let section = intersect_surfaces(&surface, surface3, tol)
            .map_err(|g| OpError::Internal(Fault::Geometry(g)))?;
        let arc_curve = match section {
            SurfaceIntersection::Transversal(curves) if curves.len() == 1 => curves[0].clone(),
            SurfaceIntersection::Transversal(_)
            | SurfaceIntersection::Tangent(_)
            | SurfaceIntersection::Empty
            | SurfaceIntersection::Coincident => {
                return Err(invariant("a transversal section of the blend at its end"));
            }
        };
        let (arc_range, lo_first) = match &arc_curve {
            Curve::Circle { .. } => (
                Interval::new(0.0, beta)
                    .map_err(|_| invariant("a blend turning by a positive angle"))?,
                true,
            ),
            Curve::Ellipse { .. } => {
                // The point of the arc half way round the blend.
                let half = beta / 2.0;
                let ruling = axis_origin
                    + radius
                        * (half.cos() * frame.x().into_inner()
                            + half.sin() * frame.y().into_inner());
                let mid = ruling + ((plane3.origin() - ruling).dot(&n3) / dn) * d;
                let param = |p: Point3| -> Result<f64, OpError> {
                    arc_curve
                        .project(p)
                        .map(|q| q.t)
                        .map_err(|g| OpError::Internal(Fault::Geometry(g)))
                };
                arc_between(param(points[0])?, param(points[1])?, param(mid)?)?
            }
            Curve::Line { .. } | Curve::Nurbs(_) => {
                return Err(invariant("a conic section of the blend at its end"));
            }
        };
        let arc_tolerance = tolerance.max(face3_tolerance);
        let arc_tol = Tolerance::new(arc_tolerance, tol.angular);
        let on_face = pcurve_on(&arc_curve, arc_range, surface3, arc_tol)
            .map_err(|g| OpError::Internal(Fault::Geometry(g)))?;
        if !inside_face(m, face3, &on_face, arc_range, samples)? {
            return Err(degenerate(vec![e, forward(face3)], Reason::BlendTooLarge));
        }
        let on_blend = pcurve_on(&arc_curve, arc_range, &surface, arc_tol)
            .map_err(|g| OpError::Internal(Fault::Geometry(g)))?;
        let on_blend = placed(on_blend, arc_range.lo(), if lo_first { 0.0 } else { beta });
        ends.push(End {
            vertex,
            face: face3,
            points,
            t,
            trims,
            arc: Arc {
                curve: arc_curve,
                range: arc_range,
                lo_first,
                on_face,
                on_blend,
                tolerance: arc_tolerance,
            },
            vertex_tolerance,
        });
    }
    let Ok([start, end]): Result<[End; 2], _> = ends.try_into() else {
        return Err(invariant("two ends of the blend"));
    };
    // The contact lines between their trim points.
    let mut contacts: Vec<Contact> = Vec::with_capacity(2);
    for k in 0..2 {
        let face = contact_faces[k];
        let too_large = || degenerate(vec![e, forward(face)], Reason::BlendTooLarge);
        if end.t[k] - start.t[k] <= tolerance {
            return Err(too_large());
        }
        let range = Interval::new(start.t[k], end.t[k]).map_err(|_| too_large())?;
        let plane = m.surface(m.face(face)?.surface())?;
        let line_tol = Tolerance::new(tolerance, tol.angular);
        let on_face = pcurve_on(&lines[k], range, plane, line_tol)
            .map_err(|g| OpError::Internal(Fault::Geometry(g)))?;
        if !inside_face(m, face, &on_face, range, samples)? {
            return Err(too_large());
        }
        let u = if k == 0 { 0.0 } else { beta };
        let on_blend = pcurve_on(&lines[k], range, &surface, line_tol)
            .map_err(|g| OpError::Internal(Fault::Geometry(g)))?;
        let on_blend = placed(on_blend, range.lo(), u);
        contacts.push(Contact {
            face,
            line: lines[k].clone(),
            range,
            on_face,
            on_blend,
            tolerance,
        });
    }
    let Ok(contacts): Result<[Contact; 2], _> = contacts.try_into() else {
        return Err(invariant("two contacts of the blend"));
    };
    Ok(Blend {
        edge,
        surface,
        orientation: if convex {
            Orientation::Forward
        } else {
            Orientation::Reversed
        },
        contacts,
        ends: [start, end],
        tolerance,
    })
}

/// The tolerance a trim vertex carries: the largest of the entities that
/// meet there — the contact, the arc and the corner edge.
fn vertex_tolerance_of(start: &End, end: &End, k: usize) -> f64 {
    start.vertex_tolerance[k].max(end.vertex_tolerance[k])
}

/// The arc inserted into the face across a corner, at the junction the
/// consumed vertex stood at.
#[derive(Debug, Clone, Copy)]
struct Insertion {
    arc: EdgeKey,
    pcurve: Curve2Id,
    /// `true` when the arc's `range.lo()` is at the contact at `u = 0`.
    lo_first: bool,
    /// The corner edge the contact at `u = 0` cuts.
    edge_at_lo: EdgeId,
}

/// What a face's loops are rewritten with: a blended edge replaced by
/// the contact on this face, and an arc inserted at a consumed vertex.
#[derive(Default)]
struct FaceEdit {
    replace: BTreeMap<EdgeId, (EdgeKey, Curve2Id)>,
    insert: BTreeMap<VertexId, Insertion>,
}

/// Where a corner edge is cut at each of its ends: the new vertex and the
/// parameter.
#[derive(Default, Clone, Copy)]
struct Cuts {
    lo: Option<(usize, f64)>,
    hi: Option<(usize, f64)>,
}

/// The rewrite's indices of one blend's entities: its four trim vertices
/// `[end][contact]`, its two contact edges, its two end arcs and its
/// added face.
#[derive(Default, Clone, Copy)]
struct Made {
    vertices: [[usize; 2]; 2],
    contacts: [usize; 2],
    arcs: [usize; 2],
    added: usize,
}

/// Builds the blends of `edges`, in that order, into a rewrite of `body`
/// and returns the result with its provenance.
fn build(
    m: &mut Model,
    body: Body,
    edges: &[EdgeId],
    radius: f64,
) -> Result<(Body, Provenance), OpError> {
    let precision = m.precision();
    let tol = precision.tolerance();
    let view = View::of(m, body)?;
    // Two blended edges meeting at a vertex are a corner the closed forms
    // do not cover yet.
    let mut at_vertex: BTreeMap<VertexId, EdgeId> = BTreeMap::new();
    for &e in edges {
        let entity = *m.edge(e)?;
        for v in [entity.start(), entity.end()] {
            if at_vertex.insert(v, e).is_some() {
                return Err(degenerate(
                    vec![forward(e), forward(v)],
                    Reason::VertexBlend,
                ));
            }
        }
    }
    let mut blends: Vec<Blend> = Vec::with_capacity(edges.len());
    for &e in edges {
        blends.push(plan(m, &view, e, radius, tol, precision.check_samples)?);
    }

    let mut rw = Rewrite::default();
    let mut cuts: BTreeMap<EdgeId, Cuts> = BTreeMap::new();
    let mut edits: BTreeMap<FaceId, FaceEdit> = BTreeMap::new();
    let mut made: Vec<Made> = Vec::with_capacity(blends.len());
    for blend in &blends {
        let mut vertices = [[0usize; 2]; 2];
        for (end_index, end) in blend.ends.iter().enumerate() {
            for (k, slot) in vertices[end_index].iter_mut().enumerate() {
                *slot = rw.vertices.len();
                rw.vertices.push(VertexSpec::New {
                    point: end.points[k],
                    tolerance: vertex_tolerance_of(&blend.ends[0], &blend.ends[1], k),
                });
            }
        }
        let mut contact_edges = [0usize; 2];
        for (k, contact) in blend.contacts.iter().enumerate() {
            contact_edges[k] = rw.edges.len();
            rw.edges.push((
                EdgeSpec::New {
                    geometry: EdgeGeometry::Curve {
                        curve: m.add_curve(contact.line.clone()),
                        range: contact.range,
                    },
                    start: VertexKey::New(vertices[0][k]),
                    end: VertexKey::New(vertices[1][k]),
                    tolerance: contact.tolerance,
                },
                None,
            ));
        }
        let mut arcs = [0usize; 2];
        for (end_index, end) in blend.ends.iter().enumerate() {
            let (first, second) = if end.arc.lo_first { (0, 1) } else { (1, 0) };
            arcs[end_index] = rw.edges.len();
            rw.edges.push((
                EdgeSpec::New {
                    geometry: EdgeGeometry::Curve {
                        curve: m.add_curve(end.arc.curve.clone()),
                        range: end.arc.range,
                    },
                    start: VertexKey::New(vertices[end_index][first]),
                    end: VertexKey::New(vertices[end_index][second]),
                    tolerance: end.arc.tolerance,
                },
                None,
            ));
            for (k, trim) in end.trims.iter().enumerate() {
                let cut = cuts.entry(trim.edge).or_default();
                let slot = if trim.cuts_lo {
                    &mut cut.lo
                } else {
                    &mut cut.hi
                };
                if slot.replace((vertices[end_index][k], trim.t)).is_some() {
                    return Err(invariant("one cut per end of a corner edge"));
                }
            }
            let pcurve = m.add_curve2(end.arc.on_face.clone());
            edits.entry(end.face).or_default().insert.insert(
                end.vertex,
                Insertion {
                    arc: EdgeKey::New(arcs[end_index]),
                    pcurve,
                    lo_first: end.arc.lo_first,
                    edge_at_lo: end.trims[0].edge,
                },
            );
        }
        for (k, contact) in blend.contacts.iter().enumerate() {
            let pcurve = m.add_curve2(contact.on_face.clone());
            edits
                .entry(contact.face)
                .or_default()
                .replace
                .insert(blend.edge, (EdgeKey::New(contact_edges[k]), pcurve));
        }
        made.push(Made {
            vertices,
            contacts: contact_edges,
            arcs,
            added: 0,
        });
    }
    // The corner edges shortened, in id order.
    let mut shortened: BTreeMap<EdgeId, EdgeKey> = BTreeMap::new();
    for (&edge, cut) in &cuts {
        let entity = *m.edge(edge)?;
        let Some((curve, range)) = entity.curve() else {
            return Err(invariant("a corner edge's curve"));
        };
        let lo = cut.lo.map_or(range.lo(), |(_, t)| t);
        let hi = cut.hi.map_or(range.hi(), |(_, t)| t);
        let range = Interval::new(lo, hi)
            .map_err(|_| degenerate(vec![forward(edge)], Reason::BlendTooLarge))?;
        shortened.insert(edge, EdgeKey::New(rw.edges.len()));
        rw.edges.push((
            EdgeSpec::New {
                geometry: EdgeGeometry::Curve { curve, range },
                start: cut
                    .lo
                    .map_or(VertexKey::Kept(entity.start()), |(v, _)| VertexKey::New(v)),
                end: cut
                    .hi
                    .map_or(VertexKey::Kept(entity.end()), |(v, _)| VertexKey::New(v)),
                tolerance: entity.tolerance(),
            },
            Some(edge),
        ));
    }
    // Every touched face's loops rewritten, in the body's face order.
    for &face in &view.faces {
        let entity = m.face(face)?;
        let edit = edits.get(&face);
        let touched = edit.is_some()
            || entity
                .loops()
                .iter()
                .flat_map(|l| l.coedges())
                .any(|c| shortened.contains_key(&c.edge()));
        if !touched {
            continue;
        }
        let mut loops: Vec<Vec<StoredUse>> = Vec::with_capacity(entity.loops().len());
        for l in entity.loops() {
            let mut uses: Vec<StoredUse> = Vec::with_capacity(l.coedges().len() + 1);
            for c in l.coedges() {
                let id = c.edge();
                let (edge, pcurve) = match edit.and_then(|e| e.replace.get(&id)) {
                    Some(&(key, pcurve)) => (key, pcurve),
                    None => (
                        shortened.get(&id).copied().unwrap_or(EdgeKey::Kept(id)),
                        c.pcurve(),
                    ),
                };
                uses.push(StoredUse {
                    edge,
                    orientation: c.orientation(),
                    pcurve,
                });
                let ce = m.edge(id)?;
                let junction = if c.orientation() == Orientation::Forward {
                    ce.end()
                } else {
                    ce.start()
                };
                if let Some(ins) = edit.and_then(|e| e.insert.get(&junction)) {
                    // The arc runs from the end of the corner edge just
                    // walked to the start of the next.
                    let arrived_at_lo = id == ins.edge_at_lo;
                    let orientation = if ins.lo_first == arrived_at_lo {
                        Orientation::Forward
                    } else {
                        Orientation::Reversed
                    };
                    uses.push(StoredUse {
                        edge: ins.arc,
                        orientation,
                        pcurve: ins.pcurve,
                    });
                }
            }
            loops.push(uses);
        }
        rw.faces.insert(face, loops);
    }
    // The blend faces, one per edge, after the shell's own.
    for (blend, made) in blends.iter().zip(made.iter_mut()) {
        let (contact_edges, arcs) = (made.contacts, made.arcs);
        let [start, end] = &blend.ends;
        let [lo, hi] = &blend.contacts;
        let surface = m.add_surface(blend.surface.clone());
        let start_arc = StoredUse {
            edge: EdgeKey::New(arcs[0]),
            orientation: if start.arc.lo_first {
                Orientation::Forward
            } else {
                Orientation::Reversed
            },
            pcurve: m.add_curve2(start.arc.on_blend.clone()),
        };
        let end_arc = StoredUse {
            edge: EdgeKey::New(arcs[1]),
            orientation: if end.arc.lo_first {
                Orientation::Reversed
            } else {
                Orientation::Forward
            },
            pcurve: m.add_curve2(end.arc.on_blend.clone()),
        };
        let loop_uses = vec![
            start_arc,
            StoredUse {
                edge: EdgeKey::New(contact_edges[1]),
                orientation: Orientation::Forward,
                pcurve: m.add_curve2(hi.on_blend.clone()),
            },
            end_arc,
            StoredUse {
                edge: EdgeKey::New(contact_edges[0]),
                orientation: Orientation::Reversed,
                pcurve: m.add_curve2(lo.on_blend.clone()),
            },
        ];
        made.added = rw.added.len();
        rw.added.push(AddedFace {
            shell: view.shell_of[&lo.face],
            surface,
            orientation: blend.orientation,
            loops: vec![loop_uses],
            tolerance: blend.tolerance,
        });
    }

    let out = rebuild::rewrite(m, body, rw)?;
    let mut p = out.provenance;
    for (blend, made) in blends.iter().zip(&made) {
        let origin = forward(blend.edge);
        p.add_generated(origin, forward(out.added[made.added]));
        for &k in made.contacts.iter().chain(&made.arcs) {
            p.add_generated(origin, forward(out.edges[k]));
        }
        for &v in made.vertices.iter().flatten() {
            p.add_generated(origin, forward(out.vertices[v]));
        }
    }
    Ok((out.body, p))
}

/// Blends `edges` of `body` with a rolling ball of `radius`: each edge's
/// two faces are replaced by the same faces cut back to the ball's
/// contact curves, the edge by a blend face tangent to both along them,
/// and each end of the blend is trimmed by the face across the corner,
/// whose vertex goes and whose two other edges are shortened to the
/// trim's arc (ADR-0007). Two planes blend to a cylinder of `radius` on
/// the line where their offset planes meet — its frame's `X` at one
/// contact ruling and `Z` along the edge, so the contacts sit at `u = 0`
/// and `u = π − φ` for the dihedral's normals `φ` apart and `v` is the
/// edge's own parameter — with the contact lines at distance
/// `r tan(φ/2)` from the edge on each face; the arc across an end is a
/// circle when that face is perpendicular to the edge and an ellipse
/// otherwise, exact on the plane and a `Line` or a fitted `Nurbs` on the
/// cylinder. Convex or concave is read from the dihedral. Every surface
/// is exact, every untouched entity keeps its id, and the result's ids
/// are the same for any order of the same edges (the blends are built
/// in the body's iteration order of them).
///
/// Provenance, every record against the blended edge: the blend face, its
/// two contact edges, its two end arcs and the four trim vertices
/// `Generated` from it; each of the edge's faces, each face across an
/// end and each corner edge the trim shortens `Modified` into its new
/// self; the edge and the two corner vertices `Deleted`; the shell and
/// the body `Modified`. `arris_topo::provenance::audit` holds on every
/// result (`docs/DATA-MODEL.md` §Provenance).
///
/// Errors, the model untouched: [`OpError::Degenerate`] with
/// [`Reason::NonFinite`] or [`Reason::NotPositive`] on the radius,
/// [`Reason::NoEdges`] for an empty list, [`Reason::RepeatedEdge`] for
/// an edge listed twice, [`Reason::EdgeNotInBody`] for one that is not
/// the body's, [`Reason::TangentChain`] where the edge's faces meet at a
/// tangent dihedral, [`Reason::VertexBlend`] at a corner the closed
/// forms do not cover — a vertex of other than three edges, or one that
/// two blended edges meet at, until the miter lands — and
/// [`Reason::BlendTooLarge`] where a contact line or an end arc leaves
/// its face through an edge that is not the corner's own or a corner
/// edge is shorter than the trim; [`OpError::Unsupported`] naming the
/// two faces for a pair outside the table (every pair but plane–plane
/// today) and naming the face across an end that is not a plane;
/// [`OpError::NotFound`] for an edge id that does not resolve.
///
/// ```
/// use arris_ops::{fillet, primitive_box};
/// use arris_ops::arris_check::arris_topo::Model;
/// use arris_ops::arris_check::arris_topo::arris_math::Point3;
///
/// let mut m = Model::default();
/// let (cube, _) = primitive_box(&mut m, Point3::origin(), Point3::new(2.0, 2.0, 2.0)).unwrap();
/// // The vertical edge at x = 2, y = 2.
/// let edge = m.edges(cube).unwrap().into_iter().find(|e| {
///     let entity = m.edge(e.id).unwrap();
///     let (curve, range) = entity.curve().unwrap();
///     let mid = m.curve(curve).unwrap().point(range.midpoint());
///     (mid - Point3::new(2.0, 2.0, 1.0)).norm() < 1e-9
/// }).unwrap();
/// let (blended, provenance) = fillet(&mut m, cube, &[edge], 0.2).unwrap();
/// assert_eq!(m.faces(blended).unwrap().len(), 7, "six faces and the blend");
/// let generated = provenance.generated_from(edge.shape());
/// assert_eq!(generated.len(), 9, "the blend face, four edges and four vertices");
/// ```
pub fn fillet(
    m: &mut Model,
    body: Body,
    edges: &[Edge],
    radius: f64,
) -> Result<(Body, Provenance), OpError> {
    crate::verify_input(m, body)?;
    let b = body.shape();
    if !radius.is_finite() {
        return Err(degenerate(vec![b], Reason::NonFinite { what: "radius" }));
    }
    if radius <= 0.0 {
        return Err(degenerate(
            vec![b],
            Reason::NotPositive {
                what: "radius",
                value: radius,
            },
        ));
    }
    if edges.is_empty() {
        return Err(degenerate(vec![b], Reason::NoEdges));
    }
    let closure = m.closure(body)?;
    let mut selected: BTreeSet<EdgeId> = BTreeSet::new();
    for edge in edges {
        m.edge(edge.id)?;
        if !selected.insert(edge.id) {
            return Err(degenerate(vec![forward(edge.id)], Reason::RepeatedEdge));
        }
        if closure.edges.binary_search(&edge.id).is_err() {
            return Err(degenerate(vec![forward(edge.id), b], Reason::EdgeNotInBody));
        }
    }
    let ordered: Vec<EdgeId> = m
        .edges(body)?
        .into_iter()
        .map(|e| e.id)
        .filter(|id| selected.contains(id))
        .collect();
    m.transaction(|m| build(m, body, &ordered, radius))
}
