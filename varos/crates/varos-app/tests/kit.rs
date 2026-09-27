//! CPU-only scripted interaction at logical 1× and Retina 2×. No window/GPU.
use egui::{Context, Event, Id, Key, Modifiers, PointerButton, Pos2, RawInput, Rect};
use varos_app::shell::{
    fonts,
    kit::{self, Availability, Control, ControlResponse, Icon},
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
