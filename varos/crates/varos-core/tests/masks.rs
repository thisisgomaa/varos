//! Clipping masks — Stages 1–2 (MASKS_PLAN §1–5), headless. Proves the USER-INVISIBLE plumbing:
//! the model form (`GroupRole::Clip` + `mask_child`), the `paint_role` flip that excludes the mask from
//! paint, the undoable core ops, the dup/`sync_tree` audits, and the scene emitting the clip form. NO
//! gesture is wired to any of this yet (Stage 3) — only these tests construct a clip, so the running app
//! is behaviour-identical for every existing flow. The GPU stencil clip itself is not headless-testable
//! (CI is headless); it is verified live. Here we lock everything that CAN be checked without a GPU.

use varos_core::editor::Editor;
use varos_core::model::{Anchor, Document, GroupRole, Node, PaintRole, Path};
use varos_core::scene::{build_scene, Group};

/// A filled axis-aligned square (base..base+3 anchor ids) — predictable geometry for hit/point tests.
fn sq(id: u32, base: u32, x: f32, y: f32, s: f32) -> Path {
    let a = |i: u32, p: [f32; 2]| Anchor { id: i, p, hin: None, hout: None, smooth: false };
    Path::new(
        id,
        vec![a(base, [x, y]), a(base + 1, [x + s, y]), a(base + 2, [x + s, y + s]), a(base + 3, [x, y + s])],
        true,
        Some([0.5, 0.5, 0.5, 1.0]),
        None,
        1.0,
    )
}

/// A document with a back square (10) and a smaller front square (11), both on Layer 1.
fn two_squares() -> Document {
    let mut d = Document::default(); // Document::default has NO artboards → no page clip to interfere
    d.paths.push(sq(10, 100, 0.0, 0.0, 40.0)); // content (back)
    d.paths.push(sq(11, 110, 10.0, 10.0, 20.0)); // mask   (front)
    d.ids = 200;
    d.sync_tree();
    d
}

#[test]
fn clip_group_node_serde_round_trips_and_legacy_loads_normal() {
    // an old `.vrs` node has neither key → loads Normal / None (no format bump, old files unchanged).
    let legacy = r#"{"id":9,"kind":"Group"}"#;
    let n: Node = serde_json::from_str(legacy).expect("a pre-mask node still deserializes");
    assert_eq!(n.role, GroupRole::Normal, "missing role key ⇒ Normal");
    assert_eq!(n.mask_child, None, "missing mask_child key ⇒ None");

    // a clip node round-trips byte-for-byte (the id is durable).
    let mut clip = n.clone();
    clip.role = GroupRole::Clip;
    clip.mask_child = Some(42);
    let back: Node = serde_json::from_str(&serde_json::to_string(&clip).unwrap()).unwrap();
    assert_eq!(clip, back, "a clip node round-trips unchanged");
    assert_eq!(back.role, GroupRole::Clip);
    assert_eq!(back.mask_child, Some(42));

    // an un-clipped node omits the mask_child key entirely (stays byte-clean).
    assert!(!serde_json::to_string(&n).unwrap().contains("mask_child"), "None mask_child is skipped");
}

#[test]
fn clip_group_builds_the_topology_and_release_tears_it_down() {
    let mut d = two_squares();
    let clip = d.clip_group(&[10, 11], 11).expect("a 2-shape selection clips");

    // the new group is a Clip whose mask_child is the DIRECT child holding the mask path (front unit rule).
    let node = d.node(clip).unwrap();
    assert_eq!(node.role, GroupRole::Clip);
    let mc = node.mask_child.expect("a clip records its mask");
    assert!(node.children.contains(&mc), "mask_child is a direct child (sync_tree's invariant holds from birth)");
    assert_eq!(d.node_paths(mc), vec![11], "the mask subtree is exactly the front-most selected shape");
    assert_eq!(d.clip_group_of(10), Some(clip), "the clipped content resolves to the clip unit");
    assert_eq!(d.clip_group_of(11), Some(clip), "the mask belongs to the clip too");

    // release keeps the group + both shapes; it just stops clipping (nothing is deleted).
    d.release_clip(clip);
    let node = d.node(clip).unwrap();
    assert_eq!(node.role, GroupRole::Normal);
    assert_eq!(node.mask_child, None);
    assert_eq!(d.paint_role(11), PaintRole::Normal, "the released mask paints again");
    assert!(d.pidx(10).is_some() && d.pidx(11).is_some(), "release ≠ delete");
}

#[test]
fn paint_list_excludes_the_mask_source_but_keeps_it_a_first_class_row() {
    // the BLOCKER-1 test (LAYERS_VISION §5): the mask is dropped from the paint run but stays in
    // doc.paths for hit-test / rename / thumbnail — a mask that double-paints would slip past the z-order
    // golden, so this checks the exclusion directly.
    let mut d = two_squares();
    d.clip_group(&[10, 11], 11).unwrap();

    assert_eq!(d.paint_role(11), PaintRole::MaskSource, "the mask shapes the clip, not itself");
    assert_eq!(d.paint_role(10), PaintRole::Normal, "the clipped content is ordinary paint");
    let painted: Vec<u32> = d.paint_list().map(|(_, p)| p.id).collect();
    assert!(!painted.contains(&11), "the mask is excluded from the content/paint run");
    assert!(painted.contains(&10), "the clipped content still paints");
    assert!(d.pidx(11).is_some(), "the mask stays a first-class, clickable/renamable row in doc.paths");

    // and a document with NO clip is byte-for-byte paint_list == doc.paths, every role Normal (the golden
    // this diffs against — nothing changed for un-clipped art).
    let plain = two_squares();
    let via: Vec<u32> = plain.paint_list().map(|(_, p)| p.id).collect();
    assert_eq!(via, plain.paths.iter().map(|p| p.id).collect::<Vec<_>>());
    assert!(plain.paths.iter().all(|p| plain.paint_role(p.id) == PaintRole::Normal));
}

#[test]
fn clip_group_is_undoable() {
    let mut ed = Editor::new();
    ed.doc = two_squares();
    ed.begin();
    ed.doc.clip_group(&[10, 11], 11).unwrap();
    ed.dirty = true; // a real edit sets this; commit records history only when dirty
    ed.commit();
    assert_eq!(ed.doc.paint_role(11), PaintRole::MaskSource, "the clip took effect");

    ed.undo();
    assert!(ed.doc.nodes.iter().all(|n| n.role == GroupRole::Normal), "undo removes the clip entirely");
    assert_eq!(ed.doc.paint_role(11), PaintRole::Normal, "everything paints normally again after undo");
}

#[test]
fn dup_paths_remaps_the_clip_to_its_own_mask() {
    // the dup audit (MASKS_PLAN §7): a duplicated clip must clip to its OWN copied mask, never dangle at
    // the original's — mirroring how dup_paths already remaps subtree ids.
    let mut d = two_squares();
    let clip = d.clip_group(&[10, 11], 11).unwrap();
    let orig_mc = d.node(clip).unwrap().mask_child.unwrap();

    let new_pids = d.dup_paths(&[10, 11]);
    assert_eq!(new_pids.len(), 2, "both shapes duplicated");
    let clips: Vec<u32> = d.nodes.iter().filter(|n| n.role == GroupRole::Clip).map(|n| n.id).collect();
    assert_eq!(clips.len(), 2, "the duplicate is ALSO a clip (clip-ness carries to the copy)");
    let dup_clip = clips.into_iter().find(|&c| c != clip).unwrap();
    let dmc = d.node(dup_clip).unwrap().mask_child.expect("the copy has its own mask");
    assert_ne!(dmc, orig_mc, "the duplicate clips to its OWN mask node, not the original's");
    assert!(d.node(dup_clip).unwrap().children.contains(&dmc), "the copied mask is a direct child of the copy");
    assert!(d.node_paths(dmc).iter().all(|p| new_pids.contains(p)), "the copied mask is one of the new paths");
}

#[test]
fn sync_tree_demotes_a_clip_whose_mask_was_removed() {
    // BLOCKER-5: the id is authoritative, so deleting/re-parenting the mask must not leave a clip pointing
    // at a ghost. Deleting the mask path prunes its leaf → sync_tree demotes the clip to a plain group.
    let mut d = two_squares();
    let clip = d.clip_group(&[10, 11], 11).unwrap();
    d.paths.retain(|p| p.id != 11); // delete the mask
    d.sync_tree();
    let node = d.node(clip).expect("the group survives (it still holds the clipped content)");
    assert_eq!(node.role, GroupRole::Normal, "a clip whose mask vanished is demoted");
    assert_eq!(node.mask_child, None, "no dangling mask id");
    assert_eq!(d.paint_role(10), PaintRole::Normal, "the ex-content paints normally");
}

#[test]
fn scene_emits_the_clip_form_with_the_mask_rings() {
    // Stage 2: build_scene emits ONE Group::Clip carrying the mask silhouette (world rings) + the clipped
    // members. The mask itself never appears as its own paint group (paint_role excluded it).
    let mut ed = Editor::new();
    ed.doc = two_squares();
    ed.ppu = 1.0;
    ed.doc.clip_group(&[10, 11], 11).unwrap();
    let ppu = ed.ppu;
    let scene = build_scene(&ed, ppu);

    let (mask_rings, members) = scene
        .content
        .iter()
        .find_map(|g| match g {
            Group::Clip { mask_rings, members } => Some((mask_rings, members)),
            _ => None,
        })
        .expect("the clipped group emits a Group::Clip");
    assert!(!mask_rings.is_empty(), "the clip carries the mask silhouette rings");
    assert!(mask_rings[0].len() >= 3, "the mask ring is a real polygon (built from world_outline_px)");
    assert!(!members.is_empty(), "the clipped content is a member group");
}

#[test]
fn a_document_with_no_clip_emits_no_clip_group() {
    // the byte-identical guarantee: with no clip group, the scene never contains a Group::Clip (so all
    // existing scene/golden tests keep their exact prim output).
    let mut ed = Editor::new();
    ed.doc = Document::default();
    ed.doc.paths.push(sq(10, 100, 0.0, 0.0, 40.0));
    ed.doc.paths.push(sq(11, 110, 80.0, 80.0, 20.0));
    ed.doc.ids = 200;
    ed.doc.sync_tree();
    ed.ppu = 1.0;
    let ppu = ed.ppu;
    let scene = build_scene(&ed, ppu);
    assert!(scene.content.iter().all(|g| !matches!(g, Group::Clip { .. })), "no clip group ⇒ no Group::Clip emitted");
}
