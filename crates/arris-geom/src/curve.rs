//! Curves and their evaluation.

use core::fmt;

use arris_math::{Frame, Interval, Isometry, Point3, UnitVec3, Vec3};

use crate::NurbsCurve;

/// A 3D curve with the parametrisation of `docs/02-data-model.md` §Curves
/// (Open CASCADE's, so STEP round-trips without re-parametrising).
///
/// The fields are plain data: a `Curve` is a value the arena stores once
/// and never modifies, and its validity (positive radii, `major_radius ≥
/// minor_radius`) is the checker's to enforce; the NURBS variant is valid
/// by its constructor. Evaluation of any finite parameter never panics.
///
/// ```
/// use arris_geom::Curve;
/// use arris_math::{Frame, Point3};
/// use core::f64::consts::PI;
///
/// let c = Curve::Circle { frame: Frame::world(), radius: 2.0 };
/// let e = c.eval(PI);
/// assert!((e.point - Point3::new(-2.0, 0.0, 0.0)).norm() < 1e-15);
/// assert!((e.d1.y + 2.0).abs() < 1e-15); // tangent at π points along −Y
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Curve {
    /// `P(t) = O + t·D`; `t` is arc length because `D` is unit.
    Line {
        /// `O`.
        origin: Point3,
        /// `D`.
        direction: UnitVec3,
    },
    /// `P(t) = O + R(cos t·X + sin t·Y)`, counter-clockwise about `Z`.
    Circle {
        /// Centre and plane; `X` is where `t = 0`.
        frame: Frame,
        /// `R`.
        radius: f64,
    },
    /// `P(t) = O + a cos t·X + b sin t·Y` with `a ≥ b`.
    Ellipse {
        /// Centre and plane; `X` is the major axis.
        frame: Frame,
        /// `a`.
        major_radius: f64,
        /// `b`.
        minor_radius: f64,
    },
    /// A rational B-spline; parametrised by its knots.
    Nurbs(NurbsCurve),
}

/// The fieldless twin of [`Curve`], for errors and dispatch tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CurveKind {
    /// [`Curve::Line`].
    Line,
    /// [`Curve::Circle`].
    Circle,
    /// [`Curve::Ellipse`].
    Ellipse,
    /// [`Curve::Nurbs`].
    Nurbs,
}

impl fmt::Display for CurveKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            CurveKind::Line => "line",
            CurveKind::Circle => "circle",
            CurveKind::Ellipse => "ellipse",
            CurveKind::Nurbs => "NURBS",
        })
    }
}

/// A curve evaluated at one `t`: the point and its derivatives to second
/// order, with respect to `t` as stored, never normalised.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurveEval {
    /// `P(t)`.
    pub point: Point3,
    /// `dP/dt`, the tangent.
    pub d1: Vec3,
    /// `d²P/dt²`.
    pub d2: Vec3,
}

impl Curve {
    /// Which variant this is.
    pub fn kind(&self) -> CurveKind {
        match self {
            Curve::Line { .. } => CurveKind::Line,
            Curve::Circle { .. } => CurveKind::Circle,
            Curve::Ellipse { .. } => CurveKind::Ellipse,
            Curve::Nurbs(_) => CurveKind::Nurbs,
        }
    }

    /// The point and its derivatives at `t`. Defined for every finite
    /// parameter: a periodic curve wraps, a line extends, a clamped NURBS
    /// extrapolates its end piece.
    pub fn eval(&self, t: f64) -> CurveEval {
        match self {
            &Curve::Line { origin, direction } => CurveEval {
                point: origin + t * direction.into_inner(),
                d1: direction.into_inner(),
                d2: Vec3::zeros(),
            },
            &Curve::Circle { frame, radius } => {
                let (st, ct) = t.sin_cos();
                let (x, y) = (frame.x().into_inner(), frame.y().into_inner());
                let radial = ct * x + st * y;
                CurveEval {
                    point: frame.origin() + radius * radial,
                    d1: radius * (-st * x + ct * y),
                    d2: -radius * radial,
                }
            }
            &Curve::Ellipse {
                frame,
                major_radius,
                minor_radius,
            } => {
                let (st, ct) = t.sin_cos();
                let (x, y) = (frame.x().into_inner(), frame.y().into_inner());
                let radial = (major_radius * ct) * x + (minor_radius * st) * y;
                CurveEval {
                    point: frame.origin() + radial,
                    d1: (-major_radius * st) * x + (minor_radius * ct) * y,
                    d2: -radial,
                }
            }
            Curve::Nurbs(c) => c.eval(t),
        }
    }

    /// `P(t)` alone.
    pub fn point(&self, t: f64) -> Point3 {
        self.eval(t).point
    }

    /// The parametric domain: [`Interval::REAL`] for a line, the closed
    /// fundamental interval `[0, 2π]` for a circle or an ellipse, the
    /// knot range for a NURBS.
    pub fn domain(&self) -> Interval {
        match self {
            Curve::Line { .. } => Interval::REAL,
            Curve::Circle { .. } | Curve::Ellipse { .. } => Interval::TURN,
            Curve::Nurbs(c) => c.domain(),
        }
    }

    /// The period, `None` for a line or a NURBS whose knots do not wrap.
    pub fn period(&self) -> Option<f64> {
        match self {
            Curve::Line { .. } => None,
            Curve::Circle { .. } | Curve::Ellipse { .. } => Some(core::f64::consts::TAU),
            Curve::Nurbs(c) => c.period(),
        }
    }

    /// How many straight segments approximate the curve over `range`
    /// within `chord` in 3D: the twin of
    /// [`crate::region2::Piece::segment_count`] for a 3D curve, with the
    /// same bound — a chord over a parameter step `h` deviates at most
    /// `|d2| h² / 8` from a curve whose second derivative is bounded by
    /// `|d2|` — and the same floors and ceiling: one segment for a line,
    /// never fewer than [`crate::region2::MIN_SEGMENTS_PER_TURN`] per
    /// turn of a conic or [`crate::region2::MIN_SEGMENTS_PER_SPAN`] per
    /// knot span of a NURBS (whose second derivative is sampled), never
    /// more than [`crate::region2::MAX_SEGMENTS_PER_PIECE`]. An unbounded
    /// or empty range is one segment; `f64::INFINITY` asks for the
    /// minimum counts alone.
    ///
    /// ```
    /// use arris_geom::Curve;
    /// use arris_math::{Frame, Interval};
    ///
    /// let c = Curve::Circle { frame: Frame::world(), radius: 4.0 };
    /// // sqrt(8 · 1e-3 / 4) ≈ 0.0447 radians per segment: 141 of them.
    /// assert_eq!(c.chord_segments(Interval::TURN, 1e-3), 141);
    /// assert_eq!(c.chord_segments(Interval::TURN, f64::INFINITY), 8);
    /// ```
    pub fn chord_segments(&self, range: Interval, chord: f64) -> usize {
        use crate::region2::{
            CURVATURE_SAMPLES_PER_SPAN, MAX_SEGMENTS_PER_PIECE, MIN_SEGMENTS_PER_SPAN, per_turn,
        };
        let length = range.length();
        if !(length.is_finite() && length > 0.0) {
            return 1;
        }
        let (minimum, d2) = match self {
            Curve::Line { .. } => return 1,
            Curve::Circle { radius, .. } => (per_turn(length), radius.abs()),
            Curve::Ellipse { major_radius, .. } => (per_turn(length), major_radius.abs()),
            Curve::Nurbs(n) => {
                let mut knots: Vec<f64> = n
                    .knots()
                    .iter()
                    .copied()
                    .filter(|&k| range.lo() < k && k < range.hi())
                    .collect();
                knots.dedup();
                let spans = knots.len() + 1;
                let samples = spans * CURVATURE_SAMPLES_PER_SPAN;
                let d2 = (0..=samples)
                    .map(|i| n.eval(range.lerp(i as f64 / samples as f64)).d2.norm())
                    .fold(0.0, f64::max);
                (spans * MIN_SEGMENTS_PER_SPAN, d2)
            }
        };
        let from_chord = if chord.is_finite() && chord > 0.0 && d2 > 0.0 {
            (length / (8.0 * chord / d2).sqrt()).ceil()
        } else if chord == f64::INFINITY || d2 == 0.0 {
            0.0
        } else {
            f64::INFINITY
        };
        let wanted = if from_chord.is_finite() {
            from_chord as usize
        } else {
            MAX_SEGMENTS_PER_PIECE
        };
        wanted.max(minimum).min(MAX_SEGMENTS_PER_PIECE)
    }

    /// The same curve moved by `motion`, parametrisation carried along:
    /// `moved.eval(t).point == motion.apply(self.eval(t).point)` to
    /// rounding.
    pub fn transformed(&self, motion: &Isometry) -> Curve {
        match self {
            &Curve::Line { origin, direction } => Curve::Line {
                origin: motion.apply(origin),
                direction: motion.apply_unit(direction),
            },
            &Curve::Circle { frame, radius } => Curve::Circle {
                frame: frame.transformed(motion),
                radius,
            },
            &Curve::Ellipse {
                frame,
                major_radius,
                minor_radius,
            } => Curve::Ellipse {
                frame: frame.transformed(motion),
                major_radius,
                minor_radius,
            },
            Curve::Nurbs(c) => Curve::Nurbs(c.transformed(motion)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domains_and_periods_follow_the_table() {
        let line = Curve::Line {
            origin: Point3::origin(),
            direction: Vec3::x_axis(),
        };
        assert_eq!(line.domain(), Interval::REAL);
        assert_eq!(line.period(), None);
        assert_eq!(line.kind(), CurveKind::Line);
        let circle = Curve::Circle {
            frame: Frame::world(),
            radius: 1.0,
        };
        assert_eq!(circle.domain(), Interval::TURN);
        assert_eq!(circle.period(), Some(core::f64::consts::TAU));
        assert_eq!(circle.kind().to_string(), "circle");
        let ellipse = Curve::Ellipse {
            frame: Frame::world(),
            major_radius: 2.0,
            minor_radius: 1.0,
        };
        assert_eq!(ellipse.domain(), Interval::TURN);
        assert_eq!(ellipse.kind(), CurveKind::Ellipse);
        assert_eq!(line.point(2.0), Point3::new(2.0, 0.0, 0.0));
    }
}
