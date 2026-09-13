//! A surface and curves without a body render through the rasteriser
//! (`docs/ARCHITECTURE.md` §Formats and tools), written to
//! `target/inspect/cylinder-wireframe.png` for the agent to read.

use arris_debug::render::colors;
use arris_debug::{View, polyline_of, render, render_png, wireframe_of};
use arris_geom::{Curve, Surface};
use arris_math::{Frame, Interval, Point3, Vec3};
use arris_mesh::TriMesh;

#[test]
fn cylinder_wireframe_with_a_circle_and_an_ellipse_renders() {
    let axis = Frame::new(Point3::new(0.0, 0.0, 0.0), Vec3::z(), Vec3::x()).unwrap();
    let cyl = Surface::Cylinder {
        frame: axis,
        radius: 1.0,
    };
    let height = Interval::new(-1.5, 1.5).unwrap();
    let mut lines = wireframe_of(&cyl, [Interval::TURN, height], 12);
    // A circle around the axis at v = −1.1 (between two wireframe rings, so
    // it reads as its own curve) and an oblique section at 30°:
    // an ellipse with minor radius R and major radius R / cos 30°.
    let circle = Curve::Circle {
        frame: Frame::new(Point3::new(0.0, 0.0, -1.1), Vec3::z(), Vec3::x()).unwrap(),
        radius: 1.0,
    };
    let tilt = 30f64.to_radians();
    let ellipse = Curve::Ellipse {
        frame: Frame::new(
            Point3::new(0.0, 0.0, 0.5),
            Vec3::new(0.0, -tilt.sin(), tilt.cos()),
            Vec3::new(0.0, tilt.cos(), tilt.sin()),
        )
        .unwrap(),
        major_radius: 1.0 / tilt.cos(),
        minor_radius: 1.0,
    };
    lines.push(polyline_of(&circle, Interval::TURN, 96));
    lines.push(polyline_of(&ellipse, Interval::TURN, 96));
    // Every ellipse point lies on the cylinder.
    for p in &lines.last().unwrap().points {
        let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
        assert!(
            (r - 1.0).abs() < 1e-12,
            "ellipse point off the cylinder: {p:?}"
        );
    }
    let mesh = TriMesh::new();
    let raster = render(&mesh, &lines, View::Iso, None);
    let drawn = raster
        .pixels()
        .iter()
        .filter(|&&px| px == colors::LINE)
        .count();
    assert!(drawn > 1000, "only {drawn} line pixels");
    let path = render_png(&mesh, &lines, View::Iso, None, "cylinder-wireframe").unwrap();
    assert!(path.exists());
}

/// The curves `intersect_surfaces` returns for a cylinder cut by a
/// parallel plane (circle), an oblique plane (ellipse) and a perpendicular
/// plane through the axis (two rulings) render on the cylinder's
/// wireframe: `target/inspect/plane-cylinder-sections.png`.
#[test]
fn plane_cylinder_sections_render() {
    use arris_geom::{SurfaceIntersection, intersect_surfaces};
    use arris_math::Precision;

    let tol = Precision::DEFAULT.tolerance();
    let cyl = Surface::Cylinder {
        frame: Frame::world(),
        radius: 1.0,
    };
    let height = Interval::new(-1.5, 1.5).unwrap();
    let mut lines = wireframe_of(&cyl, [Interval::TURN, height], 12);
    let tilt = 30f64.to_radians();
    let planes = [
        Frame::from_z(Point3::new(0.0, 0.0, -1.1), Vec3::z()).unwrap(),
        Frame::from_z(
            Point3::new(0.0, 0.0, 0.5),
            Vec3::new(0.0, -tilt.sin(), tilt.cos()),
        )
        .unwrap(),
        Frame::from_z(Point3::new(0.0, 0.0, 0.0), Vec3::x()).unwrap(),
    ];
    let mut kinds = Vec::new();
    for frame in planes {
        let plane = Surface::Plane { frame };
        let SurfaceIntersection::Transversal(curves) =
            intersect_surfaces(&plane, &cyl, tol).unwrap()
        else {
            panic!("every section here is transversal")
        };
        for c in curves {
            kinds.push(c.kind());
            let range = match c {
                Curve::Line { .. } => height,
                _ => Interval::TURN,
            };
            lines.push(polyline_of(&c, range, 96));
        }
    }
    use arris_geom::CurveKind::{Circle, Ellipse, Line};
    assert_eq!(kinds, [Circle, Ellipse, Line, Line]);
    let mesh = TriMesh::new();
    let path = render_png(&mesh, &lines, View::Iso, None, "plane-cylinder-sections").unwrap();
    assert!(path.exists());
    let front = render_png(
        &mesh,
        &lines,
        View::Front,
        None,
        "plane-cylinder-sections-front",
    )
    .unwrap();
    assert!(front.exists());
}
