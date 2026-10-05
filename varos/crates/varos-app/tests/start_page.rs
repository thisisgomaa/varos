//! Start v2 — Boards: the Start page against the real `StartModel` (lane L2) — layout against the
//! mockup, actions by pointer and keyboard, the Missing menu, the empty and list states, and source
//! rules. Real `egui::Context` frames, no GPU, no window.
//! `VAROS_START_SNAPSHOT=<dir>` also rasterizes the four mockup states on the CPU (tests only).
use egui::{Context, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect};
use varos_app::{
    shell::{fonts, kit, tokens as t},
    start::{RecoveryRow, StartAction, StartModel, StartView},
    start_page::{self, demo, ids, layout, Shape, Slot, StartPage},
    storage::recents::Recents,
};
use varos_core::board::PresetId;

const W: f32 = 1512.0;
const H: f32 = 982.0;
const BAR: f32 = 28.0;
const RID: &str = "recovery-menu-card";

fn home() -> std::path::PathBuf {
    std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_else(|| "/home/designer".into())
}
fn path(i: usize) -> std::path::PathBuf {
    demo::path(&home(), i)
}
/// The card key of demo board `i` (the path, as text — L2's `BoardCard::key`).
fn key(i: usize) -> String {
    path(i).to_string_lossy().into_owned()
}
fn recent_model() -> StartModel {
    demo::model(&home(), Some(3))
}
fn empty_model() -> StartModel {
    StartModel::without_recovery(&Recents::default(), demo::NOW, |_| false)
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
fn shape_layout(m: &StartModel, size: egui::Vec2) -> start_page::PageLayout {
    layout(area(size), &Shape::of(m))
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
    fn frame(&mut self, m: &StartModel, events: Vec<Event>) -> (Vec<StartAction>, egui::FullOutput) {
        let mut actions = vec![];
        let a = area(self.size);
        let out = self.ctx.run_ui(input(self.ctx.pixels_per_point(), self.size, events), |ui| {
            actions = self.page.draw_in(ui, a, m, None);
        });
        (actions, out)
    }
    fn rect(&self, id: egui::Id) -> Rect {
        self.ctx.read_response(id).unwrap_or_else(|| panic!("no response for {id:?}")).rect
    }
    /// Press + release on `id`'s centre (hover first, so hover-only parts are drawn).
    fn click(&mut self, m: &StartModel, id: egui::Id) -> Vec<StartAction> {
        let pos = self.rect(id).center();
        self.click_at(m, pos, PointerButton::Primary)
    }
    fn click_at(&mut self, m: &StartModel, pos: Pos2, button: PointerButton) -> Vec<StartAction> {
        let e = |pressed| Event::PointerButton { pos, button, pressed, modifiers: Modifiers::NONE };
        let mut all = self.frame(m, vec![Event::PointerMoved(pos)]).0;
        all.extend(self.frame(m, vec![e(true)]).0);
        all.extend(self.frame(m, vec![e(false)]).0);
        all
    }
    fn key(&mut self, m: &StartModel, key: Key, modifiers: Modifiers) -> Vec<StartAction> {
        let e = |pressed| Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers };
        self.frame(m, vec![e(true), e(false)]).0
    }
    fn press(&mut self, m: &StartModel, key: Key) -> Vec<StartAction> {
        self.key(m, key, Modifiers::NONE)
    }
    /// Tab until a card has the focus.
    fn tab_to_cards(&mut self, m: &StartModel) {
        for _ in 0..40 {
            self.press(m, Key::Tab);
            if matches!(self.page.focus(), Slot::Card(_)) {
                return;
            }
        }
        panic!("Tab never reached a card");
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
    let l = shape_layout(&recent_model(), egui::vec2(W, H));
    assert_near("box", l.board, Rect::from_min_max(egui::pos2(12.0, 40.0), egui::pos2(1500.0, 950.0)));
    assert_near("status", l.status, Rect::from_min_max(egui::pos2(24.0, 950.0), egui::pos2(1488.0, 982.0)));
    assert_eq!((l.content.left(), l.content.width()), (52.0, 1408.0));
    assert!(!l.stacked);
    assert_near("New board", l.new_board, r(52.0, 72.0, 272.0, 64.0));
    assert_near("Open…", l.open, r(336.0, 72.0, 272.0, 64.0));
    assert_near("presets", l.presets, r(620.0, 72.0, 840.0, 152.0));
    assert_near("keys", l.keys, r(52.0, 206.0, 556.0, 18.0));
    assert_eq!(l.preset_cells.len(), 5);
    let square = Rect::from_min_max(egui::pos2(621.0, 105.0), egui::pos2(788.6, 223.0));
    assert_near("Square cell", l.preset_cells[0], square);
    assert_near("recovered", l.recovered[0], r(52.0, 252.0, 1408.0, 52.0));
    assert_near("head", l.head, r(52.0, 332.0, 1408.0, 28.0));
    assert_eq!((l.cols, l.card_w), (5, 272.0));
    assert_near("card 1", l.cards[0], r(52.0, 376.0, 272.0, 266.0));
    assert_near("card 2", l.cards[1], r(336.0, 376.0, 272.0, 266.0));
    assert_near("card 5", l.cards[4], r(1188.0, 376.0, 272.0, 266.0));
    assert_near("card 6", l.cards[5], r(52.0, 654.0, 272.0, 266.0));
    assert_near("well", start_page::card_well(l.cards[0]), r(53.0, 377.0, 270.0, 123.0));
    assert_near("… chip", start_page::card_chip(l.cards[6]), r(575.0, 663.0, 24.0, 24.0));
    for w in l.cards.windows(2).filter(|w| w[0].top() == w[1].top()) {
        assert_eq!(w[1].left() - w[0].right(), 12.0, "gutters are 12");
    }
    assert_eq!(l.cards[5].top() - l.cards[0].bottom(), 12.0);
    assert!(l.cards.last().unwrap().bottom() <= l.board.bottom() - t::SB_PAD_BOTTOM, "two rows fit the box");
}

/// The same rects as the real frame reports them (egui responses), at 1× and Retina 2×.
#[test]
fn painted_controls_sit_on_the_mockup_rects_at_1x_and_2x() {
    for ppp in [1.0, 2.0] {
        let m = recent_model();
        let mut p = Page::new(ppp, egui::vec2(W, H));
        p.frame(&m, vec![]);
        assert_near("New board", p.rect(ids::new_board()), r(52.0, 72.0, 272.0, 64.0));
        assert_near("Open…", p.rect(ids::open()), r(336.0, 72.0, 272.0, 64.0));
        let story = Rect::from_min_max(egui::pos2(956.2, 105.0), egui::pos2(1123.8, 223.0));
        assert_near("Story", p.rect(ids::preset(PresetId::Story)), story);
        assert_near("card 1", p.rect(ids::card(&key(0))), r(52.0, 376.0, 272.0, 266.0));
        assert_near("card 10", p.rect(ids::card(&key(9))), r(1188.0, 654.0, 272.0, 266.0));
        assert_near(
            "grid seg (24-tall hit rect)",
            p.rect(ids::view_segment(StartView::Grid)),
            r(1401.0, 334.0, 28.0, 24.0),
        );
        let recover = p.rect(ids::recover(RID));
        assert_eq!((recover.right(), recover.height()), (1448.0, 28.0), "Recover: 12 in from the band's right edge");
        let all = p.rect(ids::filter(None));
        assert_eq!((all.top(), all.height()), (332.0, 28.0));
    }
}

/// With the real Inter / JetBrains Mono: nothing that the mockup sets on one line wraps or is cut —
/// hero titles/sub-labels, preset names and sizes, card names/dates/facts/paths, filter labels.
#[test]
fn real_fonts_keep_the_mockup_lines_whole() {
    let m = recent_model();
    let mut p = Page::new(2.0, egui::vec2(W, H));
    p.frame(&m, vec![]);
    let (_, out) = p.frame(&m, vec![]);
    let galleys: Vec<_> = out
        .shapes
        .iter()
        .filter_map(|s| if let egui::Shape::Text(t) = &s.shape { Some((t.pos, t.galley.clone())) } else { None })
        .collect();
    let find = |s: &str| galleys.iter().find(|(_, g)| g.text() == s).unwrap_or_else(|| panic!("{s:?} not drawn"));
    for s in [
        "New board",
        "Free canvas, no size needed",
        "A .vrs file from disk",
        "1080 × 1920 px",
        "595 × 842 pt",
        "Custom…",
        "Recent boards",
        "Recover",
        "Ramadan campaign",
        "Logo marks v3",
        "ramadan",
        "social",
        "2 artboards",
        "free",
        "Recovery on · copies every 30 seconds",
    ] {
        let (_, g) = find(s);
        assert_eq!(g.rows.len(), 1, "{s:?} wraps");
        assert!(!g.text().contains('…') || s.contains('…'), "{s:?} is cut");
    }
    assert!(!galleys.iter().any(|(_, g)| g.text().starts_with('+')), "every card shows all its tags (no +n)");
    // the hero sub-label fits inside its button, the shortcut too, with the mockup's paddings
    let (pos, sub) = find("Free canvas, no size needed");
    assert!(pos.x + sub.size().x <= 52.0 + 272.0 - 16.0 - 18.0, "sub-label runs into ⌘N");
    // every preset size line fits its 167.6-wide cell
    for s in ["1080 × 1080 px", "1080 × 1350 px", "1080 × 1920 px"] {
        assert!(find(s).1.size().x < 160.0, "{s} too wide for its cell");
    }
    // the lede sets in two lines at 500 (the mockup's max-width)
    let lede = galleys.iter().find(|(_, g)| g.text().starts_with("A board is")).unwrap();
    assert_eq!(lede.1.rows.len(), 2);
}

/// The responsive rule at other window sizes: 3…6 columns, cards 272…320 from 3 columns up, the hero
/// stacks under 1128 of content, nothing overlaps, everything stays inside the box's content column.
#[test]
fn responsive_rule_at_other_window_sizes() {
    let m = recent_model();
    for (w, h, cols, stacked) in [
        (1100.0, 800.0, 3, true),
        (1280.0, 800.0, 4, false),
        (1800.0, 1100.0, 6, false),
        (2400.0, 1400.0, 6, false),
        (900.0, 700.0, 3, true),
    ] {
        let l = shape_layout(&m, egui::vec2(w, h));
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
        assert!(!l.presets.intersects(l.new_board) && !l.presets.intersects(l.open), "{w}: presets clear");
        assert!(l.presets.right() <= l.content.right() + 0.01);
        assert!(l.recovered[0].top() >= l.presets.bottom() + t::SB_SECTION_GAP - 0.01);
        assert!(l.presets.width() >= t::SB_PRESETS_MIN_W.min(l.content.width()));
    }
    assert_eq!(start_page::grid_columns(1660.0), (5, 320.0));
    assert_eq!(start_page::grid_columns(1692.0).0, 6);
    assert_eq!(start_page::grid_columns(1408.0), (5, 272.0));
}

#[test]
fn every_control_emits_its_action_by_pointer() {
    let m = recent_model();
    let mut p = Page::new(1.0, egui::vec2(W, H));
    p.frame(&m, vec![]);
    assert_eq!(p.click(&m, ids::new_board()), [StartAction::NewBoard]);
    assert_eq!(p.click(&m, ids::open()), [StartAction::Open]);
    for preset in [PresetId::Square, PresetId::Portrait, PresetId::Story, PresetId::A4, PresetId::Custom] {
        assert_eq!(p.click(&m, ids::preset(preset)), [StartAction::NewWithPreset(preset)]);
    }
    assert_eq!(p.click(&m, ids::discard(RID)), [StartAction::DiscardRecovery(RID.into())]);
    assert_eq!(p.click(&m, ids::recover(RID)), [StartAction::Recover(RID.into())]);
    assert_eq!(p.click(&m, ids::filter(Some("client"))), [StartAction::SetTagFilter(Some("client".into()))]);
    assert_eq!(p.click(&m, ids::filter(None)), [StartAction::SetTagFilter(None)]);
    assert_eq!(p.click(&m, ids::view_segment(StartView::List)), [StartAction::SetView(StartView::List)]);
    assert_eq!(p.click(&m, ids::card(&key(2))), [StartAction::OpenRecent(path(2))]);
    // the search pill emits Search on every change
    let mut got = None;
    let bar = r(W - 300.0, 2.0, t::SB_SEARCH_W, t::SB_SEARCH_H);
    let press = |pressed| Event::PointerButton {
        pos: bar.center(),
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    for events in [
        vec![],
        vec![Event::PointerMoved(bar.center()), press(true)],
        vec![press(false)],
        vec![Event::Text("ram".into())],
    ] {
        let ctx = p.ctx.clone();
        let _ = ctx.run_ui(input(1.0, egui::vec2(W, H), events), |ui| {
            if let Some(a) = p.page.search_box(ui, bar, &m) {
                got = Some(a);
            }
        });
    }
    assert_eq!(got, Some(StartAction::Search("ram".into())));
}

/// The page yields the keyboard to the Search field while it is typing (Tab / Delete stay in it).
#[test]
fn the_search_field_owns_the_keyboard_while_it_types() {
    let m = recent_model();
    let ctx = context(1.0);
    let mut page = StartPage::new();
    let bar = r(W - 300.0, 2.0, t::SB_SEARCH_W, t::SB_SEARCH_H);
    let run = |page: &mut StartPage, events: Vec<Event>| {
        let mut actions = vec![];
        let _ = ctx.run_ui(input(1.0, egui::vec2(W, H), events), |ui| {
            actions = page.draw_in(ui, area(egui::vec2(W, H)), &m, None);
            actions.extend(page.search_box(ui, bar, &m));
        });
        actions
    };
    run(&mut page, vec![]);
    ctx.memory_mut(|mem| mem.request_focus(ids::search()));
    run(&mut page, vec![]);
    let k =
        |key, pressed| Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers: Modifiers::NONE };
    assert!(run(&mut page, vec![k(Key::Tab, true), k(Key::Tab, false)]).is_empty());
    assert!(!page.ring_visible() && page.focus() == &Slot::New, "Tab stayed with the field");
    ctx.memory_mut(|mem| mem.request_focus(ids::search()));
    run(&mut page, vec![]);
    assert!(run(&mut page, vec![k(Key::Delete, true), k(Key::Delete, false)]).is_empty());
    assert!(!page.ring_visible(), "Delete stayed with the field");
}

#[test]
fn a_disabled_or_busy_recover_never_fires_and_leaves_the_tab_ring() {
    let mut row = demo::recovered();
    row.problem = Some("The copy is damaged".into());
    let build = |row: RecoveryRow| StartModel::build(&demo::recents(&home()), demo::NOW, |_| false, vec![row]);
    let m = build(row.clone());
    let mut p = Page::new(1.0, egui::vec2(W, H));
    p.frame(&m, vec![]);
    assert!(p.click(&m, ids::recover(RID)).is_empty());
    let order = start_page::tab_order(&m, None);
    assert!(order.contains(&Slot::Discard(RID.into())));
    assert!(!order.contains(&Slot::Recover(RID.into())));
    assert_eq!(start_page::activate(&Slot::Recover(RID.into()), &m), None, "keyboard cannot recover it either");
    row.busy = true;
    let m = build(row);
    assert!(p.click(&m, ids::discard(RID)).is_empty());
    assert!(!start_page::tab_order(&m, None).iter().any(|s| matches!(s, Slot::Discard(_))));
}

#[test]
fn keyboard_ring_order_activation_and_2d_grid_moves() {
    let m = recent_model();
    let mut p = Page::new(2.0, egui::vec2(W, H));
    let (_, out) = p.frame(&m, vec![]);
    assert!(!accent_stroke(&out), "no ring before the keyboard is used");
    p.press(&m, Key::Tab);
    assert_eq!(p.page.focus(), &Slot::New, "the first Tab lands on New board");
    let (_, out) = p.frame(&m, vec![]);
    assert!(accent_stroke(&out));
    assert_eq!(p.press(&m, Key::Enter), [StartAction::NewBoard]);
    assert_eq!(p.press(&m, Key::Space), [StartAction::NewBoard]);
    // a held key (repeat) never activates twice: press, press again while down, release
    let k = |pressed| Event::Key {
        key: Key::Enter,
        physical_key: Some(Key::Enter),
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    assert_eq!(p.frame(&m, vec![k(true)]).0, [StartAction::NewBoard]);
    assert!(p.frame(&m, vec![k(true)]).0.is_empty(), "the auto-repeat press is ignored");
    p.frame(&m, vec![k(false)]);
    let mut seen = vec![p.page.focus().clone()];
    for _ in 0..40 {
        p.press(&m, Key::Tab);
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
            Slot::Filter(_) | Slot::MoreFilters => "filter",
            Slot::View(_) => "view",
            Slot::Card(_) => "card",
        })
        .collect();
    let mut dedup = kinds.clone();
    dedup.dedup();
    assert_eq!(dedup, ["new", "open", "preset", "discard", "recover", "filter", "view", "card"]);
    assert_eq!(kinds.iter().filter(|k| **k == "preset").count(), 5);
    assert_eq!(p.page.focus(), &Slot::Card(key(0)));
    assert_eq!(p.press(&m, Key::Enter), [StartAction::OpenRecent(path(0))]);
    p.press(&m, Key::ArrowRight);
    assert_eq!(p.page.focus(), &Slot::Card(key(1)));
    p.press(&m, Key::ArrowDown);
    assert_eq!(p.page.focus(), &Slot::Card(key(6)), "↓ = one grid row (5 columns)");
    p.press(&m, Key::ArrowDown);
    assert_eq!(p.page.focus(), &Slot::Card(key(6)), "no row below: stays");
    p.press(&m, Key::ArrowUp);
    assert_eq!(p.page.focus(), &Slot::Card(key(1)));
    p.press(&m, Key::ArrowLeft);
    p.press(&m, Key::ArrowLeft);
    assert_eq!(p.page.focus(), &Slot::Card(key(0)), "← stops at the row's start");
    // Delete and the Mac delete (Backspace) both remove the focused board
    assert_eq!(p.press(&m, Key::Delete), [StartAction::RemoveRecent(path(0))]);
    assert_eq!(p.press(&m, Key::Backspace), [StartAction::RemoveRecent(path(0))]);
    // the host removed it and rebuilt the model: focus moves to the card now in its place
    let mut recents = demo::recents(&home());
    recents.remove(&path(0));
    let after = StartModel::build(&recents, demo::NOW, |p| p == path(3), vec![demo::recovered()]);
    p.frame(&after, vec![]);
    assert_eq!(p.page.focus(), &Slot::Card(key(1)));
    p.key(&after, Key::Tab, Modifiers::SHIFT);
    assert_eq!(p.page.focus(), &Slot::View(StartView::List));
    assert_eq!(p.press(&after, Key::Enter), [StartAction::SetView(StartView::List)]);
    p.press(&after, Key::ArrowLeft);
    assert_eq!(p.page.focus(), &Slot::View(StartView::Grid));
    p.press(&after, Key::ArrowLeft);
    assert_eq!(p.page.focus(), &Slot::View(StartView::Grid), "← does not leave the toggle");
    let pos = p.rect(ids::open()).center();
    let down = Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE };
    p.frame(&after, vec![down]);
    assert!(!p.page.ring_visible(), "a pointer press hides the ring");
}

#[test]
fn presets_filters_and_recovered_by_keyboard() {
    let m = recent_model();
    let mut p = Page::new(1.0, egui::vec2(W, H));
    p.frame(&m, vec![]);
    for _ in 0..3 {
        p.press(&m, Key::Tab);
    }
    assert_eq!(p.page.focus(), &Slot::Preset(PresetId::Square));
    p.press(&m, Key::ArrowRight);
    p.press(&m, Key::ArrowRight);
    assert_eq!(p.press(&m, Key::Enter), [StartAction::NewWithPreset(PresetId::Story)]);
    for _ in 0..3 {
        p.press(&m, Key::Tab);
    }
    assert_eq!(p.page.focus(), &Slot::Discard(RID.into()));
    p.press(&m, Key::ArrowRight);
    assert_eq!(p.press(&m, Key::Space), [StartAction::Recover(RID.into())]);
    p.press(&m, Key::Tab);
    assert_eq!(p.page.focus(), &Slot::Filter(None));
    p.press(&m, Key::ArrowRight);
    assert_eq!(p.press(&m, Key::Enter), [StartAction::SetTagFilter(Some("client".into()))]);
}

#[test]
fn filters_and_search_come_from_the_model() {
    let mut m = recent_model();
    assert!(m.apply(&StartAction::SetTagFilter(Some("print".into()))));
    let mut p = Page::new(1.0, egui::vec2(W, H));
    let (_, out) = p.frame(&m, vec![]);
    let l = shape_layout(&m, egui::vec2(W, H));
    assert_eq!(l.cards.len(), 3, "print: 3 boards");
    let azure: Vec<Rect> = rects(&out).into_iter().filter(|r| r.fill == t::ACCENT).map(|r| r.rect).collect();
    let print = p.rect(ids::filter(Some("print")));
    assert!(azure.len() == 1 && print.contains_rect(azure[0]), "the bar sits under the selected tag");
    m.apply(&StartAction::SetTagFilter(None));
    m.apply(&StartAction::Search("zzz".into()));
    let (_, out) = p.frame(&m, vec![]);
    assert!(texts(&out).iter().any(|s| s.starts_with("No boards match")));
}

#[test]
fn command_keys_only_when_the_page_owns_them() {
    let m = recent_model();
    let mut p = Page::new(1.0, egui::vec2(W, H));
    p.frame(&m, vec![]);
    assert!(p.key(&m, Key::N, Modifiers::COMMAND).is_empty(), "the app's host owns ⌘N (K2 row 1)");
    p.page.command_keys = true;
    assert_eq!(p.key(&m, Key::N, Modifiers::COMMAND), [StartAction::NewBoard]);
    assert_eq!(p.key(&m, Key::O, Modifiers::COMMAND), [StartAction::Open]);
}

#[test]
fn missing_card_menu_has_locate_others_only_remove_and_esc_closes() {
    let m = recent_model();
    assert_eq!(start_page::menu_entries(false).len(), 1);
    assert_eq!(start_page::menu_entries(true).len(), 3);
    let mut p = Page::new(1.0, egui::vec2(W, H));
    p.frame(&m, vec![]);
    let card = p.rect(ids::card(&key(3)));
    p.frame(&m, vec![Event::PointerMoved(card.center())]);
    assert_near("chip", p.rect(ids::chip(&key(3))), start_page::card_chip(card));
    let got = p.click(&m, ids::chip(&key(3)));
    assert!(got.is_empty(), "{got:?}");
    assert_eq!(p.page.menu_for(), Some(key(3).as_str()));
    assert!(kit::is_menu_open(&p.ctx, start_page::menu_owner(&key(3))));
    p.press(&m, Key::ArrowDown);
    assert_eq!(p.press(&m, Key::Enter), [StartAction::Locate(path(3))]);
    assert!(!kit::menu_open(&p.ctx));
    let card6 = p.rect(ids::card(&key(6)));
    p.frame(&m, vec![Event::PointerMoved(card6.center())]);
    p.click(&m, ids::chip(&key(6)));
    assert_eq!(p.page.menu_for(), Some(key(6).as_str()));
    assert!(p.press(&m, Key::Escape).is_empty());
    p.frame(&m, vec![]);
    assert!(!kit::menu_open(&p.ctx));
    let pos = p.rect(ids::card(&key(6))).center();
    p.click_at(&m, pos, PointerButton::Secondary);
    p.press(&m, Key::ArrowDown);
    assert_eq!(p.press(&m, Key::Enter), [StartAction::RemoveRecent(path(6))]);
    let (_, out) = p.frame(&m, vec![]);
    let text = texts(&out);
    assert!(text.iter().any(|s| s == "Missing") && text.iter().any(|s| s == "File not found"));
    let card3 = &m.cards()[3];
    let same_date = m.cards().iter().filter(|c| !c.missing && c.modified_text == card3.modified_text).count();
    assert_eq!(
        text.iter().filter(|s| **s == card3.modified_text).count(),
        same_date,
        "the Missing card's date is replaced"
    );
}

#[test]
fn first_launch_is_the_centred_empty_page() {
    let m = empty_model();
    assert!(start_page::is_first_launch(&m));
    let l = shape_layout(&m, egui::vec2(W, H));
    assert!(l.first_launch && l.cards.is_empty() && l.recovered.is_empty());
    assert_near("title", l.title, r(52.0, 264.0, 1408.0, 40.0));
    assert_near("New board", l.new_board, r(478.0, 396.0, 272.0, 64.0));
    assert_near("Open…", l.open, r(762.0, 396.0, 272.0, 64.0));
    assert_near("presets", l.presets, r(336.0, 554.0, 840.0, 152.0));
    let mut p = Page::new(1.0, egui::vec2(W, H));
    let (_, out) = p.frame(&m, vec![]);
    let text = texts(&out);
    for s in ["Start with a board", "New board", "Open…", "Return", "Custom…", "any size"] {
        assert!(text.iter().any(|t| t == s), "empty page shows {s:?}: {text:?}");
    }
    assert!(!text.iter().any(|t| t == "Recent boards" || t == "Move"));
    assert_near("New board painted", p.rect(ids::new_board()), l.new_board);
    assert_eq!(p.press(&m, Key::Enter), [StartAction::NewBoard], "Return on the untouched page = New board");
    assert_eq!(start_page::tab_order(&m, None).len(), 7);
}

#[test]
fn list_view_is_the_numbered_table() {
    let mut m = recent_model();
    m.apply(&StartAction::SetView(StartView::List));
    let l = shape_layout(&m, egui::vec2(W, H));
    assert!(l.cards.is_empty());
    assert_near("table head", l.table_head, r(52.0, 376.0, 1408.0, 32.0));
    assert_eq!(l.rows.len(), 10);
    assert_near("row 1", l.rows[0], r(52.0, 408.0, 1408.0, 50.0));
    assert_near("row 10", l.rows[9], r(52.0, 858.0, 1408.0, 50.0));
    assert_eq!(start_page::list_wide_column(1408.0), 300.0);
    let mut p = Page::new(1.0, egui::vec2(W, H));
    let (_, out) = p.frame(&m, vec![]);
    let text = texts(&out);
    let first_date = m.cards()[0].modified_text.clone();
    for s in ["#", "Name", "Tags", "Folder", "Modified", "1", "10", first_date.as_str()] {
        assert!(text.iter().any(|t| t == s), "list shows {s:?}");
    }
    assert_near("row painted", p.rect(ids::row(&key(0))), l.rows[0]);
    assert_eq!(p.click(&m, ids::row(&key(4))), [StartAction::OpenRecent(path(4))]);
    p.page.reset_focus();
    p.tab_to_cards(&m);
    assert_eq!(p.page.focus(), &Slot::Card(key(0)));
    p.press(&m, Key::ArrowDown);
    assert_eq!(p.page.focus(), &Slot::Card(key(1)), "↓ = one list row");
    p.press(&m, Key::ArrowRight);
    assert_eq!(p.page.focus(), &Slot::Card(key(1)), "←/→ do nothing in the list");
}

#[test]
fn folder_text_elides_in_the_middle_and_helpers_match_the_copy() {
    let home = std::path::Path::new("/Users/ahmed");
    let file = home.join("Design/Clients/Noor Foods/Ramadan 2026/Ramadan campaign.vrs");
    assert_eq!(start_page::folder_text(&file, Some(home)), "~/Design/Clients/Noor Foods/Ramadan 2026");
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
    assert_eq!(start_page::preset_size_text(PresetId::Portrait), "1080 × 1350 px");
    assert_eq!(start_page::preset_size_text(PresetId::A4), "595 × 842 pt");
    assert_eq!(start_page::preset_size_text(PresetId::Custom), "any size");
}

#[test]
fn only_tokens_colours_and_azure_only_for_focus_and_the_filter_bar() {
    assert_eq!(t::WELL_DOT, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 18));
    let m = recent_model();
    let mut p = Page::new(1.0, egui::vec2(W, H));
    let (_, out) = p.frame(&m, vec![]);
    let azure: Vec<Rect> = rects(&out).into_iter().filter(|r| r.fill == t::ACCENT).map(|r| r.rect).collect();
    assert_eq!(azure.len(), 1, "{azure:?}");
    assert_eq!(azure[0].height(), t::SB_FILTER_BAR);
    assert!(!accent_stroke(&out));
}

/// Every Start control's HIT rect is at least the kit minimum (24 pt) both ways — the view toggle's
/// 22-tall segments and the 24-square "…" chips included (review fix 1/2).
#[test]
fn every_start_control_hit_target_is_at_least_24pt() {
    let mut m = recent_model();
    let mut p = Page::new(1.0, egui::vec2(W, H));
    p.frame(&m, vec![]);
    let card = p.rect(ids::card(&key(6)));
    p.frame(&m, vec![Event::PointerMoved(card.center())]);
    let mut ids_to_check = vec![ids::new_board(), ids::open(), ids::discard(RID), ids::recover(RID), ids::filter(None)];
    ids_to_check.extend([PresetId::Square, PresetId::A4, PresetId::Custom].map(ids::preset));
    ids_to_check.extend([StartView::Grid, StartView::List].map(ids::view_segment));
    ids_to_check.extend(m.tags().iter().take(3).map(|tc| ids::filter(Some(tc.tag.as_str()))));
    ids_to_check.extend([ids::card(&key(0)), ids::chip(&key(6))]);
    for id in &ids_to_check {
        let r = p.rect(*id);
        assert!(r.width() >= 24.0 && r.height() >= 24.0, "{id:?} hit rect {r:?} is under 24 pt");
    }
    m.apply(&StartAction::SetView(StartView::List));
    p.frame(&m, vec![]);
    let row = p.rect(ids::row(&key(2)));
    p.frame(&m, vec![Event::PointerMoved(row.center())]);
    for id in [ids::row(&key(2)), ids::chip(&key(2))] {
        let r = p.rect(id);
        assert!(r.width() >= 24.0 && r.height() >= 24.0, "{id:?} hit rect {r:?} is under 24 pt");
    }
}

/// At a narrow window the filters that do not fit go behind "+N" (a kit menu with counts); the
/// selected filter is always shown, even when it would not fit (review fix 3).
#[test]
fn filters_that_do_not_fit_go_behind_plus_n_and_the_selected_one_stays_visible() {
    let size = egui::vec2(900.0, 900.0);
    let mut m = recent_model();
    let mut p = Page::new(1.0, size);
    p.frame(&m, vec![]);
    let plan = p.page.filter_plan().clone();
    assert!(!plan.hidden.is_empty(), "900 wide: some tags do not fit ({plan:?})");
    assert_eq!(plan.shown[0].0, None, "All is always first");
    let more = p.rect(ids::more_filters());
    assert!(more.right() <= start_page::layout(area(size), &Shape::of(&m)).head.right(), "the +N tab fits the row");
    // choose the LAST hidden tag from the +N menu
    let last = plan.hidden.last().unwrap().clone();
    assert!(p.click(&m, ids::more_filters()).is_empty());
    assert!(kit::is_menu_open(&p.ctx, ids::more_filters()));
    for _ in 0..plan.hidden.len() {
        p.press(&m, Key::ArrowDown);
    }
    assert_eq!(p.press(&m, Key::Enter), [StartAction::SetTagFilter(Some(last.clone()))]);
    m.apply(&StartAction::SetTagFilter(Some(last.clone())));
    p.frame(&m, vec![]);
    let plan = p.page.filter_plan().clone();
    assert!(plan.shown.iter().any(|(t, _)| t.as_deref() == Some(last.as_str())), "the selected tag is shown: {plan:?}");
    let bar = rects(&p.frame(&m, vec![]).1).into_iter().filter(|r| r.fill == t::ACCENT).map(|r| r.rect).next();
    assert!(bar.is_some_and(|b| p.rect(ids::filter(Some(last.as_str()))).contains_rect(b)), "its azure bar is visible");
    // the "+N" tab is a keyboard stop too: Enter opens its menu
    p.page.reset_focus();
    for _ in 0..40 {
        p.press(&m, Key::Tab);
        if p.page.focus() == &Slot::MoreFilters {
            break;
        }
    }
    assert_eq!(p.page.focus(), &Slot::MoreFilters);
    p.press(&m, Key::Enter);
    assert!(kit::is_menu_open(&p.ctx, ids::more_filters()), "Enter on +N opens its menu");
}

/// Derived text (elided paths, counts, dates, the HOME prefix) is rebuilt only when the model or the
/// width changed: identical frames rebuild nothing (review fix 4).
#[test]
fn identical_frames_rebuild_no_derived_text() {
    let mut m = recent_model();
    let mut p = Page::new(1.0, egui::vec2(W, H));
    p.frame(&m, vec![]);
    let builds = p.page.derived_builds();
    for _ in 0..3 {
        p.frame(&m, vec![]);
    }
    assert_eq!(p.page.derived_builds(), builds, "identical frames: no rebuild");
    let pos = p.rect(ids::card(&key(1))).center();
    p.frame(&m, vec![Event::PointerMoved(pos)]);
    assert_eq!(p.page.derived_builds(), builds, "a hover is not a model change");
    m.apply(&StartAction::SetTagFilter(Some("client".into())));
    p.frame(&m, vec![]);
    assert_eq!(p.page.derived_builds(), builds + 1, "a filter change rebuilds once");
    p.frame(&m, vec![]);
    assert_eq!(p.page.derived_builds(), builds + 1);
    p.size = egui::vec2(1300.0, H);
    p.frame(&m, vec![]);
    assert_eq!(p.page.derived_builds(), builds + 2, "a new width rebuilds once");
}

/// ⇧F10 on a focused card opens its "…" menu (egui reports no Menu key); the chip shows while the card
/// has the ring; list rows get the same chip on hover and focus (review fix 8).
#[test]
fn shift_f10_opens_a_focused_cards_menu_and_list_rows_have_the_chip() {
    let mut m = recent_model();
    let mut p = Page::new(1.0, egui::vec2(W, H));
    p.frame(&m, vec![]);
    p.tab_to_cards(&m);
    p.press(&m, Key::ArrowRight);
    p.press(&m, Key::ArrowRight);
    p.press(&m, Key::ArrowRight);
    assert_eq!(p.page.focus(), &Slot::Card(key(3)));
    p.frame(&m, vec![]);
    assert!(p.ctx.read_response(ids::chip(&key(3))).is_some(), "the focused card shows its chip");
    p.key(&m, Key::F10, Modifiers::SHIFT);
    p.frame(&m, vec![]);
    assert!(kit::is_menu_open(&p.ctx, start_page::menu_owner(&key(3))), "⇧F10 opened the menu");
    p.press(&m, Key::ArrowDown);
    assert_eq!(p.press(&m, Key::Enter), [StartAction::Locate(path(3))], "↓ Enter: Locate… (Missing)");
    // list rows: the chip on focus, ⇧F10 too
    m.apply(&StartAction::SetView(StartView::List));
    p.frame(&m, vec![]);
    p.press(&m, Key::ArrowDown);
    let focused = match p.page.focus() {
        Slot::Card(k) => k.clone(),
        other => panic!("{other:?}"),
    };
    p.frame(&m, vec![]);
    assert!(p.ctx.read_response(ids::chip(&focused)).is_some(), "the focused row shows its chip");
    p.key(&m, Key::F10, Modifiers::SHIFT);
    p.frame(&m, vec![]);
    assert!(kit::is_menu_open(&p.ctx, start_page::menu_owner(&focused)));
    p.press(&m, Key::Escape);
    p.page.reset_focus();
    let row = p.rect(ids::row(&key(5)));
    p.frame(&m, vec![Event::PointerMoved(row.center())]);
    assert!(p.ctx.read_response(ids::chip(&key(5))).is_some(), "a hovered row shows its chip");
    assert!(p.click(&m, ids::chip(&key(5))).is_empty());
    assert!(kit::is_menu_open(&p.ctx, start_page::menu_owner(&key(5))), "the row's chip opens its menu");
}

/// A test thumbnail source: board `i`'s key → a real PNG written once in a temp dir.
struct PngSource {
    dir: std::path::PathBuf,
    asked: std::sync::Mutex<Vec<String>>,
}
impl start_page::ThumbSource for PngSource {
    fn find(&self, key: &varos_app::start::ThumbKey, _modified: u64) -> Option<(std::path::PathBuf, bool)> {
        self.asked.lock().unwrap().push(key.0.clone());
        let file = self.dir.join("thumb.png");
        (key.0 == key_of(0) || key.0 == key_of(1)).then_some((file, true))
    }
}
fn key_of(i: usize) -> String {
    key(i)
}

/// The seam: the first frame draws the placeholder (never waits on disk), the worker decodes the PNG,
/// the texture appears; cards without a thumbnail keep the placeholder; textures are bounded and the
/// ones of removed boards are freed (review item 5).
#[test]
fn thumbnails_decode_off_the_ui_thread_and_appear_when_ready() {
    let dir = std::env::temp_dir().join(format!("varos-start-thumbs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    image::RgbaImage::from_pixel(544, 246, image::Rgba([200, 60, 40, 255])).save(dir.join("thumb.png")).unwrap();
    let source = std::sync::Arc::new(PngSource { dir: dir.clone(), asked: Default::default() });
    let m = recent_model();
    let mut p = Page::new(1.0, egui::vec2(W, H));
    p.page.set_thumb_source(source.clone());
    let (_, out) = p.frame(&m, vec![]);
    assert!(!textured(&out), "the first frame draws placeholders, no texture yet");
    let mut seen = false;
    for _ in 0..200 {
        std::thread::sleep(std::time::Duration::from_millis(10));
        let (_, out) = p.frame(&m, vec![]);
        if textured(&out) {
            seen = true;
            break;
        }
    }
    assert!(seen, "the decoded thumbnail is drawn once ready");
    assert_eq!(p.page.thumb_textures(), 2, "two boards have thumbnails; the rest stay placeholders");
    let asked = source.asked.lock().unwrap().len();
    p.frame(&m, vec![]);
    assert_eq!(source.asked.lock().unwrap().len(), asked, "a board is looked up once, not every frame");
    // a removed board's texture is freed
    let mut recents = demo::recents(&home());
    recents.remove(&path(0));
    let after = StartModel::build(&recents, demo::NOW, |p| p == path(3), vec![demo::recovered()]);
    p.frame(&after, vec![]);
    assert_eq!(p.page.thumb_textures(), 1);
    let _ = std::fs::remove_dir_all(dir);
}
/// A card-sized textured rect (a thumbnail; icons are textured too, but 13–20 pt).
fn textured(out: &egui::FullOutput) -> bool {
    out.shapes.iter().any(|s| match &s.shape {
        egui::Shape::Rect(r) => r.brush.is_some() && r.rect.width() > 100.0,
        _ => false,
    })
}

/// Ratchet: the new files hand-paint with the kit — no egui default widgets, no literal colours, no
/// float literals outside `const` items (0, ½, 1, 2 allowed), and nothing that animates.
#[test]
fn source_rules_no_default_widgets_no_literals_no_animation() {
    let files = [
        ("start_page.rs", include_str!("../src/start_page.rs")),
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
    std::fs::create_dir_all(&dir).unwrap();
    for state in ["recent", "hover", "list", "empty"] {
        let mut m = match state {
            "empty" => StartModel::without_recovery(&Recents::default(), demo::NOW, |_| false),
            _ => recent_model(),
        };
        if state == "list" {
            m.apply(&StartAction::SetView(StartView::List));
        }
        let mut p = Page::new(2.0, egui::vec2(W, H));
        // real thumbnails (L3's rasteriser, written by `thumbs::raster` tests with VAROS_START_THUMBS)
        if let Some(thumbs) = std::env::var_os("VAROS_START_THUMBS") {
            p.page.set_thumb_source(std::sync::Arc::new(DirSource(thumbs.into())));
        }
        let mut atlas = std::collections::HashMap::new();
        let mut events = vec![];
        let (_, out) = p.frame(&m, vec![]);
        apply_textures(&mut atlas, &out);
        if state == "hover" {
            let card = p.rect(ids::card(&key(3)));
            let (_, out) = p.frame(&m, vec![Event::PointerMoved(card.center())]);
            apply_textures(&mut atlas, &out);
            let chip = p.rect(ids::chip(&key(3))).center();
            for pressed in [true, false] {
                let e = Event::PointerButton {
                    pos: chip,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                };
                let (_, out) = p.frame(&m, vec![e]);
                apply_textures(&mut atlas, &out);
            }
            p.press(&m, Key::ArrowDown);
            p.press(&m, Key::ArrowDown);
            events.push(Event::PointerMoved(p.rect(ids::card(&key(6))).center()));
        }
        let (_, out) = p.frame(&m, events);
        apply_textures(&mut atlas, &out);
        for _ in 0..50 {
            // let the decode worker land (textures upload in a later frame)
            std::thread::sleep(std::time::Duration::from_millis(10));
            let (_, out) = p.frame(&m, vec![]);
            apply_textures(&mut atlas, &out);
        }
        let (_, out) = p.frame(&m, vec![]);
        apply_textures(&mut atlas, &out);
        let img = rasterize(&p.ctx, out, &atlas, 2.0);
        img.save(dir.join(format!("{state}.png"))).unwrap();
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

/// Snapshot thumbnails: demo board `i`'s key → `<dir>/<i>.png` when that file exists.
struct DirSource(std::path::PathBuf);
impl start_page::ThumbSource for DirSource {
    fn find(&self, key: &varos_app::start::ThumbKey, _modified: u64) -> Option<(std::path::PathBuf, bool)> {
        let i = (0..demo::BOARDS.len()).find(|&i| key.0 == key_of(i))?;
        let file = self.0.join(format!("{i}.png"));
        file.is_file().then_some((file, true))
    }
}

/// The decode is bounded: an oversize file is not read, a huge-dimension header is refused before any
/// pixel buffer exists, a truncated or corrupt PNG is refused — the placeholder, never a panic.
#[test]
fn thumbnail_decode_refuses_oversize_huge_and_corrupt_files() {
    let dir = std::env::temp_dir().join(format!("varos-thumb-bounds-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ok = dir.join("ok.png");
    image::RgbaImage::from_pixel(544, 246, image::Rgba([1, 2, 3, 255])).save(&ok).unwrap();
    assert!(start_page::decode_thumbnail(&ok).is_some(), "the expected size decodes");
    // over 2 MB on disk
    let big = dir.join("big.png");
    std::fs::write(&big, vec![0u8; start_page::THUMB_MAX_BYTES as usize + 1]).unwrap();
    assert!(start_page::decode_thumbnail(&big).is_none());
    // a small file whose header claims 60000 × 60000 (would be ~14 GB of RGBA)
    let mut huge = std::fs::read(&ok).unwrap();
    huge[16..20].copy_from_slice(&60_000u32.to_be_bytes()); // IHDR width
    huge[20..24].copy_from_slice(&60_000u32.to_be_bytes()); // IHDR height
    let huge_path = dir.join("huge.png");
    std::fs::write(&huge_path, &huge).unwrap();
    assert!(start_page::decode_thumbnail(&huge_path).is_none());
    // a real PNG just over the bound (1089 wide)
    let wide = dir.join("wide.png");
    image::RgbaImage::new(start_page::THUMB_MAX_W + 1, 10).save(&wide).unwrap();
    assert!(start_page::decode_thumbnail(&wide).is_none());
    // truncated, and plain garbage
    let bytes = std::fs::read(&ok).unwrap();
    let cut = dir.join("cut.png");
    std::fs::write(&cut, &bytes[..bytes.len() / 2]).unwrap();
    assert!(start_page::decode_thumbnail(&cut).is_none());
    let junk = dir.join("junk.png");
    std::fs::write(&junk, b"not a png at all").unwrap();
    assert!(start_page::decode_thumbnail(&junk).is_none());
    assert!(start_page::decode_thumbnail(&dir.join("absent.png")).is_none());
    let _ = std::fs::remove_dir_all(dir);
}

/// At the window's minimum width (800) with a 32-character selected tag, the selected tab is shown,
/// fits (elided, full name in its tooltip) and overlaps neither "+N" nor the view toggle.
#[test]
fn a_maximum_length_selected_tag_never_overlaps_at_the_minimum_width() {
    let long = "a".repeat(varos_core::board::MAX_TAG_CHARS);
    let mut recents = demo::recents(&home());
    let mut tags: Vec<String> = ["client", "ramadan", "social", "print", "logo"].map(String::from).to_vec();
    tags.push(long.clone());
    let summary = varos_app::storage::recents::BoardSummary {
        name: "Long".into(),
        description: String::new(),
        tags,
        artboards: 1,
    };
    recents.record_board(&home().join("Long.vrs"), None, demo::NOW, summary, demo::NOW);
    for size in [egui::vec2(800.0, 560.0), egui::vec2(560.0, 560.0)] {
        let mut m = StartModel::build(&recents, demo::NOW, |_| false, vec![]);
        m.apply(&StartAction::SetTagFilter(Some(long.clone())));
        let mut p = Page::new(1.0, size);
        p.frame(&m, vec![]);
        p.frame(&m, vec![]);
        let plan = p.page.filter_plan().clone();
        assert!(plan.shown.iter().any(|(t, _)| t.as_deref() == Some(long.as_str())), "{size:?}: selected shown");
        let sel = p.rect(ids::filter(Some(long.as_str())));
        let seg = p.rect(ids::view_segment(StartView::Grid));
        let mut right = sel.right();
        if !plan.hidden.is_empty() {
            let more = p.rect(ids::more_filters());
            assert!(sel.right() <= more.left() + 0.01, "{size:?}: selected {sel:?} overlaps +N {more:?}");
            right = more.right();
        }
        assert!(right <= seg.left() - t::SB_FILTERS_GAP + 0.01, "{size:?}: the row runs into the toggle");
    }
}

/// A decode that lands after its board left Recent is dropped (never uploaded).
#[test]
fn a_decode_landing_after_its_board_was_removed_is_dropped() {
    let dir = std::env::temp_dir().join(format!("varos-start-thumbs-late-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    image::RgbaImage::from_pixel(544, 246, image::Rgba([9, 9, 9, 255])).save(dir.join("thumb.png")).unwrap();
    let source = std::sync::Arc::new(PngSource { dir: dir.clone(), asked: Default::default() });
    let m = recent_model();
    let mut recents = demo::recents(&home());
    recents.remove(&path(0));
    recents.remove(&path(1));
    let without = StartModel::build(&recents, demo::NOW, |p| p == path(3), vec![demo::recovered()]);
    let mut p = Page::new(1.0, egui::vec2(W, H));
    p.page.set_thumb_source(source.clone());
    p.frame(&m, vec![]); // asks for boards 0 and 1
    p.frame(&without, vec![]); // both leave Recent before their decodes are uploaded
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(10));
        p.frame(&without, vec![]);
    }
    assert_eq!(p.page.thumb_textures(), 0, "late decodes for removed boards are not uploaded");
    let _ = std::fs::remove_dir_all(dir);
}
