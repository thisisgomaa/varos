use kurbo::Shape;
use varos_core::{
    effects::{self, Action, Cache, Effect, WarpStyle},
    model::{Anchor, Paint, Path},
    stroke::{self, StrokeCap, StrokeJoin},
    width_profile::WidthProfile,
    EditCommand, Editor, View,
};
fn rect() -> Path {
    Path::new(
        10,
        [[20., 20.], [120., 20.], [120., 100.], [20., 100.]]
            .into_iter()
            .enumerate()
            .map(|(i, p)| Anchor { id: 11 + i as u32, p, hin: None, hout: None, smooth: false })
            .collect(),
        true,
        Some([0., 0.5, 1., 1.]),
        Some([1., 0., 0., 1.]),
        10.,
    )
}
fn ed() -> Editor {
    let mut e = Editor::new();
    e.doc.paths.push(rect());
    e.doc.ids = 30;
    e.doc.sync_tree();
    e.objsel.insert(10);
    e
}
fn transform() -> Effect {
    Effect::Transform { copies: 0, movement: [30., 15.], scale: [1., 1.], rotate: 0., reflect: [false, false] }
}
#[test]
fn offset_golden_retains_stroke_and_source() {
    let mut p = rect();
    p.effects.push(Effect::Offset { delta: 10., join: StrokeJoin::Miter, miter: 10. });
    let before = p.clone();
    let q = effects::evaluate(&p).unwrap();
    assert_eq!(varos_core::geom::kurbo::to_bez_path(&q).bounding_box(), kurbo::Rect::new(10., 10., 130., 110.));
    assert_eq!(q.stroke, p.stroke);
    assert_eq!(q.stroke_width, 10.);
    assert_eq!(p, before);
    assert!(q.effects.is_empty());
}
#[test]
fn zigzag_straight_edge_golden_and_smooth_handles() {
    let mut p = rect();
    p.closed = false;
    p.anchors = vec![
        Anchor { id: 1, p: [0., 0.], hin: None, hout: None, smooth: false },
        Anchor { id: 2, p: [100., 0.], hin: None, hout: None, smooth: false },
    ];
    p.effects = vec![Effect::ZigZag { size: 5., ridges: 3, smooth: false }];
    let q = effects::evaluate(&p).unwrap();
    assert_eq!(
        q.anchors.iter().map(|a| a.p).collect::<Vec<_>>(),
        vec![[0., -5.], [25., 5.], [50., -5.], [75., 5.], [100., -5.]]
    );
    p.effects = vec![Effect::ZigZag { size: 5., ridges: 3, smooth: true }];
    assert!(effects::evaluate(&p).unwrap().anchors.iter().all(|a| a.smooth && a.hout.is_some()));
}
#[test]
fn transform_golden_and_closed_copies() {
    let mut p = rect();
    p.effects = vec![transform()];
    let q = effects::evaluate(&p).unwrap();
    assert_eq!(q.anchors[0].p, [50., 35.]);
    assert_eq!(q.anchors[2].p, [150., 115.]);
    p.effects = vec![Effect::Transform {
        copies: 2,
        movement: [200., 0.],
        scale: [1., 1.],
        rotate: 0.,
        reflect: [false, false],
    }];
    let q = effects::evaluate(&p).unwrap();
    assert_eq!(q.holes.len(), 2);
    assert_eq!(q.holes[0][0].p, [220., 20.]);
    assert_eq!(q.holes[1][0].p, [420., 20.]);
}
#[test]
fn warp_arch_golden_bends_straight_edges() {
    let mut p = rect();
    p.effects = vec![Effect::Warp { style: WarpStyle::Arch, bend: 50., h: 0., v: 0. }];
    let q = effects::evaluate(&p).unwrap();
    assert!(q.anchors.len() > p.anchors.len());
    let midpoint = q.anchors.iter().find(|a| (a.p[0] - 70.).abs() < 0.001 && a.p[1] < 30.).unwrap();
    assert!((midpoint.p[1] - 0.).abs() < 0.001);
}
#[test]
fn every_warp_is_finite_and_zero_is_exact() {
    for style in [
        WarpStyle::Arc,
        WarpStyle::ArcLower,
        WarpStyle::ArcUpper,
        WarpStyle::Arch,
        WarpStyle::Bulge,
        WarpStyle::ShellLower,
        WarpStyle::ShellUpper,
        WarpStyle::Flag,
        WarpStyle::Wave,
        WarpStyle::Fish,
        WarpStyle::Rise,
        WarpStyle::Fisheye,
        WarpStyle::Inflate,
        WarpStyle::Squeeze,
        WarpStyle::Twist,
    ] {
        let mut p = rect();
        p.effects = vec![Effect::Warp { style, bend: 0., h: 0., v: 0. }];
        assert_eq!(effects::evaluate(&p).unwrap().anchors, p.anchors);
        p.effects = vec![Effect::Warp { style, bend: -60., h: 30., v: -40. }];
        let q = effects::evaluate(&p).unwrap();
        assert!(q.anchors.iter().all(|a| a.p.iter().all(|v| v.is_finite())));
    }
}
#[test]
fn ordered_effects_and_cache_invalidation() {
    let mut p = rect();
    p.effects = vec![transform()];
    let mut c = Cache::default();
    let a = c.resolve(&p).unwrap();
    assert_eq!(c.resolve(&p).unwrap(), a);
    assert_eq!(c.evaluations, 1);
    p.anchors[0].p[0] += 2.;
    assert_ne!(c.resolve(&p).unwrap(), a);
    assert_eq!(c.evaluations, 2);
    p.effects.push(Effect::Offset { delta: 2., join: StrokeJoin::Round, miter: 10. });
    c.resolve(&p).unwrap();
    assert_eq!(c.evaluations, 3);
}
#[test]
fn expand_bakes_identically_and_is_one_undo_step() {
    let mut e = ed();
    e.try_execute(EditCommand::LiveEffects(Action::Set { ids: vec![10], effects: vec![transform()] })).unwrap();
    let expected = effects::evaluate(&e.doc.paths[0]).unwrap();
    let rev = e.rev;
    e.try_execute(EditCommand::LiveEffects(Action::Expand { ids: vec![10] })).unwrap();
    assert_eq!(e.rev, rev + 1);
    assert_eq!(
        e.doc.paths[0].anchors.iter().map(|a| a.p).collect::<Vec<_>>(),
        expected.anchors.iter().map(|a| a.p).collect::<Vec<_>>()
    );
    assert!(e.doc.paths[0].effects.is_empty());
    e.undo();
    assert_eq!(e.doc.paths[0].effects, vec![transform()]);
}
#[test]
fn preview_cancel_ok_and_history_restore() {
    let mut e = ed();
    let doc = e.doc.clone();
    let rev = e.rev;
    for x in [10., 20., 30.] {
        e.try_execute(EditCommand::LiveEffects(Action::Preview {
            ids: vec![10],
            effects: vec![Effect::Offset { delta: x, join: StrokeJoin::Round, miter: 10. }],
        }))
        .unwrap();
        assert_eq!(e.rev, rev);
    }
    e.try_execute(EditCommand::LiveEffects(Action::EndPreview { accept: false })).unwrap();
    assert_eq!(e.doc, doc);
    e.try_execute(EditCommand::LiveEffects(Action::Preview { ids: vec![10], effects: vec![transform()] })).unwrap();
    e.try_execute(EditCommand::LiveEffects(Action::EndPreview { accept: true })).unwrap();
    assert_eq!(e.rev, rev + 1);
    e.undo();
    assert_eq!(e.doc.paths, doc.paths);
}
#[test]
fn refuse_atomic_invalid_numeric_and_budgets() {
    let mut e = ed();
    let doc = e.doc.clone();
    for effect in [
        Effect::Offset { delta: f32::NAN, join: StrokeJoin::Miter, miter: 10. },
        Effect::ZigZag { size: 4., ridges: 101, smooth: false },
        Effect::Transform { copies: 1001, movement: [0., 0.], scale: [1., 1.], rotate: 0., reflect: [false, false] },
        Effect::Warp { style: WarpStyle::Arc, bend: 101., h: 0., v: 0. },
    ] {
        assert!(e.try_execute(EditCommand::LiveEffects(Action::Set { ids: vec![10], effects: vec![effect] })).is_err());
        assert_eq!(e.doc, doc);
    }
}
#[test]
fn width_presets_geometry_and_asymmetric_custom() {
    let mut p = rect();
    p.closed = false;
    p.fill = Paint::None;
    p.anchors.truncate(2);
    p.stroke_style.cap = StrokeCap::Butt;
    let bp = varos_core::geom::kurbo::to_bez_path(&p);
    for preset in WidthProfile::PRESETS {
        let profile = WidthProfile::preset(preset.id).unwrap();
        profile.validate().unwrap();
        assert!(varos_core::width_geometry::width_outline(&bp, 10., &profile, &p.stroke_style, 0.01)
            .area()
            .is_finite());
    }
    p.stroke_style.width_profile = Some(WidthProfile::lens());
    let coverage = stroke::evaluate(&p, 0.01, &|| false).unwrap();
    let area: f32 = coverage
        .rings
        .iter()
        .map(|r| {
            (r.iter().zip(r.iter().cycle().skip(1)).map(|(a, b)| a[0] * b[1] - a[1] * b[0]).sum::<f32>() * 0.5).abs()
        })
        .sum();
    assert!((area - 500.).abs() < 0.1);
    let one = WidthProfile { points: vec![(0., 1., 0.), (1., 1., 0.)] };
    let outline = varos_core::width_geometry::width_outline(&bp, 10., &one, &p.stroke_style, 0.01);
    assert_eq!(outline.bounding_box(), kurbo::Rect::new(20., 15., 120., 20.));
}
#[test]
fn dashed_profile_uses_whole_path_fractions() {
    let mut p = rect();
    p.closed = false;
    p.anchors.truncate(2);
    p.stroke_style.cap = StrokeCap::Butt;
    p.stroke_style.dash = vec![20., 10.];
    p.stroke_style.width_profile = Some(WidthProfile::taper_end());
    let rings = stroke::evaluate(&p, 0.01, &|| false).unwrap().rings;
    let first = rings.iter().flatten().filter(|p| p[0] < 40.).map(|p| (p[1] - 20.).abs()).fold(0., f32::max);
    let last = rings.iter().flatten().filter(|p| p[0] > 110.).map(|p| (p[1] - 20.).abs()).fold(0., f32::max);
    assert!(first > 4. && last < 0.6, "{first} {last}");
}
#[test]
fn width_tool_drag_targets_length_fraction_and_one_undo() {
    let mut e = ed();
    e.doc.paths[0].closed = false;
    e.doc.paths[0].anchors.truncate(2);
    e.try_execute(EditCommand::LiveEffects(Action::Tool)).unwrap();
    let rev = e.rev;
    e.pointer_down([70., 20.]);
    e.pointer_move([70., 10.]);
    e.pointer_move([70., 5.]);
    e.pointer_up();
    assert_eq!(e.rev, rev + 1);
    let p = e.doc.paths[0].stroke_style.width_profile.as_ref().unwrap();
    assert!(p.points.iter().any(|(t, l, _)| (*t - 0.5).abs() < 1e-6 && (*l - 3.).abs() < 1e-6));
    e.undo();
    assert!(e.doc.paths[0].stroke_style.width_profile.is_none());
}
#[test]
fn changed_effect_invalidates_flatten_stroke_and_scene() {
    let mut e = ed();
    let view = View::identity();
    let before = varos_core::scene::scene_signature(&e, view, [300, 300]);
    let cache = stroke::canvas::CanvasStrokeCache::default();
    let xf = e.doc.unit_xform(10);
    cache.lookup(&e.doc.paths[0], xf, 1.);
    let n = cache.evaluations();
    e.try_execute(EditCommand::LiveEffects(Action::Set { ids: vec![10], effects: vec![transform()] })).unwrap();
    assert_ne!(before, varos_core::scene::scene_signature(&e, view, [300, 300]));
    cache.lookup(&e.doc.paths[0], xf, 1.);
    assert!(cache.evaluations() > n);
    let n = cache.evaluations();
    cache.lookup(&e.doc.paths[0], xf, 1.);
    assert_eq!(n, cache.evaluations());
    assert!(varos_core::flatten::control_bbox(&e.doc, 0).0 >= 50.);
}
#[test]
fn cli_typed_batch_reaches_effects_and_width() {
    let mut e = ed();
    let commands=varos_core::bridge::parse_batch(br#"{"api":"1.2","commands":[{"LiveEffects":{"action":"set","ids":[10],"effects":[{"type":"offset","delta":2,"join":"Round","miter":10}]}},{"LiveEffects":{"action":"width","ids":[10],"profile":{"points":[[0,0,0],[0.5,1,1],[1,0,0]]}}}]}"#).unwrap();
    e.execute_batch(commands).unwrap();
    assert_eq!(e.doc.paths[0].effects.len(), 1);
    assert_eq!(e.doc.paths[0].stroke_style.width_profile, Some(WidthProfile::lens()));
}

#[test]
fn transform_open_copies_keep_open_contours_and_shared_box_pivot() {
    let mut p = rect();
    p.closed = false;
    p.anchors.truncate(2);
    p.effects = vec![Effect::Transform {
        copies: 2,
        movement: [40., 20.],
        scale: [1., 1.],
        rotate: 0.,
        reflect: [false, false],
    }];
    let parts = effects::evaluate_many(&p).unwrap();
    assert_eq!(parts.len(), 3);
    assert!(parts.iter().all(|p| !p.closed && p.holes.is_empty()));
    assert_eq!(parts[2].anchors[0].p, [100., 60.]);
}
#[test]
fn object_expand_copies_and_width_is_one_undo_step() {
    let mut e = ed();
    e.doc.paths[0].effects = vec![Effect::Transform {
        copies: 1,
        movement: [60., 0.],
        scale: [1., 1.],
        rotate: 0.,
        reflect: [false, false],
    }];
    e.doc.paths[0].stroke_style.width_profile = Some(WidthProfile::lens());
    let before = e.doc.clone();
    let rev = e.rev;
    e.try_execute(EditCommand::PathAdvanced(varos_core::path_advanced::Action::Expand)).unwrap();
    assert_eq!(e.rev, rev + 1);
    assert!(e.doc.paths.len() >= 4);
    assert!(e.doc.paths.iter().all(|p| p.effects.is_empty() && p.stroke_style.width_profile.is_none()));
    e.undo();
    assert_eq!(e.doc.paths, before.paths);
}
#[test]
fn preview_is_cancelled_before_an_unrelated_command() {
    let mut e = ed();
    e.try_execute(EditCommand::LiveEffects(Action::Preview { ids: vec![10], effects: vec![transform()] })).unwrap();
    e.try_execute(EditCommand::SetOpacity(0.5)).unwrap();
    assert!(e.doc.paths[0].effects.is_empty());
    assert!(e.effects_preview.is_none());
    e.undo();
    assert_eq!(e.doc.paths[0].opacity, 1.);
}

#[test]
fn preview_appends_to_mixed_stacks_without_erasing_them() {
    let mut e = ed();
    let mut second = rect();
    second.id = 40;
    for a in &mut second.anchors {
        a.id += 40;
    }
    second.effects = vec![Effect::Offset { delta: 2., join: StrokeJoin::Round, miter: 10. }];
    e.doc.paths.push(second);
    e.doc.ids = 100;
    e.doc.sync_tree();
    let rev = e.rev;
    let before = e.doc.paths.clone();
    e.try_execute(EditCommand::LiveEffects(Action::PreviewAppend { ids: vec![10, 40], effect: transform() })).unwrap();
    e.try_execute(EditCommand::LiveEffects(Action::PreviewAppend { ids: vec![10, 40], effect: transform() })).unwrap();
    assert_eq!(e.doc.paths.iter().find(|p| p.id == 40).unwrap().effects.len(), 2);
    e.try_execute(EditCommand::LiveEffects(Action::EndPreview { accept: true })).unwrap();
    assert_eq!(e.rev, rev + 1);
    e.undo();
    assert_eq!(e.doc.paths, before);
}

#[test]
fn displaced_and_overlapping_copies_hit_the_authored_object() {
    let mut e = ed();
    e.doc.paths[0].stroke_style = Default::default();
    e.doc.paths[0].effects = vec![Effect::Transform {
        copies: 2,
        movement: [200., 0.],
        scale: [1., 1.],
        rotate: 0.,
        reflect: [false, false],
    }];
    assert_eq!(e.path_under([460., 60.]), Some(10));
    assert_eq!(e.path_under([170., 60.]), None);
    e.doc.paths[0].effects = vec![Effect::Transform {
        copies: 1,
        movement: [20., 0.],
        scale: [1., 1.],
        rotate: 0.,
        reflect: [false, false],
    }];
    assert_eq!(e.path_under([70., 60.]), Some(10));
}
