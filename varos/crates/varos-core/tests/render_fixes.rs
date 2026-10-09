use varos_core::{
    geom::painted_extent,
    model::{Anchor, GroupRole, Paint, Path},
    scene::{build_scene_in_view, scene_signature},
    Editor, View,
};
fn path(id: u32) -> Path {
    Path::new(
        id,
        [[0., 0.], [20., 0.], [20., 20.], [0., 20.]]
            .into_iter()
            .enumerate()
            .map(|(i, p)| Anchor { id: id * 10 + i as u32, p, hin: None, hout: None, smooth: false })
            .collect(),
        true,
        Some([1., 0., 0., 1.]),
        Some([0., 0., 1., 1.]),
        4.,
    )
}
#[test]
fn every_paint_field_invalidates_live_signature() {
    let mut ed = Editor::new();
    ed.doc.paths = vec![path(10)];
    ed.doc.sync_tree();
    ed.objsel.insert(10);
    let signature = |e: &Editor| scene_signature(e, View::identity(), [100, 100]);
    let before = signature(&ed);
    for slot in 0..2 {
        for channel in 0..4 {
            let mut changed = Editor::new();
            changed.doc = ed.doc.clone();
            changed.objsel = ed.objsel.clone();
            let mut c = if slot == 0 {
                changed.doc.paths[0].fill.solid().unwrap()
            } else {
                changed.doc.paths[0].stroke.solid().unwrap()
            };
            c[channel] = 0.37;
            if slot == 0 {
                changed.doc.paths[0].fill = Paint::Solid(c)
            } else {
                changed.doc.paths[0].stroke = Paint::Solid(c)
            };
            assert_ne!(before, signature(&changed));
        }
    }
    for slot in 0..4 {
        let mut changed = Editor::new();
        changed.doc = ed.doc.clone();
        changed.objsel = ed.objsel.clone();
        let p = &mut changed.doc.paths[0];
        match slot {
            0 => p.fill = Paint::None,
            1 => p.stroke = Paint::None,
            2 => p.stroke_width = 12.,
            _ => p.opacity = 0.5,
        };
        assert_ne!(before, signature(&changed));
    }
}
#[test]
fn thick_stroke_extent_keeps_screen_edge_content() {
    let mut ed = Editor::new();
    let mut p = path(10);
    for a in &mut p.anchors {
        a.p[0] -= 80.;
    }
    p.stroke_width = 140.;
    ed.doc.paths = vec![p];
    ed.doc.sync_tree();
    let r = painted_extent(&ed.doc.paths[0]);
    assert_eq!(r.2, 10.);
    assert!(!build_scene_in_view(&ed, View::identity(), [100, 100]).content.is_empty());
}
#[test]
fn copied_clip_keeps_mask_and_round_trips() {
    assert!(!GroupRole::Normal.is_mask_group());
    for role in [GroupRole::Clip, GroupRole::MaskAlpha, GroupRole::MaskLuma] {
        assert!(role.is_mask_group());
    }
    let mut ed = Editor::new();
    ed.doc.paths = vec![path(10), path(11)];
    ed.doc.ids = 200;
    ed.doc.sync_tree();
    ed.doc.clip_group(&[10, 11], 11).unwrap();
    ed.objsel.extend([10, 11]);
    ed.copy_selection();
    ed.paste(None);
    let clips: Vec<_> = ed.doc.nodes.iter().filter(|n| n.role.is_mask_group()).collect();
    assert_eq!(clips.len(), 2);
    for n in clips {
        assert!(n.children.contains(&n.mask_child.unwrap()));
    }
    let blob = varos_core::format::encode_model(&ed.doc, &varos_core::format::Limits::DEFAULT).unwrap();
    let decoded =
        varos_core::format::decode_model(blob.as_bytes(), None, &varos_core::format::Limits::DEFAULT).unwrap();
    assert_eq!(decoded.doc, ed.doc);
}

#[test]
fn normalization_never_repairs_unsupported_soft_masks_into_normal_groups() {
    let mut ed = Editor::new();
    ed.doc.paths = vec![path(10), path(11)];
    ed.doc.ids = 200;
    ed.doc.sync_tree();
    let clip = ed.doc.clip_group(&[10, 11], 11).unwrap();
    for role in [GroupRole::MaskAlpha, GroupRole::MaskLuma] {
        for broken in [false, true] {
            let mut doc = ed.doc.clone();
            let n = doc.nodes.iter_mut().find(|n| n.id == clip).unwrap();
            n.role = role;
            if broken {
                n.mask_child = None;
            }
            let bytes = serde_json::to_vec(&serde_json::json!({"varos":4,"doc":doc})).unwrap();
            assert!(varos_core::format::decode_model(&bytes, None, &varos_core::format::Limits::DEFAULT).is_err());
            assert!(varos_core::format::encode_model(&doc, &varos_core::format::Limits::DEFAULT).is_err());
        }
    }
}
