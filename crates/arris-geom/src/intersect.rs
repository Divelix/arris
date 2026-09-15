//! Surface–surface intersection: the closed-form table.

use core::f64::consts::FRAC_PI_2;

use arris_math::{Frame, Point3, Tolerance, UnitVec3, Vec3};

use crate::{Curve, GeomError, GeomKind, Surface};

/// What two surfaces have in common.
///
/// Every curve is an exact analytic curve lying on both surfaces to
/// rounding, with a frame that is Arris's own deterministic choice
/// (`docs/DATA-MODEL.md` §Curves): a circle on a cylinder takes the
/// cylinder's `X`, a plane's ellipse on a cylinder has its `X` along its
/// major axis in the direction of increasing `v`, two crossing cylinders'
/// ellipses have each `Z` and `X` signed so the largest-magnitude
/// component is positive, a ruling on a cylinder runs along its `Z` from
/// the point nearest the cylinder's origin (the first cylinder's, for two
/// parallel ones), and the line of two planes starts at its point nearest
/// the first plane's origin, a circle about a shared axis takes the
/// frame of the first operand that carries the axis (ADR-0008), and a
/// plane through a cone's or a torus's axis gives a ruling from the apex
/// along `∂P/∂v` or a tube circle whose `t` is the torus's `v`. Swapping
/// the operands gives the same point sets, up to the orientation of a
/// line and the order of two parallel cylinders' rulings — and two
/// crossing cylinders' ellipses bit for bit.
#[derive(Debug, Clone, PartialEq)]
pub enum SurfaceIntersection {
    /// The surfaces do not meet: parallel planes apart by more than the
    /// linear tolerance, a plane clear of a cylinder, two cylinders apart
    /// or one nested in the other, coaxial surfaces of revolution whose
    /// meridians never meet.
    Empty,
    /// The surfaces are the same surface within the tolerance; there is
    /// no curve to return.
    Coincident,
    /// The surfaces cross along these curves.
    Transversal(Vec<Curve>),
    /// The surfaces touch along these curves without crossing: a plane
    /// tangent to a cylinder along a ruling, two cylinders with parallel
    /// axes touching outside or inside, a torus on a plane or inside a
    /// cylinder, a sphere on a cylinder of its radius.
    Tangent(Vec<Curve>),
    /// The surfaces meet only at these isolated points: a touch — a plane
    /// tangent to a sphere, two spheres touching — or a crossing through a
    /// singular point — a plane perpendicular to a cone through its apex,
    /// two cones closing on one apex. Every point lies on the surfaces'
    /// shared axis, and they come ascending along it in the direction the
    /// first operand carrying the axis points.
    Points(Vec<Point3>),
}

/// The intersection of two surfaces, by the case table: every
/// pair with a closed form is computed exactly, every other pair is an
/// explicit [`GeomError::Unsupported`] arm — no wildcard, no marcher.
///
/// Guarantees: the result is symmetric under swapping `a` and `b` up to
/// a line's orientation, deterministic bit for bit, and each returned
/// curve lies on both surfaces to rounding. `tol.angular` decides
/// parallel and perpendicular; `tol.linear` decides coincident, tangent
/// and empty.
///
/// The table: plane–plane is `Empty`, `Coincident` or one `Transversal`
/// line; plane–cylinder is a circle when the normal is parallel to the
/// axis, an ellipse when oblique (`b = R`, `a = R / |n · Z|`, centred at
/// the axis's piercing point), and when perpendicular two `Transversal`
/// rulings, one `Tangent` ruling or `Empty` by the axis-to-plane distance
/// against `R`. Cylinder–cylinder with parallel axes is `Coincident` or
/// `Empty` when coaxial, one `Tangent` ruling when the axes are `ra + rb`
/// or `|ra − rb|` apart, two `Transversal` rulings between those
/// distances and `Empty` beyond them; with crossing axes and equal radii
/// it is two `Transversal` ellipses in the planes bisecting the axes;
/// with skew axes further apart than `ra + rb` it is `Empty`. Crossing
/// axes of unequal radii and skew axes within `ra + rb` meet in a quartic
/// space curve and are `Unsupported`, C3's. Every pair with a cone, a
/// sphere or a torus in it is decided when the two share an axis — a
/// plane perpendicular to it, a cylinder, cone or torus on it, a sphere
/// centred on it, and every plane–sphere and sphere–sphere pair — by one
/// arm over the meridian sections in the plane through the axis
/// (ADR-0008, `docs/DATA-MODEL.md` §Curves): circles about the axis,
/// `Transversal` or `Tangent`, `Points` on it, `Coincident` or `Empty`;
/// a plane through a cone's or a torus's axis cuts its meridian, two
/// `Transversal` rulings through the apex or two tube circles; a pair
/// sharing no axis, and a result that would mix kinds, is
/// `Unsupported`. Read `IntAna_QuadQuadGeo` in the reference tree for
/// the case analysis, reimplemented on our frames.
///
/// ```
/// use arris_geom::{Curve, Surface, SurfaceIntersection, intersect_surfaces};
/// use arris_math::{Frame, Point3, Precision, Vec3};
///
/// let cap = Surface::Plane { frame: Frame::from_z(Point3::new(0.0, 0.0, 5.0), Vec3::z()).unwrap() };
/// let wall = Surface::Cylinder { frame: Frame::world(), radius: 2.0 };
/// let hit = intersect_surfaces(&cap, &wall, Precision::DEFAULT.tolerance()).unwrap();
/// let SurfaceIntersection::Transversal(curves) = hit else { panic!() };
/// let Curve::Circle { frame, radius } = &curves[0] else { panic!() };
/// assert_eq!(*radius, 2.0);
/// assert_eq!(frame.origin(), Point3::new(0.0, 0.0, 5.0));
/// ```
pub fn intersect_surfaces(
    a: &Surface,
    b: &Surface,
    tol: Tolerance,
) -> Result<SurfaceIntersection, GeomError> {
    if !tol.is_consistent() {
        return Err(GeomError::InvalidTolerance(tol));
    }
    match (a, b) {
        (Surface::Plane { frame: pa }, Surface::Plane { frame: pb }) => {
            Ok(plane_plane(pa, pb, tol))
        }
        (Surface::Plane { frame: plane }, Surface::Cylinder { frame, radius })
        | (Surface::Cylinder { frame, radius }, Surface::Plane { frame: plane }) => {
            plane_cylinder(plane, frame, *radius, tol)
        }
        (
            Surface::Cylinder {
                frame: ca,
                radius: ra,
            },
            Surface::Cylinder {
                frame: cb,
                radius: rb,
            },
        ) => cylinder_cylinder(a, b, ca, *ra, cb, *rb, tol),
        (
            Surface::Plane { .. }
            | Surface::Cylinder { .. }
            | Surface::Cone { .. }
            | Surface::Sphere { .. }
            | Surface::Torus { .. },
            Surface::Cone { .. } | Surface::Sphere { .. } | Surface::Torus { .. },
        )
        | (
            Surface::Cone { .. } | Surface::Sphere { .. } | Surface::Torus { .. },
            Surface::Plane { .. } | Surface::Cylinder { .. },
        ) => crate::meridian::intersect_coaxial(a, b, tol),
        (
            Surface::Nurbs(_),
            Surface::Plane { .. }
            | Surface::Cylinder { .. }
            | Surface::Cone { .. }
            | Surface::Sphere { .. }
            | Surface::Torus { .. }
            | Surface::Nurbs(_),
        )
        | (
            Surface::Plane { .. }
            | Surface::Cylinder { .. }
            | Surface::Cone { .. }
            | Surface::Sphere { .. }
            | Surface::Torus { .. },
            Surface::Nurbs(_),
        ) => Err(GeomError::Unsupported {
            a: GeomKind::Surface(a.kind()),
            b: GeomKind::Surface(b.kind()),
        }),
    }
}

/// Two cylinders, as far as there is a closed form: parallel axes in
/// [`parallel_cylinders`], crossing axes of equal radii in
/// [`crossing_cylinders`], and skew axes further apart than the two radii
/// `Empty` — every point of a cylinder is within its radius of its axis,
/// so by the triangle inequality the two never meet. Crossing axes of
/// unequal radii and skew axes within the radii meet in a quartic space
/// curve with no conic form, which is C3's (`docs/DATA-MODEL.md` §Curves,
/// the open question): [`GeomError::Unsupported`]. The unsupported poses
/// are written out, not a wildcard: a new surface kind still fails the
/// match to compile.
fn cylinder_cylinder(
    a: &Surface,
    b: &Surface,
    ca: &Frame,
    ra: f64,
    cb: &Frame,
    rb: f64,
    tol: Tolerance,
) -> Result<SurfaceIntersection, GeomError> {
    let unsupported = || GeomError::Unsupported {
        a: GeomKind::Surface(a.kind()),
        b: GeomKind::Surface(b.kind()),
    };
    let offset = cb.origin() - ca.origin();
    if line_angle(&ca.z(), &cb.z()) <= tol.angular {
        return Ok(parallel_cylinders(ca, ra, rb, offset, tol));
    }
    // The axes are not parallel, so their cross product has a length of at
    // least sin(tol.angular) and normalises; only a non-finite frame fails.
    let Some(common) = UnitVec3::try_new(ca.z().cross(&cb.z()), 0.0) else {
        return Err(frame_degenerate(GeomKind::Surface(
            crate::SurfaceKind::Cylinder,
        )));
    };
    // The length of the axes' common perpendicular: their nearest approach.
    let gap = common.dot(&offset).abs();
    if gap > tol.linear {
        return if gap > ra + rb + tol.linear {
            Ok(SurfaceIntersection::Empty)
        } else {
            Err(unsupported())
        };
    }
    if (ra - rb).abs() > tol.linear {
        return Err(unsupported());
    }
    crossing_cylinders(ca, cb, offset, 0.5 * (ra + rb))
}

/// Two cylinders whose axes are parallel, `d` the distance between the
/// axes: coaxial (`d` within `tol.linear`) is `Coincident` when the radii
/// agree and `Empty` when they do not; `d` within `tol.linear` of
/// `ra + rb` (outside) or of `|ra − rb|` (inside) is one `Tangent` ruling;
/// strictly between the two, the two `Transversal` rulings through the
/// crossing points of the two circles in a plane across the axes;
/// otherwise `Empty`. Every ruling runs along `ca`'s `Z` from its point in
/// the plane across the axis through `ca`'s origin — the point nearest
/// that origin — and the two are ordered by their offset along
/// `Z × ŵ`, `ŵ` the unit vector from `ca`'s axis toward `cb`'s: negative
/// first. The tangent ruling is on `ca` to rounding, `ra` along `±ŵ`.
fn parallel_cylinders(
    ca: &Frame,
    ra: f64,
    rb: f64,
    offset: Vec3,
    tol: Tolerance,
) -> SurfaceIntersection {
    let axis = ca.z();
    let z: Vec3 = axis.into_inner();
    let across = offset - offset.dot(&z) * z;
    let d = across.norm();
    if d <= tol.linear {
        return if (ra - rb).abs() <= tol.linear {
            SurfaceIntersection::Coincident
        } else {
            SurfaceIntersection::Empty
        };
    }
    let towards = across / d;
    // The crossing points' offset along `towards` from the first axis, by
    // the radical line of the two circles.
    let along = (d * d + ra * ra - rb * rb) / (2.0 * d);
    let outside = (d - (ra + rb)).abs() <= tol.linear;
    let inside = (d - (ra - rb).abs()).abs() <= tol.linear;
    if outside || inside {
        // `along` is `ra` at an outside touch and at an inside one around
        // the smaller cylinder, `−ra` at an inside one within the larger:
        // its sign says which side of the first axis the ruling is on.
        return SurfaceIntersection::Tangent(vec![Curve::Line {
            origin: ca.origin() + ra.copysign(along) * towards,
            direction: axis,
        }]);
    }
    if d > ra + rb || d < (ra - rb).abs() {
        return SurfaceIntersection::Empty;
    }
    // Strictly between the tangent distances `|along| < ra`, clear of it by
    // more than the tolerance; the clamp only absorbs rounding.
    let half = (ra * ra - along * along).max(0.0).sqrt();
    let side = z.cross(&towards);
    let foot = ca.origin() + along * towards;
    SurfaceIntersection::Transversal(vec![
        Curve::Line {
            origin: foot - half * side,
            direction: axis,
        },
        Curve::Line {
            origin: foot + half * side,
            direction: axis,
        },
    ])
}

/// Two cylinders of one `radius` whose axes cross: two `Transversal`
/// ellipses in the planes that bisect the axes, centred at the crossing.
/// With `a` and `b` the axes, `b` flipped so that `a · b ≥ 0` and `ψ` the
/// angle between them, the first ellipse has `Z` along `a − b`, `X` along
/// `a + b` and major radius `R / sin(ψ/2)`; the second has `Z` along
/// `a + b`, `X` along `a − b` and major radius `R / cos(ψ/2)`; both have
/// minor radius `R` along `a × b`. A point equidistant from both axes in
/// a plane through the crossing is on one cylinder exactly when it is on
/// the other, and those planes are the bisectors; each ellipse is then
/// `ca`'s oblique section. Each `Z` and `X` takes the sign that makes its
/// largest-magnitude component positive, the lower index on a tie, and
/// the crossing is the midpoint of the axes' nearest points summed in
/// either order: swapping the operands negates `a − b` exactly and leaves
/// `a + b` and the midpoint as they were, so it gives the same ellipses bit
/// for bit, the same way round — one parametrisation, and one fit of each
/// pcurve. The two ellipses cross each other at `±R` along `a × b`.
fn crossing_cylinders(
    ca: &Frame,
    cb: &Frame,
    offset: Vec3,
    radius: f64,
) -> Result<SurfaceIntersection, GeomError> {
    let a: Vec3 = ca.z().into_inner();
    let b: Vec3 = if ca.z().dot(&cb.z()) < 0.0 {
        -cb.z().into_inner()
    } else {
        cb.z().into_inner()
    };
    // The nearest points of the two axes: `s` along `a` from `ca`'s origin
    // and `t` along `b` from `cb`'s, `1 − c²` being `sin²ψ`, not zero.
    let c = a.dot(&b);
    let denom = 1.0 - c * c;
    let (on_a, on_b) = (offset.dot(&a), offset.dot(&b));
    let s = (on_a - c * on_b) / denom;
    let t = (c * on_a - on_b) / denom;
    let near_a = ca.origin() + s * a;
    let near_b = cb.origin() + t * b;
    let centre = Point3::from(0.5 * (near_a.coords + near_b.coords));
    // Signed so that neither operand order is preferred: see the doc.
    let canonical = |v: Vec3| {
        let k = (0..3).fold(0, |k, i| if v[i].abs() > v[k].abs() { i } else { k });
        if v[k] < 0.0 { -v } else { v }
    };
    let (minus, plus) = (canonical(a - b), canonical(a + b));
    // `|a − b| = 2 sin(ψ/2)` and `|a + b| = 2 cos(ψ/2)`, each measured
    // directly so a small angle keeps its digits.
    let ellipse = |z: Vec3, x: Vec3, half_chord: f64| {
        Frame::new(centre, z, x)
            .map(|frame| Curve::Ellipse {
                frame,
                major_radius: 2.0 * radius / half_chord,
                minor_radius: radius,
            })
            .map_err(|_| frame_degenerate(GeomKind::Surface(crate::SurfaceKind::Cylinder)))
    };
    Ok(SurfaceIntersection::Transversal(vec![
        ellipse(minus, plus, minus.norm())?,
        ellipse(plus, minus, plus.norm())?,
    ]))
}

/// The angle in `[0, π/2]` between the lines carried by two unit vectors:
/// `atan2` of the cross and dot magnitudes, well conditioned at both ends
/// where `acos` is not.
pub(crate) fn line_angle(a: &UnitVec3, b: &UnitVec3) -> f64 {
    a.cross(b).norm().atan2(a.dot(b).abs())
}

fn plane_plane(pa: &Frame, pb: &Frame, tol: Tolerance) -> SurfaceIntersection {
    let (n1, n2) = (pa.z(), pb.z());
    if line_angle(&n1, &n2) <= tol.angular {
        let gap = n1.dot(&(pb.origin() - pa.origin())).abs();
        return if gap <= tol.linear {
            SurfaceIntersection::Coincident
        } else {
            SurfaceIntersection::Empty
        };
    }
    // The normals are not parallel, so the cross product has a length of
    // at least sin(tol.angular) and normalises; a failure here is a
    // non-finite frame, which no constructor produces.
    let Some(direction) = UnitVec3::try_new(n1.cross(&n2), 0.0) else {
        return SurfaceIntersection::Empty;
    };
    // Of the points on both planes, the one nearest to plane a's origin:
    // solve n1·x = h1, n2·x = h2 in the span of n1 and n2 over a's origin.
    let c = n1.dot(&n2);
    let h1 = 0.0;
    let h2 = n2.dot(&(pb.origin() - pa.origin()));
    let denom = 1.0 - c * c;
    let s1 = (h1 - h2 * c) / denom;
    let s2 = (h2 - h1 * c) / denom;
    let origin = pa.origin() + s1 * n1.into_inner() + s2 * n2.into_inner();
    SurfaceIntersection::Transversal(vec![Curve::Line { origin, direction }])
}

fn plane_cylinder(
    plane: &Frame,
    cyl: &Frame,
    radius: f64,
    tol: Tolerance,
) -> Result<SurfaceIntersection, GeomError> {
    let (n, axis) = (plane.z(), cyl.z());
    let angle = line_angle(&n, &axis);
    if angle <= tol.angular {
        // Normal along the axis: the plane cuts a circle at the piercing
        // point, with the cylinder's own axes so the seam is shared.
        let t = n.dot(&(plane.origin() - cyl.origin())) / n.dot(&axis);
        let frame = cyl.with_origin(cyl.origin() + t * axis.into_inner());
        return Ok(SurfaceIntersection::Transversal(vec![Curve::Circle {
            frame,
            radius,
        }]));
    }
    if FRAC_PI_2 - angle <= tol.angular {
        // Normal across the axis: the axis is parallel to the plane at a
        // signed distance `dist`, and the section is made of rulings.
        let dist = n.dot(&(cyl.origin() - plane.origin()));
        let foot = cyl.origin() - dist * n.into_inner();
        if (dist.abs() - radius).abs() <= tol.linear {
            return Ok(SurfaceIntersection::Tangent(vec![Curve::Line {
                origin: foot,
                direction: axis,
            }]));
        }
        if dist.abs() < radius {
            let half = (radius * radius - dist * dist).sqrt();
            let Some(across) = UnitVec3::try_new(n.cross(&axis), 0.0) else {
                return Err(frame_degenerate(GeomKind::Surface(
                    crate::SurfaceKind::Cylinder,
                )));
            };
            let across: Vec3 = across.into_inner();
            return Ok(SurfaceIntersection::Transversal(vec![
                Curve::Line {
                    origin: foot - half * across,
                    direction: axis,
                },
                Curve::Line {
                    origin: foot + half * across,
                    direction: axis,
                },
            ]));
        }
        return Ok(SurfaceIntersection::Empty);
    }
    // Oblique: an ellipse centred at the piercing point, minor axis `R`
    // across the axis, major axis `R / cos` along the axis's projection
    // onto the plane — the direction of increasing `v`.
    let cos = n.dot(&axis).abs();
    let t = n.dot(&(plane.origin() - cyl.origin())) / n.dot(&axis);
    let centre: Point3 = cyl.origin() + t * axis.into_inner();
    let frame = Frame::new(centre, n.into_inner(), axis.into_inner())
        .map_err(|_| frame_degenerate(GeomKind::Surface(crate::SurfaceKind::Plane)))?;
    Ok(SurfaceIntersection::Transversal(vec![Curve::Ellipse {
        frame,
        major_radius: radius / cos,
        minor_radius: radius,
    }]))
}

/// The error for a frame that could not be built from an operand's axes:
/// only a non-finite frame reaches it, since the angular tests above rule
/// out parallel axes.
fn frame_degenerate(kind: GeomKind) -> GeomError {
    GeomError::Degenerate {
        kind,
        reason: "non-finite frame".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arris_math::Precision;

    fn tol() -> Tolerance {
        Precision::DEFAULT.tolerance()
    }

    #[test]
    fn coordinate_planes_meet_along_an_axis() {
        let xy = Surface::Plane {
            frame: Frame::world(),
        };
        let yz = Surface::Plane {
            frame: Frame::from_z(Point3::new(3.0, 0.0, 0.0), Vec3::x()).unwrap(),
        };
        let SurfaceIntersection::Transversal(curves) = intersect_surfaces(&xy, &yz, tol()).unwrap()
        else {
            panic!()
        };
        let Curve::Line { origin, direction } = &curves[0] else {
            panic!()
        };
        assert_eq!(*origin, Point3::new(3.0, 0.0, 0.0));
        assert_eq!(direction.into_inner().abs(), Vec3::y());
    }

    #[test]
    fn parallel_planes_are_empty_or_coincident() {
        let a = Surface::Plane {
            frame: Frame::world(),
        };
        let lifted = Surface::Plane {
            frame: Frame::from_z(Point3::new(1.0, 2.0, 0.5), Vec3::z()).unwrap(),
        };
        let flipped = Surface::Plane {
            frame: Frame::from_z(Point3::new(1.0, 2.0, 0.0), -Vec3::z()).unwrap(),
        };
        assert_eq!(
            intersect_surfaces(&a, &lifted, tol()).unwrap(),
            SurfaceIntersection::Empty
        );
        assert_eq!(
            intersect_surfaces(&a, &flipped, tol()).unwrap(),
            SurfaceIntersection::Coincident
        );
    }

    #[test]
    fn an_inconsistent_tolerance_is_an_error() {
        let a = Surface::Plane {
            frame: Frame::world(),
        };
        assert!(matches!(
            intersect_surfaces(&a, &a, Tolerance::new(0.0, 1e-12)),
            Err(GeomError::InvalidTolerance(_))
        ));
    }
}
