//! The geometry oracle: Arris's evaluations, projections and
//! intersections against Open CASCADE's for every `tests/fixtures/geom/`
//! fixture — evaluations and projected parameters to 1e-9 relative, the
//! intersection types exactly, and the oracle's sampled points on Arris's
//! curves to 1e-9 (`docs/plans/m1-geometry.md` step 7). The oracle is
//! the parametrisation's ground truth (`docs/02-data-model.md`
//! §Conventions); a mismatch here is fixed in Arris.

use std::collections::BTreeMap;

use arris_debug::fixtures::geom::{
    Evaluation, GeomFixture, PairResult, Projection, build_curve, build_surface,
};
use arris_debug::fixtures::{Kind, corpus, kind_of};
use arris_geom::{
    Curve, CurveSurfaceIntersection, Surface, SurfaceIntersection, intersect_curve_surface,
    intersect_surfaces,
};
use arris_math::{Point3, Precision, Tolerance, Vec3};

/// Relative agreement on every evaluated and projected quantity: the
/// oracle's own rounding, several orders above it.
const REL: f64 = 1e-9;
/// Where a curve touches a surface the crossing's location is
/// conditioned as the square root of the rounding — a touch perturbed
/// by 1e-15 in distance moves its point by 1e-7·√R — so the oracle's
/// points at a touch (it reports none, one or two, having no tolerance
/// there) are held to this against Arris's tangent hit.
const TOUCH: f64 = 1e-6;

fn tol() -> Tolerance {
    Precision::DEFAULT.tolerance()
}

fn p3(a: &[f64; 3]) -> Point3 {
    Point3::new(a[0], a[1], a[2])
}

fn v3(a: &[f64; 3]) -> Vec3 {
    Vec3::new(a[0], a[1], a[2])
}

/// `|a − b| ≤ REL · max(1, |a|, |b|)`.
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= REL * a.abs().max(b.abs()).max(1.0)
}

fn close3(a: Vec3, b: Vec3) -> bool {
    (a - b).norm() <= REL * a.norm().max(b.norm()).max(1.0)
}

/// Equal modulo `period`, when there is one.
fn close_param(a: f64, b: f64, period: Option<f64>) -> bool {
    match period {
        Some(p) => {
            let d = (a - b).rem_euclid(p);
            d.min(p - d) <= REL * a.abs().max(b.abs()).max(1.0)
        }
        None => close(a, b),
    }
}

struct Built {
    surfaces: BTreeMap<String, Surface>,
    curves: BTreeMap<String, Curve>,
}

fn build(f: &GeomFixture) -> Built {
    let r = &f.recipe;
    Built {
        surfaces: r
            .surfaces
            .iter()
            .map(|(n, s)| (n.clone(), build_surface(n, s, &r.params).unwrap()))
            .collect(),
        curves: r
            .curves
            .iter()
            .map(|(n, c)| (n.clone(), build_curve(n, c, &r.params).unwrap()))
            .collect(),
    }
}

fn geometry_fixtures() -> Vec<GeomFixture> {
    corpus()
        .iter()
        .filter(|d| kind_of(d).unwrap() == Kind::Geometry)
        .map(|d| arris_debug::fixtures::geom::load(d).unwrap())
        .collect()
}

fn check_surface_evaluation(name: &str, s: &Surface, e: &Evaluation, errors: &mut Vec<String>) {
    let Evaluation::Surface {
        at,
        point,
        du,
        dv,
        duu,
        duv,
        dvv,
    } = e
    else {
        errors.push(format!(
            "{name}: a surface evaluation carries a curve result"
        ));
        return;
    };
    let ours = s.eval(at[0], at[1]);
    for (what, a, b) in [
        ("point", ours.point.coords, v3(point)),
        ("du", ours.du, v3(du)),
        ("dv", ours.dv, v3(dv)),
        ("duu", ours.duu, v3(duu)),
        ("duv", ours.duv, v3(duv)),
        ("dvv", ours.dvv, v3(dvv)),
    ] {
        if !close3(a, b) {
            errors.push(format!("{name} at {at:?}: {what} {a:?} vs oracle {b:?}"));
        }
    }
}

fn check_curve_evaluation(name: &str, c: &Curve, e: &Evaluation, errors: &mut Vec<String>) {
    let Evaluation::Curve { at, point, d1, d2 } = e else {
        errors.push(format!(
            "{name}: a curve evaluation carries a surface result"
        ));
        return;
    };
    let ours = c.eval(*at);
    for (what, a, b) in [
        ("point", ours.point.coords, v3(point)),
        ("d1", ours.d1, v3(d1)),
        ("d2", ours.d2, v3(d2)),
    ] {
        if !close3(a, b) {
            errors.push(format!("{name} at {at}: {what} {a:?} vs oracle {b:?}"));
        }
    }
}

fn check_surface_projection(name: &str, s: &Surface, p: &Projection, errors: &mut Vec<String>) {
    let Some(uv) = p.uv else {
        errors.push(format!("{name}: a surface projection without uv"));
        return;
    };
    let ours = match s.project(p3(&p.point)) {
        Ok(o) => o,
        Err(e) => {
            errors.push(format!("{name} projecting {:?}: {e}", p.point));
            return;
        }
    };
    let [pu, pv] = s.period();
    if !close_param(ours.uv.x, uv[0], pu) || !close_param(ours.uv.y, uv[1], pv) {
        errors.push(format!(
            "{name} projecting {:?}: uv {:?} vs oracle {uv:?}",
            p.point, ours.uv
        ));
    }
    if !close3(ours.point.coords, v3(&p.nearest)) || !close(ours.distance, p.distance) {
        errors.push(format!(
            "{name} projecting {:?}: {:?} at {} vs oracle {:?} at {}",
            p.point, ours.point, ours.distance, p.nearest, p.distance
        ));
    }
}

fn check_curve_projection(name: &str, c: &Curve, p: &Projection, errors: &mut Vec<String>) {
    let Some(t) = p.t else {
        errors.push(format!("{name}: a curve projection without t"));
        return;
    };
    let ours = match c.project(p3(&p.point)) {
        Ok(o) => o,
        Err(e) => {
            errors.push(format!("{name} projecting {:?}: {e}", p.point));
            return;
        }
    };
    if !close_param(ours.t, t, c.period()) {
        errors.push(format!(
            "{name} projecting {:?}: t {} vs oracle {t}",
            p.point, ours.t
        ));
    }
    if !close3(ours.point.coords, v3(&p.nearest)) || !close(ours.distance, p.distance) {
        errors.push(format!(
            "{name} projecting {:?}: {:?} at {} vs oracle {:?} at {}",
            p.point, ours.point, ours.distance, p.nearest, p.distance
        ));
    }
}

/// Arris's type name for a surface pair, in the oracle's vocabulary.
fn surface_type(r: &SurfaceIntersection) -> (String, &[Curve]) {
    match r {
        SurfaceIntersection::Empty => ("empty".into(), &[]),
        SurfaceIntersection::Coincident => ("coincident".into(), &[]),
        SurfaceIntersection::Transversal(c) | SurfaceIntersection::Tangent(c) => {
            let kinds: std::collections::BTreeSet<String> =
                c.iter().map(|c| c.kind().to_string()).collect();
            (kinds.into_iter().collect::<Vec<_>>().join("+"), c)
        }
    }
}

fn check_surface_pair(a: &Surface, b: &Surface, res: &PairResult, errors: &mut Vec<String>) {
    let label = format!("{} vs {}", res.a, res.b);
    let r = match intersect_surfaces(a, b, tol()) {
        Ok(r) => r,
        Err(e) => {
            errors.push(format!("{label}: {e}"));
            return;
        }
    };
    let (kind, curves) = surface_type(&r);
    if kind != res.kind || curves.len() != res.curves.len() {
        errors.push(format!(
            "{label}: {kind} with {} curves vs oracle {} with {}",
            curves.len(),
            res.kind,
            res.curves.len()
        ));
        return;
    }
    // Each oracle curve is one of ours: every sampled point lies on some
    // Arris curve of the same kind, and each Arris curve carries one
    // oracle curve.
    let mut taken = vec![false; curves.len()];
    for sample in &res.curves {
        let on = |c: &Curve| {
            c.kind().to_string() == sample.kind
                && sample.points.iter().all(|p| {
                    c.project(p3(p))
                        .map(|proj| proj.distance <= REL * p3(p).coords.norm().max(1.0))
                        .unwrap_or(false)
                })
        };
        match curves.iter().enumerate().find(|(i, c)| !taken[*i] && on(c)) {
            Some((i, _)) => taken[i] = true,
            None => errors.push(format!(
                "{label}: the oracle's {} through {:?} is not one of {curves:?}",
                sample.kind, sample.points[0]
            )),
        }
    }
}

fn check_curve_pair(c: &Curve, s: &Surface, res: &PairResult, errors: &mut Vec<String>) {
    let label = format!("{} vs {}", res.a, res.b);
    let r = match intersect_curve_surface(c, s, tol()) {
        Ok(r) => r,
        Err(e) => {
            errors.push(format!("{label}: {e}"));
            return;
        }
    };
    let hits = match (&r, res.kind.as_str()) {
        (CurveSurfaceIntersection::Coincident, "coincident") => return,
        (CurveSurfaceIntersection::Points(h), "points") => h,
        _ => {
            errors.push(format!("{label}: {r:?} vs oracle {}", res.kind));
            return;
        }
    };
    // Every oracle hit is one of ours: a transversal hit to REL, a touch
    // to TOUCH; every transversal hit of ours is an oracle hit. A touch
    // may have no oracle hit at all.
    for h in &res.hits {
        let p = p3(&h.point);
        let scale = p.coords.norm().max(1.0);
        let matched = hits.iter().any(|ours| {
            let bound = if ours.tangent { TOUCH } else { REL * scale };
            (ours.point - p).norm() <= bound
                && (ours.t - h.t).abs().min(turn_diff(ours.t, h.t, c)) <= bound
        });
        if !matched {
            errors.push(format!(
                "{label}: the oracle's hit at t = {} {:?} is not among {hits:?}",
                h.t, h.point
            ));
        }
    }
    for ours in hits.iter().filter(|h| !h.tangent) {
        let scale = ours.point.coords.norm().max(1.0);
        if !res
            .hits
            .iter()
            .any(|h| (ours.point - p3(&h.point)).norm() <= REL * scale)
        {
            errors.push(format!(
                "{label}: our hit at t = {} {:?} is not among the oracle's {:?}",
                ours.t, ours.point, res.hits
            ));
        }
    }
}

/// `|a − b|` modulo the curve's period, or infinity for a line so the
/// plain difference wins.
fn turn_diff(a: f64, b: f64, c: &Curve) -> f64 {
    match c.period() {
        Some(p) => {
            let d = (a - b).rem_euclid(p);
            d.min(p - d)
        }
        None => f64::INFINITY,
    }
}

#[test]
fn every_geometry_fixture_matches_the_oracle() {
    let fixtures = geometry_fixtures();
    assert!(
        fixtures.len() >= 2,
        "expected geom/analytic-eval and geom/c1-intersections"
    );
    let mut errors = Vec::new();
    for f in &fixtures {
        let built = build(f);
        for (sample, result) in f.recipe.samples.iter().zip(&f.expected.samples) {
            let name = format!("{}: {}", f.name, sample.of);
            if let Some(s) = built.surfaces.get(&sample.of) {
                for e in &result.evaluations {
                    check_surface_evaluation(&name, s, e, &mut errors);
                }
                for p in &result.projections {
                    check_surface_projection(&name, s, p, &mut errors);
                }
            } else if let Some(c) = built.curves.get(&sample.of) {
                for e in &result.evaluations {
                    check_curve_evaluation(&name, c, e, &mut errors);
                }
                for p in &result.projections {
                    check_curve_projection(&name, c, p, &mut errors);
                }
            }
        }
        for result in &f.expected.pairs {
            let Some(b) = built.surfaces.get(&result.b) else {
                errors.push(format!("{}: {} is not a surface", f.name, result.b));
                continue;
            };
            if let Some(a) = built.surfaces.get(&result.a) {
                check_surface_pair(a, b, result, &mut errors);
            } else if let Some(c) = built.curves.get(&result.a) {
                check_curve_pair(c, b, result, &mut errors);
            }
        }
    }
    assert!(
        errors.is_empty(),
        "{} mismatches:\n{}",
        errors.len(),
        errors.join("\n")
    );
}

/// What Arris says about every pair of `geom/c1-intersections`, by name:
/// the case each was built as. The oracle comparison above is one-sided
/// at a touch (it may report nothing), so the classification is pinned
/// here, where a tangent case cannot pass vacuously.
#[test]
fn the_c1_intersection_cases_classify_as_built() {
    let f = geometry_fixtures()
        .into_iter()
        .find(|f| f.name == "geom/c1-intersections")
        .expect("geom/c1-intersections");
    let built = build(&f);
    // Surface pairs: the type and the curve count; curve pairs: the
    // transversal and tangent hit counts, or coincident.
    type SurfaceCase = (&'static str, &'static str, &'static str, usize);
    type CurveCase = (&'static str, &'static str, Option<(usize, usize)>);
    let surface_cases: &[SurfaceCase] = &[
        ("cap", "cyl", "circle", 1),
        ("oblique", "cyl", "ellipse", 1),
        ("chordal", "cyl", "line", 2),
        ("touching", "cyl", "tangent line", 1),
        ("clear", "cyl", "empty", 0),
        ("cap", "oblique", "line", 1),
        ("cap", "cap_lifted", "empty", 0),
        ("cap", "cap_flipped", "coincident", 0),
    ];
    let curve_cases: &[CurveCase] = &[
        ("skewer", "cap", Some((1, 0))),
        ("hover", "cap", Some((0, 0))),
        ("flat", "cap", None),
        ("chord", "cyl", Some((2, 0))),
        ("grazing", "cyl", Some((0, 1))),
        ("miss", "cyl", Some((0, 0))),
        ("ruling", "cyl", None),
        ("offside", "cyl", Some((0, 0))),
        ("crossing_ring", "cap", Some((2, 0))),
        ("kissing_ring", "cap", Some((0, 1))),
        ("hovering_ring", "cap", Some((0, 0))),
        ("lying_ring", "cap", None),
        ("floating_ring", "cap", Some((0, 0))),
        ("parallel", "cyl", None),
        ("meridional", "cyl", Some((4, 0))),
        ("two", "cyl", Some((2, 0))),
        ("outside_touch", "cyl", Some((0, 1))),
        ("inside_touch", "cyl", Some((0, 1))),
        ("outside_miss", "cyl", Some((0, 0))),
        ("inside_miss", "cyl", Some((0, 0))),
        ("section", "cyl", None),
        ("section", "oblique", None),
        ("section", "cut_minor", Some((2, 0))),
        ("section", "touch_major", Some((0, 1))),
        ("section", "clear_major", Some((0, 0))),
        ("meridional_ellipse", "cyl", Some((4, 0))),
        ("grazing_ellipse", "cyl", Some((0, 2))),
        ("inner_ellipse", "cyl", Some((0, 0))),
    ];
    assert_eq!(
        surface_cases.len() + curve_cases.len(),
        f.recipe.pairs.len(),
        "every pair of the fixture is pinned here"
    );
    for (a, b, kind, count) in surface_cases {
        let r = intersect_surfaces(&built.surfaces[*a], &built.surfaces[*b], tol()).unwrap();
        let (got, curves) = surface_type(&r);
        let got = if matches!(r, SurfaceIntersection::Tangent(_)) {
            format!("tangent {got}")
        } else {
            got
        };
        assert_eq!(
            (got.as_str(), curves.len()),
            (*kind, *count),
            "{a} vs {b}: {r:?}"
        );
    }
    for (a, b, expected) in curve_cases {
        let r = intersect_curve_surface(&built.curves[*a], &built.surfaces[*b], tol()).unwrap();
        let got = match &r {
            CurveSurfaceIntersection::Coincident => None,
            CurveSurfaceIntersection::Points(h) => Some((
                h.iter().filter(|h| !h.tangent).count(),
                h.iter().filter(|h| h.tangent).count(),
            )),
        };
        assert_eq!(got, *expected, "{a} vs {b}: {r:?}");
    }
}
