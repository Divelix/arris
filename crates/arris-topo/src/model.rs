//! The `Model`: the arena every entity and geometry value of a document
//! lives in (`docs/01-architecture.md` §The model).

use arris_geom::{Curve, Curve2, Surface};
use arris_math::Precision;

use crate::arena::Arena;
use crate::entity::{Body, Edge, Face, Shell, Vertex};
use crate::error::{NotFound, TopoError};
use crate::id::{BodyId, Curve2Id, CurveId, EdgeId, FaceId, ShellId, SurfaceId, VertexId};

/// The arena: every vertex, edge, face, shell, body, curve, surface and
/// pcurve ever created in it, each behind a typed generational id.
///
/// Guarantees: entities and geometry are immutable once inserted and the
/// arena is append-only, so an older handle stays valid and two bodies
/// that share an entity share its id; ids are minted sequentially in
/// creation order, identically on every platform; an accessor returns
/// [`NotFound`] for an index past the arena or a stale generation, never
/// another entity. `Clone` copies a list of chunk pointers and the first
/// append after a clone copies only the tail chunk ([`CHUNK_SIZE`]
/// slots), so a clone for a background evaluation is cheap and the two
/// then diverge without touching each other.
///
/// Nothing appends topology except the raw insert ([`Model::raw`], test
/// scaffolding that checks nothing), the builder and `import`; geometry
/// is appended by value through [`Model::add_curve`] and its siblings and
/// is never deduplicated.
///
/// ```
/// use arris_topo::entity::Vertex;
/// use arris_topo::{Model, VertexId};
/// use arris_topo::arris_math::{Point3, Precision};
///
/// let mut m = Model::new(Precision::DEFAULT).unwrap();
/// let v = m.raw().add_vertex(Vertex::new(Point3::origin(), 1e-7));
/// assert_eq!(v, VertexId::new(0, 0));
/// assert_eq!(m.vertex(v).unwrap().point(), Point3::origin());
/// assert!(m.vertex(VertexId::new(0, 1)).is_err());
/// ```
///
/// [`CHUNK_SIZE`]: crate::CHUNK_SIZE
#[derive(Debug, Clone)]
pub struct Model {
    precision: Precision,
    vertices: Arena<Vertex>,
    edges: Arena<Edge>,
    faces: Arena<Face>,
    shells: Arena<Shell>,
    bodies: Arena<Body>,
    curves: Arena<Curve>,
    surfaces: Arena<Surface>,
    curve2s: Arena<Curve2>,
}

macro_rules! accessors {
    ($( $(#[$doc:meta])* $name:ident: $arena:ident, $id:ident => $ty:ty ),* $(,)?) => {$(
        $(#[$doc])*
        ///
        /// Errors: [`NotFound`] when the index is past the arena, the slot
        /// was freed, or the generation is not the slot's.
        pub fn $name(&self, id: $id) -> Result<&$ty, NotFound> {
            self.$arena
                .get(id.index(), id.generation())
                .ok_or_else(|| NotFound::new(id))
        }
    )*};
}

impl Model {
    /// An empty model with `precision`. Errors: [`TopoError::Precision`]
    /// when the precision is not consistent.
    pub fn new(precision: Precision) -> Result<Model, TopoError> {
        if !precision.is_consistent() {
            return Err(TopoError::Precision(precision));
        }
        Ok(Model {
            precision,
            vertices: Arena::default(),
            edges: Arena::default(),
            faces: Arena::default(),
            shells: Arena::default(),
            bodies: Arena::default(),
            curves: Arena::default(),
            surfaces: Arena::default(),
            curve2s: Arena::default(),
        })
    }

    /// The tolerance configuration this model was created with.
    pub const fn precision(&self) -> Precision {
        self.precision
    }

    accessors! {
        /// The vertex under `id`.
        vertex: vertices, VertexId => Vertex,
        /// The edge under `id`.
        edge: edges, EdgeId => Edge,
        /// The face under `id`.
        face: faces, FaceId => Face,
        /// The shell under `id`.
        shell: shells, ShellId => Shell,
        /// The body under `id`.
        body: bodies, BodyId => Body,
        /// The curve under `id`.
        curve: curves, CurveId => Curve,
        /// The surface under `id`.
        surface: surfaces, SurfaceId => Surface,
        /// The pcurve under `id`.
        curve2: curve2s, Curve2Id => Curve2,
    }

    /// Stores `curve` and returns its id. Values are never deduplicated:
    /// two equal curves inserted twice get two ids.
    pub fn add_curve(&mut self, curve: Curve) -> CurveId {
        let (i, g) = self.curves.push(curve);
        CurveId::new(i, g)
    }

    /// Stores `surface` and returns its id. Never deduplicated.
    pub fn add_surface(&mut self, surface: Surface) -> SurfaceId {
        let (i, g) = self.surfaces.push(surface);
        SurfaceId::new(i, g)
    }

    /// Stores `pcurve` and returns its id. Never deduplicated.
    pub fn add_curve2(&mut self, pcurve: Curve2) -> Curve2Id {
        let (i, g) = self.curve2s.push(pcurve);
        Curve2Id::new(i, g)
    }

    /// The raw insert API: appends entities exactly as given, with no
    /// check that their references resolve or their tolerances are
    /// ordered. Test scaffolding — the checker's tests build their
    /// violations through it — and never an operation's path into the
    /// arena.
    pub fn raw(&mut self) -> RawInsert<'_> {
        RawInsert { model: self }
    }

    pub(crate) fn push_vertex(&mut self, vertex: Vertex) -> VertexId {
        let (i, g) = self.vertices.push(vertex);
        VertexId::new(i, g)
    }

    pub(crate) fn push_edge(&mut self, edge: Edge) -> EdgeId {
        let (i, g) = self.edges.push(edge);
        EdgeId::new(i, g)
    }

    pub(crate) fn push_face(&mut self, face: Face) -> FaceId {
        let (i, g) = self.faces.push(face);
        FaceId::new(i, g)
    }

    pub(crate) fn push_shell(&mut self, shell: Shell) -> ShellId {
        let (i, g) = self.shells.push(shell);
        ShellId::new(i, g)
    }

    pub(crate) fn push_body(&mut self, body: Body) -> BodyId {
        let (i, g) = self.bodies.push(body);
        BodyId::new(i, g)
    }
}

impl Default for Model {
    /// A model over [`Precision::DEFAULT`].
    fn default() -> Self {
        Model::new(Precision::DEFAULT).expect("Precision::DEFAULT is consistent")
    }
}

/// Unchecked appends into a [`Model`]; see [`Model::raw`]. A reference
/// that does not resolve is stored as given — it is what the checker's M1
/// row reports.
#[derive(Debug)]
pub struct RawInsert<'a> {
    model: &'a mut Model,
}

impl RawInsert<'_> {
    /// Appends `vertex` and returns its id.
    pub fn add_vertex(&mut self, vertex: Vertex) -> VertexId {
        self.model.push_vertex(vertex)
    }

    /// Appends `edge` and returns its id.
    pub fn add_edge(&mut self, edge: Edge) -> EdgeId {
        self.model.push_edge(edge)
    }

    /// Appends `face` and returns its id.
    pub fn add_face(&mut self, face: Face) -> FaceId {
        self.model.push_face(face)
    }

    /// Appends `shell` and returns its id.
    pub fn add_shell(&mut self, shell: Shell) -> ShellId {
        self.model.push_shell(shell)
    }

    /// Appends `body` and returns its id.
    pub fn add_body(&mut self, body: Body) -> BodyId {
        self.model.push_body(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::CHUNK_SIZE;
    use arris_math::Point3;

    fn with_vertices(n: usize) -> Model {
        let mut m = Model::default();
        for i in 0..n {
            m.raw()
                .add_vertex(Vertex::new(Point3::new(i as f64, 0.0, 0.0), 1e-7));
        }
        m
    }

    #[test]
    fn a_clone_shares_every_chunk_and_the_first_append_copies_the_tail() {
        let a = with_vertices(CHUNK_SIZE + 1);
        let mut b = a.clone();
        assert!(a.vertices.shares_chunk(&b.vertices, 0));
        assert!(a.vertices.shares_chunk(&b.vertices, 1));
        let id = b.raw().add_vertex(Vertex::new(Point3::origin(), 1e-7));
        assert_eq!(id, VertexId::new(CHUNK_SIZE as u32 + 1, 0));
        assert!(
            a.vertices.shares_chunk(&b.vertices, 0),
            "the full chunk is untouched"
        );
        assert!(
            !a.vertices.shares_chunk(&b.vertices, 1),
            "only the tail chunk was copied"
        );
        assert!(a.vertex(id).is_err(), "the original never saw the append");
        assert!(b.vertex(id).is_ok());
    }

    #[test]
    fn model_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Model>();
    }
}
