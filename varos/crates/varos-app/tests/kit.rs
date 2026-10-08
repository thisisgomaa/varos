//! CPU-only scripted interaction at logical 1× and Retina 2×. No window/GPU.
use egui::{Context, Event, Id, Key, Modifiers, PointerButton, Pos2, RawInput, Rect};
use varos_app::shell::{
    fonts,
    kit::{
        self,
        field::{self, Edit, Label, NumberField},
        Availability, Control, ControlResponse, Icon, MenuEntry,
    },
    tokens,
};

fn input(ppp: f32, events: Vec<Event>) -> RawInput {
    let mut input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(360.0, 480.0))),
        events,
        ..Default::default()
    };
    input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().native_pixels_per_point = Some(ppp);
    input
}
fn context(ppp: f32) -> Context {
    let ctx = Context::default();
    fonts::install(&ctx);
    tokens::apply(&ctx);
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(theme, |s| s.text_styles = tokens::text_styles());
    }
    let _ = ctx.run_ui(input(ppp, vec![]), |_| {});
    ctx
}
fn key(key: Key, repeat: bool) -> Event {
    Event::Key { key, physical_key: Some(key), pressed: true, repeat, modifiers: Modifiers::NONE }
}
fn pointer(pos: Pos2, pressed: bool) -> Vec<Event> {
    vec![
        Event::PointerMoved(pos),
        Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE },
    ]
}
fn frame(
    ctx: &Context,
    events: Vec<Event>,
    state: Availability<'_>,
    selected: bool,
    pointer_only: bool,
) -> (ControlResponse, egui::FullOutput) {
    let mut result = None;
    let out = ctx.run_ui(input(ctx.pixels_per_point(), events), |ui| {
        let mut control = Control::new(Id::new("home"), "Home");
        control.icon = Some(Icon::Home);
        control.availability = state;
        control.selected = selected;
        control.pointer_only = pointer_only;
        result = Some(kit::action(ui, control, true));
    });
    (result.unwrap(), out)
}
fn focus_outline(out: &egui::FullOutput) -> bool {
    out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.stroke.color == tokens::ACCENT))
}

fn number(id: &'static str, disabled: bool) -> NumberField<'static> {
    NumberField {
        id: Id::new(id),
        width: 80.0,
        label: Label::Letter(id),
        tip: if disabled { "Nothing to scale" } else { id },
        value: 12.0,
        decimals: 0,
        speed: 1.0,
        range: 0.0..=100.0,
        disabled,
    }
}

fn number_frame(ctx: &Context, events: Vec<Event>, time: f64) -> (Vec<Edit<f32>>, egui::FullOutput) {
    let mut raw = input(ctx.pixels_per_point(), events);
    raw.time = Some(time);
    let mut edits = vec![];
    let out = ctx.run_ui(raw, |ui| {
        field::group(ui, "numbers", |ui| {
            edits.push(field::number_field(ui, number("A", false)));
            edits.push(field::number_field(ui, number("Disabled", true)));
            edits.push(field::number_field(ui, number("B", false)));
        });
    });
    (edits, out)
}

#[test]
fn disabled_number_field_is_inert_skipped_transparent_and_explains_why() {
    let ctx = context(1.0);
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(theme, |style| style.interaction.tooltip_delay = 0.0);
    }
    let (edits, out) = number_frame(&ctx, vec![], 1.0);
    let disabled = edits[1].rect;
    let painted_box =
        |shape: &egui::epaint::ClippedShape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.rect == disabled);
    assert!(out.shapes.iter().any(|shape| {
        matches!(&shape.shape, egui::Shape::Rect(rect)
            if rect.rect == disabled && rect.fill == egui::Color32::TRANSPARENT && rect.stroke.color == tokens::LINE)
    }));
    assert!(!out.shapes.iter().any(|shape| {
        painted_box(shape) && matches!(&shape.shape, egui::Shape::Rect(rect) if rect.fill != egui::Color32::TRANSPARENT)
    }));

    let at = disabled.center();
    for (events, time) in [
        (pointer(at, true), 2.0),
        (vec![Event::PointerMoved(at + egui::vec2(24.0, 0.0))], 3.0),
        (pointer(at + egui::vec2(24.0, 0.0), false), 4.0),
    ] {
        let (edits, _) = number_frame(&ctx, events, time);
        assert!(!edits[1].editing && edits[1].live.is_none() && edits[1].commit.is_none());
        assert!(!field::any_open(&ctx), "a disabled click/drag must not start a field session");
    }

    let mut tooltip_seen = false;
    for time in [5.0, 6.0] {
        let (_, out) = number_frame(&ctx, vec![Event::PointerMoved(at)], time);
        tooltip_seen |= out.shapes.iter().any(
            |shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().contains("Nothing to scale")),
        );
    }
    assert!(tooltip_seen, "the disabled field must retain its reason tooltip");
}

#[test]
fn segmented_control_paints_one_track_outline() {
    let ctx = context(1.0);
    let mut track = Rect::NOTHING;
    let mut rects = vec![];
    let out = ctx.run_ui(input(1.0, vec![]), |ui| {
        let segment = egui::vec2(tokens::SEG_W, tokens::SEG_BTN_H);
        let size = kit::board::segmented_size(3, segment);
        (track, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        rects = kit::board::segmented_frame(
            ui,
            Id::new("measured-segment"),
            track,
            &["Auto", "Selection", "Artboard"],
            0,
            segment,
            None,
        )
        .rects;
    });
    assert_eq!(track.height(), 24.0);
    assert!(rects.iter().all(|rect| rect.size() == egui::vec2(60.0, 20.0)));
    assert_eq!(rects[1].left() - rects[0].right(), 1.0);
    let outlines = out
        .shapes
        .iter()
        .filter(|shape| {
            matches!(&shape.shape, egui::Shape::Rect(rect)
                if rect.stroke.color == tokens::LINE && rect.stroke.width == 1.0)
        })
        .count();
    assert_eq!(outlines, 1, "all segments share exactly one LINE track outline");
}

#[test]
fn pointer_keyboard_repeat_and_host_owned_keyboard_are_single_actions() {
    for ppp in [1.0, 2.0] {
        let ctx = context(ppp);
        let (r, _) = frame(&ctx, vec![], Availability::Enabled, false, false);
        assert!(!r.activated);
        assert!(r.response.rect.width() >= 24.0 && r.response.rect.height() >= 24.0);
        let pos = r.response.rect.center();
        assert!(!frame(&ctx, pointer(pos, true), Availability::Enabled, false, false).0.activated);
        let (r, out) = frame(&ctx, pointer(pos, false), Availability::Enabled, false, false);
        assert!(r.activated);
        assert!(!focus_outline(&out), "mouse focus is not keyboard focus-visible");
        r.response.request_focus();
        for button in [Key::Enter, Key::Space] {
            let (r, out) = frame(&ctx, vec![key(button, false)], Availability::Enabled, true, false);
            assert!(r.activated);
            assert!(focus_outline(&out), "selected Home must retain keyboard outline");
            assert!(!frame(&ctx, vec![key(button, true)], Availability::Enabled, true, false).0.activated);
        }
        assert!(!frame(&ctx, vec![key(Key::Enter, false)], Availability::Enabled, true, true).0.activated);
        assert!(!frame(&ctx, vec![], Availability::Enabled, true, false).0.activated);
    }
}

#[test]
fn disabled_busy_and_disabled_parent_never_activate() {
    for ppp in [1.0, 2.0] {
        for state in [Availability::Disabled("File is missing"), Availability::Busy("Opening file…")] {
            let ctx = context(ppp);
            let (r, _) = frame(&ctx, vec![], state, true, false);
            let pos = r.response.rect.center();
            r.response.request_focus();
            for events in [pointer(pos, true), pointer(pos, false), vec![key(Key::Enter, false)]] {
                let (r, out) = frame(&ctx, events, state, true, false);
                assert!(!r.activated);
                assert!(!r.response.enabled());
                assert!(!focus_outline(&out));
            }
        }
        let ctx = context(ppp);
        let _ = ctx.run_ui(input(ppp, vec![key(Key::Enter, false)]), |ui| {
            ui.add_enabled_ui(false, |ui| {
                let mut c = Control::new(Id::new("parent-disabled"), "Open");
                c.focused = true;
                let r = kit::action(ui, c, false);
                assert!(!r.response.enabled() && !r.activated);
            });
        });
    }
}

#[test]
fn tab_navigation_skips_disabled_and_ids_survive_row_reordering() {
    for ppp in [1.0, 2.0] {
        let ctx = context(ppp);
        let draw = |events, reverse: bool| {
            let mut rows = vec![];
            let out = ctx.run_ui(input(ppp, events), |ui| {
                let order = if reverse { ["b", "disabled", "a"] } else { ["a", "disabled", "b"] };
                for name in order {
                    let mut c = Control::new(Id::new(name), name);
                    if name == "disabled" {
                        c.availability = Availability::Disabled("Missing");
                    }
                    rows.push(kit::list_row(
                        ui,
                        c,
                        "/Users/designer/Documents/Very long project folder/Brand identity 2026.vrs · Yesterday",
                    ));
                }
            });
            let texts: Vec<_> = out
                .shapes
                .iter()
                .filter_map(|shape| if let egui::Shape::Text(text) = &shape.shape { Some(text) } else { None })
                .collect();
            assert!(texts.iter().any(|text| text.galley.elided), "long paths need visible ellipsis");
            assert!(texts.iter().all(|text| text.pos.x + text.galley.size().x <= 360.0));
            rows
        };
        let rows = draw(vec![], false);
        rows[0].response.request_focus();
        let rows = draw(vec![key(Key::Tab, false)], false);
        assert!(rows[2].response.has_focus());
        let rows = draw(vec![], true);
        assert_eq!(rows[0].response.id, Id::new("b"));
        assert!(rows[0].response.has_focus());
        assert!(rows.iter().all(|r| r.response.rect.width() <= 360.0 && r.response.rect.height() == tokens::KIT_ROW_H));
    }
}

#[test]
fn text_and_focus_contrast_meet_the_minimum_kit_contract() {
    fn luminance(c: egui::Color32) -> f32 {
        let linear = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(c.r()) + 0.7152 * linear(c.g()) + 0.0722 * linear(c.b())
    }
    let contrast = |a, b| {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    };
    for background in [tokens::PANEL, tokens::SURFACE, tokens::HOVER] {
        assert!(contrast(tokens::TEXT, background) >= 4.5);
        assert!(contrast(tokens::ACCENT, background) >= 3.0);
    }
    for background in [tokens::PANEL, tokens::SURFACE] {
        assert!(contrast(tokens::MUTED, background) >= 4.5);
    }
}

fn release(key: Key) -> Event {
    Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers: Modifiers::NONE }
}
fn menu_frame(ctx: &Context, events: Vec<Event>) -> (Option<usize>, egui::FullOutput) {
    let mut chosen = None;
    let entries = [MenuEntry::Item("Locate…"), MenuEntry::Separator, MenuEntry::Item("Remove from Recent")];
    let out = ctx.run_ui(input(ctx.pixels_per_point(), events), |ui| {
        let _ = ui.label("host");
        chosen = kit::menu(ui.ctx(), Id::new("owner"), &entries);
    });
    (chosen, out)
}

#[test]
fn kit_menu_keyboard_pointer_escape_and_hairline_separator() {
    for ppp in [1.0, 2.0] {
        let ctx = context(ppp);
        let owner = Id::new("owner");
        kit::open_menu(&ctx, owner, Pos2::new(40.0, 40.0), None);
        let _ = menu_frame(&ctx, vec![]); // egui sizes a new Area invisibly for one frame
        let (chosen, out) = menu_frame(&ctx, vec![]);
        assert!(chosen.is_none() && kit::is_menu_open(&ctx, owner));
        assert!(!focus_outline(&out), "pointer-opened menu shows no ring until the keyboard is used");
        assert!(
            out.shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::LineSegment { stroke, .. } if stroke.color == tokens::LINE)),
            "kit separator is a LINE hairline"
        );
        assert!(!out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.fill == tokens::ACCENT)));
        // ↓ focuses the first item (ring), ↓ again skips the separator, ↓ at the end stays, Enter picks it.
        let (_, out) = menu_frame(&ctx, vec![key(Key::ArrowDown, false)]);
        assert!(focus_outline(&out));
        let (chosen, _) = menu_frame(&ctx, vec![key(Key::ArrowDown, false), key(Key::ArrowDown, false)]);
        assert!(chosen.is_none());
        let (chosen, _) = menu_frame(&ctx, vec![key(Key::Enter, false), release(Key::Enter)]);
        assert_eq!(chosen, Some(2));
        assert!(!kit::menu_open(&ctx), "activation closes the menu");
        // ↑ from nothing lands on the last item; ↑ again on the first; Enter → 0.
        kit::open_menu(&ctx, owner, Pos2::new(40.0, 40.0), None);
        let _ = menu_frame(&ctx, vec![key(Key::ArrowUp, false), key(Key::ArrowUp, false)]);
        assert_eq!(menu_frame(&ctx, vec![key(Key::Enter, false), release(Key::Enter)]).0, Some(0));
        // Esc closes without a choice.
        kit::open_menu(&ctx, owner, Pos2::new(40.0, 40.0), None);
        let _ = menu_frame(&ctx, vec![]);
        assert_eq!(menu_frame(&ctx, vec![key(Key::Escape, false)]).0, None);
        assert!(!kit::menu_open(&ctx));
        // Pointer: press + release on an item activates once; a press outside closes.
        kit::open_menu(&ctx, owner, Pos2::new(40.0, 40.0), None);
        let _ = menu_frame(&ctx, vec![]);
        let row = ctx.read_response(owner.with(("menu-item", 2usize))).unwrap().rect;
        assert!(row.height() >= tokens::KIT_MIN_TARGET);
        assert!(menu_frame(&ctx, pointer(row.center(), true)).0.is_none());
        assert_eq!(menu_frame(&ctx, pointer(row.center(), false)).0, Some(2));
        kit::open_menu(&ctx, owner, Pos2::new(40.0, 40.0), None);
        let _ = menu_frame(&ctx, vec![]);
        let _ = menu_frame(&ctx, pointer(Pos2::new(350.0, 470.0), true));
        assert!(!kit::menu_open(&ctx), "a press outside closes the menu");
    }
}

#[test]
fn kit_menu_row_is_a_kit_control_with_a_keyboard_ring_only() {
    let ctx = context(1.0);
    let row = |focused: bool, events| {
        let mut result = None;
        let out = ctx.run_ui(input(1.0, events), |ui| {
            let mut c = Control::new(Id::new("row"), "Remove from Recent");
            c.pointer_only = true;
            c.focused = focused;
            result = Some(kit::menu_row(ui, c));
        });
        (result.unwrap(), out)
    };
    let (r, out) = row(false, vec![]);
    assert!(!focus_outline(&out) && r.response.rect.height() == tokens::KIT_CONTROL_H);
    let (r, out) = row(true, vec![key(Key::Enter, false)]);
    assert!(focus_outline(&out) && !r.activated, "a pointer-only row leaves Enter to its menu");
    let pos = r.response.rect.center();
    assert!(!row(false, pointer(pos, true)).0.activated);
    assert!(row(false, pointer(pos, false)).0.activated);
}

#[test]
fn kit_icons_are_embedded_svgs_rendered_to_textures() {
    let names: Vec<_> = Icon::ALL.iter().map(|i| i.lucide().0).collect();
    assert_eq!(names[..6], ["house", "file-plus", "folder-open", "x", "file", "ellipsis"]);
    for icon in Icon::ALL {
        let (name, svg) = icon.lucide();
        let class = if icon.is_original() { format!("varos-{name}") } else { format!("lucide-{name}") };
        assert!(svg.contains(&class) && svg.contains("stroke=\"currentColor\""), "{name}");
        let (rgba, w, h) = icon.rasterize().unwrap_or_else(|| panic!("{name} must parse"));
        assert_eq!((w, h), (tokens::ICON_RASTER, tokens::ICON_RASTER));
        assert!(rgba.chunks(4).any(|px| px[3] > 0 && px[0] == 255), "{name} renders white ink");
    }
    // Painted as a texture image (a mesh), never as hand-placed line shapes.
    let ctx = context(1.0);
    let (_, out) = frame(&ctx, vec![], Availability::Enabled, false, false);
    assert!(out
        .shapes
        .iter()
        .any(|s| matches!(&s.shape, egui::Shape::Mesh(m) if m.texture_id != egui::TextureId::default())));
    assert!(!out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Path(_))));
}

#[test]
fn panel_icon_segments_have_lifted_targets_tooltips_and_a_232_pt_harmony_track() {
    let segments = [
        (Icon::HarmonyNone, "None"),
        (Icon::HarmonyComplementary, "Complementary"),
        (Icon::HarmonyAnalogous, "Analogous"),
        (Icon::HarmonySplit, "Split complementary"),
        (Icon::HarmonyTriad, "Triad"),
        (Icon::HarmonyTetradic, "Tetradic"),
        (Icon::HarmonySquare, "Square"),
        (Icon::HarmonyMono, "Monochrome"),
    ];
    for ppp in [1.0, 2.0] {
        let ctx = context(ppp);
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            ctx.style_mut_of(theme, |s| {
                s.interaction.tooltip_delay = 0.0;
                s.interaction.tooltip_grace_time = 0.0;
                s.interaction.show_tooltips_only_when_still = false;
            });
        }
        let time = std::cell::Cell::new(1.0);
        let draw = |events| {
            let mut chosen = None;
            let mut rects = vec![];
            let mut track = Rect::NOTHING;
            time.set(time.get() + 1.0 / 60.0);
            let mut raw = input(ppp, events);
            raw.time = Some(time.get());
            let out = ctx.run_ui(raw, |ui| {
                let segment = egui::vec2(tokens::PANEL_SEG_W, tokens::PANEL_SEG_H);
                let size = egui::vec2(tokens::HARMONY_TRACK_W, tokens::SEG_H);
                (track, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                (chosen, rects) = kit::board::segmented_sized(
                    ui,
                    Id::new("harmony"),
                    track,
                    &segments,
                    0,
                    None,
                    segment,
                    tokens::PANEL_SEG_GLYPH,
                );
            });
            (chosen, rects, track, out)
        };
        let (_, rects, track, out) = draw(vec![]);
        assert_eq!(track.width(), 232.0);
        assert_eq!(rects.len(), 8);
        assert!(rects
            .iter()
            .all(|r| (r.width() - 28.0).abs() < 0.001 && r.height() == 20.0 && track.contains_rect(*r)));
        assert!(out
            .shapes
            .iter()
            .any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.fill == tokens::TOGGLE_WELL && r.rect == rects[0])));
        assert!(!out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.fill == tokens::ACCENT)));
        for (i, (_, name)) in segments.iter().enumerate() {
            assert!(!name.is_empty());
            // Click in the lifted strip above the 20 pt painted segment, proving a ≥24 pt target.
            let pos = egui::pos2(rects[i].center().x, rects[i].top() - 1.0);
            draw(pointer(pos, true));
            assert_eq!(draw(pointer(pos, false)).0, Some(i));
            time.set(time.get() + 1.0);
            draw(vec![Event::PointerMoved(rects[i].center())]);
            let (_, _, _, out) = draw(vec![]);
            assert!(
                out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == *name)),
                "missing tooltip {name}: {:?}",
                out.shapes
                    .iter()
                    .filter_map(|s| if let egui::Shape::Text(t) = &s.shape { Some(t.galley.text()) } else { None })
                    .collect::<Vec<_>>()
            );
        }
    }
}
