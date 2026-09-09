//! The text dump and the sample bodies (`docs/DATA-MODEL.md` §Native
//! format, last paragraph; §Seams).

use arris_debug::{dump_text, euler_line, sample};
use arris_math::Point3;
use arris_topo::{BodyId, Curve2Id, CurveId, EdgeId, FaceId, Model, ShellId, SurfaceId, VertexId};

fn cylinder_dump() -> (Model, String) {
    let mut m = Model::default();
    let b = sample::cylinder(&mut m, 4.0, 12.0).unwrap();
    let text = dump_text(&m, b).unwrap();
    (m, text)
}

fn box_dump() -> (Model, String) {
    let mut m = Model::default();
    let b = sample::cuboid(&mut m, Point3::origin(), Point3::new(40.0, 30.0, 10.0)).unwrap();
    let text = dump_text(&m, b).unwrap();
    (m, text)
}

#[test]
fn the_seam_edge_is_listed_twice_with_opposite_signs() {
    let (_, text) = cylinder_dump();
    let coedges: Vec<&str> = text
        .lines()
        .filter(|l| l.trim_start().starts_with("coedge "))
        .collect();
    assert_eq!(coedges.len(), 6, "four on the wall, one per cap:\n{text}");
    let seam_up = coedges.iter().filter(|l| l.contains("coedge +e1 ")).count();
    let seam_down = coedges.iter().filter(|l| l.contains("coedge -e1 ")).count();
    assert_eq!((seam_up, seam_down), (1, 1), "{text}");
    // The two seam pcurves are at u = 2π and u = 0.
    assert!(
        text.contains("coedge +e1 p1 line origin (6.283185307180, 0)")
            || text.contains("coedge +e1 p1 line origin (6.28318530718, 0)"),
        "{text}"
    );
    assert!(
        text.contains("coedge -e1 p3 line origin (0, 0) direction (0, 1)"),
        "{text}"
    );
    assert!(text.ends_with("euler 2/3/3/3/1 g0 = 0\n"), "{text}");
    // The edges section lists the seam once, with its first use's sign.
    let edges: Vec<&str> = text
        .lines()
        .skip_while(|l| *l != "edges")
        .skip(1)
        .take_while(|l| *l != "vertices")
        .filter(|l| l.starts_with("  "))
        .filter(|l| !l.starts_with("    "))
        .collect();
    assert_eq!(edges.len(), 3, "{text}");
    assert!(edges[1].starts_with("  +e1 v0 -> v1 c1 [0, 12]"), "{text}");
    assert!(
        text.contains("face -f1 S1"),
        "the bottom cap is used reversed:\n{text}"
    );
}

#[test]
fn the_box_line_is_8_12_6_6() {
    let (m, text) = box_dump();
    assert!(text.ends_with("euler 8/12/6/6/1 g0 = 0\n"), "{text}");
    assert_eq!(
        euler_line(&m, arris_topo::Body::forward(BodyId::new(0, 0))).unwrap(),
        "euler 8/12/6/6/1 g0 = 0"
    );
    assert!(text.starts_with("precision default 0.0000001 min 0.000000000001 max 0.01 angular 0.000000000001 parametric 0.0000001 samples 23\nbody +b0 solid\n  shell +s0\n"), "{text}");
    assert_eq!(text.lines().filter(|l| l.contains("coedge ")).count(), 24);
    assert!(text.contains("  v7 (40, 30, 10) tol 0.0000001"), "{text}");
}

#[test]
fn two_fresh_builds_dump_byte_identically() {
    assert_eq!(cylinder_dump().1, cylinder_dump().1);
    assert_eq!(box_dump().1, box_dump().1);
    let mut both = Model::default();
    let first = sample::cylinder(&mut both, 4.0, 12.0).unwrap();
    let second = sample::cylinder(&mut both, 4.0, 12.0).unwrap();
    assert_ne!(
        dump_text(&both, first).unwrap(),
        dump_text(&both, second).unwrap(),
        "the ids differ, so the dumps do"
    );
}

/// Every token of the form `[+-]?<kind letter><index>[g<gen>]` in a dump
/// resolves in its model.
fn assert_ids_resolve(m: &Model, text: &str) {
    let mut checked = 0;
    for token in text.split(|c: char| c.is_whitespace() || "()[],".contains(c)) {
        let bare = token.trim_start_matches(['+', '-']);
        let mut chars = bare.chars();
        let Some(kind) = chars.next() else { continue };
        let rest: &str = chars.as_str();
        if !"vefsbcSp".contains(kind) || rest.is_empty() {
            continue;
        }
        let (index, generation) = match rest.split_once('g') {
            Some((i, g)) => (i, g),
            None => (rest, "0"),
        };
        let (Ok(index), Ok(generation)) = (index.parse::<u32>(), generation.parse::<u32>()) else {
            continue;
        };
        let ok = match kind {
            'v' => m.vertex(VertexId::new(index, generation)).is_ok(),
            'e' => m.edge(EdgeId::new(index, generation)).is_ok(),
            'f' => m.face(FaceId::new(index, generation)).is_ok(),
            's' => m.shell(ShellId::new(index, generation)).is_ok(),
            'b' => m.body(BodyId::new(index, generation)).is_ok(),
            'c' => m.curve(CurveId::new(index, generation)).is_ok(),
            'S' => m.surface(SurfaceId::new(index, generation)).is_ok(),
            'p' => m.curve2(Curve2Id::new(index, generation)).is_ok(),
            _ => unreachable!(),
        };
        assert!(ok, "{token} does not resolve:\n{text}");
        checked += 1;
    }
    assert!(
        checked > 20,
        "the tokenizer found only {checked} ids:\n{text}"
    );
}

#[test]
fn every_id_in_a_dump_resolves() {
    let (m, text) = cylinder_dump();
    assert_ids_resolve(&m, &text);
    let (m, text) = box_dump();
    assert_ids_resolve(&m, &text);
    assert!(!text.contains('?'), "nothing dangling:\n{text}");
}

#[test]
fn a_dangling_reference_is_marked_not_hidden() {
    use arris_topo::entity::{Body, Shell};
    let mut m = Model::default();
    let s = m
        .raw()
        .add_shell(Shell::new(vec![arris_topo::Face::forward(FaceId::new(
            9, 0,
        ))]));
    let b = m
        .raw()
        .add_body(Body::solid(vec![arris_topo::Shell::forward(s)]));
    let text = dump_text(&m, arris_topo::Body::forward(b)).unwrap();
    assert!(text.contains("    face +f9 ?\n"), "{text}");
    assert!(text.ends_with("euler 0/0/0/0/1 g1 = 0\n"), "{text}");
    assert!(dump_text(&m, arris_topo::Body::forward(BodyId::new(3, 0))).is_err());
}

#[test]
fn samples_refuse_bad_extents_and_leave_the_model_untouched() {
    let mut m = Model::default();
    assert!(sample::cylinder(&mut m, 0.0, 1.0).is_err());
    assert!(sample::cylinder(&mut m, 1.0, f64::NAN).is_err());
    assert!(sample::cuboid(&mut m, Point3::origin(), Point3::new(1.0, -1.0, 1.0)).is_err());
    assert!(m.vertex(VertexId::new(0, 0)).is_err());
    assert!(m.curve(CurveId::new(0, 0)).is_err());
    assert!(sample::unit_box(&mut m).is_ok());
}
