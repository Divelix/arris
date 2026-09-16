//! OBJ (`docs/ARCHITECTURE.md` §Formats and tools, ADR-0013): `write` of
//! an [`arris_mesh::TriMesh`], `v`/`f` always and `vt`/`vn` when the mesh
//! carries a [`arris_mesh::Corners`] block.
//!
//! `v` is [`TriMesh::positions`](arris_mesh::TriMesh::positions), shared
//! across faces as the mesh itself shares it. When the mesh has no
//! corner block, `f` names three `v` indices and nothing else — OBJ's
//! bare `f v1 v2 v3` — and there is one `g <face>` per
//! [`arris_mesh::FaceRange`], partitioning the triangles by face in the
//! body's iteration order.
//!
//! When it does, `vt` is [`Corners::uvs`](arris_mesh::Corners::uvs) and
//! `vn` is [`Corners::normals`](arris_mesh::Corners::normals), one of
//! each per face-local vertex, and `f` is `v/vt/vn v/vt/vn v/vt/vn` —
//! the shared position from the mesh's own triangle, the (u, v) and
//! normal from [`Corners::triangles`](arris_mesh::Corners::triangles)'s
//! parallel face-local one, so a corner's `vt` and `vn` are always the
//! same index: OBJ has no way to name one without the other, and a
//! face-local vertex has exactly one of each. That is what a per-corner
//! render buffer is for.
//!
//! Every real is Rust's shortest round-trip decimal, `f64` throughout
//! (ADR-0011), so two writes of one mesh are byte-identical.

use core::fmt::Write as _;

use arris_mesh::TriMesh;

use crate::MeshWriteError;

/// The OBJ text of `mesh`. Never fails: OBJ has no bound this crate
/// enforces.
///
/// ```
/// use arris_io::arris_mesh::TriMesh;
/// use arris_io::obj;
///
/// let mut mesh = TriMesh::new();
/// for p in [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
///     mesh.push_position(p).unwrap();
/// }
/// mesh.push_face(
///     arris_io::arris_check::arris_topo::FaceId::new(0, 0),
///     [[0, 1, 2]],
/// )
/// .unwrap();
/// let text = obj::write(&mesh).unwrap();
/// assert!(text.contains("v 0. 0. 0."));
/// assert!(text.contains("f 1 2 3"));
/// assert!(!text.contains("vt"), "no corner block, no vt");
/// ```
pub fn write(mesh: &TriMesh) -> Result<String, MeshWriteError> {
    let mut out = String::new();
    for p in mesh.positions() {
        let _ = writeln!(out, "v {} {} {}", real(p[0]), real(p[1]), real(p[2]));
    }
    if let Some(corners) = mesh.corners() {
        for uv in corners.uvs() {
            let _ = writeln!(out, "vt {} {}", real(uv[0]), real(uv[1]));
        }
        for n in corners.normals() {
            let _ = writeln!(out, "vn {} {} {}", real(n[0]), real(n[1]), real(n[2]));
        }
    }
    for range in mesh.faces() {
        let _ = writeln!(out, "g {}", range.face);
        for i in range.triangles.clone() {
            let v = mesh.triangles()[i];
            if let Some(corners) = mesh.corners() {
                let c = corners.triangles()[i];
                let _ = writeln!(
                    out,
                    "f {}/{}/{} {}/{}/{} {}/{}/{}",
                    v[0] + 1,
                    c[0] + 1,
                    c[0] + 1,
                    v[1] + 1,
                    c[1] + 1,
                    c[1] + 1,
                    v[2] + 1,
                    c[2] + 1,
                    c[2] + 1,
                );
            } else {
                let _ = writeln!(out, "f {} {} {}", v[0] + 1, v[1] + 1, v[2] + 1);
            }
        }
    }
    Ok(out)
}

/// A finite real in OBJ's spelling: Rust's shortest round-trip decimal
/// with a decimal point always present, mirroring `step::real` and
/// `stl::real`.
fn real(x: f64) -> String {
    let x = if x == 0.0 { 0.0 } else { x };
    let s = format!("{x}");
    if s.contains('.') { s } else { format!("{s}.") }
}

#[cfg(test)]
mod tests {
    use arris_check::arris_topo::FaceId;

    use super::*;

    fn triangle() -> TriMesh {
        let mut m = TriMesh::new();
        for p in [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            m.push_position(p).unwrap();
        }
        m.push_face(FaceId::new(0, 0), [[0, 1, 2]]).unwrap();
        m
    }

    #[test]
    fn a_mesh_without_corners_has_no_vt_or_vn() {
        let text = write(&triangle()).unwrap();
        assert!(text.contains("v 0. 0. 0."));
        assert!(text.contains("g f0"));
        assert!(text.contains("f 1 2 3"));
        assert!(!text.contains("vt"));
        assert!(!text.contains("vn"));
    }

    #[test]
    fn reals_are_shortest_round_trip_with_a_point() {
        assert_eq!(real(40.0), "40.");
        assert_eq!(real(0.5), "0.5");
        assert_eq!(real(-0.0), "0.");
    }
}
