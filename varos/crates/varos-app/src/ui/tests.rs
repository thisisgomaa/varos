pub(super) use super::*;

#[cfg(test)]
mod ui_zoom_tests {
    use super::disable_ui_keyboard_zoom;
    use egui::{Event, Key, Modifiers, RawInput};

    /// Press ⌘+key for one pass, then run one more pass (egui applies a new zoom factor between
    /// passes), and return the UI zoom factor afterwards.
    fn ui_zoom_after(ctx: &egui::Context, key: Key) -> f32 {
        let press = RawInput {
            modifiers: Modifiers::COMMAND,
            events: vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            }],
            ..Default::default()
        };
        let _ = ctx.run_ui(press, |_| {});
        let _ = ctx.run_ui(RawInput::default(), |_| {});
        ctx.zoom_factor()
    }

    /// Astra F05: ⌘+ / ⌘= / ⌘− must never scale the panels and text. The control case proves the
    /// test really drives egui's built-in shortcut (it DOES scale a default context).
    #[test]
    fn cmd_plus_minus_never_scale_the_ui() {
        let stock = egui::Context::default();
        assert_ne!(ui_zoom_after(&stock, Key::Plus), 1.0, "control: egui's default scales the UI on ⌘+");
        for key in [Key::Plus, Key::Equals, Key::Minus, Key::Num0] {
            let ctx = egui::Context::default();
            disable_ui_keyboard_zoom(&ctx);
            assert_eq!(ui_zoom_after(&ctx, key), 1.0, "⌘{key:?} changed the UI zoom");
        }
    }
}

#[cfg(test)]
mod polish_pass_tests {
    use super::*;
    use std::path::PathBuf;
    use varos_app::shell::tokens as t;

    fn src(path: &str) -> String {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src").join(path);
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn informational_text_never_faint() {
        let source = [
            "shell/kit/field.rs",
            "ui/controls.rs",
            "ui/control_bar.rs",
            "ui/panels/align.rs",
            "ui/panels/layers.rs",
            "ui/panels/properties.rs",
            "ui/picker.rs",
        ]
        .map(src)
        .join("\n");
        assert!(!source.contains("FAINT"));
    }

    #[test]
    fn number_box_is_44_and_value_is_mono_12() {
        let box_w = 64.0 - t::FIELD_LABEL_W - t::FIELD_LABEL_BOX_GAP;
        assert_eq!(box_w, 44.0);
        assert_eq!(t::NUM_TEXT, 12.0);
        assert_eq!(t::numeric_value(t::NUM_TEXT).family, egui::FontFamily::Monospace);
    }

    #[test]
    fn typing_editor_is_at_least_38_wide() {
        let box_w = 64.0 - t::FIELD_LABEL_W - t::FIELD_LABEL_BOX_GAP;
        assert!(box_w - t::NUM_INSET_X * 2.0 >= 38.0);
    }

    #[test]
    fn rail_icons_rest_muted_hover_text_active_white() {
        assert_eq!(icon_ink(false, false), MUTED);
        assert_eq!(icon_ink(false, true), TEXT);
        assert_eq!(icon_ink(true, false), Color32::WHITE);
    }

    #[test]
    fn micro_label_is_10_5_medium_tracked_muted() {
        assert_eq!((t::T_MICRO, t::MICRO_TRACKING), (10.5, 0.6));
        let source = src("shell/tokens.rs");
        assert!(
            source.contains("UI_500")
                && source.contains("extra_letter_spacing(MICRO_TRACKING)")
                && source.contains(".color(MUTED)")
        );
    }

    #[test]
    fn control_bar_name_slot_is_64_and_elides() {
        assert_eq!(t::CONTROL_BAR_NAME_W, 64.0);
        let source = src("ui/control_bar.rs");
        assert!(
            source.contains("control_bar_name(ui, &ab.name")
                && source.matches("control_bar_name(ui, &s.name").count() == 2
        );
        assert!(source.contains("format!(\"{shown}…\")"));
    }

    #[test]
    fn toggle_off_track_is_line2() {
        assert_eq!(toggle_track(false), LINE2);
        assert_eq!(toggle_knob(false), MUTED);
        assert_eq!((toggle_track(true), toggle_knob(true)), (ACCENT, TEXT));
    }

    #[test]
    fn layer_names_are_all_12() {
        let source = src("ui/panels/layers.rs");
        assert!(source.contains("small_medium()") && source.contains("small()"));
        assert!(source.contains("row.selected || !auto"));
        assert!(
            !source.contains("Color32::from_gray(208)")
                && !source.contains("&row.name,\n                                12.5")
        );
    }

    #[test]
    fn pf_glyph_fits_16() {
        assert_eq!((t::PF_INK, t::PF_SQUARE, t::PF_OFFSET, t::PF_STROKE), (16.0, 10.0, 8.0, 1.5));
        assert_eq!((t::PF_BAR_W, t::PF_BAR_H), (t::ICON_BTN_W, t::ICON_BTN_H));
        assert_eq!(t::PF_OFFSET * 2.0, t::PF_INK);
    }

    #[test]
    fn captionless_swatches_start_at_the_panel_edge() {
        let mut ed = Editor::new();
        ed.ppu = 1.0;
        ed.set_tool(ToolKind::Rect);
        ed.pointer_down([20.0, 20.0]);
        ed.pointer_move([120.0, 120.0]);
        ed.pointer_up();
        ed.set_tool(ToolKind::Object);
        ed.select_all();
        let snap = Snap::read(&ed);
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        t::apply(&ctx);
        fields::tests::clear_probes();
        paint_probes::clear();
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(360.0, 720.0))),
                ..Default::default()
            },
            |ui| {
                let none = None;
                let align = [None, None, None, None, None, None, None, None];
                let icons = DockIcons { rotate: &none, opacity: &none, strokew: &none, align: &align };
                panel_properties(
                    ui,
                    &snap,
                    &icons,
                    &mut (0.0, 0.0),
                    &mut false,
                    &mut vec![],
                    (&Default::default(), &mut vec![]),
                );
            },
        );
        let x_box = fields::tests::probed_rect("X position", 0);
        let fill = paint_probes::swatches()
            .into_iter()
            .find(|(target, _)| *target == PaintTarget::Fill)
            .expect("Fill swatch was laid out")
            .1;
        assert!(fill.left() < x_box.left(), "captionless swatch is at the panel edge");
        assert!(fill.height() >= t::KIT_MIN_TARGET);
    }

    fn legacy_align_height(ui: &mut egui::Ui) -> f32 {
        egui::Frame::NONE
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 5.0);
                let first = ui.label(RichText::new("ALIGN TO").color(MUTED).size(10.0).strong());
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for _ in 0..3 {
                        ui.allocate_exact_size(egui::vec2(60.0, 22.0), egui::Sense::hover());
                    }
                });
                ui.add_space(6.0);
                ui.label(RichText::new("ALIGN OBJECTS").color(MUTED).size(10.0).strong());
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    for _ in 0..6 {
                        ui.allocate_exact_size(egui::vec2(ICON_BTN_W, ICON_BTN_H), egui::Sense::hover());
                    }
                });
                ui.add_space(4.0);
                ui.label(RichText::new("DISTRIBUTE").color(MUTED).size(10.0).strong());
                ui.add_space(2.0);
                let last = ui.horizontal(|ui| {
                    for _ in 0..2 {
                        ui.allocate_exact_size(egui::vec2(ICON_BTN_W, ICON_BTN_H), egui::Sense::hover());
                    }
                });
                last.response.rect.bottom() - first.rect.top()
            })
            .inner
    }

    #[test]
    fn label_gap_and_align_height_are_measured_from_laid_out_rects() {
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        t::apply(&ctx);
        align_probes::clear();
        let mut current = 0.0;
        let mut legacy = 0.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(360.0, 480.0))),
            ..Default::default()
        };
        let _ = ctx.run_ui(input.clone(), |ui| {
            let none = None;
            let align: [Option<egui::TextureHandle>; 8] = std::array::from_fn(|_| None);
            let icons = DockIcons { rotate: &none, opacity: &none, strokew: &none, align: &align };
            panel_align(ui, &icons, &mut AlignTarget::Auto, &mut vec![]);
            let gaps = align_probes::gaps();
            assert_eq!(gaps.len(), 3);
            for (label, controls) in &gaps {
                assert_eq!(controls.top() - label.bottom(), 6.0, "visible label-to-controls gap");
            }
            current = gaps.last().unwrap().1.bottom() - gaps.first().unwrap().0.top();
        });
        let _ = ctx.run_ui(input, |ui| legacy = legacy_align_height(ui));
        const ALIGN_BEFORE: f32 = 147.0;
        const ALIGN_AFTER: f32 = 149.0;
        assert_eq!((legacy, current), (ALIGN_BEFORE, ALIGN_AFTER));
        assert!(current <= legacy + 2.0, "Align grew from {legacy} to {current}; maximum allowed is {}", legacy + 2.0);
    }

    #[test]
    fn properties_and_pathfinder_height_measurements() {
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        t::apply(&ctx);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(360.0, 900.0))),
            ..Default::default()
        };
        let mut ed = Editor::new();
        ed.ppu = 1.0;
        ed.set_tool(ToolKind::Rect);
        ed.pointer_down([20.0, 20.0]);
        ed.pointer_move([120.0, 120.0]);
        ed.pointer_up();
        ed.set_tool(ToolKind::Object);
        ed.select_all();
        let snap = Snap::read(&ed);
        let _ = ctx.run_ui(input.clone(), |ui| {
            let none = None;
            let align: [Option<egui::TextureHandle>; 8] = std::array::from_fn(|_| None);
            let icons = DockIcons { rotate: &none, opacity: &none, strokew: &none, align: &align };
            panel_properties(
                ui,
                &snap,
                &icons,
                &mut (0.0, 0.0),
                &mut false,
                &mut vec![],
                (&Default::default(), &mut vec![]),
            );
        });
        let properties_after = property_height_probes::get();
        let mut title_delta = 0.0;
        let _ = ctx.run_ui(input.clone(), |ui| {
            let old = ui.label(RichText::new("Rectangle").color(TEXT).size(12.5).strong()).rect.height();
            let new = ui.label(panel_title("Rectangle")).rect.height();
            title_delta = new - old;
        });
        let properties_before = properties_after + 12.0 - title_delta;

        const PANEL_VERTICAL_MARGIN: f32 = 20.0;
        let mut pathfinder_after = 0.0;
        let mut pathfinder_before = 0.0;
        let _ = ctx.run_ui(input.clone(), |ui| {
            pathfinder_after =
                ui.scope(|ui| panel_pathfinder(ui, Ok(()), &mut vec![])).response.rect.height() - PANEL_VERTICAL_MARGIN;
        });
        let _ = ctx.run_ui(input, |ui| {
            pathfinder_before = ui
                .scope(|ui| {
                    egui::Frame::NONE.inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(6.0, 5.0);
                        ui.label(RichText::new("SHAPE MODES").color(MUTED).size(10.0).strong());
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            for _ in 0..4 {
                                ui.allocate_exact_size(egui::vec2(34.0, 28.0), egui::Sense::hover());
                            }
                        });
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("Unite \u{b7} Minus Front \u{b7} Intersect \u{b7} Exclude")
                                .color(MUTED)
                                .size(10.5),
                        );
                    });
                })
                .response
                .rect
                .height()
                - PANEL_VERTICAL_MARGIN;
        });
        const PROPERTIES_BEFORE: f32 = 368.0;
        const PROPERTIES_AFTER: f32 = 357.0;
        const PATHFINDER_BEFORE: f32 = 69.0;
        const PATHFINDER_AFTER: f32 = 47.0;
        assert_eq!((properties_before, properties_after), (PROPERTIES_BEFORE, PROPERTIES_AFTER));
        assert_eq!((pathfinder_before, pathfinder_after), (PATHFINDER_BEFORE, PATHFINDER_AFTER));
    }

    #[test]
    fn section_gap_is_12() {
        assert_eq!((t::SECTION_GAP_HALF, t::LABEL_GAP), (7.0, 6.0));
        let source = src("ui/controls.rs");
        assert_eq!(source.matches("ui.add_space(SECTION_GAP_HALF)").count(), 2);
    }
}

#[cfg(test)]
mod color_tests {
    use super::{hsv_to_rgb, rgb_to_hsv};

    // The picker keeps live HSV as its source of truth — round-tripping a saturated colour through
    // rgb→hsv→rgb must return the original (within float epsilon).
    #[test]
    fn hsv_rgb_roundtrips() {
        for c in [
            [1.0f32, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            [0.12, 0.46, 0.83, 1.0],
            [0.9, 0.7, 0.2, 1.0],
            [0.35, 0.35, 0.35, 1.0], // grey: hue undefined but v/s round-trip
        ] {
            let h = rgb_to_hsv(c);
            let back = hsv_to_rgb(h[0], h[1], h[2]);
            for k in 0..3 {
                assert!((back[k] - c[k]).abs() < 1e-4, "channel {k}: {} vs {}", back[k], c[k]);
            }
        }
    }

    // Known anchors: red is hue 0, sat 1, val 1; a mid-grey is sat 0.
    #[test]
    fn hsv_known_values() {
        let red = rgb_to_hsv([1.0, 0.0, 0.0, 1.0]);
        assert!((red[0]).abs() < 1e-4 && (red[1] - 1.0).abs() < 1e-4 && (red[2] - 1.0).abs() < 1e-4);
        let grey = rgb_to_hsv([0.5, 0.5, 0.5, 1.0]);
        assert!(grey[1].abs() < 1e-4 && (grey[2] - 0.5).abs() < 1e-4);
    }
}

#[cfg(test)]
mod layer_cache_tests {
    use super::{layer_rows_key, thumb_key};
    use std::collections::HashSet;
    use varos_core::editor::Editor;
    use varos_core::model::{Anchor, Path};

    fn editor_with_path() -> Editor {
        let mut ed = Editor::new();
        ed.doc.paths.push(Path::new(
            7,
            vec![
                Anchor { id: 8, p: [0.0, 0.0], hin: None, hout: None, smooth: false },
                Anchor { id: 9, p: [10.0, 0.0], hin: None, hout: None, smooth: false },
            ],
            false,
            None,
            Some([0.0, 0.0, 0.0, 1.0]),
            2.0,
        ));
        ed.doc.sync_tree();
        ed
    }

    #[test]
    fn pointer_only_frames_keep_the_layer_rows_key() {
        let mut ed = editor_with_path();
        let collapsed = HashSet::new();
        let before = layer_rows_key(&ed, &collapsed, "", 0);
        ed.cursor = [400.0, 300.0];
        assert_eq!(before, layer_rows_key(&ed, &collapsed, "", 0));
        ed.objsel.insert(7);
        assert_ne!(before, layer_rows_key(&ed, &collapsed, "", 0));
    }

    #[test]
    fn thumbnail_key_ignores_selection_but_tracks_geometry() {
        let mut ed = editor_with_path();
        let before = thumb_key(&ed, &[7]);
        ed.objsel.insert(7);
        assert_eq!(before, thumb_key(&ed, &[7]));
        ed.doc.paths[0].anchors[0].p[0] = 4.0;
        assert_ne!(before, thumb_key(&ed, &[7]));
    }
}

#[cfg(test)]
mod text_clipboard_tests {
    use super::text_clipboard_event;

    #[test]
    fn menu_clipboard_keys_reach_a_text_field_as_clipboard_events() {
        let unread = || -> Option<String> { panic!("only ⌘V reads the clipboard") };
        assert_eq!(text_clipboard_event(egui::Key::C, unread), Some(Some(egui::Event::Copy)));
        assert_eq!(text_clipboard_event(egui::Key::X, unread), Some(Some(egui::Event::Cut)));
        assert_eq!(
            text_clipboard_event(egui::Key::V, || Some("a\r\nb".into())),
            Some(Some(egui::Event::Paste("a\nb".into())))
        );
        assert_eq!(text_clipboard_event(egui::Key::V, || Some(String::new())), Some(None), "empty ⇒ nothing");
        assert_eq!(text_clipboard_event(egui::Key::V, || None), Some(None));
        assert_eq!(text_clipboard_event(egui::Key::Z, unread), None, "⌘Z stays a key (TextEdit undo)");
    }
}

#[cfg(test)]
mod characterization_tests {
    use super::{apply_ops, Op};
    use varos_core::editor::{Editor, PaintTarget, ToolKind};
    use varos_core::model::{Anchor, Path};

    fn anchor(id: u32, x: f32, y: f32) -> Anchor {
        Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
    }

    fn selected_square() -> Editor {
        let mut ed = Editor::new();
        ed.doc.artboards.clear();
        ed.ppu = 1.0;
        ed.doc.paths.push(Path::new(
            1,
            vec![anchor(1, 0.0, 0.0), anchor(2, 20.0, 0.0), anchor(3, 20.0, 20.0), anchor(4, 0.0, 20.0)],
            true,
            Some([1.0, 0.0, 0.0, 1.0]),
            Some([0.0, 0.0, 0.0, 1.0]),
            3.0,
        ));
        ed.doc.paths[0].opacity = 0.25;
        ed.doc.ids = 4;
        ed.doc.sync_tree();
        ed.objsel.insert(1);
        ed
    }

    #[test]
    fn apply_ops_delegates_selection_edits_and_preserves_clamps() {
        let mut ed = selected_square();

        apply_ops(
            &mut ed,
            vec![
                Op::Tool(ToolKind::Direct),
                Op::SetOpacity(2.0),
                Op::SetStrokeW(-4.0),
                Op::PaintFocus(PaintTarget::Stroke),
            ],
        );

        assert!(ed.tool == ToolKind::Direct);
        assert_eq!(ed.doc.paths[0].opacity, 1.0);
        assert_eq!(ed.doc.paths[0].stroke_width, 0.0);
        assert!(ed.paint == PaintTarget::Stroke);
        assert_eq!(ed.rev, 2, "opacity and stroke width remain separate committed edits");
    }

    #[test]
    fn apply_ops_preserves_direct_document_and_view_writes() {
        let mut ed = selected_square();
        ed.doc.snap.enabled = false;
        let guides_hidden = ed.guides_hidden;
        let rulers_shown = ed.show_rulers;

        apply_ops(
            &mut ed,
            vec![Op::RulerOrigin(Some([13.0, 17.0])), Op::ToggleSnapping, Op::ToggleGuides, Op::ToggleRulers],
        );

        assert_eq!(ed.doc.ruler_origin, [13.0, 17.0]);
        assert_eq!(ed.origin_preview, Some([13.0, 17.0]));
        assert!(ed.doc.snap.enabled);
        assert_eq!(ed.guides_hidden, !guides_hidden);
        assert_eq!(ed.show_rulers, !rulers_shown);

        apply_ops(&mut ed, vec![Op::RulerOrigin(None)]);
        assert_eq!(ed.origin_preview, None);
    }
}

/// QW3 (Astra F10, PAINS_LOG P4 + FB6 nit): the Layers-row rename, driven headlessly through a bare
/// `egui::Context` with synthetic pointer/keyboard input — no window, no GPU.
#[cfg(test)]
mod layer_rename_tests {
    use super::{apply_ops, build_layer_rows, kit, panel_layers, LKind, LRow, LayerIcons, Op, Snap};
    use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput};
    use std::collections::{HashMap, HashSet};
    use varos_core::editor::{Editor, ToolKind};
    use varos_core::model::{Anchor, NodeKind, Path};

    const NAME_X: f32 = 150.0; // inside the name cell of a depth-0 row (the cell starts at x = 94)

    fn path_row(id: u32, name: &str) -> LRow {
        LRow {
            id,
            depth: 0,
            kind: LKind::Path,
            sec: u32::MAX,
            name: name.into(),
            hidden: false,
            locked: false,
            eff_hidden: false,
            eff_locked: false,
            has_children: false,
            collapsed: false,
            selected: false,
            full_sel: false,
            drag_sel: false,
            active: false,
            thumb: vec![],
        }
    }

    fn no_icons() -> LayerIcons {
        LayerIcons { eye: None, eye_off: None, lock: None, unlock: None, search: None }
    }

    /// One Layers panel in a bare context, plus the state `Ui` keeps for it between frames.
    struct Panel {
        ctx: egui::Context,
        t: f64,
        rows: Vec<LRow>,
        icons: LayerIcons,
        search: String,
        rename: Option<(u32, String)>,
        collapsed: HashSet<u32>,
        drag: Option<(u32, u32)>,
        anchor: Option<(u32, u32)>,
        ops: Vec<Op>,
    }

    impl Panel {
        fn new(rows: Vec<LRow>) -> Self {
            let ctx = egui::Context::default();
            varos_app::shell::fonts::install(&ctx);
            let mut p = Panel {
                ctx,
                t: 10.0,
                rows,
                icons: no_icons(),
                search: String::new(),
                rename: None,
                collapsed: HashSet::new(),
                drag: None,
                anchor: None,
                ops: vec![],
            };
            p.frame(vec![]); // egui hit-tests against the PREVIOUS pass's widgets: lay the panel out once
            p
        }

        /// Run one frame 1/60 s after the previous one; returns the ops it emitted.
        fn frame(&mut self, events: Vec<Event>) -> Vec<Op> {
            self.t += 1.0 / 60.0;
            let input = RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(320.0, 480.0))),
                time: Some(self.t),
                modifiers: events
                    .iter()
                    .find_map(|e| match e {
                        Event::PointerButton { modifiers, .. } => Some(*modifiers),
                        _ => None,
                    })
                    .unwrap_or_default(),
                events,
                ..Default::default()
            };
            let Panel { ctx, rows, icons, search, rename, collapsed, drag, anchor, .. } = self;
            let mut ops = vec![];
            let _ = ctx.run_ui(input, |ui| {
                panel_layers(ui, rows, icons, search, rename, collapsed, drag, anchor, &mut ops);
            });
            self.ops.extend(ops.iter().filter_map(clone_op));
            ops
        }
        fn button(&mut self, p: Pos2, button: PointerButton, pressed: bool) -> Vec<Op> {
            let mut ops = self.frame(vec![Event::PointerMoved(p)]); // hover first, as a real mouse does
            ops.extend(self.frame(vec![Event::PointerButton { pos: p, button, pressed, modifiers: Modifiers::NONE }]));
            ops
        }
        fn click(&mut self, p: Pos2) -> Vec<Op> {
            let mut ops = self.button(p, PointerButton::Primary, true);
            ops.extend(self.button(p, PointerButton::Primary, false));
            ops
        }
        fn double_click(&mut self, p: Pos2) -> Vec<Op> {
            let mut ops = self.click(p);
            ops.extend(self.click(p));
            ops
        }
        fn key(&mut self, key: Key) -> Vec<Op> {
            self.frame(vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }])
        }
        fn type_text(&mut self, s: &str) -> Vec<Op> {
            self.frame(vec![Event::Text(s.into())])
        }
        fn focused(&self) -> bool {
            self.ctx.memory(|m| m.focused().is_some())
        }
    }

    /// `Op` is not `Clone`; the log keeps only the kinds these tests inspect. A field's commit
    /// (`Op::Field`, K3) is logged as the op it carries.
    fn clone_op(op: &Op) -> Option<Op> {
        match op {
            Op::Field(op) => clone_op(op),
            Op::LayerRename(id, s) => Some(Op::LayerRename(*id, s.clone())),
            Op::LayerSelectSet(v) => Some(Op::LayerSelectSet(v.clone())),
            Op::AbName(i, s) => Some(Op::AbName(*i, s.clone())),
            _ => None,
        }
    }

    fn renames(ops: &[Op]) -> Vec<(u32, String)> {
        ops.iter()
            .filter_map(|o| match o {
                Op::LayerRename(id, s) => Some((*id, s.clone())),
                _ => None,
            })
            .collect()
    }

    /// Find the screen y of row `id` by probing single clicks down ONE laid-out panel — no layout
    /// constants assumed. Probes sit a second apart (no two can pair into a double-click) and stop as
    /// soon as the row's hit band ends.
    fn row_y(rows: &[LRow], id: u32) -> f32 {
        let mut p = Panel::new(rows.to_vec());
        let mut hits: Vec<f32> = vec![];
        for k in 0..120 {
            let y = k as f32 * 2.0;
            p.t += 1.0;
            let hit =
                p.click(egui::pos2(NAME_X, y)).iter().any(|o| matches!(o, Op::LayerSelectSet(v) if v == &vec![id]));
            assert!(p.rename.is_none(), "a probe opened the editor");
            if hit {
                hits.push(y);
            } else if !hits.is_empty() {
                break;
            }
        }
        assert!(!hits.is_empty(), "row {id} was never hit by a click");
        (hits[0] + hits[hits.len() - 1]) * 0.5
    }

    fn set_paths_filter(p: &mut Panel) {
        let id = egui::Id::new(("layer-kind-filter", None::<crate::app_command::SessionId>));
        p.ctx.data_mut(|d| d.insert_temp(id, 1usize));
        p.frame(vec![]);
    }

    #[test]
    fn filtered_click_and_shift_range_skip_hidden_kinds() {
        let visible = [path_row(3, "A"), path_row(4, "B")];
        let y1 = row_y(&visible, 3);
        let y2 = row_y(&visible, 4);
        let mut hidden = path_row(5, "Hidden group");
        hidden.kind = LKind::Group;
        let mut p = Panel::new(vec![visible[0].clone(), hidden, visible[1].clone()]);
        set_paths_filter(&mut p);
        assert!(p.click(egui::pos2(NAME_X, y1)).iter().any(|o| matches!(o, Op::LayerSelectSet(v) if v == &[3])));
        p.t += 1.0;
        let pos = egui::pos2(NAME_X, y2);
        let modifiers = Modifiers { shift: true, ..Modifiers::NONE };
        p.frame(vec![Event::PointerMoved(pos)]);
        p.frame(vec![Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers }]);
        let ops =
            p.frame(vec![Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers }]);
        assert!(ops.iter().any(|o| matches!(o, Op::LayerSelectSet(v) if v == &[3, 4])));
    }

    #[test]
    fn filtered_drag_payload_contains_only_matching_kinds() {
        let mut rows = vec![path_row(3, "A"), path_row(5, "Hidden group"), path_row(4, "B"), path_row(6, "Target")];
        rows[1].kind = LKind::Group;
        for r in &mut rows[..3] {
            r.drag_sel = true;
            r.full_sel = true;
        }
        // A fully selected nonmatching parent normally owns the drag roots.
        rows[0].drag_sel = false;
        rows[2].drag_sel = false;
        let visible = vec![rows[0].clone(), rows[2].clone(), rows[3].clone()];
        let from = egui::pos2(NAME_X, row_y(&visible, 3));
        let to = egui::pos2(NAME_X, row_y(&visible, 6) + 8.0);
        let mut p = Panel::new(rows);
        set_paths_filter(&mut p);
        p.button(from, PointerButton::Primary, true);
        p.frame(vec![Event::PointerMoved(from + egui::vec2(0.0, 8.0))]);
        p.frame(vec![Event::PointerMoved(to)]);
        p.frame(vec![]);
        let ops = p.button(to, PointerButton::Primary, false);
        assert!(ops.iter().any(|o| matches!(o, Op::LayerMove(v, 6, _) if v == &[3, 4])));
    }

    #[test]
    fn active_filters_walk_collapsed_boards_layers_and_groups_and_keep_ancestors() {
        let mut ed = two_path_editor();
        ed.doc.artboards.push(varos_core::model::Artboard::default());
        ed.objsel.extend([1, 2]);
        ed.group_selection();
        let group = ed.doc.nodes.iter().find(|n| matches!(n.kind, NodeKind::Group)).unwrap().id;
        let mut layer = ed.doc.node(group).unwrap().clone();
        layer.id = ed.doc.nid();
        layer.kind = NodeKind::Layer;
        layer.name = "Nested layer".into();
        layer.children = vec![group];
        let layer_id = layer.id;
        let parent = layer.parent.unwrap();
        ed.doc.nodes.iter_mut().find(|n| n.id == parent).unwrap().children = vec![layer_id];
        ed.doc.nodes.iter_mut().find(|n| n.id == group).unwrap().parent = Some(layer_id);
        ed.doc.nodes.push(layer);
        let collapsed = HashSet::from([layer_id, group, super::board_row_id(0)]);
        let mut cache = HashMap::new();
        assert!(!build_layer_rows(&ed, &collapsed, "", 0, &mut cache).iter().any(|r| r.kind == LKind::Path));
        let rows = build_layer_rows(&ed, &collapsed, "", 1, &mut cache);
        assert_eq!(rows.iter().filter(|r| r.kind == LKind::Path).count(), 2);
        assert!(rows.iter().any(|r| r.id == group));
        assert!(rows.iter().any(|r| r.id == layer_id));
        assert!(rows.iter().any(|r| r.kind == LKind::Board));
        let groups = build_layer_rows(&ed, &collapsed, "", 2, &mut cache);
        assert!(groups.iter().any(|r| r.id == group));
        assert!(!groups.iter().any(|r| r.kind == LKind::Path));
        assert_ne!(super::layer_rows_key(&ed, &collapsed, "", 0), super::layer_rows_key(&ed, &collapsed, "", 1));
    }

    fn two_paths() -> Vec<LRow> {
        vec![path_row(3, "<Path>"), path_row(4, "<Path>")]
    }

    /// Double-click a row name, then let the editor settle for two frames.
    fn open_editor(rows: &[LRow], id: u32) -> Panel {
        let y = row_y(rows, id);
        let mut p = Panel::new(rows.to_vec());
        p.double_click(egui::pos2(NAME_X, y));
        p.frame(vec![]);
        p.frame(vec![]);
        p
    }

    #[test]
    fn double_click_opens_rename_and_enter_commits() {
        let mut p = open_editor(&two_paths(), 4);
        assert_eq!(p.rename.as_ref().map(|r| r.0), Some(4), "double-click on a <Path> row opened no editor");
        assert!(p.focused(), "the editor never took keyboard focus");
        p.type_text("Logo");
        p.key(Key::Enter);
        p.frame(vec![]);
        assert_eq!(renames(&p.ops), vec![(4, "Logo".to_string())], "typing a name + Enter must commit exactly it");
        assert!(p.rename.is_none(), "Enter closes the editor");
    }

    #[test]
    fn rename_field_keeps_focus_on_the_frame_it_opens() {
        let rows = two_paths();
        let y = row_y(&rows, 3);
        let mut p = Panel::new(rows);
        p.double_click(egui::pos2(NAME_X, y));
        assert_eq!(p.rename.as_ref().map(|r| r.0), Some(3), "double-click opened no editor");
        // the frames right after opening: no commit sneaks out, focus arrives and stays
        for _ in 0..4 {
            let ops = p.frame(vec![]);
            assert!(renames(&ops).is_empty(), "the editor committed on its own while opening");
            assert!(p.rename.is_some(), "the editor closed while opening");
        }
        assert!(p.focused());
    }

    #[test]
    fn right_click_rename_opens_the_same_editor() {
        let rows = two_paths();
        let y = row_y(&rows, 4);
        let mut p = Panel::new(rows);
        p.button(egui::pos2(NAME_X, y), PointerButton::Secondary, true);
        p.button(egui::pos2(NAME_X, y), PointerButton::Secondary, false);
        p.frame(vec![]);
        assert!(p.rename.is_none(), "right-click alone must not start renaming");
        // the menu hangs under the row: probe downwards for its single "Rename" row
        let mut opened = false;
        for dy in (14..60).step_by(4) {
            let q = egui::pos2(NAME_X - 40.0, y + dy as f32);
            p.frame(vec![Event::PointerMoved(q)]);
            p.click(q);
            if p.rename.is_some() {
                opened = true;
                break;
            }
        }
        assert!(opened, "Rename in the row's context menu did not open the editor");
        assert_eq!(p.rename.as_ref().map(|r| r.0), Some(4));
        p.frame(vec![]);
        p.frame(vec![]);
        assert!(p.focused());
        p.type_text("Ring");
        p.key(Key::Enter);
        p.frame(vec![]);
        assert_eq!(renames(&p.ops), vec![(4, "Ring".to_string())]);
    }

    #[test]
    fn escape_or_blur_with_unchanged_name_is_harmless() {
        let rows = vec![path_row(3, "Logo"), path_row(4, "<Path>")];
        // Escape with an unchanged name: nothing is emitted
        let mut p = open_editor(&rows, 3);
        assert!(p.rename.is_some());
        p.key(Key::Escape);
        p.frame(vec![]);
        assert!(p.rename.is_none(), "Escape closes the editor");
        assert!(renames(&p.ops).is_empty(), "Escape with an unchanged name emitted a rename");
        // blur (click another row) with an unchanged name: nothing is emitted
        let y4 = row_y(&rows, 4);
        let mut p = open_editor(&rows, 3);
        assert!(p.rename.is_some());
        p.click(egui::pos2(NAME_X, y4));
        p.frame(vec![]);
        assert!(p.rename.is_none(), "clicking elsewhere closes the editor");
        assert!(renames(&p.ops).is_empty(), "blur with an unchanged name emitted a rename");
    }

    #[test]
    fn escape_cancels_an_edited_name() {
        let mut p = open_editor(&two_paths(), 4);
        assert!(p.rename.is_some());
        p.type_text("Oops");
        p.key(Key::Escape);
        p.frame(vec![]);
        assert!(p.rename.is_none());
        assert!(renames(&p.ops).is_empty(), "Escape must cancel (Illustrator), not commit the typed text");
    }

    #[test]
    fn typing_replaces_the_whole_old_name() {
        let mut p = open_editor(&[path_row(3, "Logo")], 3);
        p.type_text("Mark");
        p.key(Key::Enter);
        p.frame(vec![]);
        assert_eq!(renames(&p.ops), vec![(3, "Mark".to_string())], "the old name is selected on open");
    }

    /// K3: an empty name is invalid — Enter keeps the editor open and focused (with its reason) instead
    /// of silently closing; Esc then leaves the old name.
    #[test]
    fn emptied_name_keeps_the_old_one() {
        let mut p = open_editor(&[path_row(3, "Logo")], 3);
        assert!(p.rename.is_some());
        p.frame(vec![Event::Key {
            key: Key::A,
            physical_key: Some(Key::A),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        }]);
        p.key(Key::Backspace);
        p.key(Key::Enter);
        p.frame(vec![]);
        assert!(p.rename.is_some() && p.focused(), "an empty name keeps the editor and the keyboard");
        let id = p.ctx.memory(|m| m.focused()).unwrap();
        assert_eq!(kit::field::reason(&p.ctx, id), Some(kit::field::EMPTY_NAME), "…and says why");
        p.key(Key::Escape);
        p.frame(vec![]);
        assert!(p.rename.is_none());
        assert!(renames(&p.ops).is_empty(), "an empty name must keep the old one (no rename)");
    }

    fn anchor(id: u32, x: f32, y: f32) -> Anchor {
        Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
    }

    /// Two open paths: A (id 1, red fill, weight 2) and B (id 2, blue fill, weight 7).
    fn two_path_editor() -> Editor {
        let mut ed = Editor::new();
        ed.doc.artboards.clear();
        ed.doc.paths.push(Path::new(
            1,
            vec![anchor(11, 0.0, 0.0), anchor(12, 10.0, 0.0)],
            false,
            Some([1.0, 0.0, 0.0, 1.0]),
            Some([0.0, 0.0, 0.0, 1.0]),
            2.0,
        ));
        ed.doc.paths.push(Path::new(
            2,
            vec![anchor(21, 50.0, 50.0), anchor(22, 80.0, 60.0)],
            false,
            Some([0.0, 0.0, 1.0, 1.0]),
            Some([0.0, 0.0, 0.0, 1.0]),
            7.0,
        ));
        ed.doc.ids = 30;
        ed.doc.sync_tree();
        ed
    }

    /// The whole chain a real double-click drives: the panel's op → `apply_ops` → the rebuilt row.
    #[test]
    fn path_row_rename_shows_in_the_rebuilt_row() {
        let mut ed = two_path_editor();
        let node = ed.doc.node_of_path(2).expect("path 2 has a leaf node");
        let rev = ed.rev;
        apply_ops(&mut ed, vec![Op::LayerRename(node, "Logo".into())]);
        let mut thumbs = HashMap::new();
        let rows = build_layer_rows(&ed, &HashSet::new(), "", 0, &mut thumbs);
        let row = rows.iter().find(|r| r.id == node).expect("the path's row");
        assert_eq!(row.name, "Logo", "the rename landed nowhere the Layers row reads");
        assert_eq!(ed.rev, rev + 1, "a rename is one undoable edit");
        ed.undo();
        let rows = build_layer_rows(&ed, &HashSet::new(), "", 0, &mut thumbs);
        assert_eq!(rows.iter().find(|r| r.id == node).unwrap().name, "<Path>", "undo restores the auto-name");
        // a Layer (container) row keeps using the node rename — its name lives on the node
        let layer = ed.doc.nodes.iter().find(|n| matches!(n.kind, NodeKind::Layer)).map(|n| n.id).unwrap();
        apply_ops(&mut ed, vec![Op::LayerRename(layer, "Art".into())]);
        assert_eq!(ed.doc.node(layer).unwrap().name, "Art");
        assert_eq!(ed.doc.paths[1].name.as_deref(), None, "renaming the layer left the path alone");
    }

    /// Pen draft through the real tool: two clicks away from everything, drawn in `cur_fill` (blue).
    fn draw_two_points(ed: &mut Editor) {
        ed.cur_fill = Some([0.0, 0.0, 1.0, 1.0]);
        ed.ppu = 1.0;
        ed.set_tool(ToolKind::Pen);
        for p in [[200.0, 200.0], [260.0, 230.0]] {
            ed.pointer_down(p);
            ed.pointer_up();
        }
        assert!(ed.active.is_some(), "mid-draft");
    }

    /// PAINS_LOG FB6 nit + QW3 review P2-1: mid-draft, the dock describes the draft AND its fields edit
    /// the draft — with or without a selection left over from before the Pen. Chosen behaviour: the
    /// Transform block stays live on the draft (its anchors, as the no-selection case always did); the
    /// Pen deselects other art when the draft starts, so no field can reach the old object.
    #[test]
    fn drawing_snap_reports_active_path_not_stale_selection() {
        let read = |ed: &Editor| {
            let s = Snap::read(ed);
            (s.name, s.sel, s.direct, s.drawing, [s.x, s.y, s.w, s.h], s.fill, s.sw)
        };
        // leftover selection: A (path 1, red) selected, then the Pen draws B
        let mut stale = two_path_editor();
        stale.doc.paths[0].name = Some("Old".into());
        stale.objsel.insert(1);
        assert_eq!(read(&stale).0, "Old", "control: before the Pen, the dock names A");
        draw_two_points(&mut stale);
        // no selection: the same draft
        let mut clean = two_path_editor();
        draw_two_points(&mut clean);

        let (name, sel, direct, drawing, xywh, fill, _) = read(&stale);
        assert_eq!(name, "Drawing path\u{2026}");
        assert!(drawing && !sel && direct, "the draft (its anchors) is what the dock measures");
        assert_eq!(xywh, [200.0, 200.0, 60.0, 30.0], "live numbers of the draft, not zeros or A's");
        assert_eq!(fill, Some([0.0, 0.0, 1.0, 1.0]), "paint of the draft, not A's red");
        assert_eq!(read(&stale), read(&clean), "a leftover selection changes nothing about the draft's dock");

        // the fields write to what they show: X = 500 moves the draft, never A
        let a_before: Vec<_> = stale.doc.paths[0].anchors.iter().map(|a| a.p).collect();
        apply_ops(&mut stale, vec![Op::SetBBox(Some(500.0), None, None, None, 0.0, 0.0)]);
        let a_after: Vec<_> = stale.doc.paths[0].anchors.iter().map(|a| a.p).collect();
        assert_eq!(a_before, a_after, "a mid-draft X edit moved the old object");
        assert_eq!(read(&stale).4[0], 500.0, "…it moved the draft");

        // not drawing: the ordinary selection read is unchanged
        let mut idle = two_path_editor();
        idle.doc.paths[0].name = Some("Old".into());
        idle.objsel.insert(1);
        let s = Snap::read(&idle);
        assert!(!s.drawing && s.sel);
        assert_eq!(s.name, "Old");
        assert_eq!(s.fill, Some([1.0, 0.0, 0.0, 1.0]));
    }
}

/// DFS S1 C — headless (no GPU, no `EventLoop`) proof that `build_topbar` really drives
/// `AppCommand`s: no wgpu `Renderer`, just `egui::Context::run_ui` fed synthetic pointer input, the
/// same technique `shell::boxtree::tests` already uses for headless rendering.
#[cfg(test)]
mod tab_strip_tests {
    use super::*;
    use crate::app_command::{AppCommand, SessionId, TabView};
    use egui::{Event, PointerButton, Pos2, RawInput};

    fn icons() -> TopIcons {
        TopIcons { menu: None }
    }

    /// A context with the app's fonts: the band paints Inter (`tokens::small` / `small_medium`).
    fn ctx() -> egui::Context {
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        ctx
    }
    fn tab(id: u64, label: &str, dirty: bool) -> TabView {
        TabView { id: SessionId(id), label: label.into(), dirty, tooltip: "Not saved yet".into() }
    }
    /// The top bar's own rect — matches what `Panel::top(..).exact_size(h)` claims inside
    /// `build_topbar`, and what `crate::chrome::topbar_layout` is fed.
    fn bar_rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, crate::chrome::TOPBAR.height))
    }
    /// The FULL window `RawInput.screen_rect`: much taller than the bar itself, so `menu_below`'s
    /// `Area::constrain(true)` has room to place a dropdown BELOW the bar instead of clamping it back
    /// inside a screen that was only as tall as the bar.
    fn screen_rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0))
    }
    fn press(pos: Pos2, button: PointerButton) -> RawInput {
        RawInput {
            screen_rect: Some(screen_rect()),
            events: vec![
                Event::PointerMoved(pos),
                Event::PointerButton { pos, button, pressed: true, modifiers: Default::default() },
            ],
            ..Default::default()
        }
    }
    fn release(pos: Pos2, button: PointerButton) -> RawInput {
        RawInput {
            screen_rect: Some(screen_rect()),
            events: vec![Event::PointerButton { pos, button, pressed: false, modifiers: Default::default() }],
            ..Default::default()
        }
    }
    fn idle() -> RawInput {
        RawInput { screen_rect: Some(screen_rect()), ..Default::default() }
    }

    /// Drives one `build_topbar` frame and returns whatever `AppCommand`s it raised.
    #[allow(clippy::too_many_arguments)]
    fn frame(
        ctx: &egui::Context,
        input: RawInput,
        top: &TopIcons,
        shell: &mut varos_app::shell::ShellState,
        tabs: &[TabView],
        active: Option<SessionId>,
        show_rail: &mut bool,
        show_dock: &mut bool,
    ) -> Vec<AppCommand> {
        let mut win_action = None;
        let mut cmds = Vec::new();
        let _ = ctx.run_ui(input, |root| {
            build_topbar(
                root,
                top,
                shell,
                &mut win_action,
                tabs,
                active,
                &mut cmds,
                show_rail,
                show_dock,
                &mut Default::default(),
                None,
                false,
                false,
                false,
            );
        });
        cmds
    }

    /// The chip / `+` / "+N" / V rects for these tabs — measured with the exact same font
    /// call `build_topbar` makes (Inter 500 12, `tokens::small_medium`), on the SAME `Context`, so the
    /// rects line up with what a real frame draws.
    fn measure(ctx: &egui::Context, tabs: &[TabView], active: Option<SessionId>) -> crate::chrome::TopbarLayout {
        let bar = bar_rect();
        let mut out = None;
        let _ = ctx.run_ui(idle(), |ui| {
            let p = ui.painter().clone();
            let text_width = |t: &str| {
                p.layout_no_wrap(t.to_owned(), varos_app::shell::tokens::small_medium(), Color32::WHITE).size().x
            };
            let widths: Vec<f32> = tabs.iter().map(|t| text_width(&t.label)).collect();
            let active_index = active.and_then(|id| tabs.iter().position(|t| t.id == id));
            out = Some(crate::chrome::topbar_layout(bar, crate::chrome::TOPBAR, None, &widths, active_index));
        });
        out.unwrap()
    }

    /// A warm-up frame, then press then release `button` at `pos`. egui resolves which widget a
    /// pointer event hit from the PREVIOUS frame's registered rects, so the very first frame a chip
    /// exists in can never be the one that receives its press — the warm-up frame is what makes the
    /// chip "exist" before the click starts.
    #[allow(clippy::too_many_arguments)]
    fn click_at(
        ctx: &egui::Context,
        pos: Pos2,
        button: PointerButton,
        top: &TopIcons,
        shell: &mut varos_app::shell::ShellState,
        tabs: &[TabView],
        active: Option<SessionId>,
        show_rail: &mut bool,
        show_dock: &mut bool,
    ) -> Vec<AppCommand> {
        let _ = frame(ctx, idle(), top, shell, tabs, active, show_rail, show_dock);
        let _ = frame(ctx, press(pos, button), top, shell, tabs, active, show_rail, show_dock);
        frame(ctx, release(pos, button), top, shell, tabs, active, show_rail, show_dock)
    }

    #[test]
    fn click_emits_activate_document() {
        let ctx = ctx();
        let tabs = vec![tab(1, "A", false), tab(2, "B", false)];
        let layout = measure(&ctx, &tabs, Some(SessionId(1)));
        let (i, rect) = layout.tabs[1]; // the second placed chip
        assert_eq!(tabs[i].id, SessionId(2));
        let mut shell = varos_app::shell::ShellState::standard();
        let (mut rail, mut dock) = (true, true);
        let pos = egui::pos2(rect.left() + 20.0, rect.center().y); // clear of the × in the corner
        let cmds = click_at(
            &ctx,
            pos,
            PointerButton::Primary,
            &icons(),
            &mut shell,
            &tabs,
            Some(SessionId(1)),
            &mut rail,
            &mut dock,
        );
        assert_eq!(cmds, [AppCommand::ActivateDocument(SessionId(2))]);
    }

    #[test]
    fn middle_click_emits_close_document() {
        let ctx = ctx();
        let tabs = vec![tab(1, "A", false)];
        let layout = measure(&ctx, &tabs, Some(SessionId(1)));
        let (_, rect) = layout.tabs[0];
        let mut shell = varos_app::shell::ShellState::standard();
        let (mut rail, mut dock) = (true, true);
        let pos = egui::pos2(rect.left() + 20.0, rect.center().y);
        let cmds = click_at(
            &ctx,
            pos,
            PointerButton::Middle,
            &icons(),
            &mut shell,
            &tabs,
            Some(SessionId(1)),
            &mut rail,
            &mut dock,
        );
        assert_eq!(cmds, [AppCommand::CloseDocument(SessionId(1))]);
    }

    #[test]
    fn close_x_emits_close_document() {
        let ctx = ctx();
        let tabs = vec![tab(1, "A", false)];
        let layout = measure(&ctx, &tabs, Some(SessionId(1)));
        let (_, rect) = layout.tabs[0];
        let mut shell = varos_app::shell::ShellState::standard();
        let (mut rail, mut dock) = (true, true);
        let pos = crate::chrome::tab_close_rect(rect).center();
        let cmds = click_at(
            &ctx,
            pos,
            PointerButton::Primary,
            &icons(),
            &mut shell,
            &tabs,
            Some(SessionId(1)),
            &mut rail,
            &mut dock,
        );
        assert_eq!(cmds, [AppCommand::CloseDocument(SessionId(1))]);
    }

    #[test]
    fn plus_emits_new_document() {
        let ctx = ctx();
        let tabs = vec![tab(1, "A", false)];
        let layout = measure(&ctx, &tabs, Some(SessionId(1)));
        let plus = layout.plus.expect("+ is always placed (F15)");
        let mut shell = varos_app::shell::ShellState::standard();
        let (mut rail, mut dock) = (true, true);
        let cmds = click_at(
            &ctx,
            plus.center(),
            PointerButton::Primary,
            &icons(),
            &mut shell,
            &tabs,
            Some(SessionId(1)),
            &mut rail,
            &mut dock,
        );
        assert_eq!(cmds, [AppCommand::NewBoard]);
    }

    #[test]
    fn drag_reorder_emits_reorder_document_with_the_right_slot() {
        let ctx = ctx();
        let tabs = vec![tab(1, "A", false), tab(2, "B", false), tab(3, "C", false)];
        let layout = measure(&ctx, &tabs, Some(SessionId(1)));
        let src_rect = layout.tabs[0].1; // chip A
        let dst_rect = layout.tabs[2].1; // chip C — drop past its centre = the end slot
        let mut shell = varos_app::shell::ShellState::standard();
        let (mut rail, mut dock) = (true, true);
        let from = egui::pos2(src_rect.left() + 20.0, src_rect.center().y);
        let to = egui::pos2(dst_rect.right() - 4.0, dst_rect.center().y);
        // a warm-up frame (chips must exist in the PREVIOUS frame to be hit-tested — see `click_at`),
        // then press on A, drag past a real threshold, drop on C's right half → slot 3 (after every chip)
        let _ = frame(&ctx, idle(), &icons(), &mut shell, &tabs, Some(SessionId(1)), &mut rail, &mut dock);
        let _ = frame(
            &ctx,
            press(from, PointerButton::Primary),
            &icons(),
            &mut shell,
            &tabs,
            Some(SessionId(1)),
            &mut rail,
            &mut dock,
        );
        let mid = egui::pos2(from.x + 30.0, from.y);
        let _ = frame(
            &ctx,
            RawInput { screen_rect: Some(screen_rect()), events: vec![Event::PointerMoved(mid)], ..Default::default() },
            &icons(),
            &mut shell,
            &tabs,
            Some(SessionId(1)),
            &mut rail,
            &mut dock,
        );
        let _ = frame(
            &ctx,
            RawInput { screen_rect: Some(screen_rect()), events: vec![Event::PointerMoved(to)], ..Default::default() },
            &icons(),
            &mut shell,
            &tabs,
            Some(SessionId(1)),
            &mut rail,
            &mut dock,
        );
        let cmds = frame(
            &ctx,
            release(to, PointerButton::Primary),
            &icons(),
            &mut shell,
            &tabs,
            Some(SessionId(1)),
            &mut rail,
            &mut dock,
        );
        assert_eq!(cmds, [AppCommand::ReorderDocument(SessionId(1), 3)]);
    }

    /// Press at `from`, move past egui's drag threshold, move to `to`, release there — after a warm-up
    /// frame (see `click_at`). Returns the commands the release frame raised.
    #[allow(clippy::too_many_arguments)]
    fn drag(
        ctx: &egui::Context,
        from: Pos2,
        to: Pos2,
        tabs: &[TabView],
        active: Option<SessionId>,
        shell: &mut varos_app::shell::ShellState,
        rail: &mut bool,
        dock: &mut bool,
    ) -> Vec<AppCommand> {
        let moved = |p: Pos2| RawInput {
            screen_rect: Some(screen_rect()),
            events: vec![Event::PointerMoved(p)],
            ..Default::default()
        };
        let _ = frame(ctx, idle(), &icons(), shell, tabs, active, rail, dock);
        let _ = frame(ctx, press(from, PointerButton::Primary), &icons(), shell, tabs, active, rail, dock);
        let step = if to.x >= from.x { 30.0 } else { -30.0 };
        let _ = frame(ctx, moved(egui::pos2(from.x + step, from.y)), &icons(), shell, tabs, active, rail, dock);
        let _ = frame(ctx, moved(to), &icons(), shell, tabs, active, rail, dock);
        frame(ctx, release(to, PointerButton::Primary), &icons(), shell, tabs, active, rail, dock)
    }

    /// P15 (owner 2026-09-25: "dragging a tab moves the whole window"; Codex saw the order never
    /// change with 8 tabs). With 8 tabs overflowing the strip, a press on a drawn chip is NOT a
    /// caption-drag spot (the same `interactive_rects` → `caption_hit` both platforms run), and
    /// press → move → release across the neighbouring chip reorders — in the FULL order, also for the
    /// active chip that overflow moved into the last drawn slot.
    #[test]
    fn eight_tab_overflow_drag_reorders_and_never_starts_a_window_drag() {
        let ctx = ctx();
        let tabs: Vec<TabView> = (1..=8).map(|i| tab(i, &format!("Brand guidelines draft {i}"), false)).collect();
        let active = Some(SessionId(8));
        let layout = measure(&ctx, &tabs, active);
        let drawn: Vec<usize> = layout.tabs.iter().map(|&(i, _)| i).collect();
        assert!(drawn.len() < 8 && drawn.len() >= 3, "setup: 8 tabs overflow the 1400-px bar, got {drawn:?}");
        assert_eq!(*drawn.last().unwrap(), 7, "setup: the active (last) tab takes the last drawn slot");
        let chrome = crate::chrome::TOPBAR;
        let excl = crate::chrome::caption_exclusions(&layout.interactive_rects(), 1.0);
        let drags_window = |p: Pos2| crate::chrome::caption_hit(chrome.height as i32, &excl, p.x as i32, p.y as i32);
        for &(i, r) in &layout.tabs {
            assert!(!drags_window(egui::pos2(r.left() + 20.0, r.center().y)), "chip {i}: press drags the window");
        }
        let mut shell = varos_app::shell::ShellState::standard();
        let (mut rail, mut dock) = (true, true);

        // chip 0 lifted and dropped right onto its neighbour's slot (drawn slot 1) → the gap is after
        // chip 1 → full slot of drawn chip 2. P16: the drop spot is the LIFTED chip (grabbed 20 px in),
        // no longer the bare pointer — so the pointer ends 20 px into chip 1's slot.
        let (r0, r1) = (layout.tabs[0].1, layout.tabs[1].1);
        let cmds = drag(
            &ctx,
            egui::pos2(r0.left() + 20.0, r0.center().y),
            egui::pos2(r1.left() + 20.0, r1.center().y),
            &tabs,
            active,
            &mut shell,
            &mut rail,
            &mut dock,
        );
        assert_eq!(cmds, [AppCommand::ReorderDocument(SessionId(1), drawn[2])]);

        // the active overflow chip (last drawn) dragged onto the left half of chip 0 → full slot 0
        let rl = layout.tabs.last().unwrap().1;
        let cmds = drag(
            &ctx,
            egui::pos2(rl.left() + 20.0, rl.center().y),
            egui::pos2(r0.left() + 4.0, r0.center().y),
            &tabs,
            active,
            &mut shell,
            &mut rail,
            &mut dock,
        );
        assert_eq!(cmds, [AppCommand::ReorderDocument(SessionId(8), 0)]);
    }

    /// P16 harness: one tab strip driven frame by frame, returning the commands AND the painted shapes
    /// of each frame, so a test can read where every chip was actually DRAWN (not where the layout
    /// said it would be).
    struct Strip {
        ctx: egui::Context,
        tabs: Vec<TabView>,
        active: Option<SessionId>,
        shell: varos_app::shell::ShellState,
        rail: bool,
        dock: bool,
        /// `RawInput::focused` on every frame. egui-winit STARTS at `false` and on macOS only flips it
        /// when a winit `Focused` event arrives — which a real session may never deliver (observed:
        /// the app launched as a bundle ran a whole session with `focused == false`).
        focused: bool,
    }

    impl Strip {
        fn new(labels: &[&str], active: usize) -> Self {
            let tabs: Vec<TabView> = labels.iter().enumerate().map(|(i, l)| tab(i as u64 + 1, l, false)).collect();
            let active = Some(tabs[active].id);
            Strip {
                ctx: ctx(),
                tabs,
                active,
                shell: varos_app::shell::ShellState::standard(),
                rail: true,
                dock: true,
                focused: true,
            }
        }
        fn layout(&self) -> crate::chrome::TopbarLayout {
            measure(&self.ctx, &self.tabs, self.active)
        }
        /// Chip rect of the tab labelled `label` in the resting layout.
        fn home(&self, label: &str) -> egui::Rect {
            let i = self.tabs.iter().position(|t| t.label == label).expect("a tab with that label");
            self.layout().tabs.iter().find(|&&(j, _)| j == i).expect("chip is drawn").1
        }
        fn run(&mut self, mut input: RawInput) -> (Vec<AppCommand>, Vec<egui::epaint::ClippedShape>) {
            input.focused &= self.focused;
            let mut win_action = None;
            let mut cmds = Vec::new();
            let (tabs, active) = (&self.tabs, self.active);
            let (shell, rail, dock) = (&mut self.shell, &mut self.rail, &mut self.dock);
            let out = self.ctx.run_ui(input, |root| {
                build_topbar(
                    root,
                    &icons(),
                    shell,
                    &mut win_action,
                    tabs,
                    active,
                    &mut cmds,
                    rail,
                    dock,
                    &mut Default::default(),
                    None,
                    false,
                    false,
                    false,
                );
            });
            (cmds, out.shapes)
        }
        fn idle(&mut self) -> (Vec<AppCommand>, Vec<egui::epaint::ClippedShape>) {
            self.run(idle())
        }
        fn press(&mut self, at: Pos2) -> (Vec<AppCommand>, Vec<egui::epaint::ClippedShape>) {
            self.run(press(at, PointerButton::Primary))
        }
        fn move_to(&mut self, at: Pos2) -> (Vec<AppCommand>, Vec<egui::epaint::ClippedShape>) {
            self.run(RawInput {
                screen_rect: Some(screen_rect()),
                events: vec![Event::PointerMoved(at)],
                ..Default::default()
            })
        }
        fn release(&mut self, at: Pos2) -> (Vec<AppCommand>, Vec<egui::epaint::ClippedShape>) {
            self.run(release(at, PointerButton::Primary))
        }
        /// Warm-up frame (see `click_at`), press at `from`, then one move 30 px towards `dir` — past
        /// egui's drag threshold, so the drag has started.
        fn begin_drag(&mut self, from: Pos2, dir: f32) {
            let _ = self.idle();
            let _ = self.press(from);
            let _ = self.move_to(egui::pos2(from.x + 30.0 * dir.signum(), from.y));
        }
    }

    /// Where the chip labelled `label` was PAINTED this frame: `(left edge, label y, paint order)`.
    /// A clean chip's label starts 12 px in from its left edge (`tab_item`), and a higher paint order
    /// means drawn later = on top.
    fn painted(shapes: &[egui::epaint::ClippedShape], label: &str) -> (f32, f32, usize) {
        let hits: Vec<(f32, f32, usize)> = shapes
            .iter()
            .enumerate()
            .filter_map(|(k, cs)| match &cs.shape {
                egui::Shape::Text(t) if t.galley.text() == label => Some((t.pos.x - 12.0, t.pos.y, k)),
                _ => None,
            })
            .collect();
        assert_eq!(hits.len(), 1, "chip {label:?} must be painted exactly once, got {hits:?}");
        hits[0]
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.51
    }

    /// P16 (a): once a chip is dragged past the threshold it is LIFTED — painted under the pointer
    /// with the grab offset kept, above every other chip, y locked to the strip, and clamped to the
    /// strip's two ends.
    #[test]
    fn lifted_tab_follows_the_pointer_with_its_grab_offset_clamped_to_the_strip() {
        let mut s = Strip::new(&["A", "B", "C"], 0);
        let (a, b, c) = (s.home("A"), s.home("B"), s.home("C"));
        let grab = 25.0;
        let from = egui::pos2(b.left() + grab, b.center().y);
        s.begin_drag(from, 1.0);

        let (_, shapes) = s.move_to(egui::pos2(from.x + 17.0, from.y));
        let (left, y, order) = painted(&shapes, "B");
        let rest_y = painted(&shapes, "A").1; // A rests in the strip's row
        assert!(near(left, b.left() + 17.0), "B painted at {left}, want pointer − grab = {}", b.left() + 17.0);
        assert!(order > painted(&shapes, "A").2 && order > painted(&shapes, "C").2, "the lifted chip is drawn on top");

        // y stays locked to the strip even when the pointer leaves the bar downwards
        let (_, shapes) = s.move_to(egui::pos2(from.x + 17.0, from.y + 120.0));
        let (left, y2, _) = painted(&shapes, "B");
        assert!(near(left, b.left() + 17.0) && near(y2, y), "y must stay locked ({y2} vs {y})");
        assert!(near(y, rest_y), "the lifted chip keeps the strip's row");

        // clamped: far right → its right edge sits on the strip's right end (C's right edge)
        let (_, shapes) = s.move_to(egui::pos2(1390.0, from.y));
        let (left, _, _) = painted(&shapes, "B");
        assert!(
            near(left + b.width(), c.right()),
            "clamped right: B right {} vs strip end {}",
            left + b.width(),
            c.right()
        );
        // far left → its left edge sits on the strip's left end (A's left edge)
        let (_, shapes) = s.move_to(egui::pos2(0.0, from.y));
        let (left, _, _) = painted(&shapes, "B");
        assert!(near(left, a.left()), "clamped left: B left {left} vs strip start {}", a.left());
    }

    /// P16 (b): while a chip is lifted, the OTHER chips reflow every frame to leave a gap exactly
    /// where it would land. A neighbour crosses over (the gap jumps past it) once the lifted chip's
    /// leading edge clears that neighbour's midpoint by the 2-pt hysteresis — dragging right,
    /// dragging left, and coming back. Instant: no in-between positions.
    #[test]
    fn other_tabs_open_a_gap_that_moves_at_the_neighbours_midpoint() {
        // dragging A rightwards over B
        let mut s = Strip::new(&["A", "B", "C"], 0);
        let (a, b, c) = (s.home("A"), s.home("B"), s.home("C"));
        assert!(near(a.width(), b.width()) && near(b.width(), c.width()), "setup: equal chips");
        let grab = 20.0;
        let from = egui::pos2(a.left() + grab, a.center().y);
        s.begin_drag(from, 1.0);
        // pointer at which A's right edge sits exactly on B's midpoint
        let cross = b.center().x - a.width() + grab;
        let (_, shapes) = s.move_to(egui::pos2(cross + 1.0, from.y));
        assert!(near(painted(&shapes, "B").0, b.left()), "within the 2-pt hysteresis past the midpoint B stays put");
        assert!(near(painted(&shapes, "C").0, c.left()));
        let (_, shapes) = s.move_to(egui::pos2(cross + 3.0, from.y));
        assert!(near(painted(&shapes, "B").0, a.left()), "clear of the midpoint B jumps into A's old slot");
        assert!(near(painted(&shapes, "C").0, c.left()), "C is untouched (the gap is between B and C)");
        // and back: B returns as soon as the edge is back on the near side
        let (_, shapes) = s.move_to(egui::pos2(cross - 3.0, from.y));
        assert!(near(painted(&shapes, "B").0, b.left()), "coming back, B returns to its own slot");

        // dragging C leftwards over B
        let mut s = Strip::new(&["A", "B", "C"], 0);
        let from = egui::pos2(c.left() + grab, c.center().y);
        s.begin_drag(from, -1.0);
        // pointer at which C's left edge sits exactly on B's midpoint
        let cross = b.center().x + grab;
        let (_, shapes) = s.move_to(egui::pos2(cross - 1.0, from.y));
        assert!(near(painted(&shapes, "B").0, b.left()), "within the 2-pt hysteresis past the midpoint B stays put");
        let (_, shapes) = s.move_to(egui::pos2(cross - 3.0, from.y));
        assert!(near(painted(&shapes, "B").0, c.left()), "clear of the midpoint B jumps into C's old slot");
        assert!(near(painted(&shapes, "A").0, a.left()), "A is untouched");
        let (_, shapes) = s.move_to(egui::pos2(cross + 3.0, from.y));
        assert!(near(painted(&shapes, "B").0, b.left()), "coming back, B returns");
    }

    /// P16 (c): the release commits exactly the gap's slot through `ReorderDocument` (the Workspace
    /// still owns the order — the strip only requests the move), and the next frame is at rest.
    #[test]
    fn release_commits_the_gap_slot() {
        let key = egui::Id::new(TAB_DRAG_KEY);
        // A dragged right past B's midpoint → gap between B and C → full slot 2 ([B, A, C])
        let mut s = Strip::new(&["A", "B", "C"], 0);
        let (a, b) = (s.home("A"), s.home("B"));
        let from = egui::pos2(a.left() + 20.0, a.center().y);
        s.begin_drag(from, 1.0);
        let to = egui::pos2(b.center().x - a.width() + 20.0 + 3.0, from.y);
        let _ = s.move_to(to);
        assert!(s.ctx.data(|d| d.get_temp::<TabDrag>(key)).is_some(), "the drag is live");
        let (cmds, _) = s.release(to);
        assert_eq!(cmds, [AppCommand::ReorderDocument(SessionId(1), 2)]);
        assert!(s.ctx.data(|d| d.get_temp::<TabDrag>(key)).is_none(), "released: no drag left");
        let (cmds, shapes) = s.idle();
        assert!(cmds.is_empty() && near(painted(&shapes, "A").0, a.left()), "next frame is at rest");
        let mut ws = crate::workspace::Workspace::new();
        ws.new_untitled();
        ws.new_untitled();
        let ids: Vec<SessionId> = ws.sessions().iter().map(|t| t.id).collect();
        assert!(ws.reorder(ids[0], 2), "slot 2 moves the first tab");
        assert_eq!(ws.sessions().iter().map(|t| t.id).collect::<Vec<_>>(), [ids[1], ids[0], ids[2]]);

        // C dragged left past B's midpoint → gap between A and B → full slot 1
        let mut s = Strip::new(&["A", "B", "C"], 0);
        let c = s.home("C");
        let from = egui::pos2(c.left() + 20.0, c.center().y);
        s.begin_drag(from, -1.0);
        let to = egui::pos2(b.center().x + 20.0 - 3.0, from.y);
        let _ = s.move_to(to);
        assert_eq!(s.release(to).0, [AppCommand::ReorderDocument(SessionId(3), 1)]);

        // lifted but dropped before any midpoint → its own slot (a no-op for `Workspace::reorder`)
        let mut s = Strip::new(&["A", "B", "C"], 0);
        s.begin_drag(egui::pos2(b.left() + 20.0, b.center().y), 1.0);
        let to = egui::pos2(b.left() + 30.0, b.center().y);
        assert_eq!(s.release(to).0, [AppCommand::ReorderDocument(SessionId(2), 1)]);
    }

    fn esc() -> RawInput {
        RawInput {
            screen_rect: Some(screen_rect()),
            events: vec![Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
            ..Default::default()
        }
    }

    /// P16 (d): Esc during the drag, or the window losing focus, cancels — every chip snaps back
    /// to rest in that same frame and the later release commits nothing.
    #[test]
    fn esc_or_focus_loss_cancels_the_drag_and_restores_the_order() {
        for cancel in ["esc", "focus"] {
            let mut s = Strip::new(&["A", "B", "C"], 0);
            let (a, b, c) = (s.home("A"), s.home("B"), s.home("C"));
            let from = egui::pos2(a.left() + 20.0, a.center().y);
            s.begin_drag(from, 1.0);
            let to = egui::pos2(c.center().x, from.y); // well past B: the gap sits after B
            let (_, shapes) = s.move_to(to);
            assert!(near(painted(&shapes, "B").0, a.left()), "{cancel}: setup — B had moved over");
            assert!(s.ctx.dragged_id().is_some(), "{cancel}: setup — egui is dragging");
            let (cmds, shapes) = match cancel {
                "esc" => s.run(esc()),
                // the window really losing focus: winit's `Focused(false)` → egui's `WindowFocused(false)`
                _ => s.run(RawInput {
                    screen_rect: Some(screen_rect()),
                    focused: false,
                    events: vec![Event::WindowFocused(false)],
                    ..Default::default()
                }),
            };
            assert!(cmds.is_empty(), "{cancel}: cancel raises nothing");
            for (label, home) in [("A", a), ("B", b), ("C", c)] {
                assert!(near(painted(&shapes, label).0, home.left()), "{cancel}: {label} is back at rest");
            }
            assert!(!s.ctx.data(|d| d.get_temp::<TabDrag>(egui::Id::new(TAB_DRAG_KEY)).is_some()));
            assert!(s.ctx.dragged_id().is_none(), "{cancel}: egui's drag is stopped too");
            // still holding the button: moving no longer lifts anything, the release commits nothing
            let (_, shapes) = s.move_to(egui::pos2(to.x + 10.0, to.y));
            assert!(near(painted(&shapes, "A").0, a.left()), "{cancel}: nothing re-lifts");
            let (cmds, _) = s.release(egui::pos2(to.x + 10.0, to.y));
            assert!(
                !cmds.iter().any(|c| matches!(c, AppCommand::ReorderDocument(..))),
                "{cancel}: no reorder after a cancel, got {cmds:?}"
            );
        }

        // Codex review: the focus loss arriving in the SAME frame as the threshold crossing — a drag
        // must never start in a frame that contains `WindowFocused(false)`
        let mut s = Strip::new(&["A", "B", "C"], 0);
        let (a, b) = (s.home("A"), s.home("B"));
        let from = egui::pos2(a.left() + 20.0, a.center().y);
        let _ = s.idle();
        let _ = s.press(from);
        let cross = egui::pos2(b.center().x + 10.0, from.y);
        let (_, shapes) = s.run(RawInput {
            screen_rect: Some(screen_rect()),
            focused: false,
            events: vec![Event::PointerMoved(cross), Event::WindowFocused(false)],
            ..Default::default()
        });
        assert!(near(painted(&shapes, "A").0, a.left()), "same-frame focus loss: A never lifts");
        assert!(near(painted(&shapes, "B").0, b.left()), "same-frame focus loss: no gap opens");
        assert!(!s.ctx.data(|d| d.get_temp::<TabDrag>(egui::Id::new(TAB_DRAG_KEY)).is_some()));
        let (_, shapes) = s.move_to(egui::pos2(cross.x + 10.0, cross.y));
        assert!(near(painted(&shapes, "A").0, a.left()), "same-frame focus loss: nothing lifts later");
        let (cmds, _) = s.release(egui::pos2(cross.x + 10.0, cross.y));
        assert!(!cmds.iter().any(|c| matches!(c, AppCommand::ReorderDocument(..))), "got {cmds:?}");
    }

    /// P16 (e): 8 tabs overflowing the strip (the active one displaced into the last drawn slot,
    /// S1 F7): the active chip lifts, the drawn chips reflow around the gap, the release lands in the
    /// FULL order, and while lifted egui owns the pointer — the macOS caption gate
    /// (`mac_caption::caption_drag_allowed`) refuses a window drag, and the published caption
    /// exclusions stay the resting slots (`interactive_rects`, P15).
    #[test]
    fn eight_tab_overflow_lifted_drag_lands_in_the_full_order() {
        let names: Vec<String> = (1..=8).map(|i| format!("Brand guidelines draft {i}")).collect();
        let labels: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut s = Strip::new(&labels, 7);
        let layout = s.layout();
        let drawn: Vec<usize> = layout.tabs.iter().map(|&(i, _)| i).collect();
        assert!(drawn.len() < 8 && drawn.len() >= 3, "setup: overflow, got {drawn:?}");
        assert_eq!(*drawn.last().unwrap(), 7, "setup: the active tab is drawn last");
        let (r0, r1) = (layout.tabs[0].1, layout.tabs[1].1);
        let rl = layout.tabs.last().unwrap().1;
        let from = egui::pos2(rl.left() + 20.0, rl.center().y);
        s.begin_drag(from, -1.0);
        let to = egui::pos2(r1.left() + 20.0, from.y); // the lifted chip sits exactly on chip 1's slot
        let (_, shapes) = s.move_to(to);
        assert!(near(painted(&shapes, labels[7]).0, r1.left()), "the active chip is lifted onto slot 1");
        assert!(near(painted(&shapes, labels[drawn[0]]).0, r0.left()), "chip 0 stays");
        assert!(
            near(painted(&shapes, labels[drawn[1]]).0, r1.left() + rl.width() + crate::chrome::TAB_GAP),
            "chip 1 moves right by the lifted chip's width + gap"
        );
        let strip = layout.tab_strip().unwrap();
        for &(i, _) in &layout.tabs {
            let (left, _, _) = painted(&shapes, labels[i]);
            assert!(left >= strip.min - 0.5, "chip {i} stays inside the strip");
        }
        assert!(s.ctx.dragged_id().is_some(), "while lifted, egui owns the pointer");
        #[cfg(target_os = "macos")]
        assert!(!crate::mac_caption::caption_drag_allowed(true, false, s.ctx.dragged_id().is_some()));
        let (cmds, _) = s.release(to);
        assert_eq!(cmds, [AppCommand::ReorderDocument(SessionId(8), drawn[1])]);
    }

    /// P16 (f): a press + a move smaller than egui's drag threshold + release is a plain activate —
    /// nothing lifts, nothing reorders.
    #[test]
    fn a_sub_threshold_click_only_activates() {
        let mut s = Strip::new(&["A", "B", "C"], 0);
        let b = s.home("B");
        let at = egui::pos2(b.left() + 20.0, b.center().y);
        let _ = s.idle();
        let _ = s.press(at);
        let (_, shapes) = s.move_to(egui::pos2(at.x + 2.0, at.y));
        assert!(near(painted(&shapes, "B").0, b.left()), "under the threshold the chip does not lift");
        assert!(s.ctx.data(|d| d.get_temp::<TabDrag>(egui::Id::new(TAB_DRAG_KEY))).is_none());
        let (cmds, _) = s.release(egui::pos2(at.x + 2.0, at.y));
        assert_eq!(cmds, [AppCommand::ActivateDocument(SessionId(2))]);
    }

    /// Commit `cmds` to a real `Workspace` holding the strip's tabs (ids 1..=n, same order, same
    /// active) and return the tab ids the strip DRAWS afterwards, left → right.
    fn drawn_after(s: &Strip, cmds: &[AppCommand]) -> Vec<SessionId> {
        let mut ws = crate::workspace::Workspace::new();
        for _ in 1..s.tabs.len() {
            ws.new_untitled();
        }
        assert!(ws.activate(s.active.unwrap()));
        for c in cmds {
            if let AppCommand::ReorderDocument(id, slot) = *c {
                ws.reorder(id, slot);
            }
        }
        let label = |id: SessionId| s.tabs.iter().find(|t| t.id == id).unwrap().label.clone();
        let after: Vec<TabView> = ws.sessions().iter().map(|t| tab(t.id.0, &label(t.id), false)).collect();
        let layout = measure(&s.ctx, &after, s.active);
        layout.tabs.iter().map(|&(i, _)| after[i].id).collect()
    }

    /// Codex review of P16 (HIGH): on an overflowing strip (the active tab displaced into the last
    /// drawn slot, hidden tabs between it and the rest) a tab dropped into a VISIBLE gap must land in
    /// that visible neighbour relation and stay visible — never behind the hidden tabs.
    #[test]
    fn overflow_drop_lands_beside_the_visible_neighbours_and_stays_visible() {
        let names: Vec<String> = (1..=8).map(|i| format!("Brand guidelines draft {i}")).collect();
        let labels: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut s = Strip::new(&labels, 7);
        let layout = s.layout();
        let drawn: Vec<usize> = layout.tabs.iter().map(|&(i, _)| i).collect();
        let k = drawn.len();
        assert!((3..8).contains(&k) && drawn[k - 1] == 7, "setup: overflow with the active tab last, got {drawn:?}");
        // tab 1 lifted onto the slot of the last NON-active drawn chip → the gap sits between that
        // chip and the active one
        let (r0, rk) = (layout.tabs[0].1, layout.tabs[k - 2].1);
        let from = egui::pos2(r0.left() + 20.0, r0.center().y);
        s.begin_drag(from, 1.0);
        let to = egui::pos2(rk.left() + 20.0, from.y);
        let _ = s.move_to(to);
        let (cmds, _) = s.release(to);
        let after = drawn_after(&s, &cmds);
        let d = after.iter().position(|&id| id == SessionId(1));
        assert!(d.is_some(), "the dropped tab vanished behind the hidden tabs: drawn after = {after:?}");
        let d = d.unwrap();
        assert_eq!(after[d - 1], SessionId(drawn[k - 2] as u64 + 1), "left neighbour is the one it was dropped after");
        assert_eq!(after[d + 1], SessionId(8), "right neighbour is the active tab it was dropped before");
    }

    /// Codex review of P16 (HIGH): dropping past the LAST visible tab (the displaced active one) of
    /// an overflowing strip keeps the dropped tab visible.
    #[test]
    fn overflow_drop_after_the_last_visible_tab_stays_visible() {
        let names: Vec<String> = (1..=8).map(|i| format!("Brand guidelines draft {i}")).collect();
        let labels: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut s = Strip::new(&labels, 7);
        let r0 = s.layout().tabs[0].1;
        let from = egui::pos2(r0.left() + 20.0, r0.center().y);
        s.begin_drag(from, 1.0);
        let to = egui::pos2(1390.0, from.y); // clamped at the strip's end, past the active tab
        let _ = s.move_to(to);
        let (cmds, _) = s.release(to);
        assert!(matches!(cmds[..], [AppCommand::ReorderDocument(SessionId(1), _)]), "got {cmds:?}");
        let after = drawn_after(&s, &cmds);
        assert!(after.contains(&SessionId(1)), "the dropped tab must stay visible: drawn after = {after:?}");
        assert!(after.contains(&SessionId(8)), "the active tab stays visible (S1 F7)");
    }

    /// P16 owner re-test (2026-09-26, "مش شغال" — the tab did not move at all): egui-winit's
    /// `RawInput::focused` starts `false` and on macOS only changes on a winit `Focused` event, which a
    /// real session may never get (the bundle launched by `open` ran a whole session with `focused ==
    /// false`, and every drag was cancelled on its first lifted frame). A stale "unfocused" flag must
    /// never cancel a drag: an inactive AND the active tab both lift, follow, and commit — 3 and 8 tabs.
    #[test]
    fn dragging_any_tab_works_while_egui_believes_the_window_is_unfocused() {
        let names: Vec<String> = (1..=8).map(|i| format!("Brand guidelines draft {i}")).collect();
        let eight: Vec<&str> = names.iter().map(String::as_str).collect();
        for labels in [vec!["A", "B", "C"], eight] {
            let n = labels.len();
            for dragged in ["inactive", "active"] {
                let mut s = Strip::new(&labels, n - 1);
                s.focused = false;
                let layout = s.layout();
                let (r0, r1) = (layout.tabs[0].1, layout.tabs[1].1);
                let (k, onto) = if dragged == "inactive" { (0, r1) } else { (layout.tabs.len() - 1, r0) };
                let (i, home) = layout.tabs[k];
                let from = egui::pos2(home.left() + 20.0, home.center().y);
                s.begin_drag(from, onto.left() - home.left());
                let to = egui::pos2(onto.left() + 20.0, from.y);
                let (_, shapes) = s.move_to(to);
                let (_, shapes2) = s.move_to(to); // and it is STILL lifted a frame later
                for sh in [&shapes, &shapes2] {
                    let left = painted(sh, labels[i]).0;
                    assert!(
                        near(left, onto.left()),
                        "{n} tabs, {dragged}: the chip must follow the pointer, at {left}"
                    );
                }
                let (cmds, _) = s.release(to);
                let after = drawn_after(&s, &cmds);
                let id = SessionId(i as u64 + 1);
                let want = if dragged == "inactive" { 1 } else { 0 };
                assert_eq!(
                    after.iter().position(|&t| t == id),
                    Some(want),
                    "{n} tabs, {dragged}: {cmds:?} → drawn after {after:?}"
                );
            }
        }
    }

    /// Codex review of P16 (MEDIUM): the tab list changing under a live drag (a tab closed by ⌘W,
    /// a new / opened document, Ctrl+Tab switching the active tab) cancels it — chips back at rest,
    /// nothing committed.
    #[test]
    fn a_workspace_change_mid_drag_cancels_it() {
        for change in ["close", "new", "switch", "reorder"] {
            let mut s = Strip::new(&["A", "B", "C"], 0);
            let (a, b) = (s.home("A"), s.home("B"));
            let from = egui::pos2(a.left() + 20.0, a.center().y);
            s.begin_drag(from, 1.0);
            let to = egui::pos2(b.center().x + 10.0, from.y);
            let (_, shapes) = s.move_to(to);
            assert!(near(painted(&shapes, "B").0, a.left()), "{change}: setup — B had moved over");
            match change {
                "close" => drop(s.tabs.remove(2)),
                "new" => s.tabs.push(tab(4, "D", false)),
                "switch" => s.active = Some(SessionId(2)),
                _ => s.tabs.swap(1, 2),
            }
            let (_, shapes) = s.move_to(egui::pos2(to.x + 1.0, to.y));
            assert!(near(painted(&shapes, "A").0, a.left()), "{change}: A must be back at rest");
            let (cmds, _) = s.release(egui::pos2(to.x + 1.0, to.y));
            assert!(
                !cmds.iter().any(|c| matches!(c, AppCommand::ReorderDocument(..))),
                "{change}: nothing may be committed, got {cmds:?}"
            );
        }
    }

    /// One idle (or hovered) frame of a strip with these tabs; the painted shapes and the layout.
    fn shapes_of(
        tabs: &[TabView],
        active: SessionId,
        hover: Option<Pos2>,
    ) -> (Vec<egui::epaint::ClippedShape>, crate::chrome::TopbarLayout) {
        let ctx = ctx();
        let layout = measure(&ctx, tabs, Some(active));
        let mut shell = varos_app::shell::ShellState::standard();
        let (mut rail, mut dock) = (true, true);
        let mut out = vec![];
        let inputs = match hover {
            Some(p) => vec![idle(), RawInput { events: vec![Event::PointerMoved(p)], ..idle() }, idle()],
            None => vec![idle()],
        };
        for input in inputs {
            let mut win_action = None;
            let mut cmds = Vec::new();
            out = ctx
                .run_ui(input, |root| {
                    build_topbar(
                        root,
                        &icons(),
                        &mut shell,
                        &mut win_action,
                        tabs,
                        Some(active),
                        &mut cmds,
                        &mut rail,
                        &mut dock,
                        &mut Default::default(),
                        None,
                        false,
                        false,
                        false,
                    );
                })
                .shapes;
        }
        (out, layout)
    }
    /// The dirty dots painted inside `rect`: (centre, radius, fill).
    fn dots(shapes: &[egui::epaint::ClippedShape], rect: egui::Rect) -> Vec<(Pos2, f32, Color32)> {
        shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Circle(c) if rect.contains(c.center) && c.radius > 0.0 => {
                    Some((c.center, c.radius, c.fill))
                }
                _ => None,
            })
            .collect()
    }

    /// The dirty dot (`tab_item`, 4b): a 6-px dot on the RIGHT (centre right − 16, the band centre)
    /// only when `TabView::dirty` — TEXT on the active tab, MUTED elsewhere, never azure.
    #[test]
    fn dirty_dot_is_drawn_only_when_dirty() {
        let tabs = vec![tab(1, "A", true), tab(2, "B", true), tab(3, "C", false)];
        let (shapes, layout) = shapes_of(&tabs, SessionId(1), None);
        let rect = |i: usize| layout.tabs.iter().find(|&&(j, _)| j == i).unwrap().1;
        let at = |r: egui::Rect| egui::pos2(r.right() - varos_app::shell::tokens::TAB_MARK_INSET, r.center().y);
        assert_eq!(dots(&shapes, rect(0)), [(at(rect(0)), 3.0, TEXT)], "active + dirty: a TEXT dot");
        assert_eq!(dots(&shapes, rect(1)), [(at(rect(1)), 3.0, MUTED)], "inactive + dirty: a MUTED dot");
        assert!(dots(&shapes, rect(2)).is_empty(), "a clean tab draws no dot");
    }

    /// Hovering a dirty chip turns its dot into the × (the close mark), right where the dot was.
    #[test]
    fn hover_turns_the_dot_into_the_close_mark() {
        let tabs = vec![tab(1, "A", false), tab(2, "B", true)];
        let (rest, layout) = shapes_of(&tabs, SessionId(1), None);
        let b = layout.tabs[1].1;
        assert_eq!(dots(&rest, b).len(), 1, "setup: B shows its dot at rest");
        let (hovered, _) = shapes_of(&tabs, SessionId(1), Some(egui::pos2(b.left() + 20.0, b.center().y)));
        assert!(dots(&hovered, b).is_empty(), "hovered: the dot gives way");
        let x = crate::chrome::tab_close_rect(b);
        let has_mark = |shapes: &[egui::epaint::ClippedShape]| {
            shapes
                .iter()
                .any(|cs| matches!(&cs.shape, egui::Shape::Mesh(m) if m.vertices.iter().all(|v| x.contains(v.pos))))
        };
        assert!(has_mark(&hovered), "hovered: the × is painted in the dot's place");
        assert!(!has_mark(&rest), "at rest an inactive chip shows no ×");
    }

    /// Widths are measured at weight 500 for every tab: a chip never changes width when it becomes
    /// active (only its fill and its name's weight / colour change).
    #[test]
    fn tab_width_ignores_active_state() {
        let tabs = vec![tab(1, "Ramadan campaign", false), tab(2, "Logo marks v3", false)];
        let ctx = ctx();
        let a = measure(&ctx, &tabs, Some(SessionId(1)));
        let b = measure(&ctx, &tabs, Some(SessionId(2)));
        assert_eq!(a.tabs, b.tabs, "the same rects whichever tab is active");
    }

    /// "+N ⌄": its list holds every hidden tab; picking a row activates that tab (the active swap
    /// then draws it in the strip).
    #[test]
    fn overflow_list_row_activates_its_tab() {
        let names: Vec<String> = (1..=12).map(|i| format!("Brand guidelines draft {i}")).collect();
        let tabs: Vec<TabView> = names.iter().enumerate().map(|(i, n)| tab(i as u64 + 1, n, i == 3)).collect();
        let ctx = ctx();
        let active = Some(SessionId(1));
        let layout = measure(&ctx, &tabs, active);
        let ov = layout.overflow.expect("setup: twelve long tabs overflow the 1400-pt band");
        assert!(!layout.hidden.is_empty());
        let mut shell = varos_app::shell::ShellState::standard();
        let (mut rail, mut dock) = (true, true);
        let none = click_at(
            &ctx,
            ov.center(),
            PointerButton::Primary,
            &icons(),
            &mut shell,
            &tabs,
            active,
            &mut rail,
            &mut dock,
        );
        assert!(none.is_empty(), "opening the list raises nothing");
        assert!(kit::menu_open(&ctx), "clicking +N opens its list");
        // the list hangs under "+N", rows `KIT_CONTROL_H` tall after the popup's 4-px pad + 1-px border
        let t = varos_app::shell::tokens::KIT_CONTROL_H;
        let row = |k: f32| {
            egui::pos2(ov.left() + 40.0, ov.bottom() + varos_app::shell::tokens::KIT_MENU_GAP + 5.0 + k * t + t / 2.0)
        };
        let k = 1; // the second hidden tab
        let cmds = {
            let p = row(k as f32);
            let _ = frame(
                &ctx,
                RawInput { events: vec![Event::PointerMoved(p)], ..idle() },
                &icons(),
                &mut shell,
                &tabs,
                active,
                &mut rail,
                &mut dock,
            );
            let _ = frame(
                &ctx,
                press(p, PointerButton::Primary),
                &icons(),
                &mut shell,
                &tabs,
                active,
                &mut rail,
                &mut dock,
            );
            frame(&ctx, release(p, PointerButton::Primary), &icons(), &mut shell, &tabs, active, &mut rail, &mut dock)
        };
        assert_eq!(cmds, [AppCommand::ActivateDocument(tabs[layout.hidden[k]].id)]);
        // the dirty hidden tab carries its dot in the list
        assert_eq!(overflow_row_label(&tabs[3]), "Brand guidelines draft 4  \u{2022}");
        assert_eq!(overflow_row_label(&tabs[4]), "Brand guidelines draft 5");
    }
}

/// DFS S1 / UI audit `docs/audits/ui-2026-09-24/05-defects-and-glue-map.md` finding 1: every burger
/// row and every top-bar button must either DO something observable (raise an `AppCommand`, open a
/// submenu, flip a toggle) or be drawn disabled with a tooltip reason — never silently swallow a
/// click (spec §2's "enabled dead button"). This test drives the exact controls the audit named.
#[cfg(test)]
mod dead_control_tests {
    use super::*;
    use crate::app_command::{AppCommand, SessionId, TabView};
    use egui::{Event, PointerButton, Pos2, RawInput};

    fn icons() -> TopIcons {
        TopIcons { menu: None }
    }

    /// A context with the app's fonts: the band paints Inter (`tokens::small` / `small_medium`).
    fn ctx() -> egui::Context {
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        ctx
    }
    fn one_tab() -> Vec<TabView> {
        vec![TabView { id: SessionId(1), label: "Untitled-1".into(), dirty: false, tooltip: "Not saved yet".into() }]
    }
    /// The top bar's own rect — matches what `Panel::top(..).exact_size(h)` claims inside
    /// `build_topbar`, and what `crate::chrome::topbar_layout` is fed.
    fn bar_rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, crate::chrome::TOPBAR.height))
    }
    /// The FULL window `RawInput.screen_rect`: much taller than the bar, so `menu_below`'s
    /// `Area::constrain(true)` has room to place a dropdown BELOW the bar (see `tab_strip_tests`).
    fn screen_rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0))
    }
    fn press(pos: Pos2, button: PointerButton) -> RawInput {
        RawInput {
            screen_rect: Some(screen_rect()),
            events: vec![
                Event::PointerMoved(pos),
                Event::PointerButton { pos, button, pressed: true, modifiers: Default::default() },
            ],
            ..Default::default()
        }
    }
    fn release(pos: Pos2, button: PointerButton) -> RawInput {
        RawInput {
            screen_rect: Some(screen_rect()),
            events: vec![Event::PointerButton { pos, button, pressed: false, modifiers: Default::default() }],
            ..Default::default()
        }
    }

    /// A tiny state machine around one `Context`, mirroring the fields `Ui::run` threads through
    /// `build_topbar` every frame.
    struct Bar {
        ctx: egui::Context,
        shell: varos_app::shell::ShellState,
        tabs: Vec<TabView>,
        active: Option<SessionId>,
        rail: bool,
        dock: bool,
        /// The window action the last frame raised (the V mark's About).
        win: Option<WinAction>,
        /// The document's snapping flags the burger's View rows edit (`Ui::run` writes them back).
        snap: varos_core::model::SnapConfig,
    }
    impl Bar {
        fn new() -> Self {
            Self {
                ctx: ctx(),
                shell: varos_app::shell::ShellState::standard(),
                tabs: one_tab(),
                active: Some(SessionId(1)),
                rail: true,
                dock: true,
                win: None,
                snap: varos_core::model::SnapConfig::default(),
            }
        }
        fn frame(&mut self, input: RawInput) -> Vec<AppCommand> {
            let mut cmds = Vec::new();
            let win_action = &mut self.win;
            *win_action = None;
            let _ = self.ctx.run_ui(input, |root| {
                build_topbar(
                    root,
                    &icons(),
                    &mut self.shell,
                    win_action,
                    &self.tabs,
                    self.active,
                    &mut cmds,
                    &mut self.rail,
                    &mut self.dock,
                    &mut self.snap,
                    None,
                    false,
                    false,
                    false,
                );
            });
            cmds
        }
        /// A warm-up frame (a control must exist in the PREVIOUS frame to be hit-tested), then
        /// press+release — see `tab_strip_tests::click_at`.
        fn click(&mut self, pos: Pos2) -> Vec<AppCommand> {
            let _ = self.frame(RawInput { screen_rect: Some(screen_rect()), ..Default::default() });
            let _ = self.frame(press(pos, PointerButton::Primary));
            self.frame(release(pos, PointerButton::Primary))
        }
        fn layout(&mut self) -> crate::chrome::TopbarLayout {
            let bar = bar_rect();
            let tabs = self.tabs.clone();
            let active = self.active;
            let mut out = None;
            let _ = self.ctx.run_ui(RawInput { screen_rect: Some(screen_rect()), ..Default::default() }, |ui| {
                let p = ui.painter().clone();
                let text_width = |t: &str| {
                    p.layout_no_wrap(t.to_owned(), varos_app::shell::tokens::small_medium(), Color32::WHITE).size().x
                };
                let widths: Vec<f32> = tabs.iter().map(|t| text_width(&t.label)).collect();
                let active_index = active.and_then(|id| tabs.iter().position(|t| t.id == id));
                out = Some(crate::chrome::topbar_layout(bar, crate::chrome::TOPBAR, None, &widths, active_index));
            });
            out.unwrap()
        }
        /// Open the burger and return the centre of row `k` (0-based) with `seps` separators above it.
        fn burger_row(&mut self, k: usize, seps: usize) -> Pos2 {
            const SEP_H: f32 = 9.0; // menu_sep: add_space(4) + a 1px line + add_space(4)
            let top_left = self.open_burger();
            let y = top_left.y + seps as f32 * SEP_H + k as f32 * MENU_ROW_H + MENU_ROW_H / 2.0;
            egui::pos2(top_left.x + 100.0, y)
        }
        /// Open the burger menu (a press+release on its cell) and return its content's top-left —
        /// the geometry `menu_below`'s flush frame uses: `(menu.left(), bar.bottom() + MENU_PAD_V)`.
        fn open_burger(&mut self) -> Pos2 {
            let menu = self.layout().menu;
            let _ = self.click(menu.center());
            egui::pos2(menu.left(), bar_rect().bottom() + MENU_PAD_V as f32)
        }
    }

    /// New / Open… / Save / Save As… each raise their `AppCommand`; Export… raises the same
    /// `ShowExport` as File ▸ Export ▸ PDF… and the top-bar button (DFS S6). Row geometry:
    /// `MENU_ROW_H` tall, contiguous (`menu_below` zeroes row spacing), then one `menu_sep`
    /// (4 + 1 + 4 px) before the Export row.
    #[test]
    fn burger_rows_either_emit_a_command_or_are_disabled() {
        const SEP_H: f32 = 9.0; // menu_sep: add_space(4) + a 1px line + add_space(4)
        let rows = [
            ("New", Some(AppCommand::NewBoard)),
            ("Open", Some(AppCommand::OpenDialog)),
            ("Save", Some(AppCommand::Save(SessionId(1)))),
            ("Save As", Some(AppCommand::SaveAs(SessionId(1)))),
        ];
        for (i, (name, want)) in rows.iter().enumerate() {
            let mut bar = Bar::new();
            let top_left = bar.open_burger();
            let y = top_left.y + i as f32 * MENU_ROW_H + MENU_ROW_H / 2.0;
            let pos = egui::pos2(top_left.x + 100.0, y);
            let cmds = bar.click(pos);
            assert_eq!(cmds, want.iter().cloned().collect::<Vec<_>>(), "row {name}");
        }
        // Export…, past the separator after the 4 rows above — the one Export command (DFS S6)
        let mut bar = Bar::new();
        let top_left = bar.open_burger();
        let y = top_left.y + 4.0 * MENU_ROW_H + SEP_H + MENU_ROW_H / 2.0;
        let pos = egui::pos2(top_left.x + 100.0, y);
        let cmds = bar.click(pos);
        let menu = crate::host::to_app_command(crate::chrome::FileCmd::Export, Some(SessionId(1)));
        assert_eq!(cmds, vec![AppCommand::ShowExport(SessionId(1))]);
        assert_eq!(cmds.first(), menu.as_ref(), "the burger row = File ▸ Export ▸ PDF…'s command");
    }

    /// Owner 2026-10-06 ("شيل خانة البحث"): the editor band has no Search. A real frame's click where
    /// it sat (the right zone, left of the V mark) raises no command and no window action, and the
    /// bar publishes no rect there — so the OS / macOS caption hit test drags the window from it.
    #[test]
    fn the_editor_band_has_no_search_and_its_old_spot_drags_the_window() {
        let mut bar = Bar::new();
        let layout = bar.layout();
        let old_slot = egui::pos2(layout.brand.left() - 120.0, layout.brand.center().y);
        let cmds = bar.click(old_slot);
        assert!(cmds.is_empty(), "nothing is wired where Search was");
        assert!(bar.win.is_none());
        let chrome = crate::chrome::TOPBAR;
        for ppp in [1.0, 2.0] {
            let excl = crate::chrome::caption_exclusions(&layout.interactive_rects(), ppp);
            let h = (chrome.height * ppp) as i32;
            let at = |p: Pos2| crate::chrome::caption_hit(h, &excl, (p.x * ppp) as i32, (p.y * ppp) as i32);
            assert!(at(old_slot), "the old Search spot drags the window (ppp {ppp})");
            assert!(!at(layout.brand.center()), "the V mark stays a control (ppp {ppp})");
        }
    }

    /// The V mark: on macOS a click asks for the native About panel (the same one Varos ▸ About
    /// opens); elsewhere it is hover-only — no command, no window action, never a dead button.
    #[test]
    fn brand_click_asks_for_about() {
        let mut bar = Bar::new();
        let layout = bar.layout();
        let cmds = bar.click(layout.brand.center());
        assert!(cmds.is_empty(), "V raises no document command");
        if cfg!(target_os = "macos") {
            assert!(matches!(bar.win, Some(WinAction::About)), "V = Varos ▸ About");
        } else {
            assert!(bar.win.is_none(), "V is hover-only off macOS");
        }
    }

    /// 4b moved the Window rows out of the band: the Windows burger (the only menu there) carries them
    /// now — after the File rows and a separator: Tool rail, Control bar, every dockable panel.
    #[test]
    fn burger_window_rows_toggle_the_rail_and_the_panels() {
        // New · Open · Save · Save As | Export · Home | 3 guide rows | 2 snap rows | Tool rail · …
        let mut bar = Bar::new();
        let at = bar.burger_row(11, 4);
        let _ = bar.click(at);
        assert!(!bar.rail, "burger ▸ Tool rail flips the rail");
        let mut bar = Bar::new();
        let at = bar.burger_row(12, 4);
        let _ = bar.click(at);
        assert!(!bar.dock, "burger ▸ Control bar flips the control bar");
        let first = varos_app::shell::PanelId::DOCKABLE[0];
        let mut bar = Bar::new();
        let was = bar.shell.is_open(first);
        let at = bar.burger_row(13, 4);
        let _ = bar.click(at);
        assert_ne!(bar.shell.is_open(first), was, "burger ▸ {} toggles it", first.title());
    }

    #[test]
    fn burger_reset_layout_uses_the_same_command_as_the_native_window_menu() {
        let mut bar = Bar::new();
        let at = bar.burger_row(17, 5);
        let cmds = bar.click(at);
        assert_eq!(cmds, vec![AppCommand::Window(crate::app_command::WindowCmd::ResetLayout)]);
        assert_eq!(
            crate::host::menu_route(crate::chrome::MenuCmd::ResetLayout, None),
            Some(crate::host::MenuRoute::App(cmds[0].clone()))
        );
    }

    /// Windows has no native menu bar: the four snapping controls the magnet held (and Smart Guides)
    /// live in its burger, each a check row on its own flag, flipping exactly that flag.
    #[test]
    fn burger_snapping_rows_flip_their_flags() {
        type Flag = fn(&varos_core::model::SnapConfig) -> bool;
        let rows: [(usize, usize, &str, Flag); 5] = [
            (6, 2, "Smart Guides", |s| s.smart),
            (7, 2, "Alignment Guides", |s| s.alignment_guides),
            (8, 2, "Geometric Guides", |s| s.object_geometry),
            (9, 3, "Snap to Grid", |s| s.grid),
            (10, 3, "Snap to Point", |s| s.key_points),
        ];
        for (k, seps, name, flag) in rows {
            let mut bar = Bar::new();
            let before = bar.snap;
            let at = bar.burger_row(k, seps);
            let _ = bar.click(at);
            assert_ne!(flag(&bar.snap), flag(&before), "burger ▸ {name} flips its flag");
            let others = rows.iter().filter(|r| r.2 != name).all(|r| (r.3)(&bar.snap) == (r.3)(&before));
            assert!(others, "burger ▸ {name} flips ONLY its flag");
        }
    }

    /// End to end: the Smart Guides menu row and the ⌘U / Ctrl+U key produce the same state
    /// transition (the menu row's flags reach the document through `SetSnapConfig`, as `Ui::run`
    /// writes them back; the key runs `apply_key`). On macOS the View row IS the ⌘U key path.
    #[test]
    fn smart_guides_menu_and_shortcut_have_the_same_state_transition() {
        use varos_core::{editor::Editor, geom::View};
        // the menu row, committed exactly as `Ui::run` ends the frame (`apply_frame`)
        let (mut from_menu, mut from_key) = (Editor::new(), Editor::new());
        let mut bar = Bar::new();
        bar.snap = from_menu.doc.snap;
        let at = bar.burger_row(6, 2);
        let _ = bar.click(at);
        super::apply_frame(&mut from_menu, bar.snap, vec![]);
        // …and the key
        crate::apply_key(&mut from_key, &mut View::identity(), [0.0, 0.0], "KeyU", true, false, false);
        assert_ne!(from_key.doc.snap.smart, Editor::new().doc.snap.smart, "setup: the key flipped it");
        assert_eq!(from_menu.doc.snap, from_key.doc.snap, "the row and the key: the same transition");

        // the write-back runs BEFORE the panels' ops: a panel's `ToggleSnapping` in the same frame
        // still wins over the band's snapshot (and the band's change is not lost either)
        let mut ed = Editor::new();
        let before = ed.doc.snap;
        let mut bar = Bar::new();
        bar.snap = before;
        let at = bar.burger_row(6, 2);
        let _ = bar.click(at);
        super::apply_frame(&mut ed, bar.snap, vec![super::Op::ToggleSnapping]);
        assert_eq!(ed.doc.snap.enabled, !before.enabled, "the panel's ToggleSnapping wins");
        assert_eq!(ed.doc.snap.smart, !before.smart, "the burger's Smart Guides change reaches the document");
    }
}

/// 4b (MAC_CHROME.md §A′): the band → the boxes → the status line, on one black backdrop. Headless
/// `Ui::run` composition (the band, the status line, the void underlay, the box tree in
/// `editor_tree_rect`) — no window, no GPU.
#[cfg(test)]
mod band_backdrop_tests {
    use super::*;
    use egui::{Pos2, RawInput};

    /// One frame of the editor's chrome as `Ui::run` stacks it; returns the Board box rect, the
    /// painted shapes, and the panel column the tree published.
    fn editor_frame(size: egui::Vec2) -> (egui::Rect, Vec<egui::epaint::ClippedShape>, Option<egui::Rangef>) {
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        varos_app::shell::tokens::apply(&ctx);
        let mut shell = varos_app::shell::ShellState::standard();
        let (mut rail, mut dock, mut fit) = (true, true, None);
        let mut board = egui::Rect::NOTHING;
        let mut shapes = vec![];
        for k in 0..4 {
            let input = RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, size)),
                time: Some(1.0 + f64::from(k)),
                ..Default::default()
            };
            let (mut win, mut cmds) = (None, vec![]);
            let column = shell.side_column_span();
            let out = ctx.run_ui(input, |root| {
                build_topbar(
                    root,
                    &TopIcons { menu: None },
                    &mut shell,
                    &mut win,
                    &[],
                    None,
                    &mut cmds,
                    &mut rail,
                    &mut dock,
                    &mut Default::default(),
                    column,
                    false,
                    false,
                    true,
                );
                build_statusbar(root, 0, 1, 1.0, &None, &mut fit, "");
                let mid = root.available_rect_before_wrap();
                paint_void_underlay(root.painter(), mid, None);
                let mut host = |panel: varos_app::shell::PanelId, ui: &mut egui::Ui| {
                    if panel == varos_app::shell::PanelId::Board {
                        board = ui.max_rect();
                        corner_voids(ui.painter(), board);
                        return true;
                    }
                    false
                };
                root.scope_builder(egui::UiBuilder::new().max_rect(editor_tree_rect(mid)), |ui| {
                    shell.ui_hosted(ui, &mut host)
                });
            });
            shapes = out.shapes;
        }
        (board, shapes, shell.side_column_span())
    }

    /// The Board box starts exactly at the band's bottom (52), 6 in from the left as before.
    #[test]
    fn editor_tree_starts_at_the_band_bottom() {
        let (board, _, column) = editor_frame(egui::vec2(1512.0, 982.0));
        assert_eq!(crate::chrome::TOPBAR.height, if cfg!(target_os = "macos") { 52.0 } else { 46.0 });
        assert_eq!(board.top(), crate::chrome::TOPBAR.height, "Board box top = band bottom: {board:?}");
        assert_eq!(board.left(), 6.0, "editor side margins stay 6 (the box system is unchanged)");
        let col = column.expect("the standard tree publishes its panel column");
        assert!(col.min > board.right() && (col.max - 1506.0).abs() <= 0.5, "column {col:?}");
    }

    /// One flat backdrop: SEAM is #000, and the band, the status line, the void underlay and the
    /// Board's corner wedges all fill it — no other dark fill anywhere outside the boxes.
    #[test]
    fn void_painters_use_the_one_backdrop() {
        assert_eq!(SEAM, Color32::from_rgb(0, 0, 0), "4b: the backdrop is #000000");
        let (board, shapes, _) = editor_frame(egui::vec2(1512.0, 982.0));
        let band = crate::chrome::TOPBAR.height;
        let fills: Vec<(egui::Rect, Color32)> = shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Rect(r) => Some((r.rect, r.fill)),
                _ => None,
            })
            .collect();
        // the band: a full-width SEAM rect from the window top to 52
        assert!(fills.iter().any(|&(r, c)| c == SEAM && r.top() <= 0.0 && r.bottom() >= band && r.width() >= 1512.0));
        // the status line: a full-width SEAM rect reaching the window bottom
        assert!(fills.iter().any(|&(r, c)| c == SEAM && r.bottom() >= 982.0 && r.width() >= 1512.0 && r.top() > 900.0));
        // the void underlay (no canvas hole here): SEAM over the whole middle
        assert!(fills.iter().any(|&(r, c)| c == SEAM && r.top() <= band && r.bottom() >= board.bottom()));
        // the four corner wedges are SEAM too (no lighter specks at the box corners)
        let wedges: Vec<Color32> = shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Path(p) if p.closed && p.points.iter().any(|q| board.expand(0.5).contains(*q)) => {
                    Some(p.fill)
                }
                _ => None,
            })
            .collect();
        assert_eq!(wedges.len(), 4, "four corner wedges");
        assert!(wedges.iter().all(|&c| c == SEAM), "{wedges:?}");
    }
}

/// Home's band publishes only what it paints: no `+` there, so no dead 28×28 spot that neither acts
/// nor drags the window (review 2026-10-05).
#[cfg(test)]
mod home_band_tests {
    use super::*;
    use crate::app_command::{SessionId, TabView};
    use egui::{Pos2, RawInput};

    fn bar() -> egui::Rect {
        egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(1512.0, crate::chrome::TOPBAR.height))
    }
    fn layout(widths: &[f32], home: bool) -> crate::chrome::TopbarLayout {
        band_layout(crate::chrome::topbar_layout(bar(), crate::chrome::TOPBAR, None, widths, None), home)
    }
    fn drags(l: &crate::chrome::TopbarLayout, p: Pos2) -> bool {
        drags_with(l, crate::chrome::TOPBAR, 2.0, p)
    }
    fn drags_with(l: &crate::chrome::TopbarLayout, chrome: crate::chrome::TopbarChrome, ppp: f32, p: Pos2) -> bool {
        let excl = crate::chrome::caption_exclusions(&l.interactive_rects(), ppp);
        crate::chrome::caption_hit((chrome.height * ppp) as i32, &excl, (p.x * ppp) as i32, (p.y * ppp) as i32)
    }

    #[test]
    fn home_publishes_no_plus_and_its_spot_drags_the_window() {
        for widths in [vec![], vec![80.0, 120.0, 95.0]] {
            let n = widths.len();
            let editor = layout(&widths, false);
            let plus = editor.plus.expect("the editor places +");
            assert!(!drags(&editor, plus.center()), "{n} tabs: the editor's + is a control");
            let home = layout(&widths, true);
            assert!(home.plus.is_none(), "{n} tabs: Home has no +");
            assert!(drags(&home, plus.center()), "{n} tabs: where + would be is empty band on Home");
            for r in [home.menu, home.brand].into_iter().chain(home.tabs.iter().map(|&(_, r)| r)) {
                assert!(!drags(&home, r.center()), "{n} tabs: painted control {r:?} never drags");
            }
        }
    }

    /// Home's chip is a 28×28 square; on Windows the menu CELL around it is 36 × 46. Only the chip is
    /// published: its whole area (edges included) never drags, the strips around it in the old cell do.
    #[test]
    fn home_publishes_the_chip_not_the_menu_cell() {
        use varos_app::shell::tokens as t;
        for chrome in [crate::chrome::topbar_chrome(true), crate::chrome::topbar_chrome(false)] {
            let bar = egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(1512.0, chrome.height));
            for widths in [vec![], vec![80.0, 120.0]] {
                let cell = crate::chrome::topbar_layout(bar, chrome, None, &widths, None).menu;
                let home = band_layout(crate::chrome::topbar_layout(bar, chrome, None, &widths, None), true);
                let chip = home.menu;
                assert_eq!(chip.size(), egui::Vec2::splat(t::BAND_CHIP_H), "the published Home rect is the chip");
                assert_eq!(chip.center(), cell.center(), "centred in its cell");
                for ppp in [1.0, 2.0] {
                    let i = chip.shrink(0.5);
                    for p in [chip.center(), i.left_top(), i.right_top(), i.left_bottom(), i.right_bottom()] {
                        assert!(!drags_with(&home, chrome, ppp, p), "chip at {p:?} (ppp {ppp}, h {})", chrome.height);
                    }
                    // the strips: above / below the chip (always), left / right of it inside the old cell
                    let mut strips = vec![
                        egui::pos2(chip.center().x, bar.top() + 1.0),
                        egui::pos2(chip.center().x, chip.top() - 2.0),
                        egui::pos2(chip.center().x, chip.bottom() + 2.0),
                        egui::pos2(chip.center().x, bar.bottom() - 1.0),
                    ];
                    if cell.width() > chip.width() + 2.0 {
                        strips.push(egui::pos2(chip.left() - 2.0, chip.center().y));
                        strips.push(egui::pos2(chip.right() + 2.0, chip.center().y));
                    }
                    for p in strips {
                        assert!(bar.top() <= p.y && p.y < bar.bottom(), "setup: {p:?} is in the band");
                        assert!(
                            drags_with(&home, chrome, ppp, p),
                            "strip at {p:?} drags (ppp {ppp}, h {})",
                            chrome.height
                        );
                    }
                }
            }
        }
    }

    /// The real Home frame paints nothing where `+` would be (zero tabs and some tabs).
    #[test]
    fn home_frame_paints_nothing_at_the_plus_spot() {
        for n in [0u64, 3] {
            let ctx = egui::Context::default();
            varos_app::shell::fonts::install(&ctx);
            let tabs: Vec<TabView> = (1..=n)
                .map(|i| TabView {
                    id: SessionId(i),
                    label: format!("Board {i}"),
                    dirty: false,
                    tooltip: String::new(),
                })
                .collect();
            let mut shell = varos_app::shell::ShellState::standard();
            let (mut rail, mut dock) = (true, true);
            let (mut shapes, mut widths) = (vec![], vec![]);
            for _ in 0..2 {
                let screen = egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(1512.0, 982.0));
                let input = RawInput { screen_rect: Some(screen), ..Default::default() };
                let (mut win, mut cmds) = (None, vec![]);
                let out = ctx.run_ui(input, |root| {
                    let p = root.painter().clone();
                    let font = varos_app::shell::tokens::small_medium();
                    widths =
                        tabs.iter().map(|t| p.layout_no_wrap(t.label.clone(), font.clone(), TEXT).size().x).collect();
                    build_topbar(
                        root,
                        &TopIcons { menu: None },
                        &mut shell,
                        &mut win,
                        &tabs,
                        None,
                        &mut cmds,
                        &mut rail,
                        &mut dock,
                        &mut Default::default(),
                        None,
                        false,
                        true,
                        true,
                    );
                });
                shapes = out.shapes;
            }
            let plus = layout(&widths, false).plus.unwrap();
            let painted_there = shapes.iter().any(|cs| match &cs.shape {
                egui::Shape::Mesh(m) => !m.vertices.is_empty() && m.vertices.iter().all(|v| plus.contains(v.pos)),
                egui::Shape::Rect(r) => plus.contains_rect(r.rect),
                _ => false,
            });
            assert!(!painted_there, "{n} tabs: Home paints nothing at the + spot {plus:?}");
        }
    }
}

/// PAINS_LOG P16 (owner 2026-09-25): "the Pathfinder buttons do nothing". Two overlapping shapes drawn
/// with the real Rectangle gestures, selected, then each boolean button CLICKED — in the real box tree
/// (`ShellState::standard`) hosting the real panel bodies exactly as `Ui::run` does, the frame's ops
/// applied with `apply_ops` exactly as `Ui::run` does. No window, no GPU.
#[cfg(test)]
pub(super) mod pathfinder_click_tests {
    use super::{apply_ops, panel_pathfinder, panel_properties, DockIcons, Op, Snap};
    use egui::{Event, Modifiers, PointerButton, Pos2, RawInput};
    use std::cell::RefCell;
    use varos_app::shell::{PanelId, ShellState};
    use varos_core::boolean::BoolOp;

    #[test]
    fn disabled_pathfinder_ink_differs_from_enabled_rest() {
        assert_eq!(super::pf_btn_ink(true, false), super::DISABLED);
        assert_eq!(super::pf_btn_ink(false, false), super::MUTED);
        assert_ne!(super::pf_btn_ink(true, false), super::pf_btn_ink(false, false));
    }
    use varos_core::editor::{Editor, ToolKind};

    thread_local! {
        /// Where `pf_btn` put each boolean button in the last frame, and its disabled reason (test probe).
        pub(crate) static PF_RECTS: RefCell<Vec<(BoolOp, egui::Rect, Option<String>)>> = const { RefCell::new(vec![]) };
    }

    const OPS: [(BoolOp, &str); 4] = [
        (BoolOp::Unite, "Unite"),
        (BoolOp::MinusFront, "Minus Front"),
        (BoolOp::Intersect, "Intersect"),
        (BoolOp::Exclude, "Exclude"),
    ];

    fn same(a: BoolOp, b: BoolOp) -> bool {
        std::mem::discriminant(&a) == std::mem::discriminant(&b)
    }

    /// Two overlapping 100×100 rectangles drawn with the Rectangle tool, then Selection tool + Select All.
    fn two_selected() -> Editor {
        let mut ed = Editor::new();
        ed.ppu = 1.0;
        for (a, b) in [([100.0, 100.0], [200.0, 200.0]), ([150.0, 150.0], [250.0, 250.0])] {
            ed.set_tool(ToolKind::Rect);
            ed.pointer_down(a);
            ed.pointer_move(b);
            ed.pointer_up();
        }
        ed.set_tool(ToolKind::Object);
        ed.select_all();
        assert_eq!(ed.doc.paths.len(), 2, "premise: two shapes");
        assert_eq!(ed.objsel.len(), 2, "premise: both selected");
        ed
    }

    /// The right column of the real app: the standard box tree with the Properties + Pathfinder bodies
    /// hosted exactly as `Ui::run` hosts them.
    struct App {
        ctx: egui::Context,
        shell: ShellState,
        t: f64,
    }
    impl App {
        fn new(front: PanelId) -> Self {
            let mut shell = ShellState::standard();
            if front == PanelId::Pathfinder {
                shell.toggle_panel(PanelId::Pathfinder); // buried behind Align → surfaced (Window menu)
            }
            let ctx = egui::Context::default();
            varos_app::shell::fonts::install(&ctx);
            App { ctx, shell, t: 1.0 }
        }
        /// One `Ui::run`-shaped frame: lay out, collect ops, `apply_ops`.
        fn frame(&mut self, ed: &mut Editor, events: Vec<Event>) {
            self.t += 1.0 / 60.0;
            PF_RECTS.with(|r| r.borrow_mut().clear());
            let input = RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(1400.0, 900.0))),
                time: Some(self.t),
                events,
                ..Default::default()
            };
            let snap = Snap::read(ed);
            let none = None;
            let align = [None, None, None, None, None, None, None, None];
            let icons = DockIcons { rotate: &none, opacity: &none, strokew: &none, align: &align };
            let (mut refpt, mut lock) = ((0.0, 0.0), false);
            let mut ops: Vec<Op> = vec![];
            let shell = &mut self.shell;
            let _ = self.ctx.run_ui(input, |root| {
                let mut host = |panel: PanelId, ui: &mut egui::Ui| -> bool {
                    match panel {
                        PanelId::Board => true,
                        PanelId::Properties => {
                            panel_properties(
                                ui,
                                &snap,
                                &icons,
                                &mut refpt,
                                &mut lock,
                                &mut ops,
                                (&Default::default(), &mut Vec::new()),
                            );
                            true
                        }
                        PanelId::Pathfinder => {
                            panel_pathfinder(ui, snap.pathfinder, &mut ops);
                            true
                        }
                        _ => false,
                    }
                };
                shell.ui_hosted(root, &mut host);
            });
            apply_ops(ed, ops);
        }
        fn button_at(&mut self, ed: &mut Editor, op: BoolOp) -> Pos2 {
            self.frame(ed, vec![]);
            let rects = PF_RECTS.with(|r| r.borrow().clone());
            rects.iter().find(|(o, ..)| same(*o, op)).map(|(_, r, _)| r.center()).expect("the button is drawn")
        }
        /// A mouse click (press and release in separate frames), or a trackpad tap (`tap`: press and
        /// release arrive in ONE frame's events, as a fast macOS tap-to-click delivers them).
        fn click(&mut self, ed: &mut Editor, p: Pos2, tap: bool) {
            let btn = |pressed| Event::PointerButton {
                pos: p,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            };
            self.frame(ed, vec![Event::PointerMoved(p)]);
            if tap {
                self.frame(ed, vec![btn(true), btn(false)]);
            } else {
                self.frame(ed, vec![btn(true)]);
                self.frame(ed, vec![btn(false)]);
            }
            self.frame(ed, vec![]);
        }
    }

    fn click_each_op(front: PanelId, tap: bool) {
        for (op, name) in OPS {
            let mut ed = two_selected();
            let rev0 = ed.rev;
            let mut app = App::new(front);
            let at = app.button_at(&mut ed, op);
            app.click(&mut ed, at, tap);
            // Unite: one outline · Minus Front: one L · Intersect: the overlap · Exclude: two L pieces
            let want = if same(op, BoolOp::Exclude) { 2 } else { 1 };
            assert_eq!(ed.doc.paths.len(), want, "{front:?} ▸ {name}: the two shapes were not combined");
            assert_eq!(ed.rev, rev0 + 1, "{front:?} ▸ {name}: exactly one committed edit");
            ed.undo();
            assert_eq!(ed.doc.paths.len(), 2, "{front:?} ▸ {name}: ONE undo brings both shapes back");
        }
    }

    #[test]
    fn pathfinder_panel_buttons_combine_the_selection() {
        click_each_op(PanelId::Pathfinder, false);
        click_each_op(PanelId::Pathfinder, true);
    }

    #[test]
    fn properties_shape_row_buttons_combine_the_selection() {
        click_each_op(PanelId::Properties, false);
        click_each_op(PanelId::Properties, true);
    }

    /// `Editor::pathfinder_enabled` drives both homes: fewer than two closed shapes → every button is
    /// drawn disabled with the core's reason, and a click emits nothing; two → enabled, no reason.
    #[test]
    fn buttons_are_disabled_with_the_reason_below_two_closed_shapes() {
        for front in [PanelId::Pathfinder, PanelId::Properties] {
            let mut ed = two_selected();
            let reason = {
                let keep = ed.doc.paths[0].id;
                ed.objsel.retain(|p| *p == keep);
                ed.pathfinder_enabled().expect_err("premise: one shape is not enough")
            };
            let mut app = App::new(front);
            app.frame(&mut ed, vec![]);
            let rects = PF_RECTS.with(|r| r.borrow().clone());
            assert!(!rects.is_empty() && rects.len().is_multiple_of(4), "{front:?}: all four buttons are drawn");
            assert!(rects.iter().all(|(.., why)| why.as_deref() == Some(reason)), "{front:?}: disabled + reason");
            let (rev, at) = (ed.rev, rects[0].1.center());
            app.click(&mut ed, at, false);
            assert_eq!((ed.rev, ed.doc.paths.len()), (rev, 2), "{front:?}: a disabled button does nothing");
            // two closed shapes: enabled, no reason
            ed.select_all();
            app.frame(&mut ed, vec![]);
            let rects = PF_RECTS.with(|r| r.borrow().clone());
            assert!(rects.iter().all(|(.., why)| why.is_none()), "{front:?}: enabled");
        }
    }
}

/// P16 owner re-test / Codex review: egui-winit starts `RawInput::focused` at `false` and on macOS only
/// updates it on a winit `Focused` event, which a bundle launched via `open` never received — so egui
/// believed the window unfocused for the whole session. `Response::has_focus()` reads that flag, so
/// text fields dropped their typed buffers and hid their carets. The host seeds the flag from winit's
/// `window.has_focus()` every frame (`Ui::run` → `egui_focus_seed`); winit's real `Focused(false)`
/// path stays untouched.
#[cfg(test)]
mod window_focus_tests {
    use super::*;
    use egui::{Event, RawInput};

    #[test]
    fn the_seed_raises_focus_for_a_key_window_and_never_lowers_it() {
        // (window is key per winit, egui-winit's flag) → the flag egui runs the frame with
        assert!(egui_focus_seed(true, false), "key window, flag never set (bundle launch): seeded true");
        assert!(egui_focus_seed(true, true));
        assert!(!egui_focus_seed(false, false), "a window that is not key stays unfocused");
        assert!(egui_focus_seed(false, true), "never lowers: losing focus stays winit's Focused(false) path");
    }

    /// The artboard-name field (`fields::name`) on a key window whose egui-winit flag never became
    /// true: with the host seed the typed text survives across frames and is committed on blur.
    #[test]
    fn a_text_field_keeps_its_typed_buffer_once_the_host_seeds_window_focus() {
        let ctx = egui::Context::default();
        // the raw input egui-winit hands over on a macOS bundle launch — focused never set — after the
        // host's seed for a window that IS key
        let raw = |events: Vec<Event>| {
            let mut r = RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0))),
                focused: false,
                events,
                ..Default::default()
            };
            r.focused = egui_focus_seed(true, r.focused);
            r
        };
        let frame = |events: Vec<Event>, focus: bool| {
            let mut ops = vec![];
            let _ = ctx.run_ui(raw(events), |ui| {
                if focus {
                    let id = doc_id(ui, ("abname", kit::field::home(ui), "t"));
                    ui.memory_mut(|m| m.request_focus(id));
                }
                fields::name(ui, 160.0, "Artboard", "t", &mut ops, |v| Op::AbName(0, v));
            });
            ops.into_iter().find_map(|op| match op {
                Op::Field(op) => match *op {
                    Op::AbName(_, v) => Some(v),
                    _ => None,
                },
                _ => None,
            })
        };
        let enter = Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        };
        assert_eq!(frame(vec![], false), None);
        assert_eq!(frame(vec![], true), None, "focus the field");
        assert_eq!(frame(vec![Event::Text("X".into())], false), None, "type");
        assert_eq!(frame(vec![], false), None, "a frame later the buffer must still hold the X");
        let committed = frame(vec![enter], false).expect("Enter commits the buffer");
        assert!(committed.contains('X') && committed.len() == "Artboard".len() + 1, "committed {committed:?}");
    }
}

#[cfg(test)]
mod recovery_strip_tests {
    use super::*;
    use varos_app::recovery_card::{ids, ReviewRow};

    fn context(ppp: f32) -> egui::Context {
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        varos_app::shell::tokens::apply(&ctx);
        let _ = ctx.run_ui(input(ppp, vec![]), |_| {});
        ctx
    }
    fn input(ppp: f32, events: Vec<egui::Event>) -> egui::RawInput {
        let mut input = egui::RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1512.0, 982.0))),
            ..Default::default()
        };
        input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().native_pixels_per_point = Some(ppp);
        input
    }
    /// Hover, press, release — one frame each (the pointer reaches a control before it clicks).
    fn click(pos: egui::Pos2) -> [Vec<egui::Event>; 3] {
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        [vec![egui::Event::PointerMoved(pos)], vec![button(true)], vec![button(false)]]
    }

    #[test]
    fn restored_notice_floats_saves_defers_per_tab_and_yields_to_review() {
        for ppp in [1.0, 2.0] {
            let ctx = context(ppp);
            let board = egui::Rect::from_min_max(egui::pos2(12.0, 52.0), egui::pos2(1200.0, 950.0));
            let restored = crate::recovery_host::RecoveryUi {
                sid: Some(SessionId(42)),
                recovered_notice: Some("Restored copy of Menu card — save it to keep it".into()),
                ..Default::default()
            };
            let frame = |recovery: &crate::recovery_host::RecoveryUi, events| {
                let mut cmds = vec![];
                let _ = ctx.run_ui(input(ppp, events), |ui| {
                    let before = ui.available_rect_before_wrap();
                    build_recovery_card(ui.ctx(), board, recovery, &mut cmds);
                    assert_eq!(before, ui.available_rect_before_wrap(), "floating notice consumes no root space");
                });
                cmds
            };
            let press = |recovery: &crate::recovery_host::RecoveryUi, id| {
                let pos = ctx.read_response(id).unwrap().rect.center();
                let mut cmds = vec![];
                for events in click(pos) {
                    cmds.extend(frame(recovery, events));
                }
                cmds
            };
            let both = crate::recovery_host::RecoveryUi { banner: true, rows: rows(), ..restored.clone() };
            frame(&both, vec![]);
            frame(&both, vec![]);
            assert!(ctx.read_response(ids::review()).is_some());
            assert!(ctx.read_response(ids::save_as()).is_none(), "review rows take priority");
            frame(&restored, vec![]);
            frame(&restored, vec![]);
            assert!(ctx.read_response(ids::review()).is_none());
            assert_eq!(press(&restored, ids::save_as()), [AppCommand::SaveAs(SessionId(42))]);
            assert!(press(&restored, ids::later()).is_empty());
            frame(&restored, vec![]);
            frame(&restored, vec![]);
            assert!(ctx.read_response(ids::card()).is_none(), "Later hides this tab's notice");
            let other = crate::recovery_host::RecoveryUi { sid: Some(SessionId(43)), ..restored.clone() };
            frame(&other, vec![]);
            frame(&other, vec![]);
            assert!(ctx.read_response(ids::save_as()).is_some(), "another tab retains its notice");
        }
    }

    fn rows() -> Vec<ReviewRow> {
        ["menu", "untitled"]
            .iter()
            .map(|rid| ReviewRow {
                rid: (*rid).into(),
                name: format!("Copy {rid}"),
                when: "never saved".into(),
                ..Default::default()
            })
            .collect()
    }

    /// The card raises the commands Start's Recovered band raises (one adapter): Later = DeferRecovery,
    /// Restore = Recover(rid) (the same open command), Discard = DiscardRecovery(rid) (the host confirms);
    /// Review itself raises nothing — it opens the panel in place, it never goes Home.
    #[test]
    fn recovery_card_commands_are_starts_commands() {
        let ctx = context(2.0);
        let board = egui::Rect::from_min_max(egui::pos2(12.0, 52.0), egui::pos2(1200.0, 950.0));
        let recovery = crate::recovery_host::RecoveryUi {
            banner: true,
            rows: rows(),
            footer: "Recovery on · copies every 30 seconds".into(),
            ..Default::default()
        };
        let frame = |events| {
            let mut cmds = Vec::new();
            let _ = ctx.run_ui(input(2.0, events), |ui| build_recovery_card(ui.ctx(), board, &recovery, &mut cmds));
            cmds
        };
        let press = |id: egui::Id| {
            let pos = ctx.read_response(id).unwrap_or_else(|| panic!("{id:?} not drawn")).rect.center();
            let [hover, press, release] = click(pos);
            assert!(frame(hover).is_empty() && frame(press).is_empty());
            frame(release)
        };
        frame(vec![]);
        assert_eq!(press(ids::review()), [], "Review never sends Home");
        frame(vec![]);
        assert!(ctx.read_response(ids::panel()).is_some(), "the panel is open in place");
        assert_eq!(press(ids::restore("menu")), [AppCommand::Recover("menu".into())]);
        assert_eq!(press(ids::discard("untitled")), [AppCommand::DiscardRecovery("untitled".into())]);
        assert_eq!(press(ids::close()), [AppCommand::DeferRecovery], "× = Later");
        frame(vec![]);
        assert_eq!(press(ids::later()), [AppCommand::DeferRecovery]);
        // after Later the host clears `banner`: nothing is drawn and nothing blocks the canvas
        let hidden = crate::recovery_host::RecoveryUi { banner: false, ..recovery.clone() };
        for _ in 0..2 {
            let _ = ctx.run_ui(input(2.0, vec![]), |ui| build_recovery_card(ui.ctx(), board, &hidden, &mut vec![]));
        }
        let probe = egui::pos2(board.center().x, board.bottom() - 40.0);
        let floating = ctx.layer_id_at(probe).is_some_and(|l| l.order != egui::Order::Background);
        assert!(!floating, "no card layer is left over the canvas (the pointer test gives it back)");
    }
}

/// Icon stage 1 (ICON_LIBRARY_STUDY §4, owner: "icons instead of text"): every converted panel button
/// still emits exactly what its text predecessor emitted — by pointer AND by keyboard (focus + Enter) —
/// every icon action carries a tooltip with its old label, and no file that draws icons types a raw
/// icon size. CPU-only: bare egui contexts, no window, no GPU.
#[cfg(test)]
pub(super) mod icon_action_tests {
    use super::{
        build_color_modal, panel_artboard, panel_layers, panel_properties, AbSnap, Chan, ColorModal, DockIcons,
        Harmony, IconAction, LayerIcons, MTab, MTarget, Op, Snap, ICON_ACTIONS,
    };
    use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput};
    use std::cell::RefCell;
    use varos_core::editor::{Editor, PaintTarget, ToolKind};

    thread_local! {
        /// Where `IconAction::show` drew each action in the last frame: (key, widget id, rect).
        pub(crate) static PROBE: RefCell<Vec<(&'static str, egui::Id, egui::Rect)>> = const { RefCell::new(vec![]) };
    }

    /// The emissions the tests compare (`Op` is neither `Clone` nor `Debug`). Per-frame bookkeeping ops
    /// such as the picker's live preview are not button emissions and are skipped.
    fn describe(op: &Op) -> Option<String> {
        Some(match op {
            Op::LayerGroup => "LayerGroup".into(),
            Op::LayerDeleteSel => "LayerDeleteSel".into(),
            Op::AbAdd => "AbAdd".into(),
            Op::AbDup(i) => format!("AbDup({i})"),
            Op::AbDel(i) => format!("AbDel({i})"),
            Op::AbOrient(i) => format!("AbOrient({i})"),
            Op::Flip(h) => format!("Flip({h})"),
            Op::Paint(PaintTarget::Fill, None) => "Paint(Fill, None)".into(),
            Op::Paint(PaintTarget::Stroke, None) => "Paint(Stroke, None)".into(),
            Op::PickerCancel => "PickerCancel".into(),
            Op::AbColor(_, Some(_)) => "Opaque".into(),
            Op::AbClip(i) => format!("AbClip({i})"),
            Op::AbMoveArt(value) => format!("AbMoveArt({value})"),
            Op::AbActive(i) => format!("AbActive({i})"),
            Op::Tool(ToolKind::Artboard) => "EditArtboards".into(),
            Op::FitArtboard(i) => format!("FitArtboard({i})"),
            Op::SetClipExempt(value) => format!("ClipExempt({value})"),
            Op::ToggleSnapping => "ToggleSnapping".into(),
            Op::ToggleGuides => "ToggleGuides".into(),
            Op::ToggleRulers => "ToggleRulers".into(),
            Op::ToggleGuidesLock => "ToggleGuidesLock".into(),
            Op::ToggleSmartGuides => "ToggleSmartGuides".into(),
            Op::ToggleSnapPoint => "ToggleSnapPoint".into(),
            Op::ToggleSnapGrid => "ToggleSnapGrid".into(),
            _ => return None,
        })
    }

    /// Which panel a case draws, with the state the panel keeps between frames.
    #[derive(Clone, Copy)]
    enum Scene {
        Layers,
        /// The artboard inspector with `count` artboards; `portrait` picks the page shape.
        Artboard {
            count: usize,
            portrait: bool,
        },
        Properties,
        Document,
        Picker,
    }

    struct Rig {
        ctx: egui::Context,
        t: f64,
        scene: Scene,
        ed: Editor,
        lock: bool,
        fit: Option<usize>,
        modal: Option<ColorModal>,
    }

    impl Rig {
        fn new(scene: Scene) -> Self {
            let mut ed = Editor::new();
            ed.ppu = 1.0;
            ed.set_tool(ToolKind::Rect);
            ed.pointer_down([100.0, 100.0]);
            ed.pointer_move([200.0, 260.0]);
            ed.pointer_up();
            ed.set_tool(ToolKind::Object);
            ed.select_all();
            ed.execute(varos_core::command::EditCommand::AddArtboard);
            ed.doc.artboards[0].clip = true;
            let modal = Some(ColorModal {
                target: MTarget::Paint(PaintTarget::Fill),
                orig: None,
                hsva: [0.0, 0.0, 1.0, 1.0],
                chan: Chan::H,
                tab: MTab::Picker,
                harmony: Harmony::None,
                eyedropping: false,
                eyedrop_prev_down: false,
                eyedrop_return: [0.0, 0.0, 1.0, 1.0],
            });
            let ctx = egui::Context::default();
            varos_app::shell::fonts::install(&ctx);
            let mut rig = Rig { ctx, t: 1.0, scene, ed, lock: false, fit: None, modal };
            rig.frame(vec![]); // egui hit-tests against the previous pass: lay out once
            rig
        }

        /// One frame; returns the described emissions (ops + the panel-state flips the old buttons made).
        fn frame(&mut self, events: Vec<Event>) -> Vec<String> {
            self.t += 1.0 / 60.0;
            PROBE.with(|p| p.borrow_mut().clear());
            let input = RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(1200.0, 1600.0))),
                time: Some(self.t),
                events,
                ..Default::default()
            };
            let mut ops: Vec<Op> = vec![];
            let (lock0, fit0) = (self.lock, self.fit);
            let snap = Snap::read(&self.ed);
            let Rig { ctx, scene, lock, fit, modal, ed, .. } = self;
            let _ = ctx.run_ui(input, |ui| match *scene {
                Scene::Layers => {
                    let ic = LayerIcons { eye: None, eye_off: None, lock: None, unlock: None, search: None };
                    let (mut search, mut rename, mut collapsed, mut drag, mut anchor) =
                        (String::new(), None, Default::default(), None, None);
                    panel_layers(
                        ui,
                        &[],
                        &ic,
                        &mut search,
                        &mut rename,
                        &mut collapsed,
                        &mut drag,
                        &mut anchor,
                        &mut ops,
                    );
                }
                Scene::Artboard { count, portrait } => {
                    let (w, h) = if portrait { (595.0, 842.0) } else { (842.0, 595.0) };
                    let s = AbSnap {
                        count,
                        active: 0,
                        name: "Artboard 1".into(),
                        x: 0.0,
                        y: 0.0,
                        w,
                        h,
                        color: None,
                        clip: true,
                        move_art: true,
                    };
                    panel_artboard(ui, &s, lock, &mut ops, fit);
                }
                Scene::Properties => {
                    let none = None;
                    let align = [None, None, None, None, None, None, None, None];
                    let ic = DockIcons { rotate: &none, opacity: &none, strokew: &none, align: &align };
                    let mut refpt = (0.0, 0.0);
                    panel_properties(ui, &snap, &ic, &mut refpt, lock, &mut ops, (&Default::default(), &mut vec![]));
                }
                Scene::Document => {
                    let mut snapshot = Snap::read(ed);
                    snapshot.artboards = 2;
                    snapshot.active_artboard = 0;
                    snapshot.artboard_names = vec!["First".into(), "Second".into()];
                    super::document_section(ui, &snapshot, 264.0, &mut ops, (&Default::default(), &mut vec![]));
                }
                Scene::Picker => build_color_modal(ui.ctx(), modal, &snap, &None, &mut ops),
            });
            let mut out: Vec<String> = ops.iter().filter_map(describe).collect();
            if self.lock != lock0 {
                out.push(format!("lock={}", self.lock));
            }
            if self.fit != fit0 {
                out.push(format!("fit={:?}", self.fit));
            }
            out
        }

        fn probe(&self, key: &str) -> (egui::Id, egui::Rect) {
            PROBE
                .with(|p| p.borrow().iter().find(|(k, _, _)| *k == key).map(|(_, id, r)| (*id, *r)))
                .unwrap_or_else(|| panic!("icon action {key} was not drawn"))
        }

        fn click(&mut self, key: &str) -> Vec<String> {
            let (_, rect) = self.probe(key);
            let p = rect.center();
            let btn = |pressed| Event::PointerButton {
                pos: p,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            };
            let mut out = self.frame(vec![Event::PointerMoved(p)]);
            out.extend(self.frame(vec![btn(true)]));
            out.extend(self.frame(vec![btn(false)]));
            out.extend(self.frame(vec![]));
            out
        }

        fn enter(&mut self, key: &str) -> Vec<String> {
            let (id, _) = self.probe(key);
            self.ctx.memory_mut(|m| m.request_focus(id));
            let mut out = self.frame(vec![]);
            out.extend(self.frame(vec![Event::Key {
                key: Key::Enter,
                physical_key: Some(Key::Enter),
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }]));
            out.extend(self.frame(vec![]));
            out
        }
    }

    /// (action, scene, what the old TEXT/glyph button emitted). The right column is read from the
    /// pre-stage-1 code: "+ Add" → AbAdd, "Duplicate" → AbDup(i), "Delete" → AbDel(i) (inert with one
    /// artboard), "×" → Paint(target, None) / the picker's cancel, the footer → LayerGroup/LayerDeleteSel…
    fn table() -> Vec<(IconAction, Scene, Vec<&'static str>)> {
        use super::*;
        let two = Scene::Artboard { count: 2, portrait: true };
        vec![
            (IA_LAYER_GROUP, Scene::Layers, vec!["LayerGroup"]),
            (IA_LAYER_FILTER, Scene::Layers, vec![]),
            (IA_LAYER_DELETE, Scene::Layers, vec!["LayerDeleteSel"]),
            (IA_AB_ADD, two, vec!["AbAdd"]),
            (IA_AB_DUP, two, vec!["AbDup(0)"]),
            // Start v2: a New board has zero artboards — Duplicate is disabled, never a silent no-op
            (IA_AB_DUP, Scene::Artboard { count: 0, portrait: true }, vec![]),
            (IA_AB_DEL, two, vec!["AbDel(0)"]),
            (IA_AB_DEL, Scene::Artboard { count: 1, portrait: true }, vec![]),
            (IA_AB_LINK, two, vec!["lock=true"]),
            (IA_AB_PORTRAIT, Scene::Artboard { count: 2, portrait: false }, vec!["AbOrient(0)"]),
            (IA_AB_PORTRAIT, two, vec![]),
            (IA_AB_LANDSCAPE, two, vec!["AbOrient(0)"]),
            (IA_AB_FIT, two, vec!["fit=Some(0)"]),
            (IA_PROP_LINK, Scene::Properties, vec!["lock=true"]),
            (IA_FLIP_H, Scene::Properties, vec!["Flip(true)"]),
            (IA_FLIP_V, Scene::Properties, vec!["Flip(false)"]),
            (IA_NO_FILL, Scene::Properties, vec!["Paint(Fill, None)"]),
            (IA_NO_STROKE, Scene::Properties, vec!["Paint(Stroke, None)"]),
            (IA_PICKER_CLOSE, Scene::Picker, vec!["PickerCancel"]),
            (IA_TRANSPARENT, two, vec!["Opaque"]),
            (IA_TRANSPARENT, Scene::Artboard { count: 0, portrait: true }, vec![]),
            (IA_CLIP, two, vec!["AbClip(0)"]),
            (IA_CLIP, Scene::Artboard { count: 0, portrait: true }, vec![]),
            (IA_MOVE, two, vec!["AbMoveArt(false)"]),
            (IA_OBJECT_CLIP, Scene::Properties, vec!["ClipExempt(true)"]),
            (IA_STROKE_PRESETS, Scene::Properties, vec![]),
            (IA_SNAP, Scene::Document, vec!["ToggleSnapping"]),
            (IA_GUIDES, Scene::Document, vec!["ToggleGuides"]),
            (IA_RULERS, Scene::Document, vec!["ToggleRulers"]),
            (IA_GRID, Scene::Document, vec![]),
            (IA_GUIDES_LOCK, Scene::Document, vec!["ToggleGuidesLock"]),
            (IA_SMART, Scene::Document, vec!["ToggleSmartGuides"]),
            (IA_POINT, Scene::Document, vec!["ToggleSnapPoint"]),
            (IA_SNAP_GRID, Scene::Document, vec!["ToggleSnapGrid"]),
            (IA_EDIT_BOARDS, Scene::Document, vec!["EditArtboards"]),
            (IA_FIT_BOARD, Scene::Document, vec!["FitArtboard(0)"]),
            (IA_ADD_BOARD, Scene::Document, vec!["AbAdd"]),
            (IA_PREV_BOARD, Scene::Document, vec![]),
            (IA_NEXT_BOARD, Scene::Document, vec!["AbActive(1)"]),
        ]
    }

    #[test]
    fn converted_icon_buttons_emit_what_their_text_buttons_emitted() {
        for (action, scene, want) in table() {
            let mut rig = Rig::new(scene);
            let rect = rig.probe(action.key).1;
            assert!(rect.width() >= 24.0 && rect.height() >= 24.0, "{} target", action.key);
            let got = rig.click(action.key);
            assert_eq!(got, want, "{} by pointer", action.key);
            let got = Rig::new(scene).enter(action.key);
            assert_eq!(got, want, "{} by keyboard (focus + Enter)", action.key);
        }
    }

    #[test]
    fn every_icon_action_is_drawn_and_carries_its_label_as_a_tooltip() {
        use super::{IA_AB_DEL, IA_AB_FIT, IA_LAYER_GROUP};
        let drawn: Vec<&str> = table().iter().map(|(a, _, _)| a.key).collect();
        let mut keys = std::collections::HashSet::new();
        for a in ICON_ACTIONS {
            assert!(keys.insert(a.key), "duplicate icon action key {}", a.key);
            assert!(!a.label.trim().is_empty(), "{}: empty label", a.key);
            let tip = a.tooltip();
            assert!(tip.starts_with(a.label), "{}: the tooltip must lead with the old label", a.key);
            assert!(drawn.contains(&a.key), "{} is not covered by the emission table", a.key);
        }
        assert!(IA_LAYER_GROUP.tooltip().contains(&varos_app::shell::tokens::shortcut_label("G")));
        assert!(IA_AB_FIT.tooltip().contains(&varos_app::shell::tokens::shortcut_label("0")));
        let reason = varos_app::shell::kit::icon_tooltip(
            &IA_AB_DEL.tooltip(),
            varos_app::shell::kit::IconState::Disabled("the last artboard can't be deleted"),
        );
        assert!(reason.starts_with("Delete artboard") && reason.contains("last artboard"));
    }

    /// Every icon draw in `src` — a texture `.image(…)` or a registry `Icon::….paint(…)` — as
    /// (enclosing fn, call, drawn at a raw numeric size?).
    fn icon_draws(src: &str) -> Vec<(String, String, bool)> {
        let numeric = |s: &str| s.starts_with(|c: char| c.is_ascii_digit());
        let mut out = vec![];
        for pattern in [".image(", ".paint("] {
            let mut from = 0;
            while let Some(at) = src[from..].find(pattern) {
                let start = from + at;
                let open = start + pattern.len() - 1;
                // the balanced argument list and its top-level arguments (whitespace dropped)
                let (mut depth, mut end, mut args, mut arg) = (0, open, vec![], String::new());
                for (i, ch) in src[open..].char_indices() {
                    match ch {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                end = open + i;
                                break;
                            }
                        }
                        ',' if depth == 1 => {
                            args.push(std::mem::take(&mut arg));
                            continue;
                        }
                        _ => {}
                    }
                    if !(depth == 1 && ch == '(') && !ch.is_whitespace() {
                        arg.push(ch);
                    }
                }
                args.push(arg);
                let call: String = src[start..=end].chars().filter(|c| !c.is_whitespace()).collect();
                let sized = ["vec2(", "splat("]
                    .iter()
                    .any(|f| call.match_indices(f).any(|(i, _)| numeric(&call[i + f.len()..])));
                let bare = pattern == ".paint(" && args.iter().any(|a| numeric(a));
                let func = src[..start].rsplit("fn ").next().unwrap().split('(').next().unwrap().trim();
                out.push((func.to_string(), call, sized || bare));
                from = end.max(start + 1);
            }
        }
        out
    }

    /// The lint the study asked for (§7 note): in every file that draws icons, every icon draw takes its
    /// size from a token. The top-bar draws are left for the top-bar owner this stage, each function
    /// capped at the number of raw-size draws it has today — a cap may only go down, never up.
    #[test]
    fn icon_sizes_come_from_tokens() {
        // 4b: the band's glyphs are registry icons on token sizes now; only Windows' burger texture is left
        const TOP_BAR_CAPS: [(&str, usize); 1] = [("build_topbar", 1)];
        fn production_rs(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    production_rs(&path, out);
                } else if path.extension().is_some_and(|ext| ext == "rs")
                    && path.file_name().is_some_and(|name| name != "tests.rs")
                {
                    out.push(path);
                }
            }
        }
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut paths = vec![src.join("ui.rs")];
        production_rs(&src.join("ui"), &mut paths);
        paths.extend([
            src.join("chrome.rs"),
            src.join("start_page.rs"),
            src.join("shell/kit/board.rs"),
            src.join("shell/boxtree.rs"),
            src.join("shell/kit/mod.rs"),
            src.join("shell/kit/icons.rs"),
            src.join("recovery_card.rs"),
        ]);
        let mut raw: std::collections::HashMap<String, usize> = Default::default();
        let mut scanned = 0;
        for path in paths {
            let source = std::fs::read_to_string(&path).unwrap();
            for (func, call, is_raw) in icon_draws(&source) {
                scanned += 1;
                if is_raw {
                    let capped = TOP_BAR_CAPS.iter().any(|(f, _)| *f == func);
                    assert!(capped, "{}: `{func}` draws an icon at a raw size: {call}", path.display());
                    *raw.entry(func).or_default() += 1;
                }
            }
        }
        // 27 since 2026-10-06: the band's two Search glyphs and the Custom… preset's plus were removed;
        // 29 with the recovery card's Review panel (its history and shield glyphs; × is a kit icon button)
        assert!(scanned >= 29, "the scan must see every current production icon draw (saw {scanned})");
        for (func, cap) in TOP_BAR_CAPS {
            let n = raw.get(func).copied().unwrap_or(0);
            assert!(n > 0, "`{func}` must remain covered while its raw-size exception exists");
            assert!(n <= cap, "`{func}` has {n} raw-size icon draws; its cap is {cap} and may only shrink");
        }
    }

    /// The scanner itself catches a raw size in both draw forms and passes token sizes.
    #[test]
    fn icon_size_scanner_catches_both_draw_forms() {
        let src = "fn a() { Icon::Fit.paint(&p, c, 16.0, MUTED); }\n\
                   fn b() { p.image(t.id(), egui::Rect::from_center_size(c, egui::vec2(15.0, 15.0)), UV01(), col); }\n\
                   fn c() { Icon::Fit.paint(ui.painter(), egui::pos2(x - 4.0, y), ICON_SM, MUTED); }\n\
                   fn d() { p.image(t.id(), egui::Rect::from_center_size(c, egui::Vec2::splat(ICON_MD)), UV01(), col); }";
        let draws = icon_draws(src);
        let raw: Vec<&str> = draws.iter().filter(|d| d.2).map(|d| d.0.as_str()).collect();
        assert_eq!(draws.len(), 4);
        assert_eq!(raw, ["b", "a"], "image(vec2 literal) and paint(bare literal) are raw; tokens are not");
    }
}

/// Start v2 mounted: Home's body is THE Start page (the band has no Search since 2026-10-06); its
/// actions reach the host as `AppCommand`s through the one adapter, filter actions stay on the model.
#[cfg(test)]
mod home_page_tests {
    use super::*;
    use crate::app_command::AppCommand;
    use egui::{Event, PointerButton, Pos2, RawInput};
    use varos_app::start::StartModel;
    use varos_app::start_page::{demo, ids, StartPage};
    use varos_core::board::PresetId;

    const SIZE: egui::Vec2 = egui::vec2(1512.0, 982.0);
    struct Home {
        ctx: egui::Context,
        shell: varos_app::shell::ShellState,
        page: StartPage,
        model: StartModel,
    }
    #[test]
    fn home_frame_removes_document_recovery_card() {
        let mut home = Home::new();
        let rows = [varos_app::recovery_card::ReviewRow {
            rid: "copy".into(),
            name: "Recovered board".into(),
            ..Default::default()
        }];
        for _ in 0..2 {
            let _ = home.ctx.run_ui(
                RawInput { screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, SIZE)), ..Default::default() },
                |ui| {
                    varos_app::recovery_card::show(ui.ctx(), ui.max_rect(), &rows, "Recovery on");
                },
            );
        }
        assert!(home.ctx.read_response(varos_app::recovery_card::ids::card()).is_some());
        home.frame(vec![]);
        home.frame(vec![]);
        for id in [varos_app::recovery_card::ids::card(), varos_app::recovery_card::ids::panel()] {
            assert!(home.ctx.read_response(id).is_none(), "run_home's painting pass has no floating recovery object");
        }
    }
    impl Home {
        fn new() -> Self {
            let ctx = egui::Context::default();
            varos_app::shell::fonts::install(&ctx);
            varos_app::shell::tokens::apply(&ctx);
            let home = std::path::PathBuf::from("/Users/designer");
            let mut h = Self {
                ctx,
                shell: varos_app::shell::ShellState::standard(),
                page: StartPage::new(),
                model: demo::model(&home, Some(3)),
            };
            h.frame(vec![]);
            h
        }
        fn frame(&mut self, events: Vec<Event>) -> Vec<AppCommand> {
            let input = RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, SIZE)),
                events,
                ..Default::default()
            };
            let mut cmds = vec![];
            let icons = TopIcons { menu: None };
            let (mut win, mut rail, mut dock) = (None, true, true);
            let _ = self.ctx.run_ui(input, |root| {
                build_home_frame(
                    root,
                    &icons,
                    &mut self.shell,
                    &mut win,
                    &[],
                    &mut cmds,
                    &mut rail,
                    &mut dock,
                    &mut self.page,
                    &mut self.model,
                    None,
                    false,
                );
            });
            cmds
        }
        fn rect(&self, id: egui::Id) -> egui::Rect {
            self.ctx.read_response(id).unwrap_or_else(|| panic!("{id:?} not drawn")).rect
        }
        fn click(&mut self, id: egui::Id) -> Vec<AppCommand> {
            let pos = self.rect(id).center();
            self.click_at(pos)
        }
        fn click_at(&mut self, pos: Pos2) -> Vec<AppCommand> {
            let e = |pressed| Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            let mut out = self.frame(vec![Event::PointerMoved(pos)]);
            out.extend(self.frame(vec![e(true)]));
            out.extend(self.frame(vec![e(false)]));
            out
        }
    }

    #[test]
    fn home_draws_the_start_page_and_the_band_has_no_search() {
        let mut h = Home::new();
        h.frame(vec![]);
        let bar = crate::chrome::TOPBAR.height;
        let new_board = h.rect(ids::new_board());
        assert_eq!(new_board.size(), egui::vec2(272.0, 64.0), "the mockup's hero button");
        let pad = varos_app::shell::tokens::SB_PAD_TOP;
        assert_eq!(new_board.top(), bar + pad, "4b: the box starts at the band's bottom");
        assert_eq!(new_board.top(), 72.0, "…and the content sits where the owner approved it (y 72)");
        // owner 2026-10-06: no "Search boards" in the band — nothing is drawn under its old id, and a
        // click where it sat (the right zone, left of the V mark) raises nothing and focuses nothing
        assert!(h.ctx.read_response(egui::Id::new("start-v2-search")).is_none(), "no Search field");
        let l = band_layout(
            crate::chrome::topbar_layout(
                egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(SIZE.x, bar)),
                crate::chrome::TOPBAR,
                None,
                &[],
                None,
            ),
            true,
        );
        let old_slot = egui::pos2(l.brand.left() - 120.0, l.brand.center().y);
        assert!(l.interactive_rects().iter().all(|r| !r.contains(old_slot)), "nothing published there");
        assert!(h.click_at(old_slot).is_empty());
        assert_eq!(h.ctx.memory(|m| m.focused()), None, "no field took the keyboard");
    }

    #[test]
    fn home_actions_become_app_commands_and_filters_stay_on_the_model() {
        let mut h = Home::new();
        assert_eq!(h.click(ids::new_board()), [AppCommand::NewBoard]);
        assert_eq!(h.click(ids::preset(PresetId::Story)), [AppCommand::NewWithPreset(PresetId::Story)]);
        assert_eq!(h.click(ids::open()), [AppCommand::OpenDialog]);
        assert!(h.click(ids::filter(Some("print"))).is_empty(), "a filter is not a host command");
        assert_eq!(h.model.filter().tag.as_deref(), Some("print"));
        assert_eq!(h.model.visible_count(), 3);
        // two frames with the filtered cards before aiming at one (egui reads widget rects a pass late)
        h.frame(vec![]);
        h.frame(vec![]);
        let card = h.model.visible_cards().next().unwrap().clone();
        assert_eq!(h.click(ids::card(&card.key)), [AppCommand::OpenRecent(card.path.clone())]);
    }
}

#[cfg(test)]
mod panel_icons_lane2_tests {
    use super::*;
    use varos_core::{command::EditCommand, units::Unit};

    #[test]
    fn properties_mirrors_the_native_view_checks_after_panel_and_menu_edits() {
        let mut ed = Editor::new();
        let check = |ed: &Editor| {
            let s = Snap::read(ed);
            for (kind, value) in [
                (crate::chrome::Check::Rulers, s.rulers_on),
                (crate::chrome::Check::Guides, s.guides_on),
                (crate::chrome::Check::GuidesLocked, s.guides_locked),
                (crate::chrome::Check::SmartGuides, s.snap_config.smart),
                (crate::chrome::Check::SnapPoint, s.snap_config.key_points),
                (crate::chrome::Check::SnapGrid, s.snap_config.grid),
            ] {
                assert_eq!(crate::editor_check(ed, kind), Some(value));
            }
            assert_eq!(s.snap_enabled, ed.doc.snap.enabled);
        };
        check(&ed);
        let initial = ed.doc.snap;
        apply_frame(
            &mut ed,
            initial,
            vec![
                Op::ToggleSnapping,
                Op::ToggleGuides,
                Op::ToggleRulers,
                Op::ToggleGuidesLock,
                Op::ToggleSmartGuides,
                Op::ToggleSnapPoint,
                Op::ToggleSnapGrid,
            ],
        );
        check(&ed);
        assert_eq!(ed.doc.snap.grid, !initial.grid);
        assert_eq!(ed.doc.snap.key_points, !initial.key_points);
        assert_eq!(ed.doc.snap.smart, !initial.smart);
        crate::menu_snap_toggle(&mut ed, crate::chrome::SnapRow::Point);
        crate::menu_snap_toggle(&mut ed, crate::chrome::SnapRow::Grid);
        ed.execute(EditCommand::ToggleGuidesLocked);
        ed.execute(EditCommand::ToggleSmartGuides);
        check(&ed);
        assert_eq!(ed.doc.snap.grid, initial.grid);
        assert_eq!(ed.doc.snap.key_points, initial.key_points);
        assert_eq!(ed.doc.snap.smart, initial.smart);
    }

    #[test]
    fn document_artboard_navigation_is_bounded_and_uses_the_existing_app_op() {
        let mut ed = Editor::new();
        assert_eq!(adjacent_artboard(0, 0, true), None);
        for _ in 0..3 {
            ed.execute(EditCommand::AddArtboard);
        }
        apply_ops(&mut ed, vec![Op::AbActive(0)]);
        assert_eq!(adjacent_artboard(0, 3, false), None);
        let next = adjacent_artboard(ed.doc.active, 3, true).unwrap();
        apply_ops(&mut ed, vec![Op::AbActive(next)]);
        assert_eq!(Snap::read(&ed).active_artboard, 1);
        let previous = adjacent_artboard(ed.doc.active, 3, false).unwrap();
        apply_ops(&mut ed, vec![Op::AbActive(previous)]);
        assert_eq!(ed.doc.active, 0);
        apply_ops(&mut ed, vec![Op::AbActive(2)]);
        assert_eq!(adjacent_artboard(ed.doc.active, 3, true), None);
        apply_ops(&mut ed, vec![Op::AbActive(99)]);
        assert_eq!(ed.doc.active, 2);
    }

    #[test]
    fn document_unit_dropdown_commits_one_undo_step() {
        let mut ed = Editor::new();
        let before = ed.doc.units.display;
        apply_ops(&mut ed, vec![Op::Units(Unit::In)]);
        assert_eq!(Snap::read(&ed).units_label, "in");
        apply_ops(&mut ed, vec![Op::Units(Unit::In)]);
        ed.undo();
        assert_eq!(ed.doc.units.display, before);
        ed.redo();
        assert_eq!(ed.doc.units.display, Unit::In);
    }

    #[test]
    fn layers_filter_menu_changes_only_visible_kinds_and_is_document_scoped() {
        use egui::{Event, Key, Modifiers};
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        varos_app::shell::tokens::apply(&ctx);
        let mut ed = Editor::new();
        ed.execute(EditCommand::AddArtboard);
        let rows = build_layer_rows(&ed, &Default::default(), "", 0, &mut Default::default());
        let mut path = rows[0].clone();
        path.kind = LKind::Path;
        path.name = "Test path".into();
        path.id = 1;
        path.active = false;
        let mut group = path.clone();
        group.kind = LKind::Group;
        group.name = "Test group".into();
        group.id = 2;
        let mut layer = path.clone();
        layer.kind = LKind::Layer;
        layer.name = "Test layer".into();
        layer.id = 3;
        let mut rows = rows;
        rows.extend([path, group, layer]);
        let frame = |events, doc| {
            set_doc_salt(&ctx, Some(crate::app_command::SessionId(doc)));
            let mut ops = vec![];
            let out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300.0, 600.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let icons = LayerIcons { eye: None, eye_off: None, lock: None, unlock: None, search: None };
                    panel_layers(
                        ui,
                        &rows,
                        &icons,
                        &mut String::new(),
                        &mut None,
                        &mut Default::default(),
                        &mut None,
                        &mut None,
                        &mut ops,
                    );
                },
            );
            assert!(ops.is_empty());
            out.shapes
                .into_iter()
                .filter_map(|s| match s.shape {
                    egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert!(frame(vec![], 1).iter().any(|s| s == "Test group"));
        let (id, rect) = icon_action_tests::PROBE.with(|p| {
            p.borrow().iter().find(|(k, _, _)| *k == "layer-filter").map(|(_, id, rect)| (*id, *rect)).unwrap()
        });
        assert!(rect.width() >= 24.0 && rect.height() >= 24.0);
        ctx.memory_mut(|m| m.request_focus(id));
        let key =
            |key| Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
        frame(vec![key(Key::Enter)], 1);
        frame(
            vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
            1,
        );
        assert!(kit::menu_open(&ctx));
        frame(vec![key(Key::ArrowDown)], 1); // All
        frame(vec![key(Key::ArrowDown)], 1); // Paths
        frame(vec![key(Key::Enter)], 1);
        let text = frame(vec![], 1);
        assert!(text.iter().any(|s| s == "Test path"), "{text:?}");
        assert!(!text.iter().any(|s| s == "Test group" || s == "Test layer"));
        assert!(frame(vec![], 2).iter().any(|s| s == "Test group"));
        for filter in 1..=3 {
            let shown = rows.iter().filter(|row| layer_kind_matches(row.kind, filter)).collect::<Vec<_>>();
            assert!(shown.iter().all(|row| row.kind == [LKind::Path, LKind::Group, LKind::Board][filter - 1]));
        }
    }
}

#[test]
fn menu_owner_absent_for_one_frame_releases_keyboard_and_closes_menu() {
    let ctx = egui::Context::default();
    let owner = egui::Id::new("disappearing-owner");
    let _ = ctx.run_ui(Default::default(), |ui| {
        kit::open_menu(ui.ctx(), owner, egui::pos2(10.0, 10.0), None);
        kit::menu(ui.ctx(), owner, &[kit::MenuEntry::Item("Action")]);
    });
    assert!(super::wants_keyboard(&ctx));
    let _ = ctx.run_ui(Default::default(), |_| {});
    assert!(!super::wants_keyboard(&ctx));
    assert!(!kit::is_menu_open(&ctx, owner));
}
