//! Curves and surfaces as polylines, so geometry without a body renders
//! through the M0 rasteriser (`.agents/skills/inspect/SKILL.md`, the PNG
//! row).

use arris_geom::{Curve, Surface};
use arris_math::Interval;
use arris_mesh::Polyline;

/// `curve` sampled at `segments + 1` parameters spread evenly over `range`,
/// both ends included. Fewer than one segment gives the two end points;
/// an unbounded `range` gives an empty polyline, since there is nothing
/// finite to draw.
///
/// ```
/// use arris_debug::polyline_of;
/// use arris_geom::Curve;
/// use arris_math::{Frame, Interval};
///
/// let circle = Curve::Circle { frame: Frame::world(), radius: 1.0 };
/// let p = polyline_of(&circle, Interval::TURN, 64);
/// assert_eq!(p.points.len(), 65);
/// let (first, last) = (p.points[0], p.points[64]);
/// assert!((first[1] - last[1]).abs() < 1e-15); // closed to rounding, not bit-exactly
/// ```
pub fn polyline_of(curve: &Curve, range: Interval, segments: usize) -> Polyline {
    if !range.is_bounded() {
        return Polyline::default();
    }
    let n = segments.max(1);
    let points = (0..=n)
        .map(|i| {
            let t = range.lerp(i as f64 / n as f64);
            let p = curve.point(t);
            [p.x, p.y, p.z]
        })
        .collect();
    Polyline::new(points)
}

/// `surface` as isoparametric lines: `lines + 1` lines of constant `u`
/// and `lines + 1` of constant `v`, evenly spread over `domain` with both
/// ends included, each sampled at `4 · lines` segments. Fewer than one line
/// gives the four boundary lines; an unbounded `domain` gives nothing.
///
/// ```
/// use arris_debug::wireframe_of;
/// use arris_geom::Surface;
/// use arris_math::{Frame, Interval};
///
/// let cyl = Surface::Cylinder { frame: Frame::world(), radius: 1.0 };
/// let domain = [Interval::TURN, Interval::new(0.0, 2.0).unwrap()];
/// assert_eq!(wireframe_of(&cyl, domain, 8).len(), 18);
/// ```
pub fn wireframe_of(surface: &Surface, domain: [Interval; 2], lines: usize) -> Vec<Polyline> {
    let [du, dv] = domain;
    if !(du.is_bounded() && dv.is_bounded()) {
        return Vec::new();
    }
    let n = lines.max(1);
    let segments = 4 * n;
    let mut out = Vec::with_capacity(2 * (n + 1));
    for i in 0..=n {
        let u = du.lerp(i as f64 / n as f64);
        out.push(iso(segments, |s| {
            let v = dv.lerp(s);
            surface.point(u, v)
        }));
    }
    for j in 0..=n {
        let v = dv.lerp(j as f64 / n as f64);
        out.push(iso(segments, |s| {
            let u = du.lerp(s);
            surface.point(u, v)
        }));
    }
    out
}

fn iso(segments: usize, point_at: impl Fn(f64) -> arris_math::Point3) -> Polyline {
    Polyline::new(
        (0..=segments)
            .map(|k| {
                let p = point_at(k as f64 / segments as f64);
                [p.x, p.y, p.z]
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use arris_math::{Frame, Point3, Vec3};

    #[test]
    fn unbounded_ranges_draw_nothing() {
        let line = Curve::Line {
            origin: Point3::origin(),
            direction: Vec3::x_axis(),
        };
        assert!(polyline_of(&line, Interval::REAL, 8).points.is_empty());
        let plane = Surface::Plane {
            frame: Frame::world(),
        };
        assert!(wireframe_of(&plane, plane.domain(), 8).is_empty());
        let bounded = Interval::new(-1.0, 1.0).unwrap();
        assert_eq!(polyline_of(&line, bounded, 0).points.len(), 2);
        assert_eq!(polyline_of(&line, bounded, 4).length(), 2.0);
        assert_eq!(wireframe_of(&plane, [bounded, bounded], 0).len(), 4);
    }
}
