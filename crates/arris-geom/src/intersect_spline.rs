//! A `Curve::Nurbs` against every analytic surface: each span of the
//! curve substituted into the surface's implicit polynomial, in
//! Bernstein form (ADR-0018; `docs/DATA-MODEL.md` §Curves).
//!
//! Every analytic surface is the zero set of a polynomial `F` of degree
//! `d` in its own frame — one for a plane, two for a quadric, four for a
//! torus. A rational span `C(s) = A(s) / w(s)` of degree `p` put into it
//! and cleared of its denominator, `g(s) = w(s)ᵈ F(A(s) / w(s))`, is a
//! polynomial of degree `d·p` with the sign of `F` along the span, the
//! weights being positive. `g` is built from the span's homogeneous
//! Bézier points by products in the Bernstein basis
//! (`crate::bernstein`), never through power coefficients.
//!
//! What is decided on `g` is only *where to look*: the sign changes of
//! `g′` are the extrema of `g`, between two of which `g` is monotone and
//! crosses zero at most once. What is decided there is decided in
//! length, as every other arm of the table decides it: on the signed
//! **distance** `δ(t)` from the curve's point to the surface, the exact
//! one the line arms walk. `g = φ·δ` with `φ > 0` wherever `δ` is small,
//! so near the surface an extremum of `g` is an extremum of `δ` to
//! second order in `δ`, and `g` and `δ` change sign together.
//!
//! `IntCurveSurface`'s polynomial case and `IntAna_IntConicQuad` in the
//! reference tree were read for how a conic is put into a quadric;
//! nothing of either is here — they solve in power coefficients.

use arris_math::roots;
use arris_math::{Frame, Interval, Point3, Tolerance, Vec3};

use crate::bernstein::{Binomials, combine, derivative, mul, sign_change_candidates};
use crate::intersect_curve::{hit, points};
use crate::nurbs::BezierSpan;
use crate::project::ellipse_nearest;
use crate::{Curve, CurveSurfaceIntersection, GeomError, GeomKind, NurbsCurve, Surface};

/// Rounding slack for deciding that a Bernstein coefficient of the
/// substituted polynomial *vanishes*: `|c| ≤ BERNSTEIN_ROUNDING · B`,
/// with `B` the sum of the magnitudes of the terms `c` was summed from.
/// A control point moved into the surface's frame carries a few `ε` of
/// its own magnitude, a product in the Bernstein basis twice that, the
/// quartic form nests two products and a difference of the results can
/// cancel all of them: `64ε` covers the chain with slack to spare. Like
/// [`roots::POLYNOMIAL_ROUNDING`] it is a statement about `f64`, never a
/// geometric tolerance; what it costs is that an extremum whose whole
/// rise is below it is not told from a flat stretch, and the flat
/// stretch's middle is looked at instead.
const BERNSTEIN_ROUNDING: f64 = 64.0 * f64::EPSILON;

/// A surface as the zero set of a polynomial in its own frame.
struct Implicit<'a> {
    frame: &'a Frame,
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
    fn of(surface: &'a Surface) -> Option<Self> {
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
    fn degree(&self) -> usize {
        match self.form {
            Form::Plane => 1,
            Form::Cylinder { .. }
            | Form::EllipticCylinder { .. }
            | Form::Cone { .. }
            | Form::Sphere { .. } => 2,
            Form::Torus { .. } => 4,
        }
    }

    /// `g` over one span, from the span's homogeneous coordinates in the
    /// surface's frame, each in Bernstein form.
    fn along(&self, [x, y, z, w]: [&[f64]; 4], binomials: &Binomials) -> Vec<f64> {
        let square = |p: &[f64]| mul(p, p, binomials);
        match self.form {
            Form::Plane => z.to_vec(),
            Form::Cylinder { radius } => combine(&[
                (1.0, &square(x)),
                (1.0, &square(y)),
                (-radius * radius, &square(w)),
            ]),
            Form::EllipticCylinder { a, b } => combine(&[
                (1.0 / (a * a), &square(x)),
                (1.0 / (b * b), &square(y)),
                (-1.0, &square(w)),
            ]),
            Form::Cone { radius, sin, cos } => {
                let h = combine(&[(1.0, z), (radius * cos / sin, w)]);
                combine(&[
                    (cos * cos, &square(x)),
                    (cos * cos, &square(y)),
                    (-sin * sin, &square(&h)),
                ])
            }
            Form::Sphere { radius } => combine(&[
                (1.0, &square(x)),
                (1.0, &square(y)),
                (1.0, &square(z)),
                (-radius * radius, &square(w)),
            ]),
            Form::Torus { major, minor } => {
                let across = combine(&[(1.0, &square(x)), (1.0, &square(y))]);
                let ww = square(w);
                let q = combine(&[
                    (1.0, &across),
                    (1.0, &square(z)),
                    (major * major - minor * minor, &ww),
                ]);
                combine(&[
                    (1.0, &square(&q)),
                    (-4.0 * major * major, &mul(&across, &ww, binomials)),
                ])
            }
        }
    }

    /// The sum of the magnitudes of the terms of `g`, for homogeneous
    /// coordinates no larger than `reach` and weights no larger than
    /// `weight`: what a coefficient of `g` is zero to rounding against.
    fn magnitude(&self, reach: f64, weight: f64) -> f64 {
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

    /// The signed distance from `p` to the surface, with the sign of the
    /// polynomial: negative behind a plane, inside a cylinder, a nappe of
    /// a cone, a sphere or a torus's tube. Exact — for the cone the
    /// distance to the nearer ruling in the half-plane through the
    /// point, as the line arm walks it; for the elliptic cylinder the
    /// distance to the section through its nearest point, or to the
    /// section's point at the query's own eccentric anomaly where that
    /// is not unique, deep inside.
    fn distance(&self, p: Point3) -> f64 {
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
    fn gradient(&self, p: Point3) -> Vec3 {
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

/// A parameter at which the distance is looked at: an extremum of it
/// among its neighbours, or an end of an open curve.
#[derive(Clone, Copy)]
struct Stop {
    t: f64,
    distance: f64,
    /// Within `tol.linear` of the surface.
    touch: bool,
    /// An extremum inside the curve rather than an end of it.
    interior: bool,
}

/// A rational B-spline curve against an analytic surface.
///
/// The parameters looked at are every distinct knot of the domain and,
/// on every span, every candidate for a sign change of `g′`
/// ([`sign_change_candidates`], against [`BERNSTEIN_ROUNDING`] of the
/// span's magnitude): between two consecutive ones `g` is monotone. Of
/// those, the **stops** are the ones where the distance `δ` is an
/// extremum among its neighbours, and the two ends of a curve that is
/// not periodic, which are extrema of a function on a closed interval.
/// Then, as `line_by_distance` has it: every stop within `tol.linear`
/// is the whole curve within it, `Coincident`; a stop within
/// `tol.linear` is a hit that absorbs the crossings on the stretches
/// beside it, and a run of such stops with no other between them is one
/// hit, at the one nearest the surface; two consecutive other stops of
/// opposite sign hold one crossing, by bracketed Newton on `δ`. A hit at
/// a stop is `tangent` when its run holds an extremum inside the curve:
/// a curve that only *ends* within `tol.linear` of the surface meets it
/// there without touching it — that is a section edge ending on a face,
/// which the pave model makes a vertex of, not a graze. A periodic
/// curve's stops go round, and its hits come back in `[knots[p],
/// knots[n])`.
pub(crate) fn spline_surface(
    curve: &Curve,
    spline: &NurbsCurve,
    surface: &Surface,
    tol: Tolerance,
) -> Result<CurveSurfaceIntersection, GeomError> {
    let unsupported = || GeomError::Unsupported {
        a: GeomKind::Curve(curve.kind()),
        b: GeomKind::Surface(surface.kind()),
    };
    let implicit = Implicit::of(surface).ok_or_else(unsupported)?;
    let domain = spline.domain();
    let period = spline.period();
    let binomials = Binomials::new(implicit.degree() * spline.degree());

    let mut splits: Vec<f64> = Vec::new();
    for span in spline.bezier_spans() {
        splits.push(span.lo);
        splits.push(span.hi);
        let width = span.hi - span.lo;
        splits.extend(
            extrema_on(&implicit, &span, &binomials)
                .into_iter()
                .map(|s| span.lo + width * s),
        );
    }
    // A periodic curve's last knot is its first again.
    if let Some(period) = period {
        for t in &mut splits {
            if *t >= domain.hi() {
                *t -= period;
            }
            *t = t.max(domain.lo());
        }
    }
    splits.retain(|t| t.is_finite());
    splits.sort_by(f64::total_cmp);
    splits.dedup();
    if splits.is_empty() {
        // A validated curve has a span; one whose knots are not finite
        // numbers could not have been built.
        return Ok(CurveSurfaceIntersection::Points(Vec::new()));
    }

    let distance = |t: f64| implicit.distance(spline.eval(t).point);
    let slope = |t: f64| {
        let e = spline.eval(t);
        implicit
            .gradient(e.point)
            .dot(&implicit.frame.vec_to_local(e.d1))
    };
    let values: Vec<f64> = splits.iter().map(|&t| distance(t)).collect();
    let n = splits.len();
    let cyclic = period.is_some();
    let neighbour = |i: usize, step: isize| -> Option<f64> {
        let j = i as isize + step;
        if cyclic {
            Some(values[j.rem_euclid(n as isize) as usize])
        } else if j < 0 || j >= n as isize {
            None
        } else {
            Some(values[j as usize])
        }
    };
    let mut stops: Vec<Stop> = Vec::new();
    for i in 0..n {
        let v = values[i];
        let (before, after) = (neighbour(i, -1), neighbour(i, 1));
        let low = before.is_none_or(|b| b >= v) && after.is_none_or(|a| a >= v);
        let high = before.is_none_or(|b| b <= v) && after.is_none_or(|a| a <= v);
        if low || high {
            stops.push(Stop {
                t: splits[i],
                distance: v,
                touch: v.abs() <= tol.linear,
                interior: before.is_some() && after.is_some(),
            });
        }
    }
    if stops.iter().all(|s| s.touch) {
        return Ok(CurveSurfaceIntersection::Coincident);
    }
    if cyclic {
        // Started at a stop clear of the surface, no run of touches goes
        // round the end of the list.
        let first = stops.iter().position(|s| !s.touch).unwrap_or(0);
        stops.rotate_left(first);
    }
    let mut runs: Vec<Stop> = Vec::with_capacity(stops.len());
    for stop in stops {
        match runs.last_mut() {
            Some(last) if last.touch && stop.touch => {
                let interior = last.interior || stop.interior;
                if stop.distance.abs() < last.distance.abs() {
                    *last = stop;
                }
                last.interior = interior;
            }
            _ => runs.push(stop),
        }
    }

    let crossing = |lo: f64, hi: f64| -> Result<f64, GeomError> {
        let degenerate = |reason: String| GeomError::Degenerate {
            kind: GeomKind::Curve(curve.kind()),
            reason,
        };
        let bracket = Interval::new(lo, hi)
            .map_err(|_| degenerate(format!("crossing bracket [{lo}, {hi}]")))?;
        roots::newton_in_interval(distance, slope, bracket, 0.0)
            .map_err(|e| degenerate(format!("crossing in [{lo}, {hi}]: {e}")))
    };
    let mut hits = Vec::new();
    for (i, stop) in runs.iter().enumerate() {
        if stop.touch {
            hits.push(hit(curve, surface, stop.t, stop.interior)?);
        }
        // The stretch to the next stop; a periodic curve's goes round,
        // past the end of the domain wherever the next stop is behind.
        let next = match (runs.get(i + 1), period) {
            (Some(next), _) => *next,
            (None, Some(_)) if runs.len() > 1 => runs[0],
            (None, _) => continue,
        };
        let next_t = match period {
            Some(period) if next.t <= stop.t => next.t + period,
            _ => next.t,
        };
        if stop.touch || next.touch || (stop.distance < 0.0) == (next.distance < 0.0) {
            continue;
        }
        let mut t = crossing(stop.t, next_t)?;
        if let Some(period) = period {
            if t >= domain.hi() {
                t = (t - period).max(domain.lo());
            }
        }
        hits.push(hit(curve, surface, t, false)?);
    }
    Ok(points(hits))
}

/// The candidates for an extremum of `g` on one span, as parameters of
/// the span's own `[0, 1]`.
fn extrema_on(implicit: &Implicit<'_>, span: &BezierSpan<3>, binomials: &Binomials) -> Vec<f64> {
    let origin = implicit.frame.origin().coords;
    let mut coords: [Vec<f64>; 4] = Default::default();
    let (mut reach, mut weight) = (0.0f64, 0.0f64);
    for (a, w) in &span.control {
        // `w·P` in the frame: `w (P − O)` turned into it.
        let local = implicit.frame.vec_to_local(a - *w * origin);
        coords[0].push(local.x);
        coords[1].push(local.y);
        coords[2].push(local.z);
        coords[3].push(*w);
        reach = reach.max(a.norm() + w * origin.norm());
        weight = weight.max(*w);
    }
    let [x, y, z, w] = &coords;
    let g = implicit.along([x, y, z, w], binomials);
    let degree = g.len().saturating_sub(1);
    // A derivative's coefficient is `degree` times a difference of two
    // of `g`'s.
    let floor = BERNSTEIN_ROUNDING * implicit.magnitude(reach, weight) * 2.0 * degree as f64;
    sign_change_candidates(&derivative(&g), floor)
}
