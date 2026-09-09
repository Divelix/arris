//! The operands as the pave model reads them: every face with its loops
//! discretised once, its box and the (u, v) answer to "is this point on
//! me"; every edge with its curve, range, ends and box.

use std::collections::BTreeSet;

use arris_check::arris_topo::arris_geom::region2::{Polygon2, Side, discretise, point_side};
use arris_check::arris_topo::arris_geom::{Curve, Surface};
use arris_check::arris_topo::arris_math::{Aabb, Interval, Point2, Point3, Vec2, is_negligible};
use arris_check::arris_topo::entity::Face;
use arris_check::arris_topo::{
    Body, EdgeId, FaceId, Model, NotFound, Orientation, Shape, VertexId,
};

/// How far a (u, v) step may be and still be within `tolerance` in 3D at
/// `uv`, in the tighter of the two directions: the tolerance divided by
/// the surface's speed there, the raw tolerance where a speed is zero or
/// negligible beside the other. The same conversion the checker applies
/// to the model's parametric tolerance (`docs/DATA-MODEL.md`
/// §Pcurves), here to an entity's own.
pub(crate) fn band(surface: &Surface, uv: Point2, tolerance: f64) -> f64 {
    let e = surface.eval(uv.x, uv.y);
    let speeds = [e.du.norm(), e.dv.norm()];
    let per = [0, 1].map(|i| {
        if is_negligible(speeds[i], speeds[1 - i]) || speeds[i] == 0.0 {
            tolerance
        } else {
            tolerance / speeds[i]
        }
    });
    per[0].min(per[1])
}

/// The offsets a periodic parameter is tried at: nothing, and a period
/// either way; once for a direction without a period.
fn shifts(period: Option<f64>) -> impl Iterator<Item = f64> {
    let p = period.unwrap_or(0.0);
    [0.0, p, -p].into_iter().take(if p == 0.0 { 1 } else { 3 })
}

/// A face of an operand, read once.
pub(crate) struct FaceInfo<'m> {
    pub id: FaceId,
    pub surface: &'m Surface,
    pub tolerance: f64,
    /// The loops as polygons in (u, v), within `chord` of the pcurves.
    pub polygons: Vec<Polygon2>,
    /// The (u, v) box of the polygons, grown by their chord deviation.
    pub uv_lo: Point2,
    pub uv_hi: Point2,
    /// The 3D box, grown by the tolerance.
    pub bounds: Aabb,
    /// The edges of the loops, each once, in loop order.
    pub edges: Vec<EdgeId>,
}

impl<'m> FaceInfo<'m> {
    /// Every face of `body` in iteration order.
    pub(crate) fn of_body(m: &'m Model, body: Body) -> Result<Vec<Self>, NotFound> {
        m.faces(body)?
            .into_iter()
            .map(|f| Self::of(m, f.id))
            .collect()
    }

    fn of(m: &'m Model, id: FaceId) -> Result<Self, NotFound> {
        let face: &Face = m.face(id)?;
        let surface = m.surface(face.surface())?;
        let tolerance = face.tolerance();
        // The chord the loops are discretised at: the face's tolerance at
        // the first point of its first pcurve, in the tighter direction.
        let at = face
            .loops()
            .iter()
            .flat_map(|l| l.coedges())
            .find_map(|c| m.curve2(c.pcurve()).ok())
            .map_or(Point2::origin(), |p| p.point(0.0));
        let chord = band(surface, at, tolerance);
        let mut polygons = Vec::with_capacity(face.loops().len());
        for l in face.loops() {
            polygons.push(discretise(&m.loop_pieces(l)?, chord));
        }
        let mut lo = Point2::new(f64::INFINITY, f64::INFINITY);
        let mut hi = Point2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
        for polygon in &polygons {
            let d = polygon.chord_deviation();
            for p in polygon.points() {
                lo = Point2::new(lo.x.min(p.x - d), lo.y.min(p.y - d));
                hi = Point2::new(hi.x.max(p.x + d), hi.y.max(p.y + d));
            }
        }
        if !(lo.x.is_finite() && hi.x.is_finite() && lo.y.is_finite() && hi.y.is_finite()) {
            lo = at;
            hi = at;
        }
        let mut edges = Vec::new();
        let mut seen = BTreeSet::new();
        let mut bounds: Option<Aabb> = None;
        for c in face.loops().iter().flat_map(|l| l.coedges()) {
            if seen.insert(c.edge()) {
                edges.push(c.edge());
            }
            let edge = m.edge(c.edge())?;
            let Some((curve, range)) = edge.curve() else {
                continue;
            };
            if let Some(b) = m.curve(curve)?.bounds(range) {
                let b = b.inflated(edge.tolerance());
                bounds = Some(bounds.map_or(b, |acc| acc.union(b)));
            }
        }
        let uv = [Interval::new(lo.x, hi.x), Interval::new(lo.y, hi.y)];
        if let (Ok(u), Ok(v)) = (uv[0], uv[1]) {
            if let Some(b) = surface.bounds([u, v]) {
                bounds = Some(bounds.map_or(b, |acc| acc.union(b)));
            }
        }
        let bounds = bounds
            .unwrap_or_else(|| Aabb::of_point(surface.point(at.x, at.y)))
            .inflated(tolerance);
        Ok(FaceInfo {
            id,
            surface,
            tolerance,
            polygons,
            uv_lo: lo,
            uv_hi: hi,
            bounds,
            edges,
        })
    }

    /// The face as a `Forward` shape, for an error.
    pub(crate) fn shape(&self) -> Shape {
        Shape::new(self.id, Orientation::Forward)
    }

    /// Where `uv` lies with respect to the face's loops, with the whole
    /// periods added to `uv` that put it in the translate of the domain
    /// the loops are written in: `Inside` wins over `Boundary` over
    /// `Outside` across the translates. The boundary band is the face's
    /// tolerance converted at `uv`.
    pub(crate) fn side(&self, uv: Point2) -> (Side, Vec2) {
        let near = band(self.surface, uv, self.tolerance);
        let periods = self.surface.period();
        let mut best = (Side::Outside, Vec2::zeros());
        for du in shifts(periods[0]) {
            for dv in shifts(periods[1]) {
                let shift = Vec2::new(du, dv);
                match point_side(&self.polygons, uv + shift, near) {
                    Side::Inside => return (Side::Inside, shift),
                    Side::Boundary => {
                        if best.0 == Side::Outside {
                            best = (Side::Boundary, shift);
                        }
                    }
                    Side::Outside => {}
                }
            }
        }
        best
    }

    /// `true` when the loops wind around `uv` (in any translate): the
    /// answer without a boundary band, for a point the band called
    /// `Boundary` and no edge or vertex of the face claims.
    pub(crate) fn winds_around(&self, uv: Point2) -> bool {
        let periods = self.surface.period();
        for du in shifts(periods[0]) {
            for dv in shifts(periods[1]) {
                let p = uv + Vec2::new(du, dv);
                let winding: i32 = self.polygons.iter().map(|g| g.winding_number(p)).sum();
                if winding != 0 {
                    return true;
                }
            }
        }
        false
    }

    /// The vertex, else the edge, of the face that `point` is within the
    /// tolerance of, most specific first; `None` when it is within
    /// neither.
    pub(crate) fn boundary_entity(
        &self,
        m: &Model,
        point: Point3,
    ) -> Result<Option<Shape>, NotFound> {
        let mut vertices = Vec::new();
        for &e in &self.edges {
            let edge = m.edge(e)?;
            for v in [edge.start(), edge.end()] {
                if !vertices.contains(&v) {
                    vertices.push(v);
                }
            }
        }
        for v in vertices {
            let vertex = m.vertex(v)?;
            if (point - vertex.point()).norm() <= vertex.tolerance() {
                return Ok(Some(Shape::new(v, Orientation::Forward)));
            }
        }
        for &e in &self.edges {
            let edge = m.edge(e)?;
            let Some((curve_id, range)) = edge.curve() else {
                continue;
            };
            let curve = m.curve(curve_id)?;
            let Ok(projection) = curve.project(point) else {
                continue;
            };
            let within = shifts(curve.period()).any(|d| range.contains(projection.t + d));
            if within && projection.distance <= edge.tolerance() {
                return Ok(Some(Shape::new(e, Orientation::Forward)));
            }
        }
        Ok(None)
    }
}

/// An edge of an operand with a curve, read once. Degenerate edges have
/// no curve to pierce anything with and are left out.
pub(crate) struct EdgeInfo<'m> {
    pub id: EdgeId,
    pub curve: &'m Curve,
    pub range: Interval,
    pub tolerance: f64,
    pub ends: [(VertexId, Point3, f64); 2],
    pub bounds: Aabb,
}

impl<'m> EdgeInfo<'m> {
    /// Every non-degenerate edge of `body` in iteration order.
    pub(crate) fn of_body(m: &'m Model, body: Body) -> Result<Vec<Self>, NotFound> {
        let mut out = Vec::new();
        for e in m.edges(body)? {
            let edge = m.edge(e.id)?;
            let Some((curve_id, range)) = edge.curve() else {
                continue;
            };
            let curve = m.curve(curve_id)?;
            let (start, end) = (m.vertex(edge.start())?, m.vertex(edge.end())?);
            let bounds = curve
                .bounds(range)
                .unwrap_or_else(|| Aabb::of_point(start.point()).union(Aabb::of_point(end.point())))
                .inflated(edge.tolerance());
            out.push(EdgeInfo {
                id: e.id,
                curve,
                range,
                tolerance: edge.tolerance(),
                ends: [
                    (edge.start(), start.point(), start.tolerance()),
                    (edge.end(), end.point(), end.tolerance()),
                ],
                bounds,
            });
        }
        Ok(out)
    }

    /// The edge as a `Forward` shape, for an error.
    pub(crate) fn shape(&self) -> Shape {
        Shape::new(self.id, Orientation::Forward)
    }

    /// `t` placed in the edge's range — a periodic parameter moved by a
    /// whole period when that puts it there — or `None` when it lies
    /// outside by more than the edge's tolerance converted to the
    /// parameter. A value within that band of an end is clamped to it.
    pub(crate) fn in_range(&self, t: f64) -> Option<f64> {
        let speed = self.curve.eval(t).d1.norm();
        let slack = if speed > 0.0 {
            self.tolerance / speed
        } else {
            0.0
        };
        for d in shifts(self.curve.period()) {
            let s = t + d;
            if self.range.lo() - slack <= s && s <= self.range.hi() + slack {
                return Some(self.range.clamp(s));
            }
        }
        None
    }

    /// The end vertex `point` is within the tolerance of, the start
    /// first.
    pub(crate) fn vertex_at(&self, point: Point3) -> Option<VertexId> {
        self.ends
            .iter()
            .find(|(_, p, tol)| (point - p).norm() <= *tol)
            .map(|(v, _, _)| *v)
    }
}
