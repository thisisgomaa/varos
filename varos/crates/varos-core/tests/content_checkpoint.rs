//! DFS S1 §3.1 — the saved-content checkpoint (`Document::content_eq`) and the small editor helpers
//! the multi-document workspace builds on (`transaction_open`, clipboard hand-off, preference carry on
//! undo/redo). Headless, pure data.
//!
//! Run with:  cargo test -p varos-core --test content_checkpoint

use varos_core::editor::{Editor, PaintTarget, ToolKind};
use varos_core::model::{Anchor, Artboard, Document, GroupRole, Guide, Path};
use varos_core::units::Unit;
use varos_core::EditCommand;

/// One named document mutation.
type Change = Box<dyn Fn(&mut Document)>;

fn sq(id: u32, base: u32, x: f32, y: f32, s: f32) -> Path {
    let a = |i: u32, p: [f32; 2]| Anchor { id: i, p, hin: None, hout: None, smooth: false };
    Path::new(
        id,
        vec![a(base, [x, y]), a(base + 1, [x + s, y]), a(base + 2, [x + s, y + s]), a(base + 3, [x, y + s])],
        true,
        Some([1.0, 0.0, 0.0, 1.0]),
        Some([0.0, 0.0, 0.0, 1.0]),
        2.0,
    )
}

/// Two squares on Layer 1 and two artboards; square 10 selected.
fn sample() -> Editor {
    let mut ed = Editor::new();
    ed.doc.artboards = vec![
        Artboard { x: 0.0, y: 0.0, w: 100.0, h: 100.0, name: "A".into(), ..Artboard::default() },
        Artboard { x: 150.0, y: 0.0, w: 100.0, h: 100.0, name: "B".into(), ..Artboard::default() },
    ];
    ed.doc.paths.push(sq(10, 100, 10.0, 10.0, 20.0));
    ed.doc.paths.push(sq(11, 110, 160.0, 10.0, 20.0));
    ed.doc.ids = 200;
    ed.doc.sync_tree();
    ed.objsel.insert(10);
    ed
}

#[test]
fn content_eq_ignores_view_and_preference_fields() {
    let base = sample().doc;
    assert!(base.content_eq(&base.clone()), "a document equals its own clone");

    let prefs: Vec<(&str, Change)> = vec![
        ("active_layer", Box::new(|d| d.active_layer = 999)),
        ("ids", Box::new(|d| d.ids += 50)),
        ("units.display", Box::new(|d| d.units.display = Unit::Mm)),
        ("active", Box::new(|d| d.active = 1)),
        ("move_art_with_ab", Box::new(|d| d.move_art_with_ab = !d.move_art_with_ab)),
        ("snap", Box::new(|d| d.snap.enabled = !d.snap.enabled)),
        ("snap.smart", Box::new(|d| d.snap.smart = !d.snap.smart)),
        ("ruler_origin", Box::new(|d| d.ruler_origin = [33.0, 44.0])),
        ("guides_locked", Box::new(|d| d.guides_locked = !d.guides_locked)),
    ];
    let mut all = base.clone();
    for (name, change) in &prefs {
        let mut d = base.clone();
        change(&mut d);
        assert_ne!(d, base, "{name}: the change is real (PartialEq sees it)");
        assert!(d.content_eq(&base), "{name} is a preference — it must not make the document dirty");
        assert!(base.content_eq(&d), "{name}: symmetric");
        change(&mut all);
    }
    assert!(all.content_eq(&base), "all preferences at once are still not content");
}

#[test]
fn content_eq_sees_every_authored_change() {
    let base = sample().doc;
    let layer = base.roots[0];
    let leaf = base.node_of_path(10).expect("square 10 has a leaf node");
    let edits: Vec<(&str, Change)> = vec![
        ("path geometry", Box::new(|d| d.paths[0].anchors[0].p[0] += 1.0)),
        ("path handle", Box::new(|d| d.paths[0].anchors[0].hout = Some([1.0, 1.0]))),
        ("path closed", Box::new(|d| d.paths[0].closed = false)),
        ("fill paint", Box::new(|d| d.paths[0].fill = varos_core::model::Paint::Solid([0.0, 1.0, 0.0, 1.0]))),
        ("stroke paint", Box::new(|d| d.paths[0].stroke = varos_core::model::Paint::None)),
        ("stroke weight", Box::new(|d| d.paths[0].stroke_width = 7.0)),
        ("opacity", Box::new(|d| d.paths[0].opacity = 0.5)),
        ("path hidden", Box::new(|d| d.paths[0].hidden = true)),
        ("path locked", Box::new(|d| d.paths[0].locked = true)),
        ("path name", Box::new(|d| d.paths[0].name = Some("Logo".into()))),
        ("hole", Box::new(|d| d.paths[0].holes.push(vec![]))),
        ("z order", Box::new(|d| d.paths.swap(0, 1))),
        ("node hidden", Box::new(move |d| d.nodes.iter_mut().find(|n| n.id == layer).unwrap().hidden = true)),
        ("node locked", Box::new(move |d| d.nodes.iter_mut().find(|n| n.id == layer).unwrap().locked = true)),
        ("node name", Box::new(move |d| d.nodes.iter_mut().find(|n| n.id == layer).unwrap().name = "Ink".into())),
        ("layer colour", Box::new(move |d| d.nodes.iter_mut().find(|n| n.id == layer).unwrap().color = Some([1.0; 4]))),
        ("clip exempt", Box::new(move |d| d.nodes.iter_mut().find(|n| n.id == leaf).unwrap().clip_exempt = true)),
        ("live transform", Box::new(move |d| d.nodes.iter_mut().find(|n| n.id == leaf).unwrap().xform.rot = 0.5)),
        ("clip role", Box::new(move |d| d.nodes.iter_mut().find(|n| n.id == layer).unwrap().role = GroupRole::Clip)),
        ("mask child", Box::new(move |d| d.nodes.iter_mut().find(|n| n.id == layer).unwrap().mask_child = Some(leaf))),
        ("root order", Box::new(|d| d.roots.push(12345))),
        ("legacy group_of", Box::new(|d| _ = d.group_of.insert(10, 77))),
        ("artboard rect", Box::new(|d| d.artboards[0].w = 101.0)),
        ("artboard name", Box::new(|d| d.artboards[0].name = "Cover".into())),
        ("artboard colour", Box::new(|d| d.artboards[0].page_color = None)),
        ("artboard bleed", Box::new(|d| d.artboards[0].bleed = 9.0)),
        ("artboard clip", Box::new(|d| d.artboards[0].clip = !d.artboards[0].clip)),
        ("artboard hidden", Box::new(|d| d.artboards[1].hidden = true)),
        ("artboard locked", Box::new(|d| d.artboards[1].locked = true)),
        ("artboard added", Box::new(|d| d.artboards.push(Artboard::default()))),
        ("guide added", Box::new(|d| d.guides.push(Guide { vertical: true, pos: 5.0 }))),
        ("ppi", Box::new(|d| d.units.ppi = 300.0)),
    ];
    for (name, edit) in &edits {
        let mut d = base.clone();
        edit(&mut d);
        assert!(!d.content_eq(&base), "{name} is authored content — it must make the document dirty");
        assert!(!base.content_eq(&d), "{name}: symmetric");
    }
    // a moved guide is content too
    let mut with_guide = base.clone();
    with_guide.guides.push(Guide { vertical: false, pos: 5.0 });
    let mut moved = with_guide.clone();
    moved.guides[0].pos = 6.0;
    assert!(!moved.content_eq(&with_guide), "moving a guide is an edit");
    // NaN can only ever read as dirty, never as clean
    let mut nan = base.clone();
    nan.paths[0].anchors[0].p[0] = f32::NAN;
    assert!(!nan.content_eq(&nan.clone()), "NaN compares unequal ⇒ a false dirty, never a false clean");
}

#[test]
fn noop_commit_and_cancelled_picker_stay_content_equal() {
    let mut ed = sample();
    let saved = ed.doc.clone();

    // a commit that changed nothing still bumps rev (today's history semantics) — but is not content
    let rev0 = ed.rev;
    ed.begin();
    ed.dirty = true;
    ed.commit();
    assert_eq!(ed.rev, rev0 + 1, "the no-op commit is still a history step");
    assert!(ed.doc.content_eq(&saved), "…but the content is unchanged ⇒ clean");

    // renaming a board to its own name
    ed.execute(EditCommand::RenameArtboard { index: 0, name: "A".into() });
    assert!(ed.doc.content_eq(&saved), "an unchanged board name is not an edit");

    // the colour picker: live preview is in flight (transaction open + dirty), Cancel restores
    assert!(!ed.transaction_open());
    ed.execute(EditCommand::PickerBegin);
    assert!(ed.transaction_open(), "PickerBegin opens one history transaction");
    ed.execute(EditCommand::PickerLivePaint { target: PaintTarget::Fill, color: [0.0, 0.0, 1.0, 1.0] });
    assert!(ed.transaction_open() && ed.dirty, "a live preview is an in-flight change");
    assert!(!ed.doc.content_eq(&saved), "the preview is visible in the document");
    let rev1 = ed.rev;
    ed.execute(EditCommand::PickerCancel);
    assert!(!ed.transaction_open(), "Cancel closes the transaction");
    assert_eq!(ed.rev, rev1, "Cancel leaves no history step");
    assert!(ed.doc.content_eq(&saved), "a cancelled picker leaves the content exactly as saved");
}

#[test]
fn undo_and_redo_back_to_a_checkpoint_are_content_equal() {
    let mut ed = sample();
    let saved = ed.doc.clone();

    ed.execute(EditCommand::SetStrokeWidth(9.0));
    assert!(!ed.doc.content_eq(&saved), "an edit is dirty");
    ed.execute(EditCommand::Undo);
    assert!(ed.doc.content_eq(&saved), "undo back to the saved state is clean again");
    ed.execute(EditCommand::Redo);
    assert!(!ed.doc.content_eq(&saved), "redo re-applies the edit");

    // save AFTER the edit, then undo + redo back to it
    let saved2 = ed.doc.clone();
    ed.execute(EditCommand::Undo);
    assert!(!ed.doc.content_eq(&saved2), "undo past the save point is dirty");
    ed.execute(EditCommand::Redo);
    assert!(ed.doc.content_eq(&saved2), "redo back to the save point is clean");

    // units and move-art stay undoable history steps, but never content
    let rev = ed.rev;
    ed.execute(EditCommand::CycleUnits);
    ed.execute(EditCommand::SetMoveArtWithArtboard(!ed.doc.move_art_with_ab));
    assert_eq!(ed.rev, rev + 2, "both are still history steps");
    assert!(ed.doc.content_eq(&saved2), "…that never dirty the document");
    ed.execute(EditCommand::Undo);
    ed.execute(EditCommand::Undo);
    assert!(ed.doc.content_eq(&saved2));
}

#[test]
fn undo_keeps_current_snap_guide_lock_and_ruler_origin() {
    let mut ed = sample();
    let w0 = ed.doc.paths[0].stroke_width;
    let (snap0, locked0, origin0) = (ed.doc.snap, ed.doc.guides_locked, ed.doc.ruler_origin);
    let active0 = ed.doc.active;

    ed.execute(EditCommand::SetStrokeWidth(9.0)); // one real history step
    ed.execute(EditCommand::ToggleSnapping);
    ed.execute(EditCommand::ToggleSmartGuides);
    ed.execute(EditCommand::ToggleGuidesLocked);
    ed.execute(EditCommand::SetRulerOrigin([500.0, 700.0]));
    let (snap1, locked1, origin1) = (ed.doc.snap, ed.doc.guides_locked, ed.doc.ruler_origin);
    assert!(snap1 != snap0 && locked1 != locked0 && origin1 != origin0, "all three preferences changed");

    ed.execute(EditCommand::Undo);
    assert_eq!(ed.doc.paths[0].stroke_width, w0, "the edit is undone");
    assert_eq!(ed.doc.snap, snap1, "undo keeps the CURRENT snapping config");
    assert_eq!(ed.doc.guides_locked, locked1, "undo keeps the CURRENT guide lock");
    assert_eq!(ed.doc.ruler_origin, origin1, "undo keeps the CURRENT ruler origin");

    ed.execute(EditCommand::ToggleGuidesLocked); // change a preference between undo and redo
    ed.execute(EditCommand::Redo);
    assert_eq!(ed.doc.paths[0].stroke_width, 9.0, "the edit is redone");
    assert_eq!(ed.doc.guides_locked, locked0, "redo keeps the CURRENT guide lock too");
    assert_eq!(ed.doc.snap, snap1);
    assert_eq!(ed.doc.ruler_origin, origin1);
    assert_eq!(ed.doc.active, active0, "`active` stays history-restored (unchanged here)");
}

#[test]
fn clipboard_moves_between_editors() {
    let mut a = sample();
    let mut b = Editor::new();
    let (rev_a, rev_b) = (a.rev, b.rev);
    let doc_a = a.doc.clone();

    a.execute(EditCommand::Copy);
    assert_eq!(a.clipboard().len(), 1);
    let clip = a.take_clipboard();
    assert!(a.clipboard().is_empty(), "take_clipboard leaves the source empty");
    assert_eq!(clip.len(), 1);
    b.set_clipboard(clip);
    assert_eq!(b.clipboard().len(), 1, "the target now holds the copy");
    assert_eq!((a.rev, b.rev), (rev_a, rev_b), "the hand-off is not a document edit");
    assert_eq!(a.doc, doc_a, "the source document is untouched");

    b.set_tool(ToolKind::Object);
    b.execute(EditCommand::Paste { offset: None });
    assert_eq!(b.doc.paths.len(), 1, "B pastes A's art");
    assert_eq!(b.doc.paths[0].anchors[0].p, doc_a.paths[0].anchors[0].p, "in place");
    assert_eq!(b.clipboard().len(), 1, "pasting keeps the clipboard");
    assert_eq!(a.doc.paths.len(), 2, "A is unchanged");
}
