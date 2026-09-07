//! Pcurves: the (u, v) image of a 3D curve on a plane or a cylinder at
//! the curve's own parameter, exact where a `Curve2` variant exists and a
//! fitted NURBS otherwise (`docs/02-data-model.md` §Pcurves), and the
//! projection of a curve onto a plane for a consumer's sketch.

use core::f64::consts::{FRAC_PI_2, TAU};

use arris_math::{
    Frame, Frame2, Handedness, Interval, Point2, Tolerance, UnitVec2, Vec2, Vec3, is_negligible,
    wrap_angle as wrap_turn,
};

use crate::{Curve, Curve2, GeomError, GeomKind, NurbsCurve, NurbsCurve2, Surface, fit_curve2};

/// How many parameters `pcurve_on` samples over the range to decide that
/// the curve lies on the surface, and the resolution of the table that
/// unwraps a periodic `u` along the curve: a curve that winds around the
/// axis by more than a quarter turn between two samples is refused. A
/// sampling density, not a tolerance; the fitted result is verified at the
/// fit's own, denser, check parameters.
pub const PCURVE_SAMPLES: usize = 256;

/// The degree of a fitted pcurve. The oblique section of a cylinder is a
/// sinusoid in (u, v) whose amplitude is `R tan α` for the angle `α`
/// between the plane's normal and the axis, so a grazing plane has an
/// amplitude thousands of times its radius; a quintic fits that to a
/// micrometre well inside `MAX_FIT_SPANS`, where a cubic would run out of
/// spans. A structural choice, not a tolerance.
pub const PCURVE_FIT_DEGREE: usize = 5;

/// `d` moved into `(−π, π]`.
fn wrap_pi(d: f64) -> f64 {
    d - TAU * (d / TAU).round()
}

/// The (u, v) coordinates of `p` in a plane's frame.
fn in_plane(plane: &Frame, p: arris_math::Point3) -> Point2 {
    let q = plane.to_local(p);
    Point2::new(q.x, q.y)
}

/// The (u, v) components of `v` in a plane's frame.
fn in_plane_vec(plane: &Frame, v: Vec3) -> Vec2 {
    let q = plane.vec_to_local(v);
    Vec2::new(q.x, q.y)
}

fn degenerate(kind: GeomKind, reason: impl Into<String>) -> GeomError {
    GeomError::Degenerate {
        kind,
        reason: reason.into(),
    }
}

/// The pcurve of `curve` over `range` on `surface`: a `Curve2` whose image
/// under the surface is the curve *at the same parameter*
/// (`surface.point(pcurve(t)) == curve.point(t)`), which is invariant E4
/// of `docs/02-data-model.md` before the checker exists.
///
/// The table is exhaustive over (curve, surface). On a plane every
/// variant is exact: a line is a `Line`, a circle a `Circle` and an
/// ellipse an `Ellipse` placed by a `Frame2` whose handedness is the sign
/// of the curve's `Z` against the plane's normal, a NURBS a `Nurbs` with
/// its control points projected. On a cylinder a ruling is a `Line` at
/// constant `u`, a circle around the axis a `Line` at constant `v` whose
/// `u` starts at the offset of the circle's `X` from the cylinder's, and
/// everything else — an oblique section, a NURBS — is a `Nurbs` fitted by
/// [`fit_curve2`] over the cylinder's projection with `u` unwrapped along
/// `t`, so a seam crossing stays continuous and `u` may leave `[0, 2π)`.
/// Cone, sphere, torus and NURBS surfaces are [`GeomError::Unsupported`]
/// arms.
///
/// Errors: [`GeomError::NotOnSurface`] when the curve is farther than
/// `tol.linear` from the surface at any of [`PCURVE_SAMPLES`] parameters
/// over the range; [`GeomError::Fit`] when the fitted arm cannot reach
/// `tol.linear`; [`GeomError::Degenerate`] for an unbounded or empty
/// range, or a curve winding faster than the sampling resolves;
/// [`GeomError::InvalidTolerance`].
///
/// ```
/// use arris_geom::{Curve, Curve2, Surface, pcurve_on};
/// use arris_math::{Frame, Interval, Point3, Precision, Vec3};
///
/// let wall = Surface::Cylinder { frame: Frame::world(), radius: 2.0 };
/// let ring = Curve::Circle { frame: Frame::from_z(Point3::new(0.0, 0.0, 3.0), Vec3::z()).unwrap(), radius: 2.0 };
/// let pc = pcurve_on(&ring, Interval::TURN, &wall, Precision::DEFAULT.tolerance()).unwrap();
/// let Curve2::Line { origin, direction } = pc else { panic!() };
/// assert_eq!(origin.y, 3.0);
/// assert_eq!(direction.x, 1.0); // u runs with t, v stays at 3
/// ```
pub fn pcurve_on(
    curve: &Curve,
    range: Interval,
    surface: &Surface,
    tol: Tolerance,
) -> Result<Curve2, GeomError> {
    if !tol.is_consistent() {
        return Err(GeomError::InvalidTolerance(tol));
    }
    let curve_kind = GeomKind::Curve(curve.kind());
    if !range.is_bounded() || range.length() <= 0.0 {
        return Err(degenerate(
            curve_kind,
            format!(
                "a pcurve needs a bounded range of positive length, not [{}, {}]",
                range.lo(),
                range.hi()
            ),
        ));
    }
    match surface {
        Surface::Plane { frame } => {
            check_on(curve, range, surface, tol, |p| frame.to_local(p).z.abs())?;
            in_plane_curve(curve, frame)
        }
        &Surface::Cylinder { ref frame, radius } => {
            check_on(curve, range, surface, tol, |p| {
                let q = frame.to_local(p);
                (q.x.hypot(q.y) - radius).abs()
            })?;
            on_cylinder(curve, range, frame, radius, surface, tol)
        }
        Surface::Cone { .. }
        | Surface::Sphere { .. }
        | Surface::Torus { .. }
        | Surface::Nurbs(_) => Err(GeomError::Unsupported {
            a: curve_kind,
            b: GeomKind::Surface(surface.kind()),
        }),
    }
}

/// [`GeomError::NotOnSurface`] at the first of [`PCURVE_SAMPLES`]
/// parameters where `distance` of the curve's point exceeds `tol.linear`.
fn check_on(
    curve: &Curve,
    range: Interval,
    surface: &Surface,
    tol: Tolerance,
    distance: impl Fn(arris_math::Point3) -> f64,
) -> Result<(), GeomError> {
    for i in 0..=PCURVE_SAMPLES {
        let t = range.lerp(i as f64 / PCURVE_SAMPLES as f64);
        let d = distance(curve.point(t));
        if d.is_nan() || d > tol.linear {
            return Err(GeomError::NotOnSurface {
                curve: GeomKind::Curve(curve.kind()),
                surface: GeomKind::Surface(surface.kind()),
                t,
                distance: d,
            });
        }
    }
    Ok(())
}

/// The exact pcurve of a curve lying in `plane`, at the same parameter.
fn in_plane_curve(curve: &Curve, plane: &Frame) -> Result<Curve2, GeomError> {
    let kind = GeomKind::Curve(curve.kind());
    match curve {
        &Curve::Line { origin, direction } => {
            let d = in_plane_vec(plane, direction.into_inner());
            let direction = UnitVec2::try_new(d, 0.0)
                .ok_or_else(|| degenerate(kind, "the line is perpendicular to the plane"))?;
            Ok(Curve2::Line {
                origin: in_plane(plane, origin),
                direction,
            })
        }
        &Curve::Circle { ref frame, radius } => Ok(Curve2::Circle {
            frame: conic_frame(plane, frame, kind)?,
            radius,
        }),
        &Curve::Ellipse {
            ref frame,
            major_radius,
            minor_radius,
        } => Ok(Curve2::Ellipse {
            frame: conic_frame(plane, frame, kind)?,
            major_radius,
            minor_radius,
        }),
        Curve::Nurbs(c) => Ok(Curve2::Nurbs(nurbs_in_plane(c, plane)?)),
    }
}

/// The `Frame2` of a conic whose frame lies in `plane`: origin and `X` in
/// plane coordinates, right-handed when the conic's `Z` is along the
/// plane's normal and left-handed when it opposes it.
fn conic_frame(plane: &Frame, frame: &Frame, kind: GeomKind) -> Result<Frame2, GeomError> {
    let handedness = if frame.z().dot(&plane.z()) >= 0.0 {
        Handedness::Right
    } else {
        Handedness::Left
    };
    Frame2::new(
        in_plane(plane, frame.origin()),
        in_plane_vec(plane, frame.x().into_inner()),
        handedness,
    )
    .map_err(|e| degenerate(kind, format!("conic axes in the plane: {e:?}")))
}

/// A NURBS with its control points in plane coordinates: the projection
/// is affine, so knots and weights carry over and the parameter is the
/// same.
fn nurbs_in_plane(c: &NurbsCurve, plane: &Frame) -> Result<NurbsCurve2, GeomError> {
    NurbsCurve2::new(
        c.degree(),
        c.knots().to_vec(),
        c.control_points()
            .iter()
            .map(|p| in_plane(plane, *p))
            .collect(),
        c.weights().to_vec(),
    )
}

/// The angle in `[0, π/2]` between the lines carried by two vectors.
fn line_angle(a: &Vec3, b: &Vec3) -> f64 {
    a.cross(b).norm().atan2(a.dot(b).abs())
}

fn on_cylinder(
    curve: &Curve,
    range: Interval,
    cyl: &Frame,
    radius: f64,
    surface: &Surface,
    tol: Tolerance,
) -> Result<Curve2, GeomError> {
    match curve {
        &Curve::Line { origin, direction } => {
            let d = cyl.vec_to_local(direction.into_inner());
            if d.x.hypot(d.y).atan2(d.z.abs()) <= tol.angular {
                // A ruling: constant `u`, `v` running with `t` along ±Z.
                let q = cyl.to_local(origin);
                return Ok(Curve2::Line {
                    origin: Point2::new(wrap_turn(q.y.atan2(q.x)), q.z),
                    direction: UnitVec2::new_unchecked(Vec2::new(0.0, d.z.signum())),
                });
            }
            fitted_on_cylinder(curve, range, cyl, surface, tol)
        }
        &Curve::Circle {
            ref frame,
            radius: r,
        } => {
            let centre = cyl.to_local(frame.origin());
            let parallel = line_angle(&frame.z(), &cyl.z()) <= tol.angular
                && centre.x.hypot(centre.y) <= tol.linear
                && (r - radius).abs() <= tol.linear;
            if parallel {
                // Constant `v`; `u` starts where the circle's `X` sits
                // against the cylinder's and runs with `t` in the sense of
                // the circle's `Z` against the cylinder's.
                let x = cyl.vec_to_local(frame.x().into_inner());
                let u0 = wrap_turn(x.y.atan2(x.x));
                let sense = frame.z().dot(&cyl.z()).signum();
                return Ok(Curve2::Line {
                    origin: Point2::new(u0, centre.z),
                    direction: UnitVec2::new_unchecked(Vec2::new(sense, 0.0)),
                });
            }
            fitted_on_cylinder(curve, range, cyl, surface, tol)
        }
        Curve::Ellipse { .. } | Curve::Nurbs(_) => {
            fitted_on_cylinder(curve, range, cyl, surface, tol)
        }
    }
}

/// A NURBS pcurve fitted over the cylinder's projection of the curve, `u`
/// unwrapped along `t` through a table of [`PCURVE_SAMPLES`] parameters.
fn fitted_on_cylinder(
    curve: &Curve,
    range: Interval,
    cyl: &Frame,
    surface: &Surface,
    tol: Tolerance,
) -> Result<Curve2, GeomError> {
    let kind = GeomKind::Curve(curve.kind());
    let raw = |t: f64| {
        let q = cyl.to_local(curve.point(t));
        (q.y.atan2(q.x), q.z)
    };
    let n = PCURVE_SAMPLES;
    let mut table = Vec::with_capacity(n + 1);
    let mut last = wrap_turn(raw(range.lo()).0);
    table.push(last);
    for i in 1..=n {
        let t = range.lerp(i as f64 / n as f64);
        let step = wrap_pi(raw(t).0 - last);
        if step.abs() >= FRAC_PI_2 {
            return Err(degenerate(
                kind,
                format!(
                    "winds {step} radians around the axis between two of {n} samples: faster than the pcurve sampling resolves"
                ),
            ));
        }
        last += step;
        table.push(last);
    }
    let f = |t: f64| {
        let s = ((t - range.lo()) / range.length() * n as f64).round();
        let i = (s.max(0.0) as usize).min(n);
        let (u_raw, v) = raw(t);
        Point2::new(table[i] + wrap_pi(u_raw - table[i]), v)
    };
    let deviation = |t: f64, q: Point2| (surface.point(q.x, q.y) - curve.point(t)).norm();
    let fit = fit_curve2(f, range, PCURVE_FIT_DEGREE, deviation, tol.linear)?;
    Ok(Curve2::Nurbs(fit))
}

/// The orthogonal projection of `curve` onto `plane` as a `Curve2` in the
/// plane's (u, v), for a consumer's sketch (`docs/01-architecture.md`
/// §Facade): a line stays a `Line`, a circle becomes a `Circle` when its
/// plane is parallel and an `Ellipse` otherwise, an ellipse an `Ellipse`,
/// a NURBS a `Nurbs` with its control points projected. This is a
/// point-set projection: the parameter is the variant's own — a line's
/// arc length in the plane, a conic's angle about its projected axes,
/// which differs from the 3D parameter by a phase when the conic is
/// oblique — and only a NURBS, or a curve lying in the plane, keeps the
/// 3D parameter. For the same-parameter pcurve of a curve on the plane
/// use [`pcurve_on`].
///
/// Errors: [`GeomError::Degenerate`] when the projection collapses — a
/// line perpendicular to the plane, a conic in a plane perpendicular to it.
///
/// ```
/// use arris_geom::{Curve, Curve2, project_to_plane};
/// use arris_math::{Frame, Point3, Vec3};
///
/// // A unit circle tilted by 60° about x projects to an ellipse 1 × 0.5.
/// let tilted = Frame::new(Point3::origin(), Vec3::new(0.0, -(3f64.sqrt()) / 2.0, 0.5), Vec3::x()).unwrap();
/// let circle = Curve::Circle { frame: tilted, radius: 1.0 };
/// let Curve2::Ellipse { major_radius, minor_radius, .. } = project_to_plane(&circle, &Frame::world()).unwrap() else { panic!() };
/// assert!((major_radius - 1.0).abs() < 1e-15 && (minor_radius - 0.5).abs() < 1e-15);
/// ```
pub fn project_to_plane(curve: &Curve, plane: &Frame) -> Result<Curve2, GeomError> {
    let kind = GeomKind::Curve(curve.kind());
    match curve {
        &Curve::Line { origin, direction } => {
            let d = in_plane_vec(plane, direction.into_inner());
            if is_negligible(d.norm(), 1.0) {
                return Err(degenerate(
                    kind,
                    "the line is perpendicular to the plane: its projection is a point",
                ));
            }
            Ok(Curve2::Line {
                origin: in_plane(plane, origin),
                direction: UnitVec2::new_normalize(d),
            })
        }
        &Curve::Circle { ref frame, radius } => {
            if is_negligible(frame.z().cross(&plane.z()).norm(), 1.0) {
                return in_plane_curve(curve, plane);
            }
            projected_conic(plane, frame, radius, radius, kind)
        }
        &Curve::Ellipse {
            ref frame,
            major_radius,
            minor_radius,
        } => {
            if is_negligible(frame.z().cross(&plane.z()).norm(), 1.0) {
                return in_plane_curve(curve, plane);
            }
            projected_conic(plane, frame, major_radius, minor_radius, kind)
        }
        Curve::Nurbs(c) => Ok(Curve2::Nurbs(nurbs_in_plane(c, plane)?)),
    }
}

/// The ellipse `o + a cos t·X + b sin t·Y` projected onto `plane`: the
/// image is `o' + M (cos t, sin t)ᵀ` with `M = [a X' | b Y']` the projected
/// axes, and the singular value decomposition of the 2 × 2 `M` gives the
/// image's semi-axes (the singular values) and their directions (the left
/// singular vectors); a negative second singular value is a reflection,
/// which the frame's handedness records.
fn projected_conic(
    plane: &Frame,
    frame: &Frame,
    a: f64,
    b: f64,
    kind: GeomKind,
) -> Result<Curve2, GeomError> {
    let col1 = a * in_plane_vec(plane, frame.x().into_inner());
    let col2 = b * in_plane_vec(plane, frame.y().into_inner());
    // M = [[p, q], [r, s]] by rows.
    let (p, q, r, s) = (col1.x, col2.x, col1.y, col2.y);
    let (e, f, g, h) = (0.5 * (p + s), 0.5 * (p - s), 0.5 * (r + q), 0.5 * (r - q));
    let (big, small) = (e.hypot(h), f.hypot(g));
    let major = big + small;
    let minor_signed = big - small;
    if is_negligible(minor_signed, major) {
        return Err(degenerate(
            kind,
            "the conic's plane is perpendicular to the target: its projection is a segment",
        ));
    }
    let phi = 0.5 * (h.atan2(e) + g.atan2(f));
    let handedness = if minor_signed > 0.0 {
        Handedness::Right
    } else {
        Handedness::Left
    };
    let frame2 = Frame2::new(
        in_plane(plane, frame.origin()),
        Vec2::new(phi.cos(), phi.sin()),
        handedness,
    )
    .map_err(|e| degenerate(kind, format!("projected axes: {e:?}")))?;
    Ok(Curve2::Ellipse {
        frame: frame2,
        major_radius: major,
        minor_radius: minor_signed.abs(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use arris_math::{Point3, Precision};

    fn tol() -> Tolerance {
        Precision::DEFAULT.tolerance()
    }

    #[test]
    fn a_lifted_curve_is_not_on_the_surface_and_names_where() {
        let plane = Surface::Plane {
            frame: Frame::world(),
        };
        let lifted = Curve::Line {
            origin: Point3::new(0.0, 0.0, 1e-3),
            direction: Vec3::x_axis(),
        };
        let range = Interval::new(0.0, 1.0).unwrap();
        let err = pcurve_on(&lifted, range, &plane, tol()).unwrap_err();
        assert!(
            matches!(err, GeomError::NotOnSurface { distance, .. } if (distance - 1e-3).abs() < 1e-15),
            "{err}"
        );
        assert!(matches!(
            pcurve_on(&lifted, Interval::REAL, &plane, tol()),
            Err(GeomError::Degenerate { .. })
        ));
        let sphere = Surface::Sphere {
            frame: Frame::world(),
            radius: 1.0,
        };
        assert!(matches!(
            pcurve_on(&lifted, range, &sphere, tol()),
            Err(GeomError::Unsupported { .. })
        ));
        assert!(matches!(
            pcurve_on(&lifted, range, &plane, Tolerance::new(0.0, 1.0)),
            Err(GeomError::InvalidTolerance(_))
        ));
    }

    #[test]
    fn a_ruling_and_a_parallel_are_lines_in_uv() {
        let wall = Surface::Cylinder {
            frame: Frame::world(),
            radius: 2.0,
        };
        let ruling = Curve::Line {
            origin: Point3::new(0.0, 2.0, 5.0),
            direction: -Vec3::z_axis(),
        };
        let pc = pcurve_on(&ruling, Interval::new(-1.0, 1.0).unwrap(), &wall, tol()).unwrap();
        let Curve2::Line { origin, direction } = pc else {
            panic!("{pc:?}")
        };
        assert!((origin.x - FRAC_PI_2).abs() < 1e-15 && origin.y == 5.0);
        assert_eq!(direction.into_inner(), Vec2::new(0.0, -1.0));
        // A parallel traversed against the cylinder's Z runs u backwards.
        let ring = Curve::Circle {
            frame: Frame::new(Point3::new(0.0, 0.0, 1.0), -Vec3::z(), Vec3::y()).unwrap(),
            radius: 2.0,
        };
        let pc = pcurve_on(&ring, Interval::TURN, &wall, tol()).unwrap();
        let Curve2::Line { origin, direction } = pc else {
            panic!("{pc:?}")
        };
        assert!((origin.x - FRAC_PI_2).abs() < 1e-15 && origin.y == 1.0);
        assert_eq!(direction.into_inner(), Vec2::new(-1.0, 0.0));
    }

    #[test]
    fn projection_degenerates_are_named() {
        let plane = Frame::world();
        let vertical = Curve::Line {
            origin: Point3::origin(),
            direction: Vec3::z_axis(),
        };
        assert!(matches!(
            project_to_plane(&vertical, &plane),
            Err(GeomError::Degenerate { .. })
        ));
        let edge_on = Curve::Circle {
            frame: Frame::from_z(Point3::origin(), Vec3::x()).unwrap(),
            radius: 1.0,
        };
        assert!(matches!(
            project_to_plane(&edge_on, &plane),
            Err(GeomError::Degenerate { .. })
        ));
        let flat = Curve::Circle {
            frame: Frame::from_z(Point3::new(1.0, 2.0, 3.0), -Vec3::z()).unwrap(),
            radius: 1.5,
        };
        let Curve2::Circle { frame, radius } = project_to_plane(&flat, &plane).unwrap() else {
            panic!()
        };
        assert_eq!(radius, 1.5);
        assert!(!frame.is_right_handed());
        assert_eq!(frame.origin(), Point2::new(1.0, 2.0));
    }
}
