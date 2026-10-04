//! Start v2 — Boards (lane L4): layout against the mockup, intents by pointer and keyboard, the Missing
//! menu, the empty and list states, and source rules. Real `egui::Context` frames, no GPU, no window.
//! `VAROS_START_SNAPSHOT=<dir>` also rasterizes the four mockup states on the CPU (tests only).
use egui::{Context, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect};
use varos_app::{
    shell::{fonts, tokens as t},
    start_page::{self, ids, layout, Slot, StartPage},
    start_view::{demo, PresetId, StartIntent, StartView, ViewMode},
};

const W: f32 = 1512.0;
const H: f32 = 982.0;
const BAR: f32 = 28.0;

fn home() -> std::path::PathBuf {
    std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_else(|| "/home/designer".into())
}
fn recent_view() -> StartView {
    demo::view(&home(), Some(3), None, "")
}
fn context(ppp: f32) -> Context {
    let ctx = Context::default();
    fonts::install(&ctx);
    t::apply(&ctx);
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(theme, |s| s.text_styles = t::text_styles());
    }
    let _ = ctx.run_ui(input(ppp, egui::vec2(W, H), vec![]), |_| {});
    ctx
}
fn input(ppp: f32, size: egui::Vec2, events: Vec<Event>) -> RawInput {
    let mut input = RawInput { events, screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), ..Default::default() };
    input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().native_pixels_per_point = Some(ppp);
    input
}
fn area(size: egui::Vec2) -> Rect {
    Rect::from_min_max(egui::pos2(0.0, BAR), size.to_pos2())
}
struct Page {
    ctx: Context,
    page: StartPage,
    size: egui::Vec2,
}
impl Page {
    fn new(ppp: f32, size: egui::Vec2) -> Self {
        Self { ctx: context(ppp), page: StartPage::new(), size }
    }
    fn frame(&mut self, view: &StartView, events: Vec<Event>) -> (Vec<StartIntent>, egui::FullOutput) {
        let mut intents = vec![];
        let a = area(self.size);
        let out = self.ctx.run_ui(input(self.ctx.pixels_per_point(), self.size, events), |ui| {
            intents = self.page.draw_in(ui, a, view);
        });
        (intents, out)
    }
    fn rect(&self, id: egui::Id) -> Rect {
        self.ctx.read_response(id).unwrap_or_else(|| panic!("no response for {id:?}")).rect
    }
    /// Press + release on `id`'s centre (hover first, so hover-only parts are drawn).
    fn click(&mut self, view: &StartView, id: egui::Id) -> Vec<StartIntent> {
        let pos = self.rect(id).center();
        self.click_at(view, pos, PointerButton::Primary)
    }
    fn click_at(&mut self, view: &StartView, pos: Pos2, button: PointerButton) -> Vec<StartIntent> {
        let mut all = self.frame(view, vec![Event::PointerMoved(pos)]).0;
        all.extend(
            self.frame(view, vec![Event::PointerButton { pos, button, pressed: true, modifiers: Modifiers::NONE }]).0,
        );
        all.extend(
            self.frame(view, vec![Event::PointerButton { pos, button, pressed: false, modifiers: Modifiers::NONE }]).0,
        );
        all
    }
    fn key(&mut self, view: &StartView, key: Key, modifiers: Modifiers) -> Vec<StartIntent> {
        let e = |pressed| Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers };
        self.frame(view, vec![e(true), e(false)]).0
    }
    fn press(&mut self, view: &StartView, key: Key) -> Vec<StartIntent> {
        self.key(view, key, Modifiers::NONE)
    }
}
fn near(a: Rect, b: Rect) -> bool {
    (a.min - b.min).length() <= 1.0 && (a.max - b.max).length() <= 1.0
}
fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h))
}
#[track_caller]
fn assert_near(what: &str, got: Rect, want: Rect) {
    assert!(near(got, want), "{what}: got {got:?}, mockup {want:?}");
}

/// The mockup's numbers (src.html at 1512 × 982): box, hero, presets, Recovered, head, 5 × 272 grid.
#[test]
fn layout_matches_the_mockup_at_1512_by_982() {
    let v = recent_view();
    let l = layout(area(egui::vec2(W, H)), &v);
    assert_near("box", l.board, Rect::from_min_max(egui::pos2(12.0, 40.0), egui::pos2(1500.0, 950.0)));
    assert_near("status", l.status, Rect::from_min_max(egui::pos2(24.0, 950.0), egui::pos2(1488.0, 982.0)));
    assert_eq!((l.content.left(), l.content.width()), (52.0, 1408.0));
    assert!(!l.stacked);
    assert_near("New board", l.new_board, r(52.0, 72.0, 272.0, 64.0));
    assert_near("Open…", l.open, r(336.0, 72.0, 272.0, 64.0));
    assert_near("presets", l.presets, r(620.0, 72.0, 840.0, 152.0));
    assert_near("keys", l.keys, r(52.0, 206.0, 556.0, 18.0));
    assert_eq!(l.preset_cells.len(), 5);
    assert_near(
        "Square cell",
        l.preset_cells[0],
        Rect::from_min_max(egui::pos2(621.0, 105.0), egui::pos2(788.6, 223.0)),
    );
    assert_near("recovered", l.recovered[0], r(52.0, 252.0, 1408.0, 52.0));
    assert_near("head", l.head, r(52.0, 332.0, 1408.0, 28.0));
    assert_eq!((l.cols, l.card_w), (5, 272.0));
    assert_near("card 1", l.cards[0], r(52.0, 376.0, 272.0, 266.0));
    assert_near("card 2", l.cards[1], r(336.0, 376.0, 272.0, 266.0));
    assert_near("card 5", l.cards[4], r(1188.0, 376.0, 272.0, 266.0));
    assert_near("card 6", l.cards[5], r(52.0, 654.0, 272.0, 266.0));
    assert_near("well", start_page::card_well(l.cards[0]), r(53.0, 377.0, 270.0, 123.0));
    assert_near("… chip", start_page::card_chip(l.cards[6]), r(575.0, 663.0, 24.0, 24.0));
    // gutters are 12 everywhere in the grid
    for w in l.cards.windows(2).filter(|w| w[0].top() == w[1].top()) {
        assert_eq!(w[1].left() - w[0].right(), 12.0);
    }
    assert_eq!(l.cards[5].top() - l.cards[0].bottom(), 12.0);
    assert!(l.cards.last().unwrap().bottom() <= l.board.bottom() - t::SB_PAD_BOTTOM, "two rows fit the box");
}

/// The same rects as the real frame reports them (egui responses), at 1× and Retina 2×.
#[test]
fn painted_controls_sit_on_the_mockup_rects_at_1x_and_2x() {
    for ppp in [1.0, 2.0] {
        let v = recent_view();
        let mut p = Page::new(ppp, egui::vec2(W, H));
        p.frame(&v, vec![]);
        assert_near("New board", p.rect(ids::new_board()), r(52.0, 72.0, 272.0, 64.0));
        assert_near("Open…", p.rect(ids::open()), r(336.0, 72.0, 272.0, 64.0));
        assert_near(
            "Story",
            p.rect(ids::preset(PresetId::Story)),
            Rect::from_min_max(egui::pos2(956.2, 105.0), egui::pos2(1123.8, 223.0)),
        );
        assert_near("card 1", p.rect(ids::card("board-0")), r(52.0, 376.0, 272.0, 266.0));
        assert_near("card 10", p.rect(ids::card("board-9")), r(1188.0, 654.0, 272.0, 266.0));
        assert_near("grid seg", p.rect(ids::view_segment(ViewMode::Grid)), r(1401.0, 334.0, 28.0, 22.0));
        let recover = p.rect(ids::recover("recovery-menu-card"));
        assert_eq!((recover.right(), recover.height()), (1448.0, 28.0), "Recover: 12 in from the band's right edge");
        let all = p.rect(ids::filter(None));
        assert_eq!((all.top(), all.height()), (332.0, 28.0));
    }
}

/// The responsive rule at other window sizes: 3…6 columns, cards 272…320 from 3 columns up, the hero
/// stacks under 1128 of content, nothing overlaps, everything stays inside the box's content column.
#[test]
fn responsive_rule_at_other_window_sizes() {
    let v = recent_view();
    for (w, h, cols, stacked) in [
        (1100.0, 800.0, 3, true),
        (1280.0, 800.0, 4, false),
        (1800.0, 1100.0, 6, false),
        (2400.0, 1400.0, 6, false),
        (900.0, 700.0, 3, true),
    ] {
        let l = layout(area(egui::vec2(w, h)), &v);
        assert_eq!(l.cols, cols, "{w}: columns");
        assert_eq!(l.stacked, stacked, "{w}: stacked hero");
        if l.content.width() >= 3.0 * t::SB_COL + 2.0 * t::SB_GUTTER {
            assert!((t::SB_COL..=t::SB_COL_MAX).contains(&l.card_w), "{w}: card {}", l.card_w);
        }
        assert!(l.content.width() <= t::SB_CONTENT_MAX);
        assert!((l.content.center().x - l.board.center().x).abs() < 0.5, "{w}: content centred in the box");
        for c in &l.cards {
            assert!(c.left() >= l.content.left() && c.right() <= l.content.right() + 0.01, "{w}: card inside");
            assert_eq!(c.height(), t::SB_CARD_H);
        }
        for (i, a) in l.cards.iter().enumerate() {
            for b in &l.cards[i + 1..] {
                assert!(!a.intersects(*b), "{w}: cards overlap");
            }
        }
        let hero = [l.new_board, l.open, l.presets];
        assert!(!hero[2].intersects(l.new_board) && !hero[2].intersects(l.open), "{w}: presets clear of the buttons");
        assert!(l.presets.right() <= l.content.right() + 0.01);
        assert!(l.recovered[0].top() >= l.presets.bottom() + t::SB_SECTION_GAP - 0.01);
        assert!(l.presets.width() >= t::SB_PRESETS_MIN_W.min(l.content.width()));
    }
    // the gap ranges are honest: past 5 × 320 + 48, six columns only start at 6 × 272 + 60
    assert_eq!(start_page::grid_columns(1660.0), (5, 320.0));
    assert_eq!(start_page::grid_columns(1692.0).0, 6);
    assert_eq!(start_page::grid_columns(1408.0), (5, 272.0));
}

#[test]
fn every_control_emits_its_intent_by_pointer() {
    let v = recent_view();
    let mut p = Page::new(1.0, egui::vec2(W, H));
    p.frame(&v, vec![]);
    assert_eq!(p.click(&v, ids::new_board()), [StartIntent::NewBoard]);
    assert_eq!(p.click(&v, ids::open()), [StartIntent::Open]);
    for preset in PresetId::ALL {
        assert_eq!(p.click(&v, ids::preset(preset)), [StartIntent::NewWithPreset(preset)]);
    }
    assert_eq!(p.click(&v, ids::discard("recovery-menu-card")), [StartIntent::Discard("recovery-menu-card".into())]);
    assert_eq!(p.click(&v, ids::recover("recovery-menu-card")), [StartIntent::Recover("recovery-menu-card".into())]);
    assert_eq!(p.click(&v, ids::filter(Some("client"))), [StartIntent::SetTagFilter(Some("client".into()))]);
    assert_eq!(p.click(&v, ids::filter(None)), [StartIntent::SetTagFilter(None)]);
    assert_eq!(p.click(&v, ids::view_segment(ViewMode::List)), [StartIntent::SetView(ViewMode::List)]);
    assert_eq!(p.click(&v, ids::card("board-2")), [StartIntent::OpenBoard("board-2".into())]);
    // the search pill emits Search on every change
    let mut ctx_intent = None;
    let bar = r(W - 300.0, 2.0, t::SB_SEARCH_W, t::SB_SEARCH_H);
    for events in [
        vec![],
        vec![
            Event::PointerMoved(bar.center()),
            Event::PointerButton {
                pos: bar.center(),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
        vec![Event::PointerButton {
            pos: bar.center(),
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
        vec![Event::Text("ram".into())],
    ] {
        let ctx = p.ctx.clone();
        let _ = ctx.run_ui(input(1.0, egui::vec2(W, H), events), |ui| {
            if let Some(i) = p.page.search_box(ui, bar, &v) {
                ctx_intent = Some(i);
            }
        });
    }
    assert_eq!(ctx_intent, Some(StartIntent::Search("ram".into())));
}

#[test]
fn a_disabled_or_busy_recover_never_fires_and_leaves_the_tab_ring() {
    let mut v = recent_view();
    v.recovered[0].problem = Some("The copy is damaged".into());
    let mut p = Page::new(1.0, egui::vec2(W, H));
    p.frame(&v, vec![]);
    assert!(p.click(&v, ids::recover("recovery-menu-card")).is_empty());
    let order = start_page::tab_order(&v, usize::MAX);
    assert!(order.contains(&Slot::Discard("recovery-menu-card".into())));
    assert!(!order.contains(&Slot::Recover("recovery-menu-card".into())));
    v.recovered[0].busy = true;
    assert!(p.click(&v, ids::discard("recovery-menu-card")).is_empty());
    assert!(!start_page::tab_order(&v, usize::MAX).iter().any(|s| matches!(s, Slot::Discard(_))));
}

#[test]
fn keyboard_ring_order_activation_and_2d_grid_moves() {
    let v = recent_view();
    let mut p = Page::new(2.0, egui::vec2(W, H));
    let (_, out) = p.frame(&v, vec![]);
    assert!(!accent_stroke(&out), "no ring before the keyboard is used");
    // the first Tab lands on New board and the ring shows
    p.press(&v, Key::Tab);
    assert_eq!(p.page.focus(), &Slot::New);
    let (_, out) = p.frame(&v, vec![]);
    assert!(accent_stroke(&out));
    assert_eq!(p.press(&v, Key::Enter), [StartIntent::NewBoard]);
    assert_eq!(p.press(&v, Key::Space), [StartIntent::NewBoard]);
    // the documented order
    let mut seen = vec![p.page.focus().clone()];
    for _ in 0..40 {
        p.press(&v, Key::Tab);
        seen.push(p.page.focus().clone());
        if matches!(p.page.focus(), Slot::Card(_)) {
            break;
        }
    }
    let kinds: Vec<&str> = seen
        .iter()
        .map(|s| match s {
            Slot::New => "new",
            Slot::Open => "open",
            Slot::Preset(_) => "preset",
            Slot::Discard(_) => "discard",
            Slot::Recover(_) => "recover",
            Slot::Filter(_) => "filter",
            Slot::View(_) => "view",
            Slot::Card(_) => "card",
        })
        .collect();
    let mut dedup = kinds.clone();
    dedup.dedup();
    assert_eq!(dedup, ["new", "open", "preset", "discard", "recover", "filter", "view", "card"]);
    assert_eq!(kinds.iter().filter(|k| **k == "preset").count(), 5);
    assert_eq!(p.page.focus(), &Slot::Card("board-0".into()));
    assert_eq!(p.press(&v, Key::Enter), [StartIntent::OpenBoard("board-0".into())]);
    p.press(&v, Key::ArrowRight);
    assert_eq!(p.page.focus(), &Slot::Card("board-1".into()));
    p.press(&v, Key::ArrowDown);
    assert_eq!(p.page.focus(), &Slot::Card("board-6".into()), "↓ = one grid row (5 columns)");
    p.press(&v, Key::ArrowDown);
    assert_eq!(p.page.focus(), &Slot::Card("board-6".into()), "no row below: stays");
    p.press(&v, Key::ArrowUp);
    assert_eq!(p.page.focus(), &Slot::Card("board-1".into()));
    p.press(&v, Key::ArrowLeft);
    p.press(&v, Key::ArrowLeft);
    assert_eq!(p.page.focus(), &Slot::Card("board-0".into()), "← stops at the row's start");
    assert_eq!(p.press(&v, Key::Delete), [StartIntent::Remove("board-0".into())]);
    assert_eq!(p.press(&v, Key::Backspace), [StartIntent::Remove("board-0".into())]);
    // the model removed it: focus moves to the card now in its place
    let mut after = v.clone();
    after.cards.remove(0);
    p.frame(&after, vec![]);
    assert_eq!(p.page.focus(), &Slot::Card("board-1".into()));
    // ⇧Tab walks back into the view toggle; ←/→ stay inside it
    p.key(&after, Key::Tab, Modifiers::SHIFT);
    assert_eq!(p.page.focus(), &Slot::View(ViewMode::List));
    assert_eq!(p.press(&after, Key::Enter), [StartIntent::SetView(ViewMode::List)]);
    p.press(&after, Key::ArrowLeft);
    assert_eq!(p.page.focus(), &Slot::View(ViewMode::Grid));
    p.press(&after, Key::ArrowLeft);
    assert!(matches!(p.page.focus(), Slot::View(ViewMode::Grid)), "← does not leave the toggle");
    // a pointer press hides the ring
    let pos = p.rect(ids::open()).center();
    p.frame(
        &after,
        vec![Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }],
    );
    assert!(!p.page.ring_visible());
}

#[test]
fn presets_filters_and_recovered_by_keyboard() {
    let v = recent_view();
    let mut p = Page::new(1.0, egui::vec2(W, H));
    p.frame(&v, vec![]);
    p.press(&v, Key::Tab);
    p.press(&v, Key::Tab);
    p.press(&v, Key::Tab);
    assert_eq!(p.page.focus(), &Slot::Preset(PresetId::Square));
    p.press(&v, Key::ArrowRight);
    p.press(&v, Key::ArrowRight);
    assert_eq!(p.press(&v, Key::Enter), [StartIntent::NewWithPreset(PresetId::Story)]);
    for _ in 0..3 {
        p.press(&v, Key::Tab);
    }
    assert_eq!(p.page.focus(), &Slot::Discard("recovery-menu-card".into()));
    p.press(&v, Key::ArrowRight);
    assert_eq!(p.press(&v, Key::Space), [StartIntent::Recover("recovery-menu-card".into())]);
    p.press(&v, Key::Tab);
    assert_eq!(p.page.focus(), &Slot::Filter(None));
    p.press(&v, Key::ArrowRight);
    assert_eq!(p.press(&v, Key::Enter), [StartIntent::SetTagFilter(Some("client".into()))]);
}

#[test]
fn command_keys_only_when_the_page_owns_them() {
    let v = recent_view();
    let mut p = Page::new(1.0, egui::vec2(W, H));
    p.frame(&v, vec![]);
    assert!(p.key(&v, Key::N, Modifiers::COMMAND).is_empty(), "the app's host owns ⌘N (K2 row 1)");
    p.page.command_keys = true;
    assert_eq!(p.key(&v, Key::N, Modifiers::COMMAND), [StartIntent::NewBoard]);
    assert_eq!(p.key(&v, Key::O, Modifiers::COMMAND), [StartIntent::Open]);
}

#[test]
fn missing_card_menu_has_locate_others_only_remove_and_esc_closes() {
    let v = recent_view();
    assert_eq!(start_page::menu_entries(false).len(), 1);
    assert_eq!(start_page::menu_entries(true).len(), 3);
    let mut p = Page::new(1.0, egui::vec2(W, H));
    p.frame(&v, vec![]);
    // hover shows the "…" chip; clicking it opens the menu under it, right-aligned
    let card = p.rect(ids::card("board-3"));
    p.frame(&v, vec![Event::PointerMoved(card.center())]);
    let chip = p.rect(ids::chip("board-3"));
    assert_near("chip", chip, start_page::card_chip(card));
    let got = p.click(&v, ids::chip("board-3"));
    assert!(got.is_empty(), "{got:?}");
    assert_eq!(p.page.menu_for(), Some("board-3"));
    assert!(varos_app::shell::kit::is_menu_open(&p.ctx, start_page::menu_owner("board-3")));
    // keyboard inside the menu: ↓ Locate…, Enter
    p.press(&v, Key::ArrowDown);
    assert_eq!(p.press(&v, Key::Enter), [StartIntent::Locate("board-3".into())]);
    assert!(!varos_app::shell::kit::menu_open(&p.ctx));
    // reopen, Esc closes without an intent
    let card6 = p.rect(ids::card("board-6"));
    p.frame(&v, vec![Event::PointerMoved(card6.center())]);
    p.click(&v, ids::chip("board-6"));
    assert_eq!(p.page.menu_for(), Some("board-6"));
    assert!(p.press(&v, Key::Escape).is_empty());
    p.frame(&v, vec![]);
    assert!(!varos_app::shell::kit::menu_open(&p.ctx));
    // a non-missing card's menu is Remove only: ↓ Enter = Remove
    let pos = p.rect(ids::card("board-6")).center();
    p.click_at(&v, pos, PointerButton::Secondary);
    p.press(&v, Key::ArrowDown);
    assert_eq!(p.press(&v, Key::Enter), [StartIntent::Remove("board-6".into())]);
    // a Missing card shows the pill, not its date
    let (_, out) = p.frame(&v, vec![]);
    let text = texts(&out);
    assert!(text.iter().any(|s| s == "Missing") && text.iter().any(|s| s == "File not found"));
    assert!(!text.iter().any(|s| s == "Yesterday 16:10"), "the Missing card's date is replaced");
}

#[test]
fn first_launch_is_the_centred_empty_page() {
    let v = StartView::default();
    assert!(v.is_first_launch());
    let l = layout(area(egui::vec2(W, H)), &v);
    assert!(l.first_launch && l.cards.is_empty() && l.recovered.is_empty());
    assert_near("title", l.title, r(52.0, 264.0, 1408.0, 40.0));
    assert_near("New board", l.new_board, r(478.0, 396.0, 272.0, 64.0));
    assert_near("Open…", l.open, r(762.0, 396.0, 272.0, 64.0));
    assert_near("presets", l.presets, r(336.0, 554.0, 840.0, 152.0));
    let mut p = Page::new(1.0, egui::vec2(W, H));
    let (_, out) = p.frame(&v, vec![]);
    let text = texts(&out);
    for s in ["Start with a board", "New board", "Open…", "Return", "Custom…", "any size"] {
        assert!(text.iter().any(|t| t == s), "empty page shows {s:?}: {text:?}");
    }
    assert!(!text.iter().any(|t| t == "Recent boards" || t == "Move"));
    assert_near("New board painted", p.rect(ids::new_board()), l.new_board);
    // Return on the untouched page = New board (the hint)
    assert_eq!(p.press(&v, Key::Enter), [StartIntent::NewBoard]);
    // only hero + presets in the ring
    assert_eq!(start_page::tab_order(&v, usize::MAX).len(), 7);
}

#[test]
fn list_view_is_the_numbered_table() {
    let mut v = recent_view();
    v.view = ViewMode::List;
    let l = layout(area(egui::vec2(W, H)), &v);
    assert!(l.cards.is_empty());
    assert_near("table head", l.table_head, r(52.0, 376.0, 1408.0, 32.0));
    assert_eq!(l.rows.len(), 10);
    assert_near("row 1", l.rows[0], r(52.0, 408.0, 1408.0, 50.0));
    assert_near("row 10", l.rows[9], r(52.0, 858.0, 1408.0, 50.0));
    assert_eq!(start_page::list_wide_column(1408.0), 300.0);
    let mut p = Page::new(1.0, egui::vec2(W, H));
    let (_, out) = p.frame(&v, vec![]);
    let text = texts(&out);
    for s in ["#", "Name", "Tags", "Folder", "Modified", "1", "10", "Today 14:32"] {
        assert!(text.iter().any(|t| t == s), "list shows {s:?}");
    }
    assert_near("row painted", p.rect(ids::row("board-0")), l.rows[0]);
    assert_eq!(p.click(&v, ids::row("board-4")), [StartIntent::OpenBoard("board-4".into())]);
    // ↑/↓ move by one row in the list; ←/→ do nothing there
    p.page.reset_focus();
    for _ in 0..40 {
        p.press(&v, Key::Tab);
        if matches!(p.page.focus(), Slot::Card(_)) {
            break;
        }
    }
    assert_eq!(p.page.focus(), &Slot::Card("board-0".into()));
    p.press(&v, Key::ArrowDown);
    assert_eq!(p.page.focus(), &Slot::Card("board-1".into()));
    p.press(&v, Key::ArrowRight);
    assert_eq!(p.page.focus(), &Slot::Card("board-1".into()));
}

#[test]
fn folder_text_elides_in_the_middle_and_helpers_match_the_copy() {
    let home = std::path::Path::new("/Users/ahmed");
    let path = home.join("Design/Clients/Noor Foods/Ramadan 2026/Ramadan campaign.vrs");
    assert_eq!(start_page::folder_text(&path, Some(home)), "~/Design/Clients/Noor Foods/Ramadan 2026");
    assert_eq!(start_page::folder_text(std::path::Path::new("/tmp/a.vrs"), Some(home)), "/tmp");
    let full = "~/Design/Clients/Noor Foods/Ramadan 2026";
    assert_eq!(start_page::elide_middle(full, |s| s.chars().count() <= 60), full);
    let cut = start_page::elide_middle(full, |s| s.chars().count() <= 32);
    assert!(cut.starts_with("~/Design/") && cut.ends_with("/Ramadan 2026") && cut.contains("/…/"), "{cut}");
    let tight = start_page::elide_middle(full, |s| s.chars().count() <= 10);
    assert!(tight.chars().count() <= 10 && tight.contains('…'), "{tight}");
    assert_eq!(start_page::initials("Ramadan campaign"), "RC");
    assert_eq!(start_page::facts(0), "free");
    assert_eq!(start_page::facts(1), "1 artboard");
    assert_eq!(start_page::facts(2), "2 artboards");
    assert_eq!(start_page::version_text(), "Varos 0.1 α");
    assert_eq!(PresetId::Portrait.size_text(), "1080 × 1350 px");
    assert_eq!(PresetId::Custom.size_text(), "any size");
}

#[test]
fn only_tokens_colours_and_azure_only_for_focus_and_the_filter_bar() {
    assert_eq!(t::WELL_DOT, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 18));
    let v = recent_view();
    let mut p = Page::new(1.0, egui::vec2(W, H));
    let (_, out) = p.frame(&v, vec![]);
    // at rest, azure appears exactly once: the selected filter's 2 px bar
    let azure: Vec<Rect> = rects(&out).into_iter().filter(|r| r.fill == t::ACCENT).map(|r| r.rect).collect();
    assert_eq!(azure.len(), 1, "{azure:?}");
    assert_eq!(azure[0].height(), t::SB_FILTER_BAR);
    assert!(!accent_stroke(&out));
}

/// Ratchet: the new files hand-paint with the kit — no egui default widgets, no literal colours, no
/// float literals outside `const` items (0, ½, 1, 2 allowed), and nothing that animates.
#[test]
fn source_rules_no_default_widgets_no_literals_no_animation() {
    let files = [
        ("start_page.rs", include_str!("../src/start_page.rs")),
        ("start_view.rs", include_str!("../src/start_view.rs")),
        ("kit/board.rs", include_str!("../src/shell/kit/board.rs")),
    ];
    let banned = [
        "ui.button(",
        "ui.label(",
        "egui::Button",
        "egui::Label",
        "ui.add(",
        "RichText",
        "ui.heading(",
        "selectable_",
        "checkbox",
        "radio",
        "ui.separator(",
        "TextEdit",
        "ComboBox",
        "Slider",
        "DragValue",
        "hyperlink",
        "CollapsingHeader",
        "egui::Window",
        "Color32::from_",
        "Color32::from_gray",
        "0x",
        "animate_",
        "animation_time",
        "request_repaint_after",
        "Instant",
        "stable_dt",
        "i.time",
    ];
    for (name, src) in files {
        for (n, line) in src.lines().enumerate() {
            let code = strip_strings(line.split("//").next().unwrap());
            let code = code.as_str();
            for b in banned {
                assert!(!code.contains(b), "{name}:{}: `{b}` in {line:?}", n + 1);
            }
            let trimmed = code.trim_start();
            if trimmed.starts_with("pub const") || trimmed.starts_with("const") {
                continue;
            }
            for lit in float_literals(code) {
                assert!(
                    ["0.0", "0.5", "1.0", "2.0"].contains(&lit.as_str()),
                    "{name}:{}: literal {lit} in {line:?}",
                    n + 1
                );
            }
        }
    }
    assert_eq!(float_literals("a(12.0, x.0, 0.5, v2)"), ["12.0", "0.5"]);
    assert_eq!(strip_strings(r#"f("1.5 \"x\" 2.5", 3.0)"#), r#"f("", 3.0)"#);
}

/// The line with the contents of its string literals blanked (copy may hold numbers: "1.5 stroke").
fn strip_strings(line: &str) -> String {
    let mut out = String::new();
    let mut in_str = false;
    let mut escaped = false;
    for c in line.chars() {
        if in_str {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
                out.push(c);
            }
            continue;
        }
        if c == '"' {
            in_str = true;
        }
        out.push(c);
    }
    out
}

fn float_literals(code: &str) -> Vec<String> {
    let b = code.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        let prev_ok = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_' || b[i - 1] == b'.');
        if b[i].is_ascii_digit() && prev_ok {
            let s = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i + 1 < b.len() && b[i] == b'.' && b[i + 1].is_ascii_digit() {
                i += 1;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                out.push(code[s..i].to_string());
            }
        } else {
            i += 1;
        }
    }
    out
}

fn rects(out: &egui::FullOutput) -> Vec<egui::epaint::RectShape> {
    out.shapes.iter().filter_map(|s| if let egui::Shape::Rect(r) = &s.shape { Some(r.clone()) } else { None }).collect()
}
fn accent_stroke(out: &egui::FullOutput) -> bool {
    rects(out).iter().any(|r| r.stroke.color == t::ACCENT && r.stroke.width > 0.0)
}
fn texts(out: &egui::FullOutput) -> Vec<String> {
    out.shapes
        .iter()
        .filter_map(|s| if let egui::Shape::Text(t) = &s.shape { Some(t.galley.text().to_string()) } else { None })
        .collect()
}

// ───────────────────────────── CPU snapshot (tests only) ─────────────────────────────

/// Rasterize the four mockup states at 2× when `VAROS_START_SNAPSHOT=<dir>` is set (for eyes only:
/// a plain triangle rasterizer over egui's own tessellation, gamma-space blending, no GPU).
#[test]
fn snapshot_the_mockup_states_on_the_cpu() {
    let Some(dir) = std::env::var_os("VAROS_START_SNAPSHOT") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let recent = recent_view();
    for state in ["recent", "hover", "list", "empty"] {
        let mut v = match state {
            "empty" => StartView::default(),
            _ => recent.clone(),
        };
        if state == "list" {
            v.view = ViewMode::List;
        }
        let mut p = Page::new(2.0, egui::vec2(W, H));
        let mut atlas = std::collections::HashMap::new();
        let mut events = vec![];
        let (_, out) = p.frame(&v, vec![]);
        apply_textures(&mut atlas, &out);
        if state == "hover" {
            let card = p.rect(ids::card("board-3"));
            let (_, out) = p.frame(&v, vec![Event::PointerMoved(card.center())]);
            apply_textures(&mut atlas, &out);
            let chip = p.rect(ids::chip("board-3")).center();
            for pressed in [true, false] {
                let (_, out) = p.frame(
                    &v,
                    vec![Event::PointerButton {
                        pos: chip,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    }],
                );
                apply_textures(&mut atlas, &out);
            }
            p.press(&v, Key::ArrowDown);
            p.press(&v, Key::ArrowDown);
            let card6 = p.rect(ids::card("board-6")).center();
            events.push(Event::PointerMoved(card6));
        }
        let (_, out) = p.frame(&v, events);
        apply_textures(&mut atlas, &out);
        let (_, out) = p.frame(&v, vec![]);
        apply_textures(&mut atlas, &out);
        let img = rasterize(&p.ctx, out, &atlas, 2.0);
        img.save(dir.join(format!("start-{state}.png"))).unwrap();
    }
}

fn apply_textures(atlas: &mut std::collections::HashMap<egui::TextureId, egui::ColorImage>, out: &egui::FullOutput) {
    for (id, delta) in &out.textures_delta.set {
        let egui::ImageData::Color(img) = &delta.image;
        match delta.pos {
            None => {
                atlas.insert(*id, (**img).clone());
            }
            Some([x0, y0]) => {
                let dst = atlas.get_mut(id).expect("patch to a known texture");
                for y in 0..img.size[1] {
                    for x in 0..img.size[0] {
                        dst.pixels[(y0 + y) * dst.size[0] + x0 + x] = img.pixels[y * img.size[0] + x];
                    }
                }
            }
        }
    }
}

fn rasterize(
    ctx: &Context,
    out: egui::FullOutput,
    atlas: &std::collections::HashMap<egui::TextureId, egui::ColorImage>,
    ppp: f32,
) -> image::RgbaImage {
    let (w, h) = ((W * ppp) as u32, (H * ppp) as u32);
    let mut px = vec![[0f32; 4]; (w * h) as usize];
    for prim in ctx.tessellate(out.shapes, ppp) {
        let egui::epaint::Primitive::Mesh(mesh) = prim.primitive else { continue };
        let clip = prim.clip_rect;
        let (cx0, cy0) = ((clip.min.x * ppp).max(0.0) as i32, (clip.min.y * ppp).max(0.0) as i32);
        let (cx1, cy1) = ((clip.max.x * ppp).min(w as f32) as i32, (clip.max.y * ppp).min(h as f32) as i32);
        let tex = atlas.get(&mesh.texture_id);
        for tri in mesh.indices.chunks(3) {
            let v = [mesh.vertices[tri[0] as usize], mesh.vertices[tri[1] as usize], mesh.vertices[tri[2] as usize]];
            let p: Vec<(f32, f32)> = v.iter().map(|v| (v.pos.x * ppp, v.pos.y * ppp)).collect();
            let area = (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[2].0 - p[0].0) * (p[1].1 - p[0].1);
            if area.abs() < 1e-6 {
                continue;
            }
            let x0 = (p.iter().map(|q| q.0).fold(f32::MAX, f32::min).floor() as i32).max(cx0);
            let x1 = (p.iter().map(|q| q.0).fold(f32::MIN, f32::max).ceil() as i32).min(cx1);
            let y0 = (p.iter().map(|q| q.1).fold(f32::MAX, f32::min).floor() as i32).max(cy0);
            let y1 = (p.iter().map(|q| q.1).fold(f32::MIN, f32::max).ceil() as i32).min(cy1);
            for y in y0..y1 {
                for x in x0..x1 {
                    let (sx, sy) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = ((p[1].0 - sx) * (p[2].1 - sy) - (p[2].0 - sx) * (p[1].1 - sy)) / area;
                    let w1 = ((p[2].0 - sx) * (p[0].1 - sy) - (p[0].0 - sx) * (p[2].1 - sy)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let mut c = [0f32; 4];
                    for (k, c) in c.iter_mut().enumerate() {
                        *c =
                            (v[0].color[k] as f32 * w0 + v[1].color[k] as f32 * w1 + v[2].color[k] as f32 * w2) / 255.0;
                    }
                    if let Some(tex) = tex {
                        let u = v[0].uv.x * w0 + v[1].uv.x * w1 + v[2].uv.x * w2;
                        let vv = v[0].uv.y * w0 + v[1].uv.y * w1 + v[2].uv.y * w2;
                        let tx = ((u * tex.size[0] as f32) as usize).min(tex.size[0] - 1);
                        let ty = ((vv * tex.size[1] as f32) as usize).min(tex.size[1] - 1);
                        let s = tex.pixels[ty * tex.size[0] + tx];
                        for (k, c) in c.iter_mut().enumerate() {
                            *c *= s[k] as f32 / 255.0;
                        }
                    }
                    let d = &mut px[(y as u32 * w + x as u32) as usize];
                    for k in 0..4 {
                        d[k] = c[k] + d[k] * (1.0 - c[3]);
                    }
                }
            }
        }
    }
    image::RgbaImage::from_fn(w, h, |x, y| {
        let d = px[(y * w + x) as usize];
        image::Rgba([(d[0] * 255.0) as u8, (d[1] * 255.0) as u8, (d[2] * 255.0) as u8, 255])
    })
}
