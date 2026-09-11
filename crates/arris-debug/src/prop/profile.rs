//! Strategies for random sketches: a star polygon with arcs and holes in
//! a random plane pose, in either orientation.
//!
//! Every profile a strategy here produces is a *valid* one —
//! `Profile::edges` accepts it — so a property over sweeps never has to
//! discard a case. That is bought with the shape of the generator rather
//! than with a filter: the outer loop's vertices sit at strictly
//! increasing angles about a centre, which makes the polygon star-shaped
//! and so simple, and an arc replaces a chord by a shallow bulge *outward*
//! whose sagitta stays a small fraction of the chord, which keeps it
//! inside the angular sector the chord spans and keeps the region
//! star-shaped. The holes are placed inside the largest disc about the
//! centre that the chords leave free.

use arris_geom::profile::{Profile, ProfileLoop, ProfileSegment};
use arris_math::{Point2, Vec2};
use proptest::prelude::*;

use super::{finite_f64, frame};

/// The radius the outer loop's vertices are drawn within. Far below
/// [`super::DEFAULT_SCALE`], so a profile in a random pose stays in the box the
/// other strategies place their geometry in.
pub const PROFILE_RADIUS: f64 = 10.0;

/// Vertices of the outer loop: enough for a star, few enough that its
/// notches stay shallow (see [`JITTER`]).
pub const VERTICES: core::ops::RangeInclusive<usize> = 5..=8;

/// How far a vertex's angle may stray from its even share of the turn, as
/// a fraction of that share: at 0.2 two neighbours are never closer than
/// 0.6 of the even spacing, which bounds how sharp a notch between two
/// far vertices can get — and with it how much two outward arcs meeting
/// there may turn towards each other.
pub const JITTER: f64 = 0.2;

/// A vertex's radius, as a fraction of [`PROFILE_RADIUS`]: the notches
/// are deep enough to make the polygon non-convex and shallow enough to
/// keep every interior angle well away from a full turn.
pub const RADIUS_FRACTION: core::ops::RangeInclusive<f64> = 0.6..=1.0;

/// An arc's sagitta as a fraction of its chord. Bounded above so the arc
/// stays inside the sector its chord spans; bounded below so the arc is
/// never within rounding of its chord.
pub const SAGITTA_FRACTION: core::ops::RangeInclusive<f64> = 0.03..=0.12;

/// A hole's radius and the distance of its centre from the profile's
/// centre, as fractions of the largest disc about the centre the outer
/// loop's chords leave free: together below one, so a hole is inside the
/// outer loop, and far enough apart that two holes never meet.
const HOLE_RADIUS_FRACTION: f64 = 0.12;
const HOLE_OFFSET_FRACTION: f64 = 0.45;

/// The raw draw of [`star`], before it is turned into a [`Profile`].
type Draw = (
    arris_math::Frame,
    Vec<(f64, f64)>,
    Vec<Option<f64>>,
    usize,
    Vec<bool>,
);

/// A star polygon in a random plane pose: `VERTICES` vertices at strictly
/// increasing angles, some chords replaced by outward arcs, with zero,
/// one or two holes — a circle and a triangle, so both loop kinds and
/// both `ProfileLoop` variants appear — and every loop written in a random
/// orientation, since a profile carries none.
pub fn star() -> impl Strategy<Value = Profile> {
    let vertices = VERTICES;
    (
        frame(),
        proptest::collection::vec(
            (finite_f64(-JITTER..=JITTER), finite_f64(RADIUS_FRACTION)),
            vertices.clone(),
        ),
        proptest::collection::vec(proptest::option::of(finite_f64(SAGITTA_FRACTION)), vertices),
        0usize..=2,
        proptest::collection::vec(any::<bool>(), 2),
    )
        .prop_map(build)
}

fn build((plane, vertices, arcs, holes, flipped): Draw) -> Profile {
    let n = vertices.len().min(arcs.len());
    let centre = Vec2::new(0.0, 0.0);
    let share = core::f64::consts::TAU / n as f64;
    let points: Vec<Point2> = (0..n)
        .map(|k| {
            let (jitter, fraction) = vertices[k];
            let angle = (k as f64 + jitter) * share;
            let r = fraction * PROFILE_RADIUS;
            Point2::new(centre.x + r * angle.cos(), centre.y + r * angle.sin())
        })
        .collect();
    // The sagitta of each chord, positive outward from the centre.
    let vias: Vec<Option<Point2>> = (0..n)
        .map(|k| {
            let (a, b) = (points[k], points[(k + 1) % n]);
            let fraction = arcs[k]?;
            let chord = b - a;
            let mid = a + chord / 2.0;
            let outward = mid - Point2::from(centre);
            let outward = outward / outward.norm();
            Some(mid + fraction * chord.norm() * outward)
        })
        .collect();
    // The largest disc about the centre the chords leave free.
    let free = (0..n)
        .map(|k| distance_to_segment(Point2::from(centre), points[k], points[(k + 1) % n]))
        .fold(f64::INFINITY, f64::min);
    let outer = path_loop(&points, &vias, flipped[0]);
    let mut loops = Vec::new();
    if holes >= 1 {
        loops.push(ProfileLoop::Circle {
            center: Point2::from(centre + Vec2::new(HOLE_OFFSET_FRACTION * free, 0.0)),
            radius: HOLE_RADIUS_FRACTION * free,
        });
    }
    if holes >= 2 {
        let c = centre + Vec2::new(-HOLE_OFFSET_FRACTION * free, 0.0);
        let r = HOLE_RADIUS_FRACTION * free;
        let corner = |i: usize| {
            let a = core::f64::consts::TAU * i as f64 / 3.0;
            Point2::from(c + r * Vec2::new(a.cos(), a.sin()))
        };
        let triangle: Vec<Point2> = (0..3).map(corner).collect();
        loops.push(path_loop(&triangle, &[None, None, None], flipped[1]));
    }
    Profile {
        plane,
        outer,
        holes: loops,
    }
}

/// The loop through `points`, segment `k` running from `points[k]` to the
/// next and bulging through `vias[k]` when there is one, written forwards
/// or backwards.
fn path_loop(points: &[Point2], vias: &[Option<Point2>], backwards: bool) -> ProfileLoop {
    let n = points.len();
    let segment = |to: usize, via: Option<Point2>| match via {
        None => ProfileSegment::LineTo(points[to]),
        Some(via) => ProfileSegment::ArcTo {
            to: points[to],
            via,
        },
    };
    if backwards {
        let start = points[0];
        let segments = (0..n)
            .map(|i| {
                // Backwards, the i-th step goes from point n−i to n−i−1,
                // which is the consumer's segment n−i−1 walked the other
                // way; its via is that segment's.
                let to = (n - i + n - 1) % n;
                segment(to, vias[to])
            })
            .collect();
        ProfileLoop::Path { start, segments }
    } else {
        let segments = (0..n).map(|k| segment((k + 1) % n, vias[k])).collect();
        ProfileLoop::Path {
            start: points[0],
            segments,
        }
    }
}

/// The distance from `p` to the segment `a`–`b`.
fn distance_to_segment(p: Point2, a: Point2, b: Point2) -> f64 {
    let d = b - a;
    let len2 = d.norm_squared();
    if len2 <= 0.0 {
        return (p - a).norm();
    }
    let t = ((p - a).dot(&d) / len2).clamp(0.0, 1.0);
    (p - (a + t * d)).norm()
}
