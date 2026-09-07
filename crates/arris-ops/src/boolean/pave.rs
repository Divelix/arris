//! Building the pave model (ADR-0004): face pairs, edge-on-face hits,
//! the hits merged into section vertices, paves, section curves cut into
//! blocks and the blocks kept as section edges with their pcurves.

use core::f64::consts::PI;
use std::collections::BTreeMap;

use arris_check::arris_topo::arris_geom::region2::Side;
use arris_check::arris_topo::arris_geom::{
    Curve, Curve2, CurveSurfaceIntersection, GeomError, SurfaceIntersection,
    intersect_curve_surface, intersect_surfaces, pcurve_on,
};
use arris_check::arris_topo::arris_math::{
    Interval, Point2, Point3, Precision, Tolerance, Vec2, wrap_angle,
};
use arris_check::arris_topo::{
    Body, EdgeId, FaceId, Model, NotFound, Shape, Vertex as VertexHandle, VertexId,
};

use super::faces::{EdgeInfo, FaceInfo, band};
use super::{
    EdgeFaceHit, FacePair, Interferences, Landing, Pave, SectionCurve, SectionEdge, SectionVertex,
    VertexSource,
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
            existing: self.existing.clone(),
            source: self.source,
        }
    }
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
    vertices: Vec<VertexBuild>,
    paves: BTreeMap<EdgeId, Vec<Pave>>,
    curves: Vec<SectionCurve>,
    sections: Vec<SectionEdge>,
    coincident: Vec<(EdgeId, FaceId)>,
}

pub(super) fn build(m: &Model, a: Body, b: Body) -> Result<Interferences, OpError> {
    let of = |body: Body| move |_: NotFound| OpError::NotFound(shape_of(body));
    let faces = [
        FaceInfo::of_body(m, a).map_err(of(a))?,
        FaceInfo::of_body(m, b).map_err(of(b))?,
    ];
    let edges = [
        EdgeInfo::of_body(m, a).map_err(of(a))?,
        EdgeInfo::of_body(m, b).map_err(of(b))?,
    ];
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
        vertices: Vec::new(),
        paves: BTreeMap::new(),
        curves: Vec::new(),
        sections: Vec::new(),
        coincident: Vec::new(),
    };
    build.face_pairs()?;
    build.hits()?;
    build.merge()?;
    build.pave_edges();
    build.sections()?;
    Ok(build.finish())
}

fn shape_of(body: Body) -> Shape {
    Shape::new(body.id, body.orientation)
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
    fn not_found(&self, side: usize) -> OpError {
        OpError::NotFound(shape_of(if side == 0 { self.a } else { self.b }))
    }

    /// Every face pair whose boxes overlap, intersected.
    fn face_pairs(&mut self) -> Result<(), OpError> {
        for (ia, fa) in self.faces[0].iter().enumerate() {
            for (ib, fb) in self.faces[1].iter().enumerate() {
                if !fa.bounds.intersects(&fb.bounds) {
                    continue;
                }
                let tol = tolerance_of(&self.precision, fa.tolerance, fb.tolerance);
                let intersection = intersect_surfaces(fa.surface, fb.surface, tol)
                    .map_err(|e| geometry(e, fa.shape(), fb.shape()))?;
                self.pairs.push(FacePair {
                    a: fa.id,
                    b: fb.id,
                    intersection,
                });
                self.pair_faces.push((ia, ib));
            }
        }
        Ok(())
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
                    self.hit_edge_face(e, f, other, &mut found, &mut coincident)?;
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
        face_side: usize,
        found: &mut Vec<(EdgeFaceHit, f64)>,
        coincident: &mut Vec<(EdgeId, FaceId)>,
    ) -> Result<(), OpError> {
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
            let (side, shift) = f.side(h.uv);
            let landing = match side {
                Side::Outside => continue,
                Side::Inside => Landing::Interior,
                Side::Boundary => match f
                    .boundary_entity(self.m, h.point)
                    .map_err(|_| self.not_found(face_side))?
                {
                    Some(shape) => Landing::Boundary(shape),
                    // Within the (u, v) band of a loop's polygon but within
                    // no edge's or vertex's own tolerance: not on the
                    // boundary, so the winding number alone decides.
                    None if f.winds_around(h.uv) => Landing::Interior,
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

    /// Hits merged into section vertices: a hit joins the first vertex
    /// (in creation order) that shares an operand vertex with it or whose
    /// point is within the larger of the two tolerances; otherwise it
    /// starts one. A touch joins nothing.
    fn merge(&mut self) -> Result<(), OpError> {
        let m = self.m;
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
            let mut base = hit_tol;
            for &v in &existing {
                base = base.max(m.vertex(v).map_err(|_| self.not_found(0))?.tolerance());
            }
            let found = self.vertices.iter().position(|v| {
                v.existing.iter().any(|x| existing.contains(x))
                    || (v.point(m) - point).norm() <= v.tolerance(m).max(hit_tol)
            });
            let k = match found {
                Some(k) => {
                    let v = &mut self.vertices[k];
                    v.points.push(point);
                    v.base = v.base.max(base);
                    v.hits.push(i);
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
                        hits: vec![i],
                        existing,
                        source: VertexSource::Hits,
                    });
                    self.vertices.len() - 1
                }
            };
            self.hits[i].vertex = Some(k);
        }
        for v in &self.vertices {
            let wanted = v.tolerance(m);
            if wanted > self.precision.max_tolerance {
                let entity = v
                    .hits
                    .first()
                    .map(|&h| Shape::new(self.hits[h].edge, self.a.orientation))
                    .unwrap_or_else(|| shape_of(self.a));
                return Err(OpError::Tolerance { entity, wanted });
            }
        }
        Ok(())
    }

    /// The paves on the operand edges: each hit's vertex at its `t`,
    /// unless the hit is at the edge's own end; one pave per vertex per
    /// edge, ascending by `t`.
    fn pave_edges(&mut self) {
        for h in &self.hits {
            let Some(vertex) = h.vertex else {
                continue;
            };
            if h.at_vertex.is_some() {
                continue;
            }
            let list = self.paves.entry(h.edge).or_default();
            if list.iter().all(|p| p.vertex != vertex) {
                list.push(Pave { t: h.t, vertex });
            }
        }
        for list in self.paves.values_mut() {
            list.sort_by(|x, y| x.t.total_cmp(&y.t));
        }
    }

    /// `point` projected onto both faces' surfaces and inside both, with
    /// the (u, v) of each in the translate the face's loops use.
    fn inside_both(fa: &FaceInfo<'m>, fb: &FaceInfo<'m>, point: Point3) -> Option<[Point2; 2]> {
        let mut out = [Point2::origin(); 2];
        for (i, f) in [fa, fb].into_iter().enumerate() {
            let projection = f.surface.project(point).ok()?;
            let (side, shift) = f.side(projection.uv);
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
            };
            for (ci, curve) in curves.iter().enumerate() {
                self.section_curve(pi, ci, curve)?;
            }
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
                let period = curve.period().unwrap_or(0.0);
                blocks.push((last.t, first.t + period, last.vertex, first.vertex));
            }
        }
        let mut edges = Vec::new();
        for (lo, hi, start, end) in blocks {
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

    /// The pcurve of a section block on one face of its pair, placed,
    /// with the largest deviation of its image from the curve at the
    /// model's check parameters.
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

    fn finish(self) -> Interferences {
        let m = self.m;
        let vertices = self.vertices.iter().map(|v| v.finish(m)).collect();
        Interferences {
            a: self.a,
            b: self.b,
            pairs: self.pairs,
            hits: self.hits,
            vertices,
            paves: self.paves,
            curves: self.curves,
            sections: self.sections,
            coincident: self.coincident,
        }
    }
}
