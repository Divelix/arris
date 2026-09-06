//! A body to a [`TriMesh`]: edges discretised once, faces triangulated in
//! their own (u, v) through the same-parameter pcurves (ADR-0003,
//! `docs/01-architecture.md` §Tessellation).

use std::collections::BTreeMap;

use arris_topo::arris_geom::Surface;
use arris_topo::arris_geom::region2::{MAX_SEGMENTS_PER_PIECE, Polygon2, discretise};
use arris_topo::arris_math::{Interval, Point2};
use arris_topo::entity::{EdgeGeometry, Face as FaceEntity};
use arris_topo::{Body, EdgeId, Model, NotFound, Orientation, VertexId};

use crate::cdt::{self, CdtError, VertexRef};
use crate::{MeshError, TriMesh};

/// The samples of one edge: `n + 1` parameters over its range and the
/// mesh index of each, the first and last being the end vertices'.
struct EdgeSamples {
    params: Vec<f64>,
    indices: Vec<u32>,
}

/// The triangle mesh of `body` at `chord`: every position within `chord`
/// of the geometry, as ADR-0003 and `docs/01-architecture.md`
/// §Tessellation guarantee.
///
/// Every topo vertex of the body is one mesh vertex. Every edge is
/// sampled once, at `n` uniform parameters over its range — `n` the
/// largest of its curve's [`arris_topo::arris_geom::Curve::chord_segments`]
/// at `chord` and, per coedge, the count that keeps each step's `u`- and
/// `v`-travel under the face's surface's
/// [`arris_topo::arris_geom::Surface::chord_steps`], so a triangle
/// standing on the edge is within `chord` of the surface — and its
/// samples are one run of positions, its [`crate::EdgeRange`] the
/// polyline from its start vertex to its end vertex along its curve's
/// parameter; a degenerate edge's range is one index. Every face's loops
/// are the *same* parameters through each coedge's pcurve, triangulated
/// by [`cdt::triangulate`] and mapped back to the shared indices, so the
/// mesh of a solid is closed by construction; a seam's run is used twice
/// with opposite directions, triangles that collapse onto one index are
/// dropped, and the triangles of a face used `Reversed` are turned so
/// every triangle is counter-clockwise seen from outside. Ranges are in
/// the body's iteration order (`Model::faces`, `Model::edges`). The
/// output is the same on every platform for the same body and chord.
///
/// The chord is the consumer's request, a number like the render's
/// resolution, not a model tolerance; it must be finite and positive.
/// In debug builds the body passes `arris_check` at `Level::Fast` first.
///
/// Errors: [`MeshError::Chord`]; [`MeshError::InvalidInput`] (debug
/// builds); [`MeshError::NotFound`] for the body or anything it refers
/// to; [`MeshError::Unsupported`] naming a face on a sphere, torus or
/// NURBS surface until their interior grids land; [`MeshError::Face`]
/// when a face's loops are not the simple nested polygons a valid face
/// has; [`MeshError::NonFinitePosition`] when the geometry evaluates to
/// a non-finite point.
///
/// ```
/// use arris_debug::sample;
/// use arris_mesh::tessellate;
/// use arris_topo::Model;
/// use core::f64::consts::PI;
///
/// let mut m = Model::default();
/// let cylinder = sample::cylinder(&mut m, 4.0, 12.0).unwrap();
/// let mesh = tessellate(&m, cylinder, 1e-3).unwrap();
/// assert!(mesh.is_closed());
/// assert_eq!(mesh.faces().len(), 3);
/// assert_eq!(mesh.edges().len(), 3);
/// let volume = mesh.signed_volume().unwrap();
/// let exact = PI * 16.0 * 12.0;
/// // An inscribed prism at sagitta δ on radius r: within 4δ / (3r).
/// assert!(volume > 0.0 && exact - volume <= exact * 4.0 * 1e-3 / (3.0 * 4.0));
/// ```
pub fn tessellate(m: &Model, body: Body, chord: f64) -> Result<TriMesh, MeshError> {
    if !(chord.is_finite() && chord > 0.0) {
        return Err(MeshError::Chord(chord));
    }
    // A handle that does not resolve is `NotFound` in every build; the
    // debug check below would otherwise report it as an M1 violation.
    m.body(body.id)?;
    #[cfg(debug_assertions)]
    {
        let report = arris_check::check(m, body, arris_check::Level::Fast);
        if !report.is_ok() {
            return Err(MeshError::InvalidInput {
                body,
                report: Box::new(report),
            });
        }
    }
    let faces = m.faces(body)?;
    let edges = m.edges(body)?;
    let vertices = m.vertices(body)?;

    // Pass one: what each face asks of the edges it uses, so a step along
    // an edge never travels farther in (u, v) than the surface allows.
    let mut required: BTreeMap<EdgeId, usize> = BTreeMap::new();
    for f in &faces {
        let face = m.face(f.id)?;
        let surface = m.surface(face.surface())?;
        match surface {
            Surface::Plane { .. } | Surface::Cylinder { .. } | Surface::Cone { .. } => {}
            Surface::Sphere { .. } | Surface::Torus { .. } | Surface::Nurbs(_) => {
                return Err(MeshError::Unsupported {
                    face: f.id,
                    kind: surface.kind(),
                });
            }
        }
        let steps = surface.chord_steps(chord, region_bounds(m, face)?);
        for coedge in face.loops().iter().flat_map(|l| l.coedges()) {
            let range = m.edge(coedge.edge())?.range();
            let speed = m.curve2(coedge.pcurve())?.speed_bounds(range);
            let mut n = 0usize;
            for dir in 0..2 {
                let travel = range.length() * speed[dir];
                if travel <= 0.0 {
                    continue;
                }
                let wanted = (travel / steps[dir]).ceil();
                n = n.max(if wanted.is_finite() {
                    wanted as usize
                } else {
                    MAX_SEGMENTS_PER_PIECE
                });
            }
            let entry = required.entry(coedge.edge()).or_default();
            *entry = (*entry).max(n);
        }
    }

    // Pass two: every vertex a position, every edge a run of samples.
    let mut mesh = TriMesh::new();
    let mut vertex_index: BTreeMap<VertexId, u32> = BTreeMap::new();
    for v in &vertices {
        let p = m.vertex(v.id)?.point();
        let index = mesh.push_position([p.x, p.y, p.z])?;
        vertex_index.insert(v.id, index);
    }
    let mut samples: BTreeMap<EdgeId, EdgeSamples> = BTreeMap::new();
    for e in &edges {
        let edge = m.edge(e.id)?;
        let range = edge.range();
        let start = *vertex_index
            .get(&edge.start())
            .ok_or(NotFound::new(edge.start()))?;
        let end = *vertex_index
            .get(&edge.end())
            .ok_or(NotFound::new(edge.end()))?;
        let curve = match edge.geometry() {
            EdgeGeometry::Curve { curve, .. } => Some(m.curve(curve)?),
            EdgeGeometry::Degenerate { .. } => None,
        };
        let n = curve
            .map_or(1, |c| c.chord_segments(range, chord))
            .max(required.get(&e.id).copied().unwrap_or(0))
            .clamp(1, MAX_SEGMENTS_PER_PIECE);
        let params: Vec<f64> = (0..=n).map(|i| range.lerp(i as f64 / n as f64)).collect();
        let indices = match curve {
            Some(c) => {
                let mut indices = Vec::with_capacity(n + 1);
                indices.push(start);
                for &t in &params[1..n] {
                    let p = c.point(t);
                    indices.push(mesh.push_position([p.x, p.y, p.z])?);
                }
                indices.push(end);
                indices
            }
            None => vec![start; n + 1],
        };
        if curve.is_some() {
            mesh.push_edge(e.id, &indices)?;
        } else {
            mesh.push_edge(e.id, &[start])?;
        }
        samples.insert(e.id, EdgeSamples { params, indices });
    }

    // Pass three: every face's loops through its pcurves at the edges'
    // parameters, triangulated, mapped back to the shared indices.
    for f in &faces {
        let face = m.face(f.id)?;
        let mut polygons: Vec<Polygon2> = Vec::with_capacity(face.loops().len());
        let mut rings: Vec<Vec<u32>> = Vec::with_capacity(face.loops().len());
        for l in face.loops() {
            let mut points: Vec<Point2> = Vec::new();
            let mut indices: Vec<u32> = Vec::new();
            for coedge in l.coedges() {
                let s = samples
                    .get(&coedge.edge())
                    .ok_or(NotFound::new(coedge.edge()))?;
                let pcurve = m.curve2(coedge.pcurve())?;
                let n = s.params.len() - 1;
                // All but the last sample in walking order: the next
                // coedge's first sample is the junction.
                let order: Vec<usize> = if coedge.orientation() == Orientation::Forward {
                    (0..n).collect()
                } else {
                    (1..=n).rev().collect()
                };
                for i in order {
                    let uv = pcurve.point(s.params[i]);
                    if points.last() != Some(&uv) {
                        points.push(uv);
                        indices.push(s.indices[i]);
                    }
                }
            }
            if points.len() > 1 && points.first() == points.last() {
                points.pop();
                indices.pop();
            }
            let polygon = Polygon2::from_points(points.iter().copied());
            if polygon.points().len() != indices.len() {
                return Err(MeshError::Face {
                    face: f.id,
                    source: CdtError::Internal("a loop's ring and its index ring differ in length"),
                });
            }
            polygons.push(polygon);
            rings.push(indices);
        }
        let triangulation = cdt::triangulate(&polygons, &[])
            .map_err(|source| MeshError::Face { face: f.id, source })?;
        let index_of = |v: usize| -> Result<u32, MeshError> {
            match triangulation.vertex_ref(v) {
                Some(VertexRef::Polygon { polygon, vertex }) => rings
                    .get(polygon)
                    .and_then(|r| r.get(vertex))
                    .copied()
                    .ok_or(MeshError::Face {
                        face: f.id,
                        source: CdtError::Internal("a triangle corner names no loop point"),
                    }),
                _ => Err(MeshError::Face {
                    face: f.id,
                    source: CdtError::Internal("a triangle corner is not a loop point"),
                }),
            }
        };
        let reversed = f.orientation == Orientation::Reversed;
        let mut triangles = Vec::with_capacity(triangulation.triangles().len());
        for &[a, b, c] in triangulation.triangles() {
            let (ia, ib, ic) = (index_of(a)?, index_of(b)?, index_of(c)?);
            if ia == ib || ib == ic || ic == ia {
                continue;
            }
            triangles.push(if reversed { [ia, ic, ib] } else { [ia, ib, ic] });
        }
        mesh.push_face(f.id, triangles)?;
    }
    Ok(mesh)
}

/// The (u, v) box a face's loops span, padded by the chord deviation of
/// the polygons they were read from so it holds the true boundary; the
/// whole plane when the face has no loop.
fn region_bounds(m: &Model, face: &FaceEntity) -> Result<[Interval; 2], NotFound> {
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for l in face.loops() {
        let pieces = m.loop_pieces(l)?;
        let polygon = discretise(&pieces, f64::INFINITY);
        let pad = polygon.chord_deviation();
        for p in polygon.points() {
            lo = [lo[0].min(p.x - pad), lo[1].min(p.y - pad)];
            hi = [hi[0].max(p.x + pad), hi[1].max(p.y + pad)];
        }
    }
    Ok([
        Interval::new(lo[0], hi[0]).unwrap_or(Interval::REAL),
        Interval::new(lo[1], hi[1]).unwrap_or(Interval::REAL),
    ])
}
