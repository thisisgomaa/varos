//! P20: every tool path that can hit a compound-path hole anchor must use the ring-aware address.

use varos_core::editor::{Editor, ToolKind};
use varos_core::model::{Anchor, Path};

fn anc(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}

fn donut() -> Editor {
    let mut ed = Editor::new();
    let mut path = Path::new(
        10,
        vec![anc(1, 0.0, 0.0), anc(2, 100.0, 0.0), anc(3, 100.0, 100.0), anc(4, 0.0, 100.0)],
        true,
        Some([0.5, 0.5, 0.5, 1.0]),
        None,
        1.0,
    );
    path.holes = vec![vec![anc(5, 30.0, 30.0), anc(6, 60.0, 30.0), anc(7, 60.0, 60.0), anc(8, 30.0, 60.0)]];
    ed.doc.paths.push(path);
    ed.doc.ids = 20;
    ed.doc.snap.enabled = false;
    ed.doc.sync_tree();
    ed.objsel.insert(10);
    ed
}

#[test]
fn pen_deletes_a_hole_anchor_and_reconnects_the_closed_hole_in_one_step() {
    let mut ed = donut();
    ed.set_tool(ToolKind::Pen);
    let rev = ed.rev;
    ed.pointer_down([30.0, 30.0]);
    ed.pointer_up();
    assert_eq!(ed.rev, rev + 1);
    assert!(ed.doc.anchor(5).is_none());
    assert_eq!(ed.doc.paths[0].holes[0].len(), 3, "the closed hole reconnects around the removed point");
    ed.undo();
    assert!(ed.doc.anchor(5).is_some());
}

#[test]
fn convert_pulls_handles_from_a_hole_corner_in_one_step() {
    let mut ed = donut();
    ed.set_tool(ToolKind::Convert);
    let rev = ed.rev;
    ed.pointer_down([30.0, 30.0]);
    ed.pointer_move([20.0, 30.0]);
    ed.pointer_up();
    let anchor = ed.doc.anchor(5).unwrap();
    assert!(anchor.smooth && anchor.hin.is_some() && anchor.hout.is_some());
    assert_eq!(ed.rev, rev + 1);
    ed.undo();
    assert!(!ed.doc.anchor(5).unwrap().smooth);
}

#[test]
fn convert_click_turns_a_smooth_hole_anchor_into_a_corner_in_one_step() {
    let mut ed = donut();
    {
        let anchor = ed.doc.anchor_mut(5).unwrap();
        anchor.smooth = true;
        anchor.hin = Some([20.0, 30.0]);
        anchor.hout = Some([40.0, 30.0]);
    }
    ed.set_tool(ToolKind::Convert);
    let rev = ed.rev;
    ed.pointer_down([30.0, 30.0]);
    ed.pointer_up();
    let anchor = ed.doc.anchor(5).unwrap();
    assert!(!anchor.smooth && anchor.hin.is_none() && anchor.hout.is_none());
    assert_eq!(ed.rev, rev + 1);
    ed.undo();
    assert!(ed.doc.anchor(5).unwrap().smooth);
}

#[test]
fn alt_direct_duplicates_and_moves_hole_anchors_in_one_step() {
    let mut ed = donut();
    ed.set_tool(ToolKind::Direct);
    ed.mods.alt = true;
    let rev = ed.rev;
    ed.pointer_down([30.0, 30.0]);
    ed.pointer_move([50.0, 30.0]);
    ed.pointer_up();
    assert_eq!(ed.rev, rev + 1);
    assert_eq!(ed.doc.paths.len(), 2);
    let copy = ed.doc.paths.iter().find(|p| p.id != 10).unwrap();
    assert!(copy.holes[0].iter().any(|a| a.p == [50.0, 30.0]), "the copied hole anchor follows the drag");
    ed.undo();
    assert_eq!(ed.doc.paths.len(), 1);
}
