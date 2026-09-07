//! Surface–surface intersection: the closed-form table of cycle 1.

use core::f64::consts::FRAC_PI_2;

use arris_math::{Frame, Point3, Tolerance, UnitVec3, Vec3};

use crate::{Curve, GeomError, GeomKind, Surface};

/// What two surfaces have in common.
///
/// Every curve is an exact analytic curve lying on both surfaces to
/// rounding, with a frame that is Arris's own deterministic choice
/// (`docs/02-data-model.md` §Curves): a circle on a cylinder takes the
/// cylinder's `X`, an ellipse's `X` is its major axis in the direction of
/// increasing `v`, a ruling on a cylinder runs along its `Z` from the
/// point nearest the cylinder's origin, and the line of two planes starts
/// at its point nearest the first plane's origin. Swapping the operands
/// gives the same point sets, up to the orientation of a line.
#[derive(Debug, Clone, PartialEq)]
pub enum SurfaceIntersection {
    /// The surfaces do not meet: parallel planes apart by more than the
    /// linear tolerance, a plane clear of a cylinder.
    Empty,
    /// The surfaces are the same surface within the tolerance; there is
    /// no curve to return.
    Coincident,
    /// The surfaces cross along these curves.
    Transversal(Vec<Curve>),
    /// The surfaces touch along these curves without crossing: a plane
    /// tangent to a cylinder along a ruling.
    Tangent(Vec<Curve>),
}

/// The intersection of two surfaces, by the case table of cycle 1: every
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
/// against `R`; cylinder–cylinder is `Coincident` or `Empty` for two
/// coaxial cylinders and `Unsupported` for every other pose, the quartic
/// space curve being cycle 2's. Read `IntAna_QuadQuadGeo` in the
/// reference tree for the case analysis, reimplemented on our frames.
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
            | Surface::Torus { .. }
            | Surface::Nurbs(_),
            Surface::Plane { .. }
            | Surface::Cylinder { .. }
            | Surface::Cone { .. }
            | Surface::Sphere { .. }
            | Surface::Torus { .. }
            | Surface::Nurbs(_),
        ) => Err(GeomError::Unsupported {
            a: GeomKind::Surface(a.kind()),
            b: GeomKind::Surface(b.kind()),
        }),
    }
}

/// Two cylinders, as far as cycle 1 has a closed form: `Coincident` when
/// the axes are the same line and the radii agree, `Empty` when they are
/// the same line and the radii do not (two coaxial tubes never meet), and
/// [`GeomError::Unsupported`] otherwise — the curve of two crossing
/// cylinders is a quartic space curve with no conic form, and it is
/// cycle 2's (`docs/02-data-model.md` §Curves, the open question). The
/// unsupported case is written out, not a wildcard: a new surface kind
/// still fails the match to compile.
fn cylinder_cylinder(
    a: &Surface,
    b: &Surface,
    ca: &Frame,
    ra: f64,
    cb: &Frame,
    rb: f64,
    tol: Tolerance,
) -> Result<SurfaceIntersection, GeomError> {
    let coaxial = line_angle(&ca.z(), &cb.z()) <= tol.angular && {
        // The offset between the origins, across the axis: zero when the
        // two axes are one line.
        let offset = cb.origin() - ca.origin();
        offset.cross(&ca.z()).norm() <= tol.linear
    };
    if !coaxial {
        return Err(GeomError::Unsupported {
            a: GeomKind::Surface(a.kind()),
            b: GeomKind::Surface(b.kind()),
        });
    }
    Ok(if (ra - rb).abs() <= tol.linear {
        SurfaceIntersection::Coincident
    } else {
        SurfaceIntersection::Empty
    })
}

/// The angle in `[0, π/2]` between the lines carried by two unit vectors:
/// `atan2` of the cross and dot magnitudes, well conditioned at both ends
/// where `acos` is not.
fn line_angle(a: &UnitVec3, b: &UnitVec3) -> f64 {
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
