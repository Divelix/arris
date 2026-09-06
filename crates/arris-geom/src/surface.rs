//! Surfaces and their evaluation.

use core::f64::consts::{FRAC_PI_2, TAU};
use core::fmt;

use arris_math::{Frame, Interval, Isometry, Point3, UnitVec3, Vec3, is_negligible};

use crate::NurbsSurface;

/// A surface, placed by its frame, with the parametrisation of
/// `docs/02-data-model.md` §Surfaces (the one Open CASCADE's `Geom`
/// classes use, so STEP round-trips without re-parametrising).
///
/// The fields are plain data: a `Surface` is a value the arena stores once
/// and never modifies, and its validity (positive radii, a torus with
/// `major_radius > minor_radius`) is the checker's to enforce; the NURBS
/// variant is valid by its constructor. Evaluation of any finite value
/// never panics.
///
/// ```
/// use arris_geom::Surface;
/// use arris_math::{Frame, Point3};
/// use core::f64::consts::FRAC_PI_2;
///
/// let cyl = Surface::Cylinder { frame: Frame::world(), radius: 2.0 };
/// let e = cyl.eval(FRAC_PI_2, 3.0);
/// assert!((e.point - Point3::new(0.0, 2.0, 3.0)).norm() < 1e-15);
/// let n = cyl.normal(FRAC_PI_2, 3.0).unwrap();
/// assert!((n.y - 1.0).abs() < 1e-15);
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Surface {
    /// `P(u, v) = O + u·X + v·Y`; normal `Z`.
    Plane {
        /// Origin and axes.
        frame: Frame,
    },
    /// `P(u, v) = O + R(cos u·X + sin u·Y) + v·Z`; seam at `u = 0`.
    Cylinder {
        /// `Z` is the axis; `X` points at the seam.
        frame: Frame,
        /// `R`.
        radius: f64,
    },
    /// `P(u, v) = O + (R + v sin α)(cos u·X + sin u·Y) + v cos α·Z`; the
    /// apex is at `v = −R / sin α`.
    Cone {
        /// `Z` is the axis; `X` points at the seam.
        frame: Frame,
        /// `R`, the radius at `v = 0`.
        radius: f64,
        /// `α ∈ (0, π/2)`, the angle between a ruling and the axis.
        half_angle: f64,
    },
    /// `P(u, v) = O + R cos v (cos u·X + sin u·Y) + R sin v·Z`; poles at
    /// `v = ±π/2`.
    Sphere {
        /// `Z` runs pole to pole; `X` points at the seam.
        frame: Frame,
        /// `R`.
        radius: f64,
    },
    /// `P(u, v) = O + (R + r cos v)(cos u·X + sin u·Y) + r sin v·Z`.
    Torus {
        /// `Z` is the axis; `X` points at the `u` seam.
        frame: Frame,
        /// `R`, from the axis to the tube's centre circle.
        major_radius: f64,
        /// `r`, the tube's radius; `r < R` in cycle 1.
        minor_radius: f64,
    },
    /// A rational B-spline; parametrised by its knots, no frame.
    Nurbs(NurbsSurface),
}

/// The fieldless twin of [`Surface`], for errors and dispatch tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SurfaceKind {
    /// [`Surface::Plane`].
    Plane,
    /// [`Surface::Cylinder`].
    Cylinder,
    /// [`Surface::Cone`].
    Cone,
    /// [`Surface::Sphere`].
    Sphere,
    /// [`Surface::Torus`].
    Torus,
    /// [`Surface::Nurbs`].
    Nurbs,
}

impl fmt::Display for SurfaceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SurfaceKind::Plane => "plane",
            SurfaceKind::Cylinder => "cylinder",
            SurfaceKind::Cone => "cone",
            SurfaceKind::Sphere => "sphere",
            SurfaceKind::Torus => "torus",
            SurfaceKind::Nurbs => "NURBS",
        })
    }
}

/// A surface evaluated at one `(u, v)`: the point and its derivatives to
/// second order. Derivatives are with respect to the parameters as
/// stored, never normalised.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceEval {
    /// `P(u, v)`.
    pub point: Point3,
    /// `∂P/∂u`.
    pub du: Vec3,
    /// `∂P/∂v`.
    pub dv: Vec3,
    /// `∂²P/∂u²`.
    pub duu: Vec3,
    /// `∂²P/∂u∂v`.
    pub duv: Vec3,
    /// `∂²P/∂v²`.
    pub dvv: Vec3,
}

impl Surface {
    /// Which variant this is.
    pub fn kind(&self) -> SurfaceKind {
        match self {
            Surface::Plane { .. } => SurfaceKind::Plane,
            Surface::Cylinder { .. } => SurfaceKind::Cylinder,
            Surface::Cone { .. } => SurfaceKind::Cone,
            Surface::Sphere { .. } => SurfaceKind::Sphere,
            Surface::Torus { .. } => SurfaceKind::Torus,
            Surface::Nurbs(_) => SurfaceKind::Nurbs,
        }
    }

    /// The placing frame of an analytic surface; `None` for a NURBS, which
    /// is placed by its control points.
    pub fn frame(&self) -> Option<&Frame> {
        match self {
            Surface::Plane { frame }
            | Surface::Cylinder { frame, .. }
            | Surface::Cone { frame, .. }
            | Surface::Sphere { frame, .. }
            | Surface::Torus { frame, .. } => Some(frame),
            Surface::Nurbs(_) => None,
        }
    }

    /// The point and its derivatives to second order at `(u, v)`. Defined
    /// for every finite parameter, inside the domain or not: a periodic
    /// parameter wraps, an unbounded one extends, a clamped NURBS
    /// extrapolates its end piece.
    pub fn eval(&self, u: f64, v: f64) -> SurfaceEval {
        let zero = Vec3::zeros();
        match self {
            Surface::Nurbs(s) => s.eval(u, v),
            Surface::Plane { frame } => {
                let (o, x, y, _) = axes(frame);
                SurfaceEval {
                    point: o + u * x + v * y,
                    du: x,
                    dv: y,
                    duu: zero,
                    duv: zero,
                    dvv: zero,
                }
            }
            &Surface::Cylinder { ref frame, radius } => {
                let (o, x, y, z) = axes(frame);
                let (su, cu) = u.sin_cos();
                let radial = cu * x + su * y;
                let tangential = -su * x + cu * y;
                SurfaceEval {
                    point: o + radius * radial + v * z,
                    du: radius * tangential,
                    dv: z,
                    duu: -radius * radial,
                    duv: zero,
                    dvv: zero,
                }
            }
            &Surface::Cone {
                ref frame,
                radius,
                half_angle,
            } => {
                let (o, x, y, z) = axes(frame);
                let (su, cu) = u.sin_cos();
                let (sa, ca) = half_angle.sin_cos();
                let rho = radius + v * sa;
                let radial = cu * x + su * y;
                let tangential = -su * x + cu * y;
                SurfaceEval {
                    point: o + rho * radial + (v * ca) * z,
                    du: rho * tangential,
                    dv: sa * radial + ca * z,
                    duu: -rho * radial,
                    duv: sa * tangential,
                    dvv: zero,
                }
            }
            &Surface::Sphere { ref frame, radius } => {
                let (o, x, y, z) = axes(frame);
                let (su, cu) = u.sin_cos();
                let (sv, cv) = v.sin_cos();
                let radial = cu * x + su * y;
                let tangential = -su * x + cu * y;
                SurfaceEval {
                    point: o + (radius * cv) * radial + (radius * sv) * z,
                    du: (radius * cv) * tangential,
                    dv: (-radius * sv) * radial + (radius * cv) * z,
                    duu: (-radius * cv) * radial,
                    duv: (-radius * sv) * tangential,
                    dvv: (-radius * cv) * radial - (radius * sv) * z,
                }
            }
            &Surface::Torus {
                ref frame,
                major_radius,
                minor_radius,
            } => {
                let (o, x, y, z) = axes(frame);
                let (su, cu) = u.sin_cos();
                let (sv, cv) = v.sin_cos();
                let rho = major_radius + minor_radius * cv;
                let radial = cu * x + su * y;
                let tangential = -su * x + cu * y;
                SurfaceEval {
                    point: o + rho * radial + (minor_radius * sv) * z,
                    du: rho * tangential,
                    dv: (-minor_radius * sv) * radial + (minor_radius * cv) * z,
                    duu: -rho * radial,
                    duv: (-minor_radius * sv) * tangential,
                    dvv: (-minor_radius * cv) * radial - (minor_radius * sv) * z,
                }
            }
        }
    }

    /// `P(u, v)` alone.
    pub fn point(&self, u: f64, v: f64) -> Point3 {
        self.eval(u, v).point
    }

    /// The surface normal `∂P/∂u × ∂P/∂v` normalised (plane `Z`; cylinder,
    /// cone and sphere radially outward; torus outward from the tube), or
    /// `None` where the parametrisation is singular: a cone's apex, a
    /// sphere's poles, any surface whose radius is zero, and a NURBS point
    /// where the two derivatives are parallel or vanish. Singular means
    /// the radial scale factor is zero to rounding
    /// ([`arris_math::is_negligible`]), so `v = π/2` in `f64` is the pole
    /// even though `cos(π/2)` is not exactly `0`. Never a direction made
    /// of rounding noise.
    pub fn normal(&self, u: f64, v: f64) -> Option<UnitVec3> {
        let singular = match *self {
            Surface::Nurbs(ref s) => return s.normal(u, v),
            Surface::Plane { .. } => false,
            Surface::Cylinder { radius, .. } => radius == 0.0,
            Surface::Cone {
                radius, half_angle, ..
            } => {
                let along = v * half_angle.sin();
                is_negligible(radius + along, radius.abs().max(along.abs()))
            }
            Surface::Sphere { radius, .. } => is_negligible(radius * v.cos(), radius),
            Surface::Torus {
                major_radius,
                minor_radius,
                ..
            } => {
                let tube = minor_radius * v.cos();
                minor_radius == 0.0
                    || is_negligible(major_radius + tube, major_radius.abs().max(tube.abs()))
            }
        };
        if singular {
            return None;
        }
        let e = self.eval(u, v);
        UnitVec3::try_new(e.du.cross(&e.dv), 0.0)
    }

    /// The parametric domain `[u, v]`: the closed fundamental interval
    /// `[0, 2π]` of a periodic direction, `[−π/2, π/2]` for the sphere's
    /// `v`, [`Interval::REAL`] where the table says ℝ, the knot ranges of
    /// a NURBS.
    pub fn domain(&self) -> [Interval; 2] {
        match self {
            Surface::Plane { .. } => [Interval::REAL, Interval::REAL],
            Surface::Cylinder { .. } | Surface::Cone { .. } => [Interval::TURN, Interval::REAL],
            Surface::Sphere { .. } => [Interval::TURN, latitude()],
            Surface::Torus { .. } => [Interval::TURN, Interval::TURN],
            Surface::Nurbs(s) => s.domain(),
        }
    }

    /// The period of each parameter, `None` where it is not periodic.
    pub fn period(&self) -> [Option<f64>; 2] {
        match self {
            Surface::Plane { .. } => [None, None],
            Surface::Cylinder { .. } | Surface::Cone { .. } | Surface::Sphere { .. } => {
                [Some(TAU), None]
            }
            Surface::Torus { .. } => [Some(TAU), Some(TAU)],
            Surface::Nurbs(s) => s.period(),
        }
    }

    /// The same surface with its frame moved by `motion`: the
    /// parametrisation is carried along, so `moved.eval(u, v).point ==
    /// motion.apply(self.eval(u, v).point)` to rounding.
    pub fn transformed(&self, motion: &Isometry) -> Surface {
        match self {
            Surface::Nurbs(s) => Surface::Nurbs(s.transformed(motion)),
            &Surface::Plane { frame } => Surface::Plane {
                frame: frame.transformed(motion),
            },
            &Surface::Cylinder { frame, radius } => Surface::Cylinder {
                frame: frame.transformed(motion),
                radius,
            },
            &Surface::Cone {
                frame,
                radius,
                half_angle,
            } => Surface::Cone {
                frame: frame.transformed(motion),
                radius,
                half_angle,
            },
            &Surface::Sphere { frame, radius } => Surface::Sphere {
                frame: frame.transformed(motion),
                radius,
            },
            &Surface::Torus {
                frame,
                major_radius,
                minor_radius,
            } => Surface::Torus {
                frame: frame.transformed(motion),
                major_radius,
                minor_radius,
            },
        }
    }
}

/// A frame's origin and axes as plain vectors.
fn axes(f: &Frame) -> (Point3, Vec3, Vec3, Vec3) {
    (
        f.origin(),
        f.x().into_inner(),
        f.y().into_inner(),
        f.z().into_inner(),
    )
}

/// `[−π/2, π/2]`: the sphere's `v`. Constants in order and not NaN, so the
/// fallback is unreachable.
fn latitude() -> Interval {
    Interval::new(-FRAC_PI_2, FRAC_PI_2).unwrap_or(Interval::UNIT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domains_and_periods_follow_the_table() {
        let f = Frame::world();
        let plane = Surface::Plane { frame: f };
        assert_eq!(plane.domain(), [Interval::REAL, Interval::REAL]);
        assert_eq!(plane.period(), [None, None]);
        let cyl = Surface::Cylinder {
            frame: f,
            radius: 1.0,
        };
        assert_eq!(cyl.domain(), [Interval::TURN, Interval::REAL]);
        assert_eq!(cyl.period(), [Some(TAU), None]);
        let sphere = Surface::Sphere {
            frame: f,
            radius: 1.0,
        };
        assert_eq!(sphere.domain()[1].lo(), -FRAC_PI_2);
        assert_eq!(sphere.period(), [Some(TAU), None]);
        let torus = Surface::Torus {
            frame: f,
            major_radius: 2.0,
            minor_radius: 1.0,
        };
        assert_eq!(torus.domain(), [Interval::TURN, Interval::TURN]);
        assert_eq!(torus.period(), [Some(TAU), Some(TAU)]);
        assert_eq!(torus.kind(), SurfaceKind::Torus);
        assert_eq!(torus.kind().to_string(), "torus");
    }

    #[test]
    fn a_zero_radius_has_no_normal() {
        let f = Frame::world();
        assert!(
            Surface::Cylinder {
                frame: f,
                radius: 0.0
            }
            .normal(1.0, 1.0)
            .is_none()
        );
        assert!(
            Surface::Sphere {
                frame: f,
                radius: 0.0
            }
            .normal(1.0, 1.0)
            .is_none()
        );
        assert!(
            Surface::Torus {
                frame: f,
                major_radius: 1.0,
                minor_radius: 0.0
            }
            .normal(1.0, 1.0)
            .is_none()
        );
        assert!(Surface::Plane { frame: f }.normal(1.0, 1.0).is_some());
    }
}
