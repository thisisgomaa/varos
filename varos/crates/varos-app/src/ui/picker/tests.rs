use super::*;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput};

fn selected(mixed: bool) -> Editor {
    let mut ed = Editor::new();
    for (id, c) in [(1, [1.0, 0.0, 0.0, 1.0]), (2, [0.0, 0.0, 1.0, 1.0])] {
        ed.doc.paths.push(varos_core::model::Path::new(id, vec![], true, Some(c), Some(c), 2.0));
        if id == 1 || mixed {
            ed.objsel.insert(id);
        }
    }
    ed.doc.sync_tree();
    ed
}
fn key(key: Key) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }
}
fn pointer(pos: Pos2, down: bool) -> Vec<Event> {
    vec![
        Event::PointerMoved(pos),
        Event::PointerButton { pos, button: PointerButton::Primary, pressed: down, modifiers: Modifiers::NONE },
    ]
}
struct Rig {
    ctx: egui::Context,
    ed: Editor,
    panel: Option<ColorPanel>,
    layout: PickerLayout,
    pending: Option<super::super::fields::Pending>,
    time: f64,
    delay: std::time::Duration,
}
impl Rig {
    fn new(ed: Editor) -> Self {
        super::super::fields::tests::clear_probes();
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        t::apply(&ctx);
        let mut r = Self {
            ctx,
            ed,
            panel: None,
            layout: PickerLayout { open: true, position: Some([300.0, 84.0]), ..Default::default() },
            pending: None,
            time: 0.0,
            delay: std::time::Duration::ZERO,
        };
        open_picker(&mut r.panel, MTarget::Paint(PaintTarget::Fill), &mut r.ed);
        r.frame(vec![], None);
        r.frame(vec![], None);
        r
    }
    fn frame(&mut self, events: Vec<Event>, sample: Option<Rgba>) {
        super::super::layout::prepare_picker_input(
            &self.ctx,
            None,
            &mut self.pending,
            &mut self.panel,
            &mut self.ed,
            &RawInput { events: events.clone(), ..Default::default() },
        );
        if let Some(m) = &mut self.panel {
            follow_selection(m, &mut self.ed);
        }
        let s = Snap::read(&self.ed);
        let mut ops = vec![];
        self.time += 0.02;
        let board = egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(1200.0, 900.0));
        let modifiers = events
            .iter()
            .rev()
            .find_map(|e| match e {
                Event::Key { modifiers, .. } | Event::PointerButton { modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or_default();
        let output = self.ctx.run_ui(
            RawInput { screen_rect: Some(board), time: Some(self.time), modifiers, events, ..Default::default() },
            |ui| {
                build_color_panel(ui.ctx(), &mut self.panel, &s, &mut ops, sample, board, &mut self.layout);
            },
        );
        self.delay = output.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
        super::super::fields::finish_frame(&self.ctx, None, &mut ops, &mut self.pending);
        apply_picker_frame(&mut self.ed, s.snap_config, ops, &mut self.panel);
    }
    fn ring(&self, h: f32) -> Pos2 {
        wheel::ring_pos(egui::pos2(423.0, 234.0), t::PICKER_RING_R - t::PICKER_RING_BAND / 2.0, h)
    }
    fn edit_hex(&mut self, text: &str) {
        super::super::fields::tests::clear_probes();
        self.frame(vec![], None);
        let at = super::super::fields::tests::probed_rect("picker hex", 0).center();
        self.frame(pointer(at, true), None);
        self.frame(pointer(at, false), None);
        self.frame(
            vec![
                Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                },
                Event::Text(text.into()),
            ],
            None,
        );
    }
}
#[test]
fn triangle_round_trip_including_rotation_and_vertices() {
    for h in [0.0, 0.1, 0.25, 0.6, 0.999] {
        for s in [0.0, 0.25, 0.7, 1.0] {
            for v in [0.0, 0.2, 0.8, 1.0] {
                let p = wheel::sv_pos(h, 80.0, s, v);
                let [ss, vv] = wheel::pos_sv(h, 80.0, p);
                assert!((vv - v).abs() < 1e-5);
                if v > 0.0 {
                    assert!((ss - s).abs() < 1e-5);
                }
            }
        }
        let [a, b, c] = wheel::vertices(h, 80.0);
        assert!((a.distance(wheel::ring_pos(Pos2::ZERO, 80.0, h))) < 1e-5);
        assert_eq!(wheel::pos_sv(h, 80.0, c)[1], 0.0);
        assert!((wheel::pos_sv(h, 80.0, a)[0] - 1.0).abs() < 1e-5);
        assert!(wheel::pos_sv(h, 80.0, b)[0] < 1e-5);
    }
}
#[test]
fn triangle_outside_drag_clamps_to_an_edge() {
    for p in [egui::pos2(500.0, -80.0), egui::pos2(-100.0, -120.0)] {
        let [s, v] = wheel::pos_sv(0.45, 80.0, p);
        assert!((0.0..=1.0).contains(&s));
        assert!((0.0..=1.0).contains(&v));
    }
}
#[test]
fn ring_angles_hits_and_red_seam() {
    let c = egui::pos2(50.0, 100.0);
    for h in [0.0, 0.125, 0.25, 0.5, 0.75, 0.9999] {
        let p = wheel::ring_pos(c, 92.0, h);
        assert!((wheel::ring_hue(c, p) - h).abs() < 1e-5);
        assert!(wheel::ring_hit(c, p, 99.0, 14.0));
    }
    assert!(!wheel::ring_hit(c, c, 99.0, 14.0));
    assert!(!wheel::ring_hit(c, c + egui::vec2(100.0, 0.0), 99.0, 14.0));
}
#[test]
fn drag_commits_exactly_one_step_and_escape_never_reverts() {
    let mut r = Rig::new(selected(false));
    let before = r.ed.doc.clone();
    r.frame(pointer(r.ring(0.1), true), None);
    assert!(r.ed.transaction_open());
    assert_eq!(r.ed.rev, 0);
    for h in [0.15, 0.22, 0.33] {
        r.frame(vec![Event::PointerMoved(r.ring(h))], None);
        assert_eq!(r.ed.rev, 0);
    }
    r.frame(pointer(r.ring(0.33), false), None);
    assert_eq!(r.ed.rev, 1);
    assert!(!r.ed.transaction_open());
    let after = r.ed.doc.clone();
    r.frame(vec![key(Key::Escape)], None);
    assert!(r.panel.is_none());
    assert!(!r.layout.open);
    assert_eq!(r.ed.doc, after);
    r.ed.undo();
    assert_eq!(r.ed.doc, before);
    assert!(!r.ed.history_available(false));
}
#[test]
fn unchanged_press_release_and_enter_record_nothing() {
    let mut r = Rig::new(selected(false));
    let before = r.ed.doc.clone();
    r.frame(pointer(r.ring(0.0), true), None);
    r.frame(pointer(r.ring(0.0), false), None);
    r.frame(vec![key(Key::Enter)], None);
    assert_eq!(r.ed.doc, before);
    assert_eq!(r.ed.rev, 0);
    assert!(!r.ed.transaction_open());
    assert!(r.panel.is_some());
    assert!(r.ed.recent_colors.is_empty());
}
#[test]
fn drag_returning_to_original_records_no_step() {
    let mut r = Rig::new(selected(false));
    let before = r.ed.doc.clone();
    r.frame(pointer(r.ring(0.2), true), None);
    r.frame(vec![Event::PointerMoved(r.ring(0.0))], None);
    r.frame(pointer(r.ring(0.0), false), None);
    assert_eq!(r.ed.doc, before);
    assert_eq!(r.ed.rev, 0);
    assert!(!r.ed.history_available(false));
}
#[test]
fn mixed_open_is_read_only_until_first_change_then_applies_to_all() {
    let mut r = Rig::new(selected(true));
    let before = r.ed.doc.clone();
    assert!(r.panel.as_ref().unwrap().mixed);
    for _ in 0..4 {
        r.frame(vec![], None);
    }
    assert_eq!(r.ed.doc, before);
    assert!(!r.ed.transaction_open());
    r.frame(pointer(r.ring(0.3), true), None);
    assert!(!Snap::read(&r.ed).fill_mixed);
    r.frame(pointer(r.ring(0.3), false), None);
    assert_eq!(r.ed.rev, 1);
    r.ed.undo();
    assert_eq!(r.ed.doc, before);
}
#[test]
fn follows_selection_same_frame_and_empty_tool_defaults() {
    let mut r = Rig::new(selected(false));
    r.ed.objsel.clear();
    r.ed.objsel.insert(2);
    r.frame(vec![], None);
    assert_eq!(r.panel.as_ref().unwrap().last_sent, Some([0.0, 0.0, 1.0, 1.0]));
    assert_eq!(r.ed.rev, 0);
    r.ed.objsel.clear();
    r.ed.cur_fill = Some([0.2, 0.5, 0.8, 1.0]);
    r.frame(vec![], None);
    assert_eq!(r.panel.as_ref().unwrap().last_sent, r.ed.cur_fill);
    assert!(!r.ed.transaction_open());
}
#[test]
fn open_panel_does_not_reserve_transaction_and_foreign_edit_reseeds() {
    let mut r = Rig::new(selected(false));
    assert!(!r.ed.transaction_open());
    let c = [0.1, 0.8, 0.2, 1.0];
    r.ed.execute(EditCommand::ApplyPaint { target: PaintTarget::Fill, color: Some(c) });
    r.frame(vec![], None);
    assert_eq!(r.panel.as_ref().unwrap().last_sent, Some(c));
    assert!(!r.ed.transaction_open());
}
#[test]
fn hex_enter_blur_escape_invalid_and_unchanged_follow_k3() {
    let mut r = Rig::new(selected(false));
    r.edit_hex("00FF00");
    assert_eq!(r.ed.rev, 0);
    r.frame(vec![key(Key::Enter)], None);
    assert_eq!(r.ed.rev, 1);
    assert_eq!(r.ed.doc.paths[0].fill.solid(), Some([0.0, 1.0, 0.0, 1.0]));
    r.edit_hex("0000FF");
    r.frame(pointer(egui::pos2(900.0, 600.0), true), None);
    r.frame(pointer(egui::pos2(900.0, 600.0), false), None);
    r.frame(vec![], None);
    assert_eq!(r.ed.rev, 2);
    r.edit_hex("FFFFFF");
    r.frame(vec![key(Key::Escape)], None);
    assert_eq!(r.ed.rev, 2);
    assert!(r.panel.is_some());
    r.edit_hex("bad-value");
    r.frame(vec![key(Key::Enter)], None);
    assert_eq!(r.ed.rev, 2);
    assert!(kit::field::blocked(&r.ctx));
    r.frame(vec![key(Key::Escape)], None);
    assert!(r.panel.is_some());
    assert!(!kit::field::any_open(&r.ctx));
    r.edit_hex("0000FF");
    r.frame(vec![key(Key::Enter)], None);
    assert_eq!(r.ed.rev, 2);
}
#[test]
fn hex_preserves_alpha_and_lifecycle_settle_commits_pending_field() {
    let mut ed = selected(false);
    ed.doc.paths[0].fill = varos_core::model::Paint::Solid([1.0, 0.0, 0.0, 0.4]);
    let mut r = Rig::new(ed);
    r.edit_hex("00FF00");
    assert!(super::super::fields::settle(&r.ctx, None, &mut r.pending, &mut r.ed));
    assert_eq!(r.ed.doc.paths[0].fill.solid(), Some([0.0, 1.0, 0.0, 0.4]));
    assert_eq!(r.ed.rev, 1);
}
#[test]
fn no_selection_edits_defaults_without_document_history() {
    let mut r = Rig::new(Editor::new());
    let before = r.ed.doc.clone();
    r.edit_hex("FF0000");
    r.frame(vec![key(Key::Enter)], None);
    assert_eq!(r.ed.cur_fill, Some([1.0, 0.0, 0.0, 1.0]));
    assert_eq!(r.ed.doc, before);
    assert_eq!(r.ed.rev, 0);
}
#[test]
fn focus_switch_and_each_swatch_are_separate_undo_steps() {
    let mut r = Rig::new(selected(false));
    let before = r.ed.doc.clone();
    let c = [0.2, 0.5, 0.7, 1.0];
    apply_picker_frame(
        &mut r.ed,
        Snap::read(&Editor::new()).snap_config,
        vec![Op::PickerSet(MTarget::Paint(PaintTarget::Fill), c), Op::PaintFocus(PaintTarget::Stroke)],
        &mut r.panel,
    );
    assert_eq!(r.ed.rev, 1);
    assert!(r.panel.as_ref().unwrap().target == MTarget::Paint(PaintTarget::Stroke));
    apply_ops(&mut r.ed, vec![Op::PickerSet(MTarget::Paint(PaintTarget::Stroke), c)]);
    assert_eq!(r.ed.rev, 2);
    r.ed.undo();
    r.ed.undo();
    assert_eq!(r.ed.doc, before);
}
#[test]
fn eyedropper_preview_click_commits_once_and_escape_keeps_it() {
    let mut r = Rig::new(selected(false));
    r.panel.as_mut().unwrap().arm();
    let c = [0.0, 1.0, 0.0, 1.0];
    r.frame(vec![], Some(c));
    assert_eq!(r.ed.rev, 0);
    r.frame(pointer(egui::pos2(80.0, 80.0), true), Some(c));
    assert_eq!(r.ed.rev, 1);
    assert!(!r.panel.as_ref().unwrap().eyedropping);
    r.frame(pointer(egui::pos2(80.0, 80.0), false), None);
    r.frame(vec![key(Key::Escape)], None);
    assert_eq!(r.ed.doc.paths[0].fill.solid(), Some(c));
}
#[test]
fn raster_snapshot_is_once_per_view_not_per_live_preview() {
    let ed = selected(false);
    let mut m = ColorPanel::new(MTarget::Paint(PaintTarget::Fill), None, false);
    m.arm();
    let hole = egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(100.0, 100.0));
    let mut view = View { pan: [0.0, 0.0], zoom: 1.0 };
    prepare_canvas_sample(&mut m, &ed, view, 2.0, hole);
    assert_eq!(m.sampling.as_ref().unwrap().builds, 1);
    m.adopt([0.0, 1.0, 0.0, 1.0]);
    prepare_canvas_sample(&mut m, &ed, view, 2.0, hole);
    assert_eq!(m.sampling.as_ref().unwrap().builds, 1);
    view.pan[0] = 20.0;
    prepare_canvas_sample(&mut m, &ed, view, 2.0, hole);
    assert_eq!(m.sampling.as_ref().unwrap().builds, 2);
}
#[test]
fn idle_panel_sleeps_via_pacing_plan() {
    let mut r = Rig::new(selected(false));
    r.frame(vec![Event::PointerMoved(egui::pos2(1100.0, 800.0))], None);
    for _ in 0..8 {
        r.frame(vec![], None);
    }
    assert_eq!(r.delay, std::time::Duration::MAX);
    r.panel.as_mut().unwrap().arm();
    for _ in 0..8 {
        r.frame(vec![], None);
    }
    assert_eq!(r.delay, std::time::Duration::MAX);
    let now = std::time::Instant::now();
    let p = crate::pacing::plan(now, None, &[], false);
    assert!(!p.redraw);
    assert_eq!(p.flow, crate::pacing::Flow::Wait);
}
#[test]
fn layout_v1_missing_picker_is_closed_and_new_preferences_round_trip() {
    let mut value = serde_json::to_value(varos_app::storage::layout::Layout::default()).unwrap();
    value.as_object_mut().unwrap().remove("picker");
    let old: varos_app::storage::layout::Layout = serde_json::from_value(value).unwrap();
    assert_eq!(old.picker, PickerLayout::default());
    let layout = PickerLayout {
        open: true,
        position: Some([44.0, 70.0]),
        drawer_open: true,
        drawer_tab: 2,
        mode: Default::default(),
        harmony: Default::default(),
    };
    assert_eq!(serde_json::from_str::<PickerLayout>(&serde_json::to_string(&layout).unwrap()).unwrap(), layout);
}

#[test]
fn canvas_pointer_sampling_uses_retina_scale_camera_and_rejects_chrome() {
    let ctx = egui::Context::default();
    let mut ed = Editor::new();
    ed.doc.artboards.push(varos_core::model::Artboard {
        x: 20.0,
        y: 30.0,
        w: 100.0,
        h: 100.0,
        page_color: Some([0.0, 1.0, 0.0, 1.0]),
        ..Default::default()
    });
    let view = View { pan: [40.0, 50.0], zoom: 2.0 };
    let hole = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(300.0, 300.0));
    let mut sample = None;
    for (pos, chrome) in
        [(egui::pos2(60.0, 75.0), false), (egui::pos2(400.0, 400.0), false), (egui::pos2(60.0, 75.0), true)]
    {
        let _ = ctx.run_ui(RawInput { events: vec![Event::PointerMoved(pos)], ..Default::default() }, |ui| {
            if chrome {
                egui::Area::new(egui::Id::new("sample-chrome"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(egui::pos2(40.0, 50.0))
                    .show(ui.ctx(), |ui| {
                        ui.allocate_space(egui::vec2(80.0, 80.0));
                    });
            }
            let mut modal = ColorPanel::new(MTarget::Paint(PaintTarget::Fill), None, false);
            modal.eyedropping = true;
            prepare_canvas_sample(&mut modal, &ed, view, 2.0, hole);
            sample = picker_canvas_sample(ui.ctx(), &modal);
        });
        if !chrome && hole.contains(pos) {
            assert_eq!(sample, Some([0.0, 1.0, 0.0, 1.0]));
        } else {
            assert!(sample.is_none());
        }
    }
}

#[test]
fn swatches_focus_on_click_and_open_on_double_click_in_all_homes() {
    for home in 0..3 {
        for target in [PaintTarget::Fill, PaintTarget::Stroke] {
            let ctx = egui::Context::default();
            varos_app::shell::fonts::install(&ctx);
            let ed = selected(true);
            let snap = Snap::read(&ed);
            let mut time = 0.0;
            let mut swatch = egui::Rect::NOTHING;
            let mut frame = |events| {
                time += 0.05;
                let mut ops = vec![];
                let _ = ctx.run_ui(RawInput { time: Some(time), events, ..Default::default() }, |ui| {
                    let start = ui.cursor().min;
                    match home {
                        0 => {
                            fill_stroke_control(ui, &snap, &mut ops);
                            let (offset, size) = match target {
                                PaintTarget::Fill => (egui::vec2(0.0, 3.0), egui::vec2(20.0, 20.0)),
                                PaintTarget::Stroke => (egui::vec2(20.0, 25.0), egui::vec2(10.0, 10.0)),
                            };
                            swatch = egui::Rect::from_min_size(start + offset, size);
                        }
                        1 => {
                            paint_row(
                                ui,
                                target,
                                snap_target_color(&snap, target),
                                snap.target_mixed(target),
                                &mut ops,
                            );
                            swatch = paint_probes::swatches().last().unwrap().1;
                        }
                        _ => {
                            ctl_chip(ui, snap_target_color(&snap, target), target, snap.target_mixed(target), &mut ops);
                            swatch = egui::Rect::from_min_size(start, egui::vec2(17.0, 17.0));
                        }
                    }
                });
                (ops, swatch.center())
            };
            let (_, at) = frame(vec![]);
            frame(vec![]);
            for click in 0..2 {
                frame(pointer(at, true));
                let (ops, _) = frame(pointer(at, false));
                assert!(
                    ops.iter().any(|o| matches!(o, Op::OpenPicker(MTarget::Paint(t)) if *t == target)) == (click == 1),
                    "home {home}, click {click}"
                );
            }
        }
    }
}

#[test]
fn press_and_release_in_one_frame_samples_once() {
    let mut r = Rig::new(selected(false));
    r.panel.as_mut().unwrap().arm();
    let p = egui::pos2(80.0, 80.0);
    let mut tap = pointer(p, true);
    tap.extend(pointer(p, false));
    r.frame(tap, Some([0.0, 1.0, 0.0, 1.0]));
    assert_eq!(r.ed.rev, 1);
    assert!(!r.ed.transaction_open());
    assert!(!r.panel.as_ref().unwrap().eyedropping);
}
#[test]
fn alpha_drag_is_one_step_and_percent_field_is_k3() {
    let mut r = Rig::new(selected(false));
    let at = super::super::fields::tests::probed_rect("picker alpha slider", 0).center();
    r.frame(pointer(at, true), None);
    r.frame(vec![Event::PointerMoved(at - egui::vec2(24.0, 0.0))], None);
    r.frame(pointer(at - egui::vec2(24.0, 0.0), false), None);
    assert_eq!(r.ed.rev, 1);
    let at = super::super::fields::tests::probed_rect("picker alpha", 0).center();
    r.frame(pointer(at, true), None);
    r.frame(pointer(at, false), None);
    r.frame(
        vec![
            Event::Key { key: Key::A, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND },
            Event::Text("50".into()),
        ],
        None,
    );
    assert_eq!(r.ed.rev, 1);
    r.frame(vec![key(Key::Enter)], None);
    assert_eq!(r.ed.rev, 2);
    assert_eq!(r.ed.doc.paths[0].fill.solid().unwrap()[3], 0.5);
}
#[test]
fn header_drag_moves_only_the_panel_and_persists_relative_position() {
    let mut r = Rig::new(selected(false));
    let before = r.ed.doc.clone();
    let at = egui::pos2(460.0, 100.0);
    r.frame(pointer(at, true), None);
    r.frame(vec![Event::PointerMoved(at + egui::vec2(35.0, 20.0))], None);
    r.frame(pointer(at + egui::vec2(35.0, 20.0), false), None);
    assert_eq!(r.layout.position, Some([335.0, 104.0]));
    assert_eq!(r.ed.doc, before);
    assert_eq!(r.ed.rev, 0);
}

/// CPU-only geometry gate. Optional mesh/texture export supports visual QA without a window/GPU.
#[test]
fn wheel_geometry_at_1x_and_2x() {
    for ppp in [1.0, 2.0] {
        super::super::fields::tests::clear_probes();
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        t::apply(&ctx);
        let mut ed = selected(false);
        let c = parse_hex("F2C94C").unwrap();
        ed.doc.paths[0].fill = varos_core::model::Paint::Solid(c);
        ed.recent_colors = vec![c, parse_hex("E66C4F").unwrap(), parse_hex("12263A").unwrap()];
        let s = Snap::read(&ed);
        let mut panel = Some(ColorPanel::new(MTarget::Paint(PaintTarget::Fill), Some(c), false));
        let mut layout = PickerLayout { open: true, position: Some([0.0, 0.0]), ..Default::default() };
        let mut textures = std::collections::HashMap::new();
        let mut output = None;
        for _ in 0..3 {
            let mut input = RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(500.0, 500.0))),
                ..Default::default()
            };
            input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().native_pixels_per_point = Some(ppp);
            let out = ctx.run_ui(input, |ui| {
                build_color_panel(ui.ctx(), &mut panel, &s, &mut vec![], None, ui.max_rect(), &mut layout)
            });
            for (id, delta) in &out.textures_delta.set {
                let egui::ImageData::Color(image) = &delta.image;
                if let Some([x, y]) = delta.pos {
                    let old: &mut egui::ColorImage = textures.get_mut(id).unwrap();
                    for row in 0..image.size[1] {
                        for col in 0..image.size[0] {
                            old.pixels[(y + row) * old.size[0] + x + col] = image.pixels[row * image.size[0] + col];
                        }
                    }
                } else {
                    textures.insert(*id, (**image).clone());
                }
            }
            output = Some(out);
        }
        let rect = super::super::fields::tests::probed_rect("picker panel", 0);
        assert_eq!(rect.width(), t::PICKER_W);
        assert_eq!(rect.height(), t::PICKER_H);
        if ppp == 2.0 {
            if let Ok(path) = std::env::var("VAROS_PICKER_MESH_EXPORT") {
                let out = output.unwrap();
                let jobs = ctx.tessellate(out.shapes, out.pixels_per_point);
                let mut meshes = Vec::new();
                for job in jobs {
                    let egui::epaint::Primitive::Mesh(mesh) = job.primitive else { continue };
                    let vertices: Vec<_> = mesh.vertices.iter().map(|v| {
                        serde_json::json!({"p": [v.pos.x, v.pos.y], "uv": [v.uv.x, v.uv.y], "c": v.color.to_array()})
                    }).collect();
                    let clip = [job.clip_rect.min.x, job.clip_rect.min.y, job.clip_rect.max.x, job.clip_rect.max.y];
                    let texture = format!("{:?}", mesh.texture_id);
                    let indices = mesh.indices;
                    meshes.push(
                        serde_json::json!({"clip": clip, "texture": texture, "indices": indices, "vertices": vertices}),
                    );
                }
                let textures: Vec<_> = textures
                    .into_iter()
                    .map(|(id, im)| {
                        let pixels: Vec<_> = im.pixels.iter().map(|c| c.to_array()).collect();
                        serde_json::json!({"id": format!("{id:?}"), "size": im.size, "pixels": pixels})
                    })
                    .collect();
                let rect = [rect.min.x, rect.min.y, rect.max.x, rect.max.y];
                let export = serde_json::json!({"rect": rect, "ppp": ppp, "meshes": meshes, "textures": textures});
                std::fs::write(path, serde_json::to_vec(&export).unwrap()).unwrap();
            }
        }
    }
}

#[test]
fn changing_target_during_sampling_reverts_the_previous_gesture() {
    let mut r = Rig::new(selected(false));
    let before = r.ed.doc.clone();
    r.panel.as_mut().unwrap().arm();
    r.frame(vec![], Some([0.0, 1.0, 0.0, 1.0]));
    assert!(r.ed.transaction_open());
    let cfg = r.ed.doc.snap;
    apply_picker_frame(&mut r.ed, cfg, vec![Op::PaintFocus(PaintTarget::Stroke)], &mut r.panel);
    assert_eq!(r.ed.rev, 0);
    assert!(!r.ed.transaction_open());
    assert!(!r.panel.as_ref().unwrap().eyedropping);
    assert!(r.ed.recent_colors.is_empty());
    assert_eq!(r.ed.doc, before);
}
#[test]
fn grey_field_edits_keep_the_hue_instead_of_snapping_to_red() {
    let mut r = Rig::new(selected(false));
    r.frame(pointer(r.ring(0.7), true), None);
    r.frame(pointer(r.ring(0.7), false), None);
    let hue = r.panel.as_ref().unwrap().hsva[0];
    r.edit_hex("808080");
    r.frame(vec![key(Key::Enter)], None);
    r.frame(vec![], None);
    assert!((r.panel.as_ref().unwrap().hsva[0] - hue).abs() < 1e-6);
}

#[test]
fn mixed_empty_field_enter_is_unchanged_and_escape_closes_without_a_write() {
    let mut r = Rig::new(selected(true));
    let before = r.ed.doc.clone();
    let at = super::super::fields::tests::probed_rect("picker hex", 0).center();
    r.frame(pointer(at, true), None);
    r.frame(pointer(at, false), None);
    r.frame(vec![key(Key::Enter)], None);
    assert!(r.panel.as_ref().unwrap().mixed);
    assert_eq!(r.ed.doc, before);
    assert_eq!(r.ed.rev, 0);
    r.frame(
        vec![Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        None,
    );
    r.frame(vec![key(Key::Escape)], None);
    assert!(r.panel.is_none());
    assert_eq!(r.ed.doc, before);
}
#[test]
fn drawer_disclosure_persists_and_soon_and_disabled_tabs_do_not_write() {
    let mut r = Rig::new(selected(false));
    let before = r.ed.doc.clone();
    let at = egui::pos2(524.0, 411.0);
    r.frame(pointer(at, true), None);
    r.frame(pointer(at, false), None);
    assert!(r.layout.drawer_open);
    for (x, tab) in [(359.0, Tab::Sliders), (389.0, Tab::Harmony)] {
        let at = egui::pos2(x, 100.0);
        r.frame(pointer(at, true), None);
        r.frame(pointer(at, false), None);
        assert!(r.panel.as_ref().unwrap().tab == tab);
    }
    let at = egui::pos2(419.0, 100.0);
    r.frame(pointer(at, true), None);
    r.frame(pointer(at, false), None);
    assert!(r.panel.as_ref().unwrap().tab == Tab::Harmony);
    assert_eq!(r.ed.doc, before);
    assert_eq!(r.ed.rev, 0);
}

#[test]
fn lifecycle_field_commit_cancels_sample_and_keeps_only_field_step() {
    let mut r = Rig::new(selected(false));
    let before = r.ed.doc.clone();
    r.panel.as_mut().unwrap().arm();
    r.frame(vec![], Some([0.0, 1.0, 0.0, 1.0]));
    r.edit_hex("0000FF");
    assert!(super::super::layout::settle_picker_fields(&r.ctx, None, &mut r.pending, &mut r.panel, &mut r.ed));
    assert_eq!(r.ed.rev, 1);
    assert_eq!(r.ed.doc.paths[0].fill.solid(), Some([0.0, 0.0, 1.0, 1.0]));
    assert!(!r.ed.transaction_open());
    r.ed.undo();
    assert_eq!(r.ed.doc, before);
}

#[test]
fn invalid_field_blocks_colour_gestures_without_leaving_a_transaction() {
    let mut r = Rig::new(selected(false));
    let before = r.ed.doc.clone();
    r.edit_hex("invalid");
    r.frame(vec![key(Key::Enter)], None);
    r.frame(pointer(r.ring(0.4), true), None);
    r.frame(pointer(r.ring(0.4), false), None);
    assert_eq!(r.ed.doc, before);
    assert_eq!(r.ed.rev, 0);
    assert!(!r.ed.transaction_open());
    assert!(!r.panel.as_ref().unwrap().gesture_active());
    r.frame(vec![key(Key::Escape)], None);
    r.frame(vec![], None);
    assert_eq!(r.ed.doc, before);
}

#[test]
fn field_blur_then_alpha_drag_have_separate_steps_and_keep_typed_rgb() {
    let mut r = Rig::new(selected(false));
    let before = r.ed.doc.clone();
    r.edit_hex("0000FF");
    let at = super::super::fields::tests::probed_rect("picker alpha slider", 0).center();
    r.frame(pointer(at, true), None);
    assert_eq!(r.ed.rev, 1);
    r.frame(vec![Event::PointerMoved(at - egui::vec2(10.0, 0.0))], None);
    r.frame(pointer(at - egui::vec2(10.0, 0.0), false), None);
    assert_eq!(r.ed.rev, 2);
    let c = r.ed.doc.paths[0].fill.solid().unwrap();
    assert_eq!(&c[..3], &[0.0, 0.0, 1.0]);
    assert!(c[3] < 1.0);
    r.ed.undo();
    assert_eq!(r.ed.doc.paths[0].fill.solid(), Some([0.0, 0.0, 1.0, 1.0]));
    r.ed.undo();
    assert_eq!(r.ed.doc, before);
}

#[test]
fn armed_escape_live_reverts_pre_arm_paints_and_keeps_picker_open() {
    let mut r = Rig::new(selected(false));
    let before = r.ed.doc.clone();
    let defaults = (r.ed.cur_fill, r.ed.cur_stroke);
    r.panel.as_mut().unwrap().arm();
    r.frame(vec![], Some([0., 1., 0., 1.]));
    r.frame(vec![Event::PointerMoved(egui::pos2(310., 100.)), key(Key::Escape)], None);
    assert!(r.panel.is_some());
    assert!(!r.panel.as_ref().unwrap().eyedropping);
    assert_eq!(r.ed.doc, before);
    assert_eq!(r.ed.rev, 0);
    assert_eq!((r.ed.cur_fill, r.ed.cur_stroke), defaults);
    assert!(r.ed.recent_colors.is_empty());
    r.frame(
        vec![Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        None,
    );
    r.frame(vec![key(Key::Escape)], None);
    assert!(r.panel.is_none());
}
#[test]
fn canvas_escape_deselects_while_panel_escape_closes_and_popover_escape_only_closes_popover() {
    let mut r = Rig::new(selected(false));
    r.frame(vec![Event::PointerMoved(egui::pos2(80., 80.)), key(Key::Escape)], None);
    // Native key router sends this to the canvas because picker_owns_escape is false.
    assert!(!super::super::layout::picker_owns_escape(&r.ctx, true));
    r.ed.escape();
    assert!(r.ed.objsel.is_empty());
    assert!(r.panel.is_some());
    let menu = egui::Id::new("escape-test-menu");
    kit::open_menu(&r.ctx, menu, egui::pos2(320., 140.), None);
    r.frame(vec![Event::PointerMoved(egui::pos2(310., 100.)), key(Key::Escape)], None);
    assert!(r.panel.is_some());
    let _ = r.ctx.run_ui(RawInput { events: vec![key(Key::Escape)], ..Default::default() }, |ui| {
        kit::menu(ui.ctx(), menu, &[kit::MenuEntry::Item("Test")]);
    });
    assert!(!kit::menu_open(&r.ctx));
    assert!(r.panel.is_some());
    r.frame(vec![key(Key::Escape)], None);
    assert!(r.panel.is_none());
}
#[test]
fn one_frame_taps_ring_triangle_and_alpha_are_single_steps() {
    for kind in 0..3 {
        let mut r = Rig::new(selected(false));
        let at = match kind {
            0 => r.ring(0.25),
            1 => egui::pos2(423., 234.) + wheel::sv_pos(0., t::PICKER_TRIANGLE_R, 0.5, 0.5).to_vec2(),
            _ => super::super::fields::tests::probed_rect("picker alpha slider", 0).center(),
        };
        let mut events = pointer(at, true);
        events.extend(pointer(at, false));
        r.frame(events, None);
        assert_eq!(r.ed.rev, 1, "kind {kind}");
        assert!(!r.ed.transaction_open());
    }
}
#[test]
fn none_hue_drag_seeds_full_saturation_and_brightness() {
    let mut ed = selected(false);
    ed.doc.paths[0].fill = varos_core::model::Paint::None;
    let mut r = Rig::new(ed);
    let at = r.ring(0.25);
    r.frame(pointer(at, true), None);
    r.frame(pointer(at, false), None);
    let c = r.ed.doc.paths[0].fill.solid().unwrap();
    let hsv = rgb_to_hsv(c);
    assert!((hsv[0] - 0.25).abs() < 1e-5);
    assert_eq!([hsv[1], hsv[2]], [1., 1.]);
    assert_eq!(r.ed.rev, 1);
}
#[test]
fn panel_press_cancels_sample_before_ring_or_alpha_transaction() {
    for alpha in [false, true] {
        let mut r = Rig::new(selected(false));
        let before = r.ed.doc.clone();
        r.panel.as_mut().unwrap().arm();
        r.frame(vec![], Some([0., 1., 0., 1.]));
        let at = if alpha {
            super::super::fields::tests::probed_rect("picker alpha slider", 0).center()
        } else {
            r.ring(0.25)
        };
        r.frame(pointer(at, true), None);
        r.frame(pointer(at, false), None);
        assert_eq!(r.ed.rev, 1);
        assert!(!r.ed.transaction_open());
        r.ed.undo();
        assert_eq!(r.ed.doc, before);
    }
}
#[test]
fn independent_op_with_picker_begin_does_not_orphan_transaction() {
    let mut r = Rig::new(selected(false));
    let mut ops = vec![Op::Tool(ToolKind::Object)];
    let m = r.panel.as_mut().unwrap();
    m.start(Gesture::Ring, &mut ops);
    m.hsva[0] = 0.25;
    m.change_requested = true;
    let cfg = r.ed.doc.snap;
    apply_picker_frame(&mut r.ed, cfg, ops, &mut r.panel);
    assert!(r.panel.as_ref().unwrap().gesture_active());
    assert!(r.ed.transaction_open());
    let mut ops = vec![];
    r.panel.as_mut().unwrap().finish(&mut ops);
    apply_ops(&mut r.ed, ops);
    assert!(!r.ed.transaction_open());
    assert_eq!(r.ed.rev, 1);
}
#[test]
fn artboard_target_survives_reorder_and_missing_id_cancels_without_step() {
    let mut r = Rig::new(selected(false));
    r.ed.doc.artboards = vec![
        varos_core::model::Artboard { id: 30, ..Default::default() },
        varos_core::model::Artboard { id: 40, ..Default::default() },
    ];
    open_picker(&mut r.panel, MTarget::Ab(40), &mut r.ed);
    r.ed.doc.artboards.swap(0, 1);
    r.frame(vec![], None);
    assert!(r.panel.as_ref().unwrap().target == MTarget::Ab(40));
    apply_ops(&mut r.ed, vec![Op::PickerSet(MTarget::Ab(40), [1., 0., 0., 1.])]);
    assert_eq!(r.ed.doc.artboards[0].page_color, Some([1., 0., 0., 1.]));
    let mut ops = vec![];
    r.panel.as_mut().unwrap().start(Gesture::Alpha, &mut ops);
    ops.push(Op::PickerLive(MTarget::Ab(40), [0., 1., 0., 0.5]));
    apply_ops(&mut r.ed, ops);
    assert!(r.ed.transaction_open());
    r.ed.doc.artboards.remove(0);
    let rev = r.ed.rev;
    r.frame(vec![], None);
    assert!(matches!(r.panel.as_ref().unwrap().target, MTarget::Paint(_)));
    assert_eq!(r.ed.rev, rev);
    assert!(!r.ed.transaction_open());
    assert_eq!(r.ed.doc.artboards.iter().map(|a| a.id).collect::<Vec<_>>(), vec![30]);
}
#[test]
fn cluster_square_corner_hits_visible_stroke_ring() {
    let mut r = Rig::new(selected(false));
    // Inside Fill's square, outside its circle, and on the visible Stroke ring.
    let at = egui::pos2(499., 321.);
    r.frame(pointer(at, true), None);
    r.frame(pointer(at, false), None);
    assert!(r.panel.as_ref().unwrap().target == MTarget::Paint(PaintTarget::Stroke));
    assert_eq!(r.ed.rev, 0);
}
#[test]
fn sliders_drag_tap_mixed_unchanged_and_value_arrows_follow_k3() {
    for mode in modes::MODES {
        let mut r = Rig::new(selected(true));
        r.layout.mode = mode;
        r.panel.as_mut().unwrap().tab = Tab::Sliders;
        r.frame(vec![], None);
        r.frame(vec![], None);
        assert_eq!(r.ed.rev, 0);
        let track = super::super::fields::tests::probed_rect("picker channel 0", 0);
        let at = track.center();
        let mut tap = pointer(at, true);
        tap.extend(pointer(at, false));
        r.frame(tap, None);
        assert_eq!(r.ed.rev, 1, "{mode:?}");
        assert!(!r.ed.transaction_open());
        r.frame(pointer(at, true), None);
        r.frame(pointer(at, false), None);
        assert_eq!(r.ed.rev, 1, "unchanged {mode:?}");
        let at = egui::pos2(track.right() + t::PICKER_PAD + t::PICKER_PERCENT_W / 2., track.center().y);
        r.frame(pointer(at, true), None);
        r.frame(pointer(at, false), None);
        let rev = r.ed.rev;
        r.frame(vec![key(Key::ArrowUp)], None);
        assert_eq!(r.ed.rev, rev + 1, "arrow {mode:?}");
        let stepped = r.panel.as_ref().unwrap().channel_values()[0];
        r.frame(
            vec![Event::Key {
                key: Key::ArrowDown,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            }],
            None,
        );
        assert_eq!(r.ed.rev, rev + 2, "shift arrow {mode:?}");
        let step = if mode == modes::Mode::Web { 51.0 } else { 10.0 };
        assert!((r.panel.as_ref().unwrap().channel_values()[0] - (stepped - step)).abs() < 1e-4);
    }
}
#[test]
fn eyedropper_button_disarms_without_rearming_and_selection_change_reverts() {
    let mut r = Rig::new(selected(false));
    let before = r.ed.doc.clone();
    r.panel.as_mut().unwrap().arm();
    r.frame(vec![], Some([0., 1., 0., 1.]));
    let eye = egui::pos2(493., 100.);
    r.frame(pointer(eye, true), None);
    r.frame(pointer(eye, false), None);
    assert!(!r.panel.as_ref().unwrap().eyedropping);
    assert_eq!(r.ed.doc, before);
    assert_eq!(r.ed.rev, 0);
    r.panel.as_mut().unwrap().arm();
    r.frame(vec![], Some([0., 1., 0., 1.]));
    r.ed.objsel.clear();
    r.ed.objsel.insert(2);
    r.frame(vec![], None);
    assert_eq!(r.ed.doc, before);
    assert_eq!(r.ed.rev, 0);
    assert!(r.ed.recent_colors.is_empty());
}
#[test]
fn drawer_real_height_uses_token_and_slider_mode_defaults_and_persists() {
    let mut r = Rig::new(selected(false));
    r.layout.drawer_open = true;
    r.frame(vec![], None);
    r.frame(vec![], None);
    let rect = r.ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new("picker-panel-rect"))).unwrap();
    assert_eq!(rect.height(), t::PICKER_H + t::PICKER_DRAWER_H);
    let actual = super::super::fields::tests::probed_rect("picker panel", 2);
    assert_eq!(actual.height(), t::PICKER_H + t::PICKER_DRAWER_H);
    let old: PickerLayout = serde_json::from_str("{}").unwrap();
    assert_eq!(old.mode, modes::Mode::Hsb);
    for mode in modes::MODES {
        r.layout.mode = mode;
        assert_eq!(
            serde_json::from_str::<PickerLayout>(&serde_json::to_string(&r.layout).unwrap()).unwrap().mode,
            mode
        );
    }
}
#[test]
fn slider_value_enter_blur_invalid_and_mode_menu_are_k3_and_read_only() {
    let mut r = Rig::new(selected(false));
    r.panel.as_mut().unwrap().tab = Tab::Sliders;
    r.layout.mode = modes::Mode::Rgb;
    r.frame(vec![], None);
    r.frame(vec![], None);
    let track = super::super::fields::tests::probed_rect("picker channel 0", 0);
    let at = egui::pos2(track.right() + t::PICKER_PAD + t::PICKER_PERCENT_W / 2., track.center().y);
    let edit = |r: &mut Rig, text: &str| {
        r.frame(pointer(at, true), None);
        r.frame(pointer(at, false), None);
        r.frame(
            vec![
                Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                },
                Event::Text(text.into()),
            ],
            None,
        );
    };
    edit(&mut r, "100");
    r.frame(vec![key(Key::Enter)], None);
    assert_eq!(r.ed.rev, 1);
    edit(&mut r, "120");
    r.frame(pointer(egui::pos2(80., 80.), true), None);
    r.frame(pointer(egui::pos2(80., 80.), false), None);
    assert_eq!(r.ed.rev, 2);
    edit(&mut r, "invalid");
    let before = r.ed.doc.clone();
    let state = r.panel.as_ref().unwrap().color();
    r.frame(vec![key(Key::Enter)], None);
    r.frame(pointer(track.center(), true), None);
    r.frame(pointer(track.center(), false), None);
    assert_eq!(r.ed.doc, before);
    assert_eq!(r.panel.as_ref().unwrap().color(), state);
    assert!(kit::field::blocked(&r.ctx));
    r.frame(vec![key(Key::Escape)], None);
    assert!(r.panel.is_some());
    let mode = egui::pos2(350., 135.);
    r.frame(pointer(mode, true), None);
    r.frame(pointer(mode, false), None);
    assert!(kit::menu_open(&r.ctx));
    r.frame(vec![key(Key::Escape)], None);
    assert!(!kit::menu_open(&r.ctx));
    assert!(r.panel.is_some());
    assert_eq!(r.ed.rev, 2);
}

#[test]
fn slider_drag_is_one_step_and_wheel_readout_follows_every_mode() {
    let mut r = Rig::new(selected(false));
    let before = r.ed.doc.clone();
    r.layout.mode = modes::Mode::Rgb;
    r.panel.as_mut().unwrap().tab = Tab::Sliders;
    r.frame(vec![], None);
    r.frame(vec![], None);
    let track = super::super::fields::tests::probed_rect("picker channel 1", 0);
    let at = |f| egui::pos2(track.left() + track.width() * f, track.center().y);
    r.frame(pointer(at(0.2), true), None);
    for f in [0.4, 0.6, 0.8] {
        r.frame(vec![Event::PointerMoved(at(f))], None);
        assert_eq!(r.ed.rev, 0);
    }
    r.frame(pointer(at(0.8), false), None);
    assert_eq!(r.ed.rev, 1);
    assert!(!r.ed.transaction_open());
    r.ed.undo();
    assert_eq!(r.ed.doc, before);
    for (mode, expected) in modes::MODES.into_iter().zip([
        vec!["H", "S", "B"],
        vec!["H", "S", "L"],
        vec!["R", "G", "B"],
        vec!["C", "M", "Y", "K"],
        vec!["L", "a", "b"],
        vec![""],
    ]) {
        r.layout.mode = mode;
        r.panel.as_mut().unwrap().tab = Tab::Wheel;
        r.frame(vec![], None);
        let m = r.panel.as_ref().unwrap();
        let readout = modes::readout(m.mode, m.color(), m.channel_values());
        assert_eq!(readout.iter().map(|(label, _)| *label).collect::<Vec<_>>(), expected);
        if mode == modes::Mode::Web {
            assert_eq!(readout[0].1, hex_of(m.color()));
        }
    }
}
#[test]
fn sample_click_outside_canvas_never_accepts() {
    let mut r = Rig::new(selected(false));
    let before = r.ed.doc.clone();
    r.panel.as_mut().unwrap().arm();
    r.frame(vec![], Some([0., 1., 0., 1.]));
    let p = egui::pos2(1300., 950.);
    let mut tap = pointer(p, true);
    tap.extend(pointer(p, false));
    r.frame(tap, Some([0., 1., 0., 1.]));
    assert_eq!(r.ed.rev, 0);
    assert!(r.panel.as_ref().unwrap().eyedropping);
    let mut ops = vec![];
    r.panel.as_mut().unwrap().finish(&mut ops);
    apply_ops(&mut r.ed, ops);
    assert_eq!(r.ed.doc, before);
    assert!(r.ed.recent_colors.is_empty());
}

#[test]
fn invalid_hex_blocks_pipette_and_sampling_and_unowned_live_is_inert() {
    let mut ed = selected(false);
    ed.doc.artboards.push(varos_core::model::Artboard { id: 40, ..Default::default() });
    let mut r = Rig::new(ed);
    let before = r.ed.doc.clone();
    let defaults = (r.ed.cur_fill, r.ed.cur_stroke);
    r.edit_hex("invalid");
    let eye = egui::pos2(493., 100.);
    r.frame(pointer(eye, true), None);
    r.frame(pointer(eye, false), None);
    assert!(kit::field::blocked(&r.ctx));
    assert!(!r.panel.as_ref().unwrap().eyedropping);
    r.panel.as_mut().unwrap().arm();
    r.frame(vec![], Some([0., 1., 0., 1.]));
    assert!(!r.ed.transaction_open());
    for target in [MTarget::Paint(PaintTarget::Fill), MTarget::Ab(r.ed.doc.artboards[0].id)] {
        apply_ops(&mut r.ed, vec![Op::PickerLive(target, [0., 1., 0., 1.])]);
    }
    assert_eq!(r.ed.doc, before);
    assert_eq!((r.ed.cur_fill, r.ed.cur_stroke), defaults);
    assert_eq!(r.ed.rev, 0);
}
#[test]
fn channel_only_edits_preserve_hue_and_degenerate_saturation_without_writes() {
    for (mode, c) in [
        (modes::Mode::Hsb, [0.5, 0.5, 0.5, 1.]),
        (modes::Mode::Hsb, [0., 0., 0., 1.]),
        (modes::Mode::Hsl, [0., 0., 0., 1.]),
        (modes::Mode::Hsl, [1., 1., 1., 1.]),
    ] {
        let mut m = ColorPanel::new(MTarget::Paint(PaintTarget::Fill), Some(c), false);
        m.mode = mode;
        m.adopt_channel(0, 240.);
        assert_eq!(m.channel_values()[0], 240.);
        assert!((m.hsva[0] - 2. / 3.).abs() < 1e-6);
        assert!(m.live_color().is_none());
        if c[0] != 0.5 {
            m.adopt_channel(1, 75.);
            assert_eq!(m.channel_values()[1], 75.);
            assert!(m.live_color().is_none());
            m.adopt_channel(2, 50.);
            assert_eq!(m.channel_values()[1], 75.);
            assert!(m.live_color().is_some());
        }
    }
}
#[test]
fn hsb_sliders_and_wheel_share_black_saturation_after_mode_switch() {
    let mut m = ColorPanel::new(MTarget::Paint(PaintTarget::Fill), Some([0., 0., 0., 1.]), false);
    m.hsva = [0.4, 0.75, 0., 1.];
    m.mode = modes::Mode::Rgb;
    let _ = m.channel_values();
    m.mode = modes::Mode::Hsb;
    assert_eq!(m.channel_values()[1], 75.);
    m.adopt_channel(2, 50.);
    assert_eq!(m.hsva[1], 0.75);
    assert_eq!(m.channel_values()[1], 75.);
}
#[test]
fn canvas_escape_while_sampling_preserves_selection_and_panel() {
    let mut r = Rig::new(selected(false));
    let selection = r.ed.selected_pids();
    let before = r.ed.doc.clone();
    r.panel.as_mut().unwrap().arm();
    r.frame(vec![], Some([0., 1., 0., 1.]));
    r.frame(vec![Event::PointerMoved(egui::pos2(80., 80.)), key(Key::Escape)], None);
    assert!(!r.panel.as_ref().unwrap().eyedropping);
    assert!(!r.ed.transaction_open());
    assert_eq!(r.ed.selected_pids(), selection);
    assert_eq!(r.ed.doc, before);
    assert_eq!(r.ed.rev, 0);
    assert!(r.layout.open);
}

#[test]
fn typed_grey_hue_survives_enter_without_document_change() {
    let mut ed = selected(false);
    ed.doc.paths[0].fill = varos_core::model::Paint::Solid([0.5, 0.5, 0.5, 1.]);
    let mut r = Rig::new(ed);
    let before = r.ed.doc.clone();
    r.panel.as_mut().unwrap().tab = Tab::Sliders;
    r.frame(vec![], None);
    r.frame(vec![], None);
    let track = super::super::fields::tests::probed_rect("picker channel 0", 0);
    let at = egui::pos2(track.right() + t::PICKER_PAD + t::PICKER_PERCENT_W / 2., track.center().y);
    r.frame(pointer(at, true), None);
    r.frame(pointer(at, false), None);
    r.frame(
        vec![
            Event::Key { key: Key::A, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND },
            Event::Text("240".into()),
        ],
        None,
    );
    r.frame(vec![key(Key::Enter)], None);
    r.frame(vec![], None);
    assert_eq!(r.panel.as_ref().unwrap().channel_values()[0], 240.);
    assert_eq!(r.ed.doc, before);
    assert_eq!(r.ed.rev, 0);
}
#[test]
fn web_wheel_paints_hex_without_empty_label_punctuation() {
    let ctx = egui::Context::default();
    varos_app::shell::fonts::install(&ctx);
    t::apply(&ctx);
    let ed = selected(false);
    let snap = Snap::read(&ed);
    let mut m = ColorPanel::new(MTarget::Paint(PaintTarget::Fill), Some([1., 0., 0., 1.]), false);
    m.mode = modes::Mode::Web;
    let _ = ctx.run_ui(RawInput::default(), |ui| wheel::show(ui, &mut m, &snap, &mut vec![]));
    let output = ctx.run_ui(RawInput::default(), |ui| wheel::show(ui, &mut m, &snap, &mut vec![]));
    let text: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some(t.galley.job.text.as_str()),
            _ => None,
        })
        .collect();
    assert!(text.contains(&hex_of(m.color()).as_str()), "{text:?}");
    assert!(!text.contains(&":"));
}

#[test]
fn grey_slider_state_is_invalidated_by_real_wheel_hue_rotation() {
    let mut ed = selected(false);
    ed.doc.paths[0].fill = varos_core::model::Paint::Solid([0.5, 0.5, 0.5, 1.]);
    let mut r = Rig::new(ed);
    r.panel.as_mut().unwrap().adopt_channel(0, 240.);
    let at = r.ring(0.5);
    r.frame(pointer(at, true), None);
    r.frame(pointer(at, false), None);
    r.panel.as_mut().unwrap().tab = Tab::Sliders;
    r.frame(vec![], None);
    let m = r.panel.as_ref().unwrap();
    assert!((m.channel_values()[0] - 180.).abs() < 1e-4);
    assert_eq!(r.ed.rev, 0);
}

#[test]
fn harmony_recovered_angles_wrap_and_original_brightness_are_preserved() {
    use varos_app::storage::layout::HarmonyRule as R;
    let base = [350. / 360., 0.65, 0.8];
    for (rule, angles) in [
        (R::Complementary, vec![180.]),
        (R::Analogous, vec![-30., 30.]),
        (R::Split, vec![150., 210.]),
        (R::Triadic, vec![120., 240.]),
        (R::Tetradic, vec![60., 180., 240.]),
        (R::Square, vec![90., 180., 270.]),
    ] {
        let set = harmony_rules::linked(rule, base);
        assert_eq!(set[0], base);
        assert_eq!(set.len(), angles.len() + 1);
        for (c, angle) in set.iter().skip(1).zip(angles) {
            assert!((c[0] - ((350.0_f32 + angle) / 360.).rem_euclid(1.)).abs() < 1e-6);
            assert_eq!([c[1], c[2]], [base[1], base[2]]);
        }
        assert_eq!(harmony_rules::swatches(rule, base).len(), 6);
    }
    for rule in [R::Mono, R::Shades] {
        for (c, k) in harmony_rules::linked(rule, base).iter().zip([1., 0.78, 0.56, 0.36]) {
            assert_eq!([c[0], c[1]], [base[0], base[1]]);
            assert!((c[2] - base[2] * k).abs() < 1e-6);
        }
        assert_eq!(harmony_rules::linked(rule, [1., 0.5, 0.])[3], [1., 0.5, 0.06]);
        assert_eq!(harmony_rules::swatches(rule, base).len(), 6);
    }
    assert_eq!(harmony_rules::linked(R::Complementary, [0.5, 0.7, 0.9])[1], [0., 0.7, 0.9]);
}
#[test]
fn harmony_click_drag_persistence_and_idle_are_change_only() {
    use varos_app::storage::layout::HarmonyRule as R;
    let mut r = Rig::new(selected(false));
    super::super::fields::tests::clear_probes();
    r.panel.as_mut().unwrap().tab = Tab::Harmony;
    r.frame(vec![], None);
    r.frame(vec![], None);
    let at = super::super::fields::tests::probed_rect("harmony swatch 1", 0).center();
    r.frame(pointer(at, true), None);
    r.frame(pointer(at, false), None);
    assert_eq!(r.ed.rev, 1);
    assert!(!r.ed.transaction_open());
    r.ed.undo();
    r.frame(vec![], None);
    let at = super::super::fields::tests::probed_rect("harmony swatch 0", 0).center();
    let rev = r.ed.rev;
    r.frame(pointer(at, true), None);
    r.frame(pointer(at, false), None);
    assert_eq!(r.ed.rev, rev);
    let wheel = super::super::fields::tests::probed_rect("picker wheel", 0);
    let center = wheel.min + egui::vec2(t::PICKER_RING_CENTER[0], t::PICKER_RING_CENTER[1]);
    let point = |h| wheel::ring_pos(center, t::PICKER_RING_R - t::PICKER_RING_BAND / 2., h);
    r.frame(pointer(point(0.25), true), None);
    r.frame(vec![Event::PointerMoved(point(0.5))], None);
    assert_eq!(r.ed.rev, rev);
    r.frame(pointer(point(0.5), false), None);
    assert_eq!(r.ed.rev, rev + 1);
    // Select a rule through the actual glyph hit, and persist it additively.
    let rule = egui::pos2(
        300. + t::PICKER_PAD + 3. * (t::PICKER_TAB_H + t::PICKER_TAB_GAP) + t::PICKER_TAB_H / 2.,
        84. + t::PICKER_HEADER_H + t::PICKER_PAD + t::PICKER_TAB_H / 2.,
    );
    r.frame(pointer(rule, true), None);
    r.frame(pointer(rule, false), None);
    assert_eq!(r.layout.harmony, R::Triadic);
    assert_eq!(
        serde_json::from_str::<PickerLayout>(&serde_json::to_string(&r.layout).unwrap()).unwrap().harmony,
        R::Triadic
    );
    assert_eq!(serde_json::from_str::<PickerLayout>("{}").unwrap().harmony, R::Complementary);
    r.frame(vec![Event::PointerMoved(egui::pos2(1100., 800.))], None);
    for _ in 0..8 {
        r.frame(vec![], None);
    }
    assert_eq!(r.delay, std::time::Duration::MAX);
}
fn open_mini(r: &mut Rig, id: u32) {
    let anchor = egui::Rect::from_min_size(egui::pos2(500., 80.), egui::Vec2::splat(24.));
    let cfg = r.ed.doc.snap;
    apply_picker_frame(&mut r.ed, cfg, vec![Op::OpenMini(id, anchor)], &mut r.panel);
    r.frame(vec![], None);
    r.frame(vec![], None);
}
#[test]
fn mini_structure_page_drag_escape_outside_and_single_owner() {
    let mut r = Rig::new(selected(false));
    r.ed.doc.artboards = vec![
        varos_core::model::Artboard { id: 30, ..Default::default() },
        varos_core::model::Artboard { id: 40, page_color: Some([1., 0., 0., 1.]), ..Default::default() },
    ];
    // A live big-panel gesture must finish before the popover can own a new one.
    let at = r.ring(0.25);
    r.frame(pointer(at, true), None);
    super::super::fields::tests::clear_probes();
    open_mini(&mut r, 40);
    assert_eq!(r.ed.rev, 1);
    assert!(!r.ed.transaction_open());
    assert!(r.panel.as_ref().unwrap().mini());
    assert!(r.panel.as_ref().unwrap().target == MTarget::Ab(40));
    let rect = super::super::fields::tests::probed_rect("picker mini", 0);
    assert_eq!(rect.width(), t::PICKER_MINI_W);
    assert_eq!(rect.height(), t::PICKER_MINI_BODY_H + t::PICKER_FIELD_ROW_H + t::PICKER_SWATCH_ROW_H);
    assert_eq!(super::super::fields::tests::probe_count("picker cluster"), 0);
    assert_eq!(super::super::fields::tests::probe_count("picker header"), 0);
    assert_eq!(super::super::fields::tests::probe_count("picker wheel"), 2);
    assert_eq!(super::super::fields::tests::probe_count("picker hex"), 2);
    assert_eq!(super::super::fields::tests::probe_count("picker alpha slider"), 2);
    r.ed.doc.artboards.swap(0, 1);
    r.frame(vec![], None);
    let center = rect.min + egui::vec2(t::PICKER_MINI_CENTER[0], t::PICKER_MINI_CENTER[1]);
    let point = |h| wheel::ring_pos(center, t::PICKER_MINI_RING_R - t::PICKER_RING_BAND / 2., h);
    let before = r.ed.doc.clone();
    r.frame(pointer(point(0.25), true), None);
    r.frame(vec![Event::PointerMoved(point(0.5))], None);
    assert_eq!(r.ed.rev, 1);
    r.frame(pointer(point(0.5), false), None);
    assert_eq!(r.ed.rev, 2);
    assert_ne!(r.ed.doc.artboards[0].page_color, before.artboards[0].page_color);
    assert_eq!(r.ed.doc.artboards[1], before.artboards[1]);
    r.ed.undo();
    assert_eq!(r.ed.doc, before);
    // Escape is owned even from the canvas; it cannot deselect the artwork.
    assert!(super::super::layout::picker_owns_escape(&r.ctx, true));
    r.frame(vec![Event::PointerMoved(egui::pos2(80., 80.)), key(Key::Escape)], None);
    assert!(r.panel.is_none());
    open_mini(&mut r, 40);
    r.frame(pointer(egui::pos2(80., 80.), true), None);
    assert!(r.panel.is_none());
}
#[test]
fn drawer_tabs_choose_three_distinct_sources_and_mini_idle_sleeps() {
    let mut r = Rig::new(selected(false));
    r.ed.doc.artboards.push(varos_core::model::Artboard { id: 30, ..Default::default() });
    r.ed.push_recent([0., 1., 0., 1.]);
    let snap = Snap::read(&r.ed);
    assert_eq!(snap.recent[0], [0., 1., 0., 1.]);
    assert_eq!(snap.doc_colors, vec![[1., 0., 0., 1.], [0., 0., 1., 1.]]);
    assert_eq!(drawer::source(&snap, 0), &snap.recent);
    assert_eq!(drawer::source(&snap, 1), &snap.board_colors);
    assert_eq!(drawer::source(&snap, 2), &snap.doc_colors);
    for tab in 0..3 {
        r.layout.drawer_tab = tab;
        r.layout.drawer_open = true;
        r.frame(vec![], None);
        assert_eq!(
            serde_json::from_str::<PickerLayout>(&serde_json::to_string(&r.layout).unwrap()).unwrap().drawer_tab,
            tab
        );
    }
    r.layout.drawer_open = false;
    open_mini(&mut r, 30);
    r.frame(vec![Event::PointerMoved(egui::pos2(1100., 800.))], None);
    for _ in 0..8 {
        r.frame(vec![], None);
    }
    assert_eq!(r.delay, std::time::Duration::MAX);
    assert_eq!(r.ed.rev, 0);
}

#[test]
fn mini_deleted_page_closes_without_retargeting_or_undo() {
    let mut r = Rig::new(selected(false));
    r.ed.doc.artboards.push(varos_core::model::Artboard { id: 40, ..Default::default() });
    open_mini(&mut r, 40);
    let rev = r.ed.rev;
    r.ed.doc.artboards.clear();
    r.frame(vec![], None);
    assert!(r.panel.is_none());
    assert_eq!(r.ed.rev, rev);
    assert!(!r.ed.transaction_open());
}
