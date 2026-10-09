use varos_core::{
    format::{decode_model, encode_model, Invalid, Limits, LoadError},
    model::{Anchor, Document, Path},
    stroke::{evaluate, ArrowAlign, ArrowHead, StrokeAlign, StrokeCap, StrokeError, StrokeJoin, StrokeStyle},
    EditCommand, Editor,
};
fn anchor(id: u32, p: [f32; 2]) -> Anchor {
    Anchor { id, p, hin: None, hout: None, smooth: false }
}
fn line() -> Path {
    Path::new(10, vec![anchor(11, [0.0, 0.0]), anchor(12, [100.0, 0.0])], false, None, Some([1.0, 0.0, 0.0, 1.0]), 10.0)
}
fn hit(p: &Path, q: [f32; 2]) -> bool {
    varos_core::stroke::evaluate::contains(&evaluate(p, 0.01, &|| false).unwrap().rings, q, 0.0)
}
fn document(p: Path) -> Document {
    let mut d = Document { ids: 100, ..Default::default() };
    d.paths.push(p);
    d.sync_tree();
    d
}
#[test]
fn serde_defaults_and_strictness() {
    let p = line();
    let original = serde_json::to_string(&p).unwrap();
    assert!(!original.contains("stroke_style"));
    let mut v = serde_json::to_value(&p).unwrap();
    v["stroke_style"] = serde_json::json!({});
    assert_eq!(serde_json::from_value::<Path>(v.clone()).unwrap(), p);
    for style in [
        serde_json::json!(null),
        serde_json::json!({"cap":"Flat"}),
        serde_json::json!({"unknown":1}),
        serde_json::json!({"arrows":{"unknown":1}}),
        serde_json::json!({"arrows":null}),
    ] {
        v["stroke_style"] = style;
        assert!(serde_json::from_value::<Path>(v.clone()).is_err());
    }
    let s: StrokeStyle = serde_json::from_str(r#"{"cap":"Butt","dash":[6,3],"arrows":{"end":"Triangle"}}"#).unwrap();
    assert_eq!(serde_json::to_string(&s).unwrap(), r#"{"cap":"Butt","dash":[6.0,3.0],"arrows":{"end":"Triangle"}}"#);
}
#[test]
fn invalid_style_is_symmetric_and_atomic() {
    let mut ed = Editor::new();
    ed.replace_doc(document(line()));
    let original = ed.doc.clone();
    for style in [
        StrokeStyle { dash: vec![1.0], ..Default::default() },
        StrokeStyle { dash: vec![0.0, 0.0], ..Default::default() },
        StrokeStyle { dash: vec![0.00001, 1.0], ..Default::default() },
        StrokeStyle { miter_limit: 0.0, ..Default::default() },
        StrokeStyle { dash_phase: 1_000_001.0, ..Default::default() },
    ] {
        assert!(ed.try_execute(EditCommand::SetStrokeStyle { ids: vec![10], style: style.clone() }).is_err());
        assert_eq!(ed.doc, original);
        let mut d = original.clone();
        d.paths[0].stroke_style = style;
        assert!(encode_model(&d, &Limits::DEFAULT).is_err());
        let bytes = serde_json::to_vec(&serde_json::json!({"varos":5,"doc":d})).unwrap();
        assert!(decode_model(&bytes, None, &Limits::DEFAULT).is_err());
    }
}
#[test]
fn cap_join_and_degenerate_coverage() {
    let mut p = line();
    for cap in [StrokeCap::Butt, StrokeCap::Round, StrokeCap::Square] {
        p.stroke_style.cap = cap;
        assert_eq!(hit(&p, [-4.0, 0.0]), cap != StrokeCap::Butt);
        assert_eq!(hit(&p, [-4.0, 4.0]), cap == StrokeCap::Square);
    }
    p.anchors[1].p = p.anchors[0].p;
    for cap in [StrokeCap::Butt, StrokeCap::Round, StrokeCap::Square] {
        p.stroke_style.cap = cap;
        assert_eq!(hit(&p, [1.0, 1.0]), cap != StrokeCap::Butt);
    }
    p = line();
    p.anchors.push(anchor(13, [100.0, 100.0]));
    for join in [StrokeJoin::Miter, StrokeJoin::Round, StrokeJoin::Bevel] {
        p.stroke_style.join = join;
        assert_eq!(hit(&p, [104.0, -4.0]), join == StrokeJoin::Miter);
    }
    p.stroke_style.join = StrokeJoin::Miter;
    p.stroke_style.miter_limit = 1.0;
    assert!(!hit(&p, [104.0, -4.0]));
}
#[test]
fn dashes_phase_dots_and_fitting() {
    let mut p = line();
    p.stroke_style.cap = StrokeCap::Butt;
    for dash in [vec![6.0, 3.0], vec![6.0, 3.0, 2.0, 3.0], vec![6.0, 3.0, 2.0, 3.0, 1.0, 3.0]] {
        p.stroke_style.dash = dash;
        assert!(hit(&p, [2.0, 0.0]));
        assert!(!hit(&p, [7.0, 0.0]));
    }
    p.stroke_style.dash = vec![6.0, 3.0];
    p.stroke_style.dash_phase = 3.0;
    assert!(!hit(&p, [4.0, 0.0]));
    p.stroke_style.dash_phase = -3.0;
    assert!(!hit(&p, [1.0, 0.0]));
    assert!(hit(&p, [4.0, 0.0]));
    p.stroke_width = 2.0;
    p.stroke_style.dash = vec![0.0, 10.0];
    p.stroke_style.dash_phase = 0.0;
    p.stroke_style.cap = StrokeCap::Round;
    assert!(hit(&p, [10.0, 0.0]));
    assert!(!hit(&p, [5.0, 0.0]));
    p.stroke_style.dash = vec![6.0, 3.0];
    p.stroke_style.align_dashes_to_corners = true;
    let a = evaluate(&p, 0.01, &|| false).unwrap();
    p.stroke_style.dash_phase = 99.0;
    assert_eq!(a, evaluate(&p, 0.01, &|| false).unwrap());
}
#[test]
fn alignment_holes_open_notes_and_arrows() {
    let mut p = line();
    p.stroke_style.align = StrokeAlign::Inside;
    let c = evaluate(&p, 0.01, &|| false).unwrap();
    assert_eq!(c.report.notes[0].kind, "stroke_align_open_center");
    p.anchors =
        vec![anchor(11, [0.0, 0.0]), anchor(12, [100.0, 0.0]), anchor(13, [100.0, 100.0]), anchor(14, [0.0, 100.0])];
    p.closed = true;
    p.stroke_style.align = StrokeAlign::Inside;
    assert!(hit(&p, [5.0, 50.0]));
    assert!(!hit(&p, [-5.0, 50.0]));
    p.stroke_style.align = StrokeAlign::Outside;
    assert!(!hit(&p, [5.0, 50.0]));
    assert!(hit(&p, [-5.0, 50.0]));
    p.holes.push(vec![
        anchor(20, [30.0, 30.0]),
        anchor(21, [70.0, 30.0]),
        anchor(22, [70.0, 70.0]),
        anchor(23, [30.0, 70.0]),
    ]);
    p.stroke_style.align = StrokeAlign::Inside;
    assert!(hit(&p, [25.0, 50.0]));
    assert!(!hit(&p, [35.0, 50.0]));
    p.stroke_style.arrows.end = Some(ArrowHead::Triangle);
    assert!(evaluate(&p, 0.01, &|| false)
        .unwrap()
        .report
        .notes
        .iter()
        .any(|n| n.kind == "stroke_arrows_closed_ignored"));
    for head in ArrowHead::ALL {
        for align in [ArrowAlign::Tip, ArrowAlign::Extend] {
            let mut p = line();
            p.stroke_width = 2.0;
            p.stroke_style.arrows.start = Some(head);
            p.stroke_style.arrows.end = Some(head);
            p.stroke_style.arrows.align = align;
            p.stroke_style.arrows.scale_start = 0.5;
            p.stroke_style.arrows.scale_end = 1.5;
            let c = evaluate(&p, 0.01, &|| false).unwrap();
            assert!(!c.rings.is_empty(), "{head:?}");
            assert!(!varos_core::stroke::evaluate::triangles(&c.rings).unwrap().is_empty());
        }
    }
    let mut p = line();
    p.stroke_style.arrows.end = Some(ArrowHead::CircleOpen);
    assert!(!hit(&p, [80.0, 0.0]));
}
#[test]
fn budgets_cancellation_and_zero_width() {
    let mut p = line();
    p.stroke_style.dash = vec![0.0001, 0.0001];
    assert_eq!(evaluate(&p, 0.01, &|| false).unwrap_err(), StrokeError::LimitExceeded);
    assert_eq!(evaluate(&line(), 0.01, &|| true).unwrap_err(), StrokeError::Cancelled);
    p.stroke_width = 0.0;
    p.stroke_style.arrows.end = Some(ArrowHead::Triangle);
    assert!(evaluate(&p, 0.01, &|| false).unwrap().rings.is_empty());
}
#[test]
fn v4_migration_doc_bytes_and_refusal_order() {
    for name in ["v4_boardless.vrs", "v4_board_meta.vrs"] {
        let bytes = std::fs::read(format!("{}/tests/fixtures/v4/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let l = decode_model(&bytes, None, &Limits::DEFAULT).unwrap();
        assert!(l.migrated);
        let out = encode_model(&l.doc, &Limits::DEFAULT).unwrap();
        assert_eq!(&bytes[20..], &out.as_bytes()[20..]);
    }
    let mut v = serde_json::json!({"varos":4,"doc":document(line())});
    for style in [serde_json::json!({}), serde_json::json!(null)] {
        v["doc"]["paths"][0]["stroke_style"] = style;
        assert!(matches!(
            decode_model(&serde_json::to_vec(&v).unwrap(), None, &Limits::DEFAULT),
            Err(LoadError::Invalid(Invalid::FieldNotInFormat { field: "stroke_style", version: 4 }))
        ));
    }
    assert!(matches!(
        decode_model(format!("{{\"varos\":{},\"doc\":42}}",varos_core::format::FORMAT_VERSION+1).as_bytes(), None, &Limits::DEFAULT),
        Err(LoadError::NewerVersion { found, supported }) if found == varos_core::format::FORMAT_VERSION+1 && supported == varos_core::format::FORMAT_VERSION
    ));
}
#[test]
fn command_undo_redo_and_selection_preserved() {
    let mut ed = Editor::new();
    ed.replace_doc(document(line()));
    let before = ed.doc.clone();
    let style = StrokeStyle { cap: StrokeCap::Square, ..Default::default() };
    ed.try_execute(EditCommand::SetStrokeStyle { ids: vec![10], style: style.clone() }).unwrap();
    assert!(ed.objsel.is_empty());
    assert_eq!(ed.doc.paths[0].stroke_style, style);
    ed.undo();
    assert_eq!(ed.doc, before);
    ed.redo();
    assert_eq!(ed.doc.paths[0].stroke_style, style);
}

#[test]
fn styled_marquee_tracks_dashes_and_arrow_bounds() {
    let mut p = line();
    p.stroke_style = StrokeStyle { cap: StrokeCap::Butt, dash: vec![6.0, 12.0], ..Default::default() };
    let mut ed = Editor::new();
    ed.replace_doc(document(p));
    assert!(ed.path_in_rect(0, 1.0, -1.0, 2.0, 1.0));
    assert!(!ed.path_in_rect(0, 10.0, -1.0, 11.0, 1.0));
    ed.doc.paths[0].stroke_style.arrows.end = Some(ArrowHead::Triangle);
    assert!(ed.path_in_rect(0, 78.0, 8.0, 80.0, 9.0));
    let b = varos_core::geom::painted_extent(&ed.doc.paths[0]);
    assert!(b.1 <= -20.0 && b.3 >= 20.0);
}
#[test]
fn frozen_head_geometry_insets_and_reversed_short_curves() {
    let frozen: serde_json::Value = serde_json::from_str(include_str!("fixtures/v5/head_library.json")).unwrap();
    for (index, head) in ArrowHead::ALL.into_iter().enumerate() {
        let (outline, inset) = varos_core::stroke::heads::geometry(head);
        assert_eq!(frozen[index], serde_json::json!({"head":head,"outline":outline.to_svg(),"inset":inset}));
        for reverse in [false, true] {
            let mut p = line();
            p.anchors[1].p = [0.01, 0.001];
            p.anchors[0].hout = Some([0.001, 0.002]);
            p.anchors[1].hin = Some([0.009, -0.001]);
            if reverse {
                p.anchors.reverse();
                for a in &mut p.anchors {
                    std::mem::swap(&mut a.hin, &mut a.hout);
                }
            }
            p.stroke_style.arrows.start = Some(head);
            p.stroke_style.arrows.end = Some(head);
            for placement in [ArrowAlign::Tip, ArrowAlign::Extend] {
                p.stroke_style.arrows.align = placement;
                let c = evaluate(&p, 0.01, &|| false).unwrap();
                assert!(!c.rings.is_empty(), "{head:?} {reverse} {placement:?}");
            }
        }
    }
}
#[test]
fn generation_cancelled_during_work_and_job_budget_is_aggregate() {
    let count = std::cell::Cell::new(0);
    let mut p = line();
    p.stroke_style.dash = vec![0.1, 0.1];
    assert_eq!(
        evaluate(&p, 0.01, &|| {
            count.set(count.get() + 1);
            count.get() > 10
        })
        .unwrap_err(),
        StrokeError::Cancelled
    );
    let mut budget = varos_core::stroke::evaluate::StrokeBudget::default();
    let coverage = varos_core::stroke::evaluate::StrokeCoverage { generated_elements: 600_000, ..Default::default() };
    budget.charge(&coverage).unwrap();
    assert_eq!(budget.charge(&coverage), Err(StrokeError::LimitExceeded));
}
#[test]
fn refused_style_fixture_corpus() {
    for (name, bytes) in [
        ("stroke_null", include_bytes!("fixtures/refused/stroke_null.json").as_slice()),
        ("stroke_odd_dash", include_bytes!("fixtures/refused/stroke_odd_dash.json").as_slice()),
        ("stroke_zero_period", include_bytes!("fixtures/refused/stroke_zero_period.json").as_slice()),
        ("stroke_unknown", include_bytes!("fixtures/refused/stroke_unknown.json").as_slice()),
        ("stroke_on_v4", include_bytes!("fixtures/refused/stroke_on_v4.json").as_slice()),
    ] {
        assert!(decode_model(bytes, None, &Limits::DEFAULT).is_err(), "{name}");
    }
}

#[test]
fn styled_marquee_obeys_clip_mask_coverage() {
    let mut p = line();
    p.stroke_style.cap = StrokeCap::Butt;
    let mut d = document(p);
    d.paths.push(Path::new(
        30,
        vec![anchor(31, [20.0, -10.0]), anchor(32, [40.0, -10.0]), anchor(33, [40.0, 10.0]), anchor(34, [20.0, 10.0])],
        true,
        Some([1.0; 4]),
        None,
        1.0,
    ));
    d.sync_tree();
    d.clip_group(&[10, 30], 30).unwrap();
    let mut ed = Editor::new();
    ed.replace_doc(d);
    assert!(!ed.path_in_rect(0, 5.0, -1.0, 6.0, 1.0));
    assert!(ed.path_in_rect(0, 25.0, -1.0, 26.0, 1.0));
}

#[test]
fn corner_fitting_does_not_split_a_smooth_closed_seam() {
    let mut p = line();
    p.closed = true;
    p.anchors = [[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]]
        .into_iter()
        .enumerate()
        .map(|(i, p)| anchor(11 + i as u32, p))
        .collect();
    p.stroke_style.dash = vec![6.0, 3.0];
    p.stroke_style.align_dashes_to_corners = true;
    let canonical = evaluate(&p, 0.01, &|| false).unwrap();
    p.anchors = [[50.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0], [0.0, 0.0]]
        .into_iter()
        .enumerate()
        .map(|(i, p)| anchor(11 + i as u32, p))
        .collect();
    let smooth_seam = evaluate(&p, 0.01, &|| false).unwrap();
    for x in -4..105 {
        for y in [-4.137, -0.137, 0.137, 4.137, 95.137, 99.137, 100.137, 104.137] {
            let q = [x as f32 + 0.137, y];
            assert_eq!(
                varos_core::stroke::evaluate::contains(&canonical.rings, q, 0.0),
                varos_core::stroke::evaluate::contains(&smooth_seam.rings, q, 0.0),
                "{q:?}"
            );
        }
    }
}

#[test]
fn path_reverse_swaps_head_orientation_and_scales_with_undo() {
    let mut p = line();
    p.stroke_style.arrows.start = Some(ArrowHead::Triangle);
    p.stroke_style.arrows.end = Some(ArrowHead::Circle);
    p.stroke_style.arrows.scale_start = 0.5;
    p.stroke_style.arrows.scale_end = 2.0;
    let mut ed = Editor::new();
    ed.replace_doc(document(p));
    ed.objsel.insert(10);
    let original = ed.doc.clone();
    ed.begin();
    ed.reverse(0);
    ed.dirty = true; // the existing path-operation gesture owns the transaction
    ed.commit();
    assert_eq!(ed.doc.paths[0].stroke_style.arrows.start, Some(ArrowHead::Circle));
    assert_eq!(ed.doc.paths[0].stroke_style.arrows.scale_start, 2.0);
    assert_eq!(ed.doc.paths[0].stroke_style.arrows.end, Some(ArrowHead::Triangle));
    assert_eq!(ed.doc.paths[0].stroke_style.arrows.scale_end, 0.5);
    ed.undo();
    assert_eq!(ed.doc, original);
}

#[test]
fn coverage_tessellation_refuses_nonfinite_world_coordinates() {
    assert_eq!(
        varos_core::stroke::evaluate::triangles(&[vec![[f32::INFINITY, 0.0], [1.0, 1.0], [0.0, 2.0]]]),
        Err(StrokeError::Numeric)
    );
}

#[test]
fn identical_stroke_style_preserves_revision_history_and_redo() {
    let mut ed = Editor::new();
    ed.replace_doc(document(line()));
    let original = ed.doc.clone();
    let style = StrokeStyle { cap: StrokeCap::Butt, ..Default::default() };
    ed.try_execute(EditCommand::SetStrokeStyle { ids: vec![10], style: style.clone() }).unwrap();
    ed.undo();
    let rev = ed.rev;
    ed.try_execute(EditCommand::SetStrokeStyle { ids: vec![10], style: StrokeStyle::default() }).unwrap();
    assert_eq!(ed.rev, rev);
    assert!(!ed.dirty);
    assert_eq!(ed.doc, original);
    ed.redo();
    assert_eq!(ed.doc.paths[0].stroke_style, style);
    ed.undo();
    assert_eq!(ed.doc, original);
}

#[test]
fn tangent_check_uses_active_handles_without_arc_length_integration() {
    let mut p = line();
    p.anchors[1].p = p.anchors[0].p;
    assert!(!varos_core::stroke::evaluate::has_length(&p));
    p.anchors[0].hin = Some([10.0, 0.0]); // inactive on an open contour
    assert!(!varos_core::stroke::evaluate::has_length(&p));
    p.anchors[0].hout = Some([10.0, 0.0]);
    assert!(varos_core::stroke::evaluate::has_length(&p));
}

#[test]
fn reverse_path_swaps_authored_heads_and_scales_and_is_involutive() {
    let mut p = line();
    p.stroke_style.arrows.start = Some(ArrowHead::Triangle);
    p.stroke_style.arrows.end = Some(ArrowHead::Bar);
    p.stroke_style.arrows.scale_start = 2.0;
    p.stroke_style.arrows.scale_end = 3.0;
    let mut ed = Editor::new();
    ed.replace_doc(document(p));
    let original = ed.doc.clone();
    ed.reverse(0);
    let arrows = &ed.doc.paths[0].stroke_style.arrows;
    assert_eq!(arrows.start, Some(ArrowHead::Bar));
    assert_eq!(arrows.end, Some(ArrowHead::Triangle));
    assert_eq!((arrows.scale_start, arrows.scale_end), (3.0, 2.0));
    ed.reverse(0);
    assert_eq!(ed.doc, original);
}

#[test]
fn dash_difference_preserves_each_targets_unedited_gaps() {
    let base = StrokeStyle { dash: vec![6.0, 3.0], ..Default::default() };
    let mut next = base.clone();
    next.dash[0] = 7.0;
    let mut target = StrokeStyle { dash: vec![10.0, 8.0, 4.0, 5.0], ..Default::default() };
    varos_core::stroke::apply_difference(&base, &next, &mut target);
    assert_eq!(target.dash, vec![7.0, 8.0, 4.0, 5.0]);
}
