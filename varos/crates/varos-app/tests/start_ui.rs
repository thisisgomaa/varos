//! Headless E2 rendering/interaction, including F2's future recovery slots.
use egui::{Context, Event, Id, Key, Modifiers, Pos2, RawInput, Rect};
use varos_app::{
    shell::{fonts, tokens},
    start::{RecoveryRow, StartAction, StartModel},
    start_ui::StartPage,
    storage::recents::Recents,
};
fn key(key: Key, repeat: bool) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat, modifiers: Modifiers::NONE }
}
fn frame(ctx: &Context, page: &mut StartPage, ppp: f32, events: Vec<Event>) -> (Vec<StartAction>, egui::FullOutput) {
    let mut input = RawInput {
        events,
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(440.0, 700.0))),
        ..Default::default()
    };
    input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().native_pixels_per_point = Some(ppp);
    let mut actions = vec![];
    let out = ctx.run_ui(input, |ui| actions = page.draw(ui, None));
    (actions, out)
}
fn context() -> Context {
    let ctx = Context::default();
    fonts::install(&ctx);
    tokens::apply(&ctx);
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(theme, |s| s.text_styles = tokens::text_styles());
    }
    ctx
}
#[test]
fn start_only_uses_accent_for_focus_and_keyboard_emits_once_at_both_scales() {
    for ppp in [1.0, 2.0] {
        let ctx = context();
        let mut page = StartPage::new(StartModel::without_recovery(&Recents::default(), 0, |_| false));
        let (_, out) = frame(&ctx, &mut page, ppp, vec![]);
        assert!(!out.shapes.iter().any(
            |s| matches!(&s.shape, egui::Shape::Rect(r) if r.fill == tokens::ACCENT || r.stroke.color == tokens::ACCENT)
        ));
        let (_, out) = frame(&ctx, &mut page, ppp, vec![key(Key::Tab, false)]);
        assert!(out.shapes.iter().any(
            |s| matches!(&s.shape, egui::Shape::Rect(r) if r.stroke.color == tokens::ACCENT && r.fill != tokens::ACCENT)
        ));
        assert_eq!(frame(&ctx, &mut page, ppp, vec![key(Key::Enter, false)]).0, [StartAction::Open]);
        assert!(frame(&ctx, &mut page, ppp, vec![key(Key::Enter, true)]).0.is_empty());
        assert!(frame(&ctx, &mut page, ppp, vec![]).0.is_empty());
        let pos = ctx.read_response(Id::new(("start-action", 0))).unwrap().rect.center();
        let pointer = |pressed| {
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE },
            ]
        };
        assert!(frame(&ctx, &mut page, ppp, pointer(true)).0.is_empty());
        assert_eq!(frame(&ctx, &mut page, ppp, pointer(false)).0, [StartAction::New]);
    }
}
#[test]
fn recovery_slots_and_recent_actions_follow_the_pure_models_focus_order() {
    for ppp in [1.0, 2.0] {
        let ctx = context();
        let mut recent = Recents::default();
        recent.record(std::path::Path::new("/missing.vrs"), None, 1);
        let mut page = StartPage::new(StartModel::build(
            &recent,
            2,
            |_| true,
            vec![RecoveryRow {
                rid: "recovery-a".into(),
                name: "Recovered design".into(),
                original_dir: None,
                saved_at_text: "saved 14:32".into(),
            }],
        ));
        let (_, out) = frame(&ctx, &mut page, ppp, vec![]);
        let text: String = out
            .shapes
            .iter()
            .filter_map(|s| if let egui::Shape::Text(t) = &s.shape { Some(t.galley.text()) } else { None })
            .collect();
        assert!(text.contains("Missing") && text.contains("Recovered design"));
        for (focus, expected) in [
            (2, StartAction::Recover("recovery-a".into())),
            (3, StartAction::DiscardRecovery("recovery-a".into())),
            (4, StartAction::Later),
            (5, StartAction::OpenRecent("/missing.vrs".into())),
            (6, StartAction::ClearRecent),
        ] {
            page.model.set_focus(focus);
            assert_eq!(frame(&ctx, &mut page, ppp, vec![key(Key::Enter, false)]).0, [expected]);
            let _ = frame(
                &ctx,
                &mut page,
                ppp,
                vec![Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }],
            );
        }
        page.model.set_focus(5);
        assert_eq!(
            frame(&ctx, &mut page, ppp, vec![key(Key::Delete, false)]).0,
            [StartAction::RemoveRecent("/missing.vrs".into())]
        );
    }
}
