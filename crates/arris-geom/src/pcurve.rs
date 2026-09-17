//! Pcurves: the (u, v) image of a 3D curve on a surface at the curve's own
//! parameter, exact where a `Curve2` variant exists and a fitted NURBS
//! otherwise (`docs/DATA-MODEL.md` §Pcurves), and the projection of a
//! curve onto a plane for a consumer's sketch.

use core::f64::consts::{FRAC_PI_2, TAU};

use arris_math::{
    Frame, Frame2, Handedness, Interval, Point2, Tolerance, UnitVec2, Vec2, Vec3, is_negligible,
    wrap_angle as wrap_turn,
};

use crate::project::{ellipse_distance, ellipse_nearest};
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
/// of `docs/DATA-MODEL.md` before the checker exists.
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
///
/// On the surfaces of revolution the exact arms are the six a revolve
/// makes, each a `Line` in (u, v) at the curve's own parameter: on a
/// **cone**, a ruling — the line through the apex — at constant `u`, and a
/// circle about the axis at constant `v`; on a **sphere**, a circle about
/// the axis at constant `v` (a parallel) and the great circle through both
/// poles at constant `u` (a meridian); on a **torus**, a circle about the
/// axis at constant `v` and a circle of the tube at constant `u`. A `u`
/// origin is the offset of the circle's `X` from the surface's, in
/// `[0, 2π)`, running in the sense of the circle's `Z` against the
/// surface's, as on the cylinder; a constant-`u` arm's `v` runs with `t`
/// or against it by the turn of the circle's own axes in the plane of the
/// axis, and a meridian's `v` leaves `[−π/2, π/2]` where the great circle
/// passes a pole onto the opposite meridian, which is where the sphere's
/// parametrisation puts it. Every other pair on these three — an oblique
/// section of a cone, a small circle of a sphere about no axis of it, a
/// Villarceau circle, a NURBS — is [`GeomError::Unsupported`] naming the
/// pair: there is no fitted fallback here, since the sweeps' curves are
/// all exact. NURBS surfaces are an `Unsupported` arm.
///
/// On an **elliptic cylinder** the exact arms are the two an extrude
/// makes (ADR-0014), each a `Line`: a ruling — a line along the axis —
/// at constant `u`, the parameter of its section point, `v` running with
/// `t` or against it by the sign of the line's direction against `Z`;
/// and the section ellipse — centred on the axis, its `Z` along the axis
/// and its major axis along the surface's `X` either way, the radii
/// agreeing within `tol.linear` — at constant `v`, `u` starting at `0` or
/// `π` by the sign of its `X` against the surface's and running in the
/// sense of its `Z` against the surface's, as a parallel does on a
/// cylinder. Every other curve on it — a circle, an ellipse off the axis
/// or of other radii, a NURBS — is `Unsupported`, with no fitted
/// fallback.
///
/// Errors: [`GeomError::NotOnSurface`] when the curve is farther than
/// `tol.linear` from the surface at any of [`PCURVE_SAMPLES`] + 1 parameters
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
        &Surface::EllipticCylinder {
            ref frame,
            major_radius,
            minor_radius,
        } => {
            check_on(curve, range, surface, tol, |p| {
                let q = frame.to_local(p);
                let noise = p.coords.norm() + frame.origin().coords.norm();
                ellipse_distance(major_radius, minor_radius, q.x, q.y, noise)
            })?;
            on_elliptic_cylinder(curve, frame, [major_radius, minor_radius], surface, tol)
        }
        &Surface::Cone {
            ref frame,
            radius,
            half_angle,
        } => {
            check_on(curve, range, surface, tol, |p| {
                cone_distance(frame, radius, half_angle, p)
            })?;
            on_cone(curve, frame, radius, half_angle, surface, tol)
        }
        &Surface::Sphere { ref frame, radius } => {
            check_on(curve, range, surface, tol, |p| {
                ((p - frame.origin()).norm() - radius).abs()
            })?;
            on_sphere(curve, range, frame, radius, surface, tol)
        }
        &Surface::Torus {
            ref frame,
            major_radius,
            minor_radius,
        } => {
            check_on(curve, range, surface, tol, |p| {
                let q = frame.to_local(p);
                ((q.x.hypot(q.y) - major_radius).hypot(q.z) - minor_radius).abs()
            })?;
            on_torus(curve, frame, major_radius, minor_radius, surface, tol)
        }
        Surface::Nurbs(_) => Err(GeomError::Unsupported {
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

/// The exact pcurves on an elliptic cylinder: a ruling at constant `u`,
/// the section ellipse at constant `v` (the table in [`pcurve_on`]).
fn on_elliptic_cylinder(
    curve: &Curve,
    cyl: &Frame,
    [a, b]: [f64; 2],
    surface: &Surface,
    tol: Tolerance,
) -> Result<Curve2, GeomError> {
    let kind = GeomKind::Curve(curve.kind());
    match curve {
        &Curve::Line { origin, direction } => {
            let d = cyl.vec_to_local(direction.into_inner());
            if d.x.hypot(d.y).atan2(d.z.abs()) > tol.angular {
                return Err(unsupported(curve, surface));
            }
            // A ruling: constant `u` at its section point, which is on
            // the ellipse (checked above) and so has a unique parameter
            // unless the ellipse is degenerate to the tolerance.
            let q = cyl.to_local(origin);
            let noise = origin.coords.norm() + cyl.origin().coords.norm();
            let (u, _) = ellipse_nearest(a, b, q.x, q.y, noise).map_err(|locus| {
                degenerate(
                    kind,
                    format!("the ruling's section point is on {locus} of the ellipse"),
                )
            })?;
            Ok(Curve2::Line {
                origin: Point2::new(u, q.z),
                direction: UnitVec2::new_unchecked(Vec2::new(0.0, d.z.signum())),
            })
        }
        &Curve::Ellipse {
            ref frame,
            major_radius,
            minor_radius,
        } => {
            let centre = cyl.to_local(frame.origin());
            let section = centre.x.hypot(centre.y) <= tol.linear
                && parallel_axes(&frame.z(), &cyl.z(), tol)
                && parallel_axes(&frame.x(), &cyl.x(), tol)
                && (major_radius - a).abs() <= tol.linear
                && (minor_radius - b).abs() <= tol.linear;
            if !section {
                return Err(unsupported(curve, surface));
            }
            Ok(parallel_pcurve(cyl, frame, centre.z, false))
        }
        Curve::Circle { .. } | Curve::Nurbs(_) => Err(unsupported(curve, surface)),
    }
}

/// The distance from `p` to a cone, whose meridian in `(ρ, z)` is the line
/// through `(R, 0)` at the half-angle from the axis — and its mirror, the
/// second nappe, which `ρ ≥ 0` folds onto the same half-plane.
fn cone_distance(cone: &Frame, radius: f64, half_angle: f64, p: arris_math::Point3) -> f64 {
    let q = cone.to_local(p);
    let (rho, z) = (q.x.hypot(q.y), q.z);
    let (sin, cos) = (half_angle.sin(), half_angle.cos());
    let near = ((rho - radius) * cos - z * sin).abs();
    let far = ((rho + radius) * cos + z * sin).abs();
    near.min(far)
}

/// [`GeomError::Unsupported`] naming the pair.
fn unsupported(curve: &Curve, surface: &Surface) -> GeomError {
    GeomError::Unsupported {
        a: GeomKind::Curve(curve.kind()),
        b: GeomKind::Surface(surface.kind()),
    }
}

/// `true` when two axes are parallel — either way round — within `tol`.
fn parallel_axes(a: &Vec3, b: &Vec3, tol: Tolerance) -> bool {
    line_angle(a, b) <= tol.angular
}

/// `true` when two axes are perpendicular within `tol`.
fn perpendicular_axes(a: &Vec3, b: &Vec3, tol: Tolerance) -> bool {
    (FRAC_PI_2 - line_angle(a, b)).abs() <= tol.angular
}

/// The pcurve of a circle about a surface of revolution's axis: a `Line`
/// at constant `v`, `u` starting at the offset of the circle's `X` from
/// the surface's and running in the sense of the circle's `Z` against the
/// surface's, exactly as on a cylinder. `flipped` is for a cone's circle
/// beyond the apex, whose radial factor `R + v sin α` is negative: the
/// surface reaches it at `u + π`.
fn parallel_pcurve(surface: &Frame, circle: &Frame, v: f64, flipped: bool) -> Curve2 {
    let x = surface.vec_to_local(circle.x().into_inner());
    let half_turn = if flipped { TAU / 2.0 } else { 0.0 };
    let u0 = wrap_turn(x.y.atan2(x.x) + half_turn);
    let sense = if circle.z().dot(&surface.z()) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    Curve2::Line {
        origin: Point2::new(u0, v),
        direction: UnitVec2::new_unchecked(Vec2::new(sense, 0.0)),
    }
}

/// The pcurve of a circle lying in a plane through a surface of
/// revolution's axis — a sphere's meridian, a torus's tube circle — at
/// constant `u`: a `Line` whose `v` runs with `t` or against it. In the
/// (radial, axis) plane the surface's own `v` measures the angle from
/// `radial`, and the circle's `(X, Y)` is that basis turned by `φ` when
/// the two agree in orientation and reflected about `φ / 2` when they do
/// not, so `v` is `φ + t` one way and `φ − t` the other.
///
/// `radial` is the unit direction, in the surface's local frame and in the
/// equatorial plane, that `u` points along; `period` wraps `φ` into
/// `[0, 2π)` on a surface whose `v` is periodic and leaves it alone on a
/// sphere, whose `v` is an angle in `[−π/2, π/2]`.
fn meridian_pcurve(surface: &Frame, circle: &Frame, radial: Vec3, periodic_v: bool) -> Curve2 {
    let u = wrap_turn(radial.y.atan2(radial.x));
    let x = surface.vec_to_local(circle.x().into_inner());
    let y = surface.vec_to_local(circle.y().into_inner());
    let (a, b) = (x.dot(&radial), x.z);
    let (c, d) = (y.dot(&radial), y.z);
    let phi = b.atan2(a);
    let phi = if periodic_v { wrap_turn(phi) } else { phi };
    let sense = if a * d - c * b >= 0.0 { 1.0 } else { -1.0 };
    Curve2::Line {
        origin: Point2::new(u, phi),
        direction: UnitVec2::new_unchecked(Vec2::new(0.0, sense)),
    }
}

/// The unit equatorial direction of the half-plane a meridian arc lies
/// in: `axis` is the candidate, `±` the one the curve's own points pick.
/// The midpoint is asked first and the start second, since a curve may
/// begin on the axis (a sphere's pole) where the half-plane is not
/// decided; neither deciding leaves the candidate as written, which names
/// the same meridian circle at `u + π`.
fn meridian_radial(
    surface: &Frame,
    curve: &Curve,
    range: Interval,
    candidate: Vec3,
    scale: f64,
) -> Vec3 {
    for t in [range.midpoint(), range.lo()] {
        let q = surface.to_local(curve.point(t));
        let radial = Vec2::new(q.x, q.y);
        if !is_negligible(radial.norm(), scale) {
            let along = radial.x * candidate.x + radial.y * candidate.y;
            return if along >= 0.0 { candidate } else { -candidate };
        }
    }
    candidate
}

/// The exact pcurves on a cone: a ruling at constant `u`, a circle about
/// the axis at constant `v`.
fn on_cone(
    curve: &Curve,
    cone: &Frame,
    radius: f64,
    half_angle: f64,
    surface: &Surface,
    tol: Tolerance,
) -> Result<Curve2, GeomError> {
    let kind = GeomKind::Curve(curve.kind());
    let (sin, cos) = (half_angle.sin(), half_angle.cos());
    if is_negligible(sin, 1.0) || cos <= 0.0 {
        return Err(degenerate(
            GeomKind::Surface(surface.kind()),
            format!("a cone's half-angle is in (0, π/2), not {half_angle}"),
        ));
    }
    match curve {
        &Curve::Line { origin, direction } => {
            // A line on a cone is a ruling through the apex: `v` runs
            // along it, and the sense is the sign of its axial part, since
            // the ruling climbs by `cos α > 0` per unit of `v`.
            let d = cone.vec_to_local(direction.into_inner());
            let sense = if d.z >= 0.0 { 1.0 } else { -1.0 };
            let equatorial = Vec2::new(sense * d.x, sense * d.y);
            if (equatorial.norm().atan2(d.z.abs()) - half_angle).abs() > tol.angular {
                return Err(unsupported(curve, surface));
            }
            if is_negligible(equatorial.norm(), 1.0) {
                return Err(degenerate(kind, "the ruling has no radial direction"));
            }
            let u = wrap_turn(equatorial.y.atan2(equatorial.x));
            let v0 = cone.to_local(origin).z / cos;
            Ok(Curve2::Line {
                origin: Point2::new(u, v0),
                direction: UnitVec2::new_unchecked(Vec2::new(0.0, sense)),
            })
        }
        Curve::Circle { frame, .. } => {
            let centre = cone.to_local(frame.origin());
            let about_axis =
                centre.x.hypot(centre.y) <= tol.linear && parallel_axes(&frame.z(), &cone.z(), tol);
            if !about_axis {
                return Err(unsupported(curve, surface));
            }
            let v = centre.z / cos;
            Ok(parallel_pcurve(cone, frame, v, radius + v * sin < 0.0))
        }
        Curve::Ellipse { .. } | Curve::Nurbs(_) => Err(unsupported(curve, surface)),
    }
}

/// The exact pcurves on a sphere: a parallel at constant `v`, a meridian
/// at constant `u`.
fn on_sphere(
    curve: &Curve,
    range: Interval,
    sphere: &Frame,
    radius: f64,
    surface: &Surface,
    tol: Tolerance,
) -> Result<Curve2, GeomError> {
    match curve {
        &Curve::Circle {
            ref frame,
            radius: rho,
        } => {
            let centre = sphere.to_local(frame.origin());
            if centre.x.hypot(centre.y) <= tol.linear && parallel_axes(&frame.z(), &sphere.z(), tol)
            {
                // A parallel: `v` is the latitude of its plane.
                return Ok(parallel_pcurve(sphere, frame, centre.z.atan2(rho), false));
            }
            // A meridian: the great circle whose plane holds the axis.
            let through_axis = centre.coords.norm() <= tol.linear
                && (rho - radius).abs() <= tol.linear
                && perpendicular_axes(&frame.z(), &sphere.z(), tol);
            if !through_axis {
                return Err(unsupported(curve, surface));
            }
            let z = sphere.vec_to_local(frame.z().into_inner());
            let candidate = Vec3::new(-z.y, z.x, 0.0);
            let Some(candidate) = candidate.try_normalize(0.0) else {
                return Err(unsupported(curve, surface));
            };
            let radial = meridian_radial(sphere, curve, range, candidate, radius);
            Ok(meridian_pcurve(sphere, frame, radial, false))
        }
        Curve::Line { .. } | Curve::Ellipse { .. } | Curve::Nurbs(_) => {
            Err(unsupported(curve, surface))
        }
    }
}

/// The exact pcurves on a torus: a circle about the axis at constant `v`,
/// a circle of the tube at constant `u`.
fn on_torus(
    curve: &Curve,
    torus: &Frame,
    major_radius: f64,
    minor_radius: f64,
    surface: &Surface,
    tol: Tolerance,
) -> Result<Curve2, GeomError> {
    match curve {
        &Curve::Circle {
            ref frame,
            radius: rho,
        } => {
            let centre = torus.to_local(frame.origin());
            let equatorial = Vec2::new(centre.x, centre.y);
            if equatorial.norm() <= tol.linear && parallel_axes(&frame.z(), &torus.z(), tol) {
                // About the axis: `v` is where the tube's own angle puts
                // this radius and height.
                let v = wrap_turn(centre.z.atan2(rho - major_radius));
                return Ok(parallel_pcurve(torus, frame, v, false));
            }
            let Some(radial) = Vec3::new(centre.x, centre.y, 0.0).try_normalize(0.0) else {
                return Err(unsupported(curve, surface));
            };
            let z = torus.vec_to_local(frame.z().into_inner());
            let of_the_tube = (equatorial.norm() - major_radius).abs() <= tol.linear
                && centre.z.abs() <= tol.linear
                && (rho - minor_radius).abs() <= tol.linear
                && perpendicular_axes(&z, &Vec3::z(), tol)
                && perpendicular_axes(&z, &radial, tol);
            if !of_the_tube {
                return Err(unsupported(curve, surface));
            }
            Ok(meridian_pcurve(torus, frame, radial, true))
        }
        Curve::Line { .. } | Curve::Ellipse { .. } | Curve::Nurbs(_) => {
            Err(unsupported(curve, surface))
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
/// plane's (u, v), for a consumer's sketch (`docs/ARCHITECTURE.md`
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
    let (major, minor_signed, phi) = principal_axes(col1, col2);
    if is_negligible(minor_signed, major) {
        return Err(degenerate(
            kind,
            "the conic's plane is perpendicular to the target: its projection is a segment",
        ));
    }
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

/// The principal axes of the ellipse `M (cos t, sin t)ᵀ`, `M = [col1 |
/// col2]` two conjugate semi-diameters in a plane's (u, v): `(major,
/// minor, φ)` with the semi-axes the singular values of `M`, `φ` the
/// angle of the major axis from `u`, and `minor` signed — negative when
/// `M` is a reflection, so the ellipse is traversed clockwise. A
/// projected conic's, and an oblique plane section's of an elliptic
/// cylinder.
pub(crate) fn principal_axes(col1: Vec2, col2: Vec2) -> (f64, f64, f64) {
    // M = [[p, q], [r, s]] by rows.
    let (p, q, r, s) = (col1.x, col2.x, col1.y, col2.y);
    let (e, f, g, h) = (0.5 * (p + s), 0.5 * (p - s), 0.5 * (r + q), 0.5 * (r - q));
    let (big, small) = (e.hypot(h), f.hypot(g));
    let phi = 0.5 * (h.atan2(e) + g.atan2(f));
    (big + small, big - small, phi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arris_math::{Point3, Precision};

    fn tol() -> Tolerance {
        Precision::DEFAULT.tolerance()
    }

    #[test]
    fn an_elliptic_cylinder_carries_its_section_and_its_rulings_as_lines() {
        let wall = Surface::EllipticCylinder {
            frame: Frame::world(),
            major_radius: 3.0,
            minor_radius: 2.0,
        };
        // The section at height 4, traversed against the axis: `u` runs
        // backwards from 0.
        let section = Curve::Ellipse {
            frame: Frame::new(Point3::new(0.0, 0.0, 4.0), -Vec3::z(), Vec3::x()).unwrap(),
            major_radius: 3.0,
            minor_radius: 2.0,
        };
        let pc = pcurve_on(&section, Interval::TURN, &wall, tol()).unwrap();
        let Curve2::Line { origin, direction } = pc else {
            panic!("{pc:?}")
        };
        assert_eq!(origin, Point2::new(0.0, 4.0));
        assert_eq!(direction.into_inner(), Vec2::new(-1.0, 0.0));
        // The ruling through the minor vertex, running down.
        let ruling = Curve::Line {
            origin: Point3::new(0.0, 2.0, 9.0),
            direction: -Vec3::z_axis(),
        };
        let pc = pcurve_on(&ruling, Interval::new(0.0, 5.0).unwrap(), &wall, tol()).unwrap();
        let Curve2::Line { origin, direction } = pc else {
            panic!("{pc:?}")
        };
        assert!((origin.x - FRAC_PI_2).abs() < 1e-12 && origin.y == 9.0);
        assert_eq!(direction.into_inner(), Vec2::new(0.0, -1.0));
        // A circle of the minor radius touches the surface at two points
        // and is off it elsewhere; a section of other radii is off it.
        let circle = Curve::Circle {
            frame: Frame::world(),
            radius: 2.0,
        };
        assert!(matches!(
            pcurve_on(&circle, Interval::TURN, &wall, tol()),
            Err(GeomError::NotOnSurface { .. })
        ));
        // An ellipse *on* the surface that is not a section — none lies
        // on it but the sections — so the unsupported arm is reached only
        // through a ruling off the axis' direction: a chord.
        let chord = Curve::Line {
            origin: Point3::new(3.0, 0.0, 0.0),
            direction: Vec3::y_axis(),
        };
        assert!(matches!(
            pcurve_on(&chord, Interval::new(0.0, 1e-9).unwrap(), &wall, tol()),
            Err(GeomError::Unsupported { .. })
        ));
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
        // A line is nowhere near a sphere: that is not an unsupported
        // pair, it is a curve off the surface.
        assert!(matches!(
            pcurve_on(&lifted, range, &sphere, tol()),
            Err(GeomError::NotOnSurface { .. })
        ));
        // A small circle *on* the sphere about no axis of it has no
        // `Curve2` variant, and is the unsupported pair.
        let small = Curve::Circle {
            frame: Frame::from_z(Point3::new(0.5, 0.0, 0.0), Vec3::x()).unwrap(),
            radius: 0.75f64.sqrt(),
        };
        assert!(matches!(
            pcurve_on(&small, Interval::TURN, &sphere, tol()),
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
