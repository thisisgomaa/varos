//! Rotate/Scale-tool logic — pure math, no UI (allowed per the math-test rule). Proves transforms happen
//! around the chosen pivot and that the pivot defaults to the selection centre. Drives the real down/move/up.

use varos_core::editor::{Editor, ToolKind};
use varos_core::model::{Anchor, Path};

fn anc(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}
fn rect(id: u32, ids: [u32; 4], x: f32, y: f32, w: f32, h: f32) -> Path {
    Path::new(
        id,
        vec![anc(ids[0], x, y), anc(ids[1], x + w, y), anc(ids[2], x + w, y + h), anc(ids[3], x, y + h)],
        true,
        Some([0.5, 0.5, 0.5, 1.0]),
        None,
        1.0,
    )
}
fn sel_rect() -> Editor {
    let mut ed = Editor::new();
    ed.doc.paths.push(rect(10, [1, 2, 3, 4], 0.0, 0.0, 100.0, 100.0));
    ed.doc.ids = 4;
    ed.ppu = 1.0;
    ed.objsel.insert(10);
    ed.set_tool(ToolKind::Rotate);
    ed
}

#[test]
fn pivot_defaults_to_selection_centre() {
    let ed = sel_rect();
    assert_eq!(ed.pivot_point(), Some([50.0, 50.0]), "with no click, the pivot is the selection bbox centre");
}

#[test]
fn rotate_tool_rotates_around_the_clicked_pivot() {
    let mut ed = sel_rect();
    ed.pointer_down([0.0, 0.0]);
    ed.pointer_up(); // a plain click relocates the pivot to the corner [0,0]
    assert_eq!(ed.pivot, Some([0.0, 0.0]), "a click should move the pivot");
    ed.pointer_down([100.0, 0.0]); // grab at 0deg from the pivot
    ed.pointer_move([0.0, 100.0]); // drag to 90deg -> rotate the selection about [0,0]
                                   // A7 Stage 4: rotation is now a LIVE transform — the stored anchor stays in LOCAL space and the WORLD
                                   // position comes through the unit transform. (Old code baked p directly; the geometry is identical.)
    let a2 = ed.doc.anchor(2).unwrap();
    let p2 = ed.doc.unit_xform(10).apply(a2.p); // corner that started at [100,0], now in world
    assert!(
        (p2[0]).abs() < 0.5 && (p2[1] - 100.0).abs() < 0.5,
        "corner [100,0] should rotate to ~[0,100] in world, got {:?}",
        p2
    );
    ed.pointer_up();
}

#[test]
fn pivot_click_snaps_to_a_nearby_anchor() {
    let mut ed = sel_rect(); // corner anchor 2 is at [100,0]
    ed.pointer_down([103.0, 3.0]);
    ed.pointer_up(); // click ~4px from the corner -> should snap onto it
    let piv = ed.pivot.expect("a click sets the pivot");
    assert!(
        (piv[0] - 100.0).abs() < 0.01 && piv[1].abs() < 0.01,
        "a pivot click near a corner must snap onto ~[100,0], got {:?}",
        piv
    );
}

#[test]
fn scale_tool_scales_around_the_pivot() {
    let mut ed = sel_rect();
    ed.set_tool(ToolKind::Scale);
    ed.pointer_down([0.0, 0.0]);
    ed.pointer_up(); // pivot -> [0,0]
    ed.pointer_down([100.0, 100.0]); // grab the far corner
    ed.pointer_move([200.0, 200.0]); // drag 2x further from the pivot -> scale 2x
    let p3 = ed.doc.anchor(3).unwrap().p; // was [100,100]
    let p1 = ed.doc.anchor(1).unwrap().p; // was [0,0] (on the pivot)
    assert!((p3[0] - 200.0).abs() < 0.5 && (p3[1] - 200.0).abs() < 0.5, "far corner should double, got {:?}", p3);
    assert!(p1[0].abs() < 0.5 && p1[1].abs() < 0.5, "the pivot corner must stay put, got {:?}", p1);
    ed.pointer_up();
}

#[test]
fn transform_again_repeats_the_rotate_copy() {
    let mut ed = sel_rect();
    ed.pointer_down([0.0, 0.0]);
    ed.pointer_up(); // pivot -> corner [0,0]
    ed.mods.alt = true; // Alt-rotate a COPY
    ed.pointer_down([100.0, 0.0]);
    ed.pointer_move([0.0, 100.0]);
    ed.pointer_up();
    assert_eq!(ed.doc.paths.len(), 2, "Alt-rotate leaves the original + one rotated copy");
    ed.mods.alt = false;
    ed.transform_again(); // Ctrl+D -> another rotated copy (radial step-and-repeat)
    assert_eq!(ed.doc.paths.len(), 3, "Transform Again repeats the rotate-copy");
}

#[test]
fn alt_drag_rotates_a_copy_and_keeps_the_original() {
    let mut ed = sel_rect();
    ed.mods.alt = true; // Alt-drag -> rotate a COPY
    ed.pointer_down([50.0, 0.0]); // grab the top-mid; pivot defaults to centre [50,50]
    ed.pointer_move([100.0, 50.0]); // drag -> some rotation of a duplicate
    ed.pointer_up();
    assert_eq!(ed.doc.paths.len(), 2, "Alt-drag should leave the original and add a rotated copy");
}
