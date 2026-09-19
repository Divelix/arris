//! The traced section of two quadrics as a `Meets` result: every branch
//! of [`crate::trace_quadrics`] fitted to a `Curve::Nurbs`, every
//! singular point a point of the result (ADR-0018, `docs/DATA-MODEL.md`
//! §Curves).

use arris_math::{Aabb, Tolerance};

use crate::{
    Curve, GeomError, MeetCurve, MeetKind, MeetPoint, SectionBranch, Surface, SurfaceIntersection,
    fit_curve, fit_curve_periodic, trace_quadrics,
};

/// The fraction of the pair's `tol.linear` a fitted section curve is
/// held to, measured from each of the two surfaces. Each face's pcurve
/// is fitted afterwards to the 3D curve (`pcurve_on`), and can come no
/// nearer the 3D curve than the 3D curve is to that face; that fit is
/// accepted at half its tolerance (the fit's own margin), so the 3D
/// curve has to sit well inside that half for the pcurve to land within
/// `tol.linear` too. A quarter leaves the pcurve the other quarter.
/// That is what keeps a section edge's tolerance at its faces' and never
/// above (`docs/DATA-MODEL.md` §Tolerances). A ratio between two fits,
/// not a tolerance.
pub const SECTION_FIT_FRACTION: f64 = 0.25;

/// The degree of a fitted section curve. Measured on metre-scale
/// cylinder pairs (radii 1 to 2, crossing, skew and tilted axes) at the
/// default tolerance: a loop takes 200 to 390 control points at degree
/// 3, 70 to 160 at degree 4 and 60 to 120 at degree 5 — the quintic is
/// the smallest, and the degree the pcurves fitted to it take
/// ([`crate::PCURVE_FIT_DEGREE`]). A structural choice, not a
/// tolerance.
pub const SECTION_FIT_DEGREE: usize = 5;

/// The section of two quadrics, one of them ruled, inside `within`, as
/// the intersector returns it: each traced branch a crossing curve, in
/// the tracer's order and orientation, fitted at the branch's own
/// parameter — periodic when the branch is closed — until neither
/// surface is farther than [`SECTION_FIT_FRACTION`] of `tol.linear`
/// from it beyond the exact branch's own distance (which is rounding,
/// except within a singular point's reach, where the tracer lets the
/// branch be within `tol.linear` of the other surface); each singular
/// point a point of the result, `Touch` where it is isolated and
/// `Crossing` where branches end at it. `Empty` when there is neither.
pub(crate) fn traced(
    a: &Surface,
    b: &Surface,
    within: &Aabb,
    tol: Tolerance,
) -> Result<SurfaceIntersection, GeomError> {
    let trace = trace_quadrics(a, b, within, tol)?;
    let curves = trace
        .branches()
        .iter()
        .map(|branch| {
            fitted(branch, a, b, tol).map(|curve| MeetCurve {
                curve,
                kind: MeetKind::Crossing,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let points: Vec<MeetPoint> = trace
        .points()
        .iter()
        .map(|p| MeetPoint {
            point: p.point,
            kind: if p.isolated {
                MeetKind::Touch
            } else {
                MeetKind::Crossing
            },
        })
        .collect();
    Ok(if curves.is_empty() && points.is_empty() {
        SurfaceIntersection::Empty
    } else {
        SurfaceIntersection::Meets { curves, points }
    })
}

/// One branch as a `Curve::Nurbs`.
fn fitted(
    branch: &SectionBranch,
    a: &Surface,
    b: &Surface,
    tol: Tolerance,
) -> Result<Curve, GeomError> {
    let off = |s: &Surface, p| s.project(p).map_or(f64::INFINITY, |on| on.distance);
    let deviation = |t: f64, q| {
        let p = branch.point(t);
        [a, b]
            .iter()
            .map(|s| (off(s, q) - off(s, p)).max(0.0))
            .fold(0.0, f64::max)
    };
    let f = |t: f64| branch.point(t);
    let target = SECTION_FIT_FRACTION * tol.linear;
    let fit = if branch.is_closed() {
        fit_curve_periodic(f, branch.domain(), SECTION_FIT_DEGREE, deviation, target)
    } else {
        fit_curve(f, branch.domain(), SECTION_FIT_DEGREE, deviation, target)
    }?;
    Ok(Curve::Nurbs(fit))
}
