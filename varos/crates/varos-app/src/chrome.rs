//! Platform window chrome — pure data + logic, GPU-free and tested (docs/foundation/MAC_CHROME.md).
//!
//! * `TOPBAR`: how our own top bar sits in the window on this platform (left inset for the macOS
//!   traffic lights, whether we paint our own ─ ☐ ✕ caps).
//! * `caption_hit`: is a physical-px point on the EMPTY part of the bar (the drag band)? Its
//!   exclusions come from ONE list, `TopbarLayout::interactive_rects` — the same rects the bar's
//!   controls and tab chips are hit-tested with — on Windows (`WM_NCHITTEST`) and macOS alike.
//! * `menus()`: the native macOS menu bar as a table. Every item is a MIRROR of a path that already
//!   exists — a ⌘-shortcut keystroke, the ✕ close path, or a toggle an egui menu already offers.
//!   The AppKit glue that turns this table into an NSMenu lives in `mac_menu.rs` (macOS only).
#![cfg_attr(not(target_os = "macos"), allow(dead_code))] // the menu table is only built on macOS

use varos_app::shell::PanelId;
use winit::keyboard::KeyCode;

/// How the top bar fits the window chrome on one platform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TopbarChrome {
    /// Logical height: match the native 28 pt title area on macOS.
    pub height: f32,
    /// Logical px before the burger cell (macOS: room for the native traffic lights).
    pub lead: f32,
    /// Logical px between the right-most bar control and the window edge when there are no caps.
    pub right_inset: f32,
    /// Do we paint our own minimize / maximize / close caps (Windows' stripped caption)?
    pub window_caps: bool,
}

/// The table: macOS keeps the native traffic lights (so no caps of ours, and a left inset that clears
/// them — they end ≈ 70 pt from the left); every other platform keeps the original Windows numbers.
pub const fn topbar_chrome(macos: bool) -> TopbarChrome {
    if macos {
        TopbarChrome { height: 28.0, lead: 78.0, right_inset: 6.0, window_caps: false }
    } else {
        TopbarChrome { height: 46.0, lead: 4.0, right_inset: 0.0, window_caps: true }
    }
}

/// This build's top-bar chrome.
pub const TOPBAR: TopbarChrome = topbar_chrome(cfg!(target_os = "macos"));

/// Logical px between two neighbouring tab chips — the resting layout (`topbar_layout`) and the live
/// drag reflow (`tab_drag_frame`) both space chips with this one number.
pub const TAB_GAP: f32 = 4.0;

/// The actual rectangles painted / hit-tested by the top bar. Text widths come from egui's
/// font measurement; all padding, vertical alignment and tab fitting live here.
pub struct TopbarLayout {
    pub caps: Option<[egui::Rect; 3]>,
    pub menu: egui::Rect,
    pub magnet: egui::Rect,
    pub window: egui::Rect,
    pub share: egui::Rect,
    pub export: egui::Rect,
    pub search: egui::Rect,
    /// `(original tab index, its chip rect)`, left → right. Not always a `0..n` prefix: when the
    /// strip overflows, the greedy fit stops early and the ACTIVE tab (spec §4 "Active document name
    /// always matches canvas/layers") takes the last visible slot even if that means displacing
    /// whichever tab the greedy pass had put there (DFS S1 F7).
    pub tabs: Vec<(usize, egui::Rect)>,
    /// The `+` new-document chip — reserved BEFORE tabs are fitted, so it is always placed (F15):
    /// only an unreasonably narrow window ever leaves this `None`.
    pub plus: Option<egui::Rect>,
}

pub fn topbar_layout(
    bar: egui::Rect,
    chrome: TopbarChrome,
    button_text_widths: [f32; 3],
    search_width: f32,
    tab_text_widths: &[f32],
    active_tab: Option<usize>,
) -> TopbarLayout {
    use egui::{pos2, vec2, Rect};
    let cy = bar.center().y;
    let caps = chrome.window_caps.then(|| {
        [3.0, 2.0, 1.0].map(|i| {
            Rect::from_min_max(
                pos2(bar.right() - i * 42.0, bar.top()),
                pos2(bar.right() - (i - 1.0) * 42.0, bar.bottom()),
            )
        })
    });
    let caps_left = caps.map_or(bar.right() - chrome.right_inset, |r| r[0].left());
    let magnet = Rect::from_center_size(pos2(caps_left - 6.0 - 14.0, cy), vec2(28.0, 28.0));
    let button = |right: f32, text_width: f32| {
        Rect::from_min_max(pos2(right - text_width - 24.0, cy - 13.0), pos2(right, cy + 13.0))
    };
    let window = button(magnet.left() - 8.0, button_text_widths[0]);
    let share = button(window.left() - 8.0, button_text_widths[1]);
    let export = button(share.left() - 8.0, button_text_widths[2]);
    let search_right = export.left() - 8.0;
    let search = Rect::from_min_max(pos2(search_right - search_width, cy - 12.0), pos2(search_right, cy + 12.0));
    let menu = Rect::from_min_size(pos2(bar.left() + chrome.lead, bar.top()), vec2(36.0, bar.height()));
    let tabs_right = search.left() - 12.0;
    const PLUS_W: f32 = 32.0;
    // reserve the `+` chip's own width BEFORE fitting tabs (F15: it must never be starved out).
    let fit_right = tabs_right - PLUS_W;
    let tab_w = |text_width: f32| (12.0 + text_width + 8.0 + 18.0 + 4.0).clamp(76.0, 220.0);
    let start_x = menu.right() + 8.0;
    let mut tx = start_x;
    let mut tabs: Vec<(usize, Rect)> = Vec::new();
    for (i, &text_width) in tab_text_widths.iter().enumerate() {
        let tw = tab_w(text_width);
        if tx + tw > fit_right {
            break;
        }
        tabs.push((i, Rect::from_min_size(pos2(tx, cy - 14.0), vec2(tw, 28.0))));
        tx += tw + TAB_GAP;
    }
    // F7: the active tab is ALWAYS visible — on overflow it takes the last visible slot.
    if let Some(active) = active_tab {
        if active < tab_text_widths.len() && !tabs.iter().any(|&(i, _)| i == active) {
            let slot_x = tabs.last().map_or(start_x, |&(_, r)| r.left());
            tabs.pop();
            let tw = tab_w(tab_text_widths[active]);
            tabs.push((active, Rect::from_min_size(pos2(slot_x, cy - 14.0), vec2(tw, 28.0))));
            tx = slot_x + tw + TAB_GAP;
        }
    }
    // clamp so a wider swapped-in active tab can never push `+` out of the bar.
    let plus_left = tx.min(tabs_right - PLUS_W).max(start_x);
    let plus = Some(Rect::from_min_size(pos2(plus_left, cy - 14.0), vec2(PLUS_W, 28.0)));
    TopbarLayout { caps, menu, magnet, window, share, export, search, tabs, plus }
}

impl TopbarLayout {
    /// Every bar rect a press BELONGS to (a control, a tab chip's FULL slot — its × lives inside it —
    /// the `+` chip, the burger, the right cluster, Windows' caps). This one list is what the bar
    /// publishes as the caption exclusions (`caption_exclusions` → `cursors::set_caption`), so the
    /// Windows `WM_NCHITTEST` band and the macOS `caption_drag_hit` test exactly the rects the strip
    /// draws and hit-tests — never a second, hand-kept copy (P15).
    pub fn interactive_rects(&self) -> Vec<egui::Rect> {
        self.interactive_rects_with(self.tabs.iter().map(|&(_, r)| r))
    }

    /// `interactive_rects` with the tab chips' rects supplied by the caller: while a chip is lifted
    /// the strip publishes the rects it actually PAINTS (`TabDragFrame::chip_rects`) instead of the
    /// resting slots — the same "published == painted" invariant (P15, P16 review).
    pub fn interactive_rects_with(&self, chips: impl IntoIterator<Item = egui::Rect>) -> Vec<egui::Rect> {
        let mut out: Vec<egui::Rect> = self.caps.map_or_else(Vec::new, |c| c.to_vec());
        out.extend([self.magnet, self.window, self.share, self.export, self.search, self.menu]);
        out.extend(chips);
        out.extend(self.plus);
        out
    }

    /// The tab strip's horizontal extent: from the first drawn chip's left edge to the last one's
    /// right edge. A lifted (dragged) chip is clamped to this span (P16) — exactly the span the
    /// reflowed chips fill, so the lifted chip can always reach the first and the last slot.
    pub fn tab_strip(&self) -> Option<egui::Rangef> {
        let first = self.tabs.first()?.1;
        let last = self.tabs.last()?.1;
        Some(egui::Rangef::new(first.left(), last.right()))
    }
}

/// Logical rects → the physical-px `[l, t, r, b]` exclusions `caption_hit` reads. Rounded OUTWARD
/// (floor / ceil), so a fractional scale factor can only grow a control's no-drag area, never shave a
/// sliver off its edge that would start a window drag.
pub fn caption_exclusions(rects: &[egui::Rect], pixels_per_point: f32) -> Vec<[i32; 4]> {
    rects
        .iter()
        .map(|r| {
            let s = |v: f32, up: bool| {
                (if up { (v * pixels_per_point).ceil() } else { (v * pixels_per_point).floor() }) as i32
            };
            [s(r.left(), false), s(r.top(), false), s(r.right(), true), s(r.bottom(), true)]
        })
        .collect()
}

pub fn tab_close_rect(tab: egui::Rect) -> egui::Rect {
    egui::Rect::from_center_size(egui::pos2(tab.right() - 13.0, tab.center().y), egui::vec2(18.0, 18.0))
}

/// How far (logical pt) past a neighbour's midpoint the lifted chip's leading edge must go before
/// the gap jumps over that neighbour — and, once it has, how far back before it jumps back (P16
/// review). Pointer jitter sitting on a boundary can never toggle the gap.
pub const TAB_DRAG_HYSTERESIS: f32 = 2.0;

/// One frame of a tab drag (P16): where every drawn chip is painted while one of them is LIFTED.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TabDragFrame {
    /// The dragged chip: its resting size, under the pointer (grab offset kept), y locked to the
    /// strip, clamped to the strip's span. Painted last, above the others.
    pub lifted: egui::Rect,
    /// The dragged chip's resting slot — where the press began.
    pub home: egui::Rect,
    /// Every OTHER drawn chip, left → right, as `(original tab index, reflowed rect)`: packed with
    /// `TAB_GAP` from the strip's start, leaving a gap of the lifted chip's width at `landing`.
    pub others: Vec<(usize, egui::Rect)>,
    /// Where the gap is: an insertion slot `0..=others.len()` in `others`' order.
    pub landing: usize,
    /// The gap itself: the rect the lifted chip lands in on release.
    pub gap: egui::Rect,
}

impl TabDragFrame {
    /// The chip rects a press belongs to during this frame: every rect PAINTED (the reflowed chips
    /// and the lifted one) plus the dragged chip's resting slot. The bar publishes these as its caption
    /// exclusions while a chip is lifted (`TopbarLayout::interactive_rects_with`), keeping P15's
    /// "published == painted" — neither the lifted chip nor the press's origin can be a caption spot.
    pub fn chip_rects(&self) -> Vec<egui::Rect> {
        self.others.iter().map(|&(_, r)| r).chain([self.lifted, self.home]).collect()
    }
}

/// The pure geometry of a tab drag (P16). `chips` = the drawn chips at rest (`TopbarLayout::tabs`),
/// `dragged` = the lifted chip's position IN `chips`, `grab_dx` = pointer x − chip left at the press,
/// `pointer_x` = the pointer now, `strip` = the span the lifted chip is clamped to
/// (`TopbarLayout::tab_strip`), `prev_landing` = last frame's `landing` (`dragged` on the first
/// frame: the gap starts at home). `None` only when `dragged` is not a drawn chip.
///
/// A neighbour sits BEFORE the gap while its resting midpoint lies left of the lifted chip's leading
/// edge — the left edge for chips left of home, the right edge for chips right of home — i.e. the gap
/// jumps over a neighbour once the lifted chip covers half of it. One comparison for both
/// directions (`edge > midpoint`), with `TAB_DRAG_HYSTERESIS` against jitter. (The centre would not
/// do: a wide chip clamped at the end of the strip could never pass a narrower end chip's midpoint,
/// so the first / last slot would be unreachable.) No time, no interpolation: the gap is where it is.
pub(crate) fn tab_drag_frame(
    chips: &[(usize, egui::Rect)],
    dragged: usize,
    grab_dx: f32,
    pointer_x: f32,
    strip: egui::Rangef,
    prev_landing: usize,
) -> Option<TabDragFrame> {
    let &(_, home) = chips.get(dragged)?;
    let w = home.width();
    let left = (pointer_x - grab_dx).clamp(strip.min, (strip.max - w).max(strip.min));
    let lifted = egui::Rect::from_min_size(egui::pos2(left, home.top()), home.size());
    // `k` = the neighbour's position among the OTHER chips; it was before the gap iff k < prev_landing
    let before = |k: usize, edge: f32, mid: f32| {
        let h = if k < prev_landing { -TAB_DRAG_HYSTERESIS } else { TAB_DRAG_HYSTERESIS };
        edge > mid + h
    };
    let landing = chips
        .iter()
        .enumerate()
        .filter(|&(k, _)| k != dragged)
        .enumerate()
        .filter(|&(k, (j, &(_, r)))| {
            let edge = if j < dragged { lifted.left() } else { lifted.right() };
            before(k, edge, r.center().x)
        })
        .count();
    let slot_at = |x: f32| egui::Rect::from_min_size(egui::pos2(x, home.top()), home.size());
    let mut x = chips[0].1.left();
    let mut gap = None;
    let mut others = Vec::with_capacity(chips.len() - 1);
    for (k, &(i, r)) in chips.iter().enumerate().filter(|&(k, _)| k != dragged).map(|(_, c)| c).enumerate() {
        if k == landing {
            gap = Some(slot_at(x));
            x += w + TAB_GAP;
        }
        others.push((i, egui::Rect::from_min_size(egui::pos2(x, r.top()), r.size())));
        x += r.width() + TAB_GAP;
    }
    let gap = gap.unwrap_or_else(|| slot_at(x));
    Some(TabDragFrame { lifted, home, others, landing, gap })
}

/// The full order after `Workspace::reorder(dragged, slot)`, as original indices `0..n`.
fn reordered(n: usize, dragged: usize, slot: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    let slot = slot.min(n);
    let dest = if slot > dragged { slot - 1 } else { slot };
    let d = order.remove(dragged);
    order.insert(dest, d);
    order
}

/// The FULL-order insertion slot a release commits (`ReorderDocument` → `Workspace::reorder`) for a
/// chip dropped into the gap (P16 review). `n` = tabs in the full order, `dragged` = the lifted tab's
/// full-order index, `others` = the other DRAWN tabs' full-order indices left → right (as painted
/// around the gap), `landing` = the gap's slot among them, `drawn_after(order)` = which tabs the
/// strip would draw for a full order (original indices in their new order) — the real
/// `topbar_layout`, never a second copy of the fitting rules.
///
/// On overflow the drawn chips are not contiguous in the full order (hidden tabs; the active one
/// displaced into the last drawn slot — S1 F7), so "the slot before chip B" can sit behind hidden
/// tabs and the dropped tab would vanish. Instead every slot is tried and the best kept: the
/// dropped tab must be DRAWN afterwards; then its drawn neighbours should be exactly the ones it was
/// dropped between (`others[landing - 1]` / `others[landing]`); then it should sit as close as
/// possible to the gap's position; ties go to the smallest move in the full order (so a drop back
/// into its own gap is a no-op, and hidden tabs are never reshuffled for nothing). When the gap is past
/// a displaced active chip no single move can draw the tab right of it (the active one keeps the
/// last drawn slot), so it lands just left of it — visible, one slot from the drop.
pub(crate) fn visible_drop_slot(
    n: usize,
    dragged: usize,
    others: &[usize],
    landing: usize,
    drawn_after: impl Fn(&[usize]) -> Vec<usize>,
) -> usize {
    let want_left = landing.checked_sub(1).and_then(|l| others.get(l).copied());
    let want_right = others.get(landing).copied();
    let score = |slot: usize| {
        let order = reordered(n, dragged, slot);
        // how far the tab moves in the FULL order — a drop back into its own gap is a no-op
        let moved = order.iter().position(|&i| i == dragged).map_or(usize::MAX, |d| d.abs_diff(dragged));
        let drawn = drawn_after(&order);
        let Some(pos) = drawn.iter().position(|&i| i == dragged) else {
            return (1, 2, usize::MAX, moved);
        };
        let left = pos.checked_sub(1).map(|p| drawn[p]);
        let right = drawn.get(pos + 1).copied();
        let mismatch = usize::from(left != want_left) + usize::from(right != want_right);
        (0, mismatch, pos.abs_diff(landing), moved)
    };
    (0..=n).min_by_key(|&slot| score(slot)).unwrap_or(dragged)
}

/// Is the window opaque from the first frame? macOS: yes — a transparent NSWindow let the title strip
/// show the desktop through (Ahmed 2026-09-23), so the splash card sits on the dark window instead of
/// floating over the desktop. Windows keeps its transparent floating splash.
pub const OPAQUE_WINDOW: bool = cfg!(target_os = "macos");

/// Is physical-px point (x, y) inside the caption band of height `h` and NOT on one of the bar's
/// interactive rects (`[l, t, r, b]`, physical px, as `cursors::set_caption` receives them)?
pub fn caption_hit(h: i32, excl: &[[i32; 4]], x: i32, y: i32) -> bool {
    y >= 0 && y < h && x >= 0 && !excl.iter().any(|r| x >= r[0] && x < r[2] && y >= r[1] && y < r[3])
}

/// A menu key equivalent: ⌘ + optional ⇧ / ⌥ + key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Accel {
    pub code: KeyCode,
    pub shift: bool,
    pub alt: bool,
}
const fn cmd(code: KeyCode) -> Option<Accel> {
    Some(Accel { code, shift: false, alt: false })
}
const fn cmd_shift(code: KeyCode) -> Option<Accel> {
    Some(Accel { code, shift: true, alt: false })
}
const fn cmd_alt(code: KeyCode) -> Option<Accel> {
    Some(Accel { code, shift: false, alt: true })
}

/// A native File-menu row's (and Varos ▸ Quit's) lifecycle identity (DFS S1 §3.6 / review F5). These
/// dispatch through `MenuCmd::File`, never through `MenuCmd::Key`'s synthetic-keystroke path (spec
/// §4: "Menus and physical keys dispatch once through command IDs, not synthetic key events") — a
/// focused text field must not swallow ⌘S. S1-D's `to_app_command` is the one place that turns a
/// `FileCmd` into an `AppCommand`; S6-C later adds `Export` to this same enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileCmd {
    New,
    Open,
    CloseTab,
    Save,
    SaveAs,
    Quit,
}

/// What a clicked item does — each one an EXISTING path in the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuCmd {
    /// The ⌘ + key shortcut, fed to the same dispatch the keyboard uses (`main.rs`).
    Key(Accel),
    /// A PLAIN key (no modifier) fed to that same dispatch — for a click-only row that shows NO key
    /// equivalent, so AppKit never takes the key from a focused text field (e.g. Edit ▸ Delete runs
    /// the Delete/Backspace path, while Backspace keeps deleting text in a field). The host runs it
    /// only when no text field wants the keyboard.
    Plain(KeyCode),
    /// A File-menu row (or Varos ▸ Quit) — see `FileCmd`.
    File(FileCmd),
    /// The bar's Window menu rows.
    ToggleRail,
    ToggleDock,
    TogglePanel(PanelId),
    /// The magnet (Snapping) quick-menu rows.
    SnapGrid,
    SnapPoint,
}

/// A check mark, read back from the real state every frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    Rulers,
    Guides,
    GuidesLocked,
    SmartGuides,
    SnapGrid,
    SnapPoint,
    Rail,
    Dock,
    Panel(PanelId),
}

/// Standard macOS items that AppKit itself performs (no Varos behaviour behind them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Native {
    About,
    Services,
    Hide,
    HideOthers,
    ShowAll,
    Minimize,
    Zoom,
    Fullscreen,
    BringAllToFront,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Item { id: String, label: &'static str, accel: Option<Accel>, cmd: MenuCmd, check: Option<Check> },
    Sub { label: &'static str, items: Vec<Entry> },
    Native(Native),
    Sep,
}

fn item(id: &str, label: &'static str, accel: Option<Accel>, cmd: MenuCmd) -> Entry {
    Entry::Item { id: id.into(), label, accel, cmd, check: None }
}
/// A shortcut item: the accelerator shown IS the keystroke it sends.
fn key(id: &str, label: &'static str, a: Option<Accel>) -> Entry {
    let acc = a.expect("a shortcut item has a key");
    Entry::Item { id: id.into(), label, accel: a, cmd: MenuCmd::Key(acc), check: None }
}
fn key_check(id: &str, label: &'static str, a: Option<Accel>, check: Check) -> Entry {
    let acc = a.expect("a shortcut item has a key");
    Entry::Item { id: id.into(), label, accel: a, cmd: MenuCmd::Key(acc), check: Some(check) }
}
/// A File-menu row (and Varos ▸ Quit): the accelerator shown IS the keystroke `FileCmd` runs — never
/// `MenuCmd::Key`'s text-field-forwarding path (review F5).
fn file_key(id: &str, label: &'static str, a: Option<Accel>, cmd: FileCmd) -> Entry {
    let accel = a.expect("a shortcut item has a key");
    Entry::Item { id: id.into(), label, accel: Some(accel), cmd: MenuCmd::File(cmd), check: None }
}
fn toggle(id: &str, label: &'static str, cmd: MenuCmd, check: Check) -> Entry {
    Entry::Item { id: id.into(), label, accel: None, cmd, check: Some(check) }
}

/// The whole menu bar, left → right. The first menu is the application menu (macOS titles it with
/// the app's name whatever label it gets).
pub fn menus() -> Vec<(&'static str, Vec<Entry>)> {
    use KeyCode as K;
    let mut window = vec![
        Entry::Native(Native::Minimize),
        Entry::Native(Native::Zoom),
        Entry::Sep,
        toggle("win.rail", "Tool rail", MenuCmd::ToggleRail, Check::Rail),
        toggle("win.dock", "Control bar", MenuCmd::ToggleDock, Check::Dock),
        Entry::Sep,
    ];
    for p in PanelId::DOCKABLE {
        window.push(Entry::Item {
            id: format!("win.panel.{}", p.title()),
            label: p.title(),
            accel: None,
            cmd: MenuCmd::TogglePanel(p),
            check: Some(Check::Panel(p)),
        });
    }
    window.extend([Entry::Sep, Entry::Native(Native::BringAllToFront)]);
    vec![
        (
            "Varos",
            vec![
                Entry::Native(Native::About),
                Entry::Sep,
                Entry::Native(Native::Services),
                Entry::Sep,
                Entry::Native(Native::Hide),
                Entry::Native(Native::HideOthers),
                Entry::Native(Native::ShowAll),
                Entry::Sep,
                file_key("app.quit", "Quit Varos", cmd(K::KeyQ), FileCmd::Quit),
            ],
        ),
        (
            "File",
            vec![
                file_key("file.new", "New", cmd(K::KeyN), FileCmd::New),
                file_key("file.open", "Open\u{2026}", cmd(K::KeyO), FileCmd::Open),
                Entry::Sep,
                file_key("file.close", "Close Tab", cmd(K::KeyW), FileCmd::CloseTab),
                file_key("file.save", "Save", cmd(K::KeyS), FileCmd::Save),
                file_key("file.saveas", "Save As\u{2026}", cmd_shift(K::KeyS), FileCmd::SaveAs),
            ],
        ),
        (
            "Edit",
            vec![
                key("edit.undo", "Undo", cmd(K::KeyZ)),
                key("edit.redo", "Redo", cmd_shift(K::KeyZ)),
                Entry::Sep,
                // the in-app clipboard (Astra F04): ⌘C / ⌘X via `apply_key`, ⌘V / ⇧⌘V via the shortcut
                // path's view-centred paste. In a focused text field `forward_shortcut` turns these
                // into egui's own Copy / Cut / Paste events, so field editing keeps working.
                key("edit.cut", "Cut", cmd(K::KeyX)),
                key("edit.copy", "Copy", cmd(K::KeyC)),
                key("edit.paste", "Paste", cmd(K::KeyV)),
                key("edit.pasteinplace", "Paste in Place", cmd_shift(K::KeyV)),
                // click-only: the Delete/Backspace key path, with no key equivalent (so a text field
                // keeps its Backspace)
                item("edit.delete", "Delete", None, MenuCmd::Plain(K::Backspace)),
                Entry::Sep,
                // ⌘A / ⇧⌘A via `apply_key`; in a focused text field ⌘A is handed to the field (select text)
                key("edit.selectall", "Select All", cmd(K::KeyA)),
                key("edit.deselect", "Deselect", cmd_shift(K::KeyA)),
            ],
        ),
        (
            "Object",
            vec![
                key("obj.again", "Transform Again", cmd(K::KeyD)),
                Entry::Sep,
                Entry::Sub {
                    label: "Arrange",
                    items: vec![
                        key("obj.front", "Bring to Front", cmd_shift(K::BracketRight)),
                        key("obj.forward", "Bring Forward", cmd(K::BracketRight)),
                        key("obj.backward", "Send Backward", cmd(K::BracketLeft)),
                        key("obj.back", "Send to Back", cmd_shift(K::BracketLeft)),
                    ],
                },
                Entry::Sep,
                key("obj.group", "Group", cmd(K::KeyG)),
                key("obj.ungroup", "Ungroup", cmd_shift(K::KeyG)),
            ],
        ),
        (
            "View",
            vec![
                key("view.fit", "Fit in Window", cmd(K::Digit0)),
                key("view.actual", "Actual Size", cmd(K::Digit1)),
                key("view.zoomin", "Zoom In", cmd(K::Equal)),
                key("view.zoomout", "Zoom Out", cmd(K::Minus)),
                Entry::Sep,
                key_check("view.rulers", "Rulers", cmd(K::KeyR), Check::Rulers),
                key_check("view.guides", "Guides", cmd(K::Semicolon), Check::Guides),
                key_check("view.lockguides", "Lock Guides", cmd_alt(K::Semicolon), Check::GuidesLocked),
                key_check("view.smart", "Smart Guides", cmd(K::KeyU), Check::SmartGuides),
                Entry::Sep,
                toggle("view.snapgrid", "Snap to Grid", MenuCmd::SnapGrid, Check::SnapGrid),
                toggle("view.snappoint", "Snap to Point", MenuCmd::SnapPoint, Check::SnapPoint),
                Entry::Sep,
                Entry::Native(Native::Fullscreen),
            ],
        ),
        ("Window", window),
    ]
}

/// Every clickable item in the bar (depth-first) — the table checks in the tests walk it.
#[cfg(test)]
pub fn flat_items(menus: &[(&'static str, Vec<Entry>)]) -> Vec<Entry> {
    fn walk(v: &[Entry], out: &mut Vec<Entry>) {
        for e in v {
            match e {
                Entry::Item { .. } => out.push(e.clone()),
                Entry::Sub { items, .. } => walk(items, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    for (_, v) in menus {
        walk(v, &mut out);
    }
    out
}

/// The egui key for a shortcut key — used to hand a menu keystroke to a focused text field, exactly
/// as the keyboard would have. Covers every key the menu table uses (tested).
pub fn egui_key(code: KeyCode) -> Option<egui::Key> {
    use egui::Key as E;
    use KeyCode as K;
    Some(match code {
        K::KeyA => E::A,
        K::KeyC => E::C,
        K::KeyD => E::D,
        K::KeyG => E::G,
        K::KeyN => E::N,
        K::KeyO => E::O,
        K::KeyQ => E::Q,
        K::KeyR => E::R,
        K::KeyS => E::S,
        K::KeyU => E::U,
        K::KeyV => E::V,
        K::KeyW => E::W,
        K::KeyX => E::X,
        K::KeyZ => E::Z,
        K::Digit0 => E::Num0,
        K::Digit1 => E::Num1,
        K::Equal => E::Equals,
        K::Minus => E::Minus,
        K::Semicolon => E::Semicolon,
        K::BracketLeft => E::OpenBracket,
        K::BracketRight => E::CloseBracket,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_topbar_numbers_are_unchanged_and_mac_clears_the_traffic_lights() {
        let win = topbar_chrome(false);
        assert_eq!(win, TopbarChrome { height: 46.0, lead: 4.0, right_inset: 0.0, window_caps: true });
        let mac = topbar_chrome(true);
        assert!(!mac.window_caps, "macOS uses the native traffic lights, never our ─ ☐ ✕");
        assert!(mac.lead >= 72.0, "the three traffic lights end ≈ 70 pt from the left edge");
        assert!(mac.right_inset > 0.0);
        assert_eq!(TOPBAR, topbar_chrome(cfg!(target_os = "macos")));
    }

    #[test]
    fn mac_topbar_controls_share_the_native_traffic_light_centre() {
        let chrome = topbar_chrome(true);
        // Include a translated bar, minimum window width, overflow tabs and a wide window.
        for origin in [egui::pos2(0.0, 0.0), egui::pos2(31.0, 47.0)] {
            for width in [800.0, 1280.0, 1920.0] {
                let bar = egui::Rect::from_min_size(origin, egui::vec2(width, chrome.height));
                let layout = topbar_layout(bar, chrome, [47.0, 34.0, 39.0], 120.0, &[65.0, 180.0, 300.0], Some(0));
                assert!(layout.caps.is_none());
                assert!(!layout.tabs.is_empty());
                assert!(layout.plus.is_some(), "the + chip is reserved before tabs are fitted (F15)");
                let tab_rects: Vec<egui::Rect> = layout.tabs.iter().map(|&(_, r)| r).collect();
                let controls = [layout.menu, layout.magnet, layout.window, layout.share, layout.export, layout.search]
                    .into_iter()
                    .chain(tab_rects.iter().copied())
                    .chain(tab_rects.iter().copied().map(tab_close_rect))
                    .chain(layout.plus);
                let traffic_light_centre = bar.top() + 14.0;
                for rect in controls {
                    assert!(bar.contains_rect(rect), "control {rect:?} escapes bar {bar:?}");
                    assert!(
                        (rect.center().y - traffic_light_centre).abs() <= 1.0,
                        "control {rect:?} is not centred on traffic lights at {traffic_light_centre}"
                    );
                }
            }
        }
    }

    #[test]
    fn plus_is_always_placed_even_with_overflowing_tabs() {
        let chrome = topbar_chrome(true);
        let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, chrome.height));
        let widths = vec![180.0; 20]; // far more than fit
        let layout = topbar_layout(bar, chrome, [47.0, 34.0, 39.0], 120.0, &widths, Some(0));
        assert!(layout.tabs.len() < widths.len(), "the strip really is overflowing here");
        let plus = layout.plus.expect("+ must survive overflow");
        assert!(bar.contains_rect(plus), "+ escapes the bar: {plus:?}");
        // + never overlaps a placed tab
        for &(_, r) in &layout.tabs {
            assert!(!r.intersects(plus), "tab {r:?} overlaps +");
        }
    }

    #[test]
    fn active_tab_is_placed_when_tabs_overflow() {
        let chrome = topbar_chrome(true);
        let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, chrome.height));
        let widths = vec![180.0; 12];
        let last = widths.len() - 1;
        let layout = topbar_layout(bar, chrome, [47.0, 34.0, 39.0], 120.0, &widths, Some(last));
        // a greedy fit alone would never reach the last tab — confirm this scenario really overflows
        let greedy = topbar_layout(bar, chrome, [47.0, 34.0, 39.0], 120.0, &widths, None);
        assert!(!greedy.tabs.iter().any(|&(i, _)| i == last), "test setup: the last tab must overflow");
        assert!(layout.tabs.iter().any(|&(i, _)| i == last), "the active (last) tab must still be placed");
        let (_, active_rect) = *layout.tabs.last().expect("at least one tab is placed");
        assert!(bar.contains_rect(active_rect), "the active tab's chip escapes the bar");
        assert!(layout.plus.is_some(), "+ still survives once the active tab claims a slot");
    }

    /// Drawn chips `(tab index, rect)` of the given widths, packed from x = 0 with `TAB_GAP`.
    fn chips(widths: &[f32], indices: &[usize]) -> Vec<(usize, egui::Rect)> {
        let mut x = 0.0;
        widths
            .iter()
            .zip(indices)
            .map(|(&w, &i)| {
                let r = egui::Rect::from_min_size(egui::pos2(x, 9.0), egui::vec2(w, 28.0));
                x += w + TAB_GAP;
                (i, r)
            })
            .collect()
    }
    fn span(c: &[(usize, egui::Rect)]) -> egui::Rangef {
        egui::Rangef::new(c[0].1.left(), c.last().unwrap().1.right())
    }
    fn lefts(f: &TabDragFrame) -> Vec<(usize, f32)> {
        f.others.iter().map(|&(i, r)| (i, r.left())).collect()
    }
    /// One drag frame of chip `d` (grab 0) at pointer `x`, the gap last at `prev`.
    fn drag(c: &[(usize, egui::Rect)], d: usize, x: f32, prev: usize) -> TabDragFrame {
        tab_drag_frame(c, d, 0.0, x, span(c), prev).unwrap()
    }

    #[test]
    fn lifted_chip_follows_the_pointer_with_its_grab_offset_and_is_clamped() {
        let c = chips(&[80.0, 80.0, 80.0], &[0, 1, 2]); // [0,80) [84,164) [168,248)
        let f = |x: f32| tab_drag_frame(&c, 1, 10.0, x, span(&c), 1).unwrap();
        assert_eq!(f(100.0).lifted, egui::Rect::from_min_size(egui::pos2(90.0, 9.0), egui::vec2(80.0, 28.0)));
        assert_eq!(f(100.0).home, c[1].1, "the resting slot is kept for the caption exclusions");
        assert_eq!(f(-50.0).lifted.left(), 0.0, "clamped to the strip's start");
        assert_eq!(f(999.0).lifted.right(), 248.0, "clamped to the strip's end");
        assert_eq!(f(100.0).lifted.top(), 9.0, "y is the strip's, never the pointer's");
        assert!(tab_drag_frame(&c, 3, 0.0, 0.0, span(&c), 3).is_none(), "not a drawn chip");
    }

    #[test]
    fn the_gap_jumps_a_neighbour_once_the_leading_edge_clears_its_midpoint_both_ways() {
        // three 80-wide chips: midpoints 40, 124, 208; hysteresis 2
        let c = chips(&[80.0, 80.0, 80.0], &[0, 1, 2]);
        assert_eq!(TAB_DRAG_HYSTERESIS, 2.0);

        // chip 0 dragged right: its right edge (x + 80) must clear chip 1's midpoint + 2 = 126
        assert_eq!((drag(&c, 0, 46.0, 0).landing, lefts(&drag(&c, 0, 46.0, 0))), (0, vec![(1, 84.0), (2, 168.0)]));
        let f = drag(&c, 0, 46.1, 0);
        assert_eq!((f.landing, lefts(&f), f.gap.left()), (1, vec![(1, 0.0), (2, 168.0)], 84.0), "1 hops left");
        // …and back only once the same edge is 2 short of the midpoint: 122
        assert_eq!(drag(&c, 0, 42.1, 1).landing, 1, "inside the band: stays");
        assert_eq!(drag(&c, 0, 41.9, 1).landing, 0, "back past it: 1 returns");
        assert_eq!(drag(&c, 0, 130.1, 0).landing, 2, "a fast move clears both neighbours in one frame");

        // chip 2 dragged left: the SAME comparison on its left edge — chip 1 stays before the gap
        // while x > 124 − 2, and gets back before it only once x > 124 + 2
        assert_eq!(drag(&c, 2, 122.1, 2).landing, 2, "inside the band: stays");
        let g = drag(&c, 2, 121.9, 2);
        assert_eq!((g.landing, lefts(&g)), (1, vec![(0, 0.0), (1, 168.0)]), "1 hops right");
        assert_eq!(drag(&c, 2, 125.9, 1).landing, 1, "inside the band: stays");
        assert_eq!(drag(&c, 2, 126.1, 1).landing, 2, "back past it: 1 returns");
        assert_eq!(drag(&c, 2, 37.9, 2).landing, 0, "past chip 0's midpoint too");
    }

    #[test]
    fn pointer_jitter_on_a_boundary_never_toggles_the_gap() {
        let c = chips(&[80.0, 80.0, 80.0], &[0, 1, 2]);
        // chip 0 dragged right; the boundary for chip 1 is at x = 44 (right edge on 124)
        let path = [44.0, 44.9, 43.1, 46.2, 44.0, 45.9, 42.1, 44.0, 41.8, 44.0, 45.5];
        let want = [0, 0, 0, 1, 1, 1, 1, 1, 0, 0, 0];
        let mut prev = 0;
        for (&x, &w) in path.iter().zip(&want) {
            prev = drag(&c, 0, x, prev).landing;
            assert_eq!(prev, w, "pointer at {x}");
        }
        // mirrored: chip 2 dragged left; the boundary for chip 1 is at x = 124
        let path = [124.0, 123.1, 124.9, 121.8, 124.0, 125.9, 126.3, 124.0];
        let want = [2, 2, 2, 1, 1, 1, 2, 2];
        let mut prev = 2;
        for (&x, &w) in path.iter().zip(&want) {
            prev = drag(&c, 2, x, prev).landing;
            assert_eq!(prev, w, "pointer at {x}");
        }
    }

    #[test]
    fn a_wide_chip_reaches_both_ends_past_narrower_ones() {
        // the reason for the leading edge, not the centre: a 220-wide chip clamped at the end has its
        // centre at 190 — it could never pass the 76-wide end chip's midpoint (262) by its centre.
        let c = chips(&[220.0, 76.0], &[0, 1]);
        let f = drag(&c, 0, 9999.0, 0);
        assert_eq!((f.lifted.right(), f.landing), (300.0, 1), "lands after the narrow chip");
        let c = chips(&[76.0, 220.0], &[0, 1]);
        let f = drag(&c, 1, -9999.0, 1);
        assert_eq!((f.lifted.left(), f.landing), (0.0, 0), "lands before the narrow chip");
    }

    #[test]
    fn reflowed_chips_and_the_gap_tile_the_strip_exactly() {
        let c = chips(&[90.0, 150.0, 76.0, 120.0], &[0, 1, 2, 3]);
        let strip = span(&c);
        for dragged in 0..c.len() {
            let mut prev = dragged;
            for x in (-100..700).step_by(7) {
                let f = tab_drag_frame(&c, dragged, 30.0, x as f32, strip, prev).unwrap();
                prev = f.landing;
                assert!(f.lifted.left() >= strip.min && f.lifted.right() <= strip.max);
                assert_eq!(f.gap.width(), c[dragged].1.width());
                let mut row: Vec<egui::Rect> = f.others.iter().map(|&(_, r)| r).collect();
                row.insert(f.landing, f.gap);
                assert_eq!(row[0].left(), strip.min);
                assert_eq!(row.last().unwrap().right(), strip.max, "same span as at rest");
                for w in row.windows(2) {
                    assert_eq!(w[1].left() - w[0].right(), TAB_GAP, "packed with the one gap");
                }
            }
        }
    }

    /// The real `topbar_layout` for `n` tabs every `text` wide in a `width`-wide Mac bar, tab
    /// `active` active: "which tabs are drawn for this full order" — exactly what the strip hands
    /// `visible_drop_slot`.
    fn drawn_for(n: usize, active: usize, width: f32, text: f32) -> impl Fn(&[usize]) -> Vec<usize> {
        move |order: &[usize]| {
            let chrome = topbar_chrome(true);
            let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(width, chrome.height));
            let act = order.iter().position(|&i| i == active);
            let l = topbar_layout(bar, chrome, [47.0, 34.0, 39.0], 120.0, &vec![text; n], act);
            l.tabs.iter().map(|&(k, _)| order[k]).collect()
        }
    }
    /// Drop tab `d` into the gap at `landing` among the drawn `others`; the drawn tabs afterwards.
    fn drop_and_draw(n: usize, drawn: &impl Fn(&[usize]) -> Vec<usize>, d: usize, landing: usize) -> Vec<usize> {
        let before = drawn(&(0..n).collect::<Vec<_>>());
        let others: Vec<usize> = before.into_iter().filter(|&i| i != d).collect();
        let slot = visible_drop_slot(n, d, &others, landing, drawn);
        drawn(&reordered(n, d, slot))
    }

    #[test]
    fn overflow_drop_lands_between_its_visible_neighbours_and_stays_visible() {
        // [0..9], 9 active: drawn [0, 1, 2, 9] — 3..8 hidden between 2 and 9
        let (n, drawn) = (10, drawn_for(10, 9, 900.0, 40.0));
        assert_eq!(drawn(&(0..n).collect::<Vec<_>>()), [0, 1, 2, 9], "setup");
        // 0 dropped between 2 and 9 — NOT behind the hidden 3..8 (it used to vanish there)
        assert_eq!(visible_drop_slot(n, 0, &[1, 2, 9], 2, &drawn), 3);
        assert_eq!(drop_and_draw(n, &drawn, 0, 2), [1, 2, 0, 9]);
        assert_eq!(drop_and_draw(n, &drawn, 0, 1), [1, 0, 2, 9], "between 1 and 2");
        // the active 9 dragged to the front / between 1 and 2 / back home
        assert_eq!(drop_and_draw(n, &drawn, 9, 0), [9, 0, 1, 2]);
        assert_eq!(drop_and_draw(n, &drawn, 9, 2), [0, 1, 9, 2]);
        assert_eq!(reordered(n, 9, visible_drop_slot(n, 9, &[0, 1, 2], 3, &drawn)), (0..n).collect::<Vec<_>>());
        // a chip dropped back into its own gap changes nothing
        assert_eq!(reordered(n, 1, visible_drop_slot(n, 1, &[0, 2, 9], 1, &drawn)), (0..n).collect::<Vec<_>>());
        // 20 tabs, 12 active (drawn [0, 1, 2, 12]): 0 dropped between 2 and 12
        let drawn20 = drawn_for(20, 12, 900.0, 40.0);
        assert_eq!(drawn20(&(0..20).collect::<Vec<_>>()), [0, 1, 2, 12], "setup");
        assert_eq!(drop_and_draw(20, &drawn20, 0, 2), [1, 2, 0, 12]);
    }

    #[test]
    fn overflow_drop_after_the_last_visible_tab_stays_visible() {
        // past the displaced active 9 no single move can draw 0 to its right (9 keeps the last drawn
        // slot), so 0 lands just left of it: visible, one slot from the drop
        let (n, drawn) = (10, drawn_for(10, 9, 900.0, 40.0));
        assert_eq!(drop_and_draw(n, &drawn, 0, 3), [1, 2, 0, 9]);
        let drawn20 = drawn_for(20, 12, 900.0, 40.0);
        assert_eq!(drop_and_draw(20, &drawn20, 1, 3), [0, 2, 1, 12]);
        // no overflow: after the last visible tab = the very end, exactly
        let drawn3 = drawn_for(3, 0, 1400.0, 40.0);
        assert_eq!(visible_drop_slot(3, 0, &[1, 2], 2, &drawn3), 3);
        assert_eq!(drop_and_draw(3, &drawn3, 0, 2), [1, 2, 0]);
        assert_eq!(drop_and_draw(3, &drawn3, 2, 0), [2, 0, 1]);
        // a single drawn chip (the active one) has nowhere else to go
        let drawn1 = drawn_for(10, 9, 900.0, 180.0);
        assert_eq!(drawn1(&(0..10).collect::<Vec<_>>()), [9], "setup");
        assert_eq!(reordered(10, 9, visible_drop_slot(10, 9, &[], 0, &drawn1)), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn caption_hit_is_the_empty_band_only() {
        let excl = [[100, 0, 200, 92]];
        assert!(caption_hit(92, &excl, 50, 10));
        assert!(caption_hit(92, &excl, 250, 91));
        assert!(!caption_hit(92, &excl, 150, 10), "an interactive rect is not a drag spot");
        assert!(!caption_hit(92, &excl, 50, 92), "below the band");
        assert!(!caption_hit(0, &[], 50, 0), "no band published yet (splash) → never drag");
    }

    /// P15: the caption predicate exactly as both platforms run it — `interactive_rects` published
    /// through `caption_exclusions` at scale `ppp`, then `caption_hit` on a physical-px point. True =
    /// a press here drags the window.
    fn drags_window(layout: &TopbarLayout, chrome: TopbarChrome, ppp: f32, pos: egui::Pos2) -> bool {
        let excl = caption_exclusions(&layout.interactive_rects(), ppp);
        caption_hit((chrome.height * ppp) as i32, &excl, (pos.x * ppp) as i32, (pos.y * ppp) as i32)
    }

    /// Points on a chip's FULL slot a press can land on: centre, the four inner corners, the ×.
    fn slot_points(r: egui::Rect) -> [egui::Pos2; 6] {
        let i = r.shrink(0.5);
        [r.center(), i.left_top(), i.right_top(), i.left_bottom(), i.right_bottom(), tab_close_rect(r).center()]
    }

    /// P16 review: during a LIVE drag frame the bar publishes `interactive_rects_with(chip_rects)` —
    /// every chip where it is painted (reflowed or lifted) plus the lifted chip's resting slot. None of
    /// those, nor `+`, may ever be a caption spot, for every chip lifted at every pointer x.
    fn assert_live_drag_never_drags_the_window(layout: &TopbarLayout, chrome: TopbarChrome, ppp: f32) {
        let strip = layout.tab_strip().expect("chips are drawn");
        for d in 0..layout.tabs.len() {
            let mut prev = d;
            let mut x = strip.min - 60.0;
            while x < strip.max + 60.0 {
                let f = tab_drag_frame(&layout.tabs, d, 20.0, x, strip, prev).expect("a drawn chip");
                prev = f.landing;
                let excl = caption_exclusions(&layout.interactive_rects_with(f.chip_rects()), ppp);
                let drags = |p: egui::Pos2| {
                    caption_hit((chrome.height * ppp) as i32, &excl, (p.x * ppp) as i32, (p.y * ppp) as i32)
                };
                for r in f.chip_rects() {
                    for pos in slot_points(r) {
                        assert!(!drags(pos), "chip {d} lifted at x {x} (ppp {ppp}): {pos:?} on {r:?} drags");
                    }
                }
                assert!(!drags(layout.plus.expect("+ is placed").center()));
                x += 5.5;
            }
        }
    }

    #[test]
    fn a_press_on_any_tab_slot_or_plus_never_drags_the_window() {
        for chrome in [topbar_chrome(true), topbar_chrome(false)] {
            for ppp in [1.0, 1.25, 1.5, 2.0] {
                let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, chrome.height));
                let layout = topbar_layout(bar, chrome, [47.0, 34.0, 39.0], 120.0, &[60.0, 90.0, 120.0], Some(1));
                assert_eq!(layout.tabs.len(), 3, "setup: all three chips drawn");
                for &(i, r) in &layout.tabs {
                    for pos in slot_points(r) {
                        assert!(!drags_window(&layout, chrome, ppp, pos), "tab {i} at {pos:?} (ppp {ppp}) drags");
                    }
                }
                let plus = layout.plus.expect("+ is placed");
                for pos in [plus.center(), plus.shrink(0.5).left_top(), plus.shrink(0.5).right_bottom()] {
                    assert!(!drags_window(&layout, chrome, ppp, pos), "+ at {pos:?} (ppp {ppp}) drags");
                }
                for r in [layout.menu, layout.magnet, layout.window, layout.share, layout.export, layout.search] {
                    assert!(!drags_window(&layout, chrome, ppp, r.center()), "control {r:?} (ppp {ppp}) drags");
                }
                if let Some(caps) = layout.caps {
                    for r in caps {
                        assert!(!drags_window(&layout, chrome, ppp, r.center()), "cap {r:?} drags");
                    }
                }
                assert_live_drag_never_drags_the_window(&layout, chrome, ppp);
            }
        }
    }

    #[test]
    fn a_press_on_empty_bar_space_drags_the_window() {
        for chrome in [topbar_chrome(true), topbar_chrome(false)] {
            for ppp in [1.0, 1.25, 2.0] {
                let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, chrome.height));
                let layout = topbar_layout(bar, chrome, [47.0, 34.0, 39.0], 120.0, &[60.0, 90.0], Some(0));
                let plus = layout.plus.expect("+ is placed");
                let y = bar.center().y;
                // the open stretch between `+` and the search pill — the main drag handle
                let open = egui::pos2((plus.right() + layout.search.left()) / 2.0, y);
                assert!(layout.search.left() - plus.right() > 40.0, "setup: a real empty stretch");
                assert!(drags_window(&layout, chrome, ppp, open), "empty bar at {open:?} (ppp {ppp})");
                // the 4-px gap between two chips is empty bar too
                let gap = egui::pos2((layout.tabs[0].1.right() + layout.tabs[1].1.left()) / 2.0, y);
                assert!(drags_window(&layout, chrome, ppp, gap), "chip gap at {gap:?} (ppp {ppp})");
                // above/below a chip, still inside the band
                let above = egui::pos2(layout.tabs[0].1.center().x, bar.top() + 0.2);
                if layout.tabs[0].1.top() - bar.top() >= 1.0 {
                    assert!(drags_window(&layout, chrome, ppp, above), "band above a chip (ppp {ppp})");
                }
                // below the band is never a caption
                assert!(!drags_window(&layout, chrome, ppp, egui::pos2(open.x, bar.bottom() + 1.0)));
            }
        }
    }

    #[test]
    fn overflow_slots_with_eight_tabs_are_still_covered() {
        let chrome = topbar_chrome(true);
        for active in [0, 4, 7] {
            for width in [800.0, 1100.0] {
                let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(width, chrome.height));
                let layout = topbar_layout(bar, chrome, [47.0, 34.0, 39.0], 120.0, &[140.0; 8], Some(active));
                assert!(layout.tabs.len() < 8, "setup: 8 tabs overflow at width {width}");
                assert!(layout.tabs.iter().any(|&(i, _)| i == active), "setup: the active tab is drawn");
                for ppp in [1.0, 2.0] {
                    for &(i, r) in &layout.tabs {
                        for pos in slot_points(r) {
                            assert!(
                                !drags_window(&layout, chrome, ppp, pos),
                                "8 tabs, active {active}, width {width}: chip {i} at {pos:?} (ppp {ppp}) drags"
                            );
                        }
                    }
                    let plus = layout.plus.expect("+ survives overflow");
                    assert!(!drags_window(&layout, chrome, ppp, plus.center()));
                    assert_live_drag_never_drags_the_window(&layout, chrome, ppp);
                }
            }
        }
    }

    #[test]
    fn caption_exclusions_round_outward() {
        let r = egui::Rect::from_min_max(egui::pos2(10.3, 2.6), egui::pos2(20.2, 30.7));
        assert_eq!(caption_exclusions(&[r], 1.0), [[10, 2, 21, 31]]);
        assert_eq!(caption_exclusions(&[r], 2.0), [[20, 5, 41, 62]]);
    }

    #[test]
    fn every_shortcut_item_shows_exactly_the_keystroke_it_sends() {
        for e in flat_items(&menus()) {
            if let Entry::Item { id, accel, cmd: MenuCmd::Key(k), .. } = e {
                assert_eq!(accel, Some(k), "{id}: shown shortcut ≠ sent keystroke");
            }
        }
    }

    #[test]
    fn ids_and_accelerators_are_unique() {
        let items = flat_items(&menus());
        let mut ids = std::collections::HashSet::new();
        let mut accels = std::collections::HashSet::new();
        for e in &items {
            if let Entry::Item { id, accel, .. } = e {
                assert!(ids.insert(id.clone()), "duplicate id {id}");
                if let Some(a) = accel {
                    assert!(accels.insert(*a), "{id}: accelerator {a:?} used twice");
                }
            }
        }
    }

    #[test]
    fn every_menu_key_can_be_handed_to_a_text_field() {
        for e in flat_items(&menus()) {
            if let Entry::Item { id, accel: Some(a), .. } = e {
                assert!(egui_key(a.code).is_some(), "{id}: no egui key for {:?}", a.code);
            }
        }
    }

    #[test]
    fn every_clipboard_row_is_its_shortcut() {
        let m = menus();
        let (_, edit) = m.iter().find(|(t, _)| *t == "Edit").expect("an Edit menu");
        let rows: Vec<(String, Accel)> = edit
            .iter()
            .filter_map(|e| match e {
                Entry::Item { id, cmd: MenuCmd::Key(k), .. } => Some((id.clone(), *k)),
                _ => None,
            })
            .collect();
        let want = [
            ("edit.cut", cmd(KeyCode::KeyX)),
            ("edit.copy", cmd(KeyCode::KeyC)),
            ("edit.paste", cmd(KeyCode::KeyV)),
            ("edit.pasteinplace", cmd_shift(KeyCode::KeyV)),
        ];
        for (id, a) in want {
            let a = a.unwrap();
            assert!(rows.iter().any(|(i, k)| i == id && *k == a), "Edit menu misses {id} = {a:?}");
            assert!(egui_key(a.code).is_some(), "{id}: a focused text field must still get the key");
        }
    }

    fn edit_rows() -> Vec<Entry> {
        let m = menus();
        m.into_iter().find(|(t, _)| *t == "Edit").expect("an Edit menu").1
    }

    #[test]
    fn edit_menu_mirrors_select_all_deselect_delete() {
        let rows = edit_rows();
        let find = |want: &str| {
            rows.iter()
                .find_map(|e| match e {
                    Entry::Item { id, label, accel, cmd, .. } if id == want => Some((*label, *accel, *cmd)),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Edit menu misses {want}"))
        };
        let all = cmd(KeyCode::KeyA);
        let none = cmd_shift(KeyCode::KeyA);
        assert_eq!(find("edit.selectall"), ("Select All", all, MenuCmd::Key(all.unwrap())));
        assert_eq!(find("edit.deselect"), ("Deselect", none, MenuCmd::Key(none.unwrap())));
        assert_eq!(find("edit.delete"), ("Delete", None, MenuCmd::Plain(KeyCode::Backspace)));
        assert!(egui_key(KeyCode::KeyA).is_some(), "a focused text field must still get ⌘A (select text)");
    }

    #[test]
    fn plain_delete_row_has_no_native_key_equivalent() {
        // every Plain row is click-only: showing a key would let AppKit steal it from a text field
        let items = flat_items(&menus());
        let plain: Vec<_> = items
            .iter()
            .filter_map(|e| match e {
                Entry::Item { id, accel, cmd: MenuCmd::Plain(_), .. } => Some((id.as_str(), *accel)),
                _ => None,
            })
            .collect();
        assert_eq!(plain, [("edit.delete", None)]);
        // …and no row anywhere claims Backspace / Delete as a key equivalent
        for e in &items {
            if let Entry::Item { id, accel: Some(a), .. } = e {
                assert!(!matches!(a.code, KeyCode::Backspace | KeyCode::Delete), "{id} claims {:?}", a.code);
            }
        }
    }

    #[test]
    fn the_bar_has_the_standard_mac_menus_and_mirrors_every_dockable_panel() {
        let m = menus();
        let titles: Vec<&str> = m.iter().map(|(t, _)| *t).collect();
        assert_eq!(titles, ["Varos", "File", "Edit", "Object", "View", "Window"]);
        let items = flat_items(&m);
        let has = |c: MenuCmd| items.iter().any(|e| matches!(e, Entry::Item { cmd, .. } if *cmd == c));
        for p in PanelId::DOCKABLE {
            assert!(has(MenuCmd::TogglePanel(p)), "Window menu misses {}", p.title());
        }
        assert!(has(MenuCmd::ToggleRail) && has(MenuCmd::ToggleDock));
        // ⌘Q quits the app; ⌘W closes only the active tab — two DIFFERENT FileCmds (review F5: no
        // longer both folded into one "Close Window" path).
        assert!(has(MenuCmd::File(FileCmd::Quit)), "Varos ▸ Quit is File(FileCmd::Quit)");
        assert!(has(MenuCmd::File(FileCmd::CloseTab)), "File ▸ Close Tab is File(FileCmd::CloseTab)");
        // mirrors only: no Export row until it has a path (S6). The clipboard keys got their path in
        // Astra F04 (`every_clipboard_row_is_its_shortcut`), ⌘A / ⇧⌘A in QW5
        // (`edit_menu_mirrors_select_all_deselect_delete`), and KeyN is File ▸ New's real key (DFS S1).
        assert!(
            !items.iter().any(|e| matches!(e, Entry::Item { id, .. } if id.contains("export"))),
            "Export has no path yet — it must not be in the menu"
        );
    }

    #[test]
    fn file_menu_rows_are_new_open_close_save_saveas_on_their_keys() {
        let m = menus();
        let (_, file) = m.iter().find(|(t, _)| *t == "File").expect("a File menu");
        let rows: Vec<(&str, Accel, FileCmd)> = file
            .iter()
            .filter_map(|e| match e {
                Entry::Item { id, accel: Some(a), cmd: MenuCmd::File(fc), .. } => Some((id.as_str(), *a, *fc)),
                _ => None,
            })
            .collect();
        let want = [
            ("file.new", cmd(KeyCode::KeyN).unwrap(), FileCmd::New),
            ("file.open", cmd(KeyCode::KeyO).unwrap(), FileCmd::Open),
            ("file.close", cmd(KeyCode::KeyW).unwrap(), FileCmd::CloseTab),
            ("file.save", cmd(KeyCode::KeyS).unwrap(), FileCmd::Save),
            ("file.saveas", cmd_shift(KeyCode::KeyS).unwrap(), FileCmd::SaveAs),
        ];
        for (id, accel, fc) in want {
            assert!(rows.iter().any(|&(i, a, f)| i == id && a == accel && f == fc), "File menu misses {id}");
        }
        assert_eq!(rows.len(), want.len(), "no extra File rows go through MenuCmd::Key any more (review F5)");
        assert!(
            file.iter().all(|e| !matches!(e, Entry::Item { cmd: MenuCmd::Key(_), .. })),
            "File rows never dispatch through the synthetic-key path (spec §4)"
        );
    }
}
