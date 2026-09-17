//! Surface–surface intersection: the closed-form table.

use core::f64::consts::FRAC_PI_2;

use arris_math::{Frame, Point2, Point3, Tolerance, UnitVec3, Vec2, Vec3};

use crate::conic2::{Conic2, ConicMeet, conic_pair};
use crate::pcurve::principal_axes;
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
/// space curve and are `Unsupported`, C3's. A plane against an
/// **elliptic cylinder** (ADR-0014) is decided in every pose: the section
/// ellipse when the normal is parallel to the axis, and when
/// perpendicular two `Transversal` rulings, one `Tangent` ruling or
/// `Empty` by the plane's offset against the section's reach along its
/// normal, and oblique an ellipse — the affine image of the section,
/// its axes the singular values of the section's semi-diameters
/// projected onto the plane. An elliptic cylinder against a cylinder or
/// another elliptic cylinder with parallel axes meets where the two
/// sections meet in the plane across the axes, through the quartic of
/// [`arris_math::roots`]: `Coincident`, `Empty`, `Tangent` rulings at
/// the touches or `Transversal` rulings at the crossings, ascending by
/// the first operand's section parameter — up to four of them; a
/// section pair that both touches and crosses mixes kinds and is
/// `Unsupported`, as are crossing axes and every pair of the elliptic
/// cylinder with a cone, a sphere, a torus or a NURBS. Every pair with a cone, a
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
            Surface::Plane { frame: plane },
            Surface::EllipticCylinder {
                frame,
                major_radius,
                minor_radius,
            },
        )
        | (
            Surface::EllipticCylinder {
                frame,
                major_radius,
                minor_radius,
            },
            Surface::Plane { frame: plane },
        ) => plane_elliptic_cylinder(plane, frame, [*major_radius, *minor_radius], tol),
        (
            Surface::Cylinder {
                frame: ca,
                radius: ra,
            },
            Surface::EllipticCylinder {
                frame: cb,
                major_radius,
                minor_radius,
            },
        ) => elliptic_pair(
            a,
            b,
            (ca, [*ra, *ra]),
            (cb, [*major_radius, *minor_radius]),
            tol,
        ),
        (
            Surface::EllipticCylinder {
                frame: ca,
                major_radius,
                minor_radius,
            },
            Surface::Cylinder {
                frame: cb,
                radius: rb,
            },
        ) => elliptic_pair(
            a,
            b,
            (ca, [*major_radius, *minor_radius]),
            (cb, [*rb, *rb]),
            tol,
        ),
        (
            Surface::EllipticCylinder {
                frame: ca,
                major_radius: aa,
                minor_radius: ab,
            },
            Surface::EllipticCylinder {
                frame: cb,
                major_radius: ba,
                minor_radius: bb,
            },
        ) => elliptic_pair(a, b, (ca, [*aa, *ab]), (cb, [*ba, *bb]), tol),
        (
            Surface::EllipticCylinder { .. },
            Surface::Cone { .. }
            | Surface::Sphere { .. }
            | Surface::Torus { .. }
            | Surface::Nurbs(_),
        )
        | (
            Surface::Cone { .. }
            | Surface::Sphere { .. }
            | Surface::Torus { .. }
            | Surface::Nurbs(_),
            Surface::EllipticCylinder { .. },
        ) => Err(GeomError::Unsupported {
            a: GeomKind::Surface(a.kind()),
            b: GeomKind::Surface(b.kind()),
        }),
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

/// A plane against an elliptic cylinder, `[a, b]` its radii, in every
/// pose (ADR-0014). Normal along the axis: the section ellipse at the
/// piercing point, with the cylinder's own axes so the seam is shared.
/// Normal across the axis: the plane cuts the section in the line at
/// its signed offset `d` from the section's centre along the plane's
/// normal, and the section reaches `±M` along that normal, `M = √((a
/// n·X)² + (b n·Y)²)`; `|d|` within `tol.linear` of `M` is one `Tangent`
/// ruling, `|d| < M` two `Transversal` rulings at `φ ± acos(d / M)`, `φ`
/// the parameter of the farthest reach, ordered by their offset along
/// `n × Z` (negative first, as plane–cylinder orders them), and beyond
/// `Empty`. Oblique: an ellipse centred at the axis's piercing point,
/// the affine image of the section — `cos u·A + sin u·B` with `A`, `B`
/// the section's semi-diameters slid along the axis into the plane —
/// whose axes are the singular values of `[A | B]` in the plane's own
/// basis (`e₁` the axis projected onto the plane, `e₂ = n × e₁`); its
/// `Z` is the plane's normal and its `X` the major axis.
fn plane_elliptic_cylinder(
    plane: &Frame,
    cyl: &Frame,
    [a, b]: [f64; 2],
    tol: Tolerance,
) -> Result<SurfaceIntersection, GeomError> {
    let (n, axis) = (plane.z(), cyl.z());
    let angle = line_angle(&n, &axis);
    let kind = GeomKind::Surface(crate::SurfaceKind::EllipticCylinder);
    if angle <= tol.angular {
        let t = n.dot(&(plane.origin() - cyl.origin())) / n.dot(&axis);
        let frame = cyl.with_origin(cyl.origin() + t * axis.into_inner());
        return Ok(SurfaceIntersection::Transversal(vec![Curve::Ellipse {
            frame,
            major_radius: a,
            minor_radius: b,
        }]));
    }
    let ruling_at = |u: f64| Curve::Line {
        origin: Surface::EllipticCylinder {
            frame: *cyl,
            major_radius: a,
            minor_radius: b,
        }
        .point(u, 0.0),
        direction: axis,
    };
    if FRAC_PI_2 - angle <= tol.angular {
        // The plane's normal in the section, and the section line's
        // offset from the centre along it.
        let local = cyl.vec_to_local(n.into_inner());
        let Some(m) = Vec2::new(local.x, local.y).try_normalize(0.0) else {
            return Err(frame_degenerate(kind));
        };
        let o = cyl.to_local(plane.origin());
        let d = m.dot(&Vec2::new(o.x, o.y));
        let reach = (a * m.x).hypot(b * m.y);
        let phase = (b * m.y).atan2(a * m.x);
        if (d.abs() - reach).abs() <= tol.linear {
            let u = if d >= 0.0 {
                phase
            } else {
                phase + core::f64::consts::PI
            };
            return Ok(SurfaceIntersection::Tangent(vec![ruling_at(u)]));
        }
        if d.abs() >= reach {
            return Ok(SurfaceIntersection::Empty);
        }
        let half = (d / reach).clamp(-1.0, 1.0).acos();
        let Some(across) = UnitVec3::try_new(n.cross(&axis), 0.0) else {
            return Err(frame_degenerate(kind));
        };
        let mut rulings = [ruling_at(phase - half), ruling_at(phase + half)];
        let offset = |c: &Curve| match c {
            Curve::Line { origin, .. } => (origin - cyl.origin()).dot(&across),
            _ => 0.0,
        };
        if offset(&rulings[0]) > offset(&rulings[1]) {
            rulings.swap(0, 1);
        }
        return Ok(SurfaceIntersection::Transversal(rulings.to_vec()));
    }
    // Oblique: the section's semi-diameters slid along the axis into the
    // plane are conjugate semi-diameters of the section ellipse.
    let nz = n.dot(&axis);
    let t = n.dot(&(plane.origin() - cyl.origin())) / nz;
    let centre: Point3 = cyl.origin() + t * axis.into_inner();
    let z: Vec3 = axis.into_inner();
    let slide = |w: Vec3| w - (n.dot(&w) / nz) * z;
    let big = a * slide(cyl.x().into_inner());
    let small = b * slide(cyl.y().into_inner());
    let Some(e1) = UnitVec3::try_new(z - nz * n.into_inner(), 0.0) else {
        return Err(frame_degenerate(kind));
    };
    let e2 = n.cross(&e1);
    let (major, minor, phi) = principal_axes(
        Vec2::new(big.dot(&e1), big.dot(&e2)),
        Vec2::new(small.dot(&e1), small.dot(&e2)),
    );
    let x = phi.cos() * e1.into_inner() + phi.sin() * e2;
    let frame = Frame::new(centre, n.into_inner(), x).map_err(|_| frame_degenerate(kind))?;
    Ok(SurfaceIntersection::Transversal(vec![Curve::Ellipse {
        frame,
        major_radius: major,
        minor_radius: minor.abs(),
    }]))
}

/// An elliptic cylinder against a cylinder or another elliptic cylinder,
/// each given as its frame and `[a, b]` (the radius twice for a
/// cylinder). Axes parallel within `tol.angular`: the pair meets where
/// the two sections meet in the plane across the first's axis through
/// its origin (`crate::conic2`), each meeting a ruling along the first's
/// `Z` from the section point — `Coincident`, `Empty`, `Tangent` rulings
/// at the touches or `Transversal` rulings at the crossings, ascending by
/// the first section's parameter; touches beside crossings would mix
/// kinds and are `Unsupported`, as are axes that are not parallel (a
/// quartic space curve, C3's).
fn elliptic_pair(
    a: &Surface,
    b: &Surface,
    (ca, [aa, ab]): (&Frame, [f64; 2]),
    (cb, [ba, bb]): (&Frame, [f64; 2]),
    tol: Tolerance,
) -> Result<SurfaceIntersection, GeomError> {
    let unsupported = || GeomError::Unsupported {
        a: GeomKind::Surface(a.kind()),
        b: GeomKind::Surface(b.kind()),
    };
    if line_angle(&ca.z(), &cb.z()) > tol.angular {
        return Err(unsupported());
    }
    let first = Conic2 {
        centre: Point2::origin(),
        x: Vec2::x(),
        y: Vec2::y(),
        a: aa,
        b: ab,
    };
    let centre = ca.to_local(cb.origin());
    let xb = ca.vec_to_local(cb.x().into_inner());
    // The second's major axis in the section; a circle takes the
    // section's own `u` axis, its implicit form being the same either way.
    let x = if (ba - bb).abs() <= tol.linear {
        Vec2::x()
    } else {
        Vec2::new(xb.x, xb.y)
            .try_normalize(0.0)
            .ok_or_else(|| frame_degenerate(GeomKind::Surface(b.kind())))?
    };
    let second = Conic2 {
        centre: Point2::new(centre.x, centre.y),
        x,
        y: Vec2::new(-x.y, x.x),
        a: ba,
        b: bb,
    };
    let meet = conic_pair(&first, &second, tol).map_err(|e| GeomError::Degenerate {
        kind: GeomKind::Surface(a.kind()),
        reason: format!("the sections' meeting: {e}"),
    })?;
    let ruling = |t: f64| {
        let p = first.point(t);
        Curve::Line {
            origin: ca.to_world(Point3::new(p.x, p.y, 0.0)),
            direction: ca.z(),
        }
    };
    Ok(match meet {
        ConicMeet::Coincident => SurfaceIntersection::Coincident,
        ConicMeet::Empty => SurfaceIntersection::Empty,
        ConicMeet::Meets(meets) => {
            let touches = meets.iter().filter(|&&(_, touch)| touch).count();
            let rulings: Vec<Curve> = meets.iter().map(|&(t, _)| ruling(t)).collect();
            if touches == meets.len() {
                SurfaceIntersection::Tangent(rulings)
            } else if touches == 0 {
                SurfaceIntersection::Transversal(rulings)
            } else {
                return Err(unsupported());
            }
        }
    })
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

    fn elliptic(origin: Point3, a: f64, b: f64) -> Surface {
        Surface::EllipticCylinder {
            frame: Frame::from_z(origin, Vec3::z()).unwrap(),
            major_radius: a,
            minor_radius: b,
        }
    }

    /// Every point of `curve` over a turn or a unit of parameter lies on
    /// both surfaces to rounding, by their projections.
    fn on_both(curve: &Curve, a: &Surface, b: &Surface) {
        for i in 0..=32 {
            let t = match curve {
                Curve::Line { .. } => i as f64 / 32.0 * 4.0 - 2.0,
                _ => i as f64 / 32.0 * core::f64::consts::TAU,
            };
            let p = curve.point(t);
            for s in [a, b] {
                let d = s.project(p).unwrap().distance;
                assert!(d < 1e-12, "{p} is {d} off {:?}", s.kind());
            }
        }
    }

    #[test]
    fn a_plane_cuts_an_elliptic_cylinder_in_every_pose() {
        let wall = elliptic(Point3::origin(), 3.0, 2.0);
        // Across the axis: the section.
        let cap = Surface::Plane {
            frame: Frame::from_z(Point3::new(1.0, 1.0, 5.0), Vec3::z()).unwrap(),
        };
        let SurfaceIntersection::Transversal(c) = intersect_surfaces(&cap, &wall, tol()).unwrap()
        else {
            panic!()
        };
        let Curve::Ellipse {
            frame,
            major_radius,
            minor_radius,
        } = &c[0]
        else {
            panic!("{c:?}")
        };
        assert_eq!(frame.origin(), Point3::new(0.0, 0.0, 5.0));
        assert_eq!((*major_radius, *minor_radius), (3.0, 2.0));
        assert_eq!(frame.x().into_inner(), Vec3::x());
        // Along the axis: two rulings at x = 1 (y = ±2√(8/9)), a tangent
        // one at x = 3, none at x = 4 — and the same by the reversed order.
        let side = |x: f64| Surface::Plane {
            frame: Frame::from_z(Point3::new(x, 0.0, 0.0), Vec3::x()).unwrap(),
        };
        let SurfaceIntersection::Transversal(c) =
            intersect_surfaces(&wall, &side(1.0), tol()).unwrap()
        else {
            panic!()
        };
        assert_eq!(c.len(), 2);
        let y = 2.0 * (8.0f64 / 9.0).sqrt();
        let (Curve::Line { origin: o0, .. }, Curve::Line { origin: o1, .. }) = (&c[0], &c[1])
        else {
            panic!("{c:?}")
        };
        // Ordered along `n × Z = −y`, negative first: the +y ruling leads.
        assert!((o0 - Point3::new(1.0, y, 0.0)).norm() < 1e-12, "{c:?}");
        assert!((o1 - Point3::new(1.0, -y, 0.0)).norm() < 1e-12, "{c:?}");
        for r in &c {
            on_both(r, &wall, &side(1.0));
        }
        let SurfaceIntersection::Tangent(c) = intersect_surfaces(&side(3.0), &wall, tol()).unwrap()
        else {
            panic!()
        };
        let Curve::Line { origin, .. } = &c[0] else {
            panic!()
        };
        assert!((origin - Point3::new(3.0, 0.0, 0.0)).norm() < 1e-12);
        assert_eq!(
            intersect_surfaces(&side(4.0), &wall, tol()).unwrap(),
            SurfaceIntersection::Empty
        );
        // Oblique: an ellipse on both surfaces.
        let tilted = Surface::Plane {
            frame: Frame::from_z(Point3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 2.0, 3.0)).unwrap(),
        };
        let SurfaceIntersection::Transversal(c) =
            intersect_surfaces(&tilted, &wall, tol()).unwrap()
        else {
            panic!()
        };
        assert!(matches!(c[0], Curve::Ellipse { .. }), "{c:?}");
        on_both(&c[0], &tilted, &wall);
        assert_eq!(
            intersect_surfaces(&wall, &tilted, tol()).unwrap(),
            SurfaceIntersection::Transversal(c)
        );
    }

    #[test]
    fn parallel_elliptic_cylinders_meet_along_rulings() {
        let wall = elliptic(Point3::origin(), 3.0, 2.0);
        // A coaxial cylinder between the radii crosses the section at
        // four parameters; of the major radius it touches at two.
        let bore = |r: f64| Surface::Cylinder {
            frame: Frame::world(),
            radius: r,
        };
        let SurfaceIntersection::Transversal(c) =
            intersect_surfaces(&wall, &bore(2.5), tol()).unwrap()
        else {
            panic!()
        };
        assert_eq!(c.len(), 4);
        for r in &c {
            on_both(r, &wall, &bore(2.5));
        }
        let SurfaceIntersection::Tangent(c) = intersect_surfaces(&bore(3.0), &wall, tol()).unwrap()
        else {
            panic!()
        };
        assert_eq!(c.len(), 2);
        assert_eq!(
            intersect_surfaces(&bore(1.0), &wall, tol()).unwrap(),
            SurfaceIntersection::Empty
        );
        assert_eq!(
            intersect_surfaces(&wall, &wall, tol()).unwrap(),
            SurfaceIntersection::Coincident
        );
        // Two equal elliptic cylinders offset along the major axis: two
        // rulings at x = 1, y = ±2√(8/9), like the slot's two ends.
        let other = elliptic(Point3::new(2.0, 0.0, 7.0), 3.0, 2.0);
        let SurfaceIntersection::Transversal(c) = intersect_surfaces(&wall, &other, tol()).unwrap()
        else {
            panic!()
        };
        assert_eq!(c.len(), 2);
        for r in &c {
            on_both(r, &wall, &other);
        }
        // A crossing axis has no closed form; a touch beside a crossing
        // mixes kinds.
        let crossing = Surface::Cylinder {
            frame: Frame::from_z(Point3::origin(), Vec3::x()).unwrap(),
            radius: 1.0,
        };
        assert!(matches!(
            intersect_surfaces(&wall, &crossing, tol()),
            Err(GeomError::Unsupported { .. })
        ));
        let mixed = Surface::Cylinder {
            frame: Frame::from_z(Point3::new(1.0, 0.0, 0.0), Vec3::z()).unwrap(),
            radius: 2.0,
        };
        assert!(matches!(
            intersect_surfaces(&wall, &mixed, tol()),
            Err(GeomError::Unsupported { .. })
        ));
    }
}
