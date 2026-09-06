//! Streaming a body to a Rerun viewer — for the human, never the agent
//! (`inspect`'s PNGs are what the agent reads). Feature-gated: `rerun`
//! pulls in the `rerun` SDK crate and is never enabled on `wasm32`.

use arris_topo::{Body, Model};

use crate::body::{DebugMeshError, mesh_of};
use crate::render::face_color;

/// A connection to a Rerun viewer, from [`spawn`]. [`log`] sends to it.
pub type Stream = ::rerun::RecordingStream;

/// Why a body could not stream to Rerun.
#[derive(Debug, thiserror::Error)]
pub enum RerunError {
    /// The body could not be meshed.
    #[error(transparent)]
    Mesh(#[from] DebugMeshError),
    /// The Rerun SDK could not send the log data.
    #[error(transparent)]
    Send(#[from] ::rerun::RecordingStreamError),
}

/// Spawns a Rerun Viewer process (or connects to one already listening)
/// and returns the [`Stream`] [`log`] sends to.
///
/// ```no_run
/// let rec = arris_debug::rerun::spawn().unwrap();
/// ```
pub fn spawn() -> Result<Stream, ::rerun::RecordingStreamError> {
    ::rerun::RecordingStreamBuilder::new("arris").spawn()
}

/// Streams `body`'s mesh ([`mesh_of`]'s chord) to `rec`: an
/// [`rerun::Mesh3D`] per face, coloured by id with
/// [`crate::render::face_color`], under `<body>/faces/<face>`; an
/// [`rerun::LineStrips3D`] per edge under `<body>/edges/<edge>`; and one
/// [`rerun::Points3D`] of every mesh vertex under `<body>/vertices` —
/// three layers a viewer toggles independently.
///
/// ```no_run
/// use arris_debug::sample;
/// use arris_topo::Model;
///
/// let mut m = Model::default();
/// let cylinder = sample::cylinder(&mut m, 4.0, 12.0).unwrap();
/// let rec = arris_debug::rerun::spawn().unwrap();
/// arris_debug::rerun::log(&rec, &m, cylinder).unwrap();
/// ```
pub fn log(rec: &Stream, m: &Model, body: Body) -> Result<(), RerunError> {
    let mesh = mesh_of(m, body)?;
    let positions: Vec<[f32; 3]> = mesh.positions().iter().map(|&p| to_f32(p)).collect();
    let root = body.id;

    for range in mesh.faces() {
        let triangles = mesh.triangles()[range.triangles.clone()].to_vec();
        let color = face_color(range.face);
        let archetype = ::rerun::Mesh3D::new(positions.clone())
            .with_triangle_indices(triangles)
            .with_albedo_factor(color);
        rec.log(format!("{root}/faces/{}", range.face), &archetype)?;
    }

    for range in mesh.edges() {
        let strip: Vec<[f32; 3]> = mesh.edge_indices()[range.indices.clone()]
            .iter()
            .map(|&i| positions[i as usize])
            .collect();
        rec.log(
            format!("{root}/edges/{}", range.edge),
            &::rerun::LineStrips3D::new([strip]),
        )?;
    }

    rec.log(
        format!("{root}/vertices"),
        &::rerun::Points3D::new(positions),
    )?;

    Ok(())
}

fn to_f32(p: [f64; 3]) -> [f32; 3] {
    [p[0] as f32, p[1] as f32, p[2] as f32]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use arris_topo::Model;

    use super::*;
    use crate::sample;

    /// One [`rerun::Mesh3D`] chunk per face, one [`rerun::LineStrips3D`]
    /// chunk per edge, one [`rerun::Points3D`] chunk for every vertex,
    /// each under the path the doc states, and nothing else besides the
    /// stream's own bookkeeping entities.
    #[test]
    fn logging_the_cylinder_yields_one_row_per_entity() {
        let mut m = Model::default();
        let cyl = sample::cylinder(&mut m, 4.0, 12.0).unwrap();
        let root = cyl.id;

        let (rec, storage) = ::rerun::RecordingStreamBuilder::new("arris-test")
            .memory()
            .unwrap();
        log(&rec, &m, cyl).unwrap();

        let mut by_path: BTreeSet<String> = BTreeSet::new();
        for msg in storage.take() {
            if let ::rerun::log::LogMsg::ArrowMsg(_, arrow_msg) = msg {
                let chunk = ::rerun::log::Chunk::from_arrow_msg(&arrow_msg).unwrap();
                let path = chunk.entity_path().to_string();
                if path.starts_with(&format!("/{root}")) {
                    assert_eq!(chunk.num_rows(), 1, "{path} logged more than one row");
                    assert!(by_path.insert(path), "a path logged twice");
                }
            }
        }

        let faces = m.faces(cyl).unwrap();
        let edges = m.edges(cyl).unwrap();
        assert_eq!(by_path.len(), faces.len() + edges.len() + 1);
        for f in &faces {
            assert!(by_path.contains(&format!("/{root}/faces/{}", f.id)));
        }
        for e in &edges {
            assert!(by_path.contains(&format!("/{root}/edges/{}", e.id)));
        }
        assert!(by_path.contains(&format!("/{root}/vertices")));
    }
}
