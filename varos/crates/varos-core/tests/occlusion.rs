//! A31 — hit-testing respects z-order OCCLUSION. Among stacked shapes only the TOP-most shape covering
//! the point is selectable there; a buried shape is unreachable under a covering fill, yet its VISIBLE
//! regions stay available. An unfilled shape's hollow interior is click-through. Pure core, no UI.
//!
//! Run with:  cargo test -p varos-core --test occlusion

use varos_core::editor::Editor;
use varos_core::geom::Rgba;
use varos_core::model::{Anchor, Path};

fn corner(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}
fn rect(pid: u32, base: u32, x0: f32, y0: f32, x1: f32, y1: f32, fill: Option<Rgba>) -> Path {
    Path::new(
        pid,
        vec![corner(base, x0, y0), corner(base + 1, x1, y0), corner(base + 2, x1, y1), corner(base + 3, x0, y1)],
        true,
        fill,
        None,
        1.0,
    )
}
const GREY: Rgba = [0.5, 0.5, 0.5, 1.0];

/// Three filled bars, wide→narrow, sharing the right side; paths vec is z bottom→top (A, B, C).
/// A covers x∈[0,120], B x∈[30,120], C x∈[60,120] — all y∈[0,40]. Test points sit ≥15 from any edge
/// (edge_r = 8), so only the FILL test decides.
fn stack() -> Editor {
    let mut ed = Editor::new();
    ed.doc.paths.clear();
    ed.doc.paths.push(rect(1, 1, 0.0, 0.0, 120.0, 40.0, Some(GREY))); // A (bottom)
    ed.doc.paths.push(rect(2, 10, 30.0, 0.0, 120.0, 40.0, Some(GREY))); // B (mid)
    ed.doc.paths.push(rect(3, 20, 60.0, 0.0, 120.0, 40.0, Some(GREY))); // C (top)
    ed.doc.ids = 100;
    ed.ppu = 1.0;
    ed
}

#[test]
fn covered_point_selects_the_topmost_shape() {
    let ed = stack();
    assert_eq!(ed.path_under([90.0, 20.0]), Some(3), "inside all three → the TOP shape (C), never a buried one");
}

#[test]
fn a_partly_covered_shape_is_reachable_only_where_it_shows() {
    let ed = stack();
    assert_eq!(ed.path_under([45.0, 20.0]), Some(2), "inside A+B but past C → the topmost there is B");
    assert_eq!(ed.path_under([15.0, 20.0]), Some(1), "the exposed left strip belongs to A alone");
}

#[test]
fn an_open_but_filled_shape_still_occludes_by_its_fill() {
    // A32 makes an OPEN path fill (implied close). That fill must also HIT — otherwise you'd see it
    // filled but the click would fall through to the shape behind it (the render/hit-test must agree).
    let mut ed = Editor::new();
    ed.doc.paths.clear();
    ed.doc.paths.push(rect(1, 1, 0.0, 0.0, 120.0, 40.0, Some(GREY))); // A, closed (bottom)
    let mut b = rect(2, 10, 0.0, 0.0, 120.0, 40.0, Some(GREY)); // B, same area (top)
    b.closed = false; // opened, as if a corner was deleted
    ed.doc.paths.push(b);
    ed.doc.ids = 100;
    ed.ppu = 1.0;
    assert_eq!(
        ed.path_under([60.0, 20.0]),
        Some(2),
        "the visibly-filled OPEN top shape must catch its own fill, not pass the click to A behind it"
    );
}

#[test]
fn an_unfilled_cover_is_click_through_but_its_outline_is_grabbable() {
    let mut ed = stack();
    // a big UNFILLED outline dropped on top must not steal interior clicks from the filled art below
    ed.doc.paths.push(rect(4, 30, 0.0, 0.0, 120.0, 40.0, None));
    ed.doc.ids = 100;
    assert_eq!(ed.path_under([90.0, 20.0]), Some(3), "hollow top shape is click-through → still hits C");
    assert_eq!(ed.path_under([0.0, 20.0]), Some(4), "…but clicking the unfilled shape's own edge selects it");
}
