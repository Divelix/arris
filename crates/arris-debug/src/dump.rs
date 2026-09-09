//! The deterministic text dump of a body (`docs/DATA-MODEL.md` §Native
//! format, last paragraph): what a fixture stores as `dump.txt` and what
//! the tests diff. Not a format — it has no reader.

use core::fmt::Write;

use arris_geom::{Curve, Curve2, NurbsCurve, NurbsCurve2, NurbsSurface, Surface};
use arris_math::{Frame, Frame2, Point2, Point3, Vec2, Vec3};
use arris_topo::entity::EdgeGeometry;
use arris_topo::{Body, Model, NotFound};

/// Decimal places every number in the dump is rounded to. Twelve is far
/// below any model tolerance and above the rounding noise of coordinates
/// up to the thousands, so two builds that differ only by an ulp dump
/// identically and a real difference shows.
pub const DUMP_DECIMALS: usize = 12;

/// The dump of `body`: the model's precision, then the body's shells,
/// faces, loops and coedges depth-first in iteration order with every
/// effective orientation and every surface and pcurve written out, then
/// the edges with their curves and the vertices in iteration order, then
/// the Euler line. Byte-identical for two models built by the same calls
/// on any platform, since every number is fixed to [`DUMP_DECIMALS`]
/// places and ids and orders are the arena's. A reference that does not
/// resolve is written as `<id> ?` so the dump of an invalid body still
/// says where it is invalid. Errors: the body does not resolve.
///
/// ```
/// use arris_debug::{dump_text, sample};
/// use arris_topo::Model;
///
/// let mut m = Model::default();
/// let body = sample::cylinder(&mut m, 4.0, 12.0).unwrap();
/// let text = dump_text(&m, body).unwrap();
/// assert!(text.contains("coedge +e1 "), "the seam, walked up");
/// assert!(text.contains("coedge -e1 "), "and walked down");
/// assert!(text.ends_with("euler 2/3/3/3/1 g0 = 0\n"));
/// ```
pub fn dump_text(model: &Model, body: Body) -> Result<String, NotFound> {
    let entity = model.body(body.id)?;
    let mut out = String::new();
    let p = model.precision();
    let _ = writeln!(
        out,
        "precision default {} min {} max {} angular {} parametric {} samples {}",
        num(p.default_tolerance),
        num(p.min_tolerance),
        num(p.max_tolerance),
        num(p.angular_tolerance),
        num(p.parametric_tolerance),
        p.check_samples
    );
    let _ = writeln!(out, "body {body} {}", entity.kind());
    let mut seen_faces = std::collections::BTreeSet::new();
    for shell in model.shells(body)? {
        let _ = writeln!(out, "  shell {shell}");
        let Ok(shell_entity) = model.shell(shell.id) else {
            continue;
        };
        for face_use in shell_entity.faces() {
            let face = face_use.oriented_by(shell.orientation);
            let Ok(face_entity) = model.face(face.id) else {
                let _ = writeln!(out, "    face {face} ?");
                continue;
            };
            if !seen_faces.insert(face.id) {
                let _ = writeln!(out, "    face {face} (above)");
                continue;
            }
            let _ = writeln!(
                out,
                "    face {face} {} tol {}",
                face_entity.surface(),
                num(face_entity.tolerance())
            );
            match model.surface(face_entity.surface()) {
                Ok(s) => {
                    let _ = writeln!(out, "      surface {}", surface(s));
                }
                Err(_) => {
                    let _ = writeln!(out, "      surface ?");
                }
            }
            for (li, l) in face_entity.loops().iter().enumerate() {
                let _ = writeln!(out, "      loop {li}");
                for coedge in l.coedges() {
                    let edge = coedge.edge_use().oriented_by(face.orientation);
                    match model.curve2(coedge.pcurve()) {
                        Ok(pc) => {
                            let _ = writeln!(
                                out,
                                "        coedge {edge} {} {}",
                                coedge.pcurve(),
                                curve2(pc)
                            );
                        }
                        Err(_) => {
                            let _ = writeln!(out, "        coedge {edge} {} ?", coedge.pcurve());
                        }
                    }
                }
            }
        }
    }
    for edge in entity.free_edges() {
        let _ = writeln!(out, "  free edge {}", edge.oriented_by(body.orientation));
    }
    for vertex in entity.free_vertices() {
        let _ = writeln!(out, "  free vertex {vertex}");
    }
    let _ = writeln!(out, "edges");
    for edge in model.edges(body)? {
        let Ok(e) = model.edge(edge.id) else {
            let _ = writeln!(out, "  {edge} ?");
            continue;
        };
        match e.geometry() {
            EdgeGeometry::Curve { curve, range } => {
                let _ = writeln!(
                    out,
                    "  {edge} {} -> {} {curve} [{}, {}] tol {}",
                    e.start(),
                    e.end(),
                    num(range.lo()),
                    num(range.hi()),
                    num(e.tolerance())
                );
                match model.curve(curve) {
                    Ok(c) => {
                        let _ = writeln!(out, "    curve {}", curve3(c));
                    }
                    Err(_) => {
                        let _ = writeln!(out, "    curve ?");
                    }
                }
            }
            EdgeGeometry::Degenerate { range } => {
                let _ = writeln!(
                    out,
                    "  {edge} {} -> {} degenerate [{}, {}] tol {}",
                    e.start(),
                    e.end(),
                    num(range.lo()),
                    num(range.hi()),
                    num(e.tolerance())
                );
            }
        }
    }
    let _ = writeln!(out, "vertices");
    for vertex in model.vertices(body)? {
        match model.vertex(vertex.id) {
            Ok(v) => {
                let _ = writeln!(
                    out,
                    "  {} {} tol {}",
                    vertex.id,
                    point3(v.point()),
                    num(v.tolerance())
                );
            }
            Err(_) => {
                let _ = writeln!(out, "  {} ?", vertex.id);
            }
        }
    }
    let _ = writeln!(out, "{}", euler_line(model, body)?);
    Ok(out)
}

/// The Euler line of a body: `V/E/F/L/S g<G> = <residual>`, with `G` the
/// genus the counts imply through `V − E + F − (L − F) − 2(S − G) = 0`
/// and the residual what is left once `G` is rounded down to an integer —
/// `0` for a line that closes, `1` for one that does not
/// (`docs/DATA-MODEL.md` §Euler–Poincaré). Errors: the body does not
/// resolve.
pub fn euler_line(model: &Model, body: Body) -> Result<String, NotFound> {
    let c = model.closure(body)?;
    let loops: usize = c
        .faces
        .iter()
        .filter_map(|&f| model.face(f).ok())
        .map(|f| f.loops().len())
        .sum();
    let (v, e, f, l, s) = (
        c.vertices.len() as i64,
        c.edges.len() as i64,
        c.faces.len() as i64,
        loops as i64,
        c.shells.len() as i64,
    );
    // V − E + F − (L − F) − 2(S − G) = 0  ⇒  2G = 2S − (V − E + 2F − L).
    let x = v - e + 2 * f - l;
    let genus = s - x.div_euclid(2);
    let residual = x.rem_euclid(2);
    Ok(format!("euler {v}/{e}/{f}/{l}/{s} g{genus} = {residual}"))
}

/// `x` fixed to [`DUMP_DECIMALS`] places with trailing zeros trimmed, so
/// `40` is `40`, `1e-7` is `0.0000001`, and anything within half a unit
/// of the last place of zero — a negative zero included — is `0`.
pub fn num(x: f64) -> String {
    if !x.is_finite() {
        return format!("{x}");
    }
    let s = format!("{x:.DUMP_DECIMALS$}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    match s {
        "" | "-0" => "0".to_string(),
        s => s.to_string(),
    }
}

fn point3(p: Point3) -> String {
    format!("({}, {}, {})", num(p.x), num(p.y), num(p.z))
}

fn vec3(v: Vec3) -> String {
    format!("({}, {}, {})", num(v.x), num(v.y), num(v.z))
}

fn point2(p: Point2) -> String {
    format!("({}, {})", num(p.x), num(p.y))
}

fn vec2(v: Vec2) -> String {
    format!("({}, {})", num(v.x), num(v.y))
}

fn frame(f: &Frame) -> String {
    format!(
        "origin {} x {} y {} z {}",
        point3(f.origin()),
        vec3(f.x().into_inner()),
        vec3(f.y().into_inner()),
        vec3(f.z().into_inner())
    )
}

fn frame2(f: &Frame2) -> String {
    format!(
        "origin {} x {} y {}",
        point2(f.origin()),
        vec2(f.x().into_inner()),
        vec2(f.y().into_inner())
    )
}

fn list(xs: impl IntoIterator<Item = String>) -> String {
    xs.into_iter().collect::<Vec<_>>().join(" ")
}

fn surface(s: &Surface) -> String {
    match s {
        Surface::Plane { frame: f } => format!("plane {}", frame(f)),
        Surface::Cylinder { frame: f, radius } => {
            format!("cylinder {} radius {}", frame(f), num(*radius))
        }
        Surface::Cone {
            frame: f,
            radius,
            half_angle,
        } => format!(
            "cone {} radius {} half_angle {}",
            frame(f),
            num(*radius),
            num(*half_angle)
        ),
        Surface::Sphere { frame: f, radius } => {
            format!("sphere {} radius {}", frame(f), num(*radius))
        }
        Surface::Torus {
            frame: f,
            major_radius,
            minor_radius,
        } => format!(
            "torus {} major {} minor {}",
            frame(f),
            num(*major_radius),
            num(*minor_radius)
        ),
        Surface::Nurbs(n) => nurbs_surface(n),
    }
}

fn nurbs_surface(n: &NurbsSurface) -> String {
    let [p, q] = n.degree();
    let [ku, kv] = n.knots();
    let [cu, cv] = n.counts();
    format!(
        "nurbs degree {p}x{q} counts {cu}x{cv} knots_u [{}] knots_v [{}] points [{}] weights [{}]",
        list(ku.iter().map(|&k| num(k))),
        list(kv.iter().map(|&k| num(k))),
        list(n.control_points().iter().map(|&p| point3(p))),
        list(n.weights().iter().map(|&w| num(w)))
    )
}

fn curve3(c: &Curve) -> String {
    match c {
        Curve::Line { origin, direction } => {
            format!(
                "line origin {} direction {}",
                point3(*origin),
                vec3(direction.into_inner())
            )
        }
        Curve::Circle { frame: f, radius } => {
            format!("circle {} radius {}", frame(f), num(*radius))
        }
        Curve::Ellipse {
            frame: f,
            major_radius,
            minor_radius,
        } => format!(
            "ellipse {} major {} minor {}",
            frame(f),
            num(*major_radius),
            num(*minor_radius)
        ),
        Curve::Nurbs(n) => nurbs_curve(n),
    }
}

fn nurbs_curve(n: &NurbsCurve) -> String {
    format!(
        "nurbs degree {} knots [{}] points [{}] weights [{}]",
        n.degree(),
        list(n.knots().iter().map(|&k| num(k))),
        list(n.control_points().iter().map(|&p| point3(p))),
        list(n.weights().iter().map(|&w| num(w)))
    )
}

fn curve2(c: &Curve2) -> String {
    match c {
        Curve2::Line { origin, direction } => {
            format!(
                "line origin {} direction {}",
                point2(*origin),
                vec2(direction.into_inner())
            )
        }
        Curve2::Circle { frame: f, radius } => {
            format!("circle {} radius {}", frame2(f), num(*radius))
        }
        Curve2::Ellipse {
            frame: f,
            major_radius,
            minor_radius,
        } => format!(
            "ellipse {} major {} minor {}",
            frame2(f),
            num(*major_radius),
            num(*minor_radius)
        ),
        Curve2::Nurbs(n) => nurbs_curve2(n),
    }
}

fn nurbs_curve2(n: &NurbsCurve2) -> String {
    format!(
        "nurbs degree {} knots [{}] points [{}] weights [{}]",
        n.degree(),
        list(n.knots().iter().map(|&k| num(k))),
        list(n.control_points().iter().map(|&p| point2(p))),
        list(n.weights().iter().map(|&w| num(w)))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_fixed_and_trimmed() {
        assert_eq!(num(40.0), "40");
        assert_eq!(num(1e-7), "0.0000001");
        assert_eq!(num(1e-12), "0.000000000001");
        assert_eq!(num(-0.0), "0");
        assert_eq!(num(-2.4e-16), "0");
        assert_eq!(num(4e-13), "0");
        assert_eq!(num(6e-13), "0.000000000001");
        assert_eq!(num(-1.5), "-1.5");
        assert_eq!(num(core::f64::consts::PI), "3.14159265359");
        assert_eq!(num(f64::INFINITY), "inf");
        assert_eq!(num(f64::NAN), "NaN");
    }
}
