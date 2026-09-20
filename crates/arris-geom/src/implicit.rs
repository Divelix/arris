//! An analytic surface as the zero set of a polynomial in its own frame,
//! and whatever is put into it as a polynomial in Bernstein form: a
//! rational curve span (`crate::intersect_spline`) or a rational surface
//! patch (`crate::trace_torus`).
//!
//! Every analytic surface is the zero set of a polynomial `F` of degree
//! `d` — one for a plane, two for a quadric, four for a torus. Rational
//! coordinates `(x, y, z) / w` put into it and cleared of their
//! denominator, `g = wᵈ F((x, y, z) / w)`, are a polynomial with the sign
//! of `F` wherever `w` is positive, built from the homogeneous
//! coordinates by products in the Bernstein basis and never through
//! power coefficients. [`Implicit::along`] is that substitution, over
//! whichever Bernstein arithmetic the coordinates live in
//! ([`BernsteinAlgebra`]): one variable for a curve, two for a patch.
//!
//! What is decided on `g` is only *where to look*. `g` carries a factor
//! the size of the surface's dimensions to the power `d`, so what is
//! there is decided in length, on the exact signed [`Implicit::distance`]
//! and its [`Implicit::gradient`]; [`Implicit::steepness`] is what ties
//! the two scales together where a tolerance in length has to be stated
//! on `g`.

use arris_math::{Frame, Point3, Vec3};

use crate::Surface;
use crate::bernstein::BernsteinAlgebra;
use crate::project::ellipse_nearest;

/// Rounding slack for deciding that a Bernstein coefficient of the
/// substituted polynomial *vanishes*: `|c| ≤ BERNSTEIN_ROUNDING · B`,
/// with `B` the sum of the magnitudes of the terms `c` was summed from
/// ([`Implicit::magnitude`]).
/// A control point moved into the surface's frame carries a few `ε` of
/// its own magnitude, a product in the Bernstein basis twice that, the
/// quartic form nests two products and a difference of the results can
/// cancel all of them: `64ε` covers the chain with slack to spare. Like
/// [`arris_math::roots::POLYNOMIAL_ROUNDING`] it is a statement about `f64`, never a
/// geometric tolerance; what it costs is that an extremum whose whole
/// rise is below it is not told from a flat stretch, and the flat
/// stretch's middle is looked at instead.
pub(crate) const BERNSTEIN_ROUNDING: f64 = 64.0 * f64::EPSILON;

/// A surface as the zero set of a polynomial in its own frame.
pub(crate) struct Implicit<'a> {
    /// The surface's frame, which the polynomial is written in.
    pub(crate) frame: &'a Frame,
    form: Form,
}

/// The polynomial, by the surface's dimensions.
#[derive(Clone, Copy)]
enum Form {
    /// `z`.
    Plane,
    /// `x² + y² − R²`.
    Cylinder { radius: f64 },
    /// `(x / a)² + (y / b)² − 1`.
    EllipticCylinder { a: f64, b: f64 },
    /// `cos²α (x² + y²) − sin²α h²`, `h = z + R cos α / sin α` the height
    /// above the apex: both nappes.
    Cone { radius: f64, sin: f64, cos: f64 },
    /// `x² + y² + z² − R²`.
    Sphere { radius: f64 },
    /// `(x² + y² + z² + R² − r²)² − 4R² (x² + y²)`.
    Torus { major: f64, minor: f64 },
}

impl<'a> Implicit<'a> {
    /// `None` for a NURBS surface, which has no implicit form.
    pub(crate) fn of(surface: &'a Surface) -> Option<Self> {
        let (frame, form) = match surface {
            Surface::Plane { frame } => (frame, Form::Plane),
            Surface::Cylinder { frame, radius } => (frame, Form::Cylinder { radius: *radius }),
            Surface::EllipticCylinder {
                frame,
                major_radius,
                minor_radius,
            } => (
                frame,
                Form::EllipticCylinder {
                    a: *major_radius,
                    b: *minor_radius,
                },
            ),
            Surface::Cone {
                frame,
                radius,
                half_angle,
            } => {
                let (sin, cos) = half_angle.sin_cos();
                (
                    frame,
                    Form::Cone {
                        radius: *radius,
                        sin,
                        cos,
                    },
                )
            }
            Surface::Sphere { frame, radius } => (frame, Form::Sphere { radius: *radius }),
            Surface::Torus {
                frame,
                major_radius,
                minor_radius,
            } => (
                frame,
                Form::Torus {
                    major: *major_radius,
                    minor: *minor_radius,
                },
            ),
            Surface::Nurbs(_) => return None,
        };
        Some(Implicit { frame, form })
    }

    /// The degree of the polynomial.
    pub(crate) fn degree(&self) -> usize {
        match self.form {
            Form::Plane => 1,
            Form::Cylinder { .. }
            | Form::EllipticCylinder { .. }
            | Form::Cone { .. }
            | Form::Sphere { .. } => 2,
            Form::Torus { .. } => 4,
        }
    }

    /// `g` from the homogeneous coordinates in the surface's frame, each
    /// a polynomial in Bernstein form of one degree: a curve span's in
    /// one variable, a surface patch's in two.
    pub(crate) fn along<P: Clone, A: BernsteinAlgebra<P>>(
        &self,
        [x, y, z, w]: [&P; 4],
        algebra: &A,
    ) -> P {
        let square = |p: &P| algebra.mul(p, p);
        match self.form {
            Form::Plane => z.clone(),
            Form::Cylinder { radius } => algebra.combine(&[
                (1.0, &square(x)),
                (1.0, &square(y)),
                (-radius * radius, &square(w)),
            ]),
            Form::EllipticCylinder { a, b } => algebra.combine(&[
                (1.0 / (a * a), &square(x)),
                (1.0 / (b * b), &square(y)),
                (-1.0, &square(w)),
            ]),
            Form::Cone { radius, sin, cos } => {
                let h = algebra.combine(&[(1.0, z), (radius * cos / sin, w)]);
                algebra.combine(&[
                    (cos * cos, &square(x)),
                    (cos * cos, &square(y)),
                    (-sin * sin, &square(&h)),
                ])
            }
            Form::Sphere { radius } => algebra.combine(&[
                (1.0, &square(x)),
                (1.0, &square(y)),
                (1.0, &square(z)),
                (-radius * radius, &square(w)),
            ]),
            Form::Torus { major, minor } => {
                let across = algebra.combine(&[(1.0, &square(x)), (1.0, &square(y))]);
                let ww = square(w);
                let q = algebra.combine(&[
                    (1.0, &across),
                    (1.0, &square(z)),
                    (major * major - minor * minor, &ww),
                ]);
                algebra.combine(&[
                    (1.0, &square(&q)),
                    (-4.0 * major * major, &algebra.mul(&across, &ww)),
                ])
            }
        }
    }

    /// The sum of the magnitudes of the terms of `g`, for homogeneous
    /// coordinates no larger than `reach` and weights no larger than
    /// `weight`: what a coefficient of `g` is zero to rounding against.
    pub(crate) fn magnitude(&self, reach: f64, weight: f64) -> f64 {
        let (l2, w2) = (reach * reach, weight * weight);
        match self.form {
            Form::Plane => reach,
            Form::Cylinder { radius } => 2.0 * l2 + radius * radius * w2,
            Form::EllipticCylinder { a, b } => l2 / (a * a) + l2 / (b * b) + w2,
            Form::Cone { radius, sin, cos } => {
                let h = reach + (radius * cos / sin).abs() * weight;
                2.0 * cos * cos * l2 + sin * sin * h * h
            }
            Form::Sphere { radius } => 3.0 * l2 + radius * radius * w2,
            Form::Torus { major, minor } => {
                let q = 3.0 * l2 + (major * major - minor * minor).abs() * w2;
                q * q + 8.0 * major * major * l2 * w2
            }
        }
    }

    /// A bound on `|∇F|` within `reach` of the frame's origin. `F`
    /// vanishes at the foot of a point on the surface, so by the mean
    /// value theorem along the way there `|F(p)| ≤ steepness · |δ(p)|`
    /// for a point and its foot both within `reach`: a distance below a
    /// tolerance is a value of `F` below `steepness` times it. An
    /// overestimate, which is the side a caller excluding by it needs.
    pub(crate) fn steepness(&self, reach: f64) -> f64 {
        match self.form {
            Form::Plane => 1.0,
            Form::Cylinder { .. } | Form::Sphere { .. } => 2.0 * reach,
            Form::EllipticCylinder { b, .. } => 2.0 * reach / (b * b),
            Form::Cone { radius, sin, cos } => {
                let h = reach + (radius * cos / sin).abs();
                2.0 * (cos * cos * reach + sin * sin * h)
            }
            Form::Torus { major, minor } => {
                let q = reach * reach + (major * major - minor * minor).abs();
                4.0 * q * reach + 8.0 * major * major * reach
            }
        }
    }

    /// The signed distance from `p` to the surface, with the sign of the
    /// polynomial: negative behind a plane, inside a cylinder, a nappe of
    /// a cone, a sphere or a torus's tube. Exact — for the cone the
    /// distance to the nearer ruling in the half-plane through the
    /// point, as the line arm walks it; for the elliptic cylinder the
    /// distance to the section through its nearest point, or to the
    /// section's point at the query's own eccentric anomaly where that
    /// is not unique, deep inside.
    pub(crate) fn distance(&self, p: Point3) -> f64 {
        let q = self.frame.to_local(p);
        let rho = q.x.hypot(q.y);
        match self.form {
            Form::Plane => q.z,
            Form::Cylinder { radius } => rho - radius,
            Form::EllipticCylinder { a, b } => {
                let (_, foot) = self.section_foot(a, b, p);
                let away = (q.x - foot.x).hypot(q.y - foot.y);
                if (q.x / a).powi(2) + (q.y / b).powi(2) < 1.0 {
                    -away
                } else {
                    away
                }
            }
            Form::Cone { radius, sin, cos } => cos * rho - sin * (q.z + radius * cos / sin).abs(),
            Form::Sphere { radius } => q.coords.norm() - radius,
            Form::Torus { major, minor } => (rho - major).hypot(q.z) - minor,
        }
    }

    /// The gradient of [`Self::distance`] at `p`, in the surface's frame:
    /// a unit vector wherever the distance is smooth, zero on the locus
    /// where it has none (an axis, a centre), which sends the crossing's
    /// Newton step to a bisection.
    pub(crate) fn gradient(&self, p: Point3) -> Vec3 {
        let q = self.frame.to_local(p);
        let rho = q.x.hypot(q.y);
        let radial = if rho > 0.0 {
            Vec3::new(q.x / rho, q.y / rho, 0.0)
        } else {
            Vec3::zeros()
        };
        match self.form {
            Form::Plane => Vec3::z(),
            Form::Cylinder { .. } => radial,
            Form::EllipticCylinder { a, b } => {
                let (t, _) = self.section_foot(a, b, p);
                let (st, ct) = t.sin_cos();
                Vec3::new(b * ct, a * st, 0.0)
                    .try_normalize(0.0)
                    .unwrap_or_else(Vec3::zeros)
            }
            Form::Cone { radius, sin, cos } => {
                let h = q.z + radius * cos / sin;
                cos * radial - sin * h.signum() * Vec3::z()
            }
            Form::Sphere { .. } => q.coords.try_normalize(0.0).unwrap_or_else(Vec3::zeros),
            Form::Torus { major, .. } => {
                let tube = (rho - major).hypot(q.z);
                if tube > 0.0 {
                    ((rho - major) * radial + q.z * Vec3::z()) / tube
                } else {
                    Vec3::zeros()
                }
            }
        }
    }

    /// A function of `p` with the surface's sign that vanishes on it and
    /// nowhere else, with its gradient in the surface's frame, as cheap
    /// as a root-finder calling it in a loop needs: [`Self::distance`]
    /// and [`Self::gradient`] where those are closed forms, and for the
    /// elliptic cylinder — whose distance is an iteration of its own —
    /// the polynomial over its gradient's norm, which is the distance to
    /// first order, with the unit vector along that gradient. What is
    /// decided *in length* is decided on the distance, never on this.
    pub(crate) fn level(&self, p: Point3) -> (f64, Vec3) {
        let Form::EllipticCylinder { a, b } = self.form else {
            return (self.distance(p), self.gradient(p));
        };
        let q = self.frame.to_local(p);
        let value = (q.x / a).powi(2) + (q.y / b).powi(2) - 1.0;
        let slope = Vec3::new(2.0 * q.x / (a * a), 2.0 * q.y / (b * b), 0.0);
        let steep = slope.norm();
        if steep > 0.0 {
            (value / steep, slope / steep)
        } else {
            // On the axis: inside, as far as the smaller half-axis.
            (-a.min(b), Vec3::zeros())
        }
    }

    /// The eccentric anomaly of the point of the section `(a cos t, b sin
    /// t)` nearest `p` across the axis, and that point; the query's own
    /// anomaly where the nearest is not unique.
    fn section_foot(&self, a: f64, b: f64, p: Point3) -> (f64, arris_math::Point2) {
        let q = self.frame.to_local(p);
        let noise = p.coords.norm() + self.frame.origin().coords.norm();
        let t = match ellipse_nearest(a, b, q.x, q.y, noise) {
            Ok((t, _)) => t,
            Err(_) => (q.y / b).atan2(q.x / a),
        };
        let (st, ct) = t.sin_cos();
        (t, arris_math::Point2::new(a * ct, b * st))
    }
}
