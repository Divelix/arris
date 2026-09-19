//! A `Curve::Nurbs` against every analytic surface: each span of the
//! curve substituted into the surface's implicit polynomial, in
//! Bernstein form (ADR-0018; `docs/DATA-MODEL.md` §Curves).
//!
//! Every analytic surface is the zero set of a polynomial `F` of degree
//! `d` in its own frame — one for a plane, two for a quadric, four for a
//! torus. A rational span `C(s) = A(s) / w(s)` of degree `p` put into it
//! and cleared of its denominator, `g(s) = w(s)ᵈ F(A(s) / w(s))`, is a
//! polynomial of degree `d·p` with the sign of `F` along the span, the
//! weights being positive. `g` is built from the span's homogeneous
//! Bézier points by products in the Bernstein basis
//! (`crate::bernstein`), never through power coefficients.
//!
//! What is decided on `g` is only *where to look*: the sign changes of
//! `g′` are the extrema of `g`, between two of which `g` is monotone and
//! crosses zero at most once. What is decided there is decided in
//! length, as every other arm of the table decides it: on the signed
//! **distance** `δ(t)` from the curve's point to the surface, the exact
//! one the line arms walk. `g = φ·δ` with `φ > 0` wherever `δ` is small,
//! so near the surface an extremum of `g` is an extremum of `δ` to
//! second order in `δ`, and `g` and `δ` change sign together.
//!
//! `IntCurveSurface`'s polynomial case and `IntAna_IntConicQuad` in the
//! reference tree were read for how a conic is put into a quadric;
//! nothing of either is here — they solve in power coefficients.

use arris_math::roots;
use arris_math::{Interval, Tolerance};

use crate::bernstein::{Binomials, derivative, sign_change_candidates};
use crate::implicit::{BERNSTEIN_ROUNDING, Implicit};
use crate::intersect_curve::{hit, points};
use crate::nurbs::BezierSpan;
use crate::{Curve, CurveSurfaceIntersection, GeomError, GeomKind, NurbsCurve, Surface};

/// A parameter at which the distance is looked at: an extremum of it
/// among its neighbours, or an end of an open curve.
#[derive(Clone, Copy)]
struct Stop {
    t: f64,
    distance: f64,
    /// Within `tol.linear` of the surface.
    touch: bool,
    /// An extremum inside the curve rather than an end of it.
    interior: bool,
}

/// A rational B-spline curve against an analytic surface.
///
/// The parameters looked at are every distinct knot of the domain and,
/// on every span, every candidate for a sign change of `g′`
/// ([`sign_change_candidates`], against [`BERNSTEIN_ROUNDING`] of the
/// span's magnitude): between two consecutive ones `g` is monotone. Of
/// those, the **stops** are the ones where the distance `δ` is an
/// extremum among its neighbours, and the two ends of a curve that is
/// not periodic, which are extrema of a function on a closed interval.
/// Then, as `line_by_distance` has it: every stop within `tol.linear`
/// is the whole curve within it, `Coincident`; a stop within
/// `tol.linear` is a hit that absorbs the crossings on the stretches
/// beside it, and a run of such stops with no other between them is one
/// hit, at the one nearest the surface; two consecutive other stops of
/// opposite sign hold one crossing, by bracketed Newton on `δ`. A hit at
/// a stop is `tangent` when its run holds an extremum inside the curve:
/// a curve that only *ends* within `tol.linear` of the surface meets it
/// there without touching it — that is a section edge ending on a face,
/// which the pave model makes a vertex of, not a graze. A periodic
/// curve's stops go round, and its hits come back in `[knots[p],
/// knots[n])`.
pub(crate) fn spline_surface(
    curve: &Curve,
    spline: &NurbsCurve,
    surface: &Surface,
    tol: Tolerance,
) -> Result<CurveSurfaceIntersection, GeomError> {
    let unsupported = || GeomError::Unsupported {
        a: GeomKind::Curve(curve.kind()),
        b: GeomKind::Surface(surface.kind()),
    };
    let implicit = Implicit::of(surface).ok_or_else(unsupported)?;
    let domain = spline.domain();
    let period = spline.period();
    let binomials = Binomials::new(implicit.degree() * spline.degree());

    let mut splits: Vec<f64> = Vec::new();
    for span in spline.bezier_spans() {
        splits.push(span.lo);
        splits.push(span.hi);
        let width = span.hi - span.lo;
        splits.extend(
            extrema_on(&implicit, &span, &binomials)
                .into_iter()
                .map(|s| span.lo + width * s),
        );
    }
    // A periodic curve's last knot is its first again.
    if let Some(period) = period {
        for t in &mut splits {
            if *t >= domain.hi() {
                *t -= period;
            }
            *t = t.max(domain.lo());
        }
    }
    splits.retain(|t| t.is_finite());
    splits.sort_by(f64::total_cmp);
    splits.dedup();
    if splits.is_empty() {
        // A validated curve has a span; one whose knots are not finite
        // numbers could not have been built.
        return Ok(CurveSurfaceIntersection::Points(Vec::new()));
    }

    let distance = |t: f64| implicit.distance(spline.eval(t).point);
    let slope = |t: f64| {
        let e = spline.eval(t);
        implicit
            .gradient(e.point)
            .dot(&implicit.frame.vec_to_local(e.d1))
    };
    let values: Vec<f64> = splits.iter().map(|&t| distance(t)).collect();
    let n = splits.len();
    let cyclic = period.is_some();
    let neighbour = |i: usize, step: isize| -> Option<f64> {
        let j = i as isize + step;
        if cyclic {
            Some(values[j.rem_euclid(n as isize) as usize])
        } else if j < 0 || j >= n as isize {
            None
        } else {
            Some(values[j as usize])
        }
    };
    let mut stops: Vec<Stop> = Vec::new();
    for i in 0..n {
        let v = values[i];
        let (before, after) = (neighbour(i, -1), neighbour(i, 1));
        let low = before.is_none_or(|b| b >= v) && after.is_none_or(|a| a >= v);
        let high = before.is_none_or(|b| b <= v) && after.is_none_or(|a| a <= v);
        if low || high {
            stops.push(Stop {
                t: splits[i],
                distance: v,
                touch: v.abs() <= tol.linear,
                interior: before.is_some() && after.is_some(),
            });
        }
    }
    if stops.iter().all(|s| s.touch) {
        return Ok(CurveSurfaceIntersection::Coincident);
    }
    if cyclic {
        // Started at a stop clear of the surface, no run of touches goes
        // round the end of the list.
        let first = stops.iter().position(|s| !s.touch).unwrap_or(0);
        stops.rotate_left(first);
    }
    let mut runs: Vec<Stop> = Vec::with_capacity(stops.len());
    for stop in stops {
        match runs.last_mut() {
            Some(last) if last.touch && stop.touch => {
                let interior = last.interior || stop.interior;
                if stop.distance.abs() < last.distance.abs() {
                    *last = stop;
                }
                last.interior = interior;
            }
            _ => runs.push(stop),
        }
    }

    let crossing = |lo: f64, hi: f64| -> Result<f64, GeomError> {
        let degenerate = |reason: String| GeomError::Degenerate {
            kind: GeomKind::Curve(curve.kind()),
            reason,
        };
        let bracket = Interval::new(lo, hi)
            .map_err(|_| degenerate(format!("crossing bracket [{lo}, {hi}]")))?;
        roots::newton_in_interval(distance, slope, bracket, 0.0)
            .map_err(|e| degenerate(format!("crossing in [{lo}, {hi}]: {e}")))
    };
    let mut hits = Vec::new();
    for (i, stop) in runs.iter().enumerate() {
        if stop.touch {
            hits.push(hit(curve, surface, stop.t, stop.interior)?);
        }
        // The stretch to the next stop; a periodic curve's goes round,
        // past the end of the domain wherever the next stop is behind.
        let next = match (runs.get(i + 1), period) {
            (Some(next), _) => *next,
            (None, Some(_)) if runs.len() > 1 => runs[0],
            (None, _) => continue,
        };
        let next_t = match period {
            Some(period) if next.t <= stop.t => next.t + period,
            _ => next.t,
        };
        if stop.touch || next.touch || (stop.distance < 0.0) == (next.distance < 0.0) {
            continue;
        }
        let mut t = crossing(stop.t, next_t)?;
        if let Some(period) = period {
            if t >= domain.hi() {
                t = (t - period).max(domain.lo());
            }
        }
        hits.push(hit(curve, surface, t, false)?);
    }
    Ok(points(hits))
}

/// The candidates for an extremum of `g` on one span, as parameters of
/// the span's own `[0, 1]`.
fn extrema_on(implicit: &Implicit<'_>, span: &BezierSpan<3>, binomials: &Binomials) -> Vec<f64> {
    let origin = implicit.frame.origin().coords;
    let mut coords: [Vec<f64>; 4] = Default::default();
    let (mut reach, mut weight) = (0.0f64, 0.0f64);
    for (a, w) in &span.control {
        // `w·P` in the frame: `w (P − O)` turned into it.
        let local = implicit.frame.vec_to_local(a - *w * origin);
        coords[0].push(local.x);
        coords[1].push(local.y);
        coords[2].push(local.z);
        coords[3].push(*w);
        reach = reach.max(a.norm() + w * origin.norm());
        weight = weight.max(*w);
    }
    let [x, y, z, w] = &coords;
    let g = implicit.along([x, y, z, w], binomials);
    let degree = g.len().saturating_sub(1);
    // A derivative's coefficient is `degree` times a difference of two
    // of `g`'s.
    let floor = BERNSTEIN_ROUNDING * implicit.magnitude(reach, weight) * 2.0 * degree as f64;
    sign_change_candidates(&derivative(&g), floor)
}
