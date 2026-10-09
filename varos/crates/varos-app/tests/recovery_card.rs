//! The editor's recovery card + Review panel (owner decision 2026-10-06, direction B): geometry against
//! the mockup (`recovery-B-canvas-card-*.png`, `review-panel.png`), the card → panel → closed flow, and
//! the layer that keeps clicks off the canvas. Real `egui::Context` frames with the app's fonts; no GPU,
//! no window.
use egui::{Context, Event, PointerButton, Pos2, RawInput, Rect};
use varos_app::{
    recovery_card::{self as rc, ids, ReviewRow},
    shell::{
        fonts,
        kit::field::{self, Edit, Label, NumberField},
        tokens as t,
    },
    start::StartAction,
};

const W: f32 = 1512.0;
const H: f32 = 982.0;
/// The mockup's Board box (`BOX_L` 12 · `BOX_R` 1200 · `BOX_B` 950) under the 52 band.
fn board() -> Rect {
    Rect::from_min_max(egui::pos2(12.0, 52.0), egui::pos2(1200.0, 950.0))
}
const FOOTER: &str = "Recovery on · copies every 30 seconds";

fn context() -> Context {
    let ctx = Context::default();
    varos_app::shell::kit::text::enable_trace(&ctx);
    fonts::install(&ctx);
    t::apply(&ctx);
    let _ = ctx.run_ui(input(vec![]), |_| {});
    ctx
}
fn input(events: Vec<Event>) -> RawInput {
    let mut input =
        RawInput { events, screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(W, H))), ..Default::default() };
    input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().native_pixels_per_point = Some(2.0);
    input
}
fn row(rid: &str, name: &str, when: &str, folder: Option<&str>) -> ReviewRow {
    ReviewRow {
        rid: rid.into(),
        name: name.into(),
        when: when.into(),
        folder: folder.map(Into::into),
        ..Default::default()
    }
}
/// What `Ui::wants_pointer` asks: is a floating (non-Background) layer under `p`? The test's root pass
/// is a Background layer, like the editor's box tree; the canvas owns only Background points.
fn over_ui(ctx: &Context, p: Pos2) -> bool {
    ctx.layer_id_at(p).is_some_and(|l| l.order != egui::Order::Background)
}
fn two() -> Vec<ReviewRow> {
    vec![
        row("menu", "Menu card", "unsaved changes from 11:48 today", Some("/Users/a/Design/Clients/Noor Foods")),
        row("untitled", "Untitled board", "never saved · started 11:52 today", None),
    ]
}

struct Rig {
    ctx: Context,
    board: Rect,
}
impl Rig {
    fn new(board: Rect) -> Self {
        Self { ctx: context(), board }
    }
    fn frame(&self, rows: &[ReviewRow], events: Vec<Event>) -> (Vec<StartAction>, egui::FullOutput) {
        let mut actions = vec![];
        let out = self.ctx.run_ui(input(events), |ui| actions = rc::show(ui.ctx(), self.board, rows, FOOTER));
        (actions, out)
    }
    fn rect(&self, id: egui::Id) -> Option<Rect> {
        self.ctx.read_response(id).map(|r| r.rect)
    }
    fn click(&self, rows: &[ReviewRow], id: egui::Id) -> Vec<StartAction> {
        let pos = self.rect(id).unwrap_or_else(|| panic!("{id:?} not drawn")).center();
        let ev = |pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let mut all = self.frame(rows, vec![Event::PointerMoved(pos)]).0;
        all.extend(self.frame(rows, vec![ev(true)]).0);
        all.extend(self.frame(rows, vec![ev(false)]).0);
        all
    }
    /// Every text the last frame painted.
    fn texts(ctx: &Context) -> Vec<String> {
        varos_app::shell::kit::text::paint_records(ctx).into_iter().map(|r| r.text).collect()
    }
}

#[test]
fn headline_is_singular_for_one_copy() {
    assert_eq!(rc::headline(1), "Recovered 1 unsaved copy");
    assert_eq!(rc::headline(2), "Recovered 2 unsaved copies");
    assert_eq!(rc::headline(7), "Recovered 7 unsaved copies");
}

/// The card: centred on the Board box, its bottom 24 above the box's bottom, 52 tall, its width the
/// band's paddings + the sentence + the two buttons (Later Ghost, Review Solid, 28 tall, gap 8, 12
/// from the right edge) — the mockup's 545-ish at this size.
#[test]
fn card_sits_centred_24_above_the_board_bottom_and_follows_the_sentence() {
    let r = Rig::new(board());
    r.frame(&two(), vec![]);
    let (_, _out) = r.frame(&two(), vec![]);
    let card = r.rect(ids::card()).expect("the card is drawn on a document tab");
    assert_eq!(card.height(), t::SB_RECOV_H);
    assert!((card.center().x - board().center().x).abs() <= 0.5, "centred (whole points): {card:?}");
    assert_eq!(card.bottom(), board().bottom() - t::RC_GAP);
    assert!((500.0..600.0).contains(&card.width()), "follows the sentence (mockup 545): {}", card.width());
    let (later, review) = (r.rect(ids::later()).unwrap(), r.rect(ids::review()).unwrap());
    assert_eq!(review.right(), card.right() - t::SB_RECOV_PAD_R);
    assert_eq!(later.right() + t::RC_BTN_GAP, review.left());
    assert_eq!((later.height(), review.height()), (t::SB_BTN_H, t::SB_BTN_H));
    assert!((later.center().y - card.center().y).abs() < 0.01 && (review.center().y - card.center().y).abs() < 0.01);
    let texts = Rig::texts(&r.ctx);
    for s in ["Recovered 2 unsaved copies", "from your last session", "Later", "Review"] {
        assert!(texts.iter().any(|t| t == s), "{s:?} not painted: {texts:?}");
    }
    // the card's width is exactly its parts
    let w = |s: &str, f: egui::FontId| r.ctx.fonts_mut(|fo| fo.layout_no_wrap(s.into(), f, t::TEXT)).size().x;
    let sentence = w("Recovered 2 unsaved copies", t::body_medium()) + t::SB_RECOV_TEXT_GAP + w(rc::SUBLINE, t::body());
    let btn = |s: &str| w(s, t::small_medium()) + 2.0 * t::SB_BTN_PAD;
    assert!((card.width() - rc::card_width(sentence, btn("Later"), btn("Review"))).abs() < 0.5);
}

#[test]
fn one_copy_reads_singular_and_a_narrow_box_drops_the_second_phrase() {
    let r = Rig::new(board());
    let one = vec![two().remove(0)];
    r.frame(&one, vec![]);
    let (_, _out) = r.frame(&one, vec![]);
    let texts = Rig::texts(&r.ctx);
    assert!(texts.iter().any(|t| t == "Recovered 1 unsaved copy"), "{texts:?}");
    let wide = r.rect(ids::card()).unwrap();

    let narrow = Rect::from_min_size(egui::pos2(300.0, 52.0), egui::vec2(560.0, 700.0));
    assert!(!rc::shows_subline(narrow) && rc::shows_subline(board()));
    let r = Rig::new(narrow);
    r.frame(&one, vec![]);
    let (_, _out) = r.frame(&one, vec![]);
    let texts = Rig::texts(&r.ctx);
    assert!(!texts.iter().any(|t| t == rc::SUBLINE), "under 600 the second phrase goes: {texts:?}");
    let card = r.rect(ids::card()).unwrap();
    assert!(card.width() < wide.width());
    assert!((card.center().x - narrow.center().x).abs() <= 0.5);
    assert!(card.left() >= narrow.left() + t::RC_GAP && card.right() <= narrow.right() - t::RC_GAP);
}

/// The pure placement rules: never wider than the box minus 24 a side; the panel is 620 (less in a
/// small box), grows up from the card's bottom anchor, and stops 24 under the box's top.
#[test]
fn card_and_panel_rects_stay_inside_the_board_box() {
    let tiny = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(300.0, 200.0));
    let c = rc::card_rect(tiny, 545.0);
    assert_eq!((c.left(), c.right(), c.bottom()), (24.0, 276.0, 176.0));
    let p = rc::panel_rect(board(), 2);
    assert_eq!(p.size(), egui::vec2(t::RC_PANEL_W, t::RC_HEAD_H + 2.0 * t::RC_ROW_H + t::RC_FOOT_H));
    assert_eq!((p.center().x, p.bottom()), (606.0, board().bottom() - t::RC_GAP));
    let many = rc::panel_rect(board(), 40);
    assert_eq!(many.top(), board().top() + t::RC_GAP, "too many rows: the panel stops under the box top");
    assert_eq!(rc::panel_rect(tiny, 1).width(), 300.0 - 2.0 * t::RC_GAP);
}

/// Review opens the panel IN PLACE (same bottom anchor, 620 wide) with one 56 row per copy; it raises
/// no command (it never goes Home). Restore / Discard raise Start's own actions.
#[test]
fn review_opens_the_panel_in_place_with_a_row_per_copy() {
    let r = Rig::new(board());
    let rows = two();
    r.frame(&rows, vec![]);
    assert!(!rc::review_open(&r.ctx));
    assert_eq!(r.click(&rows, ids::review()), [], "Review raises nothing: the panel opens here");
    assert!(rc::review_open(&r.ctx));
    let (_, _out) = r.frame(&rows, vec![]);
    assert!(!Rig::texts(&r.ctx).iter().any(|t| t == "Review" || t == "Later"), "the card is gone");
    let panel = r.rect(ids::panel()).expect("the panel replaces the card");
    assert_eq!(panel, rc::panel_rect(board(), 2));
    assert_eq!(panel.bottom(), board().bottom() - t::RC_GAP, "same bottom anchor as the card");
    let texts = Rig::texts(&r.ctx);
    for s in [
        "Recovered copies",
        "2",
        "Menu card",
        "unsaved changes from 11:48 today",
        "Untitled board",
        "never saved · started 11:52 today",
        "Restore",
        "Discard",
        FOOTER,
    ] {
        assert!(texts.iter().any(|t| t == s), "{s:?} not painted: {texts:?}");
    }
    assert!(!texts.iter().any(|t| t == "Recover"), "one word everywhere: Restore");
    for (i, rid) in ["menu", "untitled"].iter().enumerate() {
        let restore = r.rect(ids::restore(rid)).unwrap();
        let discard = r.rect(ids::discard(rid)).unwrap();
        let row_top = panel.top() + t::RC_HEAD_H + i as f32 * t::RC_ROW_H;
        assert!((restore.center().y - (row_top + t::RC_ROW_H / 2.0)).abs() < 0.6, "{rid}: centred in its 56 row");
        assert_eq!(discard.right() + t::RC_BTN_GAP, restore.left());
        assert!(restore.right() <= panel.right() - t::SB_RECOV_PAD_R + 0.01);
    }
    assert_eq!(r.click(&rows, ids::restore("menu")), [StartAction::Recover("menu".into())]);
    assert_eq!(r.click(&rows, ids::discard("untitled")), [StartAction::DiscardRecovery("untitled".into())]);
    assert!(rc::review_open(&r.ctx), "restore / discard keep the panel open while rows remain");
}

/// The host drops a restored / discarded copy's row; the panel shrinks with it, and when the last row
/// goes the panel goes (and a later notice starts as the card again).
#[test]
fn rows_leave_one_by_one_and_the_last_closes_the_panel() {
    let r = Rig::new(board());
    let mut rows = two();
    r.frame(&rows, vec![]);
    r.click(&rows, ids::review());
    r.frame(&rows, vec![]);
    rows.remove(0); // Menu card restored
    r.frame(&rows, vec![]);
    r.frame(&rows, vec![]);
    assert_eq!(r.rect(ids::panel()).unwrap(), rc::panel_rect(board(), 1));
    rows.clear(); // the last one discarded
    let (actions, _out) = r.frame(&rows, vec![]);
    assert!(actions.is_empty() && !rc::review_open(&r.ctx));
    assert!(Rig::texts(&r.ctx).is_empty(), "nothing is drawn without rows: {:?}", Rig::texts(&r.ctx));
    r.frame(&rows, vec![]);
    assert!(!over_ui(&r.ctx, board().center_bottom() - egui::vec2(0.0, 60.0)), "nothing left over the canvas");
}

/// Later (card) and × (panel) both answer Later and close the panel.
#[test]
fn later_and_the_close_cross_both_defer() {
    let r = Rig::new(board());
    let rows = two();
    r.frame(&rows, vec![]);
    assert_eq!(r.click(&rows, ids::later()), [StartAction::Later]);
    r.click(&rows, ids::review());
    r.frame(&rows, vec![]);
    assert_eq!(r.click(&rows, ids::close()), [StartAction::Later]);
    assert!(!rc::review_open(&r.ctx), "× closes the panel");
}

/// A busy row (its Restore / Discard is running) and a damaged one: the buttons say why and never fire.
#[test]
fn busy_and_damaged_rows_do_not_fire() {
    let r = Rig::new(board());
    let mut rows = two();
    rows[0].busy = true;
    rows[1].problem = Some("This recovery copy is damaged.".into());
    r.frame(&rows, vec![]);
    r.click(&rows, ids::review());
    let (_, _out) = r.frame(&rows, vec![]);
    assert!(Rig::texts(&r.ctx).iter().any(|t| t == "This recovery copy is damaged."), "{:?}", Rig::texts(&r.ctx));
    assert_eq!(r.click(&rows, ids::restore("menu")), []);
    assert_eq!(r.click(&rows, ids::discard("menu")), []);
    assert_eq!(r.click(&rows, ids::restore("untitled")), []);
    assert_eq!(r.click(&rows, ids::discard("untitled")), [StartAction::DiscardRecovery("untitled".into())]);
}

/// The card and the panel are a floating, non-Background layer covering exactly their rect — the
/// editor's pointer test (`Ui::wants_pointer`: any non-Background layer wins) gives those clicks to the
/// UI, never to the canvas; just outside, the canvas keeps them.
#[test]
fn card_and_panel_block_the_canvas_pointer() {
    let r = Rig::new(board());
    let rows = two();
    r.frame(&rows, vec![]);
    r.frame(&rows, vec![]);
    let card = r.rect(ids::card()).unwrap();
    for p in [card.center(), card.left_center() + egui::vec2(4.0, 0.0), card.right_bottom() - egui::vec2(2.0, 2.0)] {
        assert!(over_ui(&r.ctx, p), "the card's layer is under the pointer at {p:?}");
    }
    assert!(!over_ui(&r.ctx, card.center_top() - egui::vec2(0.0, 6.0)), "above the card: canvas");
    r.click(&rows, ids::review());
    r.frame(&rows, vec![]);
    let panel = r.rect(ids::panel()).unwrap();
    for p in [panel.center(), panel.left_top() + egui::vec2(3.0, 3.0), panel.center_bottom() - egui::vec2(0.0, 3.0)] {
        assert!(over_ui(&r.ctx, p), "the panel's layer is under the pointer at {p:?}");
    }
    assert!(!over_ui(&r.ctx, panel.left_center() - egui::vec2(6.0, 0.0)));
}

fn key(key: egui::Key) -> Vec<Event> {
    [true, false]
        .into_iter()
        .map(|pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Default::default() })
        .collect()
}

#[test]
fn tab_order_keyboard_activation_and_focus_ring() {
    let r = Rig::new(board());
    let rows = two();
    r.frame(&rows, vec![]);
    r.frame(&rows, vec![]);
    for id in [ids::later(), ids::review()] {
        let (_, out) = r.frame(&rows, key(egui::Key::Tab));
        assert_eq!(rc::focused(&r.ctx), Some(id));
        assert!(
            out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(rect) if rect.stroke.color == t::ACCENT)),
            "focus paints azure ring"
        );
    }
    assert!(r.frame(&rows, key(egui::Key::Space)).0.is_empty());
    assert!(rc::review_open(&r.ctx));
    for id in
        [ids::close(), ids::discard("menu"), ids::restore("menu"), ids::discard("untitled"), ids::restore("untitled")]
    {
        r.frame(&rows, key(egui::Key::Tab));
        assert_eq!(rc::focused(&r.ctx), Some(id));
    }
    assert_eq!(r.frame(&rows, key(egui::Key::Enter)).0, [StartAction::Recover("untitled".into())]);
    assert_eq!(r.click(&rows, ids::restore("untitled")), [StartAction::Recover("untitled".into())]);
    r.frame(&rows, key(egui::Key::Tab));
    assert_eq!(r.frame(&rows, key(egui::Key::Space)).0, [StartAction::Later]);
}

fn focus_ring(out: &egui::FullOutput) -> bool {
    out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(rect) if rect.stroke.color == t::ACCENT))
}

#[test]
fn escape_clears_card_and_panel_focus_and_unfocused_keys_do_not_activate() {
    let r = Rig::new(board());
    let rows = two();
    r.frame(&rows, vec![]);
    let (_, out) = r.frame(&rows, vec![]);
    assert_eq!(rc::focused(&r.ctx), None);
    assert!(!focus_ring(&out));
    for panel in [false, true] {
        if panel {
            r.click(&rows, ids::review());
        }
        let (_, out) = r.frame(&rows, key(egui::Key::Tab));
        assert!(focus_ring(&out));
        let (actions, out) = r.frame(&rows, key(egui::Key::Escape));
        assert!(actions.is_empty());
        assert_eq!(rc::focused(&r.ctx), None);
        assert!(!focus_ring(&out));
        for k in [egui::Key::Enter, egui::Key::Space] {
            assert!(r.frame(&rows, key(k)).0.is_empty());
            assert_eq!(rc::review_open(&r.ctx), panel);
        }
    }
}

#[test]
fn repeated_tabs_stay_in_the_card_and_walk_backwards() {
    let r = Rig::new(board());
    let rows = two();
    r.frame(&rows, vec![]);
    for (shift, expected) in [(false, ids::later()), (false, ids::review()), (true, ids::later())] {
        let events = vec![Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: true,
            modifiers: egui::Modifiers { shift, ..Default::default() },
        }];
        let _ = r.ctx.run_ui(input(events), |ui| {
            assert!(rc::show(ui.ctx(), r.board, &rows, FOOTER).is_empty());
            assert!(!ui.input(|i| i
                .events
                .iter()
                .any(|e| matches!(e, Event::Key { key: egui::Key::Tab, pressed: true, .. }))));
        });
        assert_eq!(rc::focused(&r.ctx), Some(expected));
    }
}

#[test]
fn properties_number_fields_keep_tab_and_enter_with_every_notice() {
    for mode in 0..3 {
        for shift in [false, true] {
            let r = Rig::new(board());
            let rows = two();
            r.frame(&rows, vec![]);
            if mode == 1 {
                r.click(&rows, ids::review());
            }
            let frame = |events| {
                let mut actions = vec![];
                let mut edits = vec![];
                let _ = r.ctx.run_ui(input(events), |ui| {
                    // Match the box tree: Board (notice) before the Properties column.
                    if mode == 2 {
                        assert!(!rc::show_restored(ui.ctx(), r.board, 42, "Restored copy — save it to keep it"));
                    } else {
                        actions = rc::show(ui.ctx(), r.board, &rows, FOOTER);
                    }
                    field::group(ui, "properties", |ui| {
                        for name in ["W", "H"] {
                            edits.push(field::number_field(
                                ui,
                                NumberField {
                                    id: egui::Id::new(name),
                                    width: 100.0,
                                    label: Label::Letter(name),
                                    tip: name,
                                    value: 12.0,
                                    decimals: 0,
                                    speed: 1.0,
                                    range: 0.0..=100.0,
                                    disabled: false,
                                },
                            ));
                        }
                    });
                });
                assert!(actions.is_empty(), "field input must not fire a recovery action");
                edits
            };
            frame(vec![]);
            frame(key(egui::Key::Tab)); // first acquire the notice's focus
            assert!(rc::focused(&r.ctx).is_some());
            r.ctx.memory_mut(|m| m.request_focus(egui::Id::new("W")));
            frame(vec![]);
            assert_eq!(rc::focused(&r.ctx), None, "a widget claiming focus clears the card");
            let at = frame(vec![])[0].rect.center();
            for pressed in [true, false] {
                frame(vec![
                    Event::PointerMoved(at),
                    Event::PointerButton {
                        pos: at,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ]);
            }
            assert!(frame(vec![])[0].editing);
            frame(vec![Event::Text("27".into())]);
            let mut tab = key(egui::Key::Tab);
            for event in &mut tab {
                if let Event::Key { modifiers, .. } = event {
                    modifiers.shift = shift;
                }
            }
            let edits: Vec<Edit<f32>> = frame(tab);
            assert_eq!(edits[0].commit, Some(27.0));
            assert!(edits[1].editing, "Tab/Shift+Tab must walk from W to H");
            assert_eq!(rc::focused(&r.ctx), None);
            frame(vec![Event::Text("35".into())]);
            let edits = frame(key(egui::Key::Enter));
            assert_eq!(edits[1].commit, Some(35.0));
            assert!(edits[1].closed);
            assert_eq!(rc::focused(&r.ctx), None);
            frame(key(egui::Key::Space));
            assert_eq!(rc::review_open(&r.ctx), mode == 1);
        }
    }
}

#[test]
fn panel_height_uses_the_same_inner_width_as_its_rows() {
    for (inner_width, row_height) in [(419.0, t::RC_STACK_ROW_H), (420.0, t::RC_ROW_H)] {
        let b = Rect::from_min_size(Pos2::ZERO, egui::vec2(inner_width + 2.0 * (t::RC_GAP + t::KIT_STROKE), H));
        assert_eq!(rc::panel_rect(b, 2).height(), t::RC_HEAD_H + 2.0 * row_height + t::RC_FOOT_H);
    }
}

fn assert_shapes_inside_and_text_clear(out: &egui::FullOutput, board: Rect, controls: &[Rect]) {
    for shape in &out.shapes {
        if let egui::Shape::Text(text) = &shape.shape {
            let painted = Rect::from_min_size(text.pos, text.galley.size()).intersect(shape.clip_rect);
            if !painted.is_positive() {
                continue;
            }
            assert!(board.contains_rect(painted), "text outside Board: {painted:?}");
            if !["Later", "Review", "Discard", "Restore"].contains(&text.galley.text()) {
                for button in controls {
                    assert!(
                        !painted.intersect(*button).is_positive(),
                        "text {:?} overlaps {button:?}",
                        text.galley.text()
                    );
                }
            }
        }
        let painted = shape.shape.visual_bounding_rect().intersect(shape.clip_rect);
        if painted.is_positive() {
            assert!(board.contains_rect(painted), "paint outside Board: {painted:?}");
        }
    }
}

#[test]
fn small_boards_compact_clip_and_scroll_without_collisions() {
    for size in [egui::vec2(224.0, 180.0), egui::vec2(300.0, 400.0), egui::vec2(600.0, 300.0), egui::vec2(224.0, 90.0)]
    {
        let board = Rect::from_min_size(egui::pos2(100.0, 100.0), size);
        let r = Rig::new(board);
        let rows = two();
        r.frame(&rows, vec![]);
        let (_, out) = r.frame(&rows, vec![]);
        let card = r.rect(ids::card()).unwrap();
        assert!(board.contains_rect(card));
        let controls: Vec<_> = [ids::later(), ids::review()].into_iter().filter_map(|id| r.rect(id)).collect();
        for rect in &controls {
            assert!(board.contains_rect(*rect));
        }
        assert_shapes_inside_and_text_clear(&out, board, &controls);
        r.click(&rows, ids::review());
        for _ in 0..8 {
            let (_, out) = r.frame(&rows, key(egui::Key::Tab));
            assert!(board.contains_rect(r.rect(ids::panel()).unwrap()));
            let controls: Vec<_> = r
                .ctx
                .interactive_rects_last_pass()
                .into_iter()
                .filter(|rect| rect.width() >= t::ICON_BTN_W && rect.height() <= t::SB_BTN_H)
                .collect();
            for rect in &controls {
                assert!(board.contains_rect(*rect), "control outside: {rect:?}");
            }
            assert_shapes_inside_and_text_clear(&out, board, &controls);
        }
    }
}

#[test]
fn restored_notice_keyboard_and_fallback_explanation() {
    let ctx = context();
    let notice =
        "Restored copy of Menu card — save it to keep it The newest copy was damaged; the previous copy was used.";
    let frame = |sid, events| {
        let mut save = false;
        let out = ctx.run_ui(input(events), |ui| save = rc::show_restored(ui.ctx(), board(), sid, notice));
        (save, out)
    };
    frame(42, vec![]);
    let (_, _out) = frame(42, vec![]);
    assert!(Rig::texts(&ctx).iter().any(|text| text == "The newest copy was damaged; the previous copy was used."));
    frame(42, key(egui::Key::Tab));
    assert_eq!(rc::focused(&ctx), Some(ids::later()));
    frame(42, key(egui::Key::Tab));
    assert_eq!(rc::focused(&ctx), Some(ids::save_as()));
    assert!(frame(42, key(egui::Key::Enter)).0);
    frame(42, key(egui::Key::Tab));
    frame(42, key(egui::Key::Space));
    frame(42, vec![]);
    assert!(Rig::texts(&ctx).is_empty());
    frame(43, vec![]);
    assert!(!Rig::texts(&ctx).is_empty());
}
