use varos_core::{
    model::{Anchor, Document, Path},
    scene::{build_scene, build_scene_in_view, Group, Prim},
    stroke::{
        canvas::{tolerance, ELEMENT_CAP},
        evaluate::evaluate_capped,
    },
    Editor,
};
fn rect(id: u32) -> Path {
    let anchors = [[0., 0.], [200., 0.], [200., 200.], [0., 200.]]
        .into_iter()
        .enumerate()
        .map(|(i, p)| Anchor { id: id * 10 + i as u32, p, hin: None, hout: None, smooth: false })
        .collect();
    let mut p = Path::new(id, anchors, true, None, Some([0., 0., 0., 1.]), 10.);
    p.stroke_style.dash = vec![8., 4., 3., 4., 1., 4.];
    p
}
fn editor(paths: Vec<Path>) -> Editor {
    let mut ed = Editor::new();
    ed.doc = Document { paths, ..Default::default() };
    ed.doc.sync_tree();
    ed
}
#[test]
fn rect_three_pair_dash_at_4000_percent_fits_canvas_cap() {
    let c = evaluate_capped(&rect(1), tolerance(40.), ELEMENT_CAP, &|| false).unwrap();
    assert!(!c.rings.is_empty());
    assert!(c.generated_elements <= ELEMENT_CAP);
    assert_eq!(tolerance(40.), tolerance(400.));
}
#[test]
fn repeated_scene_evaluates_zero_strokes_and_style_edit_only_one() {
    let mut ed = editor(vec![rect(1), rect(2)]);
    let first = build_scene(&ed, 40.);
    let cold = ed.canvas_stroke_cache.evaluations();
    assert_eq!(cold, 2);
    let second = build_scene(&ed, 40.);
    assert_eq!(ed.canvas_stroke_cache.evaluations(), cold);
    assert_eq!(first.content, second.content);
    ed.selected.insert(1);
    build_scene_in_view(&ed, varos_core::geom::View { pan: [-10., -10.], zoom: 40. }, [800, 600]);
    assert_eq!(ed.canvas_stroke_cache.evaluations(), cold);
    ed.doc.paths[0].stroke_style.dash_phase = 2.;
    ed.rev += 1;
    build_scene(&ed, 40.);
    assert_eq!(ed.canvas_stroke_cache.evaluations(), cold + 1);
    ed.doc.paths[0].anchors[0].p[0] += 1.; // live edits invalidate without a revision bump
    build_scene(&ed, 40.);
    assert_eq!(ed.canvas_stroke_cache.evaluations(), cold + 2);
}
#[test]
fn budget_failure_draws_native_round_base_stroke_and_caches_fallback() {
    let mut p = rect(1);
    p.stroke_style.dash = vec![0.0001, 0.0001];
    let ed = editor(vec![p]);
    let scene = build_scene(&ed, 40.);
    assert!(scene.errors.is_empty());
    assert!(scene
        .report
        .notes
        .iter()
        .any(|n| n.message == "stroke simplified at this zoom: dashes/arrows/alignment not shown"));
    assert!(scene
        .content
        .iter()
        .flat_map(Group::prims)
        .any(|p| matches!(p, Prim::Stroke { pts, .. } if pts.len() >= 2)));
    let attempts = ed.canvas_stroke_cache.evaluations();
    assert_eq!(attempts, 5);
    build_scene(&ed, 40.);
    assert_eq!(ed.canvas_stroke_cache.evaluations(), attempts);
}

#[test]
fn oversized_coverage_backs_off_before_native_fallback() {
    let mut p = rect(1);
    for a in &mut p.anchors {
        a.p = a.p.map(|v| v * 5.);
    }
    let ed = editor(vec![p]);
    let scene = build_scene(&ed, 40.);
    assert!(scene.errors.is_empty());
    assert!(ed.canvas_stroke_cache.evaluations() > 1);
    assert!(scene
        .content
        .iter()
        .flat_map(Group::prims)
        .any(|p| matches!(p, Prim::StrokeCoverage { rings, .. } if !rings.is_empty())));
    assert!(scene.report.notes.iter().any(|n| n.kind == "stroke_simplified"));
}

#[test]
fn canvas_has_no_document_wide_stroke_budget() {
    let p = rect(1);
    let cost = evaluate_capped(&p, tolerance(40.), ELEMENT_CAP, &|| false).unwrap().generated_elements;
    let count = 1_000_000 / cost + 1;
    let ed = editor((1..=count).map(|id| rect(id as u32)).collect());
    let scene = build_scene(&ed, 40.);
    assert!(scene.errors.is_empty());
    assert_eq!(
        scene.content.iter().flat_map(Group::prims).filter(|p| matches!(p, Prim::StrokeCoverage { .. })).count(),
        count
    );
    assert_eq!(ed.canvas_stroke_cache.evaluations(), count as u64);
}

#[test]
fn unit_transform_invalidates_only_its_path() {
    let mut ed = editor(vec![rect(1), rect(2)]);
    build_scene(&ed, 40.);
    let count = ed.canvas_stroke_cache.evaluations();
    let unit = ed.doc.unit_of(1).unwrap();
    ed.doc.set_node_xform(unit, varos_core::model::Xform { rot: 0.2, piv: [100., 100.] });
    build_scene(&ed, 40.);
    assert_eq!(ed.canvas_stroke_cache.evaluations(), count + 1);
}

#[test]
fn fallback_band_survives_page_clip_with_centreline_outside() {
    let mut p = rect(1);
    for a in &mut p.anchors {
        a.p = a.p.map(|v| v * 0.51 - 1.);
    }
    p.stroke_style.dash = vec![0.0001, 0.0001];
    let mut ed = editor(vec![p]);
    ed.doc.artboards = vec![varos_core::model::Artboard { w: 100., h: 100., clip: true, ..Default::default() }];
    let clipped = build_scene(&ed, 1.);
    let strokes = |scene: &varos_core::scene::Scene| {
        scene.content.iter().flat_map(Group::prims).filter(|p| matches!(p, Prim::Stroke { .. })).count()
    };
    assert!(strokes(&clipped) >= 1);
    assert!(clipped
        .content
        .iter()
        .flat_map(Group::prims)
        .filter_map(|p| match p {
            Prim::Stroke { clip, .. } => Some(*clip),
            _ => None,
        })
        .all(|clip| clip == Some([0., 0., 100., 100.])));
    assert_eq!(ed.canvas_stroke_cache.evaluations(), 3); // 0.025, 0.05, 0.1; then fallback
    ed.doc.artboards[0].clip = false;
    assert_eq!(strokes(&clipped), strokes(&build_scene(&ed, 1.)));
}

#[test]
fn undo_content_change_evaluates_once_then_reuses() {
    let mut ed = editor(vec![rect(1), rect(2)]);
    ed.objsel.insert(1);
    build_scene(&ed, 1.);
    let cold = ed.canvas_stroke_cache.evaluations();
    ed.execute_ui(varos_core::EditCommand::SetStrokeWidth(12.));
    build_scene(&ed, 1.);
    assert_eq!(ed.canvas_stroke_cache.evaluations(), cold + 1);
    build_scene(&ed, 1.);
    assert_eq!(ed.canvas_stroke_cache.evaluations(), cold + 1);
    ed.undo();
    assert_eq!(ed.doc.paths[0].stroke_width, 10.);
    build_scene(&ed, 1.);
    assert_eq!(ed.canvas_stroke_cache.evaluations(), cold + 2);
    build_scene(&ed, 1.);
    assert_eq!(ed.canvas_stroke_cache.evaluations(), cold + 2);
}
