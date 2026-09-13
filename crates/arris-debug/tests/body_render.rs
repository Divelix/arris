//! A body to a mesh and a picture, and a face to its (u, v) domain
//! (`docs/ARCHITECTURE.md` §Formats and tools): the pictures the agent reads
//! before trusting the tessellator on a real shape, written under
//! `target/inspect/` like every render.

use arris_debug::render::colors;
use arris_debug::{Highlight, Raster, View, body, sample};
use arris_math::{Point2, Point3};
use arris_topo::Model;

/// Distinct colours that are neither background nor decoration.
fn face_colors(r: &Raster) -> Vec<[u8; 3]> {
    r.histogram()
        .into_iter()
        .map(|(c, _)| c)
        .filter(|c| ![colors::BACKGROUND, colors::LINE, colors::DOT].contains(c))
        .collect()
}

fn red_pixels(r: &Raster) -> usize {
    r.pixels()
        .iter()
        .filter(|c| c[0] > 150 && c[1] < 60 && c[2] < 60)
        .count()
}

/// A box has one planar surface per face, so [`body::mesh_of`]'s shading
/// gives one colour per visible face, exactly as a hand-built cube does
/// (`render.rs`'s own tests) — proof that a real body reaches the
/// rasteriser through [`body::mesh_of`] the same way.
#[test]
fn the_box_shows_three_faces_from_iso_and_one_from_top() {
    let mut m = Model::default();
    let b = sample::unit_box(&mut m).unwrap();
    let mesh = body::mesh_of(&m, b).unwrap();
    let iso = arris_debug::render(&mesh, &[], View::Iso, None);
    let top = arris_debug::render(&mesh, &[], View::Top, None);
    assert_eq!(face_colors(&iso).len(), 3, "{:?}", face_colors(&iso));
    assert_eq!(face_colors(&top).len(), 1);
}

/// The cylinder's wall is curved, so its own shading splits into several
/// shades — [`body::mesh_of`]'s picture is read by their highlight, not by
/// counting raw colours: the seam highlights a thin run of pixels, the
/// wall a much larger area, and the cap keeps its own colour throughout.
#[test]
fn the_cylinders_seam_and_wall_highlight_and_the_png_lands_under_inspect() {
    let mut m = Model::default();
    let cyl = sample::cylinder(&mut m, 4.0, 12.0).unwrap();
    let seam = m
        .edges(cyl)
        .unwrap()
        .iter()
        .find(|e| !m.edge(e.id).unwrap().is_closed())
        .expect("the wall's seam is the one edge with distinct ends")
        .id;
    let wall = m
        .faces(cyl)
        .unwrap()
        .iter()
        .find(|f| m.face(f.id).unwrap().loops()[0].coedges().len() == 4)
        .expect("the wall's loop is bottom, seam up, top, seam down")
        .id;

    let mesh = body::mesh_of(&m, cyl).unwrap();
    let plain = arris_debug::render(&mesh, &[], View::Iso, None);
    assert_eq!(red_pixels(&plain), 0);
    let black = plain
        .pixels()
        .iter()
        .filter(|&&c| c == colors::LINE)
        .count();
    assert!(black > 0, "the rim and the seam are drawn");

    let hi_edge = arris_debug::render(&mesh, &[], View::Iso, Some(Highlight::Edge(seam)));
    let hi_face = arris_debug::render(&mesh, &[], View::Iso, Some(Highlight::Face(wall)));
    let (edge_red, face_red) = (red_pixels(&hi_edge), red_pixels(&hi_face));
    assert!(edge_red > 0, "the seam highlights a run of pixels");
    assert!(
        face_red > 10 * edge_red,
        "the whole wall is a much larger area than its one seam ({face_red} vs {edge_red})"
    );
    assert!(
        face_colors(&hi_face).len() > 1,
        "the cap keeps its own colour while the wall turns red"
    );

    let path = body::render_body(&m, cyl, View::Iso, None, "cylinder-iso").unwrap();
    assert!(path.starts_with(arris_debug::render::inspect_dir()));
    assert!(path.exists());
}

/// The (u, v) picture reads the face independently of the 3D mesh: the
/// cylinder wall's domain is the rectangle `[0, 2π] × [0, h]` filled
/// through its middle, and the frame's two-loop top face is an outer
/// filled ring with its window left open.
#[test]
fn render_domain_shows_the_walls_rectangle_and_the_frames_hole() {
    let mut m = Model::default();
    let cyl = sample::cylinder(&mut m, 4.0, 12.0).unwrap();
    let wall = *m
        .faces(cyl)
        .unwrap()
        .iter()
        .find(|f| m.face(f.id).unwrap().loops()[0].coedges().len() == 4)
        .unwrap();
    let path = body::render_domain(&m, wall, "wall-domain").unwrap();
    let img = image::open(&path).unwrap().to_rgb8();
    assert_ne!(
        img.get_pixel(400, 300).0,
        colors::BACKGROUND,
        "the rectangle is filled through its middle"
    );

    let mut fm = Model::default();
    let frame = sample::frame(
        &mut fm,
        Point3::origin(),
        Point3::new(40.0, 30.0, 10.0),
        Point2::new(10.0, 10.0),
        Point2::new(30.0, 20.0),
    )
    .unwrap();
    let two_loop = *fm
        .faces(frame)
        .unwrap()
        .iter()
        .find(|f| fm.face(f.id).unwrap().loops().len() == 2)
        .unwrap();
    let path = body::render_domain(&fm, two_loop, "frame-top-domain").unwrap();
    let img = image::open(&path).unwrap().to_rgb8();
    assert_eq!(
        img.get_pixel(400, 300).0,
        colors::BACKGROUND,
        "the window is left open"
    );
    assert_ne!(
        img.get_pixel(100, 100).0,
        colors::BACKGROUND,
        "the outer ring is filled"
    );
}
