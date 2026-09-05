//! Least-squares approximation of a 2D curve by a B-spline at the
//! caller's own parametrisation (*The NURBS Book* §9.4.1, with the knot
//! vector refined where the caller's deviation says the fit is not good
//! enough), for the pcurves that have no exact `Curve2` variant.

use core::fmt;

use arris_math::{Interval, Point2, Vec2};

use super::basis::{self, MAX_DEGREE};
use crate::NurbsCurve2;

/// The most spans a fit may refine to before it gives up with
/// [`FitError::Diverged`]: a structural bound that makes every fit
/// terminate, not a tolerance. A cubic on this many spans resolves a
/// sinusoid of amplitude a thousand to `1e-7`; a caller that needs more
/// asks for a higher degree.
pub const MAX_FIT_SPANS: usize = 1024;

/// Spans the first fit tries; refinement splits from here.
const INITIAL_SPANS: usize = 4;

/// The fit is accepted where the caller's deviation is at or below this
/// fraction of the tolerance at every check parameter. The residual of a
/// least-squares B-spline fit oscillates about `p + 1` times per span and
/// the check samples it `4p + 4` times per span, so the largest checked
/// value is within `cos(π/8)` of the true maximum; half the tolerance is
/// margin for what the samples miss, so the result meets the tolerance
/// between them too. A ratio between sampling density and the residual's
/// shape, not a tolerance.
const FIT_MARGIN: f64 = 0.5;

/// Why a fit did not produce a curve.
#[derive(Debug, Clone, PartialEq)]
pub enum FitError {
    /// The request cannot be fitted: a degree outside `1..=MAX_DEGREE`,
    /// an unbounded or empty range.
    Degenerate(String),
    /// The tolerance is not finite and positive.
    InvalidTolerance(f64),
    /// The curve or the deviation returned a non-finite value at `t`.
    NonFinite {
        /// Where.
        t: f64,
    },
    /// The deviation still exceeded the tolerance with [`MAX_FIT_SPANS`]
    /// spans: the curve is not one this degree can approximate at the
    /// caller's parametrisation to that tolerance — or the deviation is
    /// noise the fit cannot get under.
    Diverged {
        /// How many spans the last fit had.
        spans: usize,
        /// The worst deviation it left.
        deviation: f64,
    },
}

impl fmt::Display for FitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FitError::Degenerate(reason) => write!(f, "cannot fit: {reason}"),
            FitError::InvalidTolerance(tol) => write!(f, "fit tolerance {tol} is not positive"),
            FitError::NonFinite { t } => {
                write!(f, "the curve or its deviation is not finite at t = {t}")
            }
            FitError::Diverged { spans, deviation } => write!(
                f,
                "the fit still deviates by {deviation} with {spans} spans, the most it may use"
            ),
        }
    }
}

impl std::error::Error for FitError {}

/// A non-rational B-spline of `degree` over `range` approximating `f` at
/// its own parameter — `fit(t) ≈ f(t)`, same `t`, so the result is
/// same-parameter by construction and interpolates `f` at both ends —
/// with the knots refined until `deviation(t, fit(t))` is within `tol`
/// at every check parameter (the fit's samples and their midpoints, four
/// times `p + 1` per span, both ends included). `deviation` is the
/// caller's measure in the caller's units: the distance in (u, v) for a
/// plain fit, the 3D distance between the surface at the fitted point
/// and the true curve for a pcurve.
///
/// Guarantees: deterministic; terminates, with [`FitError::Diverged`]
/// once [`MAX_FIT_SPANS`] would be exceeded — never a loop; the result's
/// domain is `range` exactly and its control points are finite.
///
/// ```
/// use arris_geom::fit_curve2;
/// use arris_math::{Interval, Point2};
///
/// let f = |t: f64| Point2::new(t, t.cos());
/// let range = Interval::new(0.0, 3.0).unwrap();
/// let fit = fit_curve2(f, range, 3, |t, q| (q - f(t)).norm(), 1e-9).unwrap();
/// assert_eq!(fit.domain(), range);
/// assert!((fit.eval(1.7).point - f(1.7)).norm() <= 1e-9);
/// ```
pub fn fit_curve2(
    f: impl Fn(f64) -> Point2,
    range: Interval,
    degree: usize,
    deviation: impl Fn(f64, Point2) -> f64,
    tol: f64,
) -> Result<NurbsCurve2, FitError> {
    if degree == 0 || degree > MAX_DEGREE {
        return Err(FitError::Degenerate(format!(
            "degree {degree} is not in 1..={MAX_DEGREE}"
        )));
    }
    if !range.is_bounded() || range.length() <= 0.0 {
        return Err(FitError::Degenerate(format!(
            "range [{}, {}] is not a bounded interval of positive length",
            range.lo(),
            range.hi()
        )));
    }
    if !(tol.is_finite() && tol > 0.0) {
        return Err(FitError::InvalidTolerance(tol));
    }
    let mut breaks: Vec<f64> = (0..=INITIAL_SPANS)
        .map(|i| range.lerp(i as f64 / INITIAL_SPANS as f64))
        .collect();
    loop {
        let samples = sample_parameters(&breaks, degree);
        let curve = least_squares(&f, degree, &breaks, &samples)?;
        // Check at the samples and between them; split every span whose
        // worst check exceeds the margin.
        let mut worst = 0.0f64;
        let mut split: Vec<bool> = vec![false; breaks.len() - 1];
        let mut span = 0;
        for pair in samples.windows(2) {
            for t in [pair[0], 0.5 * (pair[0] + pair[1])] {
                let d = deviation(t, curve.eval(t).point);
                if !d.is_finite() {
                    return Err(FitError::NonFinite { t });
                }
                worst = worst.max(d);
                while span + 1 < breaks.len() - 1 && t >= breaks[span + 1] {
                    span += 1;
                }
                if d > FIT_MARGIN * tol {
                    split[span] = true;
                }
            }
        }
        let t_end = range.hi();
        let d_end = deviation(t_end, curve.eval(t_end).point);
        if !d_end.is_finite() {
            return Err(FitError::NonFinite { t: t_end });
        }
        worst = worst.max(d_end);
        if d_end > FIT_MARGIN * tol {
            split[breaks.len() - 2] = true;
        }
        if !split.iter().any(|&s| s) {
            return Ok(curve);
        }
        let spans = breaks.len() - 1;
        let new_spans = spans + split.iter().filter(|&&s| s).count();
        if new_spans > MAX_FIT_SPANS {
            return Err(FitError::Diverged {
                spans,
                deviation: worst,
            });
        }
        let mut refined = Vec::with_capacity(new_spans + 1);
        for (i, w) in breaks.windows(2).enumerate() {
            refined.push(w[0]);
            if split[i] {
                refined.push(0.5 * (w[0] + w[1]));
            }
        }
        refined.push(range.hi());
        breaks = refined;
    }
}

/// The fit's sample parameters: every break, and `2p + 1` parameters
/// evenly inside each span, ascending.
fn sample_parameters(breaks: &[f64], degree: usize) -> Vec<f64> {
    let inside = 2 * degree + 1;
    let mut out = Vec::with_capacity(breaks.len() + (breaks.len() - 1) * inside);
    for w in breaks.windows(2) {
        out.push(w[0]);
        for j in 1..=inside {
            out.push(w[0] + (w[1] - w[0]) * j as f64 / (inside + 1) as f64);
        }
    }
    out.push(breaks[breaks.len() - 1]);
    out
}

/// The clamped knot vector over `breaks`.
fn clamped_knots(breaks: &[f64], degree: usize) -> Vec<f64> {
    let (lo, hi) = (breaks[0], breaks[breaks.len() - 1]);
    let mut knots = vec![lo; degree + 1];
    knots.extend_from_slice(&breaks[1..breaks.len() - 1]);
    knots.extend(core::iter::repeat_n(hi, degree + 1));
    knots
}

/// The least-squares B-spline through the end samples and nearest the
/// rest (A9.7's normal equations, solved by a banded Cholesky
/// factorisation since `NᵀN` has bandwidth `p`).
#[allow(clippy::needless_range_loop)] // `j`, `l` index the basis row and the net together
fn least_squares(
    f: &dyn Fn(f64) -> Point2,
    degree: usize,
    breaks: &[f64],
    samples: &[f64],
) -> Result<NurbsCurve2, FitError> {
    let p = degree;
    let knots = clamped_knots(breaks, p);
    let n = knots.len() - p - 1;
    let m = samples.len() - 1;
    let data: Vec<Point2> = samples.iter().map(|&t| f(t)).collect();
    if let Some(i) = data
        .iter()
        .position(|q| !q.coords.iter().all(|c| c.is_finite()))
    {
        return Err(FitError::NonFinite { t: samples[i] });
    }
    let (q0, qm) = (data[0], data[m]);
    // Interior unknowns P_1 .. P_{n-2}.
    let unknowns = n - 2;
    let mut points = vec![Point2::origin(); n];
    points[0] = q0;
    points[n - 1] = qm;
    if unknowns > 0 {
        let mut band = Band::new(unknowns, p);
        let mut rhs = vec![Vec2::zeros(); unknowns];
        for k in 1..m {
            let t = samples[k];
            let s = basis::span(p, &knots, n, t);
            let row = basis::derivatives(p, &knots, s, t)[0];
            let first = s - p;
            // `row[0]` only weighs `Q_0` when `P_0` is active, and the last
            // active function is `P_{n-1}` only on the last span.
            let r = {
                let mut r = data[k].coords;
                for j in 0..=p {
                    let idx = first + j;
                    if idx == 0 {
                        r -= row[j] * q0.coords;
                    } else if idx == n - 1 {
                        r -= row[j] * qm.coords;
                    }
                }
                r
            };
            for j in 0..=p {
                let a = first + j;
                if a == 0 || a == n - 1 {
                    continue;
                }
                rhs[a - 1] += row[j] * r;
                for l in j..=p {
                    let b = first + l;
                    if b == 0 || b == n - 1 {
                        continue;
                    }
                    band.add(a - 1, b - 1, row[j] * row[l]);
                }
            }
        }
        let solved = band
            .solve(&rhs)
            .ok_or_else(|| FitError::Degenerate("the normal equations are singular".to_owned()))?;
        for (i, v) in solved.into_iter().enumerate() {
            points[i + 1] = Point2::from(v);
        }
    }
    NurbsCurve2::new(p, knots, points, vec![1.0; n])
        .map_err(|e| FitError::Degenerate(e.to_string()))
}

/// A symmetric band matrix of bandwidth `w`: `a[i][d]` is the entry
/// `(i, i + d)`.
struct Band {
    a: Vec<Vec<f64>>,
    w: usize,
}

impl Band {
    fn new(size: usize, w: usize) -> Self {
        Band {
            a: vec![vec![0.0; w + 1]; size],
            w,
        }
    }

    /// Adds `v` to `(i, j)` and `(j, i)`, `|i − j| ≤ w`.
    fn add(&mut self, i: usize, j: usize, v: f64) {
        let (lo, hi) = if i <= j { (i, j) } else { (j, i) };
        self.a[lo][hi - lo] += v;
    }

    fn at(&self, i: usize, j: usize) -> f64 {
        let (lo, hi) = if i <= j { (i, j) } else { (j, i) };
        if hi - lo > self.w {
            0.0
        } else {
            self.a[lo][hi - lo]
        }
    }

    /// `x` with `A x = rhs` for a positive definite `A`, by a banded
    /// Cholesky factorisation; `None` when a pivot is not positive.
    #[allow(clippy::needless_range_loop)] // the factorisation's index triangles, as written
    fn solve(&self, rhs: &[Vec2]) -> Option<Vec<Vec2>> {
        let n = rhs.len();
        let w = self.w;
        // `l[i][k]` holds `L(i, i − w + k)`, so `L(i, j)` is `l[i][j + w − i]`.
        let mut l = vec![vec![0.0; w + 1]; n];
        let get = |l: &Vec<Vec<f64>>, i: usize, j: usize| -> f64 {
            if i < j || i - j > w {
                0.0
            } else {
                l[i][j + w - i]
            }
        };
        for i in 0..n {
            for j in i.saturating_sub(w)..=i {
                let mut sum = self.at(i, j);
                for k in i.saturating_sub(w)..j {
                    sum -= get(&l, i, k) * get(&l, j, k);
                }
                if i == j {
                    if sum.is_nan() || sum <= 0.0 {
                        return None;
                    }
                    l[i][w] = sum.sqrt();
                } else {
                    l[i][j + w - i] = sum / get(&l, j, j);
                }
            }
        }
        // Forward: L y = rhs; back: Lᵀ x = y.
        let mut y = vec![Vec2::zeros(); n];
        for i in 0..n {
            let mut v = rhs[i];
            for k in i.saturating_sub(w)..i {
                v -= get(&l, i, k) * y[k];
            }
            y[i] = v / get(&l, i, i);
        }
        let mut x = vec![Vec2::zeros(); n];
        for i in (0..n).rev() {
            let mut v = y[i];
            for k in i + 1..(i + w + 1).min(n) {
                v -= get(&l, k, i) * x[k];
            }
            x[i] = v / get(&l, i, i);
        }
        Some(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_band_solver_agrees_with_a_dense_solution() {
        // A tridiagonal SPD system with a known answer.
        let mut b = Band::new(4, 1);
        for i in 0..4 {
            b.add(i, i, 4.0);
        }
        for i in 0..3 {
            b.add(i, i + 1, 1.0);
        }
        let x_true: Vec<Vec2> = (0..4)
            .map(|i| Vec2::new(i as f64, 1.0 - i as f64))
            .collect();
        let rhs: Vec<Vec2> = (0..4)
            .map(|i| {
                let mut v = 4.0 * x_true[i];
                if i > 0 {
                    v += x_true[i - 1];
                }
                if i < 3 {
                    v += x_true[i + 1];
                }
                v
            })
            .collect();
        let x = b.solve(&rhs).unwrap();
        for (a, e) in x.iter().zip(&x_true) {
            assert!((a - e).norm() < 1e-14);
        }
        let mut bad = Band::new(2, 1);
        bad.add(0, 0, 1.0);
        bad.add(0, 1, 2.0);
        bad.add(1, 1, 1.0);
        assert!(bad.solve(&[Vec2::zeros(); 2]).is_none());
    }

    #[test]
    fn a_line_is_fitted_exactly_by_a_linear_spline() {
        let f = |t: f64| Point2::new(2.0 * t + 1.0, -t);
        let fit = fit_curve2(
            f,
            Interval::new(-1.0, 3.0).unwrap(),
            1,
            |t, q| (q - f(t)).norm(),
            1e-12,
        )
        .unwrap();
        assert_eq!(fit.degree(), 1);
        assert!((fit.eval(0.37).point - f(0.37)).norm() < 1e-13);
        assert_eq!(fit.eval(3.0).point, f(3.0));
    }

    #[test]
    fn bad_requests_are_named() {
        let f = |t: f64| Point2::new(t, 0.0);
        let range = Interval::new(0.0, 1.0).unwrap();
        assert!(matches!(
            fit_curve2(f, range, 0, |_, _| 0.0, 1e-6),
            Err(FitError::Degenerate(_))
        ));
        assert!(matches!(
            fit_curve2(f, Interval::REAL, 2, |_, _| 0.0, 1e-6),
            Err(FitError::Degenerate(_))
        ));
        assert_eq!(
            fit_curve2(f, range, 2, |_, _| 0.0, 0.0),
            Err(FitError::InvalidTolerance(0.0))
        );
        assert!(matches!(
            fit_curve2(f, range, 2, |_, _| f64::NAN, 1e-6),
            Err(FitError::NonFinite { .. })
        ));
        let e = fit_curve2(f, range, 2, |_, _| 1.0, 1e-6).unwrap_err();
        assert!(
            matches!(e, FitError::Diverged { spans, .. } if spans <= MAX_FIT_SPANS),
            "{e}"
        );
    }
}
