//! Point classification against a body (`docs/01-architecture.md` §The
//! checker, ADR-0004): inside, outside, or on a named entity.
//!
//! This is B1's ray cast, made public and complete. The row that proves a
//! shell nesting and the predicate that decides which piece of a split
//! face a boolean keeps are one piece of code, so the two can never
//! disagree about a point — which is the consistency a boolean needs from
//! its classifier, not merely correctness case by case.

use std::collections::BTreeMap;
use std::fmt;

use arris_topo::arris_geom::region2::{Polygon2, Side, discretise, point_side};
use arris_topo::arris_geom::{
    Curve, CurveSurfaceIntersection, GeomError, Surface, intersect_curve_surface,
};
use arris_topo::arris_math::{Point2, Point3, Precision, UnitVec3, Vec3};
use arris_topo::entity::EdgeGeometry;
use arris_topo::{Body, EdgeId, FaceId, Model, NotFound, Orientation, Shape, VertexId};

use crate::check::uv_bounds;

/// The directions a containment ray is tried in, in order: the axes
/// first, then directions no two faces of an axis-aligned body share a
/// plane with. The first that meets no face boundary within tolerance
/// decides, so the answer is the same on every platform.
pub(crate) const RAY_DIRECTIONS: [[f64; 3]; 8] = [
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
    [1.0, 1.0, 1.0],
    [1.0, 2.0, 3.0],
    [-3.0, 1.0, 2.0],
    [2.0, -3.0, 1.0],
    [1.0, -2.0, -5.0],
];

/// Where a point lies with respect to a body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    /// Strictly inside the material.
    Inside,
    /// Strictly outside it.
    Outside,
    /// On the boundary, within the tolerance of the entity named: the
    /// vertex, else the edge, else the face — the most specific one the
    /// point is within, so a point at a corner is `On` the vertex and
    /// never one of the edges that meet there.
    On(Shape),
}

/// Why a point could not be classified. Never a guess: an operation that
/// gets one of these reports it with the entities it was about.
#[derive(Debug, Clone, PartialEq)]
pub enum ClassifyError {
    /// An id the body reaches does not resolve.
    Unresolved(NotFound),
    /// A ray against one of the body's surfaces has no closed form —
    /// a cone, a sphere, a torus or a NURBS in cycle 1.
    Geometry(GeomError),
    /// Every one of the eight ray directions was abandoned: each grazed a
    /// face's boundary, touched a surface tangentially or lay in one.
    Undecided {
        /// The body.
        body: Body,
        /// The point.
        point: Point3,
    },
}

impl fmt::Display for ClassifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClassifyError::Unresolved(e) => write!(f, "{e}"),
            ClassifyError::Geometry(e) => write!(f, "{e}"),
            ClassifyError::Undecided { body, point } => write!(
                f,
                "no ray from ({}, {}, {}) decides {body}: every direction grazed it",
                point.x, point.y, point.z
            ),
        }
    }
}

impl std::error::Error for ClassifyError {}

impl From<NotFound> for ClassifyError {
    fn from(e: NotFound) -> Self {
        ClassifyError::Unresolved(e)
    }
}

impl From<GeomError> for ClassifyError {
    fn from(e: GeomError) -> Self {
        ClassifyError::Geometry(e)
    }
}

/// Where `point` lies with respect to `body`: `On` the most specific
/// entity it is within the tolerance of, and otherwise `Inside` or
/// `Outside` by the parity of the crossings of a ray from it.
///
/// The boundary test comes first and is by the entities' *own*
/// tolerances (`docs/02-data-model.md` §Tolerances): a vertex within its
/// tolerance, then an edge within its, then a face within its. Only a
/// point that is on nothing is cast for, and a direction whose ray grazes
/// a face's boundary, touches a surface tangentially or lies in one is
/// abandoned for the next; all eight abandoned is
/// [`ClassifyError::Undecided`], never a guess.
///
/// Errors: an id that does not resolve, a surface a ray has no closed
/// form against (a cone, a sphere, a torus or a NURBS in cycle 1), or
/// `Undecided`.
///
/// ```
/// use arris_check::classify::{Classification, classify_point};
/// use arris_debug::sample;
/// use arris_topo::Model;
/// use arris_topo::arris_math::Point3;
///
/// let mut m = Model::default();
/// let body = sample::cylinder(&mut m, 4.0, 12.0)?;
/// assert_eq!(classify_point(&m, body, Point3::new(0.0, 0.0, 6.0))?, Classification::Inside);
/// assert_eq!(classify_point(&m, body, Point3::new(9.0, 0.0, 6.0))?, Classification::Outside);
/// let on = classify_point(&m, body, Point3::new(4.0, 0.0, 6.0))?;
/// assert!(matches!(on, Classification::On(_)), "on the wall");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn classify_point(
    model: &Model,
    body: Body,
    point: Point3,
) -> Result<Classification, ClassifyError> {
    let faces = model.closure(body)?.faces;
    let classifier = Classifier::over(model, faces);
    if let Some(shape) = classifier.on(point)? {
        return Ok(Classification::On(shape));
    }
    match classifier.contains(point)? {
        Some(true) => Ok(Classification::Inside),
        Some(false) => Ok(Classification::Outside),
        None => Err(ClassifyError::Undecided { body, point }),
    }
}

/// The faces a point is classified against, with their loops discretised
/// once: what [`classify_point`] builds over a body and B1 builds over one
/// shell.
pub(crate) struct Classifier<'m> {
    model: &'m Model,
    precision: Precision,
    faces: Vec<FaceId>,
    polygons: BTreeMap<FaceId, Vec<Polygon2>>,
}

impl<'m> Classifier<'m> {
    /// A classifier over `faces`, their loops discretised at the model's
    /// parametric tolerance. Faces that do not resolve are skipped; the
    /// ray cast reports them by finding no crossing, never by panicking.
    pub(crate) fn over(model: &'m Model, faces: Vec<FaceId>) -> Self {
        let precision = model.precision();
        let mut polygons = BTreeMap::new();
        for &id in &faces {
            let Ok(face) = model.face(id) else {
                continue;
            };
            let Ok(surface) = model.surface(face.surface()) else {
                continue;
            };
            // The chord tolerance is the model's parametric tolerance at
            // the first point of the face's first pcurve, in the tighter
            // of the two directions — what the checker's `Full` rows use.
            let at = face
                .loops()
                .iter()
                .flat_map(|l| l.coedges())
                .find_map(|c| model.curve2(c.pcurve()).ok())
                .map_or(Point2::origin(), |p| p.point(0.0));
            let bound = uv_bounds(&precision, surface, at);
            let chord = bound[0].min(bound[1]);
            let rings = face
                .loops()
                .iter()
                .filter_map(|l| model.loop_pieces(l).ok().map(|ps| discretise(&ps, chord)))
                .collect();
            polygons.insert(id, rings);
        }
        Classifier {
            model,
            precision,
            faces,
            polygons,
        }
    }

    /// Where `uv` lies with respect to `face`'s loops. A periodic
    /// parameter is tried a period either way as well, since a face's
    /// loops may be written in any translate of the fundamental domain
    /// and the projection's `uv` is in the domain's first copy.
    fn face_side(&self, face: FaceId, surface: &Surface, uv: Point2) -> Side {
        let Some(polygons) = self.polygons.get(&face) else {
            return Side::Outside;
        };
        let bound = uv_bounds(&self.precision, surface, uv);
        let near = bound[0].min(bound[1]);
        let period = surface.period();
        let mut best = Side::Outside;
        for du in shifts(period[0]) {
            for dv in shifts(period[1]) {
                match point_side(polygons, Point2::new(uv.x + du, uv.y + dv), near) {
                    Side::Inside => return Side::Inside,
                    Side::Boundary => best = Side::Boundary,
                    Side::Outside => {}
                }
            }
        }
        best
    }

    /// The entity `point` is on, most specific first, or `None`.
    pub(crate) fn on(&self, point: Point3) -> Result<Option<Shape>, ClassifyError> {
        let model = self.model;
        let mut vertices: Vec<VertexId> = Vec::new();
        let mut edges: Vec<EdgeId> = Vec::new();
        for &id in &self.faces {
            let Ok(face) = model.face(id) else { continue };
            for coedge in face.loops().iter().flat_map(|l| l.coedges()) {
                edges.push(coedge.edge());
                if let Ok(edge) = model.edge(coedge.edge()) {
                    vertices.extend([edge.start(), edge.end()]);
                }
            }
        }
        vertices.sort_unstable();
        vertices.dedup();
        edges.sort_unstable();
        edges.dedup();
        for id in vertices {
            let v = model.vertex(id)?;
            if (point - v.point()).norm() <= v.tolerance() {
                return Ok(Some(Shape::new(id, Orientation::Forward)));
            }
        }
        for id in edges {
            let edge = model.edge(id)?;
            let EdgeGeometry::Curve { curve, range } = edge.geometry() else {
                // A degenerate edge's locus is its vertex, which the loop
                // above already answered for.
                continue;
            };
            let curve = model.curve(curve)?;
            // A point on the axis of a circle has no nearest parameter;
            // it is a radius away from the curve, so it is not on it.
            let Ok(projection) = curve.project(point) else {
                continue;
            };
            let period = curve.period();
            let within = shifts(period).any(|d| range.contains(projection.t + d));
            if within && projection.distance <= edge.tolerance() {
                return Ok(Some(Shape::new(id, Orientation::Forward)));
            }
        }
        for &id in &self.faces {
            let face = model.face(id)?;
            let surface = model.surface(face.surface())?;
            let Ok(projection) = surface.project(point) else {
                continue;
            };
            if projection.distance <= face.tolerance()
                && self.face_side(id, surface, projection.uv) != Side::Outside
            {
                return Ok(Some(Shape::new(id, Orientation::Forward)));
            }
        }
        Ok(None)
    }

    /// Whether `point` is inside the closed surface the faces make, by
    /// the parity of the crossings of a ray from it; `None` when every
    /// one of the eight directions was abandoned, which the caller that
    /// knows the body turns into [`ClassifyError::Undecided`]. Errors:
    /// an id that does not resolve, or a ray against a surface with no
    /// closed form.
    pub(crate) fn contains(&self, point: Point3) -> Result<Option<bool>, ClassifyError> {
        let model = self.model;
        let tolerance = self.precision.tolerance();
        'direction: for d in RAY_DIRECTIONS {
            let ray = Curve::Line {
                origin: point,
                direction: UnitVec3::new_normalize(Vec3::new(d[0], d[1], d[2])),
            };
            let mut crossings = 0usize;
            for &id in &self.faces {
                let face = model.face(id)?;
                let surface = model.surface(face.surface())?;
                let hits = match intersect_curve_surface(&ray, surface, tolerance)? {
                    CurveSurfaceIntersection::Points(hits) => hits,
                    CurveSurfaceIntersection::Coincident => continue 'direction,
                };
                for hit in hits {
                    if hit.t.abs() <= self.precision.default_tolerance {
                        // The point is on this face: no parity to take.
                        continue 'direction;
                    }
                    if hit.t < 0.0 {
                        continue;
                    }
                    match self.face_side(id, surface, hit.uv) {
                        Side::Inside if hit.tangent => continue 'direction,
                        Side::Inside => crossings += 1,
                        Side::Boundary => continue 'direction,
                        Side::Outside => {}
                    }
                }
            }
            return Ok(Some(crossings % 2 == 1));
        }
        Ok(None)
    }
}

/// The offsets a periodic parameter is tried at: nothing, and a period
/// either way. A direction with no period is tried once.
fn shifts(period: Option<f64>) -> impl Iterator<Item = f64> {
    let p = period.unwrap_or(0.0);
    [0.0, p, -p].into_iter().take(if p == 0.0 { 1 } else { 3 })
}
