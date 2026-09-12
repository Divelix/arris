//! `transform`: a rigid motion of a body (`docs/ARCHITECTURE.md`
//! §Operations).

use std::collections::BTreeMap;

use arris_check::arris_topo::arris_math::Isometry;
use arris_check::arris_topo::builder::{
    Assembly, Builder, EdgeKey, EdgeSpec, FaceSpec, UseSpec, VertexKey, VertexSpec,
};
use arris_check::arris_topo::entity::EdgeGeometry;
use arris_check::arris_topo::{Body, EntityId, Model, Orientation, Provenance, Shape};

use crate::error::OpError;
use crate::verify;

/// Moves `body` rigidly by `motion`: every curve and surface appended
/// transformed (`Curve::transformed`, `Surface::transformed` carry the
/// parametrisation along, so every pcurve id is reused as it stands),
/// every vertex, edge, face, shell and the body itself appended new
/// through [`Builder::assemble`] in the body's own iteration order, each
/// recorded `Modified` one-to-one from the entity it moved. The body's
/// kind is kept.
///
/// Built over `assemble`, so it reaches exactly as far as that does: a
/// body reduced entirely to shells of faces that share nothing (a
/// `Solid`, the one kind [`Builder::finish`] builds and every operation
/// produces today) moves whole, every shell of it carried to a shell of
/// the result in the body's stored order; one that is not comes back as
/// [`OpError::Internal`] naming the builder's refusal, the same as
/// `assemble`'s own.
///
/// Errors: [`OpError::InvalidInput`] when `body` fails the checker (debug
/// builds, and release with the `paranoid` feature); [`OpError::NotFound`]
/// when it does not resolve. The model is untouched on error.
///
/// ```
/// use arris_ops::{primitive_cylinder, transform};
/// use arris_ops::arris_check::arris_topo::{Model, Shape};
/// use arris_ops::arris_check::arris_topo::arris_math::{Axis, Isometry, Point3, Vec3};
///
/// let mut m = Model::default();
/// let (body, _) = primitive_cylinder(&mut m, Axis::z_at(Point3::origin()), 4.0, 12.0).unwrap();
/// let motion = Isometry::from_translation(Vec3::new(1.0, 2.0, 3.0));
/// let (moved, provenance) = transform(&mut m, body, &motion).unwrap();
/// assert_eq!(m.faces(moved).unwrap().len(), 3);
/// assert_eq!(provenance.modified_from(Shape::from(body)).len(), 1);
/// ```
pub fn transform(
    m: &mut Model,
    body: Body,
    motion: &Isometry,
) -> Result<(Body, Provenance), OpError> {
    let not_found = || OpError::NotFound(Shape::new(body.id, body.orientation));
    crate::verify_input(m, body)?;
    let entity = m.body(body.id).map_err(|_| not_found())?.clone();
    // Every shell use with its faces seen through it, in stored order.
    let mut shells = Vec::new();
    for shell in m.shells(body).map_err(|_| not_found())? {
        let faces: Vec<_> = m
            .shell(shell.id)
            .map_err(|_| not_found())?
            .faces()
            .iter()
            .map(|f| f.oriented_by(shell.orientation))
            .collect();
        shells.push((shell, faces));
    }
    let closure = m.closure(body).map_err(|_| not_found())?;
    let tolerance = m.precision().default_tolerance;

    m.transaction(|m| {
        let mut curve_of = BTreeMap::new();
        for &c in &closure.curves {
            let moved = m.curve(c).map_err(|_| not_found())?.transformed(motion);
            curve_of.insert(c, m.add_curve(moved));
        }
        let mut surface_of = BTreeMap::new();
        for &s in &closure.surfaces {
            let moved = m.surface(s).map_err(|_| not_found())?.transformed(motion);
            surface_of.insert(s, m.add_surface(moved));
        }

        let mut vertex_index = BTreeMap::new();
        let mut vertices = Vec::with_capacity(closure.vertices.len());
        for (i, &v) in closure.vertices.iter().enumerate() {
            let old = *m.vertex(v).map_err(|_| not_found())?;
            vertices.push(VertexSpec::New {
                point: motion.apply(old.point()),
                tolerance: old.tolerance(),
            });
            vertex_index.insert(v, i);
        }

        let mut edge_index = BTreeMap::new();
        let mut edges = Vec::with_capacity(closure.edges.len());
        for (i, &e) in closure.edges.iter().enumerate() {
            let old = *m.edge(e).map_err(|_| not_found())?;
            let geometry = match old.geometry() {
                EdgeGeometry::Curve { curve, range } => EdgeGeometry::Curve {
                    curve: curve_of[&curve],
                    range,
                },
                degenerate @ EdgeGeometry::Degenerate { .. } => degenerate,
            };
            edges.push(EdgeSpec::New {
                geometry,
                start: VertexKey::New(vertex_index[&old.start()]),
                end: VertexKey::New(vertex_index[&old.end()]),
                tolerance: old.tolerance(),
            });
            edge_index.insert(e, i);
        }

        let mut shell_specs = Vec::with_capacity(shells.len());
        for (_, faces) in &shells {
            let mut face_specs = Vec::with_capacity(faces.len());
            for f in faces {
                let old = m.face(f.id).map_err(|_| not_found())?.clone();
                let loops = old
                    .loops()
                    .iter()
                    .map(|l| {
                        // `UseSpec::orientation` is the *effective* direction,
                        // as seen from outside the material — `f.orientation`
                        // composed with the coedge's own, the loop reversed to
                        // match when the face itself is reversed (the same
                        // conversion `Builder::assemble`'s `Keep` case
                        // applies).
                        let mut uses: Vec<UseSpec> = l
                            .coedges()
                            .iter()
                            .map(|c| UseSpec {
                                edge: EdgeKey::New(edge_index[&c.edge()]),
                                orientation: f.orientation.compose(c.orientation()),
                                pcurve: c.pcurve(),
                            })
                            .collect();
                        if f.orientation.is_reversed() {
                            uses.reverse();
                        }
                        uses
                    })
                    .collect();
                face_specs.push(FaceSpec::New {
                    surface: surface_of[&old.surface()],
                    orientation: f.orientation,
                    loops,
                    tolerance: old.tolerance(),
                });
            }
            shell_specs.push(face_specs);
        }

        let assembly = Assembly {
            vertices,
            edges,
            shells: shell_specs,
        };
        let b = Builder::assemble(m, tolerance, assembly)?;
        let built = b.finish(m, entity.kind())?;

        fn forward(id: impl Into<EntityId>) -> Shape {
            Shape::new(id, Orientation::Forward)
        }
        let mut provenance = Provenance::new();
        for (&old, &new) in closure.vertices.iter().zip(built.vertices.values()) {
            provenance.add_modified(forward(old), forward(new));
        }
        for (&old, &new) in closure.edges.iter().zip(built.edges.values()) {
            provenance.add_modified(forward(old), forward(new));
        }
        // `assemble` makes one face slot per spec in spec order, and one
        // shell per assembly shell in order, so both zip.
        let faces = shells.iter().flat_map(|(_, faces)| faces);
        for (old, &new) in faces.zip(built.faces.values()) {
            provenance.add_modified(forward(old.id), forward(new));
        }
        for ((old, _), &new) in shells.iter().zip(&built.shells) {
            provenance.add_modified(forward(old.id), forward(new));
        }
        provenance.add_modified(forward(body.id), forward(built.body.id));

        verify(m, built.body)?;
        Ok((built.body, provenance))
    })
}
