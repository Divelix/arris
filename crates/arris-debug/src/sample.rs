//! Valid bodies built by hand through the raw insert API, with explicit
//! pcurves: what the checker's tests start from (they cannot use
//! `arris-ops`) and what the STEP writer is first tried on. Each is the
//! shape the matching `primitive/*` fixture describes, built with the
//! conventions the primitives of `arris-ops` follow, so the two dump
//! alike.

use core::f64::consts::TAU;

use arris_geom::{Curve, Curve2, Surface};
use arris_math::{Frame, Frame2, FrameError, Interval, Point3, UnitVec2, UnitVec3, Vec3};
use arris_topo::entity::{
    Body as BodyEntity, Coedge, Edge, EdgeGeometry, Face, Loop, Shell, Vertex,
};
use arris_topo::{Body, Face as FaceHandle, Model, Orientation, Shell as ShellHandle};

/// Why a sample could not be built.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SampleError {
    /// An extent is not finite and positive.
    #[error("sample {name} must be finite and positive, not {value}")]
    Extent {
        /// Which extent.
        name: &'static str,
        /// What was given.
        value: f64,
    },
    /// A placing frame could not be built.
    #[error("sample frame: {0}")]
    Frame(#[from] FrameError),
}

fn positive(name: &'static str, value: f64) -> Result<f64, SampleError> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(SampleError::Extent { name, value })
    }
}

/// The unit cube `[0, 1]³`: [`cuboid`] from the origin.
pub fn unit_box(m: &mut Model) -> Result<Body, SampleError> {
    cuboid(m, Point3::origin(), Point3::new(1.0, 1.0, 1.0))
}

/// The axis-aligned box from `min` to `max` as a solid: eight vertices,
/// twelve line edges, six planar faces of one loop each. Every plane's
/// `Z` is the face's outward normal and every face is used `Forward`; a
/// loop walks its corners counter-clockwise in the plane's (u, v), so
/// each edge is used twice in opposite directions. Tolerances are the
/// model's `default_tolerance`. Errors: an extent that is not finite and
/// positive. The model is untouched on error.
///
/// ```
/// use arris_debug::sample;
/// use arris_topo::Model;
/// use arris_topo::arris_math::Point3;
///
/// let mut m = Model::default();
/// let b = sample::cuboid(&mut m, Point3::origin(), Point3::new(40.0, 30.0, 10.0)).unwrap();
/// assert_eq!(m.faces(b).unwrap().len(), 6);
/// assert_eq!(m.edges(b).unwrap().len(), 12);
/// ```
pub fn cuboid(m: &mut Model, min: Point3, max: Point3) -> Result<Body, SampleError> {
    let dx = positive("x extent", max.x - min.x)?;
    let dy = positive("y extent", max.y - min.y)?;
    let dz = positive("z extent", max.z - min.z)?;
    if !(min.coords.iter().all(|c| c.is_finite())) {
        return Err(SampleError::Extent {
            name: "min",
            value: f64::NAN,
        });
    }
    let tol = m.precision().default_tolerance;
    // Corner `i` has the bits (x, y, z) of `i`.
    let corner = |i: usize| {
        Point3::new(
            if i & 1 == 0 { min.x } else { min.x + dx },
            if i & 2 == 0 { min.y } else { min.y + dy },
            if i & 4 == 0 { min.z } else { min.z + dz },
        )
    };
    // Twelve edges, x-parallel first, then y, then z, each from the lower
    // corner to the upper.
    let ends: [(usize, usize); 12] = [
        (0, 1),
        (2, 3),
        (4, 5),
        (6, 7),
        (0, 2),
        (1, 3),
        (4, 6),
        (5, 7),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    // Six faces as corner cycles, counter-clockwise seen from outside,
    // with the outward normal: bottom, top, front, back, left, right.
    let cycles: [([usize; 4], Vec3); 6] = [
        ([0, 2, 3, 1], -Vec3::z()),
        ([4, 5, 7, 6], Vec3::z()),
        ([0, 1, 5, 4], -Vec3::y()),
        ([2, 6, 7, 3], Vec3::y()),
        ([0, 4, 6, 2], -Vec3::x()),
        ([1, 3, 7, 5], Vec3::x()),
    ];
    // Every frame first, so a frame error leaves the model untouched.
    let mut frames = Vec::with_capacity(6);
    for (cycle, normal) in &cycles {
        let o = corner(cycle[0]);
        let x_hint = corner(cycle[1]) - o;
        frames.push(Frame::new(o, *normal, x_hint)?);
    }

    let vertices: Vec<_> = (0..8)
        .map(|i| m.raw().add_vertex(Vertex::new(corner(i), tol)))
        .collect();
    let mut edges = Vec::with_capacity(12);
    for &(a, b) in &ends {
        let (pa, pb) = (corner(a), corner(b));
        let d = pb - pa;
        let length = d.norm();
        let curve = m.add_curve(Curve::Line {
            origin: pa,
            direction: UnitVec3::new_normalize(d),
        });
        let range = Interval::new(0.0, length).map_err(|_| SampleError::Extent {
            name: "edge length",
            value: length,
        })?;
        edges.push(m.raw().add_edge(Edge::new(
            EdgeGeometry::Curve { curve, range },
            vertices[a],
            vertices[b],
            tol,
        )));
    }
    let mut faces = Vec::with_capacity(6);
    for ((cycle, _), frame) in cycles.iter().zip(&frames) {
        let surface = m.add_surface(Surface::Plane { frame: *frame });
        let mut coedges = Vec::with_capacity(4);
        for k in 0..4 {
            let (a, b) = (cycle[k], cycle[(k + 1) % 4]);
            let ei = ends
                .iter()
                .position(|&(p, q)| (p, q) == (a, b) || (p, q) == (b, a))
                .ok_or(SampleError::Extent {
                    name: "box edge table",
                    value: k as f64,
                })?;
            let (ea, eb) = ends[ei];
            let orientation = if ea == a {
                Orientation::Forward
            } else {
                Orientation::Reversed
            };
            // Same-parameter: the pcurve starts where the edge's own start
            // maps into the plane and runs along the edge's own direction.
            let start = frame.to_local(corner(ea));
            let dir = frame.vec_to_local(corner(eb) - corner(ea));
            let pcurve = m.add_curve2(Curve2::Line {
                origin: arris_math::Point2::new(start.x, start.y),
                direction: UnitVec2::new_normalize(arris_math::Vec2::new(dir.x, dir.y)),
            });
            coedges.push(Coedge::new(edges[ei], orientation, pcurve));
        }
        faces.push(
            m.raw()
                .add_face(Face::new(surface, vec![Loop::new(coedges)], tol)),
        );
    }
    let shell = m.raw().add_shell(Shell::new(
        faces.iter().map(|&f| FaceHandle::forward(f)).collect(),
    ));
    let body = m
        .raw()
        .add_body(BodyEntity::solid(vec![ShellHandle::forward(shell)]));
    Ok(Body::forward(body))
}

/// A cylinder of `radius` and `height` on the `z` axis with its base at
/// the origin, as a solid: two vertices on the seam, a bottom circle, the
/// seam line, a top circle; one wall face on the cylinder surface whose
/// one loop is bottom circle, seam up, top circle, seam down — the two
/// seam pcurves at `u = 2π` and `u = 0` (`docs/02-data-model.md` §Seams);
/// two cap faces on planes whose `Z` is the axis, the bottom cap used
/// `Reversed`, as the reference tree's one-axis primitive builds it, so
/// both caps' circle pcurves are right-handed (the circle's `Z` is along
/// each plane's normal). The seam is where the surface's `X` points.
/// Tolerances are the model's `default_tolerance`. Errors: a radius or
/// height that is not finite and positive. The model is untouched on
/// error.
///
/// ```
/// use arris_debug::sample;
/// use arris_topo::Model;
///
/// let mut m = Model::default();
/// let b = sample::cylinder(&mut m, 4.0, 12.0).unwrap();
/// assert_eq!(m.faces(b).unwrap().len(), 3);
/// assert_eq!(m.edges(b).unwrap().len(), 3);
/// assert_eq!(m.vertices(b).unwrap().len(), 2);
/// ```
pub fn cylinder(m: &mut Model, radius: f64, height: f64) -> Result<Body, SampleError> {
    let radius = positive("radius", radius)?;
    let height = positive("height", height)?;
    let tol = m.precision().default_tolerance;
    let base = Frame::world();
    let top = base.with_origin(Point3::new(0.0, 0.0, height));
    let axis = Vec3::z_axis();

    let wall = m.add_surface(Surface::Cylinder {
        frame: base,
        radius,
    });
    let bottom_plane = m.add_surface(Surface::Plane { frame: base });
    let top_plane = m.add_surface(Surface::Plane { frame: top });

    let bottom_circle = m.add_curve(Curve::Circle {
        frame: base,
        radius,
    });
    let seam_line = m.add_curve(Curve::Line {
        origin: Point3::new(radius, 0.0, 0.0),
        direction: axis,
    });
    let top_circle = m.add_curve(Curve::Circle { frame: top, radius });

    let v0 = m
        .raw()
        .add_vertex(Vertex::new(Point3::new(radius, 0.0, 0.0), tol));
    let v1 = m
        .raw()
        .add_vertex(Vertex::new(Point3::new(radius, 0.0, height), tol));

    let turn = Interval::TURN;
    let rise = Interval::new(0.0, height).map_err(|_| SampleError::Extent {
        name: "height",
        value: height,
    })?;
    let e_bottom = m.raw().add_edge(Edge::new(
        EdgeGeometry::Curve {
            curve: bottom_circle,
            range: turn,
        },
        v0,
        v0,
        tol,
    ));
    let e_seam = m.raw().add_edge(Edge::new(
        EdgeGeometry::Curve {
            curve: seam_line,
            range: rise,
        },
        v0,
        v1,
        tol,
    ));
    let e_top = m.raw().add_edge(Edge::new(
        EdgeGeometry::Curve {
            curve: top_circle,
            range: turn,
        },
        v1,
        v1,
        tol,
    ));

    let uv_line = |m: &mut Model, u: f64, v: f64, along_u: bool| {
        m.add_curve2(Curve2::Line {
            origin: arris_math::Point2::new(u, v),
            direction: if along_u {
                arris_math::Vec2::x_axis()
            } else {
                arris_math::Vec2::y_axis()
            },
        })
    };
    // The wall: (0, 0) → (2π, 0) → (2π, h) → (0, h), counter-clockwise in
    // (u, v) with the outward normal up.
    let p_bottom = uv_line(m, 0.0, 0.0, true);
    let p_seam_up = uv_line(m, TAU, 0.0, false);
    let p_top = uv_line(m, 0.0, height, true);
    let p_seam_down = uv_line(m, 0.0, 0.0, false);
    let wall_face = m.raw().add_face(Face::new(
        wall,
        vec![Loop::new(vec![
            Coedge::new(e_bottom, Orientation::Forward, p_bottom),
            Coedge::new(e_seam, Orientation::Forward, p_seam_up),
            Coedge::new(e_top, Orientation::Reversed, p_top),
            Coedge::new(e_seam, Orientation::Reversed, p_seam_down),
        ])],
        tol,
    ));
    // The caps: each circle is the plane's own (u, v) circle at the same
    // parameter, right-handed since the circle's Z is the plane's normal.
    let cap_circle = |m: &mut Model| {
        m.add_curve2(Curve2::Circle {
            frame: Frame2::identity(),
            radius,
        })
    };
    let p_bottom_cap = cap_circle(m);
    let bottom_face = m.raw().add_face(Face::new(
        bottom_plane,
        vec![Loop::new(vec![Coedge::new(
            e_bottom,
            Orientation::Forward,
            p_bottom_cap,
        )])],
        tol,
    ));
    let p_top_cap = cap_circle(m);
    let top_face = m.raw().add_face(Face::new(
        top_plane,
        vec![Loop::new(vec![Coedge::new(
            e_top,
            Orientation::Forward,
            p_top_cap,
        )])],
        tol,
    ));

    let shell = m.raw().add_shell(Shell::new(vec![
        FaceHandle::forward(wall_face),
        FaceHandle::new(bottom_face, Orientation::Reversed),
        FaceHandle::forward(top_face),
    ]));
    let body = m
        .raw()
        .add_body(BodyEntity::solid(vec![ShellHandle::forward(shell)]));
    Ok(Body::forward(body))
}
