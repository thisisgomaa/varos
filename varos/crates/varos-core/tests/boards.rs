//! Artboard SECTIONS + the "mirror" clip rule (Ahmed 07-06), headless. Membership = the boards a
//! path's outline bbox overlaps (`path_boards`); render cuts an object once per member page —
//! straddlers show a part on BOTH pages (mirror), floaters and bleed-pages draw uncut. The Layers
//! panel sections read the same membership fns, so these tests guard both surfaces.

use varos_core::editor::Editor;
use varos_core::model::{Anchor, Artboard, Document, Path};
use varos_core::scene::{build_scene, Prim};

fn anc(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}
/// An axis-aligned filled square [x0..x0+s]² — easy to reason about post-clip.
fn sq(id: u32, base: u32, x0: f32, y0: f32, s: f32) -> Path {
    Path::new(
        id,
        vec![anc(base, x0, y0), anc(base + 1, x0 + s, y0), anc(base + 2, x0 + s, y0 + s), anc(base + 3, x0, y0 + s)],
        true,
        Some([0.5, 0.5, 0.5, 1.0]),
        None,
        1.0,
    )
}
fn board(x: f32, name: &str, clip: bool) -> Artboard {
    Artboard { x, y: 0.0, w: 100.0, h: 100.0, name: name.into(), clip, ..Artboard::default() }
}
/// Two 100×100 pages with a 50pt gap: A at x=0, B at x=150.
fn two_pages(clip: bool) -> Editor {
    let mut ed = Editor::new();
    ed.ppu = 1.0;
    ed.doc.artboards = vec![board(0.0, "A", clip), board(150.0, "B", clip)];
    ed
}
/// All Fill prims EXCEPT the page-paper fills (papers carry the page colour; art here is grey).
fn art_fills(ed: &Editor) -> Vec<Vec<varos_core::geom::Pt>> {
    build_scene(ed, 1.0)
        .content
        .iter()
        .flat_map(|g| g.prims())
        .filter_map(|p| match p {
            Prim::Fill { rings, color } if color[0] < 0.9 => Some(rings[0].clone()),
            _ => None,
        })
        .collect()
}
fn xs(ring: &[varos_core::geom::Pt]) -> (f32, f32) {
    ring.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p[0]), hi.max(p[0])))
}

#[test]
fn new_documents_clip_by_default() {
    // Ahmed 07-06 decision #5: pages cut like modern tools — the default board clips.
    assert!(Artboard::default().clip, "Artboard::default is clip=ON");
    assert!(Document::default().artboards[0].clip, "a fresh document's page clips");
}

#[test]
fn pre_artboard_files_get_a_non_clipping_page() {
    // 07-06 review fix #1: a file saved before the `artboards` key existed AT ALL was authored with
    // art bleeding freely — the serde-default page it receives must NOT cut, or loading the file
    // silently vanishes everything drawn outside it. (Fresh docs still clip — the test above.)
    let d: Document =
        serde_json::from_str(r#"{"paths": [], "ids": 1}"#).expect("a minimal pre-artboard file deserializes");
    assert_eq!(d.artboards.len(), 1, "the implicit page is guaranteed");
    assert!(!d.artboards[0].clip, "…and it must NOT clip (legacy bleed behaviour preserved)");
}

#[test]
fn membership_is_visible_overlap() {
    let mut ed = two_pages(true);
    ed.doc.paths.push(sq(1, 1, 10.0, 10.0, 30.0)); // fully on A
    ed.doc.paths.push(sq(2, 10, 80.0, 10.0, 100.0)); // straddles A (80..100) and B (150..180)
    ed.doc.paths.push(sq(3, 20, 110.0, 10.0, 20.0)); // in the gap — a floater
    ed.doc.ids = 40;
    ed.doc.sync_tree();
    assert_eq!(ed.doc.path_boards(0), vec![0], "on A only");
    assert_eq!(ed.doc.path_boards(1), vec![0, 1], "the straddler is a member of BOTH pages (mirror)");
    assert_eq!(ed.doc.path_boards(2), Vec::<usize>::new(), "the gap floater belongs to no page");
    // node_boards mirrors path membership through the tree (top-level rows classify with this)
    let leaf = ed.doc.node_of_path(2).unwrap();
    assert_eq!(ed.doc.node_boards(leaf), vec![0, 1]);
}

#[test]
fn straddler_renders_once_per_page_cut_to_each() {
    let mut ed = two_pages(true);
    ed.doc.paths.push(sq(1, 1, 80.0, 10.0, 100.0)); // 80..180 across the 100..150 gap
    ed.doc.ids = 10;
    ed.doc.sync_tree();
    let fills = art_fills(&ed);
    assert_eq!(fills.len(), 2, "mirror: one clipped copy per member page");
    let (a, b) = (xs(&fills[0]), xs(&fills[1]));
    let (l, r) = if a.0 < b.0 { (a, b) } else { (b, a) };
    assert!(l.0 >= 79.9 && l.1 <= 100.1, "left copy is cut to page A's edge (x ≤ 100), got {l:?}");
    assert!(r.0 >= 149.9 && r.1 <= 180.1, "right copy starts at page B's edge (x ≥ 150), got {r:?}");
}

#[test]
fn a_clipped_groups_members_never_escape_the_cut() {
    // Ahmed 07-06 #2: the clip unit is the whole TOP-LEVEL GROUP — a member standing on no page
    // VANISHES (Figma's out-of-frame child: still a panel row, invisible on canvas), and a member's
    // gap overhang is cut at the page edge. Nothing inside a clipped group escapes.
    let mut ed = two_pages(true);
    ed.doc.paths.push(sq(1, 1, 80.0, 10.0, 40.0)); // overhangs page A into the gap (x 80..120)
    ed.doc.paths.push(sq(2, 10, 110.0, 60.0, 20.0)); // fully in the gap — on NO page
    ed.doc.ids = 30;
    ed.doc.sync_tree();
    ed.doc.group(&[1, 2]).unwrap();
    let fills = art_fills(&ed);
    assert_eq!(fills.len(), 1, "the no-page member vanishes; the overhang draws once (cut)");
    assert_eq!(xs(&fills[0]), (80.0, 100.0), "…cut at page A's edge");

    // a group ENTIRELY off every page stays a floater — draws uncut
    let mut ed = two_pages(true);
    ed.doc.paths.push(sq(1, 1, 105.0, 10.0, 15.0));
    ed.doc.paths.push(sq(2, 10, 110.0, 60.0, 20.0));
    ed.doc.ids = 30;
    ed.doc.sync_tree();
    ed.doc.group(&[1, 2]).unwrap();
    assert_eq!(art_fills(&ed).len(), 2, "an all-floater group draws uncut");
}

#[test]
fn board_eye_hides_only_that_pages_part() {
    // Piece C: the header eye hides the PAGE — a mirror keeps its part on the visible page; art
    // fully on the hidden page is eff_hidden (unclickable, unpainted); the page paper vanishes too.
    let mut ed = two_pages(true);
    ed.doc.paths.push(sq(1, 1, 10.0, 10.0, 30.0)); // fully on A
    ed.doc.paths.push(sq(2, 10, 80.0, 10.0, 100.0)); // straddles A and B
    ed.doc.ids = 30;
    ed.doc.sync_tree();
    ed.ab_toggle_hidden(0); // hide page A
    assert!(ed.doc.artboards[0].hidden);
    assert!(ed.doc.eff_hidden(1), "art fully on the hidden page is effectively hidden");
    assert!(!ed.doc.eff_hidden(2), "the mirror stays visible — page B still shows it");
    let fills = art_fills(&ed);
    assert_eq!(fills.len(), 1, "only the B copy of the straddler draws (A's part + A's page are gone)");
    assert!(xs(&fills[0]).0 >= 149.9, "…and it is the part on page B");
    ed.undo();
    assert!(!ed.doc.artboards[0].hidden, "the header eye is undoable");
}

#[test]
fn board_lock_locks_anything_it_holds() {
    // Piece C: the header padlock locks a member if ANY of its pages lock (you can't move what a
    // locked page holds — moving the mirror would move its locked-page part too).
    let mut ed = two_pages(true);
    ed.doc.paths.push(sq(1, 1, 10.0, 10.0, 30.0)); // fully on A
    ed.doc.paths.push(sq(2, 10, 80.0, 10.0, 100.0)); // straddles A and B
    ed.doc.paths.push(sq(3, 20, 110.0, 60.0, 20.0)); // floater
    ed.doc.ids = 40;
    ed.doc.sync_tree();
    ed.objsel.insert(1);
    ed.ab_toggle_locked(0);
    assert!(ed.doc.eff_locked(1) && ed.doc.eff_locked(2), "on-page art AND the straddler lock");
    assert!(!ed.doc.eff_locked(3), "a floater belongs to no page — never board-locked");
    assert!(!ed.objsel.contains(&1), "locking a board drops its art from the selection");
}

#[test]
fn panel_drop_moves_art_onto_the_other_board() {
    // Piece C: dropping a row on another board's section = a spatial move. From a page: keep the
    // offset relative to the page (Figma). A floater: land centred on the target page. Locked: refused.
    let mut ed = two_pages(true);
    ed.doc.paths.push(sq(1, 1, 10.0, 10.0, 30.0)); // on A at offset (10,10)
    ed.doc.paths.push(sq(2, 10, 110.0, 60.0, 20.0)); // floater in the gap
    ed.doc.ids = 30;
    ed.doc.sync_tree();

    let near = |a: f32, b: f32| (a - b).abs() < 0.01; // outline sampling leaves float dust

    let row1 = ed.doc.node_of_path(1).unwrap();
    ed.layer_move_to_board(row1, Some(0), 1); // A → B (B starts at x=150)
    let pi = ed.doc.pidx(1).unwrap();
    let b = ed.doc.outline_bbox(pi);
    assert!(near(b.0, 160.0) && near(b.1, 10.0), "same offset relative to the target page, got {b:?}");
    assert_eq!(ed.doc.path_boards(pi), vec![1], "membership followed the geometry");
    assert_eq!(ed.doc.active, 1, "the target page becomes active");

    let row2 = ed.doc.node_of_path(2).unwrap();
    ed.layer_move_to_board(row2, None, 0); // floater → centre of page A
    let pi2 = ed.doc.pidx(2).unwrap();
    let b2 = ed.doc.outline_bbox(pi2);
    assert!(
        near((b2.0 + b2.2) * 0.5, 50.0) && near((b2.1 + b2.3) * 0.5, 50.0),
        "floater lands page-centred, got {b2:?}"
    );

    // a locked member refuses the whole move (no tearing)
    ed.set_locked(2, true);
    let before = ed.doc.outline_bbox(ed.doc.pidx(2).unwrap());
    ed.layer_move_to_board(row2, Some(0), 1);
    assert_eq!(ed.doc.outline_bbox(ed.doc.pidx(2).unwrap()), before, "locked art never moves");
}

#[test]
fn floaters_and_bleed_pages_draw_uncut() {
    // floater: on no page → uncut
    let mut ed = two_pages(true);
    ed.doc.paths.push(sq(1, 1, 110.0, 10.0, 20.0));
    ed.doc.ids = 10;
    ed.doc.sync_tree();
    let fills = art_fills(&ed);
    assert_eq!(fills.len(), 1, "a floater draws once");
    assert_eq!(xs(&fills[0]), (110.0, 130.0), "…and uncut");

    // bleed: a member page with clip OFF → the object draws once, uncut (Illustrator bleed)
    let mut ed = two_pages(true);
    ed.doc.artboards[0].clip = false;
    ed.doc.paths.push(sq(1, 1, 80.0, 10.0, 40.0)); // sticks out of A into the gap
    ed.doc.ids = 10;
    ed.doc.sync_tree();
    let fills = art_fills(&ed);
    assert_eq!(fills.len(), 1, "clip-off page invites bleed — one uncut draw");
    assert_eq!(xs(&fills[0]), (80.0, 120.0));
}
