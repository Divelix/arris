//! Blends: `fillet` rolls a ball of constant radius along named edges of
//! a solid (ADR-0007). Each blend is built directly from its edge's two
//! faces in closed form — two planes blend to a cylinder on the line
//! where their offset planes meet — with its contact curves read off the
//! construction, each end trimmed by the face across the corner, and the
//! result assembled through `rebuild::rewrite` with every untouched
//! entity kept by id (`docs/ARCHITECTURE.md` §Operations,
//! `docs/DATA-MODEL.md` §Provenance).

use core::f64::consts::{PI, TAU};
use std::collections::{BTreeMap, BTreeSet};

use arris_check::arris_topo::arris_geom::region2::Side;
use arris_check::arris_topo::arris_geom::{
    Curve, Curve2, GeomKind, Surface, SurfaceIntersection, SurfaceKind, intersect_surfaces,
    pcurve_on,
};
use arris_check::arris_topo::arris_math::{
    Frame, Interval, Point2, Point3, Tolerance, UnitVec3, Vec2, Vec3, wrap_angle,
};
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

/// One edge's stripe: the blend surface with its two contact lines and
/// the faces they lie on, nothing about its ends decided yet.
struct Stripe {
    edge: EdgeId,
    start: VertexId,
    end: VertexId,
    /// The edge's direction, the blend's `Z`.
    d: Vec3,
    /// A point on the blend's axis, level with the edge's line origin.
    axis_origin: Point3,
    frame: Frame,
    surface: Surface,
    radius: f64,
    /// The angle the blend turns through, `π − φ` for normals `φ` apart.
    beta: f64,
    convex: bool,
    /// The contact lines, at `u = 0` then at `u = β`.
    lines: [Curve; 2],
    /// The face each contact lies on, in the same order.
    faces: [FaceId; 2],
    /// The edge's use by each of those faces, in the same order.
    uses: [UseAt; 2],
    tolerance: f64,
}

/// One end of a blend: trimmed by the face across the corner, or met by
/// the other blend of a miter.
enum EndKind {
    Face(Box<End>),
    /// The miter at `miters[at]`, this stripe being its `side`.
    Miter {
        at: usize,
        side: usize,
    },
}

impl EndKind {
    /// The contact lines' parameters at this end, by contact.
    fn t(&self, miters: &[Miter], k: usize) -> f64 {
        match self {
            EndKind::Face(end) => end.t[k],
            EndKind::Miter { at, side } => miters[*at].t[*side][k],
        }
    }

    /// The tolerance a face end's trim vertex carries, `None` at a miter.
    fn vertex_tolerance(&self, k: usize) -> Option<f64> {
        match self {
            EndKind::Face(end) => Some(end.vertex_tolerance[k]),
            EndKind::Miter { .. } => None,
        }
    }
}

/// Two blends meeting at a vertex whose third edge stays sharp
/// (ADR-0007): the ellipse where the two equal-radius cylinders cross,
/// in the plane bisecting their axes through the ball's one centre, from
/// the point where the two contacts on the shared face cross to the
/// point on the third edge where the other two contacts meet it.
struct Miter {
    /// The two blended edges, in the blends' order.
    edges: [EdgeId; 2],
    /// For each of the two stripes, which of its contacts lies on the
    /// shared face.
    shared: [usize; 2],
    /// The contact lines' parameters at the miter, `[side][contact]`.
    t: [[f64; 2]; 2],
    /// Where the two contacts on the shared face cross.
    q: Point3,
    /// Where the other two contacts meet the third edge.
    p3: Point3,
    curve: Curve,
    range: Interval,
    /// `true` when `range.lo()` is at `q`.
    q_first: bool,
    /// For each side, `true` when `range.lo()` is at that stripe's
    /// contact at `u = 0`.
    lo_first: [bool; 2],
    /// The ellipse's pcurve on each stripe's cylinder, placed.
    on_blend: [Curve2; 2],
    /// The third edge shortened to `p3`.
    trim: Trim,
    tolerance: f64,
    q_tolerance: f64,
    p3_tolerance: f64,
}

/// One edge's blend, decided and checked, before anything is written.
struct Blend {
    stripe: Stripe,
    /// The contacts, at `u = 0` then at `u = β`.
    contacts: [Contact; 2],
    /// At the edge's `range.lo()` end, then its `range.hi()` end.
    ends: [EndKind; 2],
}

/// The parameters of the closest points of two lines with unit
/// directions, or `None` when the lines are parallel within `tol.angular`.
fn lines_cross(o1: Point3, d1: Vec3, o2: Point3, d2: Vec3, tol: Tolerance) -> Option<(f64, f64)> {
    let c = d1.dot(&d2);
    if d1.cross(&d2).norm() <= tol.angular {
        return None;
    }
    let denom = 1.0 - c * c;
    let w = o2 - o1;
    let (w1, w2) = (w.dot(&d1), w.dot(&d2));
    Some(((w1 - c * w2) / denom, (c * w1 - w2) / denom))
}

/// The origin of a contact line.
fn line_origin(line: &Curve) -> Result<Point3, OpError> {
    match line {
        Curve::Line { origin, .. } => Ok(*origin),
        Curve::Circle { .. } | Curve::Ellipse { .. } | Curve::Nurbs(_) => {
            Err(invariant("a contact line"))
        }
    }
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

/// `true` when `pcurve` over `range` lies strictly on `side` of `face` at
/// `samples` interior parameters, by the face's own domain at its own
/// tolerance: the test a contact line and an end arc pass before a blend
/// is built (ADR-0007, the `BlendTooLarge` bound). A contact lies inside
/// its face; an end arc inside the face across when the blend removes
/// material and outside it when a concave blend adds the corner to it —
/// either way, a curve that changes side crosses an edge of the face
/// that is not the corner's own.
fn on_side_of_face(
    m: &Model,
    face: FaceId,
    pcurve: &Curve2,
    range: Interval,
    side: Side,
    samples: usize,
) -> Result<bool, OpError> {
    let tolerance = m.face(face)?.tolerance();
    let domain = FaceDomain::of(m, face, tolerance)?;
    let n = samples.max(1);
    for i in 1..=n {
        let t = range.lerp(i as f64 / (n + 1) as f64);
        if domain.side(pcurve.point(t)).0 != side {
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
/// The stripe of one edge under the plane–plane arm: the blend cylinder
/// and its two contact lines decided from the edge's two faces, its ends
/// not yet.
fn stripe(
    m: &Model,
    view: &View,
    edge: EdgeId,
    radius: f64,
    tol: Tolerance,
) -> Result<Stripe, OpError> {
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
    Ok(Stripe {
        edge,
        start: entity.start(),
        end: entity.end(),
        d,
        axis_origin,
        frame,
        surface,
        radius,
        beta,
        convex,
        lines: by_u.map(|i| Curve::Line {
            origin: origin + contact_offset[i],
            direction,
        }),
        faces: by_u.map(|i| faces[i]),
        uses: by_u.map(|i| uses[i]),
        tolerance,
    })
}

/// The end of `s` at its start (`at_lo`) or its end vertex, trimmed by
/// the face across the corner: the vertex's other two edges, the plane
/// they share, where each contact pierces it, the corner edges shortened
/// there and the arc between — every one checked against the body.
fn face_end(
    m: &Model,
    view: &View,
    s: &Stripe,
    at_lo: bool,
    tol: Tolerance,
    samples: usize,
) -> Result<End, OpError> {
    let (edge, d, radius, beta) = (s.edge, s.d, s.radius, s.beta);
    let e = forward(edge);
    let vertex = if at_lo { s.start } else { s.end };
    let v = forward(vertex);
    let vertex_blend = || degenerate(vec![e, v], Reason::VertexBlend);
    // The corner's other two edges: the neighbour of the blended edge's
    // coedge at this end in each face's loop.
    let mut corner_edges = [edge; 2];
    for (k, u) in s.uses.iter().enumerate() {
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
            [f] if !s.faces.contains(f) => Ok(*f),
            _ => Err(vertex_blend()),
        }
    };
    let face3 = other_face(corner_edges[0], s.faces[0])?;
    if other_face(corner_edges[1], s.faces[1])? != face3 {
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
        let q = line_origin(&s.lines[k])?;
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
        if kept_length <= 0.0 || (points[k] - far_vertex.point()).norm() <= far_vertex.tolerance() {
            return Err(too_large());
        }
        trims[k] = Trim {
            edge: corner,
            t: tc,
            cuts_lo,
        };
        vertex_tolerance[k] = s.tolerance.max(ce.tolerance()).max(face3_tolerance);
    }
    // The arc: the blend cut by the plane across, between the two trim
    // points, on the blend's side of its axis.
    let section = intersect_surfaces(&s.surface, surface3, tol)
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
            let ruling = s.axis_origin
                + radius
                    * (half.cos() * s.frame.x().into_inner()
                        + half.sin() * s.frame.y().into_inner());
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
    let arc_tolerance = s.tolerance.max(face3_tolerance);
    let arc_tol = Tolerance::new(arc_tolerance, tol.angular);
    let on_face = pcurve_on(&arc_curve, arc_range, surface3, arc_tol)
        .map_err(|g| OpError::Internal(Fault::Geometry(g)))?;
    let arc_side = if s.convex {
        Side::Inside
    } else {
        Side::Outside
    };
    if !on_side_of_face(m, face3, &on_face, arc_range, arc_side, samples)? {
        return Err(degenerate(vec![e, forward(face3)], Reason::BlendTooLarge));
    }
    let on_blend = pcurve_on(&arc_curve, arc_range, &s.surface, arc_tol)
        .map_err(|g| OpError::Internal(Fault::Geometry(g)))?;
    let on_blend = placed(on_blend, arc_range.lo(), if lo_first { 0.0 } else { beta });
    Ok(End {
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
    })
}

/// The contact lines of `s` between their parameters at its two ends,
/// `t[end][contact]`, each checked to lie inside its face.
fn contacts(
    m: &Model,
    s: &Stripe,
    t: [[f64; 2]; 2],
    tol: Tolerance,
    samples: usize,
) -> Result<[Contact; 2], OpError> {
    let e = forward(s.edge);
    let mut contacts: Vec<Contact> = Vec::with_capacity(2);
    let spans = [0, 1].map(|k| (t[0][k], t[1][k]));
    for (k, ((&face, line), &(lo, hi))) in s.faces.iter().zip(&s.lines).zip(&spans).enumerate() {
        let too_large = || degenerate(vec![e, forward(face)], Reason::BlendTooLarge);
        if hi - lo <= s.tolerance {
            return Err(too_large());
        }
        let range = Interval::new(lo, hi).map_err(|_| too_large())?;
        let plane = m.surface(m.face(face)?.surface())?;
        let line_tol = Tolerance::new(s.tolerance, tol.angular);
        let on_face = pcurve_on(line, range, plane, line_tol)
            .map_err(|g| OpError::Internal(Fault::Geometry(g)))?;
        if !on_side_of_face(m, face, &on_face, range, Side::Inside, samples)? {
            return Err(too_large());
        }
        let u = if k == 0 { 0.0 } else { s.beta };
        let on_blend = pcurve_on(line, range, &s.surface, line_tol)
            .map_err(|g| OpError::Internal(Fault::Geometry(g)))?;
        let on_blend = placed(on_blend, range.lo(), u);
        contacts.push(Contact {
            face,
            line: line.clone(),
            range,
            on_face,
            on_blend,
            tolerance: s.tolerance,
        });
    }
    contacts
        .try_into()
        .map_err(|_| invariant("two contacts of the blend"))
}

/// The miter of stripes `a` and `b` at `vertex`, a corner of three edges
/// whose third stays sharp (ADR-0007). The two stripes share one face
/// and have equal dihedrals, so their axes cross at the ball's one
/// centre and their contacts on the faces the third edge separates meet
/// it at one point; the miter is the ellipse of the two cylinders in
/// the plane bisecting their axes, from where the two contacts on the
/// shared face cross to that point, its pcurve on each cylinder fitted
/// by the oblique-section rule. The third edge is shortened to the
/// point. A corner whose dihedrals differ, whose blends are not both
/// convex or both concave, or whose edges do not share exactly one face
/// is `Reason::VertexBlend`.
fn miter(
    m: &Model,
    view: &View,
    a: &Stripe,
    b: &Stripe,
    vertex: VertexId,
    tol: Tolerance,
) -> Result<Miter, OpError> {
    let v = forward(vertex);
    let (ea, eb) = (forward(a.edge), forward(b.edge));
    let vertex_blend = || degenerate(vec![ea, eb, v], Reason::VertexBlend);
    // The shared face, and each stripe's contact on it.
    let mut shared: Option<(usize, usize, FaceId)> = None;
    for (ka, &fa) in a.faces.iter().enumerate() {
        for (kb, &fb) in b.faces.iter().enumerate() {
            if fa == fb && shared.replace((ka, kb, fa)).is_some() {
                return Err(vertex_blend());
            }
        }
    }
    let Some((ka, kb, _)) = shared else {
        return Err(vertex_blend());
    };
    let (face_a, face_b) = (a.faces[1 - ka], b.faces[1 - kb]);
    // The third edge: the vertex's one edge that is not blended, between
    // the two faces the blends do not share.
    let at_vertex = view
        .vertex_edges
        .get(&vertex)
        .ok_or(invariant("the corner vertex's edges"))?;
    let third: Vec<EdgeId> = at_vertex
        .iter()
        .copied()
        .filter(|&e| e != a.edge && e != b.edge)
        .collect();
    let (&[e3], 3) = (third.as_slice(), at_vertex.len()) else {
        return Err(vertex_blend());
    };
    let faces3: BTreeSet<FaceId> = view
        .uses
        .get(&e3)
        .ok_or(invariant("the corner edge's uses"))?
        .iter()
        .map(|u| u.face)
        .collect();
    if faces3 != BTreeSet::from([face_a, face_b]) {
        return Err(vertex_blend());
    }
    // Equal dihedrals, both convex or both concave: what puts the two
    // axes through one centre and the two far contacts through one point
    // of the third edge; anything else is a corner of two arcs, C6's.
    if (a.beta - b.beta).abs() > tol.angular || a.convex != b.convex {
        return Err(vertex_blend());
    }
    let tolerance = a.tolerance.max(b.tolerance);
    let radius = a.radius;
    let crossing = |o1: Point3, d1: Vec3, o2: Point3, d2: Vec3, what: &'static str| {
        let (t1, t2) = lines_cross(o1, d1, o2, d2, tol).ok_or_else(vertex_blend)?;
        let (p1, p2) = (o1 + t1 * d1, o2 + t2 * d2);
        if (p1 - p2).norm() > tolerance {
            return Err(invariant(what));
        }
        Ok((p1 + (p2 - p1) * 0.5, t1, t2))
    };
    // The ball's centre, where the two axes cross.
    let (centre, _, _) = crossing(
        a.axis_origin,
        a.d,
        b.axis_origin,
        b.d,
        "the two axes through the ball's centre",
    )?;
    // Where the two contacts on the shared face cross.
    let (q, tqa, tqb) = crossing(
        line_origin(&a.lines[ka])?,
        a.d,
        line_origin(&b.lines[kb])?,
        b.d,
        "the contacts on the shared face through one point",
    )?;
    // Where the other two contacts meet the third edge.
    let e3_entity = *m.edge(e3)?;
    let Some((c3_id, range3)) = e3_entity.curve() else {
        return Err(vertex_blend());
    };
    let c3 = m.curve(c3_id)?;
    let &Curve::Line {
        origin: o3,
        direction: d3,
    } = c3
    else {
        return Err(OpError::Unsupported {
            a: (GeomKind::Curve(c3.kind()), forward(e3)),
            b: (GeomKind::Surface(SurfaceKind::Plane), forward(face_a)),
        });
    };
    let d3: Vec3 = d3.into_inner();
    let (p3, tpa, t3) = crossing(
        line_origin(&a.lines[1 - ka])?,
        a.d,
        o3,
        d3,
        "the first blend's contact through the third edge",
    )?;
    let (p3b, tpb, _) = crossing(
        line_origin(&b.lines[1 - kb])?,
        b.d,
        o3,
        d3,
        "the second blend's contact through the third edge",
    )?;
    if (p3 - p3b).norm() > tolerance {
        return Err(invariant(
            "the two contacts through one point of the third edge",
        ));
    }
    // The third edge shortened to that point.
    let too_large = || degenerate(vec![ea, eb, forward(e3)], Reason::BlendTooLarge);
    let Some(tc) = into_range(range3, t3, c3.period()) else {
        return Err(too_large());
    };
    let cuts_lo = e3_entity.start() == vertex;
    let far = if cuts_lo {
        e3_entity.end()
    } else {
        e3_entity.start()
    };
    let far_vertex = m.vertex(far)?;
    let kept_length = if cuts_lo {
        range3.hi() - tc
    } else {
        tc - range3.lo()
    };
    if kept_length <= 0.0 || (p3 - far_vertex.point()).norm() <= far_vertex.tolerance() {
        return Err(too_large());
    }
    let trim = Trim {
        edge: e3,
        t: tc,
        cuts_lo,
    };
    // The ellipse: in the plane through the centre bisecting the two
    // axes — its normal the difference of the edges' directions toward
    // the vertex — with its minor axis `r` toward the shared face and its
    // major axis `r / |n · Z|` toward the third edge.
    let toward = |s: &Stripe| if s.end == vertex { s.d } else { -s.d };
    let Some(n) = UnitVec3::try_new(toward(a) - toward(b), tol.linear) else {
        return Err(vertex_blend());
    };
    let n: Vec3 = n.into_inner();
    let cos = n.dot(&a.d).abs();
    if cos <= tol.angular || (cos - n.dot(&b.d).abs()).abs() > tol.angular {
        return Err(invariant("the miter plane at one angle to both axes"));
    }
    let y = q - centre;
    if (y.norm() - radius).abs() > tolerance {
        return Err(invariant(
            "the ball touching the shared face where the contacts cross",
        ));
    }
    let y = y / y.norm();
    let Some(x) = UnitVec3::try_new(n.cross(&y), tol.linear) else {
        return Err(invariant("the miter's major axis"));
    };
    let mut x: Vec3 = x.into_inner();
    if x.dot(&(p3 - centre)) < 0.0 {
        x = -x;
    }
    let frame = Frame::new(centre, x.cross(&y), x)?;
    let curve = Curve::Ellipse {
        frame,
        major_radius: radius / cos,
        minor_radius: radius,
    };
    let param = |p: Point3| -> Result<f64, OpError> {
        let projection = curve
            .project(p)
            .map_err(|g| OpError::Internal(Fault::Geometry(g)))?;
        if projection.distance > tolerance {
            return Err(invariant("the miter's ends on its ellipse"));
        }
        Ok(projection.t)
    };
    let (tq, tp) = (param(q)?, param(p3)?);
    // The arc between them: the way round that lies inside both blends,
    // between their contacts in `u`.
    let inside = |t: f64| {
        [a, b].iter().all(|s| {
            let l = s.frame.to_local(curve.point(t));
            let u = wrap_angle(l.y.atan2(l.x));
            u > 0.0 && u < s.beta
        })
    };
    let up = |t: f64, from: f64| if t >= from { t } else { t + TAU };
    let direct = (tq + up(tp, tq)) / 2.0;
    let mid = if inside(direct) {
        direct
    } else if inside(direct + PI) {
        direct + PI
    } else {
        return Err(invariant("a miter arc inside both blends"));
    };
    let (range, q_first) = arc_between(tq, tp, mid)?;
    let arc_tol = Tolerance::new(tolerance, tol.angular);
    let shared = [ka, kb];
    let lo_first = shared.map(|k| q_first == (k == 0));
    let mut on_blend: Vec<Curve2> = Vec::with_capacity(2);
    for (i, s) in [a, b].into_iter().enumerate() {
        let pcurve = pcurve_on(&curve, range, &s.surface, arc_tol)
            .map_err(|g| OpError::Internal(Fault::Geometry(g)))?;
        on_blend.push(placed(
            pcurve,
            range.lo(),
            if lo_first[i] { 0.0 } else { s.beta },
        ));
    }
    let on_blend: [Curve2; 2] = on_blend
        .try_into()
        .map_err(|_| invariant("the miter's two pcurves"))?;
    let mut t = [[0.0; 2]; 2];
    t[0][ka] = tqa;
    t[0][1 - ka] = tpa;
    t[1][kb] = tqb;
    t[1][1 - kb] = tpb;
    Ok(Miter {
        edges: [a.edge, b.edge],
        shared,
        t,
        q,
        p3,
        curve,
        range,
        q_first,
        lo_first,
        on_blend,
        trim,
        tolerance,
        q_tolerance: tolerance,
        p3_tolerance: tolerance.max(e3_entity.tolerance()),
    })
}

/// The tolerance a face end's trim vertex carries: the largest of the
/// entities that meet there — the contact, the arc and the corner edge —
/// at either end of the contact.
fn vertex_tolerance_of(ends: &[EndKind; 2], k: usize) -> f64 {
    ends.iter()
        .filter_map(|end| end.vertex_tolerance(k))
        .fold(0.0, f64::max)
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
/// added face. At a miter the vertices and the arc are the miter's,
/// shared with the other blend.
#[derive(Default, Clone, Copy)]
struct Made {
    vertices: [[usize; 2]; 2],
    contacts: [usize; 2],
    arcs: [usize; 2],
    added: usize,
}

/// The rewrite's indices of one miter's entities: its two vertices, `q`
/// then `p3`, and its edge.
#[derive(Clone, Copy)]
struct MiterMade {
    vertices: [usize; 2],
    edge: usize,
}

/// Records `cut` at one end of a corner edge, once.
fn cut_once(cuts: &mut BTreeMap<EdgeId, Cuts>, trim: &Trim, vertex: usize) -> Result<(), OpError> {
    let cut = cuts.entry(trim.edge).or_default();
    let slot = if trim.cuts_lo {
        &mut cut.lo
    } else {
        &mut cut.hi
    };
    if slot.replace((vertex, trim.t)).is_some() {
        return Err(invariant("one cut per end of a corner edge"));
    }
    Ok(())
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
    let samples = precision.check_samples;
    let view = View::of(m, body)?;
    // The blended edges at each vertex: two meet in a miter, three are
    // the sphere corner the closed forms do not cover yet.
    let mut at_vertex: BTreeMap<VertexId, Vec<EdgeId>> = BTreeMap::new();
    for &e in edges {
        let entity = *m.edge(e)?;
        for v in [entity.start(), entity.end()] {
            at_vertex.entry(v).or_default().push(e);
        }
    }
    for (&v, es) in &at_vertex {
        if es.len() > 2 {
            let entities = es.iter().map(|&e| forward(e)).chain([forward(v)]).collect();
            return Err(degenerate(entities, Reason::VertexBlend));
        }
    }
    let mut stripes: Vec<Stripe> = Vec::with_capacity(edges.len());
    for &e in edges {
        stripes.push(stripe(m, &view, e, radius, tol)?);
    }
    let index_of: BTreeMap<EdgeId, usize> =
        edges.iter().enumerate().map(|(i, &e)| (e, i)).collect();
    // The miters, in vertex order.
    let mut miters: Vec<Miter> = Vec::new();
    let mut miter_at: BTreeMap<VertexId, usize> = BTreeMap::new();
    for (&v, es) in &at_vertex {
        if let [ea, eb] = es[..] {
            miter_at.insert(v, miters.len());
            miters.push(miter(
                m,
                &view,
                &stripes[index_of[&ea]],
                &stripes[index_of[&eb]],
                v,
                tol,
            )?);
        }
    }
    // Each stripe's ends, then its contacts between them.
    let mut blends: Vec<Blend> = Vec::with_capacity(stripes.len());
    for s in stripes {
        let mut ends: Vec<EndKind> = Vec::with_capacity(2);
        for (at_lo, vertex) in [(true, s.start), (false, s.end)] {
            ends.push(match miter_at.get(&vertex) {
                Some(&at) => EndKind::Miter {
                    at,
                    side: usize::from(miters[at].edges[0] != s.edge),
                },
                None => EndKind::Face(Box::new(face_end(m, &view, &s, at_lo, tol, samples)?)),
            });
        }
        let ends: [EndKind; 2] = ends
            .try_into()
            .map_err(|_| invariant("two ends of the blend"))?;
        let t = [0, 1].map(|end| [0, 1].map(|k| ends[end].t(&miters, k)));
        let contacts = contacts(m, &s, t, tol, samples)?;
        blends.push(Blend {
            stripe: s,
            contacts,
            ends,
        });
    }

    let mut rw = Rewrite::default();
    let mut cuts: BTreeMap<EdgeId, Cuts> = BTreeMap::new();
    let mut edits: BTreeMap<FaceId, FaceEdit> = BTreeMap::new();
    // The miters first: their two vertices, their edge, the third edge
    // cut.
    let mut miter_made: Vec<MiterMade> = Vec::with_capacity(miters.len());
    for mt in &miters {
        let q = rw.vertices.len();
        rw.vertices.push(VertexSpec::New {
            point: mt.q,
            tolerance: mt.q_tolerance,
        });
        let p3 = rw.vertices.len();
        rw.vertices.push(VertexSpec::New {
            point: mt.p3,
            tolerance: mt.p3_tolerance,
        });
        let (first, second) = if mt.q_first { (q, p3) } else { (p3, q) };
        let edge = rw.edges.len();
        rw.edges.push((
            EdgeSpec::New {
                geometry: EdgeGeometry::Curve {
                    curve: m.add_curve(mt.curve.clone()),
                    range: mt.range,
                },
                start: VertexKey::New(first),
                end: VertexKey::New(second),
                tolerance: mt.tolerance,
            },
            None,
        ));
        cut_once(&mut cuts, &mt.trim, p3)?;
        miter_made.push(MiterMade {
            vertices: [q, p3],
            edge,
        });
    }
    let mut made: Vec<Made> = Vec::with_capacity(blends.len());
    for blend in &blends {
        let mut vertices = [[0usize; 2]; 2];
        for (end_index, end) in blend.ends.iter().enumerate() {
            match end {
                EndKind::Face(face_end) => {
                    for (k, slot) in vertices[end_index].iter_mut().enumerate() {
                        *slot = rw.vertices.len();
                        rw.vertices.push(VertexSpec::New {
                            point: face_end.points[k],
                            tolerance: vertex_tolerance_of(&blend.ends, k),
                        });
                    }
                }
                EndKind::Miter { at, side } => {
                    let shared = miters[*at].shared[*side];
                    vertices[end_index][shared] = miter_made[*at].vertices[0];
                    vertices[end_index][1 - shared] = miter_made[*at].vertices[1];
                }
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
            let face_end = match end {
                EndKind::Face(face_end) => face_end,
                EndKind::Miter { at, .. } => {
                    arcs[end_index] = miter_made[*at].edge;
                    continue;
                }
            };
            let (first, second) = if face_end.arc.lo_first {
                (0, 1)
            } else {
                (1, 0)
            };
            arcs[end_index] = rw.edges.len();
            rw.edges.push((
                EdgeSpec::New {
                    geometry: EdgeGeometry::Curve {
                        curve: m.add_curve(face_end.arc.curve.clone()),
                        range: face_end.arc.range,
                    },
                    start: VertexKey::New(vertices[end_index][first]),
                    end: VertexKey::New(vertices[end_index][second]),
                    tolerance: face_end.arc.tolerance,
                },
                None,
            ));
            for (k, trim) in face_end.trims.iter().enumerate() {
                cut_once(&mut cuts, trim, vertices[end_index][k])?;
            }
            let pcurve = m.add_curve2(face_end.arc.on_face.clone());
            edits.entry(face_end.face).or_default().insert.insert(
                face_end.vertex,
                Insertion {
                    arc: EdgeKey::New(arcs[end_index]),
                    pcurve,
                    lo_first: face_end.arc.lo_first,
                    edge_at_lo: face_end.trims[0].edge,
                },
            );
        }
        for (k, contact) in blend.contacts.iter().enumerate() {
            let pcurve = m.add_curve2(contact.on_face.clone());
            edits
                .entry(contact.face)
                .or_default()
                .replace
                .insert(blend.stripe.edge, (EdgeKey::New(contact_edges[k]), pcurve));
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
    // The blend faces, one per edge, after the shell's own: each end's
    // arc walked from one contact to the other, the miter's with its
    // pcurve on this blend's cylinder.
    for (blend, made) in blends.iter().zip(made.iter_mut()) {
        let (contact_edges, arcs) = (made.contacts, made.arcs);
        let [lo, hi] = &blend.contacts;
        let surface = m.add_surface(blend.stripe.surface.clone());
        let mut end_uses = [None; 2];
        for (end_index, end) in blend.ends.iter().enumerate() {
            let (lo_first, pcurve) = match end {
                EndKind::Face(face_end) => (face_end.arc.lo_first, &face_end.arc.on_blend),
                EndKind::Miter { at, side } => {
                    (miters[*at].lo_first[*side], &miters[*at].on_blend[*side])
                }
            };
            // The start arc is walked from `u = 0` to `u = β`, the end
            // arc back.
            let along = lo_first == (end_index == 0);
            end_uses[end_index] = Some(StoredUse {
                edge: EdgeKey::New(arcs[end_index]),
                orientation: if along {
                    Orientation::Forward
                } else {
                    Orientation::Reversed
                },
                pcurve: m.add_curve2(pcurve.clone()),
            });
        }
        let [Some(start_arc), Some(end_arc)] = end_uses else {
            return Err(invariant("two ends of the blend"));
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
            orientation: if blend.stripe.convex {
                Orientation::Forward
            } else {
                Orientation::Reversed
            },
            loops: vec![loop_uses],
            tolerance: blend.stripe.tolerance,
        });
    }

    let out = rebuild::rewrite(m, body, rw)?;
    let mut p = out.provenance;
    // Every record against the blended edge; a miter's edge and vertices
    // are generated from both edges it joins, so each records them.
    for (blend, made) in blends.iter().zip(&made) {
        let origin = forward(blend.stripe.edge);
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
/// cylinder. Two blends meeting at a vertex whose third edge stays sharp
/// meet in a miter: the ellipse of the two cylinders in the plane
/// through the ball's centre bisecting their axes, from where the two
/// contacts on the shared face cross to where the other two meet the
/// third edge, a fitted `Nurbs` pcurve on each cylinder; the third edge
/// is shortened to that point and no face takes an arc. Convex or
/// concave is read from the dihedral. Every surface is exact, every
/// untouched entity keeps its id, and the result's ids are the same for
/// any order of the same edges (the blends are built in the body's
/// iteration order of them).
///
/// Provenance, every record against the blended edge: the blend face, its
/// two contact edges, its two end arcs and the four trim vertices
/// `Generated` from it; each of the edge's faces, each face across an
/// end and each corner edge the trim shortens `Modified` into its new
/// self; the edge and the two corner vertices `Deleted`; the shell and
/// the body `Modified`. A miter's edge and two vertices are `Generated`
/// from both edges they join. `arris_topo::provenance::audit` holds on every
/// result (`docs/DATA-MODEL.md` §Provenance).
///
/// Errors, the model untouched: [`OpError::Degenerate`] with
/// [`Reason::NonFinite`] or [`Reason::NotPositive`] on the radius,
/// [`Reason::NoEdges`] for an empty list, [`Reason::RepeatedEdge`] for
/// an edge listed twice, [`Reason::EdgeNotInBody`] for one that is not
/// the body's, [`Reason::TangentChain`] where the edge's faces meet at a
/// tangent dihedral, [`Reason::VertexBlend`] at a corner the closed
/// forms do not cover — a vertex of other than three edges, a miter
/// whose two blends have unequal dihedrals or are not both convex or
/// both concave, or three blended edges at one vertex until the sphere
/// corner lands — and
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
