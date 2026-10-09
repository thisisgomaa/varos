//! Headless slice 0.7 document contracts; no GPU renderer or event loop.
use varos_core::{
    editor::view_commands::ViewAction as V,
    model::{Anchor, Guide, Path},
    EditCommand as C, Editor,
};
fn editor() -> Editor {
    let mut e = Editor::new();
    e.doc.paths.clear();
    e.doc.artboards.clear();
    for (id, x) in [(10, 0.0), (20, 40.0)] {
        e.doc.paths.push(Path::new(
            id,
            (0..4)
                .map(|i| Anchor {
                    id: id + i + 1,
                    p: [[x, 0.0], [x + 10.0, 0.0], [x + 10.0, 10.0], [x, 10.0]][i as usize],
                    hin: None,
                    hout: None,
                    smooth: false,
                })
                .collect(),
            true,
            Some([1.0, 0.0, 0.0, 1.0]),
            None,
            1.0,
        ));
    }
    e.doc.ids = 100;
    e.doc.sync_tree();
    e
}
#[test]
fn path_guides_keep_geometry_paint_and_roundtrip_then_release_and_clear_undo() {
    let mut e = editor();
    let original = e.doc.paths[0].clone();
    e.try_execute(C::SelectPaths(vec![10])).unwrap();
    e.try_execute(C::View(V::MakeGuides)).unwrap();
    assert_eq!(e.doc.guide_paths, vec![10]);
    assert_eq!(e.doc.paths[0], original);
    assert_eq!(e.doc.paint_list().count(), 1);
    let bytes = serde_json::to_vec(&e.doc).unwrap();
    let doc: varos_core::model::Document = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(doc.guide_paths, vec![10]);
    e.try_execute(C::View(V::ReleaseGuides)).unwrap();
    assert!(e.doc.guide_paths.is_empty());
    assert_eq!(e.doc.paths[0], original);
    e.execute(C::Undo).unwrap();
    assert_eq!(e.doc.guide_paths, vec![10]);
    e.doc.guides.push(Guide { vertical: true, pos: 42.0 });
    e.try_execute(C::View(V::ClearGuides)).unwrap();
    assert!(e.doc.guides.is_empty() && e.doc.guide_paths.is_empty());
    assert!(e.doc.pidx(10).is_none());
    e.execute(C::Undo).unwrap();
    assert!(e.doc.pidx(10).is_some());
    assert_eq!(e.doc.guides.len(), 1);
}
#[test]
fn guide_position_is_exact_undoable_and_refuses_locked_or_missing_guides() {
    let mut e = editor();
    e.doc.guides.push(Guide { vertical: true, pos: 42.0 });
    e.try_execute(C::View(V::GuidePosition { index: 0, position: 12.5 })).unwrap();
    assert_eq!(e.doc.guides[0].pos, 12.5);
    e.execute(C::Undo).unwrap();
    assert_eq!(e.doc.guides[0].pos, 42.0);
    e.doc.guides_locked = true;
    assert!(e.try_execute(C::View(V::GuidePosition { index: 0, position: 99.0 })).is_err());
    e.doc.guides_locked = false;
    assert!(e.try_execute(C::View(V::GuidePosition { index: 1, position: 99.0 })).is_err());
}
#[test]
fn grid_spacing_subdivisions_and_visibility_are_document_preferences() {
    let mut e = editor();
    let rev = e.rev;
    e.try_execute(C::View(V::Grid { spacing: 24.0, subdivisions: 3 })).unwrap();
    assert_eq!(e.document_grid_step(), 8.0);
    assert_eq!(varos_core::scene::build_scene(&e, 1.0).grid_step, Some(8.0));
    e.try_execute(C::View(V::ToggleGrid)).unwrap();
    assert_eq!(varos_core::scene::build_scene(&e, 1.0).grid_step, None);
    assert_eq!(e.rev, rev);
    let d: varos_core::model::Document = serde_json::from_value(serde_json::to_value(&e.doc).unwrap()).unwrap();
    assert!(!d.snap.show_grid && d.snap.grid_subdivisions == 3);
    assert!(e.try_execute(C::View(V::Grid { spacing: 0.0, subdivisions: 3 })).is_err());
    assert!(e.try_execute(C::View(V::Grid { spacing: 24.0, subdivisions: 0 })).is_err());
}
#[test]
fn artboard_fit_all_selected_reorder_and_conversion_are_one_step_each() {
    let mut e = editor();
    let a = e.artboard_add([100.0, 100.0, 20.0, 20.0], None).unwrap();
    e.try_execute(C::View(V::FitArtboard { id: a, selected: false })).unwrap();
    assert_rect(e.doc.artboards[0].rect(), (0.0, 0.0, 50.0, 10.0));
    e.try_execute(C::SelectPaths(vec![20])).unwrap();
    e.try_execute(C::View(V::FitArtboard { id: a, selected: true })).unwrap();
    assert_rect(e.doc.artboards[0].rect(), (40.0, 0.0, 50.0, 10.0));
    let b = e.artboard_add([100.0, 100.0, 20.0, 20.0], None).unwrap();
    e.try_execute(C::View(V::ReorderArtboard { id: b, position: 0 })).unwrap();
    assert_eq!(e.doc.artboards[0].id, b);
    e.execute(C::Undo).unwrap();
    assert_eq!(e.doc.artboards[0].id, a);
    e.try_execute(C::SelectPaths(vec![10, 20])).unwrap();
    let rev = e.rev;
    e.try_execute(C::View(V::ConvertArtboards)).unwrap();
    assert_eq!(e.doc.artboards.len(), 4);
    assert!(e.doc.paths.is_empty());
    assert_eq!(e.rev, rev + 1);
    e.execute(C::Undo).unwrap();
    assert_eq!(e.doc.paths.len(), 2);
    assert_eq!(e.doc.artboards.len(), 2);
}
#[test]
fn typed_zoom_request_is_transient_and_checked() {
    let mut e = editor();
    let rev = e.rev;
    e.try_execute(C::ZoomPercent(125.0)).unwrap();
    assert_eq!(e.requested_zoom, Some(125.0));
    assert_eq!(e.rev, rev);
    assert!(e.try_execute(C::ZoomPercent(f32::NAN)).is_err());
    assert!(e.try_execute(C::ZoomPercent(4001.0)).is_err());
}

fn assert_rect(actual: (f32, f32, f32, f32), expected: (f32, f32, f32, f32)) {
    for (a, b) in
        [actual.0, actual.1, actual.2, actual.3].into_iter().zip([expected.0, expected.1, expected.2, expected.3])
    {
        assert!((a - b).abs() < 0.0001);
    }
}

#[test]
fn grid_only_bridge_batch_after_path_guides_preserves_content_revision() {
    let mut e = editor();
    e.try_execute(C::SelectPaths(vec![10])).unwrap();
    e.try_execute(C::View(V::MakeGuides)).unwrap();
    let rev = e.rev;
    let batch = e
        .prepare_design_batch(
            1,
            |stage, _| stage.try_execute(C::View(V::Grid { spacing: 24.0, subdivisions: 3 })),
            |_, e| e,
            || false,
        )
        .unwrap();
    assert!(batch.document().content_eq(&e.doc), "live {:?} staged {:?}", e.doc, batch.document());
    e.publish_design_batch(batch).unwrap();
    assert_eq!(e.rev, rev);
}
