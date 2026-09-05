//! Point projection: the nearest point of a surface or a curve, with its
//! parameters.

use core::f64::consts::{FRAC_PI_2, TAU};

use arris_math::{Frame, Point2, Point3, is_negligible};

use crate::{AmbiguousLocus, Curve, CurveKind, GeomError, GeomKind, Surface};

/// The nearest point of a surface to a query point, with its parameters.
///
/// `point == surface.point(uv.x, uv.y)` to rounding, and `distance` is the
/// Euclidean distance from the query to it, computed from the surface's
/// closed form rather than by subtraction so it is exact for a point on
/// the surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceProjection {
    /// `(u, v)` of the nearest point: a periodic `u` in `[0, 2π)`, the
    /// sphere's `v` in `[−π/2, π/2]`, the torus's `v` in `[0, 2π)`.
    pub uv: Point2,
    /// The nearest point.
    pub point: Point3,
    /// How far the query is from it, `≥ 0`.
    pub distance: f64,
}

/// The nearest point of a curve to a query point, with its parameter.
///
/// `point == curve.point(t)` to rounding and `distance` is the Euclidean
/// distance from the query to it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurveProjection {
    /// The parameter of the nearest point: a periodic `t` in `[0, 2π)`.
    pub t: f64,
    /// The nearest point.
    pub point: Point3,
    /// How far the query is from it, `≥ 0`.
    pub distance: f64,
}

/// An angle from `atan2` (in `(−π, π]`) moved into `[0, 2π)`. A negative
/// angle whose sum with `2π` rounds up to `2π` becomes `0`: the same point
/// on the circle, and inside the domain.
pub(crate) fn wrap_turn(u: f64) -> f64 {
    let u = if u < 0.0 { u + TAU } else { u };
    if u >= TAU { 0.0 } else { u }
}

/// The magnitude at which a local coordinate of `p` in `frame` is rounding
/// noise: `to_local` subtracts the origin and projects, so its error is a
/// few ulps of the larger of the two magnitudes.
fn local_noise_scale(frame: &Frame, p: Point3) -> f64 {
    p.coords.norm() + frame.origin().coords.norm()
}

impl Surface {
    /// The nearest point of the surface to `p` with its `(u, v)`, by the
    /// closed form of each variant: the point is exact to rounding, never
    /// the result of an iteration, and the parameters are the
    /// parametrisation's own (`docs/02-data-model.md` §Surfaces).
    ///
    /// The whole parametric surface is the target — both nappes of a
    /// cone, the full plane — not a face's trimmed part of it.
    ///
    /// Errors: [`GeomError::Ambiguous`] where the nearest point or its
    /// parameter is not unique, decided to rounding and never by a silent
    /// choice — a point on the axis of a cylinder, a cone or a torus, in
    /// the plane through a cone's apex, at a sphere's centre, or on a
    /// torus's centre circle. A point on a sphere's axis off its centre
    /// projects to the pole, whose `u` is `0` by convention: the point is
    /// unique, only the degenerate parameter is not.
    ///
    /// ```
    /// use arris_geom::Surface;
    /// use arris_math::{Frame, Point3};
    ///
    /// let cyl = Surface::Cylinder { frame: Frame::world(), radius: 2.0 };
    /// let proj = cyl.project(Point3::new(0.0, -5.0, 3.0)).unwrap();
    /// assert!((proj.uv.x - 3.0 * core::f64::consts::FRAC_PI_2).abs() < 1e-15);
    /// assert_eq!(proj.uv.y, 3.0);
    /// assert!((proj.distance - 3.0).abs() < 1e-15);
    /// assert!((proj.point - Point3::new(0.0, -2.0, 3.0)).norm() < 1e-15);
    /// assert!(cyl.project(Point3::new(0.0, 0.0, 7.0)).is_err());
    /// ```
    pub fn project(&self, p: Point3) -> Result<SurfaceProjection, GeomError> {
        let frame = self.frame();
        let q = frame.to_local(p);
        let noise = local_noise_scale(frame, p);
        let rho = q.x.hypot(q.y);
        let ambiguous = |locus| GeomError::Ambiguous {
            kind: GeomKind::Surface(self.kind()),
            locus,
            point: p,
        };
        let (u, v, distance) = match *self {
            Surface::Plane { .. } => (q.x, q.y, q.z.abs()),
            Surface::Cylinder { radius, .. } => {
                if is_negligible(rho, noise) {
                    return Err(ambiguous(AmbiguousLocus::Axis));
                }
                (wrap_turn(q.y.atan2(q.x)), q.z, (rho - radius).abs())
            }
            Surface::Cone {
                radius, half_angle, ..
            } => {
                if is_negligible(rho, noise) {
                    return Err(ambiguous(AmbiguousLocus::Axis));
                }
                let (sa, ca) = half_angle.sin_cos();
                // In the axial half-plane through `p` the surface is two
                // rulings crossing at the apex, one per nappe; `side` is
                // the point's height above the plane through the apex,
                // scaled by sin α, and its sign says which ruling is
                // nearer (they are equally near on that plane).
                let side = radius * ca + q.z * sa;
                if is_negligible(side, radius.abs() + noise) {
                    return Err(ambiguous(AmbiguousLocus::ApexPlane));
                }
                let (u, signed_rho) = if side < 0.0 {
                    (wrap_turn((-q.y).atan2(-q.x)), -rho)
                } else {
                    (wrap_turn(q.y.atan2(q.x)), rho)
                };
                let along = signed_rho - radius;
                (u, sa * along + ca * q.z, (ca * along - sa * q.z).abs())
            }
            Surface::Sphere { radius, .. } => {
                let r = q.coords.norm();
                if is_negligible(r, noise) {
                    return Err(ambiguous(AmbiguousLocus::Centre));
                }
                if is_negligible(rho, noise) {
                    let v = if q.z > 0.0 { FRAC_PI_2 } else { -FRAC_PI_2 };
                    (0.0, v, (r - radius).abs())
                } else {
                    (
                        wrap_turn(q.y.atan2(q.x)),
                        q.z.atan2(rho),
                        (r - radius).abs(),
                    )
                }
            }
            Surface::Torus {
                major_radius,
                minor_radius,
                ..
            } => {
                if is_negligible(rho, noise) {
                    return Err(ambiguous(AmbiguousLocus::Axis));
                }
                let across = rho - major_radius;
                let tube = across.hypot(q.z);
                if is_negligible(tube, noise + major_radius.abs()) {
                    return Err(ambiguous(AmbiguousLocus::CentreCircle));
                }
                (
                    wrap_turn(q.y.atan2(q.x)),
                    wrap_turn(q.z.atan2(across)),
                    (tube - minor_radius).abs(),
                )
            }
        };
        Ok(SurfaceProjection {
            uv: Point2::new(u, v),
            point: self.point(u, v),
            distance,
        })
    }
}

impl Curve {
    /// The nearest point of the curve to `p` with its parameter, by the
    /// closed form of each variant: the point is exact to rounding and the
    /// parameter is the parametrisation's own (`docs/02-data-model.md`
    /// §Curves).
    ///
    /// Errors: [`GeomError::Ambiguous`] where the nearest point is not
    /// unique, decided to rounding — a point on a circle's axis.
    /// [`GeomError::Unsupported`] for an ellipse until the quartic that
    /// projects onto one lands (`docs/plans/m1-geometry.md` step 5).
    ///
    /// ```
    /// use arris_geom::Curve;
    /// use arris_math::{Frame, Point3};
    /// use core::f64::consts::PI;
    ///
    /// let c = Curve::Circle { frame: Frame::world(), radius: 2.0 };
    /// let proj = c.project(Point3::new(-4.0, 0.0, 3.0)).unwrap();
    /// assert!((proj.t - PI).abs() < 1e-15);
    /// assert!((proj.distance - 13f64.sqrt()).abs() < 1e-15);
    /// assert!(c.project(Point3::new(0.0, 0.0, 3.0)).is_err());
    /// ```
    pub fn project(&self, p: Point3) -> Result<CurveProjection, GeomError> {
        match *self {
            Curve::Line { origin, direction } => {
                let t = (p - origin).dot(&direction);
                let point = origin + t * direction.into_inner();
                Ok(CurveProjection {
                    t,
                    point,
                    distance: (p - point).norm(),
                })
            }
            Curve::Circle { frame, radius } => {
                let q = frame.to_local(p);
                let rho = q.x.hypot(q.y);
                if is_negligible(rho, local_noise_scale(&frame, p)) {
                    return Err(GeomError::Ambiguous {
                        kind: GeomKind::Curve(CurveKind::Circle),
                        locus: AmbiguousLocus::Axis,
                        point: p,
                    });
                }
                let t = wrap_turn(q.y.atan2(q.x));
                Ok(CurveProjection {
                    t,
                    point: self.point(t),
                    distance: (rho - radius).hypot(q.z),
                })
            }
            Curve::Ellipse { .. } => Err(GeomError::Unsupported {
                a: GeomKind::Point,
                b: GeomKind::Curve(CurveKind::Ellipse),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_turn_lands_in_the_half_open_turn() {
        assert_eq!(wrap_turn(0.0), 0.0);
        assert_eq!(wrap_turn(-1e-300), 0.0);
        assert_eq!(wrap_turn(-1.0), TAU - 1.0);
        assert_eq!(wrap_turn(core::f64::consts::PI), core::f64::consts::PI);
        assert!(wrap_turn(-f64::EPSILON) < TAU);
    }

    #[test]
    fn a_plane_projects_by_local_coordinates() {
        let plane = Surface::Plane {
            frame: Frame::world(),
        };
        let proj = plane.project(Point3::new(3.0, -4.0, 5.0)).unwrap();
        assert_eq!(proj.uv, Point2::new(3.0, -4.0));
        assert_eq!(proj.distance, 5.0);
        assert_eq!(proj.point, Point3::new(3.0, -4.0, 0.0));
    }
}
