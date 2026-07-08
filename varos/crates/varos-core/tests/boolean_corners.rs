//! A26 — a boolean (Pathfinder) result must keep sharp corners as CORNER points, not smooth them into
//! anchors with handles. The union of two axis-aligned rectangles is all right angles, so every point of
//! the result must be a clean corner (no hin/hout, not smooth). Pure core, no UI.
//!
//! Run with:  cargo test -p varos-core --test boolean_corners

use varos_core::boolean::BoolOp;
use varos_core::editor::Editor;
use varos_core::model::{Anchor, Path};

fn corner(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}
fn rect(pid: u32, base: u32, x0: f32, y0: f32, x1: f32, y1: f32) -> Path {
    Path::new(
        pid,
        vec![corner(base, x0, y0), corner(base + 1, x1, y0), corner(base + 2, x1, y1), corner(base + 3, x0, y1)],
        true,
        Some([0.5, 0.5, 0.5, 1.0]),
        None,
        1.0,
    )
}

#[test]
fn uniting_two_rectangles_yields_only_corner_points() {
    let mut ed = Editor::new();
    ed.doc.paths.clear();
    ed.doc.paths.push(rect(100, 1, 0.0, 0.0, 60.0, 40.0));
    ed.doc.paths.push(rect(200, 10, 30.0, 20.0, 90.0, 60.0)); // overlaps the first
    ed.doc.ids = 1000;
    ed.objsel.insert(100);
    ed.objsel.insert(200);
    ed.pathfinder(BoolOp::Unite);

    let mut n = 0;
    for p in &ed.doc.paths {
        for a in p.anchors.iter().chain(p.holes.iter().flatten()) {
            n += 1;
            assert!(
                a.hin.is_none() && a.hout.is_none(),
                "a right-angle corner from a boolean must have NO handles, got {a:?}"
            );
            assert!(!a.smooth, "a right-angle corner must not be a smooth point, got {a:?}");
        }
    }
    assert!(n >= 6, "the L-shaped union has at least 6 corners — the op must have produced a path, got {n}");
}
