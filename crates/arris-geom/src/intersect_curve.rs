//! Curve–surface intersection: the closed-form table of cycle 1.

use core::f64::consts::{PI, TAU};

use arris_math::roots::{self, RootError};
use arris_math::{Frame, Interval, Point2, Point3, Tolerance, Vec2, is_negligible};

use crate::project::wrap_turn;
use crate::{Curve, CurveKind, GeomError, GeomKind, Surface};

/// One point where a curve meets a surface.
///
/// `point` is the curve's point at `t`; the surface's point at `uv` is
/// within the linear tolerance of it — to rounding for a transversal hit,
/// where the curve crosses the surface, and within `tol.linear` for a
/// `tangent` one, where the curve's nearest approach to the surface is
/// that close and counts as a touch. `t` lies in the curve's domain: a
/// periodic parameter in `[0, 2π)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurveSurfaceHit {
    /// The curve parameter.
    pub t: f64,
    /// The surface parameters of the nearest surface point, from
    /// [`Surface::project`].
    pub uv: Point2,
    /// `curve.point(t)`.
    pub point: Point3,
    /// `true` when the curve touches the surface here without crossing
    /// it: the signed distance along the curve has an extremum within
    /// `tol.linear` of zero. Two crossings that close are one touch.
    pub tangent: bool,
}

/// What a curve and a surface have in common.
#[derive(Debug, Clone, PartialEq)]
pub enum CurveSurfaceIntersection {
    /// The hits, ascending by `t`; empty when the curve misses the
    /// surface.
    Points(Vec<CurveSurfaceHit>),
    /// The curve lies on the surface within the tolerance; there is no
    /// point to return.
    Coincident,
}

/// The intersection of a curve and a surface, by the case table of cycle
/// 1: every pair with a closed form is computed exactly, every other pair
/// is an explicit [`GeomError::Unsupported`] arm — no wildcard, no
/// marcher.
///
/// Guarantees: hits are sorted by `t`, each `t` is in the curve's domain,
/// each hit's `uv` is the surface's own projection of the point, the
/// result is deterministic bit for bit, and a transversal hit lies on
/// both operands to rounding. The decisions: a line is parallel to a
/// plane or a cylinder's axis within `tol.angular`, and then coincident
/// or not within `tol.linear`; a circle is coincident when it lies within
/// `tol.linear` of the surface everywhere, which its extrema of distance
/// decide; a hit is `tangent` where the distance along the curve has an
/// extremum within `tol.linear` of zero, and the crossings that extremum
/// would split into are reported as that one touch.
///
/// The table: line–plane is one hit, none (parallel), or `Coincident`;
/// line–cylinder is two hits, one tangent hit, none, or `Coincident` for
/// a ruling; circle–plane is two hits, one tangent hit, none, or
/// `Coincident`; circle–cylinder is up to four hits, found as the sign
/// changes of the radial distance between its extrema — the quartic of
/// [`arris_math::roots`] in the half-angle `tan(t/2)` locates the
/// extrema, bracketed Newton the crossings — with `Coincident` for a
/// parallel of the cylinder. Read `IntAna_IntConicQuad` in the reference
/// tree for the case analysis, reimplemented on our frames.
///
/// ```
/// use arris_geom::{Curve, CurveSurfaceIntersection, Surface, intersect_curve_surface};
/// use arris_math::{Frame, Point3, Precision, Vec3};
///
/// let wall = Surface::Cylinder { frame: Frame::world(), radius: 2.0 };
/// let ray = Curve::Line { origin: Point3::new(0.0, 0.0, 1.0), direction: Vec3::x_axis() };
/// let hit = intersect_curve_surface(&ray, &wall, Precision::DEFAULT.tolerance()).unwrap();
/// let CurveSurfaceIntersection::Points(hits) = hit else { panic!() };
/// assert_eq!(hits.len(), 2);
/// assert!((hits[0].t + 2.0).abs() < 1e-15 && (hits[1].t - 2.0).abs() < 1e-15);
/// assert!(!hits[0].tangent);
/// assert_eq!(hits[1].uv.y, 1.0);
/// ```
pub fn intersect_curve_surface(
    curve: &Curve,
    surface: &Surface,
    tol: Tolerance,
) -> Result<CurveSurfaceIntersection, GeomError> {
    if !tol.is_consistent() {
        return Err(GeomError::InvalidTolerance(tol));
    }
    match (curve, surface) {
        (Curve::Line { origin, direction }, Surface::Plane { frame }) => {
            line_plane(curve, surface, *origin, direction.into_inner(), frame, tol)
        }
        (Curve::Line { origin, direction }, Surface::Cylinder { frame, radius }) => line_cylinder(
            curve,
            surface,
            *origin,
            direction.into_inner(),
            frame,
            *radius,
            tol,
        ),
        (Curve::Circle { frame: cf, radius }, Surface::Plane { frame }) => {
            circle_plane(curve, surface, cf, *radius, frame, tol)
        }
        (Curve::Circle { frame: cf, radius }, Surface::Cylinder { frame, radius: big }) => {
            circle_cylinder(curve, surface, cf, *radius, frame, *big, tol)
        }
        (
            Curve::Line { .. } | Curve::Circle { .. } | Curve::Ellipse { .. },
            Surface::Plane { .. }
            | Surface::Cylinder { .. }
            | Surface::Cone { .. }
            | Surface::Sphere { .. }
            | Surface::Torus { .. },
        ) => Err(GeomError::Unsupported {
            a: GeomKind::Curve(curve.kind()),
            b: GeomKind::Surface(surface.kind()),
        }),
    }
}

/// A hit at `t`: the curve's point there, projected onto the surface for
/// its `uv`.
fn hit(
    curve: &Curve,
    surface: &Surface,
    t: f64,
    tangent: bool,
) -> Result<CurveSurfaceHit, GeomError> {
    let point = curve.point(t);
    let uv = surface.project(point)?.uv;
    Ok(CurveSurfaceHit {
        t,
        uv,
        point,
        tangent,
    })
}

/// `hits` sorted by `t`; a total order, since every `t` is finite.
fn points(mut hits: Vec<CurveSurfaceHit>) -> CurveSurfaceIntersection {
    hits.sort_by(|a, b| a.t.total_cmp(&b.t));
    CurveSurfaceIntersection::Points(hits)
}

fn line_plane(
    curve: &Curve,
    surface: &Surface,
    origin: Point3,
    direction: arris_math::Vec3,
    plane: &Frame,
    tol: Tolerance,
) -> Result<CurveSurfaceIntersection, GeomError> {
    let n = plane.z();
    // The angle between the line and the plane: the complement of the
    // angle to the normal, from the sine and cosine of the latter.
    let along = n.dot(&direction);
    let across = n.cross(&direction).norm();
    let height = n.dot(&(origin - plane.origin()));
    if along.abs().atan2(across) <= tol.angular {
        return Ok(if height.abs() <= tol.linear {
            CurveSurfaceIntersection::Coincident
        } else {
            CurveSurfaceIntersection::Points(Vec::new())
        });
    }
    let t = -height / along;
    Ok(points(vec![hit(curve, surface, t, false)?]))
}

fn line_cylinder(
    curve: &Curve,
    surface: &Surface,
    origin: Point3,
    direction: arris_math::Vec3,
    cyl: &Frame,
    radius: f64,
    tol: Tolerance,
) -> Result<CurveSurfaceIntersection, GeomError> {
    let q = cyl.to_local(origin);
    let d = cyl.vec_to_local(direction);
    let (q2, d2) = (Vec2::new(q.x, q.y), Vec2::new(d.x, d.y));
    let across = d2.norm();
    if across.atan2(d.z.abs()) <= tol.angular {
        // Along the axis: a ruling of the cylinder, or clear of it.
        return Ok(if (q2.norm() - radius).abs() <= tol.linear {
            CurveSurfaceIntersection::Coincident
        } else {
            CurveSurfaceIntersection::Points(Vec::new())
        });
    }
    // In the plane across the axis the line passes nearest the axis at
    // `t0`, at distance `dist`; the chord inside the circle of radius `R`
    // is symmetric about it.
    let t0 = -q2.dot(&d2) / (across * across);
    let dist = (q2 + t0 * d2).norm();
    if (dist - radius).abs() <= tol.linear {
        return Ok(points(vec![hit(curve, surface, t0, true)?]));
    }
    if dist >= radius {
        return Ok(CurveSurfaceIntersection::Points(Vec::new()));
    }
    let half = (radius - dist).sqrt() * (radius + dist).sqrt() / across;
    Ok(points(vec![
        hit(curve, surface, t0 - half, false)?,
        hit(curve, surface, t0 + half, false)?,
    ]))
}

fn circle_plane(
    curve: &Curve,
    surface: &Surface,
    circle: &Frame,
    radius: f64,
    plane: &Frame,
    tol: Tolerance,
) -> Result<CurveSurfaceIntersection, GeomError> {
    // The signed distance along the circle is `h + M cos(t − φ)`: `h` at
    // the centre, `M` the reach of the circle out of the plane.
    let n = plane.z();
    let h = n.dot(&(circle.origin() - plane.origin()));
    let a = radius * n.dot(&circle.x());
    let b = radius * n.dot(&circle.y());
    let reach = a.hypot(b);
    let phase = b.atan2(a);
    // The extrema: `h + M` at `φ`, `h − M` at `φ + π`. Both within the
    // tolerance is coincident; one is a touch.
    let far_touch = (h + reach).abs() <= tol.linear;
    let near_touch = (h - reach).abs() <= tol.linear;
    if far_touch && near_touch {
        return Ok(CurveSurfaceIntersection::Coincident);
    }
    if far_touch {
        return Ok(points(vec![hit(curve, surface, wrap_turn(phase), true)?]));
    }
    if near_touch {
        return Ok(points(vec![hit(
            curve,
            surface,
            wrap_turn(phase + PI),
            true,
        )?]));
    }
    if h.abs() >= reach {
        return Ok(CurveSurfaceIntersection::Points(Vec::new()));
    }
    // `cos(t − φ) = −h / M`, with the sine from the difference of squares
    // so a root near the touch keeps its digits.
    let sine = (reach - h).sqrt() * (reach + h).sqrt();
    let half = sine.atan2(-h);
    Ok(points(vec![
        hit(curve, surface, wrap_turn(phase - half), false)?,
        hit(curve, surface, wrap_turn(phase + half), false)?,
    ]))
}

/// The radial reach of a circle in a cylinder's local frame: the squared
/// distance from the axis along the circle is
/// `ρ²(t) = |p + cos t·X + sin t·Y|²`, with `p` the centre across the
/// axis and `X`, `Y` the radius-scaled axes across it.
struct Radial {
    p: Vec2,
    x: Vec2,
    y: Vec2,
    radius: f64,
}

impl Radial {
    /// `ρ(t) − R`, the signed distance from the cylinder along the circle
    /// (negative inside), whose sign changes are the crossings.
    fn distance(&self, t: f64) -> f64 {
        let (st, ct) = t.sin_cos();
        (self.p + ct * self.x + st * self.y).norm() - self.radius
    }

    /// `d(ρ − R)/dt = (P · P′) / ρ`.
    fn slope(&self, t: f64) -> f64 {
        let (st, ct) = t.sin_cos();
        let point = self.p + ct * self.x + st * self.y;
        let rho = point.norm();
        if rho == 0.0 {
            return 0.0;
        }
        point.dot(&(-st * self.x + ct * self.y)) / rho
    }

    /// The critical parameters of `ρ²` in `[0, 2π)`, ascending: the roots
    /// of `P · P′ = a₁ cos t + b₁ sin t + a₂ cos 2t + b₂ sin 2t`, a
    /// trigonometric polynomial of degree two, through the quartic in
    /// `s = tan(t/2)` that `(1 + s²)²` times it is, plus `t = π` when the
    /// leading coefficient vanishes and the root sits at `s = ∞`. Each
    /// candidate is polished by Newton on the trigonometric form. `None`
    /// when the polynomial vanishes identically: `ρ` is constant and the
    /// circle is a parallel of the axis.
    fn critical_parameters(&self) -> Result<Option<Vec<f64>>, GeomError> {
        let a1 = self.p.dot(&self.y);
        let b1 = -self.p.dot(&self.x);
        let a2 = self.x.dot(&self.y);
        let b2 = 0.5 * (self.y.norm_squared() - self.x.norm_squared());
        let c4 = a2 - a1;
        let c3 = 2.0 * b1 - 4.0 * b2;
        let c2 = -6.0 * a2;
        let c1 = 2.0 * b1 + 4.0 * b2;
        let c0 = a1 + a2;
        let scale = c3.abs().max(c2.abs()).max(c1.abs()).max(c0.abs());
        let at_pi = is_negligible(c4, scale);
        let found = if at_pi {
            roots::cubic(c3, c2, c1, c0)
        } else {
            roots::quartic(c4, c3, c2, c1, c0)
        };
        let found = match found {
            Ok(r) => r,
            Err(RootError::Zero) => return Ok(None),
            Err(e) => {
                return Err(GeomError::Degenerate {
                    kind: GeomKind::Curve(CurveKind::Circle),
                    reason: format!("radial extrema: {e}"),
                });
            }
        };
        let f = |t: f64| {
            let (st, ct) = t.sin_cos();
            let (s2, c2) = (2.0 * t).sin_cos();
            a1 * ct + b1 * st + a2 * c2 + b2 * s2
        };
        let df = |t: f64| {
            let (st, ct) = t.sin_cos();
            let (s2, c2) = (2.0 * t).sin_cos();
            -a1 * st + b1 * ct - 2.0 * a2 * s2 + 2.0 * b2 * c2
        };
        let mut out: Vec<f64> = found
            .iter()
            .map(|r| 2.0 * r.value.atan())
            .chain(at_pi.then_some(PI))
            .map(|t0| {
                let mut t = t0;
                for _ in 0..NEWTON_POLISH_STEPS {
                    let (ft, dft) = (f(t), df(t));
                    if ft == 0.0 || dft == 0.0 {
                        break;
                    }
                    let next = t - ft / dft;
                    if f(next).abs() < ft.abs() {
                        t = next;
                    } else {
                        break;
                    }
                }
                wrap_turn(t)
            })
            .collect();
        out.sort_by(f64::total_cmp);
        out.dedup();
        if out.is_empty() {
            // A trigonometric polynomial that is not identically zero has
            // a maximum and a minimum; reaching here means its coefficients
            // are rounding noise, which is the constant case.
            return Ok(None);
        }
        Ok(Some(out))
    }
}

/// Newton steps that polish a candidate parameter from the half-angle
/// quartic: two suffice from a root accurate to rounding, and each is
/// taken only while it reduces the residual.
const NEWTON_POLISH_STEPS: usize = 3;

fn circle_cylinder(
    curve: &Curve,
    surface: &Surface,
    circle: &Frame,
    small: f64,
    cyl: &Frame,
    radius: f64,
    tol: Tolerance,
) -> Result<CurveSurfaceIntersection, GeomError> {
    let centre = cyl.to_local(circle.origin());
    let x = cyl.vec_to_local(small * circle.x().into_inner());
    let y = cyl.vec_to_local(small * circle.y().into_inner());
    let radial = Radial {
        p: Vec2::new(centre.x, centre.y),
        x: Vec2::new(x.x, x.y),
        y: Vec2::new(y.x, y.y),
        radius,
    };
    let Some(critical) = radial.critical_parameters()? else {
        // Constant distance from the axis: a parallel, on the cylinder
        // or not.
        return Ok(if radial.distance(0.0).abs() <= tol.linear {
            CurveSurfaceIntersection::Coincident
        } else {
            CurveSurfaceIntersection::Points(Vec::new())
        });
    };
    // Between consecutive extrema the distance is monotone, so each arc
    // holds at most one crossing; an extremum within the tolerance is a
    // touch that absorbs the crossings on the arcs beside it, and every
    // extremum within it is the whole circle within it.
    let touches: Vec<bool> = critical
        .iter()
        .map(|&t| radial.distance(t).abs() <= tol.linear)
        .collect();
    if touches.iter().all(|&touch| touch) {
        return Ok(CurveSurfaceIntersection::Coincident);
    }
    let mut hits = Vec::new();
    let n = critical.len();
    for i in 0..n {
        let (lo, lo_touch) = (critical[i], touches[i]);
        let (mut hi, hi_touch) = (critical[(i + 1) % n], touches[(i + 1) % n]);
        if lo_touch {
            hits.push(hit(curve, surface, lo, true)?);
        }
        if lo_touch || hi_touch {
            continue;
        }
        if hi <= lo {
            hi += TAU;
        }
        let (d_lo, d_hi) = (radial.distance(lo), radial.distance(hi));
        if (d_lo < 0.0) == (d_hi < 0.0) {
            continue;
        }
        let Ok(bracket) = Interval::new(lo, hi) else {
            continue;
        };
        let t =
            roots::newton_in_interval(|t| radial.distance(t), |t| radial.slope(t), bracket, 0.0)
                .map_err(|e| GeomError::Degenerate {
                    kind: GeomKind::Curve(CurveKind::Circle),
                    reason: format!("crossing in [{lo}, {hi}]: {e}"),
                })?;
        // The last arc wraps past 2π; the root comes back with it.
        hits.push(hit(curve, surface, wrap_turn(t.rem_euclid(TAU)), false)?);
    }
    Ok(points(hits))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arris_math::{Precision, Vec3};

    fn tol() -> Tolerance {
        Precision::DEFAULT.tolerance()
    }

    #[test]
    fn a_line_in_the_plane_is_coincident_and_a_lifted_one_misses() {
        let plane = Surface::Plane {
            frame: Frame::world(),
        };
        let flat = Curve::Line {
            origin: Point3::new(1.0, 2.0, 0.0),
            direction: Vec3::x_axis(),
        };
        let lifted = Curve::Line {
            origin: Point3::new(1.0, 2.0, 3.0),
            direction: Vec3::x_axis(),
        };
        assert_eq!(
            intersect_curve_surface(&flat, &plane, tol()).unwrap(),
            CurveSurfaceIntersection::Coincident
        );
        assert_eq!(
            intersect_curve_surface(&lifted, &plane, tol()).unwrap(),
            CurveSurfaceIntersection::Points(Vec::new())
        );
    }

    #[test]
    fn a_parallel_of_the_cylinder_is_coincident() {
        let wall = Surface::Cylinder {
            frame: Frame::world(),
            radius: 2.0,
        };
        let ring = Curve::Circle {
            frame: Frame::from_z(Point3::new(0.0, 0.0, 3.0), Vec3::z()).unwrap(),
            radius: 2.0,
        };
        assert_eq!(
            intersect_curve_surface(&ring, &wall, tol()).unwrap(),
            CurveSurfaceIntersection::Coincident
        );
        let inner = Curve::Circle {
            frame: Frame::world(),
            radius: 1.0,
        };
        assert_eq!(
            intersect_curve_surface(&inner, &wall, tol()).unwrap(),
            CurveSurfaceIntersection::Points(Vec::new())
        );
    }

    #[test]
    fn a_meridional_circle_crosses_four_times() {
        let wall = Surface::Cylinder {
            frame: Frame::world(),
            radius: 1.0,
        };
        let circle = Curve::Circle {
            frame: Frame::from_z(Point3::origin(), Vec3::y()).unwrap(),
            radius: 2.0,
        };
        let CurveSurfaceIntersection::Points(hits) =
            intersect_curve_surface(&circle, &wall, tol()).unwrap()
        else {
            panic!()
        };
        assert_eq!(hits.len(), 4);
        for h in &hits {
            assert!((h.point.x.hypot(h.point.y) - 1.0).abs() < 1e-14);
            assert!(!h.tangent);
        }
        assert!(hits.windows(2).all(|w| w[0].t < w[1].t));
    }

    #[test]
    fn an_inconsistent_tolerance_is_an_error() {
        let plane = Surface::Plane {
            frame: Frame::world(),
        };
        let line = Curve::Line {
            origin: Point3::origin(),
            direction: Vec3::z_axis(),
        };
        assert!(matches!(
            intersect_curve_surface(&line, &plane, Tolerance::new(1e-7, 0.0)),
            Err(GeomError::InvalidTolerance(_))
        ));
    }
}
