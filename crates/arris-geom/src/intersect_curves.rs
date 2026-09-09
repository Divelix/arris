//! Curve–curve intersection: the closed-form table of cycle 1.
//!
//! Two curves meet in points, and every pair a boolean needs is decided
//! through a plane one of them already lies in: a conic's own plane
//! (ADR-0004). The one pair with no plane to use — two lines — has a
//! closed form of its own, and the coplanar cases, where the plane says
//! nothing, have theirs.

use arris_math::{Frame, Point3, Tolerance, Vec2, Vec3, wrap_angle};

use crate::{
    Curve, CurveKind, CurveSurfaceIntersection, GeomError, GeomKind, Surface,
    intersect_curve_surface,
};

/// One point two curves have in common.
///
/// `point` is `a.point(ta)`, and `b.point(tb)` is within `tol.linear` of
/// it. A periodic parameter is in `[0, 2π)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurveCurveHit {
    /// The first curve's parameter.
    pub ta: f64,
    /// The second curve's parameter.
    pub tb: f64,
    /// `a.point(ta)`.
    pub point: Point3,
    /// `true` when the curves touch here without crossing: the first
    /// curve stays on one side of the second within `tol.linear`. Two
    /// crossings that close are one touch.
    pub tangent: bool,
}

/// What two curves have in common.
#[derive(Debug, Clone, PartialEq)]
pub enum CurveIntersection {
    /// The hits, ascending by `ta`; empty when the curves miss each
    /// other.
    Points(Vec<CurveCurveHit>),
    /// The curves are the same curve within the tolerance — the same
    /// line, or the same circle — and there is no point to return.
    Coincident,
}

/// The intersection of two curves, by the case table of cycle 1: every
/// pair with a closed form is computed exactly, every other pair is an
/// explicit [`GeomError::Unsupported`] arm — no wildcard, no marcher.
///
/// Guarantees: hits are sorted by `ta`, each parameter is in its curve's
/// domain, each hit lies on both curves within `tol.linear`, and the
/// result is deterministic bit for bit.
///
/// The table. Two lines are the closed form: parallel within
/// `tol.angular` gives `Coincident` or nothing by the distance between
/// them, and otherwise the nearest approach is a hit when it is shorter
/// than `tol.linear`. Every other supported pair has a conic operand, and
/// goes through *that conic's plane*: the other curve meets the plane at
/// points ([`intersect_curve_surface`]), and a point is a hit when the
/// conic's own projection of it is within `tol.linear`, which gives the
/// conic's parameter with it. A curve the plane reports `Coincident` with
/// is the coplanar case, where the plane decides nothing and each pair
/// has its own form: a line against a circle or an ellipse is the conic
/// against the plane through the line perpendicular to the conic's — the
/// same points, and the tangency decided in the linear tolerance by an
/// arm that already exists — and two coplanar circles are the radical
/// line. A coplanar pair with an ellipse in it is `Coincident` when the
/// two are the same conic — the centres, the radii and the major axes
/// agreeing within the tolerance, which is what two booleans in a row
/// make — and otherwise `Unsupported`: the quartic that finds where two
/// distinct such conics meet is cycle 3's. Any NURBS operand is
/// `Unsupported`.
///
/// ```
/// use arris_geom::{Curve, CurveIntersection, intersect_curves};
/// use arris_math::{Frame, Point3, Precision, Vec3};
///
/// let circle = Curve::Circle { frame: Frame::world(), radius: 2.0 };
/// let axis = Curve::Line { origin: Point3::new(0.0, 0.0, -1.0), direction: Vec3::z_axis() };
/// // The axis pierces the circle's plane at its centre, which is not on it.
/// let CurveIntersection::Points(hits) =
///     intersect_curves(&axis, &circle, Precision::DEFAULT.tolerance())?
/// else { panic!() };
/// assert!(hits.is_empty());
/// assert_eq!(
///     intersect_curves(&circle, &circle, Precision::DEFAULT.tolerance())?,
///     CurveIntersection::Coincident
/// );
/// # Ok::<(), arris_geom::GeomError>(())
/// ```
pub fn intersect_curves(
    a: &Curve,
    b: &Curve,
    tol: Tolerance,
) -> Result<CurveIntersection, GeomError> {
    if !tol.is_consistent() {
        return Err(GeomError::InvalidTolerance(tol));
    }
    match (a, b) {
        (
            &Curve::Line {
                origin: oa,
                direction: da,
            },
            &Curve::Line {
                origin: ob,
                direction: db,
            },
        ) => Ok(line_line(oa, da.into_inner(), ob, db.into_inner(), tol)),
        (
            Curve::Line { .. } | Curve::Circle { .. } | Curve::Ellipse { .. },
            Curve::Circle { .. } | Curve::Ellipse { .. },
        ) => through_plane(a, b, tol),
        (Curve::Circle { .. } | Curve::Ellipse { .. }, Curve::Line { .. }) => {
            Ok(swapped(through_plane(b, a, tol)?))
        }
        (
            Curve::Nurbs(_),
            Curve::Line { .. } | Curve::Circle { .. } | Curve::Ellipse { .. } | Curve::Nurbs(_),
        )
        | (Curve::Line { .. } | Curve::Circle { .. } | Curve::Ellipse { .. }, Curve::Nurbs(_)) => {
            Err(GeomError::Unsupported {
                a: GeomKind::Curve(a.kind()),
                b: GeomKind::Curve(b.kind()),
            })
        }
    }
}

/// `hits` sorted by `ta`; a total order, since every parameter is finite.
fn points(mut hits: Vec<CurveCurveHit>) -> CurveIntersection {
    hits.sort_by(|x, y| x.ta.total_cmp(&y.ta));
    CurveIntersection::Points(hits)
}

/// The same answer with the operands the other way round.
fn swapped(found: CurveIntersection) -> CurveIntersection {
    match found {
        CurveIntersection::Coincident => CurveIntersection::Coincident,
        CurveIntersection::Points(hits) => points(
            hits.into_iter()
                .map(|h| CurveCurveHit {
                    ta: h.tb,
                    tb: h.ta,
                    // `point` is the first curve's; the two agree within
                    // `tol.linear` and the swap keeps the one that was
                    // computed, not a re-evaluation.
                    point: h.point,
                    tangent: h.tangent,
                })
                .collect(),
        ),
    }
}

/// The frame of a conic: its centre and its plane.
fn conic_frame(c: &Curve) -> Option<(&Frame, [f64; 2])> {
    match c {
        Curve::Circle { frame, radius } => Some((frame, [*radius, *radius])),
        Curve::Ellipse {
            frame,
            major_radius,
            minor_radius,
        } => Some((frame, [*major_radius, *minor_radius])),
        Curve::Line { .. } | Curve::Nurbs(_) => None,
    }
}

fn line_line(oa: Point3, da: Vec3, ob: Point3, db: Vec3, tol: Tolerance) -> CurveIntersection {
    let cross = da.cross(&db);
    let offset = ob - oa;
    if cross.norm().atan2(da.dot(&db).abs()) <= tol.angular {
        // Parallel: the same line, or never meeting.
        let across = offset - offset.dot(&da) * da;
        return if across.norm() <= tol.linear {
            CurveIntersection::Coincident
        } else {
            CurveIntersection::Points(Vec::new())
        };
    }
    // Skew or crossing: the parameters of the nearest approach, and the
    // gap between the two points there.
    let n2 = cross.norm_squared();
    let ta = offset.cross(&db).dot(&cross) / n2;
    let tb = offset.cross(&da).dot(&cross) / n2;
    let (pa, pb) = (oa + ta * da, ob + tb * db);
    if (pa - pb).norm() > tol.linear {
        return CurveIntersection::Points(Vec::new());
    }
    CurveIntersection::Points(vec![CurveCurveHit {
        ta,
        tb,
        point: pa,
        tangent: false,
    }])
}

/// `a` against the conic `b`, through `b`'s plane: every common point is
/// in that plane, so the plane's hits are the candidates and `b`'s own
/// projection keeps the ones that are on it.
fn through_plane(a: &Curve, b: &Curve, tol: Tolerance) -> Result<CurveIntersection, GeomError> {
    let Some((frame, _)) = conic_frame(b) else {
        return Err(GeomError::Unsupported {
            a: GeomKind::Curve(a.kind()),
            b: GeomKind::Curve(b.kind()),
        });
    };
    let plane = Surface::Plane { frame: *frame };
    let hits = match intersect_curve_surface(a, &plane, tol)? {
        CurveSurfaceIntersection::Points(hits) => hits,
        CurveSurfaceIntersection::Coincident => return coplanar(a, b, tol),
    };
    let mut out = Vec::with_capacity(hits.len());
    for h in hits {
        let projection = match b.project(h.point) {
            Ok(p) => p,
            // The point is the conic's centre, which is equidistant from
            // every point of it: on the conic only for a radius below the
            // tolerance, which no valid conic has.
            Err(GeomError::Ambiguous { .. }) => continue,
            Err(e) => return Err(e),
        };
        if projection.distance <= tol.linear {
            out.push(CurveCurveHit {
                ta: h.t,
                tb: projection.t,
                point: h.point,
                tangent: h.tangent,
            });
        }
    }
    Ok(points(out))
}

/// `a` and the conic `b` in one plane, where the plane decides nothing.
fn coplanar(a: &Curve, b: &Curve, tol: Tolerance) -> Result<CurveIntersection, GeomError> {
    match (a, b) {
        (&Curve::Line { origin, direction }, _) => {
            let (frame, _) = conic_frame(b).ok_or(GeomError::Unsupported {
                a: GeomKind::Curve(a.kind()),
                b: GeomKind::Curve(b.kind()),
            })?;
            line_conic_coplanar(origin, direction.into_inner(), frame, b, tol)
        }
        (
            &Curve::Circle {
                frame: fa,
                radius: ra,
            },
            &Curve::Circle {
                frame: fb,
                radius: rb,
            },
        ) => circle_circle_coplanar(a, b, &fa, ra, &fb, rb, tol),
        // A coplanar pair with an ellipse in it: the same conic twice is
        // decided by its closed form — the centres, the radii and the
        // axes — and anything else they might share is the quartic,
        // cycle 3's.
        (
            Curve::Circle { .. } | Curve::Ellipse { .. },
            Curve::Circle { .. } | Curve::Ellipse { .. },
        ) => {
            if let (Some((fa, ra)), Some((fb, rb))) = (conic_frame(a), conic_frame(b)) {
                if conics_coincide(fa, ra, fb, rb, tol) {
                    return Ok(CurveIntersection::Coincident);
                }
            }
            Err(GeomError::Unsupported {
                a: GeomKind::Curve(a.kind()),
                b: GeomKind::Curve(b.kind()),
            })
        }
        (Curve::Nurbs(_), _)
        | (Curve::Circle { .. } | Curve::Ellipse { .. }, Curve::Line { .. } | Curve::Nurbs(_)) => {
            Err(GeomError::Unsupported {
                a: GeomKind::Curve(a.kind()),
                b: GeomKind::Curve(b.kind()),
            })
        }
    }
}

/// `true` when two coplanar conics are the same point set: the centres
/// within `tol.linear`, and either both are circles of one radius, or
/// both are ellipses of the same two radii with their major axes
/// parallel within `tol.angular` (either way along), the radii swapped
/// with the axes when the frames name them the other way round. A
/// circle and an ellipse of two distinct radii are never the same.
fn conics_coincide(fa: &Frame, ra: [f64; 2], fb: &Frame, rb: [f64; 2], tol: Tolerance) -> bool {
    if (fa.origin() - fb.origin()).norm() > tol.linear {
        return false;
    }
    let same = |x: f64, y: f64| (x - y).abs() <= tol.linear;
    let round = |r: [f64; 2]| same(r[0], r[1]);
    if round(ra) || round(rb) {
        return round(ra) && round(rb) && same(ra[0], rb[0]);
    }
    let parallel = |x: Vec3, y: Vec3| x.cross(&y).norm() <= tol.angular;
    let (xa, xb, yb) = (
        fa.x().into_inner(),
        fb.x().into_inner(),
        fb.y().into_inner(),
    );
    (same(ra[0], rb[0]) && same(ra[1], rb[1]) && parallel(xa, xb))
        || (same(ra[0], rb[1]) && same(ra[1], rb[0]) && parallel(xa, yb))
}

/// A line in the conic's own plane: the points the two share are the
/// conic's points on the plane through the line perpendicular to the
/// conic's, which [`intersect_curve_surface`] already decides — including
/// the tangency, in the linear tolerance.
fn line_conic_coplanar(
    origin: Point3,
    direction: Vec3,
    conic: &Frame,
    b: &Curve,
    tol: Tolerance,
) -> Result<CurveIntersection, GeomError> {
    let normal = conic.z().cross(&direction);
    let cut = Frame::from_z(origin, normal).map_err(|_| GeomError::Degenerate {
        kind: GeomKind::Curve(CurveKind::Line),
        reason: "a line in the conic's plane whose direction is not a direction".to_string(),
    })?;
    let plane = Surface::Plane { frame: cut };
    let found = match intersect_curve_surface(b, &plane, tol)? {
        CurveSurfaceIntersection::Points(hits) => hits,
        // The conic lies in the cutting plane too, which would make it a
        // line: no valid conic does.
        CurveSurfaceIntersection::Coincident => {
            return Err(GeomError::Degenerate {
                kind: GeomKind::Curve(b.kind()),
                reason: "a conic inside a plane perpendicular to its own".to_string(),
            });
        }
    };
    Ok(points(
        found
            .into_iter()
            .map(|h| CurveCurveHit {
                ta: (h.point - origin).dot(&direction),
                tb: h.t,
                point: h.point,
                tangent: h.tangent,
            })
            .collect(),
    ))
}

/// Two circles in one plane: the radical line. Worked in `b`'s frame,
/// where `b` is the unit of the construction and `a`'s centre is one
/// vector away.
fn circle_circle_coplanar(
    a: &Curve,
    b: &Curve,
    fa: &Frame,
    ra: f64,
    fb: &Frame,
    rb: f64,
    tol: Tolerance,
) -> Result<CurveIntersection, GeomError> {
    let local = fb.to_local(fa.origin());
    let centre = Vec2::new(local.x, local.y);
    let apart = centre.norm();
    if apart <= tol.linear {
        return Ok(if (ra - rb).abs() <= tol.linear {
            CurveIntersection::Coincident
        } else {
            // Concentric and of different radii: never meeting.
            CurveIntersection::Points(Vec::new())
        });
    }
    let along = centre / apart;
    let across = Vec2::new(-along.y, along.x);
    let lift = |p: Vec2| fb.to_world(Point3::new(p.x, p.y, 0.0));
    let touch = |at: Vec2| -> Result<CurveIntersection, GeomError> {
        Ok(points(vec![hit(a, b, lift(at), true)?]))
    };
    // Outer touch, inner touch, and the two ways to miss.
    if (apart - (ra + rb)).abs() <= tol.linear {
        return touch(rb * along);
    }
    if (apart - (ra - rb).abs()).abs() <= tol.linear {
        return touch(if ra > rb { -rb * along } else { rb * along });
    }
    if apart > ra + rb || apart < (ra - rb).abs() {
        return Ok(CurveIntersection::Points(Vec::new()));
    }
    // The chord: at `x` along the line of centres, half-length `h`.
    let x = (apart * apart + rb * rb - ra * ra) / (2.0 * apart);
    let h = (rb - x).sqrt() * (rb + x).sqrt();
    Ok(points(vec![
        hit(a, b, lift(x * along + h * across), false)?,
        hit(a, b, lift(x * along - h * across), false)?,
    ]))
}

/// A hit at `point`, with each curve's parameter from its own projection.
fn hit(a: &Curve, b: &Curve, point: Point3, tangent: bool) -> Result<CurveCurveHit, GeomError> {
    Ok(CurveCurveHit {
        ta: wrap_angle_if_periodic(a, a.project(point)?.t),
        tb: wrap_angle_if_periodic(b, b.project(point)?.t),
        point,
        tangent,
    })
}

/// A periodic curve's parameter in `[0, 2π)`; anything else unchanged.
fn wrap_angle_if_periodic(c: &Curve, t: f64) -> f64 {
    if c.period().is_some() {
        wrap_angle(t)
    } else {
        t
    }
}
