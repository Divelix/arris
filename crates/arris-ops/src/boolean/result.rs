//! The boolean's result (ADR-0004): every face of both operands split
//! into pieces, each piece classified at a point inside it against the
//! other operand and kept or dropped by the selection table, the
//! survivors grouped into one shell and assembled through
//! `Builder::assemble` with every untouched entity kept by id, and the
//! provenance written from the pieces as they are made.

use std::collections::{BTreeMap, BTreeSet};

use arris_check::arris_topo::arris_geom::{GeomKind, SurfaceIntersection};
use arris_check::arris_topo::arris_math::{Interval, Precision};
use arris_check::arris_topo::builder::{
    Assembly, Builder, EdgeKey, EdgeSpec, FaceSpec, UseSpec, VertexKey, VertexSpec,
};
use arris_check::arris_topo::entity::{BodyKind, EdgeGeometry};
use arris_check::arris_topo::{
    Body, Curve2Id, CurveId, EdgeId, EntityId, Face as FaceHandle, FaceId, Model, NotFound,
    Orientation, Provenance, Shape, VertexId,
};
use arris_check::{Classification, classify_point};

use super::pieces::{ERef, PieceUse, SectionOnFace, SubEdge, VRef, split_face};
use super::{Interferences, VertexSource};
use crate::error::{Fault, OpError, Reason, SplitFault};

/// Which selection over the decomposition: one table, three
/// operations (`docs/01-architecture.md` §Operations).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Op {
    /// Keep what is outside the other operand.
    Fuse,
    /// Keep what is inside the other operand.
    Common,
    /// Keep the target outside the tool and the tool inside the target,
    /// reversed.
    Cut,
}

/// What happens to an operand's entities that survive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Policy {
    /// An untouched entity keeps its id; a piece is `Modified` from its
    /// parent.
    Reuse,
    /// Every entity is `Deleted`; a surviving piece is a new entity
    /// `Generated` from its parent — the tool of a `cut`.
    Regenerate,
}

impl Op {
    /// Whether a piece of operand `side` that is inside (`true`) or
    /// outside the other survives, and whether reversed
    /// (`docs/01-architecture.md` §Operations, the selection table).
    fn select(self, side: usize, inside: bool) -> Option<bool> {
        match (self, side, inside) {
            (Op::Fuse, _, false) | (Op::Common, _, true) | (Op::Cut, 0, false) => Some(false),
            (Op::Cut, 1, true) => Some(true),
            _ => None,
        }
    }

    fn policy(self, side: usize) -> Policy {
        match (self, side) {
            (Op::Cut, 1) => Policy::Regenerate,
            _ => Policy::Reuse,
        }
    }
}

/// A surviving piece, before assembly.
struct Kept {
    side: usize,
    /// The input face, with the operand's use of it.
    face: FaceHandle,
    /// The whole face, untouched and reusable by id.
    whole: bool,
    /// The result's use of it.
    orientation: Orientation,
    /// In the stored sense.
    loops: Vec<Vec<PieceUse>>,
}

fn forward(id: impl Into<EntityId>) -> Shape {
    Shape::new(id, Orientation::Forward)
}

/// A piece classified `On` the other operand, or a coincident face pair:
/// the flush case, which plan step 10 decides by the normals. Until
/// then it is the pair the kernel has no recipe for, named.
fn unsupported(m: &Model, face: FaceId, on: Shape) -> OpError {
    let kind = |s: Shape| -> GeomKind {
        match s.id {
            EntityId::Face(f) => m
                .face(f)
                .ok()
                .and_then(|f| m.surface(f.surface()).ok())
                .map_or(GeomKind::Point, |s| GeomKind::Surface(s.kind())),
            EntityId::Edge(e) => m
                .edge(e)
                .ok()
                .and_then(|e| e.curve())
                .and_then(|(c, _)| m.curve(c).ok())
                .map_or(GeomKind::Point, |c| GeomKind::Curve(c.kind())),
            EntityId::Vertex(_) | EntityId::Shell(_) | EntityId::Body(_) => GeomKind::Point,
        }
    };
    let a = forward(face);
    OpError::Unsupported {
        a: (kind(a), a),
        b: (kind(on), on),
    }
}

/// The whole build, over the pave model.
struct Build<'m> {
    m: &'m Model,
    precision: Precision,
    i: &'m Interferences,
    op: Op,
    bodies: [Body; 2],
    /// Each operand's vertices, ascending.
    vertices: [Vec<VertexId>; 2],
    /// Each operand's edges, in iteration order.
    edges: [Vec<EdgeId>; 2],
    /// Each operand's faces with their uses, in iteration order.
    faces: [Vec<FaceHandle>; 2],
    curve_ids: Vec<CurveId>,
    section_pcurves: Vec<[Curve2Id; 2]>,
    /// The vertex each section vertex is realised as.
    vref_of: Vec<VRef>,
    /// Operand vertices whose tolerance a section vertex raised.
    retolerated: BTreeMap<VertexId, f64>,
    /// Operand vertices merged into a section vertex that another
    /// operand vertex represents.
    merged_into: BTreeMap<VertexId, usize>,
    sub_edges: BTreeMap<EdgeId, Vec<SubEdge>>,
    touched: BTreeSet<EdgeId>,
    kept: Vec<Kept>,
    /// Per operand face, whether it was untouched and which of `kept` are
    /// its pieces.
    face_pieces: BTreeMap<FaceId, (bool, Vec<usize>)>,
}

impl<'m> Build<'m> {
    fn side_of_vertex(&self, v: VertexId) -> usize {
        usize::from(self.vertices[0].binary_search(&v).is_err())
    }

    fn not_found(&self, side: usize) -> OpError {
        OpError::NotFound(Shape::new(
            self.bodies[side].id,
            self.bodies[side].orientation,
        ))
    }

    /// A `Coincident` pair is the flush case (plan step 10).
    fn refuse_coincident(&self) -> Result<(), OpError> {
        for p in &self.i.pairs {
            if p.intersection == SurfaceIntersection::Coincident {
                return Err(unsupported(self.m, p.a, forward(p.b)));
            }
        }
        Ok(())
    }

    /// Every section vertex as a vertex of the result: its own, or the
    /// operand vertex it coincides with — one of a `Reuse` operand where
    /// there is a choice — re-tolerated when the merge grew past the
    /// stored tolerance.
    fn realise_vertices(&mut self) -> Result<(), OpError> {
        for v in &self.i.vertices {
            if v.existing.is_empty() {
                self.vref_of.push(VRef::Section(self.vref_of.len()));
                continue;
            }
            let rep = v
                .existing
                .iter()
                .copied()
                .find(|&x| self.op.policy(self.side_of_vertex(x)) == Policy::Reuse)
                .or_else(|| v.existing.first().copied())
                .ok_or_else(|| self.not_found(0))?;
            let stored = self
                .m
                .vertex(rep)
                .map_err(|_| self.not_found(self.side_of_vertex(rep)))?
                .tolerance();
            if v.tolerance > stored {
                self.retolerated.insert(rep, v.tolerance);
            }
            let k = self.vref_of.len();
            for &x in &v.existing {
                if x != rep {
                    self.merged_into.insert(x, k);
                }
            }
            self.vref_of.push(VRef::Existing(rep));
        }
        Ok(())
    }

    /// Every operand edge cut at its paves; an edge with a pave or a
    /// re-tolerated end is touched.
    fn sub_edges(&mut self) -> Result<(), OpError> {
        for side in 0..2 {
            for &e in &self.edges[side] {
                let edge = *self.m.edge(e).map_err(|_| self.not_found(side))?;
                let mut params = vec![edge.range().lo()];
                let mut vrefs = vec![VRef::Existing(edge.start())];
                let mut touched = self.retolerated.contains_key(&edge.start())
                    || self.retolerated.contains_key(&edge.end());
                if let Some(paves) = self.i.paves.get(&e) {
                    for p in paves {
                        params.push(p.t);
                        vrefs.push(self.vref_of[p.vertex]);
                        touched = true;
                    }
                }
                params.push(edge.range().hi());
                vrefs.push(VRef::Existing(edge.end()));
                let mut subs = Vec::with_capacity(params.len() - 1);
                for k in 0..params.len() - 1 {
                    let (lo, hi) = (params[k], params[k + 1]);
                    let empty =
                        || OpError::Internal(Fault::Split(SplitFault::EmptySubEdge { edge: e }));
                    if hi.partial_cmp(&lo) != Some(core::cmp::Ordering::Greater) {
                        return Err(empty());
                    }
                    subs.push(SubEdge {
                        range: Interval::new(lo, hi).map_err(|_| empty())?,
                        start: vrefs[k],
                        end: vrefs[k + 1],
                    });
                }
                self.sub_edges.insert(e, subs);
                if touched {
                    self.touched.insert(e);
                }
            }
        }
        Ok(())
    }

    /// The section edges as each face sees them.
    fn sections_by_face(&self) -> BTreeMap<FaceId, Vec<SectionOnFace>> {
        let mut on: BTreeMap<FaceId, Vec<SectionOnFace>> = BTreeMap::new();
        for (k, s) in self.i.sections.iter().enumerate() {
            let pair = &self.i.pairs[self.i.curves[s.curve].pair];
            for (side, face, other) in [(0, pair.a, pair.b), (1, pair.b, pair.a)] {
                on.entry(face).or_default().push(SectionOnFace {
                    index: k,
                    range: s.range,
                    start: self.vref_of[s.start],
                    end: self.vref_of[s.end],
                    pcurve: self.section_pcurves[k][side],
                    other,
                });
            }
        }
        on
    }

    /// Every face of both operands split, each piece classified against
    /// the other operand and kept by the selection table.
    fn select(&mut self) -> Result<(), OpError> {
        let on = self.sections_by_face();
        for side in 0..2 {
            let other = self.bodies[1 - side];
            let policy = self.op.policy(side);
            for f in self.faces[side].clone() {
                let sections = on.get(&f.id).map_or(&[][..], Vec::as_slice);
                let split = split_face(
                    self.m,
                    &self.precision,
                    f.id,
                    &self.sub_edges,
                    &self.touched,
                    sections,
                )?;
                let mut pieces = Vec::new();
                for piece in split.pieces {
                    let class = classify_point(self.m, other, piece.interior)
                        .map_err(|e| OpError::Internal(Fault::Classify(e)))?;
                    let inside = match class {
                        Classification::Inside => true,
                        Classification::Outside => false,
                        Classification::On(shape) => return Err(unsupported(self.m, f.id, shape)),
                    };
                    let Some(flip) = self.op.select(side, inside) else {
                        continue;
                    };
                    pieces.push(self.kept.len());
                    self.kept.push(Kept {
                        side,
                        face: f,
                        whole: split.untouched && policy == Policy::Reuse,
                        orientation: if flip {
                            f.orientation.flipped()
                        } else {
                            f.orientation
                        },
                        loops: piece.loops,
                    });
                }
                self.face_pieces.insert(f.id, (split.untouched, pieces));
            }
        }
        Ok(())
    }

    /// The surviving pieces grouped by shared edges: one group or the
    /// typed refusal.
    fn one_shell(&self) -> Result<(), OpError> {
        let entities = || {
            vec![
                Shape::new(self.bodies[0].id, self.bodies[0].orientation),
                Shape::new(self.bodies[1].id, self.bodies[1].orientation),
            ]
        };
        if self.kept.is_empty() {
            return Err(OpError::Degenerate {
                entities: entities(),
                reason: Reason::Empty,
            });
        }
        let mut parent: Vec<usize> = (0..self.kept.len()).collect();
        fn root(parent: &mut [usize], mut i: usize) -> usize {
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        let mut owner: BTreeMap<ERef, usize> = BTreeMap::new();
        for (k, piece) in self.kept.iter().enumerate() {
            for u in piece.loops.iter().flatten() {
                match owner.get(&u.edge) {
                    Some(&j) => {
                        let (a, b) = (root(&mut parent, k), root(&mut parent, j));
                        parent[a] = b;
                    }
                    None => {
                        owner.insert(u.edge, k);
                    }
                }
            }
        }
        let shells = (0..self.kept.len())
            .filter(|&k| root(&mut parent, k) == k)
            .count();
        if shells > 1 {
            return Err(OpError::Degenerate {
                entities: entities(),
                reason: Reason::MultiShell { shells },
            });
        }
        Ok(())
    }

    /// `true` when the vertex is appended rather than kept by id: a
    /// section vertex, a re-tolerated one, or any of a `Regenerate`
    /// operand.
    fn vertex_new(&self, v: VRef) -> bool {
        match v {
            VRef::Section(_) => true,
            VRef::Existing(id) => {
                self.retolerated.contains_key(&id)
                    || self.op.policy(self.side_of_vertex(id)) == Policy::Regenerate
            }
        }
    }

    /// `true` when the edge piece is appended rather than kept by id.
    fn edge_new(&self, e: EdgeId, side: usize) -> bool {
        self.touched.contains(&e) || self.op.policy(side) == Policy::Regenerate
    }
}

/// The result of a boolean over the pave model `i` under `op`.
pub(super) fn boolean(
    m: &mut Model,
    i: &Interferences,
    op: Op,
) -> Result<(Body, Provenance), OpError> {
    let bodies = [i.a, i.b];
    let of = |side: usize| {
        move |_: NotFound| OpError::NotFound(Shape::new(bodies[side].id, bodies[side].orientation))
    };
    let closures = [
        m.closure(i.a).map_err(of(0))?,
        m.closure(i.b).map_err(of(1))?,
    ];
    let vertices = [closures[0].vertices.clone(), closures[1].vertices.clone()];
    let mut edges: [Vec<EdgeId>; 2] = [Vec::new(), Vec::new()];
    let mut faces: [Vec<FaceHandle>; 2] = [Vec::new(), Vec::new()];
    for side in 0..2 {
        edges[side] = m
            .edges(bodies[side])
            .map_err(of(side))?
            .into_iter()
            .map(|e| e.id)
            .collect();
        faces[side] = m.faces(bodies[side]).map_err(of(side))?;
    }
    let precision = m.precision();

    m.transaction(|m| {
        // The section geometry, once.
        let curve_ids: Vec<CurveId> = i
            .curves
            .iter()
            .map(|c| m.add_curve(c.curve.clone()))
            .collect();
        let section_pcurves: Vec<[Curve2Id; 2]> = i
            .sections
            .iter()
            .map(|s| {
                [
                    m.add_curve2(s.pcurves[0].clone()),
                    m.add_curve2(s.pcurves[1].clone()),
                ]
            })
            .collect();

        let mut b = Build {
            m: &*m,
            precision,
            i,
            op,
            bodies,
            vertices: vertices.clone(),
            edges: edges.clone(),
            faces: faces.clone(),
            curve_ids,
            section_pcurves,
            vref_of: Vec::new(),
            retolerated: BTreeMap::new(),
            merged_into: BTreeMap::new(),
            sub_edges: BTreeMap::new(),
            touched: BTreeSet::new(),
            kept: Vec::new(),
            face_pieces: BTreeMap::new(),
        };
        b.refuse_coincident()?;
        b.realise_vertices()?;
        b.sub_edges()?;
        b.select()?;
        b.one_shell()?;
        let plan = b.assembly()?;
        let Build {
            face_pieces,
            vref_of,
            merged_into,
            sub_edges,
            ..
        } = b;

        let builder = Builder::assemble(m, precision.default_tolerance, plan.assembly)?;
        let built = builder.finish(m, BodyKind::Solid)?;

        // The output ids behind every reference.
        let mut out_vertex: BTreeMap<VRef, VertexId> = BTreeMap::new();
        for (v, &id) in plan.new_vertices.iter().zip(built.vertices.values()) {
            out_vertex.insert(*v, id);
        }
        let mut out_edge: BTreeMap<ERef, EdgeId> = BTreeMap::new();
        for (e, &id) in plan.new_edges.iter().zip(built.edges.values()) {
            out_edge.insert(*e, id);
        }
        let out_faces: Vec<FaceId> = built.faces.values().copied().collect();
        let vertex_id = |v: VRef| -> Option<VertexId> {
            match v {
                VRef::Existing(id) if plan.kept_vertices.contains(&id) => Some(id),
                other => out_vertex.get(&other).copied(),
            }
        };
        let edge_id = |e: ERef| -> Option<EdgeId> {
            match e {
                ERef::Sub { edge, index: 0 } if plan.kept_edges.contains(&edge) => Some(edge),
                other => out_edge.get(&other).copied(),
            }
        };

        let mut p = Provenance::new();
        for side in 0..2 {
            let policy = op.policy(side);
            let body = bodies[side];
            let record = |p: &mut Provenance, input: Shape, images: Vec<Shape>| match policy {
                Policy::Reuse => {
                    if images.is_empty() {
                        p.add_deleted(input);
                    } else if images != [input] {
                        for image in images {
                            p.add_modified(input, image);
                        }
                    }
                }
                Policy::Regenerate => {
                    p.add_deleted(input);
                    for image in images {
                        p.add_generated(input, image);
                    }
                }
            };
            for &v in &vertices[side] {
                let image = match merged_into.get(&v) {
                    Some(&k) => vertex_id(vref_of[k]),
                    None => vertex_id(VRef::Existing(v)),
                };
                record(&mut p, forward(v), image.into_iter().map(forward).collect());
            }
            for &e in &edges[side] {
                let n = sub_edges.get(&e).map_or(0, Vec::len);
                let images: Vec<Shape> = (0..n)
                    .filter_map(|index| edge_id(ERef::Sub { edge: e, index }))
                    .map(forward)
                    .collect();
                record(&mut p, forward(e), images);
            }
            for f in &faces[side] {
                let images: Vec<Shape> = face_pieces
                    .get(&f.id)
                    .map(|(_, pieces)| pieces.iter().map(|&k| forward(out_faces[k])).collect())
                    .unwrap_or_default();
                record(&mut p, forward(f.id), images);
            }
            let shells: Vec<Shape> = closures[side].shells.iter().map(|&s| forward(s)).collect();
            for s in shells {
                match policy {
                    Policy::Reuse => p.add_modified(s, forward(built.shell)),
                    Policy::Regenerate => p.add_deleted(s),
                }
            }
            match policy {
                Policy::Reuse => p.add_modified(forward(body.id), forward(built.body.id)),
                Policy::Regenerate => p.add_deleted(forward(body.id)),
            }
        }
        for (k, v) in i.vertices.iter().enumerate() {
            let VRef::Section(_) = vref_of[k] else {
                continue;
            };
            let Some(id) = out_vertex.get(&vref_of[k]) else {
                continue;
            };
            match v.source {
                VertexSource::Hits => {
                    for &h in &v.hits {
                        p.add_generated(forward(i.hits[h].edge), forward(*id));
                        p.add_generated(forward(i.hits[h].face), forward(*id));
                    }
                }
                VertexSource::CurveStart { pair, .. } => {
                    p.add_generated(forward(i.pairs[pair].a), forward(*id));
                    p.add_generated(forward(i.pairs[pair].b), forward(*id));
                }
            }
        }
        for (k, s) in i.sections.iter().enumerate() {
            let Some(&id) = out_edge.get(&ERef::Section(k)) else {
                continue;
            };
            let pair = &i.pairs[i.curves[s.curve].pair];
            p.add_generated(forward(pair.a), forward(id));
            p.add_generated(forward(pair.b), forward(id));
        }
        crate::verify(m, built.body)?;
        Ok((built.body, p))
    })
}

/// The assembly and the order its new slots were given in.
struct Plan {
    assembly: Assembly,
    /// The `VRef` behind each `VertexSpec::New`, in order.
    new_vertices: Vec<VRef>,
    /// The `ERef` behind each `EdgeSpec::New`, in order.
    new_edges: Vec<ERef>,
    /// Operand vertices kept by id.
    kept_vertices: BTreeSet<VertexId>,
    /// Operand edges kept by id.
    kept_edges: BTreeSet<EdgeId>,
}

impl Build<'_> {
    /// The assembly of the kept pieces: a vertex spec for every new
    /// vertex they reach, an edge spec for every new edge piece, a face
    /// spec per piece — `Keep` for a whole untouched face of a `Reuse`
    /// operand — in a deterministic order.
    fn assembly(&self) -> Result<Plan, OpError> {
        let m = self.m;
        let mut used_edges: BTreeSet<ERef> = BTreeSet::new();
        for piece in &self.kept {
            for u in piece.loops.iter().flatten() {
                used_edges.insert(u.edge);
            }
        }
        let mut used_vertices: BTreeSet<VRef> = BTreeSet::new();
        for &e in &used_edges {
            let (start, end) = match e {
                ERef::Sub { edge, index } => {
                    let s = &self.sub_edges[&edge][index];
                    (s.start, s.end)
                }
                ERef::Section(k) => {
                    let s = &self.i.sections[k];
                    (self.vref_of[s.start], self.vref_of[s.end])
                }
            };
            used_vertices.insert(start);
            used_vertices.insert(end);
        }

        let mut assembly = Assembly::default();
        let mut new_vertices = Vec::new();
        let mut kept_vertices = BTreeSet::new();
        let mut vkey: BTreeMap<VRef, VertexKey> = BTreeMap::new();
        // Section vertices first, ascending; then operand vertices that
        // are appended, ascending by id; the rest kept.
        let mut order: Vec<VRef> = used_vertices.iter().copied().collect();
        order.sort_by_key(|v| match *v {
            VRef::Section(k) => (0, k, None),
            VRef::Existing(id) => (1, 0, Some(id)),
        });
        for v in order {
            if self.vertex_new(v) {
                let (point, tolerance) = match v {
                    VRef::Section(k) => (self.i.vertices[k].point, self.i.vertices[k].tolerance),
                    VRef::Existing(id) => {
                        let stored = self
                            .m
                            .vertex(id)
                            .map_err(|_| self.not_found(self.side_of_vertex(id)))?;
                        (
                            stored.point(),
                            self.retolerated
                                .get(&id)
                                .copied()
                                .unwrap_or(stored.tolerance()),
                        )
                    }
                };
                assembly.vertices.push(VertexSpec::New { point, tolerance });
                vkey.insert(v, VertexKey::New(new_vertices.len()));
                new_vertices.push(v);
            } else if let VRef::Existing(id) = v {
                vkey.insert(v, VertexKey::Kept(id));
                kept_vertices.insert(id);
            }
        }

        let mut new_edges = Vec::new();
        let mut kept_edges = BTreeSet::new();
        let mut ekey: BTreeMap<ERef, EdgeKey> = BTreeMap::new();
        for side in 0..2 {
            for &e in &self.edges[side] {
                let edge = *m.edge(e).map_err(|_| self.not_found(side))?;
                let subs = &self.sub_edges[&e];
                if !self.edge_new(e, side) {
                    let whole = ERef::Sub { edge: e, index: 0 };
                    if used_edges.contains(&whole) {
                        ekey.insert(whole, EdgeKey::Kept(e));
                        kept_edges.insert(e);
                    }
                    continue;
                }
                for (index, s) in subs.iter().enumerate() {
                    let r = ERef::Sub { edge: e, index };
                    if !used_edges.contains(&r) {
                        continue;
                    }
                    let geometry = match edge.geometry() {
                        EdgeGeometry::Curve { curve, .. } => EdgeGeometry::Curve {
                            curve,
                            range: s.range,
                        },
                        EdgeGeometry::Degenerate { .. } => {
                            EdgeGeometry::Degenerate { range: s.range }
                        }
                    };
                    assembly.edges.push(EdgeSpec::New {
                        geometry,
                        start: vkey[&s.start],
                        end: vkey[&s.end],
                        tolerance: edge.tolerance(),
                    });
                    ekey.insert(r, EdgeKey::New(new_edges.len()));
                    new_edges.push(r);
                }
            }
        }
        for (k, s) in self.i.sections.iter().enumerate() {
            let r = ERef::Section(k);
            if !used_edges.contains(&r) {
                continue;
            }
            assembly.edges.push(EdgeSpec::New {
                geometry: EdgeGeometry::Curve {
                    curve: self.curve_ids[s.curve],
                    range: s.range,
                },
                start: vkey[&self.vref_of[s.start]],
                end: vkey[&self.vref_of[s.end]],
                tolerance: s.tolerance,
            });
            ekey.insert(r, EdgeKey::New(new_edges.len()));
            new_edges.push(r);
        }

        for piece in &self.kept {
            if piece.whole {
                assembly.faces.push(FaceSpec::Keep(FaceHandle::new(
                    piece.face.id,
                    piece.orientation,
                )));
                continue;
            }
            let entity = m
                .face(piece.face.id)
                .map_err(|_| self.not_found(piece.side))?;
            let loops = piece
                .loops
                .iter()
                .map(|l| {
                    let mut uses: Vec<UseSpec> = l
                        .iter()
                        .map(|u| UseSpec {
                            edge: ekey[&u.edge],
                            orientation: piece.orientation.compose(u.orientation),
                            pcurve: u.pcurve,
                        })
                        .collect();
                    if piece.orientation.is_reversed() {
                        uses.reverse();
                    }
                    uses
                })
                .collect();
            assembly.faces.push(FaceSpec::New {
                surface: entity.surface(),
                orientation: piece.orientation,
                loops,
                tolerance: entity.tolerance(),
            });
        }
        Ok(Plan {
            assembly,
            new_vertices,
            new_edges,
            kept_vertices,
            kept_edges,
        })
    }
}
