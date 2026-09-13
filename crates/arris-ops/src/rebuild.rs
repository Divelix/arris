//! The keep/regenerate assembly and its provenance writer (ADR-0004): a
//! surviving piece is assembled fresh unless it is a whole face
//! untouched under a `Reuse` operand, in which case it is kept by id;
//! every touched entity's provenance is written from the pieces as they
//! are made. `boolean::result` is the first producer of pieces; a blend
//! will be another.

use std::collections::{BTreeMap, BTreeSet};

use arris_check::arris_topo::builder::{
    Assembly, EdgeKey, EdgeSpec, FaceSpec, UseSpec, VertexKey, VertexSpec, effective_uses,
};
use arris_check::arris_topo::entity::EdgeGeometry;
use arris_check::arris_topo::{
    Body, CurveId, EdgeId, EntityId, Face as FaceHandle, FaceId, Model, Orientation, Provenance,
    Shape, ShellId, VertexId,
};

use crate::boolean::Interferences;
use crate::boolean::pieces::{ERef, PieceUse, SubEdge, VRef};
use crate::error::OpError;

/// A `Forward` handle to `id`, as a [`Shape`].
pub(crate) fn forward(id: impl Into<EntityId>) -> Shape {
    Shape::new(id, Orientation::Forward)
}

/// What happens to an operand's entities that survive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Policy {
    /// An untouched entity keeps its id; a piece is `Modified` from its
    /// parent.
    Reuse,
    /// Every entity is `Deleted`; a surviving piece is a new entity
    /// `Generated` from its parent — the tool of a `cut`.
    Regenerate,
}

/// A surviving piece, before assembly.
pub(crate) struct Kept {
    /// Which operand it came from.
    pub side: usize,
    /// The input face, with the operand's use of it.
    pub face: FaceHandle,
    /// The whole face, untouched and reusable by id.
    pub whole: bool,
    /// The result's use of it.
    pub orientation: Orientation,
    /// In the stored sense.
    pub loops: Vec<Vec<PieceUse>>,
    /// The coincident face of the other operand the piece lies on and
    /// stands in for, when it does.
    pub stands_for: Option<FaceId>,
}

/// The assembly and the order its new slots were given in.
pub(crate) struct Plan {
    pub assembly: Assembly,
    /// The kept piece behind each face spec, shell by shell in order.
    pub faces: Vec<usize>,
    /// The `VRef` behind each `VertexSpec::New`, in order.
    pub new_vertices: Vec<VRef>,
    /// The `ERef` behind each `EdgeSpec::New`, in order.
    pub new_edges: Vec<ERef>,
    /// Operand vertices kept by id.
    pub kept_vertices: BTreeSet<VertexId>,
    /// Operand edges kept by id.
    pub kept_edges: BTreeSet<EdgeId>,
}

/// The assembly of the kept `pieces`: a vertex spec for every new vertex
/// they reach, an edge spec for every new edge piece, a face spec per
/// piece — `Keep` for a whole untouched face of a `Reuse` operand —
/// shell by shell in `shells`' order, in a deterministic order.
#[allow(clippy::too_many_arguments)]
pub(crate) fn assembly(
    m: &Model,
    policy: [Policy; 2],
    vertices: &[Vec<VertexId>; 2],
    edges: &[Vec<EdgeId>; 2],
    sub_edges: &BTreeMap<EdgeId, Vec<SubEdge>>,
    touched: &BTreeSet<EdgeId>,
    retolerated: &BTreeMap<VertexId, f64>,
    edge_tolerance: &BTreeMap<ERef, f64>,
    i: &Interferences,
    curve_ids: &[CurveId],
    vref_of: &[VRef],
    pieces: &[Kept],
    shells: &[Vec<usize>],
) -> Result<Plan, OpError> {
    let side_of_vertex = |v: VertexId| usize::from(vertices[0].binary_search(&v).is_err());
    let vertex_new = |v: VRef| match v {
        VRef::Section(_) => true,
        VRef::Existing(id) => {
            retolerated.contains_key(&id) || policy[side_of_vertex(id)] == Policy::Regenerate
        }
    };
    let edge_new =
        |e: EdgeId, side: usize| touched.contains(&e) || policy[side] == Policy::Regenerate;

    let mut used_edges: BTreeSet<ERef> = BTreeSet::new();
    for piece in pieces {
        for u in piece.loops.iter().flatten() {
            used_edges.insert(u.edge);
        }
    }
    let mut used_vertices: BTreeSet<VRef> = BTreeSet::new();
    for &e in &used_edges {
        let (start, end) = match e {
            ERef::Sub { edge, index } => {
                let s = &sub_edges[&edge][index];
                (s.start, s.end)
            }
            ERef::Section(k) => {
                let s = &i.sections[k];
                (vref_of[s.start], vref_of[s.end])
            }
        };
        used_vertices.insert(start);
        used_vertices.insert(end);
    }

    let mut assembly = Assembly::default();
    let mut new_vertices = Vec::new();
    let mut kept_vertices = BTreeSet::new();
    let mut vkey: BTreeMap<VRef, VertexKey> = BTreeMap::new();
    // Section vertices first, ascending; then operand vertices that are
    // appended, ascending by id; the rest kept.
    let mut order: Vec<VRef> = used_vertices.iter().copied().collect();
    order.sort_by_key(|v| match *v {
        VRef::Section(k) => (0, k, None),
        VRef::Existing(id) => (1, 0, Some(id)),
    });
    for v in order {
        if vertex_new(v) {
            let (point, tolerance) = match v {
                VRef::Section(k) => (i.vertices[k].point, i.vertices[k].tolerance),
                VRef::Existing(id) => {
                    let stored = m.vertex(id)?;
                    (
                        stored.point(),
                        retolerated.get(&id).copied().unwrap_or(stored.tolerance()),
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
    for (side, side_edges) in edges.iter().enumerate() {
        for &e in side_edges {
            let edge = *m.edge(e)?;
            let subs = &sub_edges[&e];
            if !edge_new(e, side) {
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
                    EdgeGeometry::Degenerate { .. } => EdgeGeometry::Degenerate { range: s.range },
                };
                // The edge's own tolerance whenever no image or common
                // block raised this piece's — never a literal, since the
                // entity's tolerance is always the floor.
                let tolerance = edge_tolerance
                    .get(&r)
                    .map_or(edge.tolerance(), |t| t.max(edge.tolerance()));
                assembly.edges.push(EdgeSpec::New {
                    geometry,
                    start: vkey[&s.start],
                    end: vkey[&s.end],
                    tolerance,
                });
                ekey.insert(r, EdgeKey::New(new_edges.len()));
                new_edges.push(r);
            }
        }
    }
    for (k, s) in i.sections.iter().enumerate() {
        let r = ERef::Section(k);
        if !used_edges.contains(&r) {
            continue;
        }
        assembly.edges.push(EdgeSpec::New {
            geometry: EdgeGeometry::Curve {
                curve: curve_ids[s.curve],
                range: s.range,
            },
            start: vkey[&vref_of[s.start]],
            end: vkey[&vref_of[s.end]],
            tolerance: s.tolerance,
        });
        ekey.insert(r, EdgeKey::New(new_edges.len()));
        new_edges.push(r);
    }

    let mut face_pieces = Vec::with_capacity(pieces.len());
    for shell in shells {
        let mut faces = Vec::with_capacity(shell.len());
        for &k in shell {
            let Some(piece) = pieces.get(k) else {
                continue;
            };
            face_pieces.push(k);
            if piece.whole {
                faces.push(FaceSpec::Keep(FaceHandle::new(
                    piece.face.id,
                    piece.orientation,
                )));
                continue;
            }
            let entity = m.face(piece.face.id)?;
            let loops = piece
                .loops
                .iter()
                .map(|l| {
                    effective_uses(
                        piece.orientation,
                        l.iter().map(|u| (ekey[&u.edge], u.orientation, u.pcurve)),
                    )
                    .into_iter()
                    .map(|(edge, orientation, pcurve)| UseSpec {
                        edge,
                        orientation,
                        pcurve,
                    })
                    .collect()
                })
                .collect();
            faces.push(FaceSpec::New {
                surface: entity.surface(),
                orientation: piece.orientation,
                loops,
                tolerance: entity.tolerance(),
            });
        }
        assembly.shells.push(faces);
    }
    Ok(Plan {
        assembly,
        faces: face_pieces,
        new_vertices,
        new_edges,
        kept_vertices,
        kept_edges,
    })
}

/// One operand's entities as the provenance writer needs them.
pub(crate) struct OperandWrite<'a> {
    /// The operand body.
    pub body: Body,
    /// Its policy.
    pub policy: Policy,
    /// Its vertices, ascending.
    pub vertices: &'a [VertexId],
    /// Its edges, in iteration order.
    pub edges: &'a [EdgeId],
    /// Its faces with their uses, in iteration order.
    pub faces: &'a [FaceHandle],
    /// Its shells, before the operation.
    pub shells: &'a [ShellId],
    /// The shell each of its faces belongs to.
    pub shell_of: &'a BTreeMap<FaceId, ShellId>,
}

/// The provenance of a keep/regenerate assembly: every operand entity
/// recorded deleted, modified into its images, or generated into them
/// by `operand.policy`, every result shell related to the operand
/// shells its pieces came from, and a piece standing for a coincident
/// face `Generated` from it. Callers with their own reasons for a new
/// vertex or edge — the boolean's section geometry — add those records
/// on top of the one this returns.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_provenance(
    operands: [OperandWrite<'_>; 2],
    built_body: Body,
    sub_edges: &BTreeMap<EdgeId, Vec<SubEdge>>,
    merged_into: &BTreeMap<VertexId, usize>,
    vref_of: &[VRef],
    vertex_id: impl Fn(VRef) -> Option<VertexId>,
    edge_id: impl Fn(ERef) -> Option<EdgeId>,
    face_pieces: &BTreeMap<FaceId, (bool, Vec<usize>)>,
    out_faces: &BTreeMap<usize, FaceId>,
    kept: &[Kept],
    shells: &[Vec<usize>],
    built_shells: &[ShellId],
) -> Provenance {
    let mut p = Provenance::new();
    for operand in &operands {
        let policy = operand.policy;
        let body = operand.body;
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
        for &v in operand.vertices {
            let image = match merged_into.get(&v) {
                Some(&k) => vertex_id(vref_of[k]),
                None => vertex_id(VRef::Existing(v)),
            };
            record(&mut p, forward(v), image.into_iter().map(forward).collect());
        }
        for &e in operand.edges {
            let n = sub_edges.get(&e).map_or(0, Vec::len);
            let mut images: Vec<Shape> = Vec::new();
            for index in 0..n {
                if let Some(id) = edge_id(ERef::Sub { edge: e, index }) {
                    let image = forward(id);
                    if !images.contains(&image) {
                        images.push(image);
                    }
                }
            }
            record(&mut p, forward(e), images);
        }
        for f in operand.faces {
            let images: Vec<Shape> = face_pieces
                .get(&f.id)
                .map(|(_, pieces)| {
                    pieces
                        .iter()
                        .filter_map(|k| out_faces.get(k))
                        .map(|&id| forward(id))
                        .collect()
                })
                .unwrap_or_default();
            record(&mut p, forward(f.id), images);
        }
        if policy == Policy::Regenerate {
            for &s in operand.shells {
                p.add_deleted(forward(s));
            }
        }
        match policy {
            Policy::Reuse => p.add_modified(forward(body.id), forward(built_body.id)),
            Policy::Regenerate => p.add_deleted(forward(body.id)),
        }
    }
    // A result shell is `Modified` from every shell of a kept-by-id
    // operand a piece of it came from, and one made of a cut tool's
    // pieces alone is `Generated` from the tool's shell; a shell of a
    // kept-by-id operand no result shell came from is gone.
    let mut reached: BTreeSet<ShellId> = BTreeSet::new();
    for (pieces, &out) in shells.iter().zip(built_shells) {
        let mut from: [BTreeSet<ShellId>; 2] = [BTreeSet::new(), BTreeSet::new()];
        for piece in pieces.iter().filter_map(|&k| kept.get(k)) {
            if let Some(&s) = operands[piece.side].shell_of.get(&piece.face.id) {
                from[piece.side].insert(s);
            }
        }
        let reused: Vec<ShellId> = (0..2)
            .filter(|&side| operands[side].policy == Policy::Reuse)
            .flat_map(|side| from[side].iter().copied())
            .collect();
        if reused.is_empty() {
            for s in from.iter().flatten() {
                p.add_generated(forward(*s), forward(out));
            }
        }
        for s in reused {
            p.add_modified(forward(s), forward(out));
            reached.insert(s);
        }
    }
    for side in (0..2).filter(|&side| operands[side].policy == Policy::Reuse) {
        for &s in operands[side]
            .shells
            .iter()
            .filter(|s| !reached.contains(s))
        {
            p.add_deleted(forward(s));
        }
    }
    // A piece kept once from a coincident pair stands for the other
    // operand's face too.
    for (k, piece) in kept.iter().enumerate() {
        if let (Some(g), Some(&id)) = (piece.stands_for, out_faces.get(&k)) {
            p.add_generated(forward(g), forward(id));
        }
    }
    p
}
