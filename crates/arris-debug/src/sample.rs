//! Valid bodies built by hand, with explicit pcurves: what the checker's
//! tests start from (they cannot use `arris-ops`) and what the STEP writer
//! is first tried on. The box and the cylinder go through the raw insert
//! API and are the shapes the matching `primitive/*` fixtures describe,
//! built with the conventions the primitives of `arris-ops` follow, so
//! the two dump alike; the [`frame`] goes through the Euler operators and
//! is `boolean/frame-cut`'s twin, the cross-check for the boolean that
//! builds it by recipe in M4.

use core::f64::consts::TAU;

use arris_geom::{Curve, Curve2, GeomError, NurbsCurve, NurbsSurface, Surface};
use arris_math::{
    Frame, Frame2, FrameError, Interval, Point2, Point3, UnitVec2, UnitVec3, Vec2, Vec3,
};
use arris_topo::builder::{BuildError, Builder, FaceRef, Position, Seed, Split, Strut};
use arris_topo::entity::{
    Body as BodyEntity, BodyKind, Coedge, Edge, EdgeGeometry, Face, Loop, Shell, Vertex,
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
    /// A NURBS value could not be built.
    #[error("sample geometry: {0}")]
    Geometry(#[from] GeomError),
    /// The builder refused an operator or the finish.
    #[error("sample builder: {0}")]
    Build(#[from] BuildError),
}

/// Which geometry [`cuboid`] and [`cuboid_nurbs`] give their entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flavour {
    /// Lines and planes.
    Analytic,
    /// The first edge a degree-1 NURBS, the bottom face a bilinear NURBS.
    NurbsProbe,
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
    cuboid_with(m, min, max, Flavour::Analytic)
}

/// [`cuboid`] with the same topology, pcurves and tolerances, but its
/// first edge's line stored as a degree-1 `Curve::Nurbs` over `[0, dx]`
/// and its bottom face's plane as a bilinear `Surface::Nurbs` over the
/// face's (u, v) rectangle — both exactly the analytic geometry at the
/// same parameter, so every check and comparison that passes on
/// [`cuboid`] must pass here through the B-spline arms. Errors as
/// [`cuboid`].
///
/// ```
/// use arris_debug::sample;
/// use arris_topo::Model;
/// use arris_topo::arris_math::Point3;
///
/// let mut m = Model::default();
/// let b = sample::cuboid_nurbs(&mut m, Point3::origin(), Point3::new(40.0, 30.0, 10.0)).unwrap();
/// let text = arris_debug::dump_text(&m, b).unwrap();
/// assert_eq!(text.matches("nurbs degree").count(), 2, "one curve, one surface");
/// ```
pub fn cuboid_nurbs(m: &mut Model, min: Point3, max: Point3) -> Result<Body, SampleError> {
    cuboid_with(m, min, max, Flavour::NurbsProbe)
}

fn cuboid_with(
    m: &mut Model,
    min: Point3,
    max: Point3,
    flavour: Flavour,
) -> Result<Body, SampleError> {
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

    // Every geometry value that can fail is built before the first append,
    // so an error leaves the model untouched.
    let mut curves = Vec::with_capacity(12);
    for (i, &(a, b)) in ends.iter().enumerate() {
        let (pa, pb) = (corner(a), corner(b));
        let d = pb - pa;
        let length = d.norm();
        curves.push(if flavour == Flavour::NurbsProbe && i == 0 {
            Curve::Nurbs(NurbsCurve::new(
                1,
                vec![0.0, 0.0, length, length],
                vec![pa, pb],
                vec![1.0, 1.0],
            )?)
        } else {
            Curve::Line {
                origin: pa,
                direction: UnitVec3::new_normalize(d),
            }
        });
    }
    let mut surfaces = Vec::with_capacity(6);
    for (i, ((cycle, _), frame)) in cycles.iter().zip(&frames).enumerate() {
        surfaces.push(if flavour == Flavour::NurbsProbe && i == 0 {
            // The face's rectangle in its own (u, v): u along the first
            // edge of the cycle, v along the last, both from the origin.
            let o = corner(cycle[0]);
            let extent_u = (corner(cycle[1]) - o).norm();
            let extent_v = (corner(cycle[3]) - o).norm();
            let at = |u: f64, v: f64| frame.to_world(Point3::new(u, v, 0.0));
            Surface::Nurbs(NurbsSurface::new(
                [1, 1],
                [
                    vec![0.0, 0.0, extent_u, extent_u],
                    vec![0.0, 0.0, extent_v, extent_v],
                ],
                vec![
                    at(0.0, 0.0),
                    at(0.0, extent_v),
                    at(extent_u, 0.0),
                    at(extent_u, extent_v),
                ],
                vec![1.0; 4],
            )?)
        } else {
            Surface::Plane { frame: *frame }
        });
    }

    let vertices: Vec<_> = (0..8)
        .map(|i| m.raw().add_vertex(Vertex::new(corner(i), tol)))
        .collect();
    let mut edges = Vec::with_capacity(12);
    for (&(a, b), curve) in ends.iter().zip(curves) {
        let (pa, pb) = (corner(a), corner(b));
        let length = (pb - pa).norm();
        let curve = m.add_curve(curve);
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
    for (((cycle, _), frame), surface) in cycles.iter().zip(&frames).zip(surfaces) {
        let surface = m.add_surface(surface);
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

/// The rectangular frame of `boolean/frame-cut`: the box from `min` to
/// `max` with the window `[window_min, window_max]` in (x, y) cut through
/// its full height — sixteen vertices, twenty-four line edges, ten planar
/// faces of which the top and the bottom have two loops each, genus 1.
/// Built through the Euler operators (`docs/02-data-model.md` §Euler
/// operators): Mäntylä's box recipe, then a bridge strut into the top, the
/// window's rim as struts closed by `mef` into a plug on the *bottom's*
/// plane, `kemr` on the bridge to make the rim a ring, struts down from
/// the rim, `mef` for each inner wall, and `kfmrh` to open the plug into
/// the bottom; every pcurve is given afterwards from each edge's line in
/// each face's plane. Every plane's `Z` is the face's outward normal and
/// every face is used `Forward`. Tolerances are the model's
/// `default_tolerance`. Errors: an extent that is not finite and positive,
/// a window not strictly inside the box in (x, y). The model is untouched
/// on error.
///
/// ```
/// use arris_debug::sample;
/// use arris_topo::Model;
/// use arris_topo::arris_math::{Point2, Point3};
///
/// let mut m = Model::default();
/// let b = sample::frame(
///     &mut m,
///     Point3::origin(),
///     Point3::new(40.0, 30.0, 10.0),
///     Point2::new(10.0, 10.0),
///     Point2::new(30.0, 20.0),
/// )
/// .unwrap();
/// assert_eq!(m.faces(b).unwrap().len(), 10);
/// assert_eq!(m.edges(b).unwrap().len(), 24);
/// assert!(arris_debug::euler_line(&m, b).unwrap().ends_with("g1 = 0"));
/// ```
pub fn frame(
    m: &mut Model,
    min: Point3,
    max: Point3,
    window_min: Point2,
    window_max: Point2,
) -> Result<Body, SampleError> {
    let dx = positive("x extent", max.x - min.x)?;
    let dy = positive("y extent", max.y - min.y)?;
    let dz = positive("z extent", max.z - min.z)?;
    positive("window x extent", window_max.x - window_min.x)?;
    positive("window y extent", window_max.y - window_min.y)?;
    for (name, value) in [
        ("window min x", window_min.x - min.x),
        ("window min y", window_min.y - min.y),
        ("window max x", max.x - window_max.x),
        ("window max y", max.y - window_max.y),
    ] {
        positive(name, value)?;
    }
    if !min.coords.iter().all(|c| c.is_finite()) {
        return Err(SampleError::Extent {
            name: "min",
            value: f64::NAN,
        });
    }
    let _ = (dx, dy, dz);
    let tol = m.precision().default_tolerance;
    // Outer corners counter-clockwise from above, bottom then top; the
    // window's the same way, top (rim) then bottom (floor).
    let b = [
        Point3::new(min.x, min.y, min.z),
        Point3::new(max.x, min.y, min.z),
        Point3::new(max.x, max.y, min.z),
        Point3::new(min.x, max.y, min.z),
    ];
    let t: [Point3; 4] = core::array::from_fn(|i| Point3::new(b[i].x, b[i].y, max.z));
    let h = [
        Point3::new(window_min.x, window_min.y, max.z),
        Point3::new(window_max.x, window_min.y, max.z),
        Point3::new(window_max.x, window_max.y, max.z),
        Point3::new(window_min.x, window_max.y, max.z),
    ];
    let g: [Point3; 4] = core::array::from_fn(|i| Point3::new(h[i].x, h[i].y, min.z));
    // Planes: bottom, top, the four outer sides, the four window walls;
    // every Z the outward normal.
    let mut frames = Vec::with_capacity(10);
    frames.push(Frame::new(b[0], -Vec3::z(), Vec3::x())?);
    frames.push(Frame::new(t[0], Vec3::z(), Vec3::x())?);
    let side_normals = [-Vec3::y(), Vec3::x(), Vec3::y(), -Vec3::x()];
    for i in 0..4 {
        frames.push(Frame::new(b[i], side_normals[i], b[(i + 1) % 4] - b[i])?);
    }
    let wall_normals = [Vec3::y(), -Vec3::x(), -Vec3::y(), Vec3::x()];
    for i in 0..4 {
        frames.push(Frame::new(g[i], wall_normals[i], g[(i + 1) % 4] - g[i])?);
    }
    m.transaction(|m| {
        let surfaces: Vec<_> = frames
            .iter()
            .map(|&frame| m.add_surface(Surface::Plane { frame }))
            .collect();
        let (s_bottom, s_top) = (surfaces[0], surfaces[1]);
        let line = |m: &mut Model, p: Point3, q: Point3| -> Result<EdgeGeometry, SampleError> {
            let d = q - p;
            let length = d.norm();
            let curve = m.add_curve(Curve::Line {
                origin: p,
                direction: UnitVec3::new_normalize(d),
            });
            let range = Interval::new(0.0, length).map_err(|_| SampleError::Extent {
                name: "edge length",
                value: length,
            })?;
            Ok(EdgeGeometry::Curve { curve, range })
        };
        let strut = |m: &mut Model, p: Point3, q: Point3| -> Result<Strut, SampleError> {
            Ok(Strut {
                point: q,
                geometry: line(m, p, q)?,
                pcurves: [None, None],
            })
        };
        let split = |m: &mut Model,
                     p: Point3,
                     q: Point3,
                     surface,
                     orientation|
         -> Result<Split, SampleError> {
            Ok(Split {
                geometry: line(m, p, q)?,
                surface,
                orientation,
                pcurves: [None, None],
            })
        };
        let mut bd = Builder::new(tol);
        // The bottom's rectangle: three struts and a closing mef whose new
        // face is the lid that becomes the top.
        let (v0, f_bottom) = bd.mvfs(Seed {
            point: b[0],
            surface: s_bottom,
            orientation: Orientation::Forward,
        })?;
        let mut corners = vec![v0];
        for i in 1..4 {
            let at = bd.find_position(f_bottom, 0, corners[i - 1])?;
            let (v, _) = bd.mev(at, strut(m, b[i - 1], b[i])?)?;
            corners.push(v);
        }
        let from = bd.find_position(f_bottom, 0, corners[3])?;
        let to = bd.find_position(f_bottom, 0, corners[0])?;
        let (_, f_top) = bd.mef(from, to, split(m, b[3], b[0], s_top, Orientation::Forward)?)?;
        // Struts up from the lid's corners, then the four sides.
        let mut tops = Vec::with_capacity(4);
        for i in 0..4 {
            let at = bd.find_position(f_top, 0, corners[i])?;
            let (v, _) = bd.mev(at, strut(m, b[i], t[i])?)?;
            tops.push(v);
        }
        for i in 0..4 {
            let from = bd.find_position(f_top, 0, tops[(i + 1) % 4])?;
            let to = bd.find_position(f_top, 0, tops[i])?;
            bd.mef(
                from,
                to,
                split(
                    m,
                    t[(i + 1) % 4],
                    t[i],
                    surfaces[2 + i],
                    Orientation::Forward,
                )?,
            )?;
        }
        // The window: a bridge into the top, the rim as struts, the plug
        // on the bottom's plane facing up, the bridge cut into a ring.
        let at = bd.find_position(f_top, 0, tops[0])?;
        let (h0, bridge) = bd.mev(at, strut(m, t[0], h[0])?)?;
        let mut rim = vec![h0];
        for i in 1..4 {
            let at = bd.find_position(f_top, 0, rim[i - 1])?;
            let (v, _) = bd.mev(at, strut(m, h[i - 1], h[i])?)?;
            rim.push(v);
        }
        let from = bd.find_position(f_top, 0, rim[3])?;
        let to = match bd.find_position(f_top, 0, rim[0]) {
            Err(BuildError::Ambiguous { positions, .. }) => Position::new(f_top, 0, positions[0]),
            Ok(p) => p,
            Err(e) => return Err(e.into()),
        };
        let (_, plug) = bd.mef(
            from,
            to,
            split(m, h[3], h[0], s_bottom, Orientation::Reversed)?,
        )?;
        bd.kemr(bridge)?;
        // Struts down from the rim, the four walls, and the floor opened
        // into the bottom.
        let mut floor = Vec::with_capacity(4);
        for i in 0..4 {
            let at = bd.find_position(plug, 0, rim[i])?;
            let (v, _) = bd.mev(at, strut(m, h[i], g[i])?)?;
            floor.push(v);
        }
        for i in 0..4 {
            let from = bd.find_position(plug, 0, floor[(i + 1) % 4])?;
            let to = bd.find_position(plug, 0, floor[i])?;
            bd.mef(
                from,
                to,
                split(
                    m,
                    g[(i + 1) % 4],
                    g[i],
                    surfaces[6 + i],
                    Orientation::Forward,
                )?,
            )?;
        }
        bd.kfmrh(plug, f_bottom)?;
        // Every pcurve: the edge's line in the face's plane, at the edge's
        // own parameter.
        let plane_of = |surface| frames[surfaces.iter().position(|&s| s == surface).unwrap_or(0)];
        let mut wanted: Vec<(Position, Point3, Point3, Frame)> = Vec::new();
        for (f, face) in bd.faces() {
            let frame = plane_of(face.surface());
            for (li, lp) in face.loops().iter().enumerate() {
                for (ci, u) in lp.uses().iter().enumerate() {
                    let e = bd.edge(u.edge)?;
                    let p = bd.vertex(e.start())?.point();
                    let q = bd.vertex(e.end())?.point();
                    wanted.push((Position::new(f, li, ci), p, q, frame));
                }
            }
        }
        for (at, p, q, frame) in wanted {
            let origin = frame.to_local(p);
            let direction = frame.vec_to_local(q - p);
            let pcurve = m.add_curve2(Curve2::Line {
                origin: Point2::new(origin.x, origin.y),
                direction: UnitVec2::new_normalize(Vec2::new(direction.x, direction.y)),
            });
            bd.set_pcurve(at, pcurve)?;
        }
        let _: FaceRef = f_bottom;
        Ok(bd.finish(m, BodyKind::Solid)?.body)
    })
}
