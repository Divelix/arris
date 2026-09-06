//! Regions of a surface's (u, v) plane bounded by pcurve pieces
//! (`docs/02-data-model.md` §Pcurves): a loop discretised to a polygon,
//! its signed area and winding number, and segment intersections over
//! exact predicates. Shared by the checker's loop rows, tessellation and
//! classification; nothing here knows about tolerances of the model
//! beyond the chord tolerance the caller passes.

use core::f64::consts::FRAC_PI_2;

use arris_math::predicates::{Sign, orient2d};
use arris_math::{Interval, Point2};

use crate::Curve2;

/// The fewest segments a full turn of a conic pcurve is cut into,
/// whatever the chord tolerance: eight, so that a circle's polygon has the
/// circle's turn and sign (two segments would have zero area) and a
/// quarter turn is never one straight chord.
pub const MIN_SEGMENTS_PER_TURN: usize = 8;

/// The fewest segments per knot span of a NURBS pcurve: two, so a span
/// that bends back on itself still turns the polygon.
pub const MIN_SEGMENTS_PER_SPAN: usize = 2;

/// The most segments one piece is cut into. A chord tolerance finer than
/// this resolves is reported through [`Polygon2::chord_deviation`], never
/// met by an unbounded polygon.
pub const MAX_SEGMENTS_PER_PIECE: usize = 1 << 16;

/// Parameters at which a NURBS piece's second derivative is sampled per
/// span to bound its chord deviation; `4p + 4` per span is where the fit
/// checks itself too, and a quintic's second derivative cannot hide
/// between twenty-four samples.
const CURVATURE_SAMPLES_PER_SPAN: usize = 24;

/// One piece of a region's boundary: a pcurve over a parameter range,
/// walked along its parameter or against it. A loop is a sequence of
/// pieces whose ends meet; a seam-crossing loop is written with its two
/// seam pieces a period apart, never unwrapped.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Piece<'a> {
    /// The pcurve.
    pub curve: &'a Curve2,
    /// The parameter range walked, a sub-interval of the pcurve's domain.
    pub range: Interval,
    /// `true` to walk from `range.hi()` down to `range.lo()`.
    pub reversed: bool,
}

impl<'a> Piece<'a> {
    /// A piece of `curve` over `range`, along the parameter.
    pub const fn along(curve: &'a Curve2, range: Interval) -> Self {
        Piece {
            curve,
            range,
            reversed: false,
        }
    }

    /// A piece of `curve` over `range`, against the parameter.
    pub const fn against(curve: &'a Curve2, range: Interval) -> Self {
        Piece {
            curve,
            range,
            reversed: true,
        }
    }

    /// The point where the walk starts.
    pub fn first(&self) -> Point2 {
        self.curve.point(if self.reversed {
            self.range.hi()
        } else {
            self.range.lo()
        })
    }

    /// The point where the walk ends.
    pub fn last(&self) -> Point2 {
        self.curve.point(if self.reversed {
            self.range.lo()
        } else {
            self.range.hi()
        })
    }

    /// The interior knots of a NURBS piece within its range, ascending:
    /// where its polygon and its integration are split. Empty for the
    /// analytic variants.
    pub fn interior_knots(&self) -> Vec<f64> {
        let Curve2::Nurbs(n) = self.curve else {
            return Vec::new();
        };
        let mut knots: Vec<f64> = n
            .knots()
            .iter()
            .copied()
            .filter(|&k| self.range.lo() < k && k < self.range.hi())
            .collect();
        knots.dedup();
        knots
    }

    /// How many straight segments approximate this piece within
    /// `chord_tolerance` in (u, v), never below the minimum counts and
    /// never above [`MAX_SEGMENTS_PER_PIECE`]; `f64::INFINITY` asks for the
    /// minimum counts alone.
    pub fn segment_count(&self, chord_tolerance: f64) -> usize {
        let length = self.range.length();
        if !(length.is_finite() && length > 0.0) {
            return 1;
        }
        // A chord over a parameter step `h` deviates at most `|d2| h² / 8`
        // from a curve whose second derivative is bounded by `|d2|`.
        let (minimum, d2) = match self.curve {
            Curve2::Line { .. } => return 1,
            Curve2::Circle { radius, .. } => (per_turn(length), radius.abs()),
            Curve2::Ellipse { major_radius, .. } => (per_turn(length), major_radius.abs()),
            Curve2::Nurbs(n) => {
                let spans = self.interior_knots().len() + 1;
                let samples = spans * CURVATURE_SAMPLES_PER_SPAN;
                let d2 = (0..=samples)
                    .map(|i| n.eval(self.range.lerp(i as f64 / samples as f64)).d2.norm())
                    .fold(0.0, f64::max);
                (spans * MIN_SEGMENTS_PER_SPAN, d2)
            }
        };
        let from_tolerance = if chord_tolerance.is_finite() && chord_tolerance > 0.0 && d2 > 0.0 {
            let h = (8.0 * chord_tolerance / d2).sqrt();
            (length / h).ceil()
        } else {
            0.0
        };
        let wanted = if from_tolerance.is_finite() {
            from_tolerance as usize
        } else {
            MAX_SEGMENTS_PER_PIECE
        };
        wanted.max(minimum).min(MAX_SEGMENTS_PER_PIECE)
    }

    /// The bound on the distance between this piece and its polygon of
    /// `segments` segments: `|d2| h² / 8`; zero for a line.
    fn chord_deviation(&self, segments: usize) -> f64 {
        let d2 = match self.curve {
            Curve2::Line { .. } => return 0.0,
            Curve2::Circle { radius, .. } => radius.abs(),
            Curve2::Ellipse { major_radius, .. } => major_radius.abs(),
            Curve2::Nurbs(n) => {
                let spans = self.interior_knots().len() + 1;
                let samples = spans * CURVATURE_SAMPLES_PER_SPAN;
                (0..=samples)
                    .map(|i| n.eval(self.range.lerp(i as f64 / samples as f64)).d2.norm())
                    .fold(0.0, f64::max)
            }
        };
        let h = self.range.length() / segments as f64;
        d2 * h * h / 8.0
    }

    /// `segments + 1` points along the walk, both ends included.
    pub fn sample(&self, segments: usize) -> Vec<Point2> {
        let segments = segments.max(1);
        (0..=segments)
            .map(|i| {
                let s = i as f64 / segments as f64;
                let s = if self.reversed { 1.0 - s } else { s };
                self.curve.point(self.range.lerp(s))
            })
            .collect()
    }
}

/// The minimum segments for `length` radians of a conic.
fn per_turn(length: f64) -> usize {
    let quarter_turns = (length / FRAC_PI_2).ceil().max(1.0);
    // Never below the per-turn minimum's share of the arc, and never
    // fewer than the quarter turns it spans.
    let share = (length / core::f64::consts::TAU * MIN_SEGMENTS_PER_TURN as f64).ceil();
    (share.max(quarter_turns)) as usize
}

/// A closed polygon in (u, v): the discretisation of a loop's pieces in
/// walking order, with the junctions between pieces remembered so a
/// caller can ask how far consecutive pieces are from meeting.
#[derive(Debug, Clone, PartialEq)]
pub struct Polygon2 {
    /// The ring of vertices, consecutive duplicates removed, the closing
    /// edge implied.
    points: Vec<Point2>,
    /// Per piece, the first and last point of its walk before dedup.
    junctions: Vec<(Point2, Point2)>,
    chord_deviation: f64,
}

/// The polygon of `pieces` within `chord_tolerance`: each piece sampled
/// at [`Piece::segment_count`] segments, walked in its direction, and the
/// samples chained into one ring. A loop is closed by definition, so
/// consecutive pieces are joined at the earlier piece's last point (the
/// next piece's first sample is not a vertex) and the last piece closes
/// onto the first; how far the pieces were from meeting is reported by
/// [`Polygon2::gaps`], never drawn as a rounding-length segment that a
/// self-intersection test would trip over. A point equal to its
/// predecessor is dropped, so every segment has length.
///
/// ```
/// use arris_geom::region2::{Piece, discretise};
/// use arris_geom::Curve2;
/// use arris_math::{Frame2, Interval};
/// use core::f64::consts::PI;
///
/// let circle = Curve2::Circle { frame: Frame2::identity(), radius: 2.0 };
/// let polygon = discretise(&[Piece::along(&circle, Interval::TURN)], 1e-3);
/// let area = polygon.signed_area();
/// assert!(area > 0.0 && (area - 4.0 * PI).abs() < 2.0 * PI * 2.0 * 1e-3);
/// assert!(polygon.chord_deviation() <= 1e-3);
/// ```
pub fn discretise(pieces: &[Piece<'_>], chord_tolerance: f64) -> Polygon2 {
    let mut points: Vec<Point2> = Vec::new();
    let mut junctions = Vec::with_capacity(pieces.len());
    let mut deviation: f64 = 0.0;
    for (k, piece) in pieces.iter().enumerate() {
        let segments = piece.segment_count(chord_tolerance);
        deviation = deviation.max(piece.chord_deviation(segments));
        let samples = piece.sample(segments);
        if let (Some(&first), Some(&last)) = (samples.first(), samples.last()) {
            junctions.push((first, last));
        }
        let skip = usize::from(k > 0);
        for p in samples.into_iter().skip(skip) {
            if points.last() != Some(&p) {
                points.push(p);
            }
        }
    }
    // The last sample is the closure onto the first point.
    if points.len() > 1 {
        points.pop();
    }
    Polygon2 {
        points,
        junctions,
        chord_deviation: deviation,
    }
}

impl Polygon2 {
    /// The ring of vertices in walking order, the closing edge implied.
    pub fn points(&self) -> &[Point2] {
        &self.points
    }

    /// The bound on the distance between the pieces and this polygon.
    pub fn chord_deviation(&self) -> f64 {
        self.chord_deviation
    }

    /// The (u, v) distance from each piece's last point to the next
    /// piece's first point, the last piece closing onto the first: zero
    /// everywhere for a loop whose pieces meet exactly.
    pub fn gaps(&self) -> Vec<f64> {
        let n = self.junctions.len();
        (0..n)
            .map(|i| (self.junctions[(i + 1) % n].0 - self.junctions[i].1).norm())
            .collect()
    }

    /// The segments `(a, b)` of the ring, in order, each of positive
    /// length.
    pub fn segments(&self) -> impl Iterator<Item = (Point2, Point2)> + '_ {
        let n = self.points.len();
        (0..n)
            .map(move |i| (self.points[i], self.points[(i + 1) % n]))
            .filter(|(a, b)| a != b)
    }

    /// The shoelace area: positive for a counter-clockwise ring in (u, v),
    /// negative for a clockwise one.
    pub fn signed_area(&self) -> f64 {
        let mut twice = 0.0;
        for (a, b) in self.segments() {
            twice += a.x * b.y - b.x * a.y;
        }
        twice / 2.0
    }

    /// How many times the ring winds around `p`, counter-clockwise
    /// positive, by upward and downward crossings of the ray from `p`
    /// along `+u` decided with `orient2d`: zero outside the ring, `+1`
    /// inside a counter-clockwise loop, `−1` inside a clockwise one. A
    /// point on the ring is on a segment where the predicate returns
    /// zero and is counted as neither side; ask [`Polygon2::contains`]
    /// when that matters.
    pub fn winding_number(&self, p: Point2) -> i32 {
        let mut winding = 0;
        for (a, b) in self.segments() {
            if a.y <= p.y {
                if b.y > p.y && orient2d(a, b, p) == Sign::Positive {
                    winding += 1;
                }
            } else if b.y <= p.y && orient2d(a, b, p) == Sign::Negative {
                winding -= 1;
            }
        }
        winding
    }

    /// `true` when `p` lies on a segment of the ring, exactly.
    pub fn contains(&self, p: Point2) -> bool {
        self.segments().any(|(a, b)| on_segment(a, b, p))
    }

    /// Pairs of segment indices (into [`Polygon2::segments`]) of this ring
    /// that meet, non-adjacent ones only: a simple polygon reports none.
    /// Quadratic in the segment count.
    pub fn self_intersections(&self) -> Vec<(usize, usize)> {
        let segments: Vec<_> = self.segments().collect();
        let n = segments.len();
        let mut hits = Vec::new();
        for i in 0..n {
            for j in i + 1..n {
                let adjacent = j == i + 1 || (i == 0 && j == n - 1);
                if adjacent {
                    continue;
                }
                let (a0, a1) = segments[i];
                let (b0, b1) = segments[j];
                if segments_intersect(a0, a1, b0, b1) {
                    hits.push((i, j));
                }
            }
        }
        hits
    }

    /// Pairs `(i, j)` of a segment of this ring and a segment of `other`
    /// that meet, touching included. Quadratic in the segment counts.
    pub fn intersections(&self, other: &Polygon2) -> Vec<(usize, usize)> {
        let mine: Vec<_> = self.segments().collect();
        let theirs: Vec<_> = other.segments().collect();
        let mut hits = Vec::new();
        for (i, &(a0, a1)) in mine.iter().enumerate() {
            for (j, &(b0, b1)) in theirs.iter().enumerate() {
                if segments_intersect(a0, a1, b0, b1) {
                    hits.push((i, j));
                }
            }
        }
        hits
    }
}

/// `true` when `p` lies on the closed segment `ab`, exactly: collinear by
/// `orient2d` and within the bounding box.
fn on_segment(a: Point2, b: Point2, p: Point2) -> bool {
    orient2d(a, b, p) == Sign::Zero
        && p.x >= a.x.min(b.x)
        && p.x <= a.x.max(b.x)
        && p.y >= a.y.min(b.y)
        && p.y <= a.y.max(b.y)
}

/// `true` when the closed segments `a0a1` and `b0b1` share a point:
/// a proper crossing, an endpoint on the other segment, or collinear
/// overlap — decided exactly by `orient2d`, so two segments that touch
/// at a shared endpoint intersect.
///
/// ```
/// use arris_geom::region2::segments_intersect;
/// use arris_math::Point2;
///
/// let p = |x, y| Point2::new(x, y);
/// assert!(segments_intersect(p(0.0, 0.0), p(2.0, 2.0), p(0.0, 2.0), p(2.0, 0.0)));
/// assert!(segments_intersect(p(0.0, 0.0), p(2.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)));
/// assert!(!segments_intersect(p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0), p(1.0, 1.0)));
/// ```
pub fn segments_intersect(a0: Point2, a1: Point2, b0: Point2, b1: Point2) -> bool {
    let d1 = orient2d(b0, b1, a0);
    let d2 = orient2d(b0, b1, a1);
    let d3 = orient2d(a0, a1, b0);
    let d4 = orient2d(a0, a1, b1);
    let opposite = |x: Sign, y: Sign| {
        matches!(
            (x, y),
            (Sign::Positive, Sign::Negative) | (Sign::Negative, Sign::Positive)
        )
    };
    if opposite(d1, d2) && opposite(d3, d4) {
        return true;
    }
    (d1 == Sign::Zero && on_segment(b0, b1, a0))
        || (d2 == Sign::Zero && on_segment(b0, b1, a1))
        || (d3 == Sign::Zero && on_segment(a0, a1, b0))
        || (d4 == Sign::Zero && on_segment(a0, a1, b1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arris_math::{Frame2, Vec2};

    fn square(side: f64) -> [Curve2; 4] {
        let line = |ox: f64, oy: f64, dx: f64, dy: f64| Curve2::Line {
            origin: Point2::new(ox, oy),
            direction: arris_math::UnitVec2::new_normalize(Vec2::new(dx, dy)),
        };
        [
            line(0.0, 0.0, 1.0, 0.0),
            line(side, 0.0, 0.0, 1.0),
            line(side, side, -1.0, 0.0),
            line(0.0, side, 0.0, -1.0),
        ]
    }

    #[test]
    fn a_square_of_four_lines_has_its_area_and_no_gaps() {
        let sides = square(3.0);
        let range = Interval::new(0.0, 3.0).unwrap();
        let pieces: Vec<Piece<'_>> = sides.iter().map(|c| Piece::along(c, range)).collect();
        let polygon = discretise(&pieces, 1e-6);
        assert_eq!(polygon.points().len(), 4);
        assert_eq!(polygon.signed_area(), 9.0);
        assert!(polygon.gaps().iter().all(|&g| g == 0.0));
        assert_eq!(polygon.winding_number(Point2::new(1.0, 1.0)), 1);
        assert_eq!(polygon.winding_number(Point2::new(4.0, 1.0)), 0);
        assert!(polygon.self_intersections().is_empty());
        assert!(polygon.contains(Point2::new(1.5, 0.0)));
        // Walked backwards: clockwise.
        let reversed: Vec<Piece<'_>> = sides
            .iter()
            .rev()
            .map(|c| Piece::against(c, range))
            .collect();
        let polygon = discretise(&reversed, 1e-6);
        assert_eq!(polygon.signed_area(), -9.0);
        assert_eq!(polygon.winding_number(Point2::new(1.0, 1.0)), -1);
    }

    #[test]
    fn a_circle_meets_the_minimum_count_and_the_chord_tolerance() {
        let circle = Curve2::Circle {
            frame: Frame2::identity(),
            radius: 10.0,
        };
        let piece = Piece::along(&circle, Interval::TURN);
        assert_eq!(piece.segment_count(f64::INFINITY), MIN_SEGMENTS_PER_TURN);
        let fine = piece.segment_count(1e-4);
        assert!(fine > 100, "{fine}");
        let polygon = discretise(&[piece], 1e-4);
        assert!(polygon.chord_deviation() <= 1e-4);
        assert_eq!(
            polygon.points().len(),
            fine,
            "the closing sample is the first point"
        );
        assert_eq!(piece.segment_count(1e-300), MAX_SEGMENTS_PER_PIECE);
    }

    #[test]
    fn a_bow_tie_intersects_itself() {
        let p = |x: f64, y: f64| Point2::new(x, y);
        let seg = |a: Point2, b: Point2| Curve2::Line {
            origin: a,
            direction: arris_math::UnitVec2::new_normalize(b - a),
        };
        let corners = [p(0.0, 0.0), p(2.0, 2.0), p(2.0, 0.0), p(0.0, 2.0)];
        let sides: Vec<Curve2> = (0..4)
            .map(|i| seg(corners[i], corners[(i + 1) % 4]))
            .collect();
        let pieces: Vec<Piece<'_>> = (0..4)
            .map(|i| {
                let len = (corners[(i + 1) % 4] - corners[i]).norm();
                Piece::along(&sides[i], Interval::new(0.0, len).unwrap())
            })
            .collect();
        let polygon = discretise(&pieces, 1e-6);
        assert_eq!(polygon.self_intersections(), [(0, 2)]);
    }
}
