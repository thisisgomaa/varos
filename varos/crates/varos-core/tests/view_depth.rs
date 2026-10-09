//! Lane E headless view behaviour contracts.
use varos_core::{
    editor::view_commands::ViewAction,
    editor::Editor,
    geom::View,
    model::{Anchor, Path},
    scene::{self, Group, Prim, SceneStyle},
    view_depth::DepthAction as D,
    EditCommand,
};
fn editor() -> Editor {
    let mut ed = Editor::new();
    let a = |id, p| Anchor { id, p, hin: None, hout: None, smooth: false };
    ed.doc.paths.push(Path::new(
        10,
        vec![a(11, [10.0, 10.0]), a(12, [30.0, 10.0]), a(13, [30.0, 30.0]), a(14, [10.0, 30.0])],
        true,
        Some([1.0, 0.0, 0.0, 1.0]),
        Some([0.0, 0.0, 0.0, 1.0]),
        4.0,
    ));
    ed.doc.ids = 15;
    ed.doc.sync_tree();
    ed
}
fn style() -> SceneStyle {
    SceneStyle { checkerboard: [[0.1; 4], [0.2; 4]], outline: [0.56, 0.54, 0.53, 1.0], canvas: [0.08, 0.07, 0.07, 1.0] }
}
#[test]
fn outline_draw_list_and_export_unchanged() {
    let mut ed = editor();
    let original = scene::build_scene(&ed, 2.0).content;
    ed.execute(EditCommand::View(ViewAction::Depth(D::Outline))).unwrap();
    let view = View { zoom: 2.0, pan: [0.0, 0.0] };
    let drawn = scene::build_scene_in_view_styled(&ed, view, [100, 100], style());
    let strokes: Vec<_> = drawn
        .content
        .iter()
        .flat_map(Group::prims)
        .filter_map(|p| if let Prim::Stroke { width, color, .. } = p { Some((*width, *color)) } else { None })
        .collect();
    assert_eq!(strokes, vec![(0.5, style().outline)]);
    assert_eq!(scene::build_scene(&ed, 2.0).content, original);
}
#[test]
fn per_layer_outline_does_not_hide_the_layer() {
    let mut ed = editor();
    let leaf = ed.doc.node_of_path(10).unwrap();
    let layer = ed.doc.layer_ancestor(leaf);
    ed.execute(EditCommand::View(ViewAction::Depth(D::OutlineNode { id: layer }))).unwrap();
    ed.doc.paths[0].opacity = 0.25;
    let scene = scene::build_scene_in_view_styled(&ed, View::identity(), [100, 100], style());
    assert!(scene
        .content
        .iter()
        .flat_map(Group::prims)
        .any(|p| matches!(p,Prim::Stroke{color,..} if *color==style().outline)));
    assert!(!ed.doc.eff_hidden(10));
    assert!(varos_core::view_depth_scene::outlined(&ed, 10));
    ed.execute(EditCommand::View(ViewAction::Depth(D::OutlineNode { id: layer }))).unwrap();
    assert!(!varos_core::view_depth_scene::outlined(&ed, 10));
}
#[test]
fn snap_engine_reads_both_pixel_fields_at_document_ppi() {
    let mut ed = Editor::new();
    ed.doc.units.ppi = 144.0;
    ed.doc.snap.smart = false;
    ed.doc.snap.key_points = false;
    ed.doc.snap.object_geometry = false;
    ed.execute(EditCommand::View(ViewAction::Depth(D::SnapPixel))).unwrap();
    assert_eq!(ed.snap_xy([0.37, -0.37], true, true).0, [0.5, -0.5]);
    assert_eq!(ed.snap_anchor(&[[0.0, 0.0]], [0.37, -0.37]).0, [0.5, -0.5]);
    ed.execute(EditCommand::View(ViewAction::Depth(D::SnapPixel))).unwrap();
    ed.execute(EditCommand::View(ViewAction::Depth(D::MoveWholePixel))).unwrap();
    assert_eq!(ed.snap_move((0.1, 0.1, 20.1, 20.1), [0.37, -0.37]).0, [0.5, -0.5]);
}
#[test]
fn checked_commands_refuse_bad_navigation() {
    let mut ed = editor();
    assert!(ed.try_execute(EditCommand::View(ViewAction::Depth(D::NavigatorZoom { percent: 0.0 }))).is_err());
    assert!(ed.try_execute(EditCommand::View(ViewAction::Depth(D::OutlineNode { id: u32::MAX }))).is_err());
}
#[test]
fn preferences_and_drawing_readout() {
    let mut ed = editor();
    ed.execute(EditCommand::View(ViewAction::Depth(D::CanvasColor { rgb: [20, 19, 19] }))).unwrap();
    assert_eq!(ed.requested_canvas, Some([20, 19, 19]));
    let before = ed.doc.transparency_grid;
    ed.execute(EditCommand::View(ViewAction::Depth(D::TransparencyGrid))).unwrap();
    assert_eq!(ed.doc.transparency_grid, !before);
    assert_eq!(varos_core::view_depth::drawing_readout([0.0, 0.0], [3.0, 4.0]).1, "-53.1°   5.00 pt");
}

#[test]
fn trim_pixel_grid_stays_inside_artboards_and_outline_has_no_pixel_grid() {
    let mut ed = editor();
    ed.view_depth.pixel_preview = true;
    ed.view_depth.trim = true;
    let view = View { zoom: 6.0, pan: [0.0, 0.0] };
    let drawn = scene::build_scene_in_view_styled(&ed, view, [600, 600], style());
    assert!(drawn.overlay.is_empty());
    assert!(drawn.grid_step.is_none());
    let Group::Clip { members, .. } = &drawn.content[0] else { panic!("artboard clip missing") };
    assert!(members.iter().flat_map(Group::prims).any(|p| matches!(p,
        Prim::Stroke { width, color, .. } if *width == 1.0 / 6.0 && *color == style().outline)));
    ed.view_depth.trim = false;
    ed.view_depth.outline = true;
    let outline = scene::build_scene_in_view_styled(&ed, view, [600, 600], style());
    assert!(outline.pixel_preview.is_none());
    // Only path centerlines belong in the content; no pixel furniture in outline.
    assert_eq!(outline.content.iter().flat_map(Group::prims).count(), 1);
}

#[test]
fn presentation_settles_drawing_and_loaded_documents_reset_view_requests() {
    let mut ed = editor();
    ed.tool = varos_core::editor::ToolKind::Rect;
    ed.pointer_down([50.0, 50.0]);
    ed.pointer_move([80.0, 80.0]);
    ed.execute_ui(EditCommand::View(ViewAction::Depth(D::Presentation)));
    assert!(!ed.transaction_open());
    let before = ed.doc.clone();
    ed.pointer_down([90.0, 90.0]);
    ed.pointer_move([100.0, 100.0]);
    ed.pointer_up();
    assert_eq!(before, ed.doc);
    ed.requested_pan = Some([100.0, 100.0]);
    ed.requested_zoom = Some(600.0);
    ed.requested_canvas = Some([1, 2, 3]);
    ed.replace_doc(before);
    assert_eq!(ed.view_depth, Default::default());
    assert!(ed.requested_pan.is_none() && ed.requested_zoom.is_none() && ed.requested_canvas.is_none());
}

#[test]
fn outline_reuses_flatten_cache_and_culls_offscreen_geometry() {
    let mut ed = editor();
    let mut far = ed.doc.paths[0].clone();
    far.id = 100;
    for a in &mut far.anchors {
        a.id += 100;
        a.p[0] += 10000.0;
    }
    ed.doc.paths.push(far);
    ed.doc.sync_tree();
    ed.view_depth.outline = true;
    let view = View::identity();
    let first = scene::build_scene_in_view_styled(&ed, view, [100, 100], style());
    let misses = ed.flatten_cache.lock().stats().1;
    assert_eq!(misses, 1, "offscreen geometry is never flattened");
    ed.objsel.insert(10);
    let second = scene::build_scene_in_view_styled(&ed, view, [100, 100], style());
    assert_eq!(ed.flatten_cache.lock().stats().1, misses, "selection changes reuse geometry");
    assert_eq!(first.content, second.content);
    assert_eq!(second.content.iter().flat_map(Group::prims).count(), 1);
    for prim in second.content.iter().flat_map(Group::prims) {
        if let Prim::Stroke { pts, .. } = prim {
            assert!(pts.iter().all(|p| p[0] < 100.0));
        }
    }
}
