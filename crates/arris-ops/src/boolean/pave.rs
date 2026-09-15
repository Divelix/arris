//! Building the pave model (ADR-0004): face pairs, edge-on-face hits,
//! the crossings of a pair's section curves with one another, the hits
//! and crossings merged into section vertices, paves, section curves cut
//! into blocks and the blocks kept as section edges with their pcurves; and,
//! for the coincident pairs, the edge–edge crossings, the paves every
//! section vertex puts on the pairs' edges, and each edge piece placed
//! on the other face as an image or matched to a piece of its boundary
//! as a common block.

use core::f64::consts::PI;
use std::collections::{BTreeMap, BTreeSet};

use arris_check::arris_topo::arris_geom::region2::Side;
use arris_check::arris_topo::arris_geom::{
    Curve, Curve2, CurveIntersection, CurveSurfaceIntersection, GeomError, GeomKind, Surface,
    SurfaceIntersection, SurfaceKind, curves_coincide, intersect_curve_surface, intersect_curves,
    intersect_surfaces, pcurve_on,
};
use arris_check::arris_topo::arris_math::{
    Interval, Point2, Point3, Precision, Tolerance, Vec2, period_end, wrap_angle,
};
use arris_check::arris_topo::{
    Body, EdgeId, FaceId, Model, Shape, Vertex as VertexHandle, VertexId,
};

use arris_check::domain::band;

use super::faces::{EdgeInfo, FaceInfo};
use super::{
    CommonBlock, Contact, EdgeEdgeHit, EdgeFaceHit, EdgeImage, FacePair, Interferences, Landing,
    Pave, SectionCrossing, SectionCurve, SectionEdge, SectionVertex, VertexSource,
};
use crate::error::{Fault, OpError};

/// A section vertex while hits are still being merged into it.
struct VertexBuild {
    /// The merged points, the first hit's first.
    points: Vec<Point3>,
    /// The largest tolerance of the entities merged.
    base: f64,
    /// What the section edges ending here need: a vertex is never below
    /// its edges.
    floor: f64,
    hits: Vec<usize>,
    crossings: Vec<usize>,
    section_crossings: Vec<usize>,
    existing: Vec<VertexId>,
    source: VertexSource,
}

impl VertexBuild {
    /// The representative point: the first operand vertex's, else the
    /// first merged point.
    fn point(&self, m: &Model) -> Point3 {
        self.existing
            .first()
            .and_then(|&v| m.vertex(v).ok())
            .map_or(self.points[0], |v| v.point())
    }

    /// The base tolerance plus the spread of the merged points about the
    /// representative one, never below the floor.
    fn tolerance(&self, m: &Model) -> f64 {
        let centre = self.point(m);
        let spread = self
            .points
            .iter()
            .map(|p| (p - centre).norm())
            .fold(0.0, f64::max);
        (self.base + spread).max(self.floor)
    }

    fn finish(&self, m: &Model) -> SectionVertex {
        SectionVertex {
            point: self.point(m),
            tolerance: self.tolerance(m),
            hits: self.hits.clone(),
            crossings: self.crossings.clone(),
            section_crossings: self.section_crossings.clone(),
            existing: self.existing.clone(),
            source: self.source,
        }
    }
}

/// An end of an edge piece: the edge's own vertex, or the section vertex
/// of a pave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum End {
    Operand(VertexId),
    Section(usize),
}

/// A piece of an operand edge between consecutive paves, as the result
/// will cut it: the same enumeration `result::Build::sub_edges` makes.
struct Block {
    index: usize,
    range: Interval,
    start: End,
    end: End,
}

/// The whole build, over the two operands read once.
struct Build<'m> {
    m: &'m Model,
    precision: Precision,
    a: Body,
    b: Body,
    faces: [Vec<FaceInfo<'m>>; 2],
    edges: [Vec<EdgeInfo<'m>>; 2],
    pairs: Vec<FacePair>,
    /// Per pair, the indices of its faces in `faces`.
    pair_faces: Vec<(usize, usize)>,
    hits: Vec<EdgeFaceHit>,
    /// Per hit, the tolerance of the entities that made it.
    hit_tolerance: Vec<f64>,
    crossings: Vec<EdgeEdgeHit>,
    crossing_tolerance: Vec<f64>,
    section_crossings: Vec<SectionCrossing>,
    /// Per section crossing, the tolerance of the pair that made it.
    section_crossing_tolerance: Vec<f64>,
    /// Edge pairs of the coincident face pairs whose curves are the same
    /// curve, `a`'s edge first.
    same_curve: BTreeSet<(EdgeId, EdgeId)>,
    vertices: Vec<VertexBuild>,
    paves: BTreeMap<EdgeId, Vec<Pave>>,
    curves: Vec<SectionCurve>,
    sections: Vec<SectionEdge>,
    contacts: Vec<Contact>,
    coincident: Vec<(EdgeId, FaceId)>,
    images: Vec<EdgeImage>,
    blocks: Vec<CommonBlock>,
}

pub(super) fn build(m: &Model, a: Body, b: Body) -> Result<Interferences, OpError> {
    let faces = [FaceInfo::of_body(m, a)?, FaceInfo::of_body(m, b)?];
    let edges = [EdgeInfo::of_body(m, a)?, EdgeInfo::of_body(m, b)?];
    let mut build = Build {
        m,
        precision: m.precision(),
        a,
        b,
        faces,
        edges,
        pairs: Vec::new(),
        pair_faces: Vec::new(),
        hits: Vec::new(),
        hit_tolerance: Vec::new(),
        crossings: Vec::new(),
        crossing_tolerance: Vec::new(),
        section_crossings: Vec::new(),
        section_crossing_tolerance: Vec::new(),
        same_curve: BTreeSet::new(),
        vertices: Vec::new(),
        paves: BTreeMap::new(),
        curves: Vec::new(),
        sections: Vec::new(),
        contacts: Vec::new(),
        coincident: Vec::new(),
        images: Vec::new(),
        blocks: Vec::new(),
    };
    build.face_pairs()?;
    build.hits()?;
    build.crossings()?;
    build.section_crossings()?;
    build.merge()?;
    build.pave_edges();
    build.pave_coincident_edges();
    build.sections()?;
    build.contacts()?;
    build.coincident()?;
    Ok(build.finish())
}

fn shape_of(body: Body) -> Shape {
    Shape::new(body.id, body.orientation)
}

/// The quadric guard: a face on a cone, a sphere or a torus is refused
/// before any intersector is asked — as the same [`OpError::Unsupported`]
/// naming the pair it got while the intersector had no arm for it — so
/// the coaxial arm (ADR-0008) widens no boolean silently. Booleans with
/// quadric operand faces are C3's, with their corpus
/// (`docs/ARCHITECTURE.md` §Operations).
fn quadric(s: &Surface) -> bool {
    matches!(
        s.kind(),
        SurfaceKind::Cone | SurfaceKind::Sphere | SurfaceKind::Torus
    )
}

/// The tolerance a geometric query between two entities runs at: the
/// larger of their tolerances for lengths, the model's angle.
fn tolerance_of(precision: &Precision, a: f64, b: f64) -> Tolerance {
    Tolerance::new(a.max(b), precision.angular_tolerance)
}

/// A geometry error on validated input as the operation's: a missing
/// closed form names the two entities, anything else is a kernel fault.
fn geometry(e: GeomError, a: Shape, b: Shape) -> OpError {
    match e {
        GeomError::Unsupported { a: ka, b: kb } => OpError::Unsupported {
            a: (ka, a),
            b: (kb, b),
        },
        other => OpError::Internal(Fault::Geometry(other)),
    }
}

/// `n` parameters over `range`, both ends included.
fn samples(range: Interval, n: usize) -> Vec<f64> {
    let n = n.max(2);
    (0..n)
        .map(|i| range.lerp(i as f64 / (n - 1) as f64))
        .collect()
}

impl<'m> Build<'m> {
    /// The edge of operand `side` with this id, when it has a curve.
    fn edge_info(&self, side: usize, id: EdgeId) -> Option<&EdgeInfo<'m>> {
        self.edges[side].iter().find(|e| e.id == id)
    }

    /// Every face pair whose boxes overlap, intersected.
    fn face_pairs(&mut self) -> Result<(), OpError> {
        let mut candidates: Vec<(usize, usize)> = Vec::new();
        for (ia, fa) in self.faces[0].iter().enumerate() {
            for (ib, fb) in self.faces[1].iter().enumerate() {
                if fa.bounds.intersects(&fb.bounds) {
                    candidates.push((ia, ib));
                }
            }
        }
        for ((ia, ib), intersection) in candidates
            .iter()
            .copied()
            .zip(self.intersect_pairs(&candidates)?)
        {
            self.pairs.push(FacePair {
                a: self.faces[0][ia].id,
                b: self.faces[1][ib].id,
                intersection,
            });
            self.pair_faces.push((ia, ib));
        }
        Ok(())
    }

    /// [`intersect_surfaces`] over each candidate pair, in the
    /// candidates' order in the result however it was computed: over
    /// `rayon` behind `parallel`, a plain iterator otherwise. A pair's
    /// intersection reads two surfaces and their tolerances and touches
    /// nothing else, which is what makes it the pave model's parallel
    /// step (ADR-0004, `docs/ARCHITECTURE.md` §Threading).
    fn intersect_pairs(
        &self,
        candidates: &[(usize, usize)],
    ) -> Result<Vec<SurfaceIntersection>, OpError> {
        let one = |&(ia, ib): &(usize, usize)| {
            let (fa, fb) = (&self.faces[0][ia], &self.faces[1][ib]);
            if quadric(fa.surface) || quadric(fb.surface) {
                return Err(OpError::Unsupported {
                    a: (GeomKind::Surface(fa.surface.kind()), fa.shape()),
                    b: (GeomKind::Surface(fb.surface.kind()), fb.shape()),
                });
            }
            let tol = tolerance_of(&self.precision, fa.tolerance, fb.tolerance);
            intersect_surfaces(fa.surface, fb.surface, tol)
                .map_err(|e| geometry(e, fa.shape(), fb.shape()))
        };
        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;
            candidates.par_iter().map(one).collect()
        }
        #[cfg(not(feature = "parallel"))]
        {
            candidates.iter().map(one).collect()
        }
    }

    /// Every edge of each operand against every face of the other whose
    /// box it reaches; the hits sorted by `(edge id, t)`, the coincident
    /// edge–face pairs recorded.
    fn hits(&mut self) -> Result<(), OpError> {
        let mut found: Vec<(EdgeFaceHit, f64)> = Vec::new();
        let mut coincident = Vec::new();
        for side in 0..2 {
            let other = 1 - side;
            for e in &self.edges[side] {
                for f in &self.faces[other] {
                    if !e.bounds.intersects(&f.bounds) {
                        continue;
                    }
                    self.hit_edge_face(e, f, &mut found, &mut coincident)?;
                }
            }
        }
        found.sort_by(|x, y| {
            x.0.edge
                .cmp(&y.0.edge)
                .then_with(|| x.0.t.total_cmp(&y.0.t))
                .then_with(|| x.0.face.cmp(&y.0.face))
        });
        for (hit, tolerance) in found {
            self.hits.push(hit);
            self.hit_tolerance.push(tolerance);
        }
        self.coincident = coincident;
        Ok(())
    }

    /// The hits of one edge on one face.
    fn hit_edge_face(
        &self,
        e: &EdgeInfo<'m>,
        f: &FaceInfo<'m>,
        found: &mut Vec<(EdgeFaceHit, f64)>,
        coincident: &mut Vec<(EdgeId, FaceId)>,
    ) -> Result<(), OpError> {
        if quadric(f.surface) {
            return Err(OpError::Unsupported {
                a: (GeomKind::Curve(e.curve.kind()), e.shape()),
                b: (GeomKind::Surface(f.surface.kind()), f.shape()),
            });
        }
        let tol = tolerance_of(&self.precision, e.tolerance, f.tolerance);
        let hits = match intersect_curve_surface(e.curve, f.surface, tol)
            .map_err(|err| geometry(err, e.shape(), f.shape()))?
        {
            CurveSurfaceIntersection::Coincident => {
                coincident.push((e.id, f.id));
                return Ok(());
            }
            CurveSurfaceIntersection::Points(hits) => hits,
        };
        for h in hits {
            let Some(t) = e.in_range(h.t) else {
                continue;
            };
            let (side, shift) = f.domain.side(h.uv);
            let landing = match side {
                Side::Outside => continue,
                Side::Inside => Landing::Interior,
                Side::Boundary => match f.domain.boundary_entity(self.m, h.point)? {
                    Some(shape) => Landing::Boundary(shape),
                    // Within the (u, v) band of a loop's polygon but within
                    // no edge's or vertex's own tolerance: not on the
                    // boundary, so the winding number alone decides.
                    None if f.domain.winds_around(h.uv) => Landing::Interior,
                    None => continue,
                },
            };
            found.push((
                EdgeFaceHit {
                    edge: e.id,
                    face: f.id,
                    t,
                    uv: h.uv + shift,
                    point: h.point,
                    tangent: h.tangent,
                    landing,
                    at_vertex: e.vertex_at(h.point),
                    vertex: None,
                },
                tol.linear,
            ));
        }
        Ok(())
    }

    /// The edges of the two faces of every `Coincident` pair against one
    /// another: a crossing in both ranges is recorded, sorted by
    /// `(a's edge, ta, b's edge, tb)` and made once per edge pair and
    /// point; a pair on the same curve is remembered for the common
    /// blocks.
    fn crossings(&mut self) -> Result<(), OpError> {
        let mut found: Vec<(EdgeEdgeHit, f64)> = Vec::new();
        let mut same_curve = BTreeSet::new();
        for (pi, pair) in self.pairs.iter().enumerate() {
            if pair.intersection != SurfaceIntersection::Coincident {
                continue;
            }
            let (ia, ib) = self.pair_faces[pi];
            let (fa, fb) = (&self.faces[0][ia], &self.faces[1][ib]);
            for &ea in fa.edges() {
                let Some(ea) = self.edge_info(0, ea) else {
                    continue;
                };
                for &eb in fb.edges() {
                    let Some(eb) = self.edge_info(1, eb) else {
                        continue;
                    };
                    if !ea.bounds.intersects(&eb.bounds) {
                        continue;
                    }
                    let tol = tolerance_of(&self.precision, ea.tolerance, eb.tolerance);
                    let hits = match intersect_curves(ea.curve, eb.curve, tol)
                        .map_err(|e| geometry(e, ea.shape(), eb.shape()))?
                    {
                        CurveIntersection::Coincident => {
                            same_curve.insert((ea.id, eb.id));
                            continue;
                        }
                        CurveIntersection::Points(hits) => hits,
                    };
                    for h in hits {
                        let (Some(ta), Some(tb)) = (ea.in_range(h.ta), eb.in_range(h.tb)) else {
                            continue;
                        };
                        let seen = found.iter().any(|(x, t)| {
                            x.a == ea.id
                                && x.b == eb.id
                                && (x.point - h.point).norm() <= t.max(tol.linear)
                        });
                        if seen {
                            continue;
                        }
                        found.push((
                            EdgeEdgeHit {
                                pair: pi,
                                a: ea.id,
                                ta,
                                b: eb.id,
                                tb,
                                point: h.point,
                                tangent: h.tangent,
                                vertex: None,
                            },
                            tol.linear,
                        ));
                    }
                }
            }
        }
        found.sort_by(|x, y| {
            x.0.a
                .cmp(&y.0.a)
                .then_with(|| x.0.ta.total_cmp(&y.0.ta))
                .then_with(|| x.0.b.cmp(&y.0.b))
                .then_with(|| x.0.tb.total_cmp(&y.0.tb))
        });
        for (hit, tolerance) in found {
            self.crossings.push(hit);
            self.crossing_tolerance.push(tolerance);
        }
        self.same_curve = same_curve;
        Ok(())
    }

    /// `point` on face `f`: its (u, v) in the translate the face's loops
    /// use when the point lies inside the face or on its boundary — on an
    /// edge or a vertex of it, or within a loop's band and wound around —
    /// and `None` when it lies outside, as [`Self::hit_edge_face`]
    /// decides a hit's landing.
    fn on_face(&self, f: &FaceInfo<'m>, point: Point3) -> Result<Option<Point2>, OpError> {
        let Ok(projection) = f.surface.project(point) else {
            return Ok(None);
        };
        let (side, shift) = f.domain.side(projection.uv);
        let on = match side {
            Side::Outside => false,
            Side::Inside => true,
            Side::Boundary => {
                f.domain.boundary_entity(self.m, point)?.is_some()
                    || f.domain.winds_around(projection.uv)
            }
        };
        Ok(on.then_some(projection.uv + shift))
    }

    /// The curves of every `Transversal` pair against one another: a
    /// crossing on both faces is recorded, sorted by `(pair, curves, t on
    /// the first)`. Two curves of one pair meet where the surfaces are
    /// tangent to each other — the two ellipses of equal cylinders with
    /// crossing axes — and no edge of either operand is there to make a
    /// hit, so the crossing makes the section vertex both curves need.
    fn section_crossings(&mut self) -> Result<(), OpError> {
        let mut found: Vec<(SectionCrossing, f64)> = Vec::new();
        for (pi, pair) in self.pairs.iter().enumerate() {
            let SurfaceIntersection::Transversal(curves) = &pair.intersection else {
                continue;
            };
            let (ia, ib) = self.pair_faces[pi];
            let (fa, fb) = (&self.faces[0][ia], &self.faces[1][ib]);
            let tol = tolerance_of(&self.precision, fa.tolerance, fb.tolerance);
            for (ci, ca) in curves.iter().enumerate() {
                for (cj, cb) in curves.iter().enumerate().skip(ci + 1) {
                    let hits = match intersect_curves(ca, cb, tol)
                        .map_err(|e| geometry(e, fa.shape(), fb.shape()))?
                    {
                        // Two distinct curves of one intersection are
                        // never the same curve.
                        CurveIntersection::Coincident => {
                            return Err(OpError::Internal(Fault::Invariant {
                                what: "two section curves of one pair coinciding",
                            }));
                        }
                        CurveIntersection::Points(hits) => hits,
                    };
                    for h in hits {
                        if self.on_face(fa, h.point)?.is_none()
                            || self.on_face(fb, h.point)?.is_none()
                        {
                            continue;
                        }
                        let wrap = |c: &Curve, t: f64| {
                            if c.period().is_some() {
                                wrap_angle(t)
                            } else {
                                t
                            }
                        };
                        found.push((
                            SectionCrossing {
                                pair: pi,
                                curves: [ci, cj],
                                t: [wrap(ca, h.ta), wrap(cb, h.tb)],
                                point: h.point,
                                tangent: h.tangent,
                                vertex: None,
                            },
                            tol.linear,
                        ));
                    }
                }
            }
        }
        found.sort_by(|x, y| {
            x.0.pair
                .cmp(&y.0.pair)
                .then_with(|| x.0.curves.cmp(&y.0.curves))
                .then_with(|| x.0.t[0].total_cmp(&y.0.t[0]))
        });
        for (crossing, tolerance) in found {
            self.section_crossings.push(crossing);
            self.section_crossing_tolerance.push(tolerance);
        }
        Ok(())
    }

    /// The first vertex (in creation order) that shares an operand vertex
    /// with `existing` or whose point is within the larger of its
    /// tolerance and `tolerance` of `point`.
    fn vertex_near(&self, point: Point3, tolerance: f64, existing: &[VertexId]) -> Option<usize> {
        let m = self.m;
        self.vertices.iter().position(|v| {
            v.existing.iter().any(|x| existing.contains(x))
                || (v.point(m) - point).norm() <= v.tolerance(m).max(tolerance)
        })
    }

    /// `point` merged into [`Self::vertex_near`] it; otherwise a new
    /// vertex of `source`. The index.
    fn merge_point(
        &mut self,
        point: Point3,
        tolerance: f64,
        existing: Vec<VertexId>,
        source: VertexSource,
    ) -> Result<usize, OpError> {
        let m = self.m;
        let mut base = tolerance;
        for &v in &existing {
            base = base.max(m.vertex(v)?.tolerance());
        }
        let found = self.vertex_near(point, tolerance, &existing);
        Ok(match found {
            Some(k) => {
                let v = &mut self.vertices[k];
                v.points.push(point);
                v.base = v.base.max(base);
                for x in existing {
                    if !v.existing.contains(&x) {
                        v.existing.push(x);
                    }
                }
                v.existing.sort();
                k
            }
            None => {
                self.vertices.push(VertexBuild {
                    points: vec![point],
                    base,
                    floor: 0.0,
                    hits: Vec::new(),
                    crossings: Vec::new(),
                    section_crossings: Vec::new(),
                    existing,
                    source,
                });
                self.vertices.len() - 1
            }
        })
    }

    /// Hits, then crossings, then section crossings, merged into section
    /// vertices; then every touch that lands on one of those joins it,
    /// and any other touch joins nothing.
    fn merge(&mut self) -> Result<(), OpError> {
        for i in 0..self.hits.len() {
            if self.hits[i].tangent {
                continue;
            }
            let hit_tol = self.hit_tolerance[i];
            let point = self.hits[i].point;
            let mut existing: Vec<VertexId> = Vec::new();
            if let Some(v) = self.hits[i].at_vertex {
                existing.push(v);
            }
            if let Landing::Boundary(shape) = self.hits[i].landing {
                if let Ok(v) = VertexHandle::try_from(shape) {
                    if !existing.contains(&v.id) {
                        existing.push(v.id);
                    }
                }
            }
            let k = self.merge_point(point, hit_tol, existing, VertexSource::Hits)?;
            self.vertices[k].hits.push(i);
            self.hits[i].vertex = Some(k);
        }
        for i in 0..self.crossings.len() {
            if self.crossings[i].tangent {
                continue;
            }
            let tol = self.crossing_tolerance[i];
            let point = self.crossings[i].point;
            let mut existing: Vec<VertexId> = Vec::new();
            for (side, id) in [(0, self.crossings[i].a), (1, self.crossings[i].b)] {
                if let Some(v) = self.edge_info(side, id).and_then(|e| e.vertex_at(point)) {
                    if !existing.contains(&v) {
                        existing.push(v);
                    }
                }
            }
            let k = self.merge_point(point, tol, existing, VertexSource::Hits)?;
            self.vertices[k].crossings.push(i);
            self.crossings[i].vertex = Some(k);
        }
        for i in 0..self.section_crossings.len() {
            if self.section_crossings[i].tangent {
                continue;
            }
            let tol = self.section_crossing_tolerance[i];
            let point = self.section_crossings[i].point;
            let k = self.merge_point(point, tol, Vec::new(), VertexSource::SectionCrossing)?;
            self.vertices[k].section_crossings.push(i);
            self.section_crossings[i].vertex = Some(k);
        }
        // A touch makes no vertex of its own, but one landing on a vertex
        // made above passes through it: a ruling or a rim circle through
        // the crossing of two ellipses, where the walls are tangent to
        // each other. It joins that vertex, which then paves its edge.
        for i in 0..self.hits.len() {
            if !self.hits[i].tangent {
                continue;
            }
            let hit_tol = self.hit_tolerance[i];
            let point = self.hits[i].point;
            let existing: Vec<VertexId> = self.hits[i].at_vertex.into_iter().collect();
            if self.vertex_near(point, hit_tol, &existing).is_none() {
                continue;
            }
            let k = self.merge_point(point, hit_tol, existing, VertexSource::Hits)?;
            let v = &mut self.vertices[k];
            v.hits.push(i);
            v.hits.sort_unstable();
            self.hits[i].vertex = Some(k);
        }
        let m = self.m;
        for v in &self.vertices {
            let wanted = v.tolerance(m);
            if wanted > self.precision.max_tolerance {
                let entity = v
                    .hits
                    .first()
                    .map(|&h| Shape::new(self.hits[h].edge, self.a.orientation))
                    .or_else(|| {
                        v.crossings
                            .first()
                            .map(|&x| Shape::new(self.crossings[x].a, self.a.orientation))
                    })
                    .or_else(|| {
                        v.section_crossings.first().map(|&x| {
                            Shape::new(
                                self.pairs[self.section_crossings[x].pair].a,
                                self.a.orientation,
                            )
                        })
                    })
                    .unwrap_or_else(|| shape_of(self.a));
                return Err(OpError::Tolerance { entity, wanted });
            }
        }
        Ok(())
    }

    /// The paves on the operand edges: each hit's vertex at its `t`,
    /// unless the hit is at the edge's own end; each crossing's vertex
    /// on both edges likewise; one pave per vertex per edge, ascending
    /// by `t`.
    fn pave_edges(&mut self) {
        let mut wanted: Vec<(EdgeId, f64, usize)> = Vec::new();
        for h in &self.hits {
            if let (Some(vertex), None) = (h.vertex, h.at_vertex) {
                wanted.push((h.edge, h.t, vertex));
            }
        }
        for x in &self.crossings {
            let Some(vertex) = x.vertex else {
                continue;
            };
            for (side, id, t) in [(0, x.a, x.ta), (1, x.b, x.tb)] {
                let at_end = self
                    .edge_info(side, id)
                    .is_some_and(|e| e.vertex_at(x.point).is_some());
                if !at_end {
                    wanted.push((id, t, vertex));
                }
            }
        }
        for (edge, t, vertex) in wanted {
            let list = self.paves.entry(edge).or_default();
            if list.iter().all(|p| p.vertex != vertex) {
                list.push(Pave { t, vertex });
            }
        }
        for list in self.paves.values_mut() {
            list.sort_by(|x, y| x.t.total_cmp(&y.t));
        }
    }

    /// Every edge of a face of a `Coincident` pair paved by every section
    /// vertex that lies on it within the vertex's tolerance and is not
    /// one of its ends: the vertices the other operand's edges made on
    /// neighbouring faces, which the pieces along the shared surface
    /// have to meet at (ADR-0004).
    fn pave_coincident_edges(&mut self) {
        let m = self.m;
        let mut edges: BTreeSet<(usize, EdgeId)> = BTreeSet::new();
        for (pi, pair) in self.pairs.iter().enumerate() {
            if pair.intersection != SurfaceIntersection::Coincident {
                continue;
            }
            let (ia, ib) = self.pair_faces[pi];
            edges.extend(self.faces[0][ia].edges().iter().map(|&e| (0, e)));
            edges.extend(self.faces[1][ib].edges().iter().map(|&e| (1, e)));
        }
        // An edge lying in a face of the other operand is paved the same
        // way, whether or not a face of its own is coincident with it: its
        // pieces inside that face are images there.
        for &(e, _) in &self.coincident {
            edges.insert((usize::from(self.edge_info(0, e).is_none()), e));
        }
        let mut wanted: Vec<(EdgeId, f64, usize)> = Vec::new();
        for (side, id) in edges {
            let Some(e) = self.edge_info(side, id) else {
                continue;
            };
            for (k, v) in self.vertices.iter().enumerate() {
                let point = v.point(m);
                if e.vertex_at(point).is_some() || e.ends.iter().any(|x| v.existing.contains(&x.0))
                {
                    continue;
                }
                let Ok(projection) = e.curve.project(point) else {
                    continue;
                };
                if projection.distance > v.tolerance(m) {
                    continue;
                }
                let Some(t) = e.in_range(projection.t) else {
                    continue;
                };
                if t == e.range.lo() || t == e.range.hi() {
                    continue;
                }
                wanted.push((id, t, k));
            }
        }
        for (edge, t, vertex) in wanted {
            let list = self.paves.entry(edge).or_default();
            if list.iter().all(|p| p.vertex != vertex) {
                list.push(Pave { t, vertex });
                list.sort_by(|x, y| x.t.total_cmp(&y.t));
            }
        }
    }

    /// `point` projected onto both faces' surfaces and inside both, with
    /// the (u, v) of each in the translate the face's loops use.
    fn inside_both(fa: &FaceInfo<'m>, fb: &FaceInfo<'m>, point: Point3) -> Option<[Point2; 2]> {
        let mut out = [Point2::origin(); 2];
        for (i, f) in [fa, fb].into_iter().enumerate() {
            let projection = f.surface.project(point).ok()?;
            let (side, shift) = f.domain.side(projection.uv);
            if side != Side::Inside {
                return None;
            }
            out[i] = projection.uv + shift;
        }
        Some(out)
    }

    /// The section curves of every `Transversal` pair, paved and cut into
    /// blocks, the blocks interior to both faces kept as section edges.
    fn sections(&mut self) -> Result<(), OpError> {
        for pi in 0..self.pairs.len() {
            let curves = match &self.pairs[pi].intersection {
                SurfaceIntersection::Transversal(curves) => curves.clone(),
                SurfaceIntersection::Empty
                | SurfaceIntersection::Coincident
                | SurfaceIntersection::Tangent(_) => continue,
                // Only a pair with a cone, a sphere or a torus meets in
                // points, and the quadric guard refuses those before the
                // intersector is asked.
                SurfaceIntersection::Points(_) => {
                    return Err(OpError::Internal(Fault::Invariant {
                        what: "a face pair meeting in points past the quadric guard",
                    }));
                }
            };
            for (ci, curve) in curves.iter().enumerate() {
                self.section_curve(pi, ci, curve)?;
            }
        }
        Ok(())
    }

    /// The rulings of every `Tangent` pair, paved by the touches and cut
    /// into blocks, the blocks interior to both faces kept as contacts.
    fn contacts(&mut self) -> Result<(), OpError> {
        for pi in 0..self.pairs.len() {
            let curves = match &self.pairs[pi].intersection {
                SurfaceIntersection::Tangent(curves) => curves.clone(),
                SurfaceIntersection::Empty
                | SurfaceIntersection::Coincident
                | SurfaceIntersection::Transversal(_)
                | SurfaceIntersection::Points(_) => continue,
            };
            for (ci, curve) in curves.iter().enumerate() {
                self.contact_curve(pi, ci, curve)?;
            }
        }
        Ok(())
    }

    /// One tangent ruling: its paves are the hits of either face's edges
    /// on the other face that lie on it — where the ruling leaves one
    /// face inside the other, a touch, since every curve in a face
    /// tangent to the other surface is tangent to it there — and each
    /// block between consecutive paves whose midpoint is inside both
    /// faces is a contact. A ruling is a line, so a block that is
    /// interior to both faces has a pave at each end: the ends of the
    /// overlap of the two faces' spans along it are each an end of one
    /// span inside the other.
    fn contact_curve(&mut self, pi: usize, ci: usize, curve: &Curve) -> Result<(), OpError> {
        let (ia, ib) = self.pair_faces[pi];
        let (fa, fb) = (&self.faces[0][ia], &self.faces[1][ib]);
        let mut paves: Vec<f64> = Vec::new();
        for (k, h) in self.hits.iter().enumerate() {
            let on_pair = (h.face == fb.id && fa.edges().contains(&h.edge))
                || (h.face == fa.id && fb.edges().contains(&h.edge));
            if !on_pair {
                continue;
            }
            let Ok(projection) = curve.project(h.point) else {
                continue;
            };
            if projection.distance <= self.hit_tolerance[k] {
                paves.push(projection.t);
            }
        }
        paves.sort_by(f64::total_cmp);
        for w in paves.windows(2) {
            let Ok(range) = Interval::new(w[0], w[1]) else {
                continue;
            };
            if range.length() <= 0.0 {
                continue;
            }
            let point = curve.point(range.midpoint());
            let Some(uv) = Self::inside_both(fa, fb, point) else {
                continue;
            };
            self.contacts.push(Contact {
                pair: pi,
                curve: ci,
                range,
                point,
                uv,
            });
        }
        Ok(())
    }

    /// One section curve: its paves from the vertices on it, a seed
    /// vertex for a closed curve with none, then its blocks.
    fn section_curve(&mut self, pi: usize, ci: usize, curve: &Curve) -> Result<(), OpError> {
        let m = self.m;
        let (ia, ib) = self.pair_faces[pi];
        let (fa, fb) = (&self.faces[0][ia], &self.faces[1][ib]);
        let periodic = curve.period().is_some();
        let mut paves: Vec<Pave> = Vec::new();
        for (k, v) in self.vertices.iter().enumerate() {
            let point = v.point(m);
            let Ok(projection) = curve.project(point) else {
                continue;
            };
            if projection.distance <= v.tolerance(m) {
                let t = if periodic {
                    wrap_angle(projection.t)
                } else {
                    projection.t
                };
                paves.push(Pave { t, vertex: k });
            }
        }
        paves.sort_by(|x, y| x.t.total_cmp(&y.t).then(x.vertex.cmp(&y.vertex)));
        let curve_index = self.curves.len();
        if paves.is_empty() && periodic {
            // Nothing cuts it: it is interior to both faces or clear of
            // one, and only in the first case does it become an edge, from
            // its own point at parameter zero.
            if Self::inside_both(fa, fb, curve.point(PI)).is_some() {
                self.vertices.push(VertexBuild {
                    points: vec![curve.point(0.0)],
                    base: fa.tolerance.max(fb.tolerance),
                    floor: 0.0,
                    hits: Vec::new(),
                    crossings: Vec::new(),
                    section_crossings: Vec::new(),
                    existing: Vec::new(),
                    source: VertexSource::CurveStart {
                        pair: pi,
                        curve: ci,
                    },
                });
                paves.push(Pave {
                    t: 0.0,
                    vertex: self.vertices.len() - 1,
                });
            }
        }
        // The blocks between consecutive paves; on a periodic curve the
        // last wraps round to the first.
        let mut blocks: Vec<(f64, f64, usize, usize)> = Vec::new();
        for w in paves.windows(2) {
            blocks.push((w[0].t, w[1].t, w[0].vertex, w[1].vertex));
        }
        if periodic {
            if let (Some(first), Some(last)) = (paves.first(), paves.last()) {
                let period = curve.period().ok_or(OpError::Internal(Fault::Invariant {
                    what: "a periodic curve's period",
                }))?;
                // A block that wraps ends one period after the first
                // pave, and never more than one period after the last —
                // with a single pave the two are the same turn, and the
                // sum can round one unit in the last place past it
                // (the checker's E1).
                let hi = (first.t + period).min(period_end(last.t, period));
                blocks.push((last.t, hi, last.vertex, first.vertex));
            }
        }
        // The operand edges of the two faces that lie on this curve: a
        // block that is a piece of one of them is that edge, not a section
        // edge, whatever the polygons' band says of its midpoint — its
        // split of the other face, where one is needed, is the edge's
        // image through the coincident neighbour (ADR-0004).
        let mut along: Vec<&EdgeInfo<'m>> = Vec::new();
        for (side, f) in [(0, fa), (1, fb)] {
            for &eid in f.edges() {
                let Some(e) = self.edge_info(side, eid) else {
                    continue;
                };
                let tol =
                    tolerance_of(&self.precision, e.tolerance, fa.tolerance.max(fb.tolerance));
                // Only the verdict: an edge in the plane of a planar
                // section conic — a rim circle beside the ellipse its cap
                // plane cuts from the other wall — has no closed form for
                // where the two meet, and needs none here.
                if curves_coincide(curve, e.curve, tol)
                    .map_err(|err| geometry(err, fa.shape(), e.shape()))?
                {
                    along.push(e);
                }
            }
        }
        let is_an_edge: Vec<bool> = blocks
            .iter()
            .map(|&(lo, hi, _, _)| {
                let mid = curve.point(0.5 * (lo + hi));
                along.iter().any(|e| {
                    e.curve
                        .project(mid)
                        .is_ok_and(|on| Self::strictly_inside(e, e.range, on.t))
                })
            })
            .collect();
        drop(along);
        let mut edges = Vec::new();
        for (k, (lo, hi, start, end)) in blocks.into_iter().enumerate() {
            if is_an_edge[k] {
                continue;
            }
            let Ok(range) = Interval::new(lo, hi) else {
                continue;
            };
            if range.length() <= 0.0 {
                continue;
            }
            let Some(uv) = Self::inside_both(fa, fb, curve.point(range.midpoint())) else {
                continue;
            };
            let (fa, fb) = (&self.faces[0][ia], &self.faces[1][ib]);
            let section = self.section_edge(fa, fb, curve, range, uv, curve_index, start, end)?;
            // Every vertex ≥ its edges: the ends carry at least the
            // section edge's tolerance.
            for k in [start, end] {
                let v = &mut self.vertices[k];
                v.floor = v.floor.max(section.tolerance);
            }
            edges.push(self.sections.len());
            self.sections.push(section);
        }
        self.curves.push(SectionCurve {
            pair: pi,
            index: ci,
            curve: curve.clone(),
            paves,
            edges,
        });
        Ok(())
    }

    /// A kept block as a section edge: the pcurve on each face, placed
    /// in the face's translate of the domain, and the tolerance raised to
    /// the pcurves' residual.
    #[allow(clippy::too_many_arguments)]
    fn section_edge(
        &self,
        fa: &FaceInfo<'m>,
        fb: &FaceInfo<'m>,
        curve: &Curve,
        range: Interval,
        uv_mid: [Point2; 2],
        curve_index: usize,
        start: usize,
        end: usize,
    ) -> Result<SectionEdge, OpError> {
        let base = fa.tolerance.max(fb.tolerance);
        let (pa, ra) = self.pcurve_of(fa, fb, curve, range, uv_mid[0], base)?;
        let (pb, rb) = self.pcurve_of(fb, fa, curve, range, uv_mid[1], base)?;
        let tolerance = base.max(ra).max(rb);
        if tolerance > self.precision.max_tolerance {
            return Err(OpError::Tolerance {
                entity: fa.shape(),
                wanted: tolerance,
            });
        }
        Ok(SectionEdge {
            curve: curve_index,
            range,
            start,
            end,
            tolerance,
            pcurves: [pa, pb],
        })
    }

    /// The pcurve of a curve block on one face, placed, with the largest
    /// deviation of its image from the curve at the model's check
    /// parameters. `base` is the tolerance the fit is asked for.
    fn pcurve_of(
        &self,
        f: &FaceInfo<'m>,
        other: &FaceInfo<'m>,
        curve: &Curve,
        range: Interval,
        uv_mid: Point2,
        base: f64,
    ) -> Result<(Curve2, f64), OpError> {
        let tol = Tolerance::new(base, self.precision.angular_tolerance);
        let pc = pcurve_on(curve, range, f.surface, tol)
            .map_err(|e| geometry(e, other.shape(), f.shape()))?;
        let pc = self.place(f, other, pc, range, uv_mid, base)?;
        let residual = samples(range, self.precision.check_samples)
            .into_iter()
            .map(|t| {
                let q = pc.point(t);
                (f.surface.point(q.x, q.y) - curve.point(t)).norm()
            })
            .fold(0.0, f64::max);
        Ok((pc, residual))
    }

    /// The pcurve translated by whole periods so its point at the block's
    /// midpoint is the face's own (u, v) there, and held to the face's
    /// (u, v) box in every periodic direction within `tolerance` converted
    /// there: a block that still leaves it crosses a seam inside itself,
    /// which the seam's own hit should have paved (ADR-0004).
    fn place(
        &self,
        f: &FaceInfo<'m>,
        other: &FaceInfo<'m>,
        pc: Curve2,
        range: Interval,
        uv_mid: Point2,
        tolerance: f64,
    ) -> Result<Curve2, OpError> {
        let periods = f.surface.period();
        let at = pc.point(range.midpoint());
        let mut by = Vec2::zeros();
        for (d, period) in periods.iter().enumerate() {
            if let Some(p) = period {
                by[d] = ((uv_mid[d] - at[d]) / p).round() * p;
            }
        }
        let pc = pc.translated(by);
        let near = band(f.surface, uv_mid, tolerance);
        for (d, period) in periods.iter().enumerate() {
            if period.is_none() {
                continue;
            }
            let (lo, hi) = (f.uv_lo[d] - near, f.uv_hi[d] + near);
            for t in samples(range, self.precision.check_samples) {
                let x = pc.point(t)[d];
                if x < lo || x > hi {
                    return Err(OpError::Internal(Fault::Seam {
                        face: f.id,
                        other: other.id,
                    }));
                }
            }
        }
        Ok(pc)
    }

    // -- the coincident pairs ------------------------------------------

    /// The pieces of an edge between consecutive paves, as the result
    /// cuts it.
    fn blocks_of(&self, e: &EdgeInfo<'m>) -> Vec<Block> {
        let mut stops: Vec<(f64, End)> = vec![(e.range.lo(), End::Operand(e.ends[0].0))];
        if let Some(paves) = self.paves.get(&e.id) {
            stops.extend(paves.iter().map(|p| (p.t, End::Section(p.vertex))));
        }
        stops.push((e.range.hi(), End::Operand(e.ends[1].0)));
        stops
            .windows(2)
            .enumerate()
            .filter_map(|(index, w)| {
                let range = Interval::new(w[0].0, w[1].0).ok()?;
                (range.length() > 0.0).then_some(Block {
                    index,
                    range,
                    start: w[0].1,
                    end: w[1].1,
                })
            })
            .collect()
    }

    /// An end as the section vertex it is merged into, when it is.
    fn canonical(&self, end: End) -> End {
        match end {
            End::Section(_) => end,
            End::Operand(v) => self
                .vertices
                .iter()
                .position(|x| x.existing.contains(&v))
                .map_or(end, End::Section),
        }
    }

    /// `t`, placed in `e`'s range, strictly inside `range` by more than
    /// the edge's tolerance converted to the parameter there.
    fn strictly_inside(e: &EdgeInfo<'m>, range: Interval, t: f64) -> bool {
        let Some(t) = e.in_range(t) else {
            return false;
        };
        let speed = e.curve.eval(t).d1.norm();
        let slack = if speed > 0.0 {
            e.tolerance / speed
        } else {
            0.0
        };
        t > range.lo() + slack && t < range.hi() - slack
    }

    /// The piece of an edge of `other` that `block` of `e` is a piece
    /// of: an edge on the same curve (the curve–curve `Coincident`
    /// verdict, never the polygon band) whose piece overlaps it — either
    /// midpoint strictly inside the other's range — and then the same
    /// piece: both midpoints inside, the same ends. `None` when no piece
    /// of `other` overlaps; the fault when one overlaps without being the
    /// same piece, since every vertex the two edges share should have
    /// paved both.
    fn matching_block(
        &self,
        side: usize,
        e: &EdgeInfo<'m>,
        block: &Block,
        other: &FaceInfo<'m>,
    ) -> Result<Option<(EdgeId, Block, bool)>, OpError> {
        let fault = || {
            OpError::Internal(Fault::CommonBlock {
                edge: e.id,
                face: other.id,
            })
        };
        let mid = e.curve.point(block.range.midpoint());
        for &gid in other.edges() {
            let same = if side == 0 {
                self.same_curve.contains(&(e.id, gid))
            } else {
                self.same_curve.contains(&(gid, e.id))
            };
            if !same {
                continue;
            }
            let Some(g) = self.edge_info(1 - side, gid) else {
                continue;
            };
            let Ok(on_g) = g.curve.project(mid) else {
                continue;
            };
            for gb in self.blocks_of(g) {
                let g_mid = g.curve.point(gb.range.midpoint());
                let Ok(on_e) = e.curve.project(g_mid) else {
                    continue;
                };
                let e_in_g = Self::strictly_inside(g, gb.range, on_g.t);
                let g_in_e = Self::strictly_inside(e, block.range, on_e.t);
                if !(e_in_g || g_in_e) {
                    continue;
                }
                if !(e_in_g && g_in_e) {
                    return Err(fault());
                }
                let reversed = e.curve.eval(block.range.midpoint()).d1.dot(
                    &g.curve
                        .eval(g.in_range(on_g.t).unwrap_or(gb.range.midpoint()))
                        .d1,
                ) < 0.0;
                let (es, ee) = (self.canonical(block.start), self.canonical(block.end));
                let (gs, ge) = (self.canonical(gb.start), self.canonical(gb.end));
                let ends_match = if reversed {
                    es == ge && ee == gs
                } else {
                    es == gs && ee == ge
                };
                if !ends_match {
                    return Err(fault());
                }
                return Ok(Some((gid, gb, reversed)));
            }
        }
        Ok(None)
    }

    /// `block` of `e`, an edge of face `f` of operand `side`, as an image
    /// on `other` under pair `pi`: kept when the block lies inside that
    /// face by its polygons — within the band of a loop without being on
    /// any edge of it, the winding number decides — with its pcurve there
    /// and the edge's tolerance raised to the pcurve's residual; `None`
    /// when it lies outside.
    fn image(
        &self,
        pi: usize,
        side: usize,
        e: &EdgeInfo<'m>,
        f: &FaceInfo<'m>,
        other: &FaceInfo<'m>,
        block: &Block,
    ) -> Result<Option<EdgeImage>, OpError> {
        let mid = e.curve.point(block.range.midpoint());
        let projection = other
            .surface
            .project(mid)
            .map_err(|err| geometry(err, e.shape(), other.shape()))?;
        let (s, shift) = other.domain.side(projection.uv);
        let uv_mid = projection.uv + shift;
        let inside = match s {
            Side::Outside => false,
            Side::Inside => true,
            Side::Boundary => other.domain.winds_around(projection.uv),
        };
        if !inside {
            return Ok(None);
        }
        let fit = e.tolerance + f.tolerance.max(other.tolerance);
        let (pcurve, residual) = self.pcurve_of(other, f, e.curve, block.range, uv_mid, fit)?;
        let tolerance = e.tolerance.max(residual);
        if tolerance > self.precision.max_tolerance {
            return Err(OpError::Tolerance {
                entity: e.shape(),
                wanted: tolerance,
            });
        }
        Ok(Some(EdgeImage {
            pair: pi,
            side,
            edge: e.id,
            index: block.index,
            range: block.range,
            pcurve,
            tolerance,
        }))
    }

    /// Every piece of every edge of both faces of each `Coincident` pair
    /// decided against the other face: inside it, an image with its
    /// pcurve there; along its boundary, a common block with the piece
    /// of the other face's edge it coincides with, recorded once — from
    /// `b`'s side, under the first pair it is met in — with a pcurve for
    /// each use of `b`'s piece; outside it, nothing.
    fn coincident(&mut self) -> Result<(), OpError> {
        let mut images = Vec::new();
        let mut blocks = Vec::new();
        let mut floors: Vec<(End, f64)> = Vec::new();
        for pi in 0..self.pairs.len() {
            if self.pairs[pi].intersection != SurfaceIntersection::Coincident {
                continue;
            }
            let (ia, ib) = self.pair_faces[pi];
            for side in 0..2 {
                let (f, other) = if side == 0 {
                    (&self.faces[0][ia], &self.faces[1][ib])
                } else {
                    (&self.faces[1][ib], &self.faces[0][ia])
                };
                for &eid in f.edges() {
                    let Some(e) = self.edge_info(side, eid) else {
                        continue;
                    };
                    for block in self.blocks_of(e) {
                        // Along the other face's boundary, by the curves: a
                        // common block, recorded once, from `b`'s side and
                        // under the first pair it is met in.
                        if let Some((gid, gb, reversed)) =
                            self.matching_block(side, e, &block, other)?
                        {
                            let seen = blocks
                                .iter()
                                .any(|x: &CommonBlock| x.b == (e.id, block.index));
                            if side == 1 && !seen {
                                let (b, raised) =
                                    self.common_block(pi, e, &block, gid, &gb, reversed)?;
                                floors.extend(
                                    [block.start, block.end, gb.start, gb.end]
                                        .into_iter()
                                        .map(|end| (end, raised)),
                                );
                                blocks.push(b);
                            }
                            continue;
                        }
                        if let Some(image) = self.image(pi, side, e, f, other, &block)? {
                            floors.push((block.start, image.tolerance));
                            floors.push((block.end, image.tolerance));
                            images.push(image);
                        }
                    }
                }
            }
        }
        // An edge lying in a face of the other operand that no face of its
        // own is coincident with — a seam on a ruling two parallel walls
        // cross along, whose block of the section curve is the edge and
        // not a section edge — has no coincident neighbour to place it on
        // that face, so its pieces inside it are placed here, under the
        // pair of the first face that uses it.
        for &(eid, gid) in &self.coincident {
            let side = usize::from(self.edge_info(0, eid).is_none());
            let (Some(e), Some(other)) = (
                self.edge_info(side, eid),
                self.faces[1 - side].iter().find(|g| g.id == gid),
            ) else {
                continue;
            };
            let pair_with = |f: FaceId| {
                self.pairs.iter().position(|p| {
                    if side == 0 {
                        p.a == f && p.b == gid
                    } else {
                        p.a == gid && p.b == f
                    }
                })
            };
            let owners: Vec<(&FaceInfo<'m>, Option<usize>)> = self.faces[side]
                .iter()
                .filter(|f| f.edges().contains(&eid))
                .map(|f| (f, pair_with(f.id)))
                .collect();
            let placed = owners.iter().any(|&(_, pi)| {
                pi.is_some_and(|pi| self.pairs[pi].intersection == SurfaceIntersection::Coincident)
            });
            let Some((f, pi)) = owners.iter().find_map(|&(f, pi)| Some((f, pi?))) else {
                continue;
            };
            if placed {
                continue;
            }
            for block in self.blocks_of(e) {
                if let Some(image) = self.image(pi, side, e, f, other, &block)? {
                    floors.push((block.start, image.tolerance));
                    floors.push((block.end, image.tolerance));
                    images.push(image);
                }
            }
        }
        for (end, tolerance) in floors {
            if let End::Section(k) = self.canonical(end) {
                let v = &mut self.vertices[k];
                v.floor = v.floor.max(tolerance);
            }
        }
        self.images = images;
        self.blocks = blocks;
        Ok(())
    }

    /// A common block: `b`'s piece `block` of `e` is `a`'s piece `gb` of
    /// `gid`, with the pcurve of `a`'s piece for every use of `b`'s by a
    /// face of `b`, and the tolerance the two edges' raised to the
    /// residuals.
    fn common_block(
        &self,
        pi: usize,
        e: &EdgeInfo<'m>,
        block: &Block,
        gid: EdgeId,
        gb: &Block,
        reversed: bool,
    ) -> Result<(CommonBlock, f64), OpError> {
        let m = self.m;
        let g = self
            .edge_info(0, gid)
            .ok_or(OpError::Internal(Fault::Invariant {
                what: "a's edge info for a common block",
            }))?;
        let g_mid = g.curve.point(gb.range.midpoint());
        // `b`'s parameter of `a`'s midpoint: where each use's own pcurve
        // gives the (u, v) the fitted one has to be placed at.
        let on_e = e
            .curve
            .project(g_mid)
            .map_err(|err| geometry(err, g.shape(), e.shape()))?;
        let t_e = e.in_range(on_e.t).unwrap_or(block.range.midpoint());
        let mut tolerance = g.tolerance.max(e.tolerance);
        let mut pcurves = Vec::new();
        for fb in &self.faces[1] {
            if !fb.edges().contains(&e.id) {
                continue;
            }
            let face = m.face(fb.id)?;
            for c in face.loops().iter().flat_map(|l| l.coedges()) {
                if c.edge() != e.id {
                    continue;
                }
                let own = m.curve2(c.pcurve())?;
                let uv_mid = own.point(t_e);
                let fit = g.tolerance.max(e.tolerance) + fb.tolerance;
                let (pc, residual) =
                    self.pcurve_of(fb, g_face_of(self, gid), g.curve, gb.range, uv_mid, fit)?;
                tolerance = tolerance.max(residual);
                pcurves.push((c.pcurve(), pc));
            }
        }
        if tolerance > self.precision.max_tolerance {
            return Err(OpError::Tolerance {
                entity: e.shape(),
                wanted: tolerance,
            });
        }
        Ok((
            CommonBlock {
                pair: pi,
                a: (gid, gb.index),
                b: (e.id, block.index),
                reversed,
                pcurves,
                tolerance,
            },
            tolerance,
        ))
    }

    fn finish(self) -> Interferences {
        let m = self.m;
        let vertices = self.vertices.iter().map(|v| v.finish(m)).collect();
        Interferences {
            a: self.a,
            b: self.b,
            pairs: self.pairs,
            hits: self.hits,
            section_crossings: self.section_crossings,
            vertices,
            paves: self.paves,
            curves: self.curves,
            sections: self.sections,
            contacts: self.contacts,
            coincident: self.coincident,
            crossings: self.crossings,
            images: self.images,
            blocks: self.blocks,
        }
    }
}

/// A face of `a` that uses edge `gid`, for naming in an error; the first
/// face of `a` when none does.
fn g_face_of<'b, 'm>(build: &'b Build<'m>, gid: EdgeId) -> &'b FaceInfo<'m> {
    build.faces[0]
        .iter()
        .find(|f| f.edges().contains(&gid))
        .unwrap_or(&build.faces[0][0])
}
