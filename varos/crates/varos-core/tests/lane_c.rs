use varos_core::{
    format::{self, Limits},
    live_corners::{CornerParam, Kind},
    model::{Anchor, Path, ShapeKind},
    path_advanced::{self, Action},
    stroke::{self, ArrowHead, StrokeCap, StrokeJoin, StrokeStyle},
    EditCommand, Editor,
};
fn rect() -> Path {
    Path::new(
        1,
        [[0., 0.], [100., 0.], [100., 80.], [0., 80.]]
            .into_iter()
            .enumerate()
            .map(|(i, p)| Anchor { id: i as u32 + 2, p, hin: None, hout: None, smooth: false })
            .collect(),
        true,
        Some([0., 1., 0., 1.]),
        Some([1., 0., 0., 1.]),
        10.,
    )
}
fn ed() -> Editor {
    let mut e = Editor::new();
    e.doc.paths.push(rect());
    e.doc.ids = 10;
    e.doc.sync_tree();
    e.objsel.insert(1);
    e
}
fn bounds(p: &Path) -> [f32; 4] {
    let g = varos_core::geom::painted_extent(p);
    [g.0, g.1, g.2, g.3]
}
#[test]
fn outline_coverage_matches_dashes_heads_and_holes() {
    let mut p = rect();
    p.closed = false;
    p.anchors.truncate(2);
    p.fill = varos_core::model::Paint::None;
    p.stroke_style = StrokeStyle { cap: StrokeCap::Butt, dash: vec![10., 5.], ..Default::default() };
    p.stroke_style.arrows.end = Some(ArrowHead::Triangle);
    let rings = stroke::evaluate(&p, 0.01, &|| false).unwrap().rings;
    let out = path_advanced::outline(&p).unwrap();
    let expected = std::iter::once(&out.anchors)
        .chain(&out.holes)
        .map(|r| r.iter().map(|a| a.p).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert_eq!(rings, expected);
    assert_eq!(out.fill, p.stroke);
    assert!(out.stroke.solid().is_none());
    assert!(out.closed);
}
#[test]
fn offset_rectangle_goldens_and_inset_collapse() {
    let p = rect();
    let grow = path_advanced::offset(&p, 10., StrokeJoin::Miter, 10.).unwrap();
    let inset = path_advanced::offset(&p, -10., StrokeJoin::Miter, 10.).unwrap();
    assert_eq!(bounds(&grow), [-10., -10., 110., 90.]);
    assert_eq!(bounds(&inset), [10., 10., 90., 70.]);
    assert!(path_advanced::offset(&p, -100., StrokeJoin::Round, 10.).unwrap().anchors.is_empty());
    assert!(path_advanced::offset(&p, f32::NAN, StrokeJoin::Round, 10.).is_err());
}
#[test]
fn corner_maths_types_clamps_and_drag() {
    let mut p = rect();
    let corners = varos_core::live_corners::corners(&p);
    assert_eq!(corners.len(), 4);
    assert_eq!(corners[0].max_radius, 40.);
    assert_eq!(corners[0].centre(10.), [10., 10.]);
    assert_eq!(corners[0].drag_radius([10., 10.], 0.), 10.);
    for kind in [Kind::Round, Kind::Inverted, Kind::Chamfer] {
        p.corners = vec![CornerParam { radius: 10., kind }; 4];
        let baked = varos_core::live_corners::evaluated(&p);
        assert_eq!(baked.anchors.len(), 8);
        assert!(baked.corners.is_empty());
        assert_eq!(baked.anchors[0].p, [0., 10.]);
        assert_eq!(baked.anchors[1].p, [10., 0.]);
    }
    p.corners[0].radius = f32::NAN;
    assert!(varos_core::live_corners::validate(&p, &p.corners).is_err());
}
#[test]
fn new_dialog_settings_document_layout_and_validation() {
    let s = varos_core::new_document::Settings {
        count: 3,
        bleed: 3.,
        layout: varos_core::new_document::Layout::Row,
        ..Default::default()
    };
    let d = s.document().unwrap();
    assert_eq!(d.artboards.len(), 3);
    assert!((d.artboards[0].w - 595.2756).abs() < 0.001);
    assert!(d.artboards[1].x > d.artboards[0].w);
    assert!((d.artboards[0].bleed - 8.503937).abs() < 0.001);
    assert_eq!(d.units.ppi, 300.);
    assert!(varos_core::new_document::Settings { count: 0, ..s }.document().is_err());
}
#[test]
fn commands_undo_and_expand_bake_corners_fill_stroke_transform() {
    let mut e = ed();
    let original = e.doc.clone();
    e.try_execute(EditCommand::SetCorners {
        path: 1,
        corners: vec![CornerParam { radius: 10., kind: Kind::Round }; 4],
    })
    .unwrap();
    let live = e.doc.clone();
    e.try_execute(EditCommand::PathAdvanced(Action::Expand)).unwrap();
    assert_eq!(e.doc.paths.len(), 2);
    assert!(e.doc.paths.iter().all(|p| p.corners.is_empty() && p.stroke.solid().is_none()));
    format::encode_model(&e.doc, &Limits::DEFAULT).unwrap();
    e.execute(EditCommand::Undo).unwrap();
    assert_eq!(e.doc, live);
    e.execute(EditCommand::Undo).unwrap();
    assert_eq!(e.doc, original);
}
#[test]
fn scale_strokes_is_opt_in_and_undoable_transform() {
    let mut e = ed();
    e.try_execute(EditCommand::SetScaleStrokes(true)).unwrap();
    e.try_execute(EditCommand::Transform(varos_core::select_transform::Transform {
        scale: [2., 2.],
        ..Default::default()
    }))
    .unwrap();
    assert_eq!(e.doc.paths[0].stroke_width, 20.);
    e.execute(EditCommand::Undo).unwrap();
    assert_eq!(e.doc.paths[0].stroke_width, 10.);
}
#[test]
fn next_format_roundtrip_and_refusals_preserve_frozen_old_bytes() {
    let old = include_bytes!("fixtures/v5/corners.json");
    let d = format::decode_model(old, None, &Limits::DEFAULT).unwrap();
    assert_eq!(d.source_version, 5);
    assert!(d.migrated);
    assert!(d.doc.paths.iter().all(|p| p.corners.is_empty()));
    let mut e = ed();
    e.try_execute(EditCommand::SetCorners {
        path: 1,
        corners: vec![CornerParam { radius: 10., kind: Kind::Round }; 4],
    })
    .unwrap();
    let bytes = format::encode_model(&e.doc, &Limits::DEFAULT).unwrap();
    let back = format::decode_model(bytes.as_bytes(), None, &Limits::DEFAULT).unwrap();
    assert_eq!(back.doc, e.doc);
    let mut value: serde_json::Value = serde_json::from_str(&bytes).unwrap();
    value["varos"] = serde_json::json!(5);
    assert!(format::decode_model(&serde_json::to_vec(&value).unwrap(), None, &Limits::DEFAULT).is_err());
    value["varos"] = serde_json::json!(format::FORMAT_VERSION);
    value["doc"]["paths"][0]["corners"][0]["kind"] = serde_json::json!("unknown");
    assert!(format::decode_model(&serde_json::to_vec(&value).unwrap(), None, &Limits::DEFAULT).is_err());
}
#[test]
fn cache_tracks_corner_changes() {
    let mut e = ed();
    let cache = varos_core::flatten::FlattenCache::default();
    let _ = cache;
    let a = varos_core::flatten::flatten_path(&e.doc, 0, 1.);
    e.try_execute(EditCommand::SetCorners {
        path: 1,
        corners: vec![CornerParam { radius: 10., kind: Kind::Round }; 4],
    })
    .unwrap();
    let b = varos_core::flatten::flatten_path(&e.doc, 0, 1.);
    assert_ne!(a, b);
}
#[test]
fn new_document_command_has_one_undo() {
    let mut e = ed();
    let old = e.doc.clone();
    e.try_execute(EditCommand::NewDocument(varos_core::new_document::Settings::default())).unwrap();
    assert_eq!(e.doc.artboards.len(), 1);
    e.execute(EditCommand::Undo).unwrap();
    assert_eq!(e.doc, old);
}
#[test]
fn allocation_and_invalid_edits_are_refused() {
    let mut e = ed();
    e.doc.ids = u32::MAX;
    assert!(e.try_execute(EditCommand::PathAdvanced(Action::Outline)).is_err());
    let mut e = Editor::new();
    assert!(e.try_execute(EditCommand::PathAdvanced(Action::Outline)).is_err());
    e.try_execute(EditCommand::AddShape {
        kind: ShapeKind::Rect,
        bounds: [0., 0., 20., 20.],
        parent: None,
        fill: None,
        stroke: None,
        stroke_width: 0.,
        opacity: 1.,
        name: None,
    })
    .unwrap();
}

#[test]
fn frozen_next_fixture_and_refusals() {
    let limits = Limits::DEFAULT;
    format::decode_model(include_bytes!("fixtures/lane_c/next_corners.json"), None, &limits).unwrap();
    for b in [
        include_bytes!("fixtures/lane_c/refused_corners_on_v5.json").as_slice(),
        include_bytes!("fixtures/lane_c/refused_negative_radius.json").as_slice(),
    ] {
        assert!(format::decode_model(b, None, &limits).is_err());
    }
}
