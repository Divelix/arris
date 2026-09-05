//! Axis-aligned bounding boxes of mesh data.

/// The axis-aligned box around a set of points: `min ≤ max` on every axis,
/// both finite. A box is never empty; an empty point set has no box.
///
/// ```
/// use arris_mesh::Aabb;
///
/// let b = Aabb::of_points(&[[0.0, 0.0, 0.0], [2.0, -1.0, 3.0]]).unwrap();
/// assert_eq!(b.min, [0.0, -1.0, 0.0]);
/// assert_eq!(b.max, [2.0, 0.0, 3.0]);
/// assert_eq!(b.extent(), [2.0, 1.0, 3.0]);
/// assert_eq!(b.center(), [1.0, -0.5, 1.5]);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    /// The smallest coordinate on each axis.
    pub min: [f64; 3],
    /// The largest coordinate on each axis.
    pub max: [f64; 3],
}

impl Aabb {
    /// The box around `points`, or `None` for no points.
    pub fn of_points<'a>(points: impl IntoIterator<Item = &'a [f64; 3]>) -> Option<Aabb> {
        let mut it = points.into_iter();
        let first = *it.next()?;
        let mut b = Aabb {
            min: first,
            max: first,
        };
        for p in it {
            b = b.union(Aabb { min: *p, max: *p });
        }
        Some(b)
    }

    /// The smallest box containing both.
    pub fn union(self, other: Aabb) -> Aabb {
        let mut b = self;
        for ((lo, hi), (olo, ohi)) in b
            .min
            .iter_mut()
            .zip(b.max.iter_mut())
            .zip(other.min.iter().zip(other.max.iter()))
        {
            *lo = lo.min(*olo);
            *hi = hi.max(*ohi);
        }
        b
    }

    /// `max - min` per axis.
    pub fn extent(&self) -> [f64; 3] {
        [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ]
    }

    /// The midpoint.
    pub fn center(&self) -> [f64; 3] {
        [
            0.5 * (self.min[0] + self.max[0]),
            0.5 * (self.min[1] + self.max[1]),
            0.5 * (self.min[2] + self.max[2]),
        ]
    }

    /// The length of the box's diagonal: the scale of what it contains.
    pub fn diagonal(&self) -> f64 {
        let e = self.extent();
        (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt()
    }
}
