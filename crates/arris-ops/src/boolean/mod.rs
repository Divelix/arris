//! The boolean decomposition (ADR-0004): the pave model as a value.
//!
//! `fuse`, `common` and `cut` are three selections over one
//! decomposition of their two operands, and [`interferences`] is that
//! decomposition made visible — every face pair whose boxes overlap with
//! what its surfaces have in common, every point where an edge of one
//! operand pierces a face of the other, those points merged into section
//! vertices, the paves they put on every edge and every section curve,
//! and the section edges with a pcurve on each face. A boolean that goes
//! wrong is debugged here, at the pave, before any result exists (the
//! `inspect` skill).

mod faces;
mod pave;
mod pieces;
mod result;

use core::fmt;
use std::collections::BTreeMap;

use arris_check::arris_topo::arris_geom::{Curve, Curve2, SurfaceIntersection};
use arris_check::arris_topo::arris_math::{Interval, Point2, Point3};
use arris_check::arris_topo::{Body, EdgeId, FaceId, Model, Provenance, Shape, VertexId};

use crate::error::OpError;

/// One face pair whose bounding boxes overlap, with what their surfaces
/// have in common: the pair a boolean has to decide.
#[derive(Debug, Clone, PartialEq)]
pub struct FacePair {
    /// The face of the first operand.
    pub a: FaceId,
    /// The face of the second.
    pub b: FaceId,
    /// Their surfaces' intersection. A `Transversal` pair carries the
    /// section curves; a `Coincident` pair is decided by the arrangement
    /// of the two faces on one surface (plan step 10) and a `Tangent`
    /// one by the touch (step 11) — neither contributes a section edge
    /// here.
    pub intersection: SurfaceIntersection,
}

/// What a hit landed on within the face it pierced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Landing {
    /// Strictly inside the face's region in (u, v).
    Interior,
    /// On the face's boundary: within the tolerance of this edge or
    /// vertex of the face, the most specific one — an edge of one
    /// operand crossing an edge of the other, or passing through its
    /// vertex.
    Boundary(Shape),
}

/// One point where an edge of one operand meets a face of the other:
/// the edge's curve against the face's surface, kept when the parameter
/// is in the edge's range and the (u, v) is on the face.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeFaceHit {
    /// The edge.
    pub edge: EdgeId,
    /// The face.
    pub face: FaceId,
    /// The edge's parameter, in its range.
    pub t: f64,
    /// The face's (u, v), in the translate of the domain its loops are
    /// written in.
    pub uv: Point2,
    /// The edge's point at `t`.
    pub point: Point3,
    /// `true` when the edge touches the surface here without crossing
    /// it. A touch pierces nothing: it makes no section vertex and no
    /// pave, and is recorded for the tangent case (plan step 11).
    pub tangent: bool,
    /// Where on the face.
    pub landing: Landing,
    /// The end vertex of the edge the hit lies within the tolerance of,
    /// when it does: a vertex of this operand on a face of the other.
    pub at_vertex: Option<VertexId>,
    /// The section vertex the hit was merged into; `None` for a touch.
    pub vertex: Option<usize>,
}

/// Where a section vertex came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VertexSource {
    /// Hits merged: at least one.
    Hits,
    /// A closed section curve of the pair no hit paves, interior to both
    /// faces: its own point at the curve's parameter zero.
    CurveStart {
        /// The pair, an index into [`Interferences::pairs`].
        pair: usize,
        /// Which of the pair's curves.
        curve: usize,
    },
}

/// A point of the section, made once and shared: a pave on the edge
/// that pierced (every hit merged into it) and on every section curve it
/// lies on, so the pieces of the two operands meet exactly (ADR-0004).
#[derive(Debug, Clone, PartialEq)]
pub struct SectionVertex {
    /// The point: the first operand vertex it coincides with, else the
    /// first hit's.
    pub point: Point3,
    /// The largest tolerance of the entities whose hits it merges, plus
    /// the spread of the merged points about `point`
    /// (`docs/02-data-model.md` §Tolerances).
    pub tolerance: f64,
    /// The hits merged into it, ascending indices into
    /// [`Interferences::hits`].
    pub hits: Vec<usize>,
    /// The operand vertices it coincides with — a hit at an edge's end,
    /// or one landing on a vertex of the face — ascending. Usually none.
    pub existing: Vec<VertexId>,
    /// Where it came from.
    pub source: VertexSource,
}

/// A section vertex at a parameter of a curve: on an operand's edge, or
/// on a section curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pave {
    /// The curve's parameter; a periodic one in `[0, 2π)`.
    pub t: f64,
    /// The section vertex, an index into [`Interferences::vertices`].
    pub vertex: usize,
}

/// One curve two faces cross along, with the paves that cut it into
/// blocks and the blocks that survived as section edges.
#[derive(Debug, Clone, PartialEq)]
pub struct SectionCurve {
    /// The pair, an index into [`Interferences::pairs`].
    pub pair: usize,
    /// Which of the pair's `Transversal` curves.
    pub index: usize,
    /// The curve, as `intersect_surfaces` gave it.
    pub curve: Curve,
    /// The paves, ascending by `t`.
    pub paves: Vec<Pave>,
    /// The section edges on it, indices into [`Interferences::sections`].
    pub edges: Vec<usize>,
}

/// A block of a section curve interior to both faces: what becomes an
/// edge of the result, with its pcurve on each face already fitted.
#[derive(Debug, Clone, PartialEq)]
pub struct SectionEdge {
    /// The curve, an index into [`Interferences::curves`].
    pub curve: usize,
    /// The parameter range on it, between two consecutive paves; on a
    /// periodic curve with one pave, one whole turn from it.
    pub range: Interval,
    /// The section vertex at `range.lo()`.
    pub start: usize,
    /// The section vertex at `range.hi()`.
    pub end: usize,
    /// The larger of the two faces' tolerances, raised to the largest
    /// deviation of either pcurve's image from the curve.
    pub tolerance: f64,
    /// The pcurves on the pair's faces, `a` then `b`, same-parameter
    /// with `range`, each in the translate of the domain that face's
    /// loops are written in.
    pub pcurves: [Curve2; 2],
}

/// The pave model of two bodies (ADR-0004): what [`interferences`]
/// returns. Every list is in a deterministic order — pairs in the
/// operands' face iteration order, hits by `(edge id, t)`, vertices in
/// the order they were first hit — so two runs give the same value and
/// the same `Display`.
#[derive(Debug, Clone, PartialEq)]
pub struct Interferences {
    /// The first operand.
    pub a: Body,
    /// The second.
    pub b: Body,
    /// Every face pair whose boxes overlap, `a`'s faces outer.
    pub pairs: Vec<FacePair>,
    /// Every edge-on-face hit: `a`'s edges against `b`'s faces and `b`'s
    /// against `a`'s, ascending by `(edge id, t)`.
    pub hits: Vec<EdgeFaceHit>,
    /// The section vertices.
    pub vertices: Vec<SectionVertex>,
    /// The paves on every operand edge that has one, ascending by `t`,
    /// one per section vertex.
    pub paves: BTreeMap<EdgeId, Vec<Pave>>,
    /// The section curves of every `Transversal` pair, in pair order.
    pub curves: Vec<SectionCurve>,
    /// The section edges, in curve order and then along each curve.
    pub sections: Vec<SectionEdge>,
    /// Edges of one operand lying in the surface of a face of the other
    /// within their tolerances: the coincident case, decided at plan
    /// step 10; no hit is recorded for them here.
    pub coincident: Vec<(EdgeId, FaceId)>,
}

/// The pave model of `a` and `b`: the decomposition every boolean of the
/// two is a selection over, computed and returned without building
/// anything (a query: `&Model`, no body, no provenance).
///
/// Guarantees (ADR-0004): every face pair whose boxes overlap has been
/// intersected; every edge of each operand has been intersected with
/// every face of the other whose box it reaches, and a hit is kept
/// exactly when its parameter is in the edge's range and its (u, v) is
/// on the face; hits within tolerance of one another are one section
/// vertex, made once, whose tolerance follows the growth rule; a section
/// vertex paves every edge that hit it and every section curve it lies
/// on; a section curve's blocks between consecutive paves are kept when
/// their midpoint is inside both faces, and each kept block carries a
/// pcurve on each face that is same-parameter with the curve within the
/// edge's tolerance, translated into the copy of the domain the face's
/// loops are written in. The result is deterministic: the same value on
/// every platform.
///
/// Errors: [`OpError::InvalidInput`] when an operand fails the checker
/// (debug builds, and release with `paranoid`); [`OpError::NotFound`]
/// when one does not resolve; [`OpError::Unsupported`] naming the face
/// pair, or the edge and the face, the kernel has no closed form for —
/// a cone, a sphere, a torus or a NURBS operand, or two cylinders that
/// are not coaxial; [`OpError::Tolerance`] when a section vertex would
/// need a tolerance above the model's maximum; [`OpError::Internal`]
/// for a geometry query that failed on validated input or a section
/// edge crossing a seam without a pave.
///
/// ```
/// use arris_ops::boolean::interferences;
/// use arris_ops::{primitive_box, primitive_cylinder};
/// use arris_ops::arris_check::arris_topo::Model;
/// use arris_ops::arris_check::arris_topo::arris_math::{Axis, Point3};
///
/// let mut m = Model::default();
/// let (plate, _) = primitive_box(&mut m, Point3::origin(), Point3::new(40.0, 30.0, 10.0))?;
/// let axis = Axis::z_at(Point3::new(20.0, 15.0, -1.0));
/// let (hole, _) = primitive_cylinder(&mut m, axis, 4.0, 12.0)?;
/// let i = interferences(&m, plate, hole)?;
/// // The hole's seam pierces the top and the bottom: two section
/// // circles, each paved once, at the seam's hits.
/// assert_eq!(i.hits.len(), 2);
/// assert_eq!(i.sections.len(), 2);
/// assert!(i.curves.iter().all(|c| c.paves.len() == 1));
/// # Ok::<(), arris_ops::OpError>(())
/// ```
pub fn interferences(m: &Model, a: Body, b: Body) -> Result<Interferences, OpError> {
    crate::verify_input(m, a)?;
    crate::verify_input(m, b)?;
    pave::build(m, a, b)
}

/// `target` minus `tool`: the boolean difference of two solids whose
/// faces lie on planes and cylinders (`docs/01-architecture.md`
/// §Operations, ADR-0004).
///
/// Guarantees. The result is a `Solid` that passes the checker; the
/// decomposition is [`interferences`]'s, and the selection is the
/// table's: a piece of the target is kept when it is outside the tool, a
/// piece of the tool when it is inside the target, reversed. Every face
/// is split in its own (u, v) through its pcurves and the section
/// edges', each piece classified by `arris_check::classify_point` at a
/// point strictly inside it. Every entity of the target the operation
/// did not touch keeps its id — a face whose loops changed at all, even
/// only by a split edge, is a new face `Modified` from the old, and so
/// is a re-tolerated vertex and everything at it; a split edge is
/// `Modified` into its surviving pieces; whatever has no piece left is
/// `Deleted`. Every entity of the tool is `Deleted`, and a piece of it
/// that survives is `Generated` from the tool entity it is a piece of —
/// the hole's wall from the tool's wall (`docs/02-data-model.md`
/// §Provenance). A section vertex is `Generated` from the edge and the
/// face of every hit it merges, a section edge from both faces of its
/// pair; the result's shell and body are `Modified` from the target's.
/// Tolerances follow the growth rule: a section vertex's is the largest
/// of what it merges plus their spread, a section edge's the larger of
/// its faces' raised to the pcurves' residual, a piece keeps its
/// parent's. The result is deterministic: the same ids on every run and
/// every platform.
///
/// Errors, the model untouched on each: [`OpError::InvalidInput`] and
/// [`OpError::NotFound`] as every operation; [`OpError::Unsupported`]
/// naming the pair for a surface pair or an edge–face pair with no
/// closed form, and — until plan step 10 — for a coincident face pair
/// or a piece lying on the other operand's boundary, the flush case;
/// [`OpError::Degenerate`] with [`crate::Reason::Empty`] when nothing
/// survives (the target inside the tool), [`crate::Reason::MultiShell`]
/// when the survivors make more than one shell (a tool that splits its
/// target, an enclosed cavity), [`crate::Reason::TangentContact`] when
/// a section edge is tangent to a loop edge at a vertex;
/// [`OpError::Tolerance`] when a section vertex or edge would exceed the
/// model's maximum; [`OpError::Internal`] for a kernel bug the operation
/// caught — an arrangement that is not a subdivision, a point that
/// could not be classified, the builder refusing the assembly.
///
/// ```
/// use arris_ops::{cut, primitive_box, primitive_cylinder};
/// use arris_ops::measure::mass_properties;
/// use arris_ops::arris_check::arris_topo::Model;
/// use arris_ops::arris_check::arris_topo::arris_math::{Axis, Point3};
/// use core::f64::consts::PI;
///
/// let mut m = Model::default();
/// let (plate, _) = primitive_box(&mut m, Point3::origin(), Point3::new(40.0, 30.0, 10.0))?;
/// let axis = Axis::z_at(Point3::new(20.0, 15.0, -1.0));
/// let (hole, _) = primitive_cylinder(&mut m, axis, 4.0, 12.0)?;
/// let (plate_with_hole, provenance) = cut(&mut m, plate, hole)?;
/// assert_eq!(m.faces(plate_with_hole)?.len(), 7);
/// let volume = mass_properties(&m, plate_with_hole)?.volume;
/// assert!((volume - (12000.0 - PI * 16.0 * 10.0)).abs() < 1e-9 * 12000.0);
/// // The four side faces are kept: not a word about them in the record.
/// assert_eq!(provenance.outputs().len(), 10);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn cut(m: &mut Model, target: Body, tool: Body) -> Result<(Body, Provenance), OpError> {
    crate::verify_input(m, target)?;
    crate::verify_input(m, tool)?;
    let i = pave::build(m, target, tool)?;
    result::boolean(m, &i, result::Op::Cut)
}

/// `a` ∪ `b`: the boolean union of two solids whose faces lie on planes
/// and cylinders (`docs/01-architecture.md` §Operations, ADR-0004).
///
/// Guarantees. The result is a `Solid` that passes the checker; the
/// decomposition is [`interferences`]'s, and the selection is the
/// table's: a piece of either operand is kept when it is outside the
/// other, in the operand's own orientation. Every face is split in its
/// own (u, v) through its pcurves and the section edges', each piece
/// classified by `arris_check::classify_point` at a point strictly
/// inside it. Entities are reused from both operands: whatever the
/// operation did not touch keeps its id — a face whose loops changed at
/// all, even only by a split edge, is a new face `Modified` from the
/// old, and so is a re-tolerated vertex and everything at it; a split
/// edge is `Modified` into its surviving pieces; whatever has no piece
/// left is `Deleted`. A section vertex is `Generated` from the edge and
/// the face of every hit it merges, a section edge from both faces of
/// its pair; the result's shell and body are `Modified` from both
/// operands' (`docs/02-data-model.md` §Provenance). Tolerances follow
/// the growth rule, as [`cut`]. The result is deterministic: the same
/// ids on every run and every platform.
///
/// Errors, the model untouched on each: as [`cut`]'s, with
/// [`crate::Reason::MultiShell`] where a `cut` would rarely reach it —
/// two operands that do not overlap make two shells, which is M4's
/// "out" (`docs/plans/m4-booleans.md` `⚠ OPEN` 2).
///
/// ```
/// use arris_ops::{fuse, primitive_box, primitive_cylinder};
/// use arris_ops::measure::mass_properties;
/// use arris_ops::arris_check::arris_topo::Model;
/// use arris_ops::arris_check::arris_topo::arris_math::{Axis, Point3};
/// use core::f64::consts::PI;
///
/// let mut m = Model::default();
/// let (plate, _) = primitive_box(&mut m, Point3::origin(), Point3::new(40.0, 30.0, 10.0))?;
/// let axis = Axis::z_at(Point3::new(20.0, 15.0, 5.0));
/// let (boss, _) = primitive_cylinder(&mut m, axis, 4.0, 15.0)?;
/// let (plate_with_boss, provenance) = fuse(&mut m, plate, boss)?;
/// // The boss's wall crosses the top face; its bottom cap is swallowed.
/// assert_eq!(m.faces(plate_with_boss)?.len(), 8);
/// let volume = mass_properties(&m, plate_with_boss)?.volume;
/// assert!((volume - (12000.0 + PI * 16.0 * 10.0)).abs() < 1e-9 * 12000.0);
/// // The plate's four sides and bottom and the boss's top cap are kept:
/// // not a word about them in the record.
/// assert_eq!(provenance.outputs().len(), 7);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn fuse(m: &mut Model, a: Body, b: Body) -> Result<(Body, Provenance), OpError> {
    crate::verify_input(m, a)?;
    crate::verify_input(m, b)?;
    let i = pave::build(m, a, b)?;
    result::boolean(m, &i, result::Op::Fuse)
}

/// `a` ∩ `b`: the boolean intersection of two solids whose faces lie on
/// planes and cylinders (`docs/01-architecture.md` §Operations,
/// ADR-0004).
///
/// Guarantees. As [`fuse`], with the other selection: a piece of either
/// operand is kept when it is inside the other, in the operand's own
/// orientation. Entities are reused from both operands and the
/// provenance is written the same way; the result's shell and body are
/// `Modified` from both operands'.
///
/// Errors, the model untouched on each: as [`cut`]'s, with
/// [`crate::Reason::Empty`] where two operands share no material — the
/// common of disjoint solids.
///
/// ```
/// use arris_ops::{common, primitive_box};
/// use arris_ops::measure::mass_properties;
/// use arris_ops::arris_check::arris_topo::Model;
/// use arris_ops::arris_check::arris_topo::arris_math::Point3;
///
/// let mut m = Model::default();
/// let (a, _) = primitive_box(&mut m, Point3::new(-1.0, -1.0, -1.0), Point3::new(1.0, 1.0, 1.0))?;
/// let (b, _) = primitive_box(&mut m, Point3::origin(), Point3::new(2.0, 2.0, 2.0))?;
/// let (unit_cube, _) = common(&mut m, a, b)?;
/// assert_eq!(m.faces(unit_cube)?.len(), 6);
/// assert!((mass_properties(&m, unit_cube)?.volume - 1.0).abs() < 1e-12);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn common(m: &mut Model, a: Body, b: Body) -> Result<(Body, Provenance), OpError> {
    crate::verify_input(m, a)?;
    crate::verify_input(m, b)?;
    let i = pave::build(m, a, b)?;
    result::boolean(m, &i, result::Op::Common)
}

/// A number as the dump writes it: the shortest decimal that round-trips.
fn num(x: f64) -> String {
    format!("{x}")
}

fn point3(p: Point3) -> String {
    format!("({}, {}, {})", num(p.x), num(p.y), num(p.z))
}

fn point2(p: Point2) -> String {
    format!("({}, {})", num(p.x), num(p.y))
}

/// A curve in one line: its kind and the numbers that place it.
fn curve(c: &Curve) -> String {
    match c {
        Curve::Line { origin, direction } => format!(
            "line origin {} direction ({}, {}, {})",
            point3(*origin),
            num(direction.x),
            num(direction.y),
            num(direction.z)
        ),
        Curve::Circle { frame, radius } => format!(
            "circle centre {} radius {}",
            point3(frame.origin()),
            num(*radius)
        ),
        Curve::Ellipse {
            frame,
            major_radius,
            minor_radius,
        } => format!(
            "ellipse centre {} major {} minor {}",
            point3(frame.origin()),
            num(*major_radius),
            num(*minor_radius)
        ),
        Curve::Nurbs(n) => format!(
            "nurbs degree {} points {}",
            n.degree(),
            n.control_points().len()
        ),
    }
}

/// A pcurve in one line.
fn curve2(c: &Curve2) -> String {
    match c {
        Curve2::Line { origin, direction } => format!(
            "line origin {} direction ({}, {})",
            point2(*origin),
            num(direction.x),
            num(direction.y)
        ),
        Curve2::Circle { frame, radius } => format!(
            "circle centre {} radius {}",
            point2(frame.origin()),
            num(*radius)
        ),
        Curve2::Ellipse {
            frame,
            major_radius,
            minor_radius,
        } => format!(
            "ellipse centre {} major {} minor {}",
            point2(frame.origin()),
            num(*major_radius),
            num(*minor_radius)
        ),
        Curve2::Nurbs(n) => format!(
            "nurbs degree {} points {} u [{}, {}]",
            n.degree(),
            n.control_points().len(),
            num(n
                .control_points()
                .iter()
                .map(|p| p.x)
                .fold(f64::INFINITY, f64::min)),
            num(n
                .control_points()
                .iter()
                .map(|p| p.x)
                .fold(f64::NEG_INFINITY, f64::max))
        ),
    }
}

impl fmt::Display for Interferences {
    /// The whole model, one entity per line: the pairs with their
    /// intersection kind and curves, the hits, the vertices, the paves
    /// per edge, the section curves with their paves, the section edges
    /// with their pcurves, the coincident edges. Deterministic, so two
    /// runs are compared by their text.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "interferences {} vs {}", self.a, self.b)?;
        writeln!(f, "pairs {}", self.pairs.len())?;
        for (i, p) in self.pairs.iter().enumerate() {
            match &p.intersection {
                SurfaceIntersection::Empty => writeln!(f, "  p{i} {} x {}: empty", p.a, p.b)?,
                SurfaceIntersection::Coincident => {
                    writeln!(f, "  p{i} {} x {}: coincident", p.a, p.b)?;
                }
                SurfaceIntersection::Transversal(curves) => {
                    writeln!(f, "  p{i} {} x {}: transversal", p.a, p.b)?;
                    for c in curves {
                        writeln!(f, "    {}", curve(c))?;
                    }
                }
                SurfaceIntersection::Tangent(curves) => {
                    writeln!(f, "  p{i} {} x {}: tangent", p.a, p.b)?;
                    for c in curves {
                        writeln!(f, "    {}", curve(c))?;
                    }
                }
            }
        }
        writeln!(f, "hits {}", self.hits.len())?;
        for (i, h) in self.hits.iter().enumerate() {
            let landing = match h.landing {
                Landing::Interior => "interior".to_string(),
                Landing::Boundary(s) => format!("boundary {s}"),
            };
            let vertex = match h.vertex {
                Some(v) => format!("v{v}"),
                None => "-".to_string(),
            };
            writeln!(
                f,
                "  h{i} {} t {} on {} uv {} at {} {}{}{} -> {vertex}",
                h.edge,
                num(h.t),
                h.face,
                point2(h.uv),
                point3(h.point),
                landing,
                if h.tangent { " tangent" } else { "" },
                match h.at_vertex {
                    Some(v) => format!(" at {v}"),
                    None => String::new(),
                }
            )?;
        }
        writeln!(f, "vertices {}", self.vertices.len())?;
        for (i, v) in self.vertices.iter().enumerate() {
            let hits: Vec<String> = v.hits.iter().map(|h| format!("h{h}")).collect();
            let existing: Vec<String> = v.existing.iter().map(|e| e.to_string()).collect();
            let source = match v.source {
                VertexSource::Hits => String::new(),
                VertexSource::CurveStart { pair, curve } => {
                    format!(" start of curve {curve} of p{pair}")
                }
            };
            writeln!(
                f,
                "  v{i} {} tol {} hits [{}] existing [{}]{source}",
                point3(v.point),
                num(v.tolerance),
                hits.join(" "),
                existing.join(" ")
            )?;
        }
        writeln!(f, "paves")?;
        for (edge, paves) in &self.paves {
            let list: Vec<String> = paves
                .iter()
                .map(|p| format!("t {} v{}", num(p.t), p.vertex))
                .collect();
            writeln!(f, "  {edge}: {}", list.join(", "))?;
        }
        writeln!(f, "section curves {}", self.curves.len())?;
        for (i, c) in self.curves.iter().enumerate() {
            let paves: Vec<String> = c
                .paves
                .iter()
                .map(|p| format!("t {} v{}", num(p.t), p.vertex))
                .collect();
            let edges: Vec<String> = c.edges.iter().map(|e| format!("s{e}")).collect();
            writeln!(
                f,
                "  c{i} p{} {}: paves [{}] edges [{}]",
                c.pair,
                curve(&c.curve),
                paves.join(", "),
                edges.join(" ")
            )?;
        }
        writeln!(f, "section edges {}", self.sections.len())?;
        for (i, s) in self.sections.iter().enumerate() {
            writeln!(
                f,
                "  s{i} c{} [{}, {}] v{} -> v{} tol {}",
                s.curve,
                num(s.range.lo()),
                num(s.range.hi()),
                s.start,
                s.end,
                num(s.tolerance)
            )?;
            writeln!(f, "    on a: {}", curve2(&s.pcurves[0]))?;
            writeln!(f, "    on b: {}", curve2(&s.pcurves[1]))?;
        }
        writeln!(f, "coincident {}", self.coincident.len())?;
        for (e, face) in &self.coincident {
            writeln!(f, "  {e} in {face}")?;
        }
        Ok(())
    }
}
