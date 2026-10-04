//! Headless E2 rendering/interaction, including F2 recovery rows and disabled/busy actions.
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
    frame_sized(ctx, page, ppp, events, egui::vec2(440.0, 700.0))
}
fn frame_sized(
    ctx: &Context,
    page: &mut StartPage,
    ppp: f32,
    events: Vec<Event>,
    size: egui::Vec2,
) -> (Vec<StartAction>, egui::FullOutput) {
    let mut input = RawInput { events, screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), ..Default::default() };
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
        // The first Tab lands on the first control (New) and the ring shows at once.
        let (_, out) = frame(&ctx, &mut page, ppp, vec![key(Key::Tab, false)]);
        assert!(out.shapes.iter().any(
            |s| matches!(&s.shape, egui::Shape::Rect(r) if r.stroke.color == tokens::ACCENT && r.fill != tokens::ACCENT)
        ));
        assert!(page.keyboard_focus());
        assert_eq!(frame(&ctx, &mut page, ppp, vec![key(Key::Enter, false)]).0, [StartAction::New]);
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
                problem: None,
                busy: false,
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

fn release(key: Key) -> Event {
    Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE }
}
fn ring(out: &egui::FullOutput) -> bool {
    out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.stroke.color == tokens::ACCENT))
}
fn recents(paths: &[&str]) -> Recents {
    let mut recents = Recents::default();
    for (i, path) in paths.iter().rev().enumerate() {
        recents.record(std::path::Path::new(path), None, 1 + i as u64); // first path = newest
    }
    recents
}

#[test]
fn mac_delete_backspace_and_delete_both_remove_the_focused_recent() {
    for remove in [Key::Backspace, Key::Delete] {
        let ctx = context();
        let mut page = StartPage::new(StartModel::without_recovery(&recents(&["/a.vrs"]), 2, |_| false));
        page.model.set_focus(2);
        assert_eq!(
            frame(&ctx, &mut page, 1.0, vec![key(remove, false)]).0,
            [StartAction::RemoveRecent("/a.vrs".into())],
            "{remove:?}"
        );
        assert!(frame(&ctx, &mut page, 1.0, vec![key(remove, false)]).0.is_empty(), "held key: no repeat removal");
    }
}

#[test]
fn any_key_shows_the_ring_on_new_before_the_first_tab() {
    let ctx = context();
    let mut page = StartPage::new(StartModel::without_recovery(&Recents::default(), 0, |_| false));
    assert!(!ring(&frame(&ctx, &mut page, 1.0, vec![]).1));
    let (_, out) = frame(&ctx, &mut page, 1.0, vec![key(Key::ArrowDown, false)]);
    assert!(ring(&out), "the ring shows as soon as the keyboard is used");
    assert_eq!(page.model.focus(), 0, "resting on New");
    let _ = frame(&ctx, &mut page, 1.0, vec![key(Key::Tab, false)]);
    assert_eq!(page.model.focus(), 0, "the first Tab lands on New");
    let _ = frame(&ctx, &mut page, 1.0, vec![key(Key::Tab, false)]);
    assert_eq!(page.model.focus(), 1, "the next Tab moves on");
    page.reset_focus();
    assert!(!page.keyboard_focus() && page.model.focus() == 0);
}

#[test]
fn keyboard_focus_survives_rebuilds_and_a_remove() {
    let ctx = context();
    let paths = ["/a.vrs", "/b.vrs", "/c.vrs"];
    let mut page = StartPage::new(StartModel::without_recovery(&recents(&paths), 2, |_| false));
    let _ = frame(&ctx, &mut page, 1.0, vec![key(Key::Tab, false), release(Key::Tab)]);
    for _ in 0..3 {
        let _ = frame(&ctx, &mut page, 1.0, vec![key(Key::Tab, false), release(Key::Tab)]);
    }
    assert_eq!(page.model.activate(), Some(StartAction::OpenRecent("/b.vrs".into())));
    // An unrelated rebuild (a probe answer) keeps the same row and the visible ring.
    page.replace(StartModel::without_recovery(&recents(&paths), 3, |p| p.ends_with("a.vrs")));
    assert_eq!(page.model.activate(), Some(StartAction::OpenRecent("/b.vrs".into())));
    assert!(page.keyboard_focus());
    // Remove b: focus lands on the row now in its place, ring still visible.
    page.replace(StartModel::without_recovery(&recents(&["/a.vrs", "/c.vrs"]), 3, |_| false));
    assert!(page.keyboard_focus());
    let (_, out) = frame(&ctx, &mut page, 1.0, vec![]);
    assert!(ring(&out));
    assert_eq!(frame(&ctx, &mut page, 1.0, vec![key(Key::Enter, false)]).0, [StartAction::OpenRecent("/c.vrs".into())]);
}

#[test]
fn recent_row_menu_is_a_kit_menu_driven_by_arrows_enter_and_escape() {
    for ppp in [1.0, 2.0] {
        let ctx = context();
        let path = std::path::PathBuf::from("/gone.vrs");
        let mut page = StartPage::new(StartModel::without_recovery(&recents(&["/gone.vrs"]), 2, |_| true));
        let _ = frame(&ctx, &mut page, ppp, vec![]);
        let more = ctx.read_response(Id::new(("start-more", &path))).unwrap().rect.center();
        let click = |pressed| {
            vec![
                Event::PointerMoved(more),
                Event::PointerButton {
                    pos: more,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ]
        };
        let open = |page: &mut StartPage| {
            let _ = frame(&ctx, page, ppp, click(true));
            assert!(frame(&ctx, page, ppp, click(false)).0.is_empty());
            let _ = frame(&ctx, page, ppp, vec![]);
            assert!(varos_app::shell::kit::menu_open(&ctx));
            assert!(!egui::Popup::is_any_open(&ctx), "no egui default popup");
        };
        open(&mut page);
        // Esc closes; Start's own keys work again afterwards.
        assert!(frame(&ctx, &mut page, ppp, vec![key(Key::Escape, false), release(Key::Escape)]).0.is_empty());
        assert!(!varos_app::shell::kit::menu_open(&ctx));
        // ↓ Enter → Locate…; ↓ ↓ Enter → Remove from Recent (the separator is skipped).
        for (downs, expected) in [(1, StartAction::Locate(path.clone())), (2, StartAction::RemoveRecent(path.clone()))]
        {
            open(&mut page);
            let mut keys = vec![];
            for _ in 0..downs {
                keys.extend([key(Key::ArrowDown, false), release(Key::ArrowDown)]);
            }
            assert!(frame(&ctx, &mut page, ppp, keys).0.is_empty(), "arrows only move inside the menu");
            assert_eq!(frame(&ctx, &mut page, ppp, vec![key(Key::Enter, false), release(Key::Enter)]).0, [expected]);
            assert!(!varos_app::shell::kit::menu_open(&ctx));
        }
        // Clicking More again closes the open menu (toggle).
        open(&mut page);
        let _ = frame(&ctx, &mut page, ppp, click(true));
        let _ = frame(&ctx, &mut page, ppp, click(false));
        assert!(!varos_app::shell::kit::menu_open(&ctx));
    }
}

#[test]
fn recent_rows_and_actions_fit_narrow_and_wide_workspaces() {
    for width in [440.0, 800.0, 1460.0] {
        for ppp in [1.0, 2.0] {
            let ctx = context();
            let path = std::path::PathBuf::from("/Very long folder name/Another long folder/An unusually long document name that must stay inside its own row.vrs");
            let mut recents = Recents::default();
            recents.record(&path, None, 1);
            let mut page = StartPage::new(StartModel::without_recovery(&recents, 2, |_| true));
            for _ in 0..2 {
                frame_sized(&ctx, &mut page, ppp, vec![], egui::vec2(width, 900.0));
            }
            let row = ctx.read_response(Id::new(("start-recent", &path))).unwrap().rect;
            let more = ctx.read_response(Id::new(("start-more", &path))).unwrap().rect;
            assert!(row.left() >= 0.0 && more.right() <= width, "row/actions overflow at {width}");
            assert!(row.right() <= more.left());
            assert!(row.width() >= tokens::KIT_MIN_TARGET && more.width() >= tokens::KIT_MIN_TARGET);
            let pos = row.center();
            let pointer = |pressed| {
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ]
            };
            assert!(frame_sized(&ctx, &mut page, ppp, pointer(true), egui::vec2(width, 900.0)).0.is_empty());
            assert_eq!(
                frame_sized(&ctx, &mut page, ppp, pointer(false), egui::vec2(width, 900.0)).0,
                [StartAction::OpenRecent(path)]
            );
            let pos = more.center();
            let menu_pointer = |pressed| {
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ]
            };
            frame_sized(&ctx, &mut page, ppp, menu_pointer(true), egui::vec2(width, 900.0));
            assert!(
                frame_sized(&ctx, &mut page, ppp, menu_pointer(false), egui::vec2(width, 900.0)).0.is_empty(),
                "row menu must not open its document"
            );
            let (_, out) = frame_sized(&ctx, &mut page, ppp, vec![], egui::vec2(width, 900.0));
            let text: String = out
                .shapes
                .iter()
                .filter_map(|s| if let egui::Shape::Text(t) = &s.shape { Some(t.galley.text()) } else { None })
                .collect();
            assert!(text.contains("Locate…") && text.contains("Remove from Recent"), "missing-file menu is visible");
        }
    }
}

#[test]
fn unavailable_and_busy_recovery_actions_cannot_be_activated_by_keyboard_or_pointer() {
    for ppp in [1.0, 2.0] {
        for busy in [false, true] {
            let ctx = context();
            let mut page = StartPage::new(StartModel::build(
                &Recents::default(),
                0,
                |_| false,
                vec![RecoveryRow {
                    rid: "copy".into(),
                    name: "A long document title that must wrap safely inside the recovery section.vrs".into(),
                    original_dir: Some("/a/long/folder/path".into()),
                    saved_at_text: "Saved 14:32".into(),
                    problem: (!busy).then(|| "This recovery copy is damaged. The file is kept.".into()),
                    busy,
                }],
            ));
            let (_, out) = frame(&ctx, &mut page, ppp, vec![]);
            if !busy {
                assert!(out
                    .shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text().contains("damaged"))));
            }
            page.model.set_focus(2);
            assert!(frame(&ctx, &mut page, ppp, vec![key(Key::Enter, false)]).0.is_empty());
            let response = ctx.read_response(Id::new(("start-recovery", "copy", 0usize))).unwrap();
            assert!(!response.enabled());
            let pos = response.rect.center();
            for pressed in [true, false] {
                let events = vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ];
                assert!(frame(&ctx, &mut page, ppp, events).0.is_empty());
            }
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
            page.model.set_focus(3);
            let actions = frame(&ctx, &mut page, ppp, vec![key(Key::Enter, false)]).0;
            if busy {
                assert!(actions.is_empty());
            } else {
                assert_eq!(actions, [StartAction::DiscardRecovery("copy".into())]);
            }
        }
    }
}
