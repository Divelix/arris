//! Point chains: discretised edges and intersection curves.

use crate::Aabb;

/// An ordered chain of points in 3D: a discretised edge, an intersection
/// curve, a debugging line. Open unless its first and last points coincide
/// exactly; a closed polyline repeats its first point at the end.
///
/// ```
/// use arris_mesh::Polyline;
///
/// let p = Polyline::new(vec![[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [3.0, 4.0, 0.0]]);
/// assert_eq!(p.length(), 7.0);
/// assert!(!p.is_closed());
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Polyline {
    /// The points, in order.
    pub points: Vec<[f64; 3]>,
}

impl Polyline {
    /// A polyline through `points` in order.
    pub fn new(points: Vec<[f64; 3]>) -> Self {
        Polyline { points }
    }

    /// Sum of the segment lengths; zero for fewer than two points.
    pub fn length(&self) -> f64 {
        self.points.windows(2).map(|w| dist(w[0], w[1])).sum()
    }

    /// `true` when there are at least two points and the last equals the
    /// first exactly.
    pub fn is_closed(&self) -> bool {
        match (self.points.first(), self.points.last()) {
            (Some(a), Some(b)) => self.points.len() >= 2 && a == b,
            _ => false,
        }
    }

    /// The bounding box, or `None` for no points.
    pub fn aabb(&self) -> Option<Aabb> {
        Aabb::of_points(&self.points)
    }
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}
