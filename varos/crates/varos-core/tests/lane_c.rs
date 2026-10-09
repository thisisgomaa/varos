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
    let mut cache = varos_core::flatten::FlattenCache::default();
    let a = cache.geometry(&e.doc, 0, 1.);
    assert!(std::sync::Arc::ptr_eq(&a, &cache.geometry(&e.doc, 0, 1.)));
    assert_eq!(cache.stats(), (1, 1));
    e.try_execute(EditCommand::SetCorners {
        path: 1,
        corners: vec![CornerParam { radius: 10., kind: Kind::Round }; 4],
    })
    .unwrap();
    let b = cache.geometry(&e.doc, 0, 1.);
    assert_ne!(a, b);
    assert_eq!(*b, varos_core::flatten::flatten_path(&e.doc, 0, 1.));
    assert_eq!(cache.stats(), (1, 2));
    assert!(std::sync::Arc::ptr_eq(&b, &cache.geometry(&e.doc, 0, 1.)));
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
    let live = format::decode_model(include_bytes!("fixtures/lane_c/next_live_round.json"), None, &limits).unwrap();
    assert_eq!(live.doc.paths[0].corners[1].radius, 10.);
    assert_ne!(varos_core::live_corners::evaluated(&live.doc.paths[0]).anchors, live.doc.paths[0].anchors);
    // integration w2: Live Corners are format 9; refusals assert their specific errors.
    use varos_core::format::{Invalid, LoadError};
    for (b, version) in [
        (include_bytes!("fixtures/lane_c/refused_corners_on_v5.json").as_slice(), 5),
        (include_bytes!("fixtures/lane_c/refused_corners_on_v8.json").as_slice(), 8),
    ] {
        assert_eq!(
            format::decode_model(b, None, &limits).unwrap_err(),
            LoadError::Invalid(Invalid::FieldNotInFormat { field: "corners", version })
        );
    }
    let negative = format::decode_model(include_bytes!("fixtures/lane_c/refused_negative_radius.json"), None, &limits)
        .unwrap_err();
    assert_eq!(negative, LoadError::Invalid(Invalid::NonFinite { what: "path 10 corners".into() }));
    // ---- Lane B w3-effects: v10 is now readable; frozen malformed body still refuses ----
    assert!(matches!(
        format::decode_model(include_bytes!("fixtures/lane_c/refused_future.json"), None, &limits),
        Err(LoadError::Malformed { .. })
    ));
    // ---- end Lane B w3-effects ----
}

#[test]
fn expand_rotated_group_keeps_every_fill_and_stroke_in_world_space() {
    let mut e = ed();
    let mut second = rect();
    second.id = 20;
    for a in &mut second.anchors {
        a.id += 20;
        a.p[0] += 140.;
    }
    e.doc.paths.push(second);
    e.doc.ids = 30;
    e.doc.sync_tree();
    e.objsel.insert(20);
    e.try_execute(EditCommand::GroupSelection).unwrap();
    let unit = e.doc.unit_of(1).unwrap();
    let xf = varos_core::model::Xform { rot: 0.7, ..Default::default() };
    e.doc.set_node_xform(unit, xf);
    let before = e.doc.clone();
    let expected: Vec<_> = before
        .paths
        .iter()
        .map(|p| path_advanced::outline(p).unwrap().anchors.iter().map(|a| xf.apply(a.p)).collect::<Vec<_>>())
        .collect();
    e.try_execute(EditCommand::PathAdvanced(Action::Expand)).unwrap();
    assert_eq!(e.doc.paths.len(), 4);
    assert!(e.doc.node_xform(unit).is_identity());
    let strokes: Vec<_> = e.doc.paths.iter().filter(|p| p.fill == before.paths[0].stroke).collect();
    for (stroke, expected) in strokes.iter().zip(expected) {
        assert_eq!(stroke.anchors.iter().map(|a| a.p).collect::<Vec<_>>(), expected);
    }
    for p in &before.paths {
        let fill = &e.doc.paths[e.doc.pidx(p.id).unwrap()];
        assert_eq!(
            fill.anchors.iter().map(|a| a.p).collect::<Vec<_>>(),
            p.anchors.iter().map(|a| xf.apply(a.p)).collect::<Vec<_>>()
        );
    }
    format::encode_model(&e.doc, &Limits::DEFAULT).unwrap();
    e.execute(EditCommand::Undo).unwrap();
    assert_eq!(e.doc, before);
}

#[test]
fn offset_compound_hole_grows_and_shrinks_with_region() {
    let mut p = rect();
    p.holes = vec![[[30., 30.], [70., 30.], [70., 50.], [30., 50.]]
        .into_iter()
        .map(|p| Anchor { id: 0, p, hin: None, hout: None, smooth: false })
        .collect()];
    let inset = path_advanced::offset(&p, -5., StrokeJoin::Miter, 10.).unwrap();
    let grow = path_advanced::offset(&p, 5., StrokeJoin::Miter, 10.).unwrap();
    assert_eq!(bounds(&inset), [5., 5., 95., 75.]);
    assert_eq!(bounds(&grow), [-5., -5., 105., 85.]);
    assert_eq!(inset.holes.len(), 1);
    assert_eq!(grow.holes.len(), 1);
    let hole_bounds = |p: &Path| {
        let mut h = p.clone();
        h.anchors = h.holes.remove(0);
        h.holes.clear();
        bounds(&h)
    };
    assert_eq!(hole_bounds(&inset), [25., 25., 75., 55.]);
    assert_eq!(hole_bounds(&grow), [35., 35., 65., 45.]);
}

#[test]
fn expand_stroke_stays_adjacent_to_fill_and_inherits_clip_exemption() {
    let mut e = ed();
    let leaf = e.doc.node_of_path(1).unwrap();
    e.doc.set_node_clip_exempt(leaf, true);
    let mut second = rect();
    second.id = 20;
    for a in &mut second.anchors {
        a.id += 20;
    }
    e.doc.paths.push(second);
    e.doc.ids = 30;
    e.doc.sync_tree();
    e.try_execute(EditCommand::PathAdvanced(Action::Expand)).unwrap();
    let layer = e.doc.node(e.doc.active_layer).unwrap();
    let fill_index = layer.children.iter().position(|n| *n == leaf).unwrap();
    let stroke_leaf = layer.children[fill_index - 1];
    assert!(e.doc.node_clip_exempt(stroke_leaf));
    assert_eq!(layer.children[fill_index - 2], e.doc.node_of_path(20).unwrap());
}

#[test]
fn new_document_unit_change_preserves_size_spacing_and_bleed() {
    let mut s = varos_core::new_document::Settings { count: 3, bleed: 3., ..Default::default() };
    let before = s.document().unwrap();
    s.set_units(varos_core::units::Unit::In);
    let after = s.document().unwrap();
    for (a, b) in before.artboards.iter().zip(&after.artboards) {
        for (a, b) in [a.w, a.h, a.x, a.y, a.bleed].into_iter().zip([b.w, b.h, b.x, b.y, b.bleed]) {
            assert!((a - b).abs() < 0.001);
        }
    }
}
#[test]
fn outline_without_stroke_preserves_fill_and_history() {
    let mut e = ed();
    e.doc.paths[0].stroke = varos_core::model::Paint::None;
    e.doc.paths[0].stroke_width = 0.;
    let before = e.doc.clone();
    let rev = e.rev;
    e.try_execute(EditCommand::PathAdvanced(Action::Outline)).unwrap();
    assert_eq!(e.doc, before);
    assert_eq!(e.rev, rev);
}

#[test]
fn live_corner_stroke_outline_and_expand_match_baked_geometry() {
    for kind in [Kind::Round, Kind::Inverted, Kind::Chamfer] {
        for dashed in [false, true] {
            let mut e = ed();
            let p = &mut e.doc.paths[0];
            p.corners = vec![CornerParam { radius: 20., kind }; 4];
            p.stroke_style.join = StrokeJoin::Bevel;
            if dashed {
                p.stroke_style.dash = vec![12., 6.];
            }
            let baked = varos_core::live_corners::evaluated(p);
            let expected = stroke::evaluate(&baked, 0.01, &|| false).unwrap().rings;
            assert_eq!(stroke::evaluate(p, 0.01, &|| false).unwrap().rings, expected);
            let outlined = path_advanced::outline(p).unwrap();
            assert_eq!(outlined, path_advanced::outline(&baked).unwrap());
            e.try_execute(EditCommand::PathAdvanced(Action::Expand)).unwrap();
            let expanded_stroke = e.doc.paths.last().unwrap();
            let rings: Vec<_> = std::iter::once(&expanded_stroke.anchors)
                .chain(&expanded_stroke.holes)
                .map(|r| r.iter().map(|a| a.p).collect::<Vec<_>>())
                .collect();
            assert_eq!(rings, expected);
        }
    }
}

#[test]
fn expand_refuses_translucent_compositing_atomically_but_allows_single_paint() {
    for (opacity, stroke_alpha) in [(0.5, 1.), (1., 0.5)] {
        let mut e = ed();
        e.doc.paths[0].opacity = opacity;
        e.doc.paths[0].stroke = varos_core::model::Paint::Solid([1., 0., 0., stroke_alpha]);
        let before = e.doc.clone();
        let err = e.try_execute(EditCommand::PathAdvanced(Action::Expand)).unwrap_err();
        assert!(err.to_string().contains("compositing"));
        assert_eq!(e.doc, before);
        e.execute(EditCommand::Undo).unwrap();
        assert_eq!(e.doc, before);
        e.doc.paths[0].fill = varos_core::model::Paint::None;
        e.try_execute(EditCommand::PathAdvanced(Action::Expand)).unwrap();
        assert_eq!(e.doc.paths.last().unwrap().opacity, opacity);
    }
}

/// Integration w2 (Lane C × stroke hotfix): the canvas stroke cache re-evaluates when only the live
/// corners of a stroked path change (anchors untouched).
#[test]
fn canvas_stroke_cache_tracks_live_corner_changes() {
    use varos_core::live_corners::{CornerParam, Kind};
    let mut e = ed();
    e.doc.paths[0].stroke = varos_core::model::Paint::Solid([0., 0., 0., 1.]);
    e.doc.paths[0].stroke_width = 4.;
    e.doc.paths[0].stroke_style.dash = vec![6., 3.];
    let p = e.doc.paths[0].clone();
    let xf = e.doc.unit_xform(p.id);
    let sharp = e.canvas_stroke_cache.lookup(&p, xf, 1.).unwrap();
    let cold = e.canvas_stroke_cache.evaluations();
    let mut rounded = p.clone();
    rounded.corners = vec![CornerParam { radius: 10., kind: Kind::Round }; rounded.anchors.len()];
    let round = e.canvas_stroke_cache.lookup(&rounded, xf, 1.).unwrap();
    assert_eq!(e.canvas_stroke_cache.evaluations(), cold + 1);
    assert_ne!(sharp.rings, round.rings);
}
