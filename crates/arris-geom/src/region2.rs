//! Regions of a surface's (u, v) plane bounded by pcurve pieces
//! (`docs/DATA-MODEL.md` §Pcurves): a loop discretised to a polygon,
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
/// quarter turn is never one straight chord. An arc of any length takes
/// at least [`MIN_SEGMENTS_PER_ARC`].
pub const MIN_SEGMENTS_PER_TURN: usize = 8;

/// The fewest segments any conic arc is cut into, however short: two, so
/// the arc's midpoint is a point of the polygon and a loop of one arc and
/// one straight edge — a D, the piece a section leaves on a face it
/// barely crosses — has the area of its bulge instead of collapsing onto
/// the chord. `boolean/sliver-common` is the arc under an eighth of a
/// turn that one segment flattened.
pub const MIN_SEGMENTS_PER_ARC: usize = 2;

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
pub(crate) const CURVATURE_SAMPLES_PER_SPAN: usize = 24;

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

/// The minimum segments for `length` radians of a conic: the per-turn
/// minimum's share of the arc, never fewer than the quarter turns it
/// spans, never fewer than [`MIN_SEGMENTS_PER_ARC`].
pub(crate) fn per_turn(length: f64) -> usize {
    let quarter_turns = (length / FRAC_PI_2).ceil().max(1.0);
    let share = (length / core::f64::consts::TAU * MIN_SEGMENTS_PER_TURN as f64).ceil();
    (share.max(quarter_turns) as usize).max(MIN_SEGMENTS_PER_ARC)
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
    /// A polygon from a ring of points already in walking order: what a
    /// caller that sampled the pieces itself — tessellation, which samples
    /// each pcurve at the parameters its 3D edge was discretised at —
    /// builds instead of [`discretise`]. A point equal to its predecessor
    /// is dropped, and so is a last point equal to the first, so every
    /// segment has length; there are no junctions ([`Polygon2::gaps`] is
    /// empty) and the chord deviation is zero, since the points *are* the
    /// boundary.
    ///
    /// ```
    /// use arris_geom::region2::Polygon2;
    /// use arris_math::Point2;
    ///
    /// let p = |x, y| Point2::new(x, y);
    /// let square = Polygon2::from_points([p(0.0, 0.0), p(2.0, 0.0), p(2.0, 2.0), p(0.0, 2.0), p(0.0, 0.0)]);
    /// assert_eq!(square.points().len(), 4);
    /// assert_eq!(square.signed_area(), 4.0);
    /// ```
    pub fn from_points(points: impl IntoIterator<Item = Point2>) -> Polygon2 {
        let mut ring: Vec<Point2> = Vec::new();
        for p in points {
            if ring.last() != Some(&p) {
                ring.push(p);
            }
        }
        if ring.len() > 1 && ring.first() == ring.last() {
            ring.pop();
        }
        Polygon2 {
            points: ring,
            junctions: Vec::new(),
            chord_deviation: 0.0,
        }
    }

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
        self.segments().map(|(a, b)| crossing(a, b, p)).sum()
    }

    /// `true` when `p` lies on a segment of the ring, exactly.
    pub fn contains(&self, p: Point2) -> bool {
        self.segments().any(|(a, b)| on_segment(a, b, p))
    }

    /// Pairs of segment indices (into [`Polygon2::segments`]) of this ring
    /// that meet, non-adjacent ones only and ascending: a simple polygon
    /// reports none. Found by a sweep in `u` over the segments' spans, so
    /// a ring whose segments overlap few others — every ring a pcurve
    /// discretises to — costs `n log n`; the worst case is still every
    /// pair.
    pub fn self_intersections(&self) -> Vec<(usize, usize)> {
        let segments: Vec<_> = self.segments().collect();
        let n = segments.len();
        sweep(&segments, &segments, |i, j| {
            i < j && !(j == i + 1 || (i == 0 && j == n - 1))
        })
    }

    /// Pairs `(i, j)` of a segment of this ring and a segment of `other`
    /// that meet, touching included, ascending. The sweep of
    /// [`Polygon2::self_intersections`].
    pub fn intersections(&self, other: &Polygon2) -> Vec<(usize, usize)> {
        let mine: Vec<_> = self.segments().collect();
        let theirs: Vec<_> = other.segments().collect();
        sweep(&mine, &theirs, |_, _| true)
    }
}

/// Where a point lies with respect to a set of loop polygons: the answer
/// a face's own (u, v) gives about a point on its surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Strictly inside the region the polygons bound: the winding number
    /// is not zero and the point is clear of every segment.
    Inside,
    /// Strictly outside it.
    Outside,
    /// Within the boundary tolerance of a segment of some polygon.
    Boundary,
}

/// Where `p` lies with respect to the region `polygons` bound: within
/// `boundary_tolerance` of any segment is [`Side::Boundary`], and
/// otherwise the sum of the polygons' winding numbers decides — non-zero
/// is [`Side::Inside`] (an outer ring counter-clockwise and its holes
/// clockwise, as a stored loop is: `docs/DATA-MODEL.md` §Orientation).
/// No polygons is [`Side::Outside`].
///
/// The tolerance is a distance in the *parameter* plane and is the
/// caller's: the checker passes the model's parametric tolerance scaled
/// to the surface, a boolean the face's tolerance converted the same way.
/// Nothing here knows the model.
///
/// ```
/// use arris_geom::region2::{Polygon2, Side, point_side};
/// use arris_math::Point2;
///
/// let p = |x, y| Point2::new(x, y);
/// let square = Polygon2::from_points([p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)]);
/// let hole = Polygon2::from_points([p(1.0, 1.0), p(1.0, 2.0), p(2.0, 2.0), p(2.0, 1.0)]);
/// let region = [square, hole];
/// assert_eq!(point_side(&region, p(3.0, 3.0), 1e-9), Side::Inside);
/// assert_eq!(point_side(&region, p(1.5, 1.5), 1e-9), Side::Outside);
/// assert_eq!(point_side(&region, p(4.0, 2.0), 1e-9), Side::Boundary);
/// ```
pub fn point_side(polygons: &[Polygon2], p: Point2, boundary_tolerance: f64) -> Side {
    let mut winding = 0;
    for polygon in polygons {
        for (a, b) in polygon.segments() {
            if point_segment_distance(a, b, p) <= boundary_tolerance {
                return Side::Boundary;
            }
        }
        winding += polygon.winding_number(p);
    }
    if winding != 0 {
        Side::Inside
    } else {
        Side::Outside
    }
}

/// What the segment `a → b` adds to a winding number about `p`: `+1` for
/// an upward segment with `p` strictly on its left, `−1` for a downward
/// one with `p` strictly on its right, `0` otherwise — the crossing of the
/// horizontal ray from `p` toward increasing `u`, half-open in `v` so a
/// vertex on the ray is counted once.
fn crossing(a: Point2, b: Point2, p: Point2) -> i32 {
    if a.y <= p.y {
        i32::from(b.y > p.y && orient2d(a, b, p) == Sign::Positive)
    } else {
        -i32::from(b.y <= p.y && orient2d(a, b, p) == Sign::Negative)
    }
}

/// Segments per strip a [`SideIndex`] aims for: a tuning constant of the
/// index's cost, not a tolerance — any value gives the same answers.
const SEGMENTS_PER_STRIP: usize = 8;

/// How many units of `f64::EPSILON` times the region's `v` magnitude a
/// strip must be tall, at least, for the index to use more than one: the
/// rounding in [`point_segment_distance`] is a few such units, and one
/// neighbouring strip of slack on each side of a query then covers it
/// many times over. A region thinner than that is one strip, which is the
/// linear walk.
const STRIP_HEIGHT_IN_ROUNDINGS: f64 = (1u64 << 20) as f64;

/// [`point_side`] over a region read once: for every finite point and
/// every tolerance the same [`Side`], found among the segments near the
/// point instead of all of them.
///
/// The region's `v` span is cut into strips of equal height, about
/// eight segments' worth each, and every segment is filed
/// under each strip its `v` span meets. A segment whose `v` span does not
/// hold `p.y` adds nothing to the winding number, so the winding is summed
/// over `p`'s own strip — every segment that could count is filed there,
/// the strip of a coordinate being monotone in it, and each is counted
/// once. A segment within the tolerance of `p` has its `v` span within the
/// tolerance of `p.y`, so the boundary test is `point_side`'s own over the
/// strips that band meets and one more on each side, which absorbs the
/// distance's rounding. A ring of long segments across the whole span, or
/// many at one height, costs up to the walk; a discretised pcurve is
/// neither.
///
/// ```
/// use arris_geom::region2::{Polygon2, Side, SideIndex, point_side};
/// use arris_math::Point2;
///
/// let p = |x, y| Point2::new(x, y);
/// let square = Polygon2::from_points([p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)]);
/// let hole = Polygon2::from_points([p(1.0, 1.0), p(1.0, 2.0), p(2.0, 2.0), p(2.0, 1.0)]);
/// let region = [square, hole];
/// let index = SideIndex::new(&region);
/// for q in [p(3.0, 3.0), p(1.5, 1.5), p(4.0, 2.0), p(9.0, 9.0)] {
///     assert_eq!(index.side(q, 1e-9), point_side(&region, q, 1e-9));
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SideIndex {
    /// Every segment of every polygon, in polygon and ring order.
    segments: Vec<(Point2, Point2)>,
    /// The bottom of the first strip.
    lo: f64,
    /// The strips' common height; zero for a single strip.
    height: f64,
    /// For each strip, ascending, the indices of the segments filed there.
    strips: Vec<Vec<usize>>,
}

impl SideIndex {
    /// The index of the region `polygons` bound.
    pub fn new(polygons: &[Polygon2]) -> SideIndex {
        let segments: Vec<(Point2, Point2)> =
            polygons.iter().flat_map(Polygon2::segments).collect();
        let (lo, hi) = segments
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (a, b)| {
                (lo.min(a.y).min(b.y), hi.max(a.y).max(b.y))
            });
        let count = segments.len().div_ceil(SEGMENTS_PER_STRIP).max(1);
        let height = if count > 1 && hi > lo {
            (hi - lo) / count as f64
        } else {
            0.0
        };
        let rounding = f64::EPSILON * lo.abs().max(hi.abs());
        let height = if height > STRIP_HEIGHT_IN_ROUNDINGS * rounding {
            height
        } else {
            0.0
        };
        let mut index = SideIndex {
            segments,
            lo,
            height,
            strips: Vec::new(),
        };
        index.strips = vec![Vec::new(); if height > 0.0 { count } else { 1 }];
        let spans: Vec<(usize, usize)> = index
            .segments
            .iter()
            .map(|(a, b)| (index.strip(a.y.min(b.y)), index.strip(a.y.max(b.y))))
            .collect();
        for (i, (first, last)) in spans.into_iter().enumerate() {
            for strip in &mut index.strips[first..=last] {
                strip.push(i);
            }
        }
        index
    }

    /// The strip `v` falls in, the ends clamped to the first and last.
    fn strip(&self, v: f64) -> usize {
        if self.height > 0.0 {
            let at = ((v - self.lo) / self.height).floor().max(0.0);
            // A saturating cast: a `v` far past the top is the last strip.
            (at as usize).min(self.strips.len() - 1)
        } else {
            0
        }
    }

    /// Where `p` lies with respect to the region, as
    /// [`point_side`]`(polygons, p, boundary_tolerance)` would answer.
    pub fn side(&self, p: Point2, boundary_tolerance: f64) -> Side {
        let first = self.strip(p.y - boundary_tolerance).saturating_sub(1);
        let last = (self.strip(p.y + boundary_tolerance) + 1).min(self.strips.len() - 1);
        for strip in &self.strips[first..=last] {
            for &i in strip {
                let (a, b) = self.segments[i];
                if point_segment_distance(a, b, p) <= boundary_tolerance {
                    return Side::Boundary;
                }
            }
        }
        let winding: i32 = self.strips[self.strip(p.y)]
            .iter()
            .map(|&i| {
                let (a, b) = self.segments[i];
                crossing(a, b, p)
            })
            .sum();
        if winding != 0 {
            Side::Inside
        } else {
            Side::Outside
        }
    }
}

/// A point strictly inside the region `polygons` bound and further than
/// `clearance` from every segment, or `None` when the construction below
/// finds none.
///
/// The construction, which is what makes it deterministic: a horizontal
/// line is cut by the segments into spans; the spans whose midpoint has
/// a non-zero winding number are the inside ones; the longest of them
/// that clears every segment by more than `clearance` gives its middle.
/// The line's height is the midpoint between two consecutive distinct
/// vertex heights of the polygons — never a vertex's own height, so the
/// line runs along no segment and through no vertex — the one nearest
/// the middle of the polygons' height first, and the next ones outward
/// when that one holds no span at the clearance. A caller passes the
/// polygons' [`Polygon2::chord_deviation`] as the clearance, so the point
/// is inside the *curved* region and not merely inside its polygon.
///
/// `None` only for a region too thin to hold a point at that clearance at
/// any of those heights, or one with a single vertex height. It is never
/// a point the caller has to check again.
///
/// ```
/// use arris_geom::region2::{Polygon2, Side, interior_point, point_side};
/// use arris_math::Point2;
///
/// let p = |x, y| Point2::new(x, y);
/// let c = Polygon2::from_points([p(0.0, 0.0), p(4.0, 0.0), p(4.0, 1.0), p(1.0, 1.0),
///                                p(1.0, 3.0), p(4.0, 3.0), p(4.0, 4.0), p(0.0, 4.0)]);
/// let inside = interior_point(&[c.clone()], 0.0).unwrap();
/// assert_eq!(inside, p(0.5, 2.0), "the middle of the only inside span");
/// assert_eq!(point_side(&[c], inside, 1e-9), Side::Inside);
/// // An L whose middle height is one of its own segments: the line is
/// // taken just below it instead.
/// let l = Polygon2::from_points([p(1.0, 0.0), p(2.0, 0.0), p(2.0, 2.0), p(0.0, 2.0),
///                                p(0.0, 1.0), p(1.0, 1.0)]);
/// assert_eq!(interior_point(&[l], 0.0), Some(p(1.5, 0.5)));
/// ```
pub fn interior_point(polygons: &[Polygon2], clearance: f64) -> Option<Point2> {
    let segments: Vec<(Point2, Point2)> = polygons.iter().flat_map(Polygon2::segments).collect();
    if segments.is_empty() {
        return None;
    }
    let mut levels: Vec<f64> = segments.iter().map(|s| s.0.y).collect();
    levels.sort_by(f64::total_cmp);
    levels.dedup();
    let (lo, hi) = (levels[0], levels[levels.len() - 1]);
    let middle = 0.5 * (lo + hi);
    // The candidate heights, nearest the middle first; ties by height.
    let mut heights: Vec<f64> = levels.windows(2).map(|w| 0.5 * (w[0] + w[1])).collect();
    heights.sort_by(|a, b| {
        (a - middle)
            .abs()
            .total_cmp(&(b - middle).abs())
            .then(a.total_cmp(b))
    });
    heights
        .into_iter()
        .find_map(|height| span_middle(polygons, &segments, height, clearance))
}

/// The middle of the widest inside span of the horizontal at `height`
/// that clears every segment by more than `clearance`, when there is
/// one. `height` is no vertex's, so every segment either crosses it
/// properly or not at all.
fn span_middle(
    polygons: &[Polygon2],
    segments: &[(Point2, Point2)],
    height: f64,
    clearance: f64,
) -> Option<Point2> {
    let mut crossings: Vec<f64> = segments
        .iter()
        .filter_map(|&(a, b)| {
            if (a.y <= height) == (b.y <= height) {
                return None;
            }
            Some(a.x + (height - a.y) * (b.x - a.x) / (b.y - a.y))
        })
        .collect();
    crossings.sort_by(f64::total_cmp);
    let mut spans: Vec<(f64, Point2)> = crossings
        .windows(2)
        .filter_map(|w| {
            let middle = Point2::new(0.5 * (w[0] + w[1]), height);
            let inside = polygons
                .iter()
                .map(|p| p.winding_number(middle))
                .sum::<i32>()
                != 0;
            inside.then_some((w[1] - w[0], middle))
        })
        .collect();
    // Widest first; ties by the point, so two runs choose the same span.
    spans.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then(a.1.x.total_cmp(&b.1.x))
            .then(a.1.y.total_cmp(&b.1.y))
    });
    spans.into_iter().map(|(_, middle)| middle).find(|&middle| {
        segments
            .iter()
            .all(|&(a, b)| point_segment_distance(a, b, middle) > clearance)
    })
}

/// The distance from `p` to the closed segment `ab`, clamped to its ends.
pub(crate) fn point_segment_distance(a: Point2, b: Point2, p: Point2) -> f64 {
    let ab = b - a;
    let length = ab.norm_squared();
    if length == 0.0 {
        return (p - a).norm();
    }
    let t = ((p - a).dot(&ab) / length).clamp(0.0, 1.0);
    (p - (a + t * ab)).norm()
}

/// The pairs `(i, j)` — `i` into `a`, `j` into `b` — that `keep` admits
/// and whose segments meet, ascending. A sweep in `u`: each list is
/// visited in order of its segments' least `u`, and a segment is compared
/// only against those still spanning that `u`.
fn sweep(
    a: &[(Point2, Point2)],
    b: &[(Point2, Point2)],
    keep: impl Fn(usize, usize) -> bool,
) -> Vec<(usize, usize)> {
    let span = |s: &(Point2, Point2)| (s.0.x.min(s.1.x), s.0.x.max(s.1.x));
    let order = |xs: &[(Point2, Point2)]| {
        let mut order: Vec<usize> = (0..xs.len()).collect();
        order.sort_by(|&i, &j| {
            span(&xs[i])
                .0
                .partial_cmp(&span(&xs[j]).0)
                .unwrap_or(core::cmp::Ordering::Equal)
                .then(i.cmp(&j))
        });
        order
    };
    let (order_a, order_b) = (order(a), order(b));
    let mut hits = Vec::new();
    // Both lists are swept together: a segment of one is compared with
    // every segment of the other whose span has not ended.
    let (mut ia, mut ib) = (0, 0);
    let (mut active_a, mut active_b): (Vec<usize>, Vec<usize>) = (Vec::new(), Vec::new());
    while ia < order_a.len() || ib < order_b.len() {
        let next_a = order_a.get(ia).map(|&i| span(&a[i]).0);
        let next_b = order_b.get(ib).map(|&j| span(&b[j]).0);
        let from_a = match (next_a, next_b) {
            (Some(x), Some(y)) => x <= y,
            (Some(_), None) => true,
            _ => false,
        };
        if from_a {
            let i = order_a[ia];
            ia += 1;
            let lo = span(&a[i]).0;
            active_b.retain(|&j| span(&b[j]).1 >= lo);
            for &j in &active_b {
                if keep(i, j) && segments_intersect(a[i].0, a[i].1, b[j].0, b[j].1) {
                    hits.push((i, j));
                }
            }
            active_a.push(i);
        } else {
            let j = order_b[ib];
            ib += 1;
            let lo = span(&b[j]).0;
            active_a.retain(|&i| span(&a[i]).1 >= lo);
            for &i in &active_a {
                if keep(i, j) && segments_intersect(a[i].0, a[i].1, b[j].0, b[j].1) {
                    hits.push((i, j));
                }
            }
            active_b.push(j);
        }
    }
    hits.sort_unstable();
    hits.dedup();
    hits
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
    #[test]
    fn point_segment_distance_clamps_to_the_ends() {
        let q = |x, y| Point2::new(x, y);
        assert_eq!(
            point_segment_distance(q(0.0, 0.0), q(2.0, 0.0), q(1.0, 3.0)),
            3.0
        );
        assert_eq!(
            point_segment_distance(q(0.0, 0.0), q(2.0, 0.0), q(5.0, 0.0)),
            3.0
        );
        assert_eq!(
            point_segment_distance(q(0.0, 0.0), q(0.0, 0.0), q(0.0, 4.0)),
            4.0
        );
    }

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
