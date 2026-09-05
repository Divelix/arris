//! A surface and curves without a body render through the rasteriser: the
//! picture `docs/plans/m1-geometry.md` step 2 asks for, written to
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
