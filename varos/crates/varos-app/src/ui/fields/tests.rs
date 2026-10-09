//! K3 field law (piece P2) — headless: real `egui::Context` frames through the REAL panel bodies
//! (`panel_artboard`, `panel_properties`), the frame's ops through `finish_frame` + `apply_ops` exactly as
//! `Ui::run` runs them, and `settle` exactly as `Ui::settle` / the canvas press run it. No window, no GPU.
use std::cell::RefCell;

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput};
use varos_app::shell::kit::field as kf;
use varos_core::editor::{Editor, ToolKind};

use super::super::{
    apply_ops, board_ctlbar, panel_artboard, panel_properties, set_doc_salt, AbSnap, DockIcons, Op, Snap,
};
use super::{finish_frame, settle, Pending};
use crate::app_command::SessionId;

thread_local! {
    /// Where each field sat in the last frame, by its name (`tip` / `id_src`).
    static PROBE: RefCell<Vec<(String, egui::Rect)>> = const { RefCell::new(vec![]) };
}
pub(crate) fn probe(name: &str, rect: egui::Rect) {
    PROBE.with(|p| p.borrow_mut().push((name.to_string(), rect)));
}

pub(crate) fn probed_rect(name: &str, index: usize) -> egui::Rect {
    PROBE
        .with(|p| p.borrow().iter().filter(|(field, _)| field == name).nth(index).map(|(_, rect)| *rect))
        .unwrap_or_else(|| panic!("field {name:?} #{index} was not laid out"))
}

pub(crate) fn probe_count(name: &str) -> usize {
    PROBE.with(|p| p.borrow().iter().filter(|(field, _)| field == name).count())
}
pub(crate) fn clear_probes() {
    PROBE.with(|p| p.borrow_mut().clear());
}

#[derive(Clone, Copy, PartialEq)]
enum View {
    Artboard,
    Properties,
    /// Nothing with fields (the selection moved on / the tool changed the panel).
    Empty,
    /// The control bar (on the board, right half) AND the Properties pane (left half): mirrored fields.
    Both,
    /// The real box tree (`ShellState::standard`) hosting the Artboard inspector in the Properties pane.
    Shell,
    /// The Artboard inspector on the frame's FIRST pass only, which asks egui for a second pass (a
    /// layout change mid-frame): the field is gone in the pass the frame ends with.
    FirstPassOnly,
}

/// One document tab's panel in a bare context, plus the state `Ui` keeps between frames.
struct Bench {
    ctx: egui::Context,
    ed: Editor,
    doc: Option<SessionId>,
    pending: Option<Pending>,
    view: View,
    t: f64,
    /// An extra button drawn BEFORE the panel whose click selects these node ids (a Layers row click:
    /// the click that blurs the field also changes the selection, in the same frame).
    select_on_click: Option<Vec<u32>>,
    select_at: Pos2,
    /// A chrome button drawn before the panel that raises this op (e.g. Delete artboard).
    press_op: Option<fn() -> Op>,
    /// The Properties reference point (A7) — (1, 1) = bottom-right, so its X is the RIGHT edge.
    refpt: (f32, f32),
    shell: varos_app::shell::ShellState,
}

impl Bench {
    fn new(ed: Editor, view: View) -> Self {
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx); // the tag chips set in the named Inter 500 face
        let mut b = Bench {
            ctx,
            ed,
            doc: Some(SessionId(1)),
            pending: None,
            view,
            t: 10.0,
            select_on_click: None,
            select_at: Pos2::ZERO,
            press_op: None,
            refpt: (0.0, 0.0),
            shell: varos_app::shell::ShellState::standard(),
        };
        b.frame(vec![]); // egui hit-tests against the previous pass's widgets: lay out once
        b
    }

    /// One `Ui::run`-shaped frame.
    fn frame(&mut self, events: Vec<Event>) {
        self.t += 1.0 / 60.0;
        PROBE.with(|p| p.borrow_mut().clear());
        set_doc_salt(&self.ctx, self.doc);
        let input = RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0))),
            time: Some(self.t),
            events,
            ..Default::default()
        };
        let (absnap, snap) = (AbSnap::read(&self.ed), Snap::read(&self.ed));
        let none = None;
        let align = [None, None, None, None, None, None, None, None];
        let icons = DockIcons { rotate: &none, opacity: &none, strokew: &none, align: &align };
        let (mut refpt, mut lock, mut fit) = (self.refpt, false, None);
        let mut ops: Vec<Op> = vec![];
        let (view, select, press) = (self.view, self.select_on_click.clone(), self.press_op);
        let mut select_at = self.select_at;
        let shell = &mut self.shell;
        let _ = self.ctx.run_ui(input, |ui| {
            if let Some(ids) = &select {
                let r = ui.button("Select");
                select_at = r.rect.center();
                if r.clicked() {
                    ops.push(Op::LayerSelectSet(ids.clone()));
                }
            }
            if let Some(op) = press {
                let r = ui.button("Press");
                select_at = r.rect.center();
                if r.clicked() {
                    ops.push(op());
                }
            }
            match view {
                View::Artboard => panel_artboard(ui, &absnap, &mut lock, &mut ops, &mut fit),
                View::Properties => panel_properties(
                    ui,
                    &snap,
                    &icons,
                    &mut refpt,
                    &mut lock,
                    &mut ops,
                    (&Default::default(), &mut vec![]),
                ),
                View::Empty => {}
                View::FirstPassOnly => {
                    if ui.ctx().current_pass_index() == 0 {
                        panel_artboard(ui, &absnap, &mut lock, &mut ops, &mut fit);
                        ui.ctx().request_discard("the panel went away mid-frame");
                    }
                }
                View::Shell => {
                    use varos_app::shell::PanelId as P;
                    let mut host = |panel: P, ui: &mut egui::Ui| -> bool {
                        kf::group(ui, panel, |ui| match panel {
                            P::Properties => {
                                panel_artboard(ui, &absnap, &mut lock, &mut ops, &mut fit);
                                true
                            }
                            P::Board => true,
                            _ => false,
                        })
                    };
                    shell.ui_hosted(ui, &mut host);
                }
                View::Both => {
                    let board = egui::Rect::from_min_max(egui::pos2(420.0, 0.0), egui::pos2(800.0, 600.0));
                    kf::group(ui, "board", |ui| {
                        board_ctlbar(
                            ui.ctx(),
                            board,
                            &snap,
                            &absnap,
                            &icons,
                            &None,
                            Default::default(),
                            &mut ops,
                            &mut fit,
                        )
                    });
                    let left = egui::Rect::from_min_max(egui::pos2(0.0, 40.0), egui::pos2(400.0, 600.0));
                    ui.scope_builder(egui::UiBuilder::new().max_rect(left), |ui| {
                        kf::group(ui, "props", |ui| {
                            panel_properties(
                                ui,
                                &snap,
                                &icons,
                                &mut refpt,
                                &mut lock,
                                &mut ops,
                                (&Default::default(), &mut vec![]),
                            )
                        })
                    });
                }
            }
        });
        self.select_at = select_at;
        finish_frame(&self.ctx, self.doc, &mut ops, &mut self.pending);
        apply_ops(&mut self.ed, ops);
    }
    fn at(&self, name: &str) -> Pos2 {
        self.nth(name, 0)
    }
    /// The `n`-th field drawn under `name` this frame (mirrors share a name).
    fn nth(&self, name: &str, n: usize) -> Pos2 {
        PROBE
            .with(|p| p.borrow().iter().filter(|(f, _)| f == name).nth(n).map(|(_, r)| r.center()))
            .expect("the field is drawn")
    }
    fn button(&mut self, p: Pos2, pressed: bool) {
        self.frame(vec![Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }]);
    }
    fn click(&mut self, p: Pos2) {
        self.frame(vec![Event::PointerMoved(p)]);
        self.button(p, true);
        self.button(p, false);
    }
    /// Click into a field and let it take the keyboard.
    fn edit(&mut self, name: &str) {
        let p = self.at(name);
        self.click(p);
        self.frame(vec![]);
        assert!(self.focused(), "premise: the field {name} took the keyboard");
    }
    fn key_with(&mut self, key: Key, modifiers: Modifiers) {
        self.frame(vec![Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers }]);
    }
    fn key(&mut self, key: Key) {
        self.key_with(key, Modifiers::NONE);
    }
    fn type_text(&mut self, s: &str) {
        self.frame(vec![Event::Text(s.into())]);
    }
    /// Replace a text field's whole text (⌘A, then type).
    fn retype(&mut self, s: &str) {
        self.key_with(Key::A, Modifiers::COMMAND);
        self.type_text(s);
    }
    fn click_away(&mut self) {
        self.click(Pos2::new(790.0, 590.0));
        self.frame(vec![]);
    }
    /// What `Ui::settle` does first, before a tab switch / ⌘S / Close / Quit — and before a canvas press.
    fn settle(&mut self) -> bool {
        settle(&self.ctx, self.doc, &mut self.pending, &mut self.ed)
    }
    fn focused(&self) -> bool {
        self.ctx.memory(|m| m.focused().is_some())
    }
    fn reason(&self) -> Option<&'static str> {
        let id = self.ctx.memory(|m| m.focused())?;
        kf::reason(&self.ctx, id)
    }
    fn ab_name(&self) -> String {
        self.ed.doc.artboards[0].name.clone()
    }
    fn x(&self) -> f32 {
        Snap::read(&self.ed).x
    }
}

fn artboard() -> Bench {
    let mut ed = Editor::new();
    ed.doc.artboards.clear();
    ed.doc.artboards.push(varos_core::model::Artboard {
        w: 800.0,
        h: 600.0,
        name: "Artboard 1".into(),
        ..Default::default()
    });
    ed.doc.active = 0;
    Bench::new(ed, View::Artboard)
}

/// Draw rectangles with the real tool; the LAST one ends selected (Selection tool).
fn rects(boxes: &[([f32; 2], [f32; 2])]) -> Editor {
    let mut ed = Editor::new();
    ed.ppu = 1.0;
    for (a, b) in boxes {
        ed.set_tool(ToolKind::Rect);
        ed.pointer_down(*a);
        ed.pointer_move(*b);
        ed.pointer_up();
    }
    ed.set_tool(ToolKind::Object);
    ed
}
fn one_rect() -> Bench {
    let mut ed = rects(&[([100.0, 100.0], [200.0, 200.0])]);
    ed.select_all();
    let b = Bench::new(ed, View::Properties);
    assert_eq!(b.x(), 100.0, "premise");
    b
}

// ── the name field (bug a, b) ──

/// Bug (a): `name_field` read focus at frame start; on the click-away frame focus was already gone, so
/// it rebuilt its buffer from the model and committed that — the typed name was overwritten.
#[test]
fn name_field_commits_typed_text_on_click_away() {
    let mut b = artboard();
    let rev = b.ed.rev;
    b.edit("dock");
    b.retype("Cover");
    b.click_away();
    assert_eq!(b.ab_name(), "Cover", "the typed name stays");
    assert_eq!(b.ed.rev, rev + 1, "one undo step");
    assert!(!b.focused());
    b.ed.undo();
    assert_eq!(b.ab_name(), "Artboard 1", "⌘Z: the old name returns in one step");
}

#[test]
fn name_field_enter_and_tab_commit_once() {
    let mut b = artboard();
    let rev = b.ed.rev;
    b.edit("dock");
    b.retype("Cover");
    b.key(Key::Enter);
    b.frame(vec![]);
    assert_eq!((b.ab_name().as_str(), b.ed.rev), ("Cover", rev + 1), "Enter commits once");
    assert!(!b.focused(), "Enter blurs");
    // Tab: commits, and the keyboard goes to the next FIELD of the panel (W), never a button
    let w = b.ed.doc.artboards[0].w;
    b.edit("dock");
    b.retype("Poster");
    b.key(Key::Tab);
    b.frame(vec![]);
    b.frame(vec![]);
    assert_eq!((b.ab_name().as_str(), b.ed.rev), ("Poster", rev + 2), "Tab commits once");
    assert!(b.focused(), "Tab handed the keyboard on");
    b.type_text("640");
    b.key(Key::Enter);
    b.frame(vec![]);
    assert_eq!(b.ed.doc.artboards[0].w, 640.0, "…to the W field (was {w}), its value pre-selected");
}

/// ⌘S / tab switch / Close / Quit while typing: the host settles the Ui first, which commits the field
/// into THIS document before the command runs (the save writes the new value; Close computes dirty
/// after the commit).
#[test]
fn save_and_tab_switch_commit_first() {
    for what in ["⌘S", "Ctrl+Tab", "Close"] {
        let mut b = artboard();
        let rev = b.ed.rev;
        b.edit("dock");
        b.retype("Cover");
        assert_eq!(b.ab_name(), "Artboard 1", "{what}: premise — typing alone commits nothing");
        assert!(b.settle(), "{what}: valid text settles");
        assert_eq!((b.ab_name().as_str(), b.ed.rev), ("Cover", rev + 1), "{what}: committed once, first");
        assert!(!b.focused(), "{what}: the field closed");
        b.frame(vec![]);
        b.click_away();
        assert_eq!(b.ed.rev, rev + 1, "{what}: nothing commits again afterwards");
    }
    // the number field the same way: type W, ⌘S → the document holds the new width
    let mut b = artboard();
    b.edit("Width");
    b.type_text("1234");
    assert!(b.settle());
    assert_eq!(b.ed.doc.artboards[0].w, 1234.0);
}

#[test]
fn esc_reverts_without_history() {
    // the name field
    let mut b = artboard();
    let rev = b.ed.rev;
    b.edit("dock");
    b.retype("Oops");
    b.key(Key::Escape);
    b.frame(vec![]);
    b.click_away();
    assert_eq!((b.ab_name().as_str(), b.ed.rev), ("Artboard 1", rev), "Esc: old name, no step, not dirty");
    assert!(!b.focused());
    assert!(b.settle(), "nothing left open");
    assert_eq!(b.ed.rev, rev);
    // the number field (bug c)
    let mut b = one_rect();
    let rev = b.ed.rev;
    b.edit("X position");
    b.type_text("999");
    b.key(Key::Escape);
    b.frame(vec![]);
    b.click_away();
    assert_eq!((b.x(), b.ed.rev), (100.0, rev), "Esc in a number field reverts, never commits");
    assert!(!kf::any_open(&b.ctx));
}

/// Bug (b) / audit 01 B6: an unchanged name (or number) used to commit on every blur — an undo step and
/// a dirty document for nothing.
#[test]
fn unchanged_commit_is_not_dirty() {
    let mut b = artboard();
    let rev = b.ed.rev;
    b.edit("dock");
    b.click_away();
    b.edit("dock");
    b.key(Key::Enter);
    b.edit("dock");
    b.retype("Artboard 1"); // retyped identically
    b.key(Key::Tab);
    b.edit("dock");
    assert!(b.settle());
    assert_eq!(b.ed.rev, rev, "no undo step, nothing dirty");
    let mut b = one_rect();
    let rev = b.ed.rev;
    b.edit("X position");
    b.key(Key::Enter);
    b.edit("X position");
    b.type_text("100.0");
    b.click_away();
    assert_eq!(b.ed.rev, rev, "the same number is unchanged");
}

#[test]
fn invalid_keeps_focus_with_reason() {
    let mut b = artboard();
    let rev = b.ed.rev;
    b.edit("dock");
    b.key_with(Key::A, Modifiers::COMMAND);
    b.key(Key::Backspace);
    b.key(Key::Enter);
    b.frame(vec![]);
    assert!(b.focused(), "an empty name keeps the keyboard");
    assert_eq!(b.reason(), Some(kf::EMPTY_NAME), "…and says why");
    b.click_away();
    assert!(b.focused(), "a click elsewhere does not take it either");
    assert!(!b.settle(), "⌘S / Close / a tab switch do not run");
    assert_eq!((b.ab_name().as_str(), b.ed.rev), ("Artboard 1", rev), "nothing changed");
    b.type_text("Cover");
    b.frame(vec![]);
    assert_eq!(b.reason(), None, "the next valid keystroke clears the reason");
    b.key(Key::Enter);
    b.frame(vec![]);
    assert_eq!((b.ab_name().as_str(), b.ed.rev), ("Cover", rev + 1));
    // a number that does not parse
    let mut b = one_rect();
    let rev = b.ed.rev;
    b.edit("X position");
    b.type_text("abc");
    b.key(Key::Enter);
    b.frame(vec![]);
    assert!(b.focused());
    assert_eq!(b.reason(), Some(kf::NOT_A_NUMBER));
    assert!(!b.settle());
    b.key(Key::Escape);
    b.frame(vec![]);
    assert!(!b.focused() && b.reason().is_none(), "Esc reverts and clears the reason");
    assert_eq!((b.x(), b.ed.rev), (100.0, rev));
}

// ── the number field: every way to commit, once ──

#[test]
fn number_field_commits_once_on_enter_tab_click_away_and_settle() {
    for how in ["Enter", "Tab", "click-away", "settle"] {
        let mut b = one_rect();
        let rev = b.ed.rev;
        b.edit("X position");
        b.type_text("250"); // the value is pre-selected: typing replaces it
        match how {
            "Enter" => b.key(Key::Enter),
            "Tab" => b.key(Key::Tab),
            "click-away" => b.click_away(),
            _ => assert!(b.settle()),
        }
        b.frame(vec![]);
        b.frame(vec![]);
        assert_eq!((b.x(), b.ed.rev), (250.0, rev + 1), "{how}: one step, the typed value");
        if how == "Tab" {
            assert!(b.focused(), "Tab: the next field (W) took the keyboard");
            b.key(Key::Escape);
        }
        b.ed.undo();
        assert_eq!(b.x(), 100.0, "{how}: one undo brings it back");
    }
}

#[test]
fn arrow_steps_and_scrub_keep_working() {
    let mut b = one_rect();
    let rev = b.ed.rev;
    b.edit("X position");
    b.key(Key::ArrowUp);
    b.key_with(Key::ArrowUp, Modifiers::SHIFT);
    assert_eq!(b.x(), 111.0, "↑ +1, ⇧↑ +10, live");
    let stepped = b.ed.rev;
    assert!(stepped > rev);
    b.key(Key::Enter);
    b.frame(vec![]);
    assert_eq!((b.x(), b.ed.rev), (111.0, stepped), "the blur after a step commits nothing more");
}

/// A click that blurs the field AND changes the selection (a Layers row) in the same frame: the commit
/// lands on the object the field was editing. And a field that simply disappears (the selection or the
/// tool changed the panel) commits first too.
#[test]
fn selection_change_commits_to_old_target() {
    let mut ed = rects(&[([100.0, 100.0], [200.0, 200.0]), ([300.0, 300.0], [350.0, 350.0])]);
    let (a, bpath) = (ed.doc.paths[0].id, ed.doc.paths[1].id);
    ed.objsel.clear();
    ed.objsel.insert(a);
    let node_b = ed.doc.node_of_path(bpath).expect("B has a node");
    let mut b = Bench::new(ed, View::Properties);
    b.select_on_click = Some(vec![node_b]);
    b.frame(vec![]);
    let bx = |b: &Bench| b.ed.doc.paths.iter().find(|p| p.id == bpath).unwrap().anchors[0].p[0];
    let ax = |b: &Bench| b.ed.doc.paths.iter().find(|p| p.id == a).unwrap().anchors[0].p[0];
    b.edit("X position");
    b.type_text("500");
    let at = b.select_at;
    b.click(at);
    b.frame(vec![]);
    assert_eq!(ax(&b), 500.0, "A (the field's object) moved");
    assert_eq!(bx(&b), 300.0, "B (the new selection) did not");
    assert!(b.ed.objsel.contains(&bpath), "the selection did change");
    // the field disappears without a blur (the panel changed): its last valid text commits to A
    b.select_on_click = None;
    b.ed.objsel.clear();
    b.ed.objsel.insert(a);
    b.frame(vec![]);
    b.edit("X position");
    b.type_text("50");
    b.view = View::Empty;
    b.frame(vec![]);
    assert_eq!(ax(&b), 50.0, "the vanished field committed to what it edited");
    assert!(!kf::any_open(&b.ctx) && !b.focused());
    // …while text that does not parse is reverted silently (no trap)
    b.view = View::Properties;
    b.frame(vec![]);
    b.edit("X position");
    b.type_text("x");
    b.view = View::Empty;
    let rev = b.ed.rev;
    b.frame(vec![]);
    assert_eq!((ax(&b), b.ed.rev), (50.0, rev));
    assert!(!kf::any_open(&b.ctx));
}

/// The canvas press path (`main.rs`): `Ui::commit_fields` → `settle` runs BEFORE `pointer_down`, so a
/// press that selects another object cannot pull the commit onto it.
#[test]
fn a_canvas_press_commits_before_it_selects() {
    let mut ed = rects(&[([100.0, 100.0], [200.0, 200.0]), ([300.0, 300.0], [350.0, 350.0])]);
    let (a, bpath) = (ed.doc.paths[0].id, ed.doc.paths[1].id);
    ed.objsel.clear();
    ed.objsel.insert(a);
    let mut b = Bench::new(ed, View::Properties);
    b.edit("X position");
    b.type_text("20");
    assert!(b.settle(), "the host commits first");
    b.ed.pointer_down([320.0, 320.0]); // then the press selects B
    b.ed.pointer_up();
    b.frame(vec![]);
    let x_of = |id| b.ed.doc.paths.iter().find(|p| p.id == id).unwrap().anchors[0].p[0];
    assert_eq!((x_of(a), x_of(bpath)), (20.0, 300.0));
}

// ── DFS S1: a field edit never crosses into another tab ──

/// One pass of a bare X field for tab `doc` showing `value`; returns the frame's ops (after
/// `finish_frame`) and where the box sits.
fn x_frame(
    ctx: &egui::Context,
    doc: SessionId,
    pending: &mut Option<Pending>,
    events: Vec<Event>,
    value: f32,
) -> (Vec<Op>, Pos2) {
    set_doc_salt(ctx, Some(doc));
    let (mut ops, mut at) = (vec![], Pos2::ZERO);
    let input = RawInput {
        screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0))),
        events,
        ..Default::default()
    };
    let _ = ctx.run_ui(input, |ui| {
        at = ui.cursor().min + egui::vec2(80.0, 12.0);
        super::num(ui, 150.0, kf::Label::Letter("X"), "X position", value, 0, 1.0, -1.0e6..=1.0e6, &mut ops, |v| {
            Op::SetRot(v)
        });
    });
    finish_frame(ctx, Some(doc), &mut ops, pending);
    (ops, at)
}
fn press(pos: Pos2, pressed: bool) -> Vec<Event> {
    vec![
        Event::PointerMoved(pos),
        Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE },
    ]
}
fn committed(ops: &[Op]) -> Vec<f32> {
    ops.iter()
        .filter_map(|o| match o {
            Op::Field(op) | Op::FieldPending(_, op) => match **op {
                Op::SetRot(v) => Some(v),
                _ => None,
            },
            Op::SetRot(v) => Some(*v),
            _ => None,
        })
        .collect()
}
/// Tab A: object A's X is 10 — click the field and type 999 (the value is pre-selected).
fn type_999_on_tab_a(ctx: &egui::Context, pending: &mut Option<Pending>) {
    let a = SessionId(1);
    let (_, field) = x_frame(ctx, a, pending, vec![], 10.0);
    x_frame(ctx, a, pending, press(field, true), 10.0);
    x_frame(ctx, a, pending, press(field, false), 10.0);
    x_frame(ctx, a, pending, vec![], 10.0); // the text edit claims focus
    let (ops, _) = x_frame(ctx, a, pending, vec![Event::Text("999".into())], 10.0);
    assert!(committed(&ops).is_empty(), "premise: typing alone commits nothing");
    assert!(ctx.memory(|m| m.focused().is_some()), "premise: the field is being edited");
}
/// Tab B: object B's X is 5 — frames and a click elsewhere. Returns every value seen.
fn frames_on_tab_b(ctx: &egui::Context, pending: &mut Option<Pending>) -> Vec<f32> {
    let b = SessionId(2);
    let away = Pos2::new(600.0, 500.0);
    let mut got = committed(&x_frame(ctx, b, pending, vec![], 5.0).0);
    got.extend(committed(&x_frame(ctx, b, pending, press(away, true), 5.0).0));
    got.extend(committed(&x_frame(ctx, b, pending, press(away, false), 5.0).0));
    got.extend(committed(&x_frame(ctx, b, pending, vec![], 5.0).0));
    got
}

#[test]
fn a_typed_number_never_crosses_into_the_next_tab() {
    let ctx = egui::Context::default();
    let mut pending = None;
    type_999_on_tab_a(&ctx, &mut pending);
    // Ctrl+Tab: the lifecycle key bypasses egui; the host settles the Ui BEFORE switching — K3: the
    // edit commits into A (the outgoing document), never later into B
    let mut ed_a = Editor::new();
    assert!(settle(&ctx, Some(SessionId(1)), &mut pending, &mut ed_a));
    assert!(ctx.memory(|m| m.focused().is_none()), "settle closes the focused edit");
    assert_eq!(frames_on_tab_b(&ctx, &mut pending), Vec::<f32>::new(), "B must never receive A's typed 999");
    assert!(ctx.memory(|m| m.focused().is_none()), "no field is left focused on B");
    // back on A: the settled edit does not come back or commit again
    let a = SessionId(1);
    let away = Pos2::new(600.0, 500.0);
    for ev in [vec![], press(away, true), press(away, false)] {
        assert!(committed(&x_frame(&ctx, a, &mut pending, ev, 10.0).0).is_empty());
    }
}

/// The second wall: even an edit that was NOT settled (a future path that forgets to) lives under A's
/// id only, so B's field of the same name cannot inherit it — and A's orphaned pending never applies on B.
#[test]
fn field_state_is_scoped_to_its_document() {
    let ctx = egui::Context::default();
    let mut pending = None;
    type_999_on_tab_a(&ctx, &mut pending);
    assert_eq!(frames_on_tab_b(&ctx, &mut pending), Vec::<f32>::new(), "B must never receive A's typed 999");
}

// ── review round 1 (Codex): mirrors, commands while editing, chrome presses while invalid ──

/// H1: the control-bar X and the Properties X mirror the same value with different reference points
/// (bar: left edge; Properties here: bottom-right, so its X is the RIGHT edge). Typing into one commits
/// once, with THAT field's meaning — never the other's.
#[test]
fn mirrored_fields_commit_through_the_field_that_was_edited() {
    for (n, want_left) in [(0usize, 300.0f32), (1, 200.0)] {
        let mut ed = rects(&[([100.0, 100.0], [200.0, 200.0])]);
        ed.select_all();
        let mut b = Bench::new(ed, View::Both);
        b.refpt = (1.0, 1.0);
        b.frame(vec![]);
        let rev = b.ed.rev;
        let p = b.nth("X position", n);
        b.click(p);
        b.frame(vec![]);
        b.type_text("300");
        b.key(Key::Enter);
        b.frame(vec![]);
        b.frame(vec![]);
        let which = if n == 0 { "control bar" } else { "Properties" };
        assert_eq!((b.x(), b.ed.rev), (want_left, rev + 1), "{which}: one commit, its own reference point");
        assert!(!kf::any_open(&b.ctx), "{which}: nothing left open in the mirror");
    }
}

/// M1: a chrome press while a field holds text that does not parse is ignored (the field keeps the
/// keyboard and its reason); after Esc the same press acts.
#[test]
fn a_chrome_press_waits_for_an_invalid_field() {
    let mut b = artboard();
    b.press_op = Some(|| Op::AbDel(0));
    b.ed.ab_add(); // two boards, so Delete may act
    b.frame(vec![]);
    b.edit("dock");
    b.key_with(Key::A, Modifiers::COMMAND);
    b.key(Key::Backspace);
    let at = b.select_at;
    b.click(at);
    b.frame(vec![]);
    assert_eq!(b.ed.doc.artboards.len(), 2, "nothing deleted while the name is empty");
    assert!(b.focused(), "the field keeps the keyboard");
    assert_eq!(b.reason(), Some(kf::EMPTY_NAME), "…and shows why");
    b.key(Key::Escape);
    b.frame(vec![]);
    b.click(at);
    b.frame(vec![]);
    assert_eq!(b.ed.doc.artboards.len(), 1, "after Esc the press acts");
    assert_eq!(b.ed.doc.artboards[0].name, "Artboard 2", "the empty name never landed");
}

/// The left edge of the first path, read from its anchors (independent of the selection).
fn left(b: &Bench) -> f32 {
    b.ed.doc.paths.first().map_or(f32::NAN, |p| p.anchors.iter().map(|a| a.p[0]).fold(f32::MAX, f32::min))
}

/// The real field side of `Ui` as the host's dispatcher sees it (`DocUi`).
struct FieldUi<'a> {
    ctx: &'a egui::Context,
    doc: Option<SessionId>,
    pending: &'a mut Option<Pending>,
}
impl crate::host::DocUi for FieldUi<'_> {
    fn settle(&mut self, ed: &mut Editor) -> bool {
        settle(self.ctx, self.doc, self.pending, ed)
    }
    fn field_has_focus(&self) -> bool {
        kf::any_open(self.ctx)
    }
    fn document_switched(&mut self) {}
}

/// H3: a document command (a key, a menu row) raised while a field holds a typed value runs through
/// the ONE dispatcher, which commits the field first — to what it was editing — then acts.
/// ⌘Z / ⇧⌘Z while the field is being edited belong to the field's own text, never to the document.
#[test]
fn document_commands_commit_the_open_field_first() {
    use crate::host::DocAction;
    use winit::keyboard::KeyCode;
    let canvas = egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
    let mut view = varos_core::geom::View::identity();
    let none = varos_core::editor::Mods::default();
    let cmd = varos_core::editor::Mods { ctrl: true, ..none };
    // ⌘G: commit (the selection → X 500) THEN group — two steps, undone in reverse
    let mut ed = rects(&[([100.0, 100.0], [200.0, 200.0]), ([300.0, 300.0], [350.0, 350.0])]);
    ed.select_all();
    let mut b = Bench::new(ed, View::Properties);
    let rev = b.ed.rev;
    b.edit("X position");
    b.type_text("500");
    let mut ui = FieldUi { ctx: &b.ctx, doc: b.doc, pending: &mut b.pending };
    assert!(crate::run_doc(DocAction::Key(KeyCode::KeyG, cmd), &mut b.ed, &mut view, canvas, &mut ui));
    assert_eq!((left(&b), b.ed.rev), (500.0, rev + 2), "the typed value, then the group: two steps");
    b.ed.undo();
    assert_eq!(left(&b), 500.0, "undo ungroups; the typed value stays (it came first)");
    b.ed.undo();
    assert_eq!(left(&b), 100.0);
    // ⌘Z while typing: the document history is untouched, the field still edits
    let mut b = one_rect();
    let rev = b.ed.rev;
    b.edit("X position");
    b.type_text("500");
    let mut ui = FieldUi { ctx: &b.ctx, doc: b.doc, pending: &mut b.pending };
    assert!(crate::run_doc(DocAction::Key(KeyCode::KeyZ, cmd), &mut b.ed, &mut view, canvas, &mut ui));
    assert_eq!((left(&b), b.ed.rev), (100.0, rev), "⌘Z did not reach the document");
    assert!(kf::any_open(&b.ctx), "the field is still being edited");
}

/// H3 with Delete: the open field commits to the object it edits, THEN Delete removes it — one undo
/// brings it back WITH the typed value. A field holding invalid text holds the command (`false`).
#[test]
fn delete_with_an_open_field_commits_first_and_invalid_text_holds_it() {
    use crate::host::DocAction;
    use winit::keyboard::KeyCode;
    let canvas = egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
    let mut view = varos_core::geom::View::identity();
    let none = varos_core::editor::Mods::default();
    let mut b = one_rect();
    b.edit("X position");
    b.type_text("500");
    let mut ui = FieldUi { ctx: &b.ctx, doc: b.doc, pending: &mut b.pending };
    assert!(crate::run_doc(DocAction::Key(KeyCode::Delete, none), &mut b.ed, &mut view, canvas, &mut ui));
    assert!(b.ed.doc.paths.is_empty(), "deleted");
    b.ed.undo();
    assert_eq!(left(&b), 500.0, "the typed value landed on the object before it was deleted");
    // invalid text: the action does not run
    b.frame(vec![]);
    b.ed.select_all();
    b.frame(vec![]);
    b.edit("X position");
    b.type_text("abc");
    let mut ui = FieldUi { ctx: &b.ctx, doc: b.doc, pending: &mut b.pending };
    assert!(!crate::run_doc(DocAction::Key(KeyCode::Delete, none), &mut b.ed, &mut view, canvas, &mut ui));
    assert_eq!(b.ed.doc.paths.len(), 1, "held: nothing deleted");
}

// ── review round 2: an invalid edit only lives while its field is drawn and focused ──

/// An invalid name, then its panel stops being drawn — closed, or its box switched to the Layers tab
/// (exactly what Window ▸ Properties / Layers do: `ShellState::toggle_panel`). The session reverts in
/// THAT frame: no reason left, nothing blocked, a held ⌘Q may run; the empty name never lands.
#[test]
fn an_invalid_edit_reverts_the_frame_its_panel_goes_away() {
    use varos_app::shell::PanelId;
    for (how, panel) in [("close Properties", PanelId::Properties), ("switch the box to Layers", PanelId::Layers)] {
        let mut b = artboard();
        b.view = View::Shell;
        b.frame(vec![]);
        let rev = b.ed.rev;
        b.edit("dock");
        b.key_with(Key::A, Modifiers::COMMAND);
        b.key(Key::Backspace);
        b.key(Key::Enter);
        b.frame(vec![]);
        assert!(kf::blocked(&b.ctx) && !b.settle(), "{how}: premise — invalid, a ⌘Q would be held");
        b.shell.toggle_panel(panel);
        b.frame(vec![]); // ONE frame without the field
        assert!(!kf::blocked(&b.ctx) && !kf::any_open(&b.ctx), "{how}: reverted in that same frame");
        assert!(b.settle(), "{how}: nothing holds a command any more");
        assert_eq!((b.ab_name().as_str(), b.ed.rev), ("Artboard 1", rev), "{how}: the old name, no step");
    }
}

/// The same with VALID typed text: the field commits once, to its artboard, the frame it goes away.
#[test]
fn a_valid_edit_commits_once_when_its_panel_goes_away() {
    use varos_app::shell::PanelId;
    let mut b = artboard();
    b.view = View::Shell;
    b.frame(vec![]);
    let rev = b.ed.rev;
    b.edit("dock");
    b.retype("Cover");
    b.shell.toggle_panel(PanelId::Properties);
    b.frame(vec![]);
    b.frame(vec![]);
    assert_eq!((b.ab_name().as_str(), b.ed.rev), ("Cover", rev + 1), "committed once");
    assert!(!kf::any_open(&b.ctx));
}

/// The panel goes away mid-frame (egui re-runs the frame and the second pass has no field): the edit
/// still reverts at the end of THAT frame, not one frame later.
#[test]
fn an_invalid_edit_reverts_even_when_the_field_vanishes_in_a_second_pass() {
    let mut b = artboard();
    let rev = b.ed.rev;
    b.edit("dock");
    b.key_with(Key::A, Modifiers::COMMAND);
    b.key(Key::Backspace);
    b.key(Key::Enter);
    b.frame(vec![]);
    assert!(kf::blocked(&b.ctx), "premise");
    b.view = View::FirstPassOnly;
    b.frame(vec![]);
    assert!(!kf::blocked(&b.ctx) && !kf::any_open(&b.ctx), "reverted in that frame");
    assert!(b.settle(), "nothing held");
    assert_eq!((b.ab_name().as_str(), b.ed.rev), ("Artboard 1", rev));
}

// ── the Board section (Start v2 L5): name / description / tags in Properties, nothing selected ──

fn board() -> Bench {
    let b = Bench::new(Editor::new(), View::Properties);
    assert!(b.ed.doc.name.is_empty() && b.ed.doc.tags.is_empty(), "premise: a fresh board");
    b
}

#[test]
fn board_name_commits_on_blur_and_enter_esc_reverts_and_undo_is_one_step() {
    let mut b = board();
    let rev = b.ed.rev;
    b.edit("board name");
    b.type_text("Ramadan campaign");
    b.click_away();
    assert_eq!((b.ed.doc.name.as_str(), b.ed.rev), ("Ramadan campaign", rev + 1), "blur commits once");
    b.edit("board name");
    b.retype("Draft");
    b.key(Key::Escape);
    b.frame(vec![]);
    assert_eq!((b.ed.doc.name.as_str(), b.ed.rev), ("Ramadan campaign", rev + 1), "Esc reverts, no step");
    b.edit("board name");
    b.retype("  Eid greetings  ");
    b.key(Key::Enter);
    b.frame(vec![]);
    assert_eq!(b.ed.doc.name, "Eid greetings", "Enter commits the cleaned text");
    b.ed.undo();
    assert_eq!(b.ed.doc.name, "Ramadan campaign", "⌘Z: one step back");
}

#[test]
fn board_fields_over_the_limit_keep_the_keyboard_with_the_reason() {
    let mut b = board();
    let rev = b.ed.rev;
    b.edit("board name");
    b.type_text(&"n".repeat(varos_core::board::MAX_NAME_CHARS + 1));
    b.key(Key::Enter);
    b.frame(vec![]);
    assert!(b.focused(), "an over-long name keeps the keyboard");
    assert_eq!(b.reason(), Some(super::BOARD_NAME_TOO_LONG));
    assert!(!b.settle(), "⌘S / Close wait for it");
    b.key(Key::Escape);
    b.frame(vec![]);
    assert_eq!((b.ed.doc.name.as_str(), b.ed.rev), ("", rev), "nothing changed");
    b.edit("board description");
    b.type_text(&"d".repeat(varos_core::board::MAX_DESCRIPTION_CHARS + 1));
    b.click_away();
    assert!(b.focused());
    assert_eq!(b.reason(), Some(super::BOARD_DESCRIPTION_TOO_LONG));
    b.key(Key::Escape);
    b.frame(vec![]);
    b.edit("board tags");
    b.type_text(&"t".repeat(varos_core::board::MAX_TAG_CHARS + 1));
    b.key(Key::Enter);
    b.frame(vec![]);
    assert_eq!(b.reason(), Some(super::BOARD_TAG_TOO_LONG));
    b.key(Key::Escape);
    b.frame(vec![]);
    assert_eq!(b.ed.rev, rev, "no refused edit reached the document");
}

#[test]
fn the_description_wraps_three_rows_and_enter_commits() {
    let mut b = board();
    let rev = b.ed.rev;
    b.edit("board description");
    b.type_text("Key visual for Noor Foods — the portrait poster and its story cut-down, for Ramadan.");
    b.key(Key::Enter);
    b.frame(vec![]);
    assert!(!b.focused(), "Enter ends the edit (no line break)");
    assert_eq!(b.ed.rev, rev + 1);
    assert!(b.ed.doc.description.starts_with("Key visual") && !b.ed.doc.description.contains('\n'));
    let rows = PROBE.with(|p| p.borrow().iter().find(|(n, _)| n == "board description").map(|(_, r)| r.height()));
    let name = PROBE.with(|p| p.borrow().iter().find(|(n, _)| n == "board name").map(|(_, r)| r.height()));
    assert!(rows.unwrap() > name.unwrap() * 2.0, "three rows tall");
}

#[test]
fn tags_add_by_enter_and_comma_dedupe_remove_by_backspace_and_chip_and_undo() {
    let mut b = board();
    let rev = b.ed.rev;
    b.edit("board tags");
    b.type_text("client");
    b.key(Key::Enter);
    b.frame(vec![]);
    assert_eq!(b.ed.doc.tags, ["client"]);
    assert!(b.focused(), "Enter adds and keeps the keyboard for the next tag");
    b.frame(vec![]);
    b.type_text("social,");
    b.frame(vec![]);
    b.frame(vec![]);
    assert_eq!(b.ed.doc.tags, ["client", "social"], "a comma adds too");
    assert_eq!(b.ed.rev, rev + 2, "one step per tag");
    // a tag the board already has (by the core fold) adds nothing
    b.frame(vec![]);
    b.type_text("CLIENT");
    b.key(Key::Enter);
    b.frame(vec![]);
    assert_eq!((b.ed.doc.tags.len(), b.ed.rev), (2, rev + 2), "deduped: unchanged, no step");
    // Backspace in the empty input removes the last tag
    b.edit("board tags");
    b.key(Key::Backspace);
    b.frame(vec![]);
    assert_eq!(b.ed.doc.tags, ["client"]);
    // typing then Backspace edits the text, not the tags
    b.type_text("ab");
    b.frame(vec![]);
    b.key(Key::Backspace);
    b.frame(vec![]);
    assert_eq!(b.ed.doc.tags, ["client"], "Backspace deleted a character");
    b.key(Key::Escape);
    b.frame(vec![]);
    // a chip's × removes that tag
    b.frame(vec![]);
    let x = chip_x();
    b.click(x);
    b.frame(vec![]);
    assert!(b.ed.doc.tags.is_empty(), "× removed it");
    b.ed.undo();
    assert_eq!(b.ed.doc.tags, ["client"], "⌘Z brings it back in one step");
}

/// The × of the first tag chip: the right end of the first chip in the tag box.
fn chip_x() -> Pos2 {
    let input = PROBE.with(|p| p.borrow().iter().find(|(n, _)| n == "board tags").map(|(_, r)| *r)).unwrap();
    // the chip sits left of the input on the same line; its × is just before the input's left edge
    egui::pos2(input.left() - varos_app::shell::tokens::SB_PILL_GAP - 10.0, input.center().y)
}

#[test]
fn too_many_tags_is_refused_with_the_reason() {
    let mut ed = Editor::new();
    ed.doc.tags = (0..varos_core::board::MAX_TAGS).map(|i| format!("t{i}")).collect();
    let mut b = Bench::new(ed, View::Properties);
    let rev = b.ed.rev;
    b.edit("board tags");
    b.type_text("one more");
    b.key(Key::Enter);
    b.frame(vec![]);
    assert_eq!(b.reason(), Some(super::BOARD_TOO_MANY_TAGS));
    b.key(Key::Escape);
    b.frame(vec![]);
    assert_eq!((b.ed.doc.tags.len(), b.ed.rev), (varos_core::board::MAX_TAGS, rev));
}

#[test]
fn board_reasons_quote_the_core_limits() {
    use varos_core::board::{MAX_DESCRIPTION_CHARS, MAX_NAME_CHARS, MAX_TAGS, MAX_TAG_CHARS};
    assert!(super::BOARD_NAME_TOO_LONG.ends_with(&format!("at most {MAX_NAME_CHARS} characters")));
    assert!(super::BOARD_DESCRIPTION_TOO_LONG.ends_with(&format!("at most {MAX_DESCRIPTION_CHARS} characters")));
    assert!(super::BOARD_TOO_MANY_TAGS.ends_with(&format!("at most {MAX_TAGS} tags")));
    assert!(super::BOARD_TAG_TOO_LONG.ends_with(&format!("at most {MAX_TAG_CHARS} characters")));
}

/// The Board section's controls are kit targets: every field and every tag chip's × is at least
/// 24 pt both ways (review fix 2; the × is drawn small, its hit square is not).
#[test]
fn board_section_hit_targets_are_at_least_24pt() {
    let mut ed = Editor::new();
    ed.doc.tags = vec!["client".into(), "ramadan".into()];
    let mut b = Bench::new(ed, View::Properties);
    b.frame(vec![]);
    let rects: Vec<(String, egui::Rect)> = PROBE.with(|p| p.borrow().clone());
    let mut chips = 0;
    for (name, r) in rects.iter().filter(|(n, _)| n.starts_with("board") || n == "tag chip ×") {
        assert!(r.width() >= 24.0 && r.height() >= 24.0, "{name}: {r:?} is under 24 pt");
        chips += (name == "tag chip ×") as usize;
    }
    assert_eq!(chips, 2, "one × per tag");
}

#[test]
fn type_number_pending_settles_into_draft_before_save_or_tab_switch() {
    for (label, value) in [("Font size", "48"), ("Line height", "2"), ("Letter spacing", "1")] {
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        let doc = Some(SessionId(1));
        let mut pending = None;
        let mut ed = Editor::new();
        let mut tool = crate::text_product::TextProduct::default();
        tool.session = Some(varos_text_layout::edit::EditSession::new(
            varos_text_layout::default_text("ABC", [20., 50.]).unwrap(),
        ));
        let mut frame = |events: Vec<Event>, tool: &mut crate::text_product::TextProduct, ed: &mut Editor| {
            set_doc_salt(&ctx, doc);
            clear_probes();
            let text = tool.selected_text(ed).unwrap();
            let mut ops = vec![];
            let _ = ctx.run_ui(
                RawInput {
                    events,
                    screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(800., 800.))),
                    ..Default::default()
                },
                |ui| {
                    crate::ui::type_section(ui, &text, &mut ops);
                },
            );
            finish_frame(&ctx, doc, &mut ops, &mut pending);
            tool.finish_ops(ed, &mut ops);
            apply_ops(ed, ops);
        };
        frame(vec![], &mut tool, &mut ed);
        let rect = probed_rect(label, 0).center();
        frame(press(rect, true), &mut tool, &mut ed);
        frame(press(rect, false), &mut tool, &mut ed);
        frame(vec![], &mut tool, &mut ed);
        frame(vec![Event::Text(value.into())], &mut tool, &mut ed);
        assert!(pending.is_some(), "{label}");
        assert!(super::settle_text(&ctx, doc, &mut pending, &mut tool, &mut ed));
        assert!(ed.doc.text_boxes.is_empty());
        tool.commit(&mut ed).unwrap();
        let text = &ed.doc.text_boxes[0];
        let actual = match label {
            "Font size" => text.runs[0].style.size,
            "Line height" => text.para.line_height,
            _ => text.runs[0].style.letter_spacing,
        };
        assert_eq!(actual, value.parse::<f32>().unwrap());
        assert_eq!(ed.rev, 1);
        ed.undo();
        assert!(ed.doc.text_boxes.is_empty());
    }
}

#[test]
fn arabic_field_commit_preserves_logical_utf8_and_undo() {
    let mut b = artboard();
    let original = b.ab_name().to_owned();
    let name = "لوحة — Café logo ١٢٣";
    b.edit("dock");
    b.retype(name);
    b.key(Key::Enter);
    b.frame(vec![]);
    assert_eq!(b.ab_name(), name);
    let saved = serde_json::to_value(&b.ed.doc).unwrap();
    assert_eq!(saved["artboards"][0]["name"].as_str(), Some(name));
    b.ed.undo();
    assert_eq!(b.ab_name(), original);
}

#[test]
fn arabic_indic_digits_type_as_latin_in_numeric_fields() {
    for event in
        [Event::Text("٢٥٠٫۵".into()), Event::Paste("٢٥٠٫۵".into()), Event::Ime(egui::ImeEvent::Commit("٢٥٠٫۵".into()))]
    {
        let mut b = one_rect();
        let rev = b.ed.rev;
        b.edit("X position");
        b.frame(vec![event]);
        b.key(Key::Enter);
        b.frame(vec![]);
        assert_eq!((b.x(), b.ed.rev), (250.5, rev + 1));
        b.ed.undo();
        assert_eq!(b.x(), 100.0);
    }
}
