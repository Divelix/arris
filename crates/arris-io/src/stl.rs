//! STL, ASCII and binary (`docs/ARCHITECTURE.md` §Formats and tools,
//! ADR-0013): `write_ascii` and `write_binary` of an
//! [`arris_mesh::TriMesh`].
//!
//! A facet's normal is the triangle's own winding — `(b − a) × (c − a)`,
//! normalised, never the render buffer's corner normals, which are the
//! surface's own and can disagree with a flat facet's — because that is
//! what the format means by a facet's normal. A degenerate triangle, which
//! a mesh tessellation produces none of, writes the zero vector rather
//! than divide by zero.
//!
//! ASCII writes every real as Rust's shortest round-trip decimal, `f64`
//! throughout, so two writes are byte-identical, as STEP's are
//! (`docs/ARCHITECTURE.md` §Formats and tools). Binary writes IEEE 754
//! little-endian `f32` per coordinate and per normal component, because
//! that is the format: the one narrowing the kernel ships
//! (`.agents/rules/kernel.md`), computed in nothing but `f64` and cast
//! only here, at the writer. Both carry `name` — a `solid`/`endsolid`
//! line in ASCII, the first bytes of the 80-byte header in binary,
//! truncated to fit — so the two forms of one write name the same shape.

use core::fmt::Write as _;

use arris_mesh::TriMesh;

use crate::MeshWriteError;

/// The ASCII STL text of `mesh`: `solid <name>`, one `facet` per
/// triangle in the mesh's own order, `endsolid <name>`. Every real is
/// Rust's shortest round-trip decimal, so two writes of one mesh are
/// byte-identical. Never fails: ASCII STL has no bound on its facet
/// count.
///
/// ```
/// use arris_io::arris_mesh::TriMesh;
/// use arris_io::stl;
///
/// let mut mesh = TriMesh::new();
/// for p in [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
///     mesh.push_position(p).unwrap();
/// }
/// mesh.push_triangle([0, 1, 2]).unwrap();
/// let text = stl::write_ascii(&mesh, "triangle").unwrap();
/// assert!(text.starts_with("solid triangle\n"));
/// assert!(text.trim_end().ends_with("endsolid triangle"));
/// assert_eq!(text.matches("facet normal").count(), 1);
/// assert_eq!(stl::write_ascii(&mesh, "triangle").unwrap(), text, "deterministic");
/// ```
pub fn write_ascii(mesh: &TriMesh, name: &str) -> Result<String, MeshWriteError> {
    let mut out = String::new();
    let _ = writeln!(out, "solid {name}");
    for i in 0..mesh.triangles().len() {
        let [a, b, c] = mesh
            .triangle_positions(i)
            .expect("i is one of this mesh's own triangle indices");
        let n = facet_normal(a, b, c);
        let _ = writeln!(
            out,
            "  facet normal {} {} {}",
            real(n[0]),
            real(n[1]),
            real(n[2])
        );
        out.push_str("    outer loop\n");
        for p in [a, b, c] {
            let _ = writeln!(
                out,
                "      vertex {} {} {}",
                real(p[0]),
                real(p[1]),
                real(p[2])
            );
        }
        out.push_str("    endloop\n");
        out.push_str("  endfacet\n");
    }
    let _ = writeln!(out, "endsolid {name}");
    Ok(out)
}

/// The binary STL bytes of `mesh`: an 80-byte header holding `name`
/// (truncated to fit, zero-padded), a little-endian `u32` triangle
/// count, then per triangle a little-endian `f32` normal, three `f32`
/// vertices and a `u16` attribute byte count of zero — the format's own
/// 50-byte-per-facet layout.
///
/// Errors: [`MeshWriteError::TooManyTriangles`] when the mesh has more
/// triangles than a `u32` count can hold.
///
/// ```
/// use arris_io::arris_mesh::TriMesh;
/// use arris_io::stl;
///
/// let mut mesh = TriMesh::new();
/// for p in [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
///     mesh.push_position(p).unwrap();
/// }
/// mesh.push_triangle([0, 1, 2]).unwrap();
/// let bytes = stl::write_binary(&mesh, "triangle").unwrap();
/// assert_eq!(bytes.len(), 80 + 4 + 50);
/// assert!(bytes.starts_with(b"triangle"));
/// assert_eq!(u32::from_le_bytes(bytes[80..84].try_into().unwrap()), 1);
/// ```
pub fn write_binary(mesh: &TriMesh, name: &str) -> Result<Vec<u8>, MeshWriteError> {
    let triangles = mesh.triangles().len();
    let count =
        u32::try_from(triangles).map_err(|_| MeshWriteError::TooManyTriangles { triangles })?;
    let mut out = Vec::with_capacity(80 + 4 + triangles * 50);
    let mut header = [0u8; 80];
    let name_bytes = name.as_bytes();
    let fit = name_bytes.len().min(80);
    header[..fit].copy_from_slice(&name_bytes[..fit]);
    out.extend_from_slice(&header);
    out.extend_from_slice(&count.to_le_bytes());
    for i in 0..triangles {
        let [a, b, c] = mesh
            .triangle_positions(i)
            .expect("i is one of this mesh's own triangle indices");
        for component in facet_normal(a, b, c) {
            out.extend_from_slice(&(component as f32).to_le_bytes());
        }
        for p in [a, b, c] {
            for component in p {
                out.extend_from_slice(&(component as f32).to_le_bytes());
            }
        }
        out.extend_from_slice(&0u16.to_le_bytes());
    }
    Ok(out)
}

/// The outward normal of the triangle `(a, b, c)`, counter-clockwise seen
/// from outside as every [`TriMesh`] triangle is: `(b − a) × (c − a)`,
/// normalised. The zero vector for a degenerate triangle — a valid mesh
/// carries none, since tessellation drops a collapsed one, but nothing
/// here divides by zero on one that does.
fn facet_normal(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> [f64; 3] {
    let ab = sub(b, a);
    let ac = sub(c, a);
    let n = cross(ab, ac);
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len == 0.0 {
        [0.0, 0.0, 0.0]
    } else {
        [n[0] / len, n[1] / len, n[2] / len]
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// A finite real in ASCII STL's spelling: Rust's shortest round-trip
/// decimal with a decimal point always present, mirroring `step::real`.
fn real(x: f64) -> String {
    let x = if x == 0.0 { 0.0 } else { x };
    let s = format!("{x}");
    if s.contains('.') { s } else { format!("{s}.") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle() -> TriMesh {
        let mut m = TriMesh::new();
        for p in [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            m.push_position(p).unwrap();
        }
        m.push_triangle([0, 1, 2]).unwrap();
        m
    }

    #[test]
    fn ascii_facet_normal_is_the_windings() {
        let text = write_ascii(&triangle(), "t").unwrap();
        assert!(text.contains("facet normal 0. 0. 1."), "{text}");
    }

    #[test]
    fn binary_facet_normal_is_f32() {
        let bytes = write_binary(&triangle(), "t").unwrap();
        let facet = &bytes[84..134];
        let nx = f32::from_le_bytes(facet[0..4].try_into().unwrap());
        let ny = f32::from_le_bytes(facet[4..8].try_into().unwrap());
        let nz = f32::from_le_bytes(facet[8..12].try_into().unwrap());
        assert_eq!([nx, ny, nz], [0.0, 0.0, 1.0]);
        let attribute = u16::from_le_bytes(facet[48..50].try_into().unwrap());
        assert_eq!(attribute, 0);
    }

    #[test]
    fn a_degenerate_triangle_writes_the_zero_normal_not_nan() {
        let mut m = TriMesh::new();
        m.push_position([0.0, 0.0, 0.0]).unwrap();
        m.push_position([1.0, 0.0, 0.0]).unwrap();
        m.push_triangle([0, 1, 0]).unwrap();
        let text = write_ascii(&m, "t").unwrap();
        assert!(text.contains("facet normal 0. 0. 0."), "{text}");
    }

    #[test]
    fn a_name_longer_than_the_header_is_truncated_not_an_error() {
        let name = "x".repeat(200);
        let bytes = write_binary(&triangle(), &name).unwrap();
        assert_eq!(&bytes[0..80], "x".repeat(80).as_bytes());
    }

    #[test]
    fn reals_are_shortest_round_trip_with_a_point() {
        assert_eq!(real(40.0), "40.");
        assert_eq!(real(0.5), "0.5");
        assert_eq!(real(-0.0), "0.");
    }
}
