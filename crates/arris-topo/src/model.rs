//! The `Model`: the arena every entity and geometry value of a document
//! lives in (`docs/01-architecture.md` §The model).

use core::fmt;
use std::sync::Arc;

use arris_geom::{Curve, Curve2, Surface};
use arris_math::Precision;

use crate::arena::Arena;
use crate::entity::{Body, Edge, Face, Shell, Vertex};
use crate::error::{NotFound, TopoError};
use crate::id::{BodyId, Curve2Id, CurveId, EdgeId, FaceId, ShellId, SurfaceId, VertexId};

/// One use of an edge: the face, the loop within it and the coedge within
/// the loop. What the edge → coedge index answers with; loops and coedges
/// are not entities, so this is their address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoedgeRef {
    /// The face whose loop uses the edge.
    pub face: FaceId,
    /// Which loop of the face.
    pub loop_index: usize,
    /// Which coedge of the loop.
    pub coedge_index: usize,
}

impl fmt::Display for CoedgeRef {
    /// `f3/0/2`: face, loop, coedge.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}/{}", self.face, self.loop_index, self.coedge_index)
    }
}

/// The derived adjacency, keyed by slot index and maintained on every
/// append (`docs/02-data-model.md` §Adjacency and iteration). Shared by
/// clones and copied whole on the first append after a clone.
#[derive(Debug, Clone, Default)]
struct Indices {
    /// Edge index → its uses, in the creation order of the faces.
    edge_uses: Vec<Vec<CoedgeRef>>,
    /// Vertex index → the edges that end at it, in creation order, each
    /// once (a closed edge is listed once).
    vertex_edges: Vec<Vec<EdgeId>>,
    /// Face index → the shells that use it, in creation order.
    face_shells: Vec<Vec<ShellId>>,
}

/// The arena lengths at a transaction's entry: what a rollback restores.
#[derive(Debug, Clone, Copy)]
struct Lengths {
    vertices: usize,
    edges: usize,
    faces: usize,
    shells: usize,
    bodies: usize,
    curves: usize,
    surfaces: usize,
    curve2s: usize,
}

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
    indices: Arc<Indices>,
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
            indices: Arc::default(),
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

    /// Every use of `edge` by a loop, in the creation order of the faces:
    /// two for an edge between two faces of a solid, two in one face for a
    /// seam, one for a boundary edge of a sheet. Model-wide — an edge
    /// shared by several bodies lists every body's uses; filter through
    /// [`Model::closure`] for one body. Errors: the edge does not resolve.
    pub fn edge_uses(&self, edge: EdgeId) -> Result<&[CoedgeRef], NotFound> {
        self.edge(edge)?;
        Ok(self
            .indices
            .edge_uses
            .get(edge.index() as usize)
            .map_or(&[], Vec::as_slice))
    }

    /// Every edge that starts or ends at `vertex`, in creation order, each
    /// once. Model-wide. Errors: the vertex does not resolve.
    pub fn vertex_edges(&self, vertex: VertexId) -> Result<&[EdgeId], NotFound> {
        self.vertex(vertex)?;
        Ok(self
            .indices
            .vertex_edges
            .get(vertex.index() as usize)
            .map_or(&[], Vec::as_slice))
    }

    /// Every shell that uses `face`, in creation order. Model-wide.
    /// Errors: the face does not resolve.
    pub fn face_shells(&self, face: FaceId) -> Result<&[ShellId], NotFound> {
        self.face(face)?;
        Ok(self
            .indices
            .face_shells
            .get(face.index() as usize)
            .map_or(&[], Vec::as_slice))
    }

    /// Runs `f` on the model and, if it returns `Err`, drops every entity
    /// and geometry value it appended — arenas, indices and the next id
    /// all return to what they were at entry — so a failed operation
    /// leaves the model as it was (`docs/01-architecture.md` §The model).
    /// On `Ok` everything stays. Transactions nest: an inner `Err` undoes
    /// the inner appends only.
    ///
    /// ```
    /// use arris_topo::entity::Vertex;
    /// use arris_topo::{Model, VertexId};
    /// use arris_topo::arris_math::Point3;
    ///
    /// let mut m = Model::default();
    /// let r: Result<(), &str> = m.transaction(|m| {
    ///     m.raw().add_vertex(Vertex::new(Point3::origin(), 1e-7));
    ///     Err("changed my mind")
    /// });
    /// assert!(r.is_err());
    /// assert!(m.vertex(VertexId::new(0, 0)).is_err());
    /// let v = m.raw().add_vertex(Vertex::new(Point3::origin(), 1e-7));
    /// assert_eq!(v, VertexId::new(0, 0), "the id was not consumed");
    /// ```
    pub fn transaction<T, E>(
        &mut self,
        f: impl FnOnce(&mut Model) -> Result<T, E>,
    ) -> Result<T, E> {
        let at_entry = self.lengths();
        let result = f(self);
        if result.is_err() {
            self.rollback(at_entry);
        }
        result
    }

    fn lengths(&self) -> Lengths {
        Lengths {
            vertices: self.vertices.len(),
            edges: self.edges.len(),
            faces: self.faces.len(),
            shells: self.shells.len(),
            bodies: self.bodies.len(),
            curves: self.curves.len(),
            surfaces: self.surfaces.len(),
            curve2s: self.curve2s.len(),
        }
    }

    /// Undoes every append since `at`. The index entries an appended
    /// entity made are the tails of the lists they went into, because
    /// lists grow in creation order; so each appended entity's references
    /// are popped back off, then everything is truncated.
    fn rollback(&mut self, at: Lengths) {
        let removed_edges: Vec<Edge> = (at.edges..self.edges.len())
            .filter_map(|i| self.edges.value_at(i).copied())
            .collect();
        let removed_faces: Vec<Face> = (at.faces..self.faces.len())
            .filter_map(|i| self.faces.value_at(i).cloned())
            .collect();
        let removed_shells: Vec<Shell> = (at.shells..self.shells.len())
            .filter_map(|i| self.shells.value_at(i).cloned())
            .collect();
        let indices = Arc::make_mut(&mut self.indices);
        for edge in &removed_edges {
            for v in [edge.start(), edge.end()] {
                if let Some(list) = indices.vertex_edges.get_mut(v.index() as usize) {
                    while list.last().is_some_and(|e| e.index() as usize >= at.edges) {
                        list.pop();
                    }
                }
            }
        }
        for face in &removed_faces {
            for coedge in face.loops().iter().flat_map(|l| l.coedges()) {
                if let Some(list) = indices.edge_uses.get_mut(coedge.edge().index() as usize) {
                    while list
                        .last()
                        .is_some_and(|u| u.face.index() as usize >= at.faces)
                    {
                        list.pop();
                    }
                }
            }
        }
        for shell in &removed_shells {
            for face in shell.faces() {
                if let Some(list) = indices.face_shells.get_mut(face.id.index() as usize) {
                    while list.last().is_some_and(|s| s.index() as usize >= at.shells) {
                        list.pop();
                    }
                }
            }
        }
        indices.vertex_edges.truncate(at.vertices);
        indices.edge_uses.truncate(at.edges);
        indices.face_shells.truncate(at.faces);
        self.vertices.truncate(at.vertices);
        self.edges.truncate(at.edges);
        self.faces.truncate(at.faces);
        self.shells.truncate(at.shells);
        self.bodies.truncate(at.bodies);
        self.curves.truncate(at.curves);
        self.surfaces.truncate(at.surfaces);
        self.curve2s.truncate(at.curve2s);
    }

    pub(crate) fn push_vertex(&mut self, vertex: Vertex) -> VertexId {
        let (i, g) = self.vertices.push(vertex);
        Arc::make_mut(&mut self.indices)
            .vertex_edges
            .push(Vec::new());
        VertexId::new(i, g)
    }

    pub(crate) fn push_edge(&mut self, edge: Edge) -> EdgeId {
        let ends: Vec<VertexId> = [edge.start(), edge.end()]
            .into_iter()
            .filter(|&v| self.vertex(v).is_ok())
            .collect();
        let (i, g) = self.edges.push(edge);
        let id = EdgeId::new(i, g);
        let indices = Arc::make_mut(&mut self.indices);
        for v in ends {
            let list = &mut indices.vertex_edges[v.index() as usize];
            if list.last() != Some(&id) {
                list.push(id);
            }
        }
        indices.edge_uses.push(Vec::new());
        id
    }

    pub(crate) fn push_face(&mut self, face: Face) -> FaceId {
        let uses: Vec<(EdgeId, usize, usize)> = face
            .loops()
            .iter()
            .enumerate()
            .flat_map(|(li, l)| {
                l.coedges()
                    .iter()
                    .enumerate()
                    .map(move |(ci, c)| (c.edge(), li, ci))
            })
            .filter(|&(e, _, _)| self.edge(e).is_ok())
            .collect();
        let (i, g) = self.faces.push(face);
        let id = FaceId::new(i, g);
        let indices = Arc::make_mut(&mut self.indices);
        for (edge, loop_index, coedge_index) in uses {
            indices.edge_uses[edge.index() as usize].push(CoedgeRef {
                face: id,
                loop_index,
                coedge_index,
            });
        }
        indices.face_shells.push(Vec::new());
        id
    }

    pub(crate) fn push_shell(&mut self, shell: Shell) -> ShellId {
        let faces: Vec<FaceId> = shell
            .faces()
            .iter()
            .map(|f| f.id)
            .filter(|&f| self.face(f).is_ok())
            .collect();
        let (i, g) = self.shells.push(shell);
        let id = ShellId::new(i, g);
        let indices = Arc::make_mut(&mut self.indices);
        for f in faces {
            indices.face_shells[f.index() as usize].push(id);
        }
        id
    }

    pub(crate) fn push_body(&mut self, body: Body) -> BodyId {
        let (i, g) = self.bodies.push(body);
        BodyId::new(i, g)
    }
}

/// One slot of an arena as the native format stores it: the generation,
/// and the value when the slot is live.
#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
struct SlotRepr<T> {
    generation: u32,
    value: Option<T>,
}

/// The wire form of a [`Model`] (`docs/02-data-model.md` §Native format):
/// the precision, then every arena's slots in index order, freed ones
/// included, so the model read back has the same ids and mints the same
/// next one. The adjacency indices are derived and rebuilt on the way in.
#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
struct ModelRepr {
    precision: Precision,
    vertices: Vec<SlotRepr<Vertex>>,
    edges: Vec<SlotRepr<Edge>>,
    faces: Vec<SlotRepr<Face>>,
    shells: Vec<SlotRepr<Shell>>,
    bodies: Vec<SlotRepr<Body>>,
    curves: Vec<SlotRepr<Curve>>,
    surfaces: Vec<SlotRepr<Surface>>,
    curve2s: Vec<SlotRepr<Curve2>>,
}

#[cfg(feature = "serde")]
fn slots_of<T: Clone>(arena: &Arena<T>) -> Vec<SlotRepr<T>> {
    arena
        .slots()
        .map(|(generation, value)| SlotRepr {
            generation,
            value: value.cloned(),
        })
        .collect()
}

#[cfg(feature = "serde")]
impl serde::Serialize for Model {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ModelRepr {
            precision: self.precision,
            vertices: slots_of(&self.vertices),
            edges: slots_of(&self.edges),
            faces: slots_of(&self.faces),
            shells: slots_of(&self.shells),
            bodies: slots_of(&self.bodies),
            curves: slots_of(&self.curves),
            surfaces: slots_of(&self.surfaces),
            curve2s: slots_of(&self.curve2s),
        }
        .serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Model {
    /// Rejects an inconsistent precision as [`TopoError::Precision`]
    /// would; every entity is stored as read (a dangling reference is the
    /// checker's M1 to report), and the indices are rebuilt from the
    /// entities in slot order.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let r = ModelRepr::deserialize(deserializer)?;
        let mut m = Model::new(r.precision).map_err(serde::de::Error::custom)?;
        for s in r.vertices {
            m.vertices.push_slot(s.generation, s.value);
        }
        for s in r.edges {
            m.edges.push_slot(s.generation, s.value);
        }
        for s in r.faces {
            m.faces.push_slot(s.generation, s.value);
        }
        for s in r.shells {
            m.shells.push_slot(s.generation, s.value);
        }
        for s in r.bodies {
            m.bodies.push_slot(s.generation, s.value);
        }
        for s in r.curves {
            m.curves.push_slot(s.generation, s.value);
        }
        for s in r.surfaces {
            m.surfaces.push_slot(s.generation, s.value);
        }
        for s in r.curve2s {
            m.curve2s.push_slot(s.generation, s.value);
        }
        m.rebuild_indices();
        Ok(m)
    }
}

impl Model {
    /// Recomputes the adjacency indices from the entities in slot order:
    /// what a model read from the native format does, and what `retain`
    /// does once it has freed slots. Lists come out in slot order, which
    /// is creation order for a model that never freed a slot.
    pub(crate) fn rebuild_indices(&mut self) {
        let mut indices = Indices {
            vertex_edges: vec![Vec::new(); self.vertices.len()],
            edge_uses: vec![Vec::new(); self.edges.len()],
            face_shells: vec![Vec::new(); self.faces.len()],
        };
        for (i, edge) in self.edges.slots().enumerate() {
            let (generation, Some(edge)) = edge else {
                continue;
            };
            let id = EdgeId::new(i as u32, generation);
            for v in [edge.start(), edge.end()] {
                if self.vertex(v).is_ok() {
                    let list = &mut indices.vertex_edges[v.index() as usize];
                    if list.last() != Some(&id) {
                        list.push(id);
                    }
                }
            }
        }
        for (i, face) in self.faces.slots().enumerate() {
            let (generation, Some(face)) = face else {
                continue;
            };
            let id = FaceId::new(i as u32, generation);
            for (loop_index, l) in face.loops().iter().enumerate() {
                for (coedge_index, c) in l.coedges().iter().enumerate() {
                    if self.edge(c.edge()).is_ok() {
                        indices.edge_uses[c.edge().index() as usize].push(CoedgeRef {
                            face: id,
                            loop_index,
                            coedge_index,
                        });
                    }
                }
            }
        }
        for (i, shell) in self.shells.slots().enumerate() {
            let (generation, Some(shell)) = shell else {
                continue;
            };
            let id = ShellId::new(i as u32, generation);
            for f in shell.faces() {
                if self.face(f.id).is_ok() {
                    indices.face_shells[f.id.index() as usize].push(id);
                }
            }
        }
        self.indices = Arc::new(indices);
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
