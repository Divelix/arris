//! The triangle mesh and its measurements.

use core::ops::Range;
use std::collections::BTreeMap;

use arris_topo::{EdgeId, FaceId};

use crate::aabb::Aabb;

/// The triangles of one B-Rep face: a contiguous run of a
/// [`TriMesh`]'s triangle list, in the body's face iteration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceRange {
    /// The face.
    pub face: FaceId,
    /// Indices into [`TriMesh::triangles`].
    pub triangles: Range<usize>,
}

/// The discretisation of one B-Rep edge: a contiguous run of a
/// [`TriMesh`]'s edge index list, forming one polyline through the mesh's
/// vertices, in the body's edge iteration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeRange {
    /// The edge.
    pub edge: EdgeId,
    /// Indices into [`TriMesh::edge_indices`].
    pub indices: Range<usize>,
}

/// Why a mesh could not be built or extended.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MeshError {
    /// A triangle or edge index does not name a position.
    #[error("index {index} is out of range for {positions} positions")]
    IndexOutOfRange {
        /// The offending index.
        index: u32,
        /// How many positions the mesh has.
        positions: usize,
    },
    /// A face or edge range does not fit in the list it indexes.
    #[error("range {start}..{end} is out of bounds for a list of {len}")]
    RangeOutOfBounds {
        /// Range start.
        start: usize,
        /// Range end.
        end: usize,
        /// Length of the indexed list.
        len: usize,
    },
    /// A position has a non-finite coordinate.
    #[error("position {index} is not finite")]
    NonFinitePosition {
        /// The position's index.
        index: usize,
    },
}

/// An indexed triangle mesh with `f64` positions, the output type of
/// tessellation and the input of the rasteriser.
///
/// Triangles are counter-clockwise seen from outside, so a closed mesh's
/// signed volume is positive when its normals point out of the material.
/// The mesh is a buffer, not geometry: positions are plain arrays so a
/// consumer can upload them, and the per-face and per-edge ranges are what
/// let it colour or pick by B-Rep entity.
///
/// Every index is validated when it enters, so the measurements below never
/// panic; a mesh with no triangles is not closed and has no volume.
///
/// ```
/// use arris_mesh::TriMesh;
///
/// // A tetrahedron with outward normals.
/// let mut m = TriMesh::new();
/// for p in [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
///     m.push_position(p).unwrap();
/// }
/// for t in [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]] {
///     m.push_triangle(t).unwrap();
/// }
/// assert!(m.is_closed());
/// assert!((m.signed_volume().unwrap() - 1.0 / 6.0).abs() < 1e-15);
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TriMesh {
    positions: Vec<[f64; 3]>,
    triangles: Vec<[u32; 3]>,
    faces: Vec<FaceRange>,
    edge_indices: Vec<u32>,
    edges: Vec<EdgeRange>,
}

impl TriMesh {
    /// An empty mesh.
    pub fn new() -> Self {
        Self::default()
    }

    /// A mesh from its parts, validated once: every index names a
    /// position, every range fits its list, every position is finite.
    pub fn from_parts(
        positions: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
        faces: Vec<FaceRange>,
        edge_indices: Vec<u32>,
        edges: Vec<EdgeRange>,
    ) -> Result<Self, MeshError> {
        for (index, p) in positions.iter().enumerate() {
            if !p.iter().all(|c| c.is_finite()) {
                return Err(MeshError::NonFinitePosition { index });
            }
        }
        let n = positions.len();
        for t in &triangles {
            check_indices(t, n)?;
        }
        check_indices(&edge_indices, n)?;
        for f in &faces {
            check_range(&f.triangles, triangles.len())?;
        }
        for e in &edges {
            check_range(&e.indices, edge_indices.len())?;
        }
        Ok(TriMesh {
            positions,
            triangles,
            faces,
            edge_indices,
            edges,
        })
    }

    /// Adds a position and returns its index.
    pub fn push_position(&mut self, p: [f64; 3]) -> Result<u32, MeshError> {
        if !p.iter().all(|c| c.is_finite()) {
            return Err(MeshError::NonFinitePosition {
                index: self.positions.len(),
            });
        }
        self.positions.push(p);
        Ok((self.positions.len() - 1) as u32)
    }

    /// Adds a triangle and returns its index.
    pub fn push_triangle(&mut self, t: [u32; 3]) -> Result<usize, MeshError> {
        check_indices(&t, self.positions.len())?;
        self.triangles.push(t);
        Ok(self.triangles.len() - 1)
    }

    /// Adds the triangles of one face and records their range under `face`.
    pub fn push_face(
        &mut self,
        face: FaceId,
        triangles: impl IntoIterator<Item = [u32; 3]>,
    ) -> Result<&FaceRange, MeshError> {
        let start = self.triangles.len();
        for t in triangles {
            self.push_triangle(t)?;
        }
        self.faces.push(FaceRange {
            face,
            triangles: start..self.triangles.len(),
        });
        Ok(self.faces.last().expect("just pushed"))
    }

    /// Adds the polyline of one edge, as position indices, and records its
    /// range under `edge`.
    pub fn push_edge(&mut self, edge: EdgeId, polyline: &[u32]) -> Result<&EdgeRange, MeshError> {
        check_indices(polyline, self.positions.len())?;
        let start = self.edge_indices.len();
        self.edge_indices.extend_from_slice(polyline);
        self.edges.push(EdgeRange {
            edge,
            indices: start..self.edge_indices.len(),
        });
        Ok(self.edges.last().expect("just pushed"))
    }

    /// The vertex positions.
    pub fn positions(&self) -> &[[f64; 3]] {
        &self.positions
    }

    /// The triangles as index triples, counter-clockwise from outside.
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }

    /// The per-face ranges, in face iteration order.
    pub fn faces(&self) -> &[FaceRange] {
        &self.faces
    }

    /// The concatenated edge polylines as position indices.
    pub fn edge_indices(&self) -> &[u32] {
        &self.edge_indices
    }

    /// The per-edge ranges, in edge iteration order.
    pub fn edges(&self) -> &[EdgeRange] {
        &self.edges
    }

    /// The triangles of `face`, or `None` if the mesh has no range for it.
    pub fn face_triangles(&self, face: FaceId) -> Option<&[[u32; 3]]> {
        let r = self.faces.iter().find(|f| f.face == face)?;
        self.triangles.get(r.triangles.clone())
    }

    /// The polyline of `edge` as position indices, or `None` if the mesh
    /// has no range for it.
    pub fn edge_polyline(&self, edge: EdgeId) -> Option<&[u32]> {
        let r = self.edges.iter().find(|e| e.edge == edge)?;
        self.edge_indices.get(r.indices.clone())
    }

    /// The three corner positions of triangle `i`.
    pub fn triangle_positions(&self, i: usize) -> Option<[[f64; 3]; 3]> {
        let t = self.triangles.get(i)?;
        Some([self.pos(t[0]), self.pos(t[1]), self.pos(t[2])])
    }

    fn pos(&self, i: u32) -> [f64; 3] {
        // Every index was validated on entry.
        self.positions[i as usize]
    }

    /// `true` when the triangles form a closed, consistently oriented
    /// surface: every directed edge `(i, j)` occurs exactly once and so
    /// does its opposite `(j, i)`. A flipped triangle, a missing one, a
    /// degenerate one or an empty mesh all make this `false`.
    pub fn is_closed(&self) -> bool {
        if self.triangles.is_empty() {
            return false;
        }
        let mut count: BTreeMap<(u32, u32), u32> = BTreeMap::new();
        for t in &self.triangles {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                if a == b {
                    return false;
                }
                *count.entry((a, b)).or_insert(0) += 1;
            }
        }
        count
            .iter()
            .all(|(&(a, b), &n)| n == 1 && count.get(&(b, a)) == Some(&1))
    }

    /// The enclosed volume by the divergence theorem, `Σ a · (b × c) / 6`
    /// over the triangles: positive for outward normals. `None` when the
    /// mesh is not closed, because the sum then depends on the origin and
    /// means nothing.
    pub fn signed_volume(&self) -> Option<f64> {
        self.is_closed().then(|| {
            self.triangles
                .iter()
                .map(|t| {
                    let a = self.pos(t[0]);
                    let b = self.pos(t[1]);
                    let c = self.pos(t[2]);
                    dot(a, cross(b, c))
                })
                .sum::<f64>()
                / 6.0
        })
    }

    /// Sum of the triangle areas.
    pub fn area(&self) -> f64 {
        self.triangles
            .iter()
            .map(|t| {
                let a = self.pos(t[0]);
                let b = self.pos(t[1]);
                let c = self.pos(t[2]);
                norm(cross(sub(b, a), sub(c, a))) / 2.0
            })
            .sum::<f64>()
    }

    /// The bounding box of the positions, or `None` for no positions.
    pub fn aabb(&self) -> Option<Aabb> {
        Aabb::of_points(&self.positions)
    }
}

fn check_indices(indices: &[u32], positions: usize) -> Result<(), MeshError> {
    match indices.iter().find(|&&i| i as usize >= positions) {
        Some(&index) => Err(MeshError::IndexOutOfRange { index, positions }),
        None => Ok(()),
    }
}

fn check_range(r: &Range<usize>, len: usize) -> Result<(), MeshError> {
    if r.start > r.end || r.end > len {
        return Err(MeshError::RangeOutOfBounds {
            start: r.start,
            end: r.end,
            len,
        });
    }
    Ok(())
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cube [-1, 1]³ as 8 positions and 12 outward triangles, one
    /// `FaceRange` per face in the order -z, +z, -y, +y, -x, +x.
    pub(crate) fn cube() -> TriMesh {
        let mut m = TriMesh::new();
        for z in [-1.0, 1.0] {
            for y in [-1.0, 1.0] {
                for x in [-1.0, 1.0] {
                    m.push_position([x, y, z]).unwrap();
                }
            }
        }
        // Position index = x + 2y + 4z with each coordinate 0 or 1.
        let quads: [[u32; 4]; 6] = [
            [0, 2, 3, 1], // -z, normal down
            [4, 5, 7, 6], // +z
            [0, 1, 5, 4], // -y
            [2, 6, 7, 3], // +y
            [0, 4, 6, 2], // -x
            [1, 3, 7, 5], // +x
        ];
        for (i, q) in quads.iter().enumerate() {
            m.push_face(
                FaceId::new(i as u32, 0),
                [[q[0], q[1], q[2]], [q[0], q[2], q[3]]],
            )
            .unwrap();
        }
        m
    }

    #[test]
    fn cube_measures_eight_twenty_four_closed() {
        let m = cube();
        assert_eq!(m.triangles().len(), 12);
        assert_eq!(m.faces().len(), 6);
        assert!(m.is_closed());
        assert_eq!(m.signed_volume(), Some(8.0));
        assert_eq!(m.area(), 24.0);
        assert_eq!(
            m.aabb(),
            Some(Aabb {
                min: [-1.0; 3],
                max: [1.0; 3]
            })
        );
        assert_eq!(
            m.face_triangles(FaceId::new(1, 0)),
            Some(&[[4, 5, 7], [4, 7, 6]][..])
        );
        assert_eq!(m.face_triangles(FaceId::new(9, 0)), None);
    }

    #[test]
    fn one_flipped_triangle_is_not_closed() {
        let m = cube();
        let mut tris = m.triangles().to_vec();
        tris[5].swap(1, 2);
        let flipped =
            TriMesh::from_parts(m.positions().to_vec(), tris, vec![], vec![], vec![]).unwrap();
        assert!(!flipped.is_closed());
        assert_eq!(
            flipped.signed_volume(),
            None,
            "volume of an inconsistent mesh is unreliable"
        );
        assert_eq!(flipped.area(), 24.0, "area does not care about orientation");
    }

    #[test]
    fn one_missing_triangle_is_open() {
        let m = cube();
        let mut tris = m.triangles().to_vec();
        tris.pop();
        let open =
            TriMesh::from_parts(m.positions().to_vec(), tris, vec![], vec![], vec![]).unwrap();
        assert!(!open.is_closed());
        assert_eq!(
            open.signed_volume(),
            None,
            "volume of an open mesh is unreliable"
        );
        assert_eq!(open.area(), 22.0);
    }

    #[test]
    fn inside_out_cube_is_closed_with_negative_volume() {
        let m = cube();
        let tris: Vec<[u32; 3]> = m.triangles().iter().map(|t| [t[0], t[2], t[1]]).collect();
        let inv =
            TriMesh::from_parts(m.positions().to_vec(), tris, vec![], vec![], vec![]).unwrap();
        assert!(inv.is_closed());
        assert_eq!(inv.signed_volume(), Some(-8.0));
    }

    #[test]
    fn volume_is_translation_invariant_only_when_closed() {
        let m = cube();
        let shifted: Vec<[f64; 3]> = m
            .positions()
            .iter()
            .map(|p| [p[0] + 10.0, p[1] - 3.0, p[2] + 0.5])
            .collect();
        let s =
            TriMesh::from_parts(shifted, m.triangles().to_vec(), vec![], vec![], vec![]).unwrap();
        assert!((s.signed_volume().unwrap() - 8.0).abs() < 1e-12);
    }

    #[test]
    fn empty_and_degenerate_are_not_closed() {
        assert!(!TriMesh::new().is_closed());
        assert_eq!(TriMesh::new().signed_volume(), None);
        assert_eq!(TriMesh::new().aabb(), None);
        let mut m = TriMesh::new();
        m.push_position([0.0; 3]).unwrap();
        m.push_position([1.0, 0.0, 0.0]).unwrap();
        m.push_triangle([0, 1, 1]).unwrap();
        assert!(!m.is_closed());
    }

    #[test]
    fn bad_input_is_a_typed_error_not_a_panic() {
        let mut m = TriMesh::new();
        m.push_position([0.0; 3]).unwrap();
        assert_eq!(
            m.push_triangle([0, 0, 7]),
            Err(MeshError::IndexOutOfRange {
                index: 7,
                positions: 1
            })
        );
        assert_eq!(
            m.push_position([f64::NAN, 0.0, 0.0]),
            Err(MeshError::NonFinitePosition { index: 1 })
        );
        assert_eq!(
            m.push_edge(EdgeId::new(0, 0), &[0, 3]),
            Err(MeshError::IndexOutOfRange {
                index: 3,
                positions: 1
            })
        );
        let bad_range = TriMesh::from_parts(
            vec![[0.0; 3]],
            vec![],
            vec![FaceRange {
                face: FaceId::new(0, 0),
                triangles: 0..1,
            }],
            vec![],
            vec![],
        );
        assert_eq!(
            bad_range,
            Err(MeshError::RangeOutOfBounds {
                start: 0,
                end: 1,
                len: 0
            })
        );
    }

    #[test]
    fn edges_are_polylines_over_mesh_vertices() {
        let mut m = cube();
        m.push_edge(EdgeId::new(0, 0), &[0, 1]).unwrap();
        m.push_edge(EdgeId::new(1, 0), &[1, 3, 7]).unwrap();
        assert_eq!(m.edge_polyline(EdgeId::new(1, 0)), Some(&[1, 3, 7][..]));
        assert_eq!(m.edges()[1].indices, 2..5);
    }
}
