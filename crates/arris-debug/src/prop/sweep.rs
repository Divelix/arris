//! Pappus's centroid theorems as the oracle of the sweeps
//! (`docs/plans/m5-sweeps.md`): the volume and the area a revolve must
//! have, computed in the profile's plane and never over the swept faces
//! — an independent path from `measure`'s flux over the B-Rep.
//!
//! A revolve by `θ` of a region of area `A` whose centroid is `ρ̄` from
//! the axis has volume `θ ρ̄ A`, and its swept surface has area `θ ∮ ρ dℓ`
//! over the region's boundary, plus `2A` for the two flat ends of a
//! partial turn. `ρ̄ A` is `∬ ρ dA`, taken by `integrate::region_integral`
//! over the profile's oriented edges; the boundary integral is
//! Gauss–Legendre over each edge's pcurve in pieces of at most a quarter
//! turn, exact for a line and to rounding for an arc.

use core::f64::consts::{FRAC_PI_2, TAU};

use arris_geom::integrate::{gauss_legendre, region_integral};
use arris_geom::profile::{Profile, ProfileError};
use arris_geom::region2::Piece;
use arris_math::{Axis, Point2, Tolerance, Vec2};

/// What Pappus says a sweep measures.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pappus {
    /// The enclosed volume.
    pub volume: f64,
    /// The total face area, the flat ends included.
    pub area: f64,
}

/// The volume and area of `profile` revolved about `axis` by `angle`,
/// with `angle` within `tol.angular` of `2π` a full turn (no flat ends).
/// The axis is read in the profile's plane by projection; the sign of the
/// distance from it is immaterial, since every integral is taken in
/// absolute value.
///
/// Errors: as `Profile::edges`.
///
/// ```
/// use arris_debug::prop::sweep::revolved;
/// use arris_geom::profile::{Profile, ProfileLoop};
/// use arris_math::{Axis, Frame, Point2, Point3, Precision};
/// use core::f64::consts::{PI, TAU};
///
/// // A disc of radius 1 centred 3 from the axis: a ring, 2π²·3·1² and 4π²·3·1.
/// let disc = Profile {
///     plane: Frame::world(),
///     outer: ProfileLoop::Circle { center: Point2::new(3.0, 0.0), radius: 1.0 },
///     holes: Vec::new(),
/// };
/// let axis = Axis { origin: Point3::origin(), direction: arris_math::Vec3::y_axis() };
/// let p = revolved(&disc, &axis, TAU, Precision::DEFAULT.tolerance()).unwrap();
/// assert!((p.volume - 6.0 * PI * PI).abs() < 1e-12 * p.volume);
/// assert!((p.area - 12.0 * PI * PI).abs() < 1e-12 * p.area);
/// ```
pub fn revolved(
    profile: &Profile,
    axis: &Axis,
    angle: f64,
    tol: Tolerance,
) -> Result<Pappus, ProfileError> {
    let loops = profile.edges(tol)?;
    let o = profile.plane.to_local(axis.origin);
    let origin = Point2::new(o.x, o.y);
    let a = profile.plane.vec_to_local(axis.direction.into_inner());
    let along = Vec2::new(a.x, a.y).normalize();
    let radial = Vec2::new(-along.y, along.x);
    let rho = |p: Point2| (p - origin).dot(&radial);

    let pieces: Vec<Piece<'_>> = loops
        .iter()
        .flatten()
        .map(|e| Piece::along(&e.pcurve, e.range))
        .collect();
    let area = region_integral(&pieces, f64::INFINITY, |_, _| 1.0);
    let moment = region_integral(&pieces, f64::INFINITY, |u, v| rho(Point2::new(u, v)));

    let nodes = gauss_legendre();
    let mut boundary = 0.0;
    for e in loops.iter().flatten() {
        let (lo, hi) = (e.range.lo(), e.range.hi());
        let steps = ((hi - lo) / FRAC_PI_2).ceil().max(1.0) as usize;
        let h = (hi - lo) / steps as f64;
        for i in 0..steps {
            let (a, b) = (lo + i as f64 * h, lo + (i + 1) as f64 * h);
            let (mid, half) = ((a + b) / 2.0, (b - a) / 2.0);
            for &(x, w) in &nodes {
                let ev = e.pcurve.eval(mid + half * x);
                boundary += w * half * rho(ev.point).abs() * ev.d1.norm();
            }
        }
    }
    let full = (angle - TAU).abs() <= tol.angular;
    let ends = if full { 0.0 } else { 2.0 * area.abs() };
    Ok(Pappus {
        volume: angle * moment.abs(),
        area: angle * boundary + ends,
    })
}
