//! One solid's topology: a `MANIFOLD_SOLID_BREP` or a `BREP_WITH_VOIDS`
//! read into a body through [`Builder::assemble`] (ADR-0025).
//!
//! - **Vertices** come from `VERTEX_POINT`, one per instance.
//! - **Edges** come from `EDGE_CURVE`. An Arris edge runs along its
//!   curve, so one whose `same_sense` — composed with a `TRIMMED_CURVE`'s
//!   sense — runs against it swaps its two vertices, and every use of it
//!   is turned. The range is the parameters of the two vertices projected
//!   onto the curve, the upper one moved on by the period where it falls
//!   below; a closed edge on a periodic curve takes the full period from
//!   its vertex, and one on a closed curve that is not periodic the
//!   curve's whole domain.
//! - **Faces** take their orientation from the surface variant's normal
//!   against the file's ([`ReadSurface::reversed`]), the face's
//!   `same_sense`, an `ORIENTED_FACE`'s flag and the shell's. A bound's
//!   edges are walked in the `EDGE_LOOP`'s order with each
//!   `ORIENTED_EDGE`'s flag, the whole bound turned where its own flag is
//!   false; that walk keeps the face on its left about the file face's
//!   normal, and it is turned again where the shell (an
//!   `ORIENTED_CLOSED_SHELL` of orientation false, a void's) or an
//!   `ORIENTED_FACE` turns that normal, which gives the builder's
//!   effective walk.
//! - **Pcurves** are rebuilt, never read: [`pcurve_on`] for every edge on
//!   every face it bounds, each use's copy moved by whole periods so that
//!   it starts where the use before it in the loop ends. An edge used
//!   twice in one loop is a seam, and the walk around the loop puts its
//!   two copies a period apart. A loop is then moved by whole periods so
//!   that its box's centre lies in the period that starts at the widest
//!   loop's lower bound, so a hole lies inside its outer loop.
//!
//! Every entity is given the model's default tolerance; per-entity
//! tolerances from measured gaps are a later step's. The body is checked
//! at `Level::Fast` in every build (ADR-0025 §5), and a violation is the
//! file's: [`Refusal::Invalid`].

use std::collections::BTreeMap;

use arris_check::arris_topo::arris_geom::{Curve, Curve2, Surface, pcurve_on};
use arris_check::arris_topo::arris_math::{Aabb, Interval, Point2, Point3, Vec2};
use arris_check::arris_topo::builder::{
    Assembly, Builder, EdgeKey, EdgeSpec, FaceSpec, UseSpec, VertexKey, VertexSpec,
};
use arris_check::arris_topo::entity::{BodyKind, EdgeGeometry};
use arris_check::arris_topo::provenance::{FileEntity, Role};
use arris_check::arris_topo::{Body, CurveId, Model, Orientation, Provenance, Shape, SurfaceId};
use arris_check::{Level, check};

use super::Refusal;
use super::entities::{Args, describe, malformed};
use super::geometry::{Ball, Geometry, ReadCurve, ReadSurface};

/// A solid read: its body and the record naming the file entity each of
/// its entities was generated from.
pub(crate) struct Solid {
    pub(crate) body: Body,
    pub(crate) provenance: Provenance,
}

/// An `EDGE_CURVE` as the file gives it.
struct FileEdge {
    id: u64,
    /// The start and end vertices, as indices into the vertex list.
    start: usize,
    end: usize,
    curve: ReadCurve,
    /// The edge runs from its start to its end along the file curve's
    /// parametrisation.
    same_sense: bool,
}

/// One use of an edge by a loop: the edge's index and whether the loop
/// walks it from its file start to its file end.
#[derive(Clone, Copy)]
struct Step {
    edge: usize,
    forward: bool,
}

/// A face as the file gives it.
struct FileFace {
    id: u64,
    surface: ReadSurface,
    /// The file face's normal is its surface's.
    same_sense: bool,
    /// The face is turned by an `ORIENTED_FACE` or its shell, so its
    /// bounds' walk is turned with it.
    turned: bool,
    /// Each bound's walk, the face on its left about the file face's
    /// normal.
    bounds: Vec<Vec<Step>>,
}

/// A closed shell as the file gives it.
struct FileShell {
    id: u64,
    faces: Vec<FileFace>,
}

/// What the file says about one solid, before any geometry is resolved.
struct FileSolid {
    /// Every `VERTEX_POINT`, in first-use order.
    vertices: Vec<u64>,
    edges: Vec<FileEdge>,
    shells: Vec<FileShell>,
}

/// How deep an `ORIENTED_FACE` may nest before the reader stops: a file
/// nests none or one, and a cycle of references must end.
const ORIENTED_DEPTH: u8 = 4;

impl Geometry<'_> {
    /// Reads the solid `solid`, placement `instance`, into `model`.
    /// Errors: a refusal naming the file entity; the model is then as it
    /// was.
    pub(crate) fn solid(
        &self,
        model: &mut Model,
        solid: u64,
        instance: u32,
    ) -> Result<Solid, Refusal> {
        let file = self.file_solid(solid)?;
        model.transaction(|m| self.build(m, solid, instance, &file))
    }

    /// The shells, faces, bounds, edges and vertices of `solid`, with every
    /// curve and surface read but not yet bounded.
    fn file_solid(&self, solid: u64) -> Result<FileSolid, Refusal> {
        let instance = self.entities.get(solid, solid)?;
        let mut shells = Vec::new();
        if let Some(r) = instance.record("BREP_WITH_VOIDS") {
            let args = Args {
                id: solid,
                record: r,
            };
            shells.push(args.reference(1)?);
            shells.extend(args.references(2)?);
        } else if let Some(r) = instance.record("MANIFOLD_SOLID_BREP") {
            shells.push(
                Args {
                    id: solid,
                    record: r,
                }
                .reference(1)?,
            );
        } else {
            return Err(Refusal::Unsupported {
                entity: solid,
                name: describe(instance),
            });
        }
        let mut read = FileSolid {
            vertices: Vec::new(),
            edges: Vec::new(),
            shells: Vec::new(),
        };
        let mut vertex_of: BTreeMap<u64, usize> = BTreeMap::new();
        let mut edge_of: BTreeMap<u64, usize> = BTreeMap::new();
        for shell in shells {
            let (closed, turned) = self.closed_shell(solid, shell)?;
            let args = self.entities.record(shell, closed, "CLOSED_SHELL")?;
            let mut faces = Vec::new();
            for face in args.references(1)? {
                faces.push(self.face(
                    closed,
                    face,
                    turned,
                    ORIENTED_DEPTH,
                    &mut read,
                    &mut vertex_of,
                    &mut edge_of,
                )?);
            }
            read.shells.push(FileShell { id: closed, faces });
        }
        Ok(read)
    }

    /// The `CLOSED_SHELL` a solid's shell reference names, and whether an
    /// `ORIENTED_CLOSED_SHELL` turns it.
    fn closed_shell(&self, from: u64, id: u64) -> Result<(u64, bool), Refusal> {
        let instance = self.entities.get(from, id)?;
        if instance.record("CLOSED_SHELL").is_some() {
            return Ok((id, false));
        }
        if let Some(r) = instance.record("ORIENTED_CLOSED_SHELL") {
            let args = Args { id, record: r };
            let element = args.reference(2)?;
            let orientation = logical(&args, 3)?;
            self.entities.record(id, element, "CLOSED_SHELL")?;
            return Ok((element, !orientation));
        }
        Err(Refusal::Unsupported {
            entity: id,
            name: describe(instance),
        })
    }

    /// A face of a closed shell: an `ADVANCED_FACE` (or its supertype
    /// `FACE_SURFACE`), or an `ORIENTED_FACE` of one.
    #[allow(clippy::too_many_arguments)]
    fn face(
        &self,
        from: u64,
        id: u64,
        turned: bool,
        depth: u8,
        read: &mut FileSolid,
        vertex_of: &mut BTreeMap<u64, usize>,
        edge_of: &mut BTreeMap<u64, usize>,
    ) -> Result<FileFace, Refusal> {
        let instance = self.entities.get(from, id)?;
        if let Some(r) = instance.record("ORIENTED_FACE") {
            let args = Args { id, record: r };
            let element = args.reference(2)?;
            let orientation = logical(&args, 3)?;
            let Some(depth) = depth.checked_sub(1) else {
                return Err(malformed(id, "ORIENTED_FACE nested past any file's need"));
            };
            return self.face(
                id,
                element,
                turned ^ !orientation,
                depth,
                read,
                vertex_of,
                edge_of,
            );
        }
        let record = instance
            .record("ADVANCED_FACE")
            .or_else(|| instance.record("FACE_SURFACE"))
            .ok_or_else(|| Refusal::Unsupported {
                entity: id,
                name: describe(instance),
            })?;
        let args = Args { id, record };
        let surface = self.surface(id, args.reference(2)?)?;
        let same_sense = logical(&args, 3)?;
        let mut bounds = Vec::new();
        for bound in args.references(1)? {
            bounds.push(self.bound(id, bound, read, vertex_of, edge_of)?);
        }
        Ok(FileFace {
            id,
            surface,
            same_sense,
            turned,
            bounds,
        })
    }

    /// A `FACE_BOUND` or `FACE_OUTER_BOUND`: its `EDGE_LOOP`'s walk, turned
    /// where the bound's flag is false.
    fn bound(
        &self,
        face: u64,
        id: u64,
        read: &mut FileSolid,
        vertex_of: &mut BTreeMap<u64, usize>,
        edge_of: &mut BTreeMap<u64, usize>,
    ) -> Result<Vec<Step>, Refusal> {
        let instance = self.entities.get(face, id)?;
        let record = instance
            .record("FACE_OUTER_BOUND")
            .or_else(|| instance.record("FACE_BOUND"))
            .ok_or_else(|| {
                malformed(
                    id,
                    format!("is {}, where a face bound belongs", describe(instance)),
                )
            })?;
        let args = Args { id, record };
        let lp = args.reference(1)?;
        let orientation = logical(&args, 2)?;
        let lp_instance = self.entities.get(id, lp)?;
        let Some(edges) = lp_instance.record("EDGE_LOOP") else {
            // A VERTEX_LOOP is a pole's, which is a later step's; a
            // POLY_LOOP a faceted face's.
            return Err(Refusal::Unsupported {
                entity: lp,
                name: describe(lp_instance),
            });
        };
        let mut walk = Vec::new();
        for oe in (Args {
            id: lp,
            record: edges,
        })
        .references(1)?
        {
            let args = self.entities.record(lp, oe, "ORIENTED_EDGE")?;
            let element = args.reference(3)?;
            let forward = logical(&args, 4)?;
            let edge = match edge_of.get(&element) {
                Some(&i) => i,
                None => {
                    let e = self.edge(oe, element, read, vertex_of)?;
                    read.edges.push(e);
                    edge_of.insert(element, read.edges.len() - 1);
                    read.edges.len() - 1
                }
            };
            walk.push(Step { edge, forward });
        }
        if walk.is_empty() {
            return Err(malformed(lp, "an EDGE_LOOP of no edges"));
        }
        if !orientation {
            turn(&mut walk);
        }
        Ok(walk)
    }

    /// An `EDGE_CURVE`, its vertices entered in the solid's list.
    fn edge(
        &self,
        from: u64,
        id: u64,
        read: &mut FileSolid,
        vertex_of: &mut BTreeMap<u64, usize>,
    ) -> Result<FileEdge, Refusal> {
        let args = self.entities.record(from, id, "EDGE_CURVE")?;
        let mut vertex = |v: u64| -> Result<usize, Refusal> {
            self.entities.record(id, v, "VERTEX_POINT")?;
            Ok(*vertex_of.entry(v).or_insert_with(|| {
                read.vertices.push(v);
                read.vertices.len() - 1
            }))
        };
        let start = vertex(args.reference(1)?)?;
        let end = vertex(args.reference(2)?)?;
        let curve = self.curve(id, args.reference(3)?)?;
        let same_sense = logical(&args, 4)?;
        Ok(FileEdge {
            id,
            start,
            end,
            curve,
            same_sense,
        })
    }

    /// The body of `file`, appended to `model`.
    fn build(
        &self,
        model: &mut Model,
        solid: u64,
        instance: u32,
        file: &FileSolid,
    ) -> Result<Solid, Refusal> {
        let precision = model.precision();
        let tol = precision.tolerance();
        let tolerance = precision.default_tolerance;
        let file_entity = |id: u64| Role::File(FileEntity { id, instance });

        let points = file
            .vertices
            .iter()
            .map(|&v| {
                let args = self.entities.record(v, v, "VERTEX_POINT")?;
                self.units.point(&self.entities, v, args.reference(1)?)
            })
            .collect::<Result<Vec<Point3>, Refusal>>()?;

        // An unbounded curve is bounded by the vertices alone; every
        // surface by those and every edge.
        let mut part = ball_of(points.iter().map(|&p| Aabb::of_point(p)), tolerance);
        let mut curves: BTreeMap<u64, (Curve, CurveId)> = BTreeMap::new();
        let mut edges = Vec::with_capacity(file.edges.len());
        for e in &file.edges {
            let (curve, curve_id) = match curves.get(&e.curve.id) {
                Some(c) => c.clone(),
                None => {
                    let c = e.curve.resolve(&part)?;
                    let id = model.add_curve(c.clone());
                    curves.insert(e.curve.id, (c.clone(), id));
                    (c, id)
                }
            };
            // The Arris edge runs along its curve.
            let along = e.same_sense != e.curve.reversed;
            let (start, end) = if along {
                (e.start, e.end)
            } else {
                (e.end, e.start)
            };
            let range = edge_range(e.id, &curve, points[start], points[end], start == end)?;
            edges.push(Edge {
                geometry: curve,
                curve: curve_id,
                range,
                start,
                end,
                along,
            });
        }
        let boxes = edges.iter().filter_map(|e| e.geometry.bounds(e.range));
        part = ball_of(
            points.iter().map(|&p| Aabb::of_point(p)).chain(boxes),
            tolerance,
        );

        let mut surfaces: BTreeMap<u64, (Surface, SurfaceId)> = BTreeMap::new();
        let mut shells = Vec::with_capacity(file.shells.len());
        for shell in &file.shells {
            let mut faces = Vec::with_capacity(shell.faces.len());
            for face in &shell.faces {
                let (surface, surface_id) = match surfaces.get(&face.surface.id) {
                    Some(s) => s.clone(),
                    None => {
                        let s = face.surface.resolve(&part)?;
                        let id = model.add_surface(s.clone());
                        surfaces.insert(face.surface.id, (s.clone(), id));
                        (s, id)
                    }
                };
                // Outward is the file face's normal turned by the shell or
                // an ORIENTED_FACE, the file face's normal is the file
                // surface's turned where `same_sense` is false, and the
                // variant's normal is the file surface's turned by
                // `reversed`.
                let flipped = face.surface.reversed ^ !face.same_sense ^ face.turned;
                let mut pcurves: BTreeMap<usize, Curve2> = BTreeMap::new();
                let mut loops = Vec::with_capacity(face.bounds.len());
                for bound in &face.bounds {
                    let mut walk = bound.clone();
                    if face.turned {
                        turn(&mut walk);
                    }
                    let mut uses = Vec::with_capacity(walk.len());
                    for s in walk {
                        let edge = &edges[s.edge];
                        let pcurve = match pcurves.get(&s.edge) {
                            Some(p) => p.clone(),
                            None => {
                                let p = pcurve_on(&edge.geometry, edge.range, &surface, tol)
                                    .map_err(|e| Refusal::Pcurve {
                                        edge: file.edges[s.edge].id,
                                        face: face.id,
                                        what: e.to_string(),
                                    })?;
                                pcurves.insert(s.edge, p.clone());
                                p
                            }
                        };
                        let orientation = if s.forward == edge.along {
                            Orientation::Forward
                        } else {
                            Orientation::Reversed
                        };
                        uses.push(Use {
                            edge: s.edge,
                            orientation,
                            pcurve,
                            range: edge.range,
                        });
                    }
                    chain(&mut uses, surface.period());
                    loops.push(uses);
                }
                place_loops(&mut loops, surface.period());
                let loops = loops
                    .into_iter()
                    .map(|uses| {
                        uses.into_iter()
                            .map(|u| UseSpec {
                                edge: EdgeKey::New(u.edge),
                                orientation: u.orientation,
                                pcurve: model.add_curve2(u.pcurve),
                            })
                            .collect()
                    })
                    .collect();
                faces.push(FaceSpec::New {
                    surface: surface_id,
                    orientation: if flipped {
                        Orientation::Reversed
                    } else {
                        Orientation::Forward
                    },
                    loops,
                    tolerance,
                });
            }
            shells.push(faces);
        }

        let assembly = Assembly {
            vertices: points
                .iter()
                .map(|&point| VertexSpec::New { point, tolerance })
                .collect(),
            edges: edges
                .iter()
                .map(|e| EdgeSpec::New {
                    geometry: EdgeGeometry::Curve {
                        curve: e.curve,
                        range: e.range,
                    },
                    start: VertexKey::New(e.start),
                    end: VertexKey::New(e.end),
                    tolerance,
                })
                .collect(),
            shells,
        };
        let topology = |e: arris_check::arris_topo::builder::BuildError| Refusal::Topology {
            entity: solid,
            what: e.to_string(),
        };
        let (builder, slots) = Builder::assemble(model, tolerance, assembly).map_err(topology)?;
        let built = builder.finish(model, BodyKind::Solid).map_err(topology)?;

        let report = check(model, built.body, Level::Fast);
        if !report.is_ok() {
            return Err(Refusal::Invalid {
                entity: solid,
                report: Box::new(report),
            });
        }

        let mut provenance = Provenance::new();
        for (&v, slot) in file.vertices.iter().zip(&slots.vertices) {
            if let Some(&id) = built.vertices.get(slot) {
                provenance.add_generated(file_entity(v), Shape::new(id, Orientation::Forward));
            }
        }
        for (e, slot) in file.edges.iter().zip(&slots.edges) {
            if let Some(&id) = built.edges.get(slot) {
                provenance.add_generated(file_entity(e.id), Shape::new(id, Orientation::Forward));
            }
        }
        for (shell, shell_slots) in file.shells.iter().zip(&slots.faces) {
            for (face, slot) in shell.faces.iter().zip(shell_slots) {
                if let Some(&id) = built.faces.get(slot) {
                    provenance
                        .add_generated(file_entity(face.id), Shape::new(id, Orientation::Forward));
                }
            }
        }
        for (shell, &id) in file.shells.iter().zip(&built.shells) {
            provenance.add_generated(file_entity(shell.id), Shape::new(id, Orientation::Forward));
        }
        provenance.add_generated(file_entity(solid), built.body);
        Ok(Solid {
            body: built.body,
            provenance,
        })
    }
}

/// An edge with its geometry resolved.
struct Edge {
    geometry: Curve,
    curve: CurveId,
    range: Interval,
    /// The Arris edge's vertices: the file's, swapped where it runs
    /// against its curve.
    start: usize,
    end: usize,
    /// The file edge runs along its Arris curve.
    along: bool,
}

/// One use of a loop, with its pcurve moved to where the loop needs it.
struct Use {
    edge: usize,
    orientation: Orientation,
    pcurve: Curve2,
    range: Interval,
}

impl Use {
    /// Where the use starts and ends in (u, v).
    fn ends(&self) -> (Point2, Point2) {
        let (a, b) = (
            self.pcurve.point(self.range.lo()),
            self.pcurve.point(self.range.hi()),
        );
        match self.orientation {
            Orientation::Forward => (a, b),
            Orientation::Reversed => (b, a),
        }
    }
}

/// A walk turned: the other way round, each use the other way.
fn turn(walk: &mut [Step]) {
    walk.reverse();
    for s in walk {
        s.forward = !s.forward;
    }
}

/// A `.T.`/`.F.` parameter.
fn logical(args: &Args<'_>, i: usize) -> Result<bool, Refusal> {
    match args.enumeration(i)? {
        "T" => Ok(true),
        "F" => Ok(false),
        other => Err(args.malformed(format!(
            "{} parameter {} is .{other}., not .T. or .F.",
            args.record.name,
            i + 1
        ))),
    }
}

/// The ball around a union of boxes, its radius at least `floor` so that
/// a part of one point still has one.
fn ball_of(boxes: impl Iterator<Item = Aabb>, floor: f64) -> Ball {
    match boxes.reduce(Aabb::union) {
        Some(b) => {
            let [x, y, z] = b.center();
            Ball {
                centre: Point3::new(x, y, z),
                radius: (0.5 * b.diagonal()).max(floor),
            }
        }
        None => Ball {
            centre: Point3::origin(),
            radius: floor,
        },
    }
}

/// The range of the edge `id` on `curve` from `start` to `end`: the
/// vertices' parameters, the upper moved on by the period where it falls
/// below the lower. A closed edge — one vertex — takes the full period
/// from it, or the domain of a curve that closes without one.
fn edge_range(
    id: u64,
    curve: &Curve,
    start: Point3,
    end: Point3,
    closed: bool,
) -> Result<Interval, Refusal> {
    let degenerate = |what: String| Refusal::Degenerate { entity: id, what };
    let at = |p: Point3| -> Result<f64, Refusal> {
        let t = curve
            .project(p)
            .map_err(|e| {
                degenerate(format!(
                    "a vertex of the edge does not project onto its curve: {e}"
                ))
            })?
            .t;
        Ok(match curve.period() {
            Some(_) => t,
            None => curve.domain().clamp(t),
        })
    };
    let range = |lo: f64, mut hi: f64| {
        // `lo + p − lo` can round past `p`, which a range on a periodic
        // curve may not span (the checker's E1): the end comes down by
        // the units in the last place it went over.
        if let Some(p) = curve.period() {
            while hi - lo > p {
                hi = below(hi);
            }
        }
        Interval::new(lo, hi).map_err(|e| degenerate(e.to_string()))
    };
    let t0 = at(start)?;
    match (closed, curve.period()) {
        (true, Some(p)) => range(t0, t0 + p),
        (true, None) => {
            let d = curve.domain();
            if !d.is_bounded() {
                return Err(degenerate("a closed edge on an open curve".into()));
            }
            range(d.lo(), d.hi())
        }
        (false, period) => {
            let mut t1 = at(end)?;
            if let Some(p) = period {
                if t1 <= t0 {
                    t1 += p * ((t0 - t1) / p).floor() + p;
                }
            }
            if t1 <= t0 {
                return Err(degenerate(format!(
                    "the edge's end is at {t1} on its curve, not past its start at {t0}"
                )));
            }
            range(t0, t1)
        }
    }
}

/// The largest `f64` below the finite `x`: `f64::next_down`, stable from
/// Rust 1.86, where the workspace supports 1.85.
fn below(x: f64) -> f64 {
    if x == 0.0 {
        return -f64::from_bits(1);
    }
    let bits = x.to_bits();
    f64::from_bits(if x > 0.0 { bits - 1 } else { bits + 1 })
}

/// Moves each use's pcurve by whole periods so that it starts where the
/// use before it ends: the loop's walk made continuous in (u, v) across a
/// seam, and a seam's two uses put a period apart.
fn chain(uses: &mut [Use], period: [Option<f64>; 2]) {
    for i in 1..uses.len() {
        let (_, end) = uses[i - 1].ends();
        let (start, _) = uses[i].ends();
        let by = whole_periods(end - start, period);
        if by != Vec2::zeros() {
            uses[i].pcurve = uses[i].pcurve.translated(by);
        }
    }
}

/// `d` rounded to whole periods in each periodic parameter, zero in the
/// others.
fn whole_periods(d: Vec2, period: [Option<f64>; 2]) -> Vec2 {
    let round = |x: f64, p: Option<f64>| p.map_or(0.0, |p| p * (x / p).round());
    Vec2::new(round(d.x, period[0]), round(d.y, period[1]))
}

/// Moves each loop by whole periods so that the centre of its box in
/// (u, v) lies in the period starting at the lower bound of the widest
/// loop's box, parameter by parameter: a hole lands inside its outer
/// loop, whose box spans at most a period.
fn place_loops(loops: &mut [Vec<Use>], period: [Option<f64>; 2]) {
    let boxes: Vec<[[f64; 2]; 2]> = loops.iter().map(|l| loop_box(l)).collect();
    for k in 0..2 {
        let Some(p) = period[k] else { continue };
        let Some(widest) = (0..boxes.len()).max_by(|&a, &b| {
            let w = |i: usize| boxes[i][1][k] - boxes[i][0][k];
            w(a).total_cmp(&w(b)).then(b.cmp(&a))
        }) else {
            continue;
        };
        let lo = boxes[widest][0][k];
        for (i, l) in loops.iter_mut().enumerate() {
            let centre = 0.5 * (boxes[i][0][k] + boxes[i][1][k]);
            let shift = -p * ((centre - lo) / p).floor();
            if shift != 0.0 {
                let mut by = Vec2::zeros();
                by[k] = shift;
                for u in l.iter_mut() {
                    u.pcurve = u.pcurve.translated(by);
                }
            }
        }
    }
}

/// The box in (u, v) of a loop's uses' ends and midpoints.
fn loop_box(uses: &[Use]) -> [[f64; 2]; 2] {
    let mut b = [[f64::INFINITY; 2], [f64::NEG_INFINITY; 2]];
    for u in uses {
        let (s, e) = u.ends();
        let m = u.pcurve.point(u.range.midpoint());
        for p in [s, e, m] {
            for k in 0..2 {
                b[0][k] = b[0][k].min(p[k]);
                b[1][k] = b[1][k].max(p[k]);
            }
        }
    }
    b
}
