//! Start v2 — Boards: THE Start page (lanes L4 + integration). Draws the pure [`StartModel`] (lane L2:
//! cards from Recent's cache, tag counts, the filter) to the design of record
//! (design-reference/mockups/start-v2/: `inter-recent`, `inter-empty`, `inter-hover`, `inter-list`;
//! numbers from its `src.html`) and returns [`StartAction`]s. Filter actions (`SetTagFilter`,
//! `SetView`) go to `StartModel::apply`; the rest to `host::start_command`. Presentation
//! only: no files, no filtering, no commands. Hand-painted with the kit (`shell/kit/board.rs`); tokens
//! only (`shell/tokens.rs`: `SB_*` layout, the type tokens); nothing animates.
//!
//! Layout is computed once per frame by the pure [`layout`] (testable without a context), then painted.
//! The responsive rule ([`grid_columns`]): the content column is the box minus 40 px each side, at most
//! six 320 px columns wide (centred beyond that). Columns = ⌊(content + 12) / (272 + 12)⌋ clamped to
//! 3…6; a card is (content − gutters) / columns wide, capped at 320 (so 272…320 from 3 columns up; under
//! 852 px of content three columns shrink below 272). The hero puts the actions (556) and the preset
//! panel (content − 568, clamped 560…840) side by side, and stacks them (actions, then the panel) when
//! the content is narrower than 1128 px (a window under ~1232 px).
//!
//! Keyboard (K2 row 2, the page owns it): Tab / ⇧Tab ring New board → Open… → presets → Recovered
//! actions → tag filters → view toggle → cards (the first Tab lands on New board); ←/→ move inside a
//! row (hero buttons, presets, a Recovered band, filters, toggle, grid), ↑/↓ move by a grid row (2D) or a
//! list row; Enter / Space activate; Delete / Backspace on a card = Remove from Recent; Esc closes the
//! "…" menu; ⇧F10 on a focused card or list row opens its "…" menu (egui reports no Menu key; the chip
//! also shows while a card has the ring), then ↑/↓ Enter Esc inside it. Tag filters that do not fit sit
//! behind a "+N" tab (a kit menu with counts); the selected filter is always shown. The ring is the
//! kit's 2 px azure outside ring with a 1 px gap, shown for keyboard only.
//! ⌘N / ⌘O belong to the host (K2 row 1); [`StartPage::command_keys`] is for the example gallery.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};

use egui::text::{LayoutJob, TextFormat};
use egui::{Align, Color32, Event, FontId, Galley, Id, Key, Rect, Sense, TextureId, Ui};
use varos_core::board::{PresetId, PRESETS};

use crate::shell::kit::{self, board as kb, Availability, Icon, MenuEntry, MenuLook};
use crate::shell::tokens as t;
mod templates;
use crate::start::{BoardCard, RecoveryRow, StartAction, StartModel, StartView, ThumbKey};

/// How the page sets each kind of text: a type token (lane L1) in the mockup's CSS line box, with the
/// mockup's tracking (`--ls-h` −0.02 em on headings, names and hero titles).
#[derive(Clone, Copy, Debug)]
pub struct Role {
    pub font: fn() -> FontId,
    pub tracking_em: f32,
    /// The CSS line box this role is set in.
    pub line: f32,
}
pub mod roles {
    use super::Role;
    use crate::shell::tokens as t;
    const HEADING: f32 = -0.02;
    const PLAIN: f32 = 0.0;
    pub const H1: Role = Role { font: t::h1, tracking_em: HEADING, line: 40.0 };
    pub const H2: Role = Role { font: t::h2, tracking_em: HEADING, line: 28.0 };
    pub const BUTTON_PRIMARY: Role = Role { font: t::button_strong, tracking_em: HEADING, line: 20.0 };
    pub const BUTTON: Role = Role { font: t::button, tracking_em: HEADING, line: 20.0 };
    pub const NAME: Role = Role { font: t::name, tracking_em: HEADING, line: 20.0 };
    pub const LIST_NAME: Role = Role { font: t::name, tracking_em: HEADING, line: 19.0 };
    pub const LEDE_FIRST: Role = Role { font: t::lede, tracking_em: PLAIN, line: 24.0 };
    pub const BODY: Role = Role { font: t::body, tracking_em: PLAIN, line: 20.0 };
    pub const BODY_MEDIUM: Role = Role { font: t::body_medium, tracking_em: PLAIN, line: 16.0 };
    pub const SMALL: Role = Role { font: t::small, tracking_em: PLAIN, line: 16.0 };
    pub const SMALL_MEDIUM: Role = Role { font: t::small_medium, tracking_em: PLAIN, line: 16.0 };
    pub const DESC: Role = Role { font: t::small, tracking_em: PLAIN, line: 17.0 };
    pub const TAG: Role = Role { font: t::tag, tracking_em: PLAIN, line: 20.0 };
    pub const MICRO: Role = Role { font: t::micro, tracking_em: PLAIN, line: 16.0 };
    pub const MONO: Role = Role { font: t::mono, tracking_em: PLAIN, line: 16.0 };
    /// The Review panel's title (Inter 600 13, `t::panel_title_font`).
    pub const PANEL_TITLE: Role = Role { font: t::panel_title_font, tracking_em: PLAIN, line: 16.0 };
}

/// Copy (owner-approved mockup text).
pub const LEDE: &str = "A board is a free canvas with a name, a short description and tags. Artboards inside are optional — add one when a piece needs a fixed size.";
pub const FIRST_TITLE: &str = "Start with a board";
/// The recovery copy's open action, one word everywhere (owner 2026-10-06): Start's Recovered band and
/// the editor's Review panel.
pub const RESTORE: &str = "Restore";
pub const NO_RECENT_COPY: &str = "No recent boards yet.";
const LOCATE: &str = "Locate…";
const REMOVE: &str = "Remove from Recent";

/// Stable ids of the page's controls (tests and the host read their responses).
pub mod ids {
    use egui::Id;
    use varos_core::board::PresetId;
    pub fn new_board() -> Id {
        Id::new("start-v2-new")
    }
    pub fn open() -> Id {
        Id::new("start-v2-open")
    }
    pub fn preset(p: PresetId) -> Id {
        Id::new(("start-v2-preset", p))
    }
    pub fn recover(rid: &str) -> Id {
        Id::new(("start-v2-recover", rid))
    }
    pub fn discard(rid: &str) -> Id {
        Id::new(("start-v2-discard", rid))
    }
    pub fn filter(tag: Option<&str>) -> Id {
        Id::new(("start-v2-filter", tag))
    }
    pub fn view() -> Id {
        Id::new("start-v2-view")
    }
    pub fn view_segment(v: crate::start::StartView) -> Id {
        view().with(v as usize)
    }
    pub fn card(key: &str) -> Id {
        Id::new(("start-v2-card", key))
    }
    pub fn row(key: &str) -> Id {
        Id::new(("start-v2-row", key))
    }
    pub fn chip(key: &str) -> Id {
        Id::new(("start-v2-chip", key))
    }
    /// The "+N" tab holding the tag filters that do not fit.
    pub fn more_filters() -> Id {
        Id::new("start-v2-more-filters")
    }
}

// ───────────────────────────── pure layout ─────────────────────────────

/// What the layout needs to know about the model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    pub first_launch: bool,
    pub recovered: usize,
    pub view: StartView,
    pub cards: usize,
}
impl Shape {
    pub fn of(model: &StartModel) -> Self {
        Shape {
            first_launch: is_first_launch(model),
            recovered: model.recovery().len(),
            view: model.filter().view,
            cards: model.visible_count(),
        }
    }
}

/// No board in Recent and nothing recovered: the centred "Start with a board" page.
pub fn is_first_launch(model: &StartModel) -> bool {
    model.cards().is_empty() && model.recovery().is_empty()
}

/// Every block's rect for one frame, in screen coordinates (scroll offset 0).
#[derive(Clone, Debug, PartialEq)]
pub struct PageLayout {
    /// The area the page owns (everything under the top bar).
    pub area: Rect,
    /// The BG box (8 px corners, LINE border).
    pub board: Rect,
    /// The status line in the void under the box.
    pub status: Rect,
    /// The content column (left/right edges; spans the content's full height).
    pub content: Rect,
    pub first_launch: bool,
    /// The hero is stacked (actions above the preset panel).
    pub stacked: bool,
    /// First launch only: the "Start with a board" line box.
    pub title: Rect,
    pub new_board: Rect,
    pub open: Rect,
    pub lede: Rect,
    pub keys: Rect,
    pub presets: Rect,
    pub preset_cells: Vec<Rect>,
    pub recovered: Vec<Rect>,
    pub head: Rect,
    pub cols: usize,
    pub card_w: f32,
    pub cards: Vec<Rect>,
    pub table_head: Rect,
    pub rows: Vec<Rect>,
    /// Where the content (with its bottom padding) ends.
    pub content_bottom: f32,
}
impl PageLayout {
    fn translated(&self, dy: f32) -> Self {
        let v = egui::vec2(0.0, dy);
        let mv = |r: &Rect| r.translate(v);
        Self {
            content: mv(&self.content),
            title: mv(&self.title),
            new_board: mv(&self.new_board),
            open: mv(&self.open),
            lede: mv(&self.lede),
            keys: mv(&self.keys),
            presets: mv(&self.presets),
            preset_cells: self.preset_cells.iter().map(mv).collect(),
            recovered: self.recovered.iter().map(mv).collect(),
            head: mv(&self.head),
            cards: self.cards.iter().map(mv).collect(),
            table_head: mv(&self.table_head),
            rows: self.rows.iter().map(mv).collect(),
            content_bottom: self.content_bottom + dy,
            ..self.clone()
        }
    }
}

/// The responsive rule: columns and card width for a content column `width` wide.
pub fn grid_columns(width: f32) -> (usize, f32) {
    let fit = ((width + t::SB_GUTTER) / (t::SB_COL + t::SB_GUTTER)).floor().max(0.0) as usize;
    let cols = fit.clamp(t::SB_COLS_MIN, t::SB_COLS_MAX);
    let card = ((width - t::SB_GUTTER * (cols - 1) as f32) / cols as f32).min(t::SB_COL_MAX);
    (cols, card)
}

/// The list's five columns have four gaps between them.
const LIST_GAPS: f32 = 4.0;

/// Width of the list's Tags and Folder columns for a content column `width` wide.
pub fn list_wide_column(width: f32) -> f32 {
    let fixed = t::SB_COL_NUM + t::SB_COL_DATE + t::SB_COL_GAP * LIST_GAPS;
    ((width - fixed) * t::SB_COL_WIDE_SHARE).clamp(t::SB_COL_WIDE_MIN, t::SB_COL_WIDE)
}

/// Lay the page out in `area` (the region under the top bar).
pub fn layout(area: Rect, shape: &Shape) -> PageLayout {
    // 4b: the band above already holds the top 12 — the box starts right at `area`'s top
    let board = Rect::from_min_max(
        area.min + egui::vec2(t::SEAM_GAP, 0.0),
        egui::pos2(area.right() - t::SEAM_GAP, area.bottom() - t::SB_STATUS_H),
    );
    let status = Rect::from_min_max(
        egui::pos2(area.left() + t::SB_STATUS_INSET, board.bottom()),
        egui::pos2(area.right() - t::SB_STATUS_INSET, area.bottom()),
    );
    let inner_w = (board.width() - t::SB_PAD_X * 2.0).max(0.0);
    let cw = inner_w.min(t::SB_CONTENT_MAX);
    let x = board.left() + t::SB_PAD_X + (inner_w - cw) / 2.0;
    let top = board.top() + t::SB_PAD_TOP;
    let (cols, card_w) = grid_columns(cw);
    let zero = Rect::from_min_size(egui::pos2(x, top), egui::Vec2::ZERO);
    let mut l = PageLayout {
        area,
        board,
        status,
        content: Rect::from_min_max(egui::pos2(x, top), egui::pos2(x + cw, board.bottom() - t::SB_PAD_BOTTOM)),
        first_launch: shape.first_launch,
        stacked: cw < t::SB_HERO_STACK_W,
        title: zero,
        new_board: zero,
        open: zero,
        lede: zero,
        keys: zero,
        presets: zero,
        preset_cells: vec![],
        recovered: vec![],
        head: zero,
        cols,
        card_w,
        cards: vec![],
        table_head: zero,
        rows: vec![],
        content_bottom: board.bottom(),
    };
    let big = egui::vec2(t::SB_BIG_W, t::SB_BIG_H);
    if l.first_launch {
        let lede_h = roles::LEDE_FIRST.line * 2.0;
        let block = roles::H1.line
            + t::SB_FIRST_LEDE_GAP
            + lede_h
            + t::SB_FIRST_BTNS_GAP
            + t::SB_BIG_H
            + t::SB_FIRST_KEYS_GAP
            + t::SB_KBD_H
            + t::SB_FIRST_PRESETS_GAP
            + t::SB_HERO_H;
        let avail = board.bottom() - t::SB_PAD_BOTTOM - t::SB_FIRST_PAD_BOTTOM - top;
        let mut y = top + ((avail - block) / 2.0).max(0.0);
        let cx = x + cw / 2.0;
        l.title = Rect::from_min_size(egui::pos2(x, y), egui::vec2(cw, roles::H1.line));
        y += roles::H1.line + t::SB_FIRST_LEDE_GAP;
        let lede_w = t::SB_FIRST_LEDE_W.min(cw);
        l.lede = Rect::from_min_size(egui::pos2(cx - lede_w / 2.0, y), egui::vec2(lede_w, lede_h));
        y += lede_h + t::SB_FIRST_BTNS_GAP;
        l.new_board = Rect::from_min_size(egui::pos2(cx - t::SB_ACTIONS_W / 2.0, y), big);
        l.open = Rect::from_min_size(egui::pos2(l.new_board.right() + t::SB_GUTTER, y), big);
        y += t::SB_BIG_H + t::SB_FIRST_KEYS_GAP;
        l.keys = Rect::from_min_size(egui::pos2(x, y), egui::vec2(cw, t::SB_KBD_H));
        y += t::SB_KBD_H + t::SB_FIRST_PRESETS_GAP;
        let pw = t::SB_PRESETS_W.min(cw);
        l.presets = Rect::from_min_size(egui::pos2(cx - pw / 2.0, y), egui::vec2(pw, t::SB_HERO_H));
        l.content_bottom = (l.presets.bottom() + t::SB_PAD_BOTTOM).max(board.bottom());
    } else {
        l.new_board = Rect::from_min_size(egui::pos2(x, top), big);
        l.open = Rect::from_min_size(egui::pos2(x + t::SB_BIG_W + t::SB_GUTTER, top), big);
        let lede_top = top + t::SB_BIG_H + t::SB_LEDE_GAP;
        l.lede = Rect::from_min_size(egui::pos2(x, lede_top), egui::vec2(t::SB_LEDE_W.min(cw), roles::BODY.line * 2.0));
        l.keys = Rect::from_min_size(
            egui::pos2(x, top + t::SB_HERO_H - t::SB_KBD_H),
            egui::vec2(t::SB_ACTIONS_W.min(cw), t::SB_KBD_H),
        );
        let mut y = top + t::SB_HERO_H;
        l.presets = if l.stacked {
            let r = Rect::from_min_size(
                egui::pos2(x, y + t::SB_SECTION_GAP),
                egui::vec2(t::SB_PRESETS_W.min(cw), t::SB_HERO_H),
            );
            y = r.bottom();
            r
        } else {
            let px = x + t::SB_ACTIONS_W + t::SB_GUTTER;
            let pw = (cw - t::SB_ACTIONS_W - t::SB_GUTTER).clamp(t::SB_PRESETS_MIN_W, t::SB_PRESETS_W);
            Rect::from_min_size(egui::pos2(px, top), egui::vec2(pw, t::SB_HERO_H))
        };
        for i in 0..shape.recovered {
            y += if i == 0 { t::SB_SECTION_GAP } else { t::SB_GUTTER };
            l.recovered.push(Rect::from_min_size(egui::pos2(x, y), egui::vec2(cw, t::SB_RECOV_H)));
            y += t::SB_RECOV_H;
        }
        y += t::SB_SECTION_GAP;
        l.head = Rect::from_min_size(egui::pos2(x, y), egui::vec2(cw, t::SB_HEAD_H));
        y += t::SB_HEAD_H + t::SB_HEAD_GAP;
        let mut bottom = y + roles::BODY.line;
        match shape.view {
            StartView::Grid => {
                for i in 0..shape.cards {
                    let (c, r) = (i % cols, i / cols);
                    let min = egui::pos2(
                        x + c as f32 * (card_w + t::SB_GUTTER),
                        y + r as f32 * (t::SB_CARD_H + t::SB_GUTTER),
                    );
                    let card = Rect::from_min_size(min, egui::vec2(card_w, t::SB_CARD_H));
                    bottom = card.bottom();
                    l.cards.push(card);
                }
            }
            StartView::List => {
                l.table_head = Rect::from_min_size(egui::pos2(x, y), egui::vec2(cw, t::SB_TH_H));
                bottom = l.table_head.bottom();
                for _ in 0..shape.cards {
                    let row = Rect::from_min_size(egui::pos2(x, bottom), egui::vec2(cw, t::SB_ROW_H));
                    bottom = row.bottom();
                    l.rows.push(row);
                }
            }
        }
        l.content_bottom = (bottom + t::SB_PAD_BOTTOM).max(board.bottom());
    }
    let inner = l.presets.shrink(t::KIT_STROKE);
    let cell_w = inner.width() / PRESETS.len() as f32;
    let row_top = inner.top() + t::SB_PANEL_HEAD_H;
    l.preset_cells = (0..PRESETS.len())
        .map(|i| {
            Rect::from_min_max(
                egui::pos2(inner.left() + cell_w * i as f32, row_top),
                egui::pos2(inner.left() + cell_w * (i + 1) as f32, inner.bottom()),
            )
        })
        .collect();
    l
}

/// A card's well (the thumbnail area): inside the card border, 123 tall including its hairline.
pub fn card_well(card: Rect) -> Rect {
    Rect::from_min_max(
        card.min + egui::vec2(t::KIT_STROKE, t::KIT_STROKE),
        egui::pos2(card.right() - t::KIT_STROKE, card.top() + t::KIT_STROKE + t::SB_WELL_H),
    )
}

/// The card's "…" chip.
pub fn card_chip(card: Rect) -> Rect {
    Rect::from_min_size(
        egui::pos2(card.right() - t::SB_CHIP_INSET - t::SB_CHIP, card.top() + t::SB_CHIP_INSET),
        egui::Vec2::splat(t::SB_CHIP),
    )
}

/// The folder a board lives in, `~`-relative when under `home`, with `/` separators.
pub fn folder_text(path: &Path, home: Option<&Path>) -> String {
    let dir = path.parent().unwrap_or(path);
    if let Some(rest) = home.and_then(|h| dir.strip_prefix(h).ok()) {
        let rest: Vec<String> = rest.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        return if rest.is_empty() { "~".into() } else { format!("~/{}", rest.join("/")) };
    }
    dir.to_string_lossy().replace('\\', "/")
}

/// Middle elision for a folder: drop whole middle segments first (keeping the first two when it can,
/// then as many trailing segments as fit), then fall back to a character-level middle cut.
pub fn elide_middle(text: &str, fits: impl Fn(&str) -> bool) -> String {
    if fits(text) {
        return text.to_string();
    }
    let segs: Vec<&str> = text.split('/').collect();
    let n = segs.len();
    if n >= 3 {
        for kept in (2..n).rev() {
            let mut heads: Vec<usize> = (1..kept).collect();
            heads.sort_by_key(|h| (h.abs_diff(2), *h));
            for h in heads {
                let candidate = format!("{}/…/{}", segs[..h].join("/"), segs[n - (kept - h)..].join("/"));
                if fits(&candidate) {
                    return candidate;
                }
            }
        }
    }
    let chars: Vec<char> = text.chars().collect();
    for keep in (1..chars.len()).rev() {
        let head = keep.div_ceil(2);
        let tail = keep - head;
        let candidate: String =
            chars[..head].iter().chain(['…'].iter()).chain(chars[chars.len() - tail..].iter()).collect();
        if fits(&candidate) {
            return candidate;
        }
    }
    "…".to_string()
}

/// Up to two initials for the typographic placeholder ("Ramadan campaign" → "RC").
pub fn initials(name: &str) -> String {
    name.split_whitespace().filter_map(|w| w.chars().next()).take(2).flat_map(char::to_uppercase).collect()
}

/// "2 artboards" / "1 artboard" / "free".
pub fn facts(artboards: u32) -> String {
    match artboards {
        0 => "free".into(),
        1 => "1 artboard".into(),
        n => format!("{n} artboards"),
    }
}

/// The mono size line under a preset ("1080 × 1350 px").
pub fn preset_size_text(id: PresetId) -> String {
    let p = varos_core::board::preset(id);
    format!("{} × {} {}", p.w.round() as u32, p.h.round() as u32, p.unit.suffix())
}

/// "Varos 0.1 α" from the crate version.
pub fn version_text() -> String {
    let v = env!("CARGO_PKG_VERSION");
    let short: Vec<&str> = v.split('.').take(2).collect();
    format!("Varos {} α", short.join("."))
}

// ───────────────────────────── text ─────────────────────────────

pub(crate) fn format(role: Role) -> TextFormat {
    let font_id = (role.font)();
    TextFormat {
        extra_letter_spacing: font_id.size * role.tracking_em,
        font_id,
        color: Color32::PLACEHOLDER,
        ..Default::default()
    }
}
pub(crate) fn text(ui: &Ui, s: &str, role: Role) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    job.append(s, 0.0, format(role));
    ui.fonts_mut(|f| f.layout_job(job))
}
pub(crate) fn text_elided(ui: &Ui, s: &str, role: Role, width: f32) -> Arc<Galley> {
    text_wrapped(ui, s, role, width, 1, Align::Min)
}
fn text_wrapped(ui: &Ui, s: &str, role: Role, width: f32, rows: usize, halign: Align) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    let mut f = format(role);
    if rows > 1 {
        f.line_height = Some(role.line);
        f.valign = Align::Center;
    }
    job.append(s, 0.0, f);
    job.wrap.max_width = width.max(0.0);
    job.wrap.max_rows = rows;
    job.halign = halign;
    ui.fonts_mut(|f| f.layout_job(job))
}
fn width_of(ui: &Ui, s: &str, role: Role) -> f32 {
    text(ui, s, role).size().x
}
pub(crate) fn path_galley(ui: &Ui, folder: &str, width: f32) -> Arc<Galley> {
    let fitted = elide_middle(folder, |c| width_of(ui, c, roles::MONO) <= width);
    text(ui, &fitted, roles::MONO)
}

// ───────────────────────────── focus ─────────────────────────────

/// A keyboard stop. Cards are keyed, so focus survives a rebuild of the model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Slot {
    New,
    Open,
    Preset(PresetId),
    Discard(String),
    Recover(String),
    Filter(Option<String>),
    /// The "+N" tab (the filters that do not fit).
    MoreFilters,
    View(StartView),
    Card(String),
    Template(PathBuf),
}
impl Slot {
    /// The row a slot belongs to (←/→ move inside one row).
    fn row(&self) -> (u8, &str) {
        match self {
            Slot::New | Slot::Open => (0, ""),
            Slot::Preset(_) => (1, ""),
            Slot::Discard(id) | Slot::Recover(id) => (2, id),
            Slot::Filter(_) | Slot::MoreFilters => (3, ""),
            Slot::View(_) => (4, ""),
            Slot::Card(_) => (5, ""),
            Slot::Template(_) => (6, ""),
        }
    }
}

/// The page's Tab order (disabled actions are skipped). `plan` = the filter tabs drawn (the "+N" tab
/// after them when some do not fit); `None` = every filter, as if all fit.
pub fn tab_order(model: &StartModel, plan: Option<&FilterPlan>) -> Vec<Slot> {
    let mut slots = vec![Slot::New, Slot::Open];
    slots.extend(PRESETS.iter().map(|p| Slot::Preset(p.id)));
    if is_first_launch(model) {
        slots.extend(model.templates.iter().cloned().map(Slot::Template));
        return slots;
    }
    for r in model.recovery() {
        if !r.busy {
            slots.push(Slot::Discard(r.rid.clone()));
            if r.problem.is_none() {
                slots.push(Slot::Recover(r.rid.clone()));
            }
        }
    }
    match plan {
        Some(plan) => {
            slots.extend(plan.shown.iter().map(|(tag, _)| Slot::Filter(tag.clone())));
            if !plan.hidden.is_empty() {
                slots.push(Slot::MoreFilters);
            }
        }
        None => {
            slots.push(Slot::Filter(None));
            slots.extend(model.tags().iter().map(|tc| Slot::Filter(Some(tc.tag.clone()))));
        }
    }
    slots.extend([Slot::View(StartView::Grid), Slot::View(StartView::List)]);
    slots.extend(model.visible_cards().map(|c| Slot::Card(c.key.clone())));
    slots.extend(model.templates.iter().cloned().map(Slot::Template));
    slots
}

/// What Enter / Space on `slot` asks for.
pub fn activate(slot: &Slot, model: &StartModel) -> Option<StartAction> {
    Some(match slot {
        Slot::New => StartAction::NewBoard,
        Slot::Open => StartAction::Open,
        Slot::Template(path) => StartAction::OpenTemplate(path.clone()),
        Slot::Preset(p) => StartAction::NewWithPreset(*p),
        Slot::Discard(id) => StartAction::DiscardRecovery(id.clone()),
        Slot::Recover(id) => {
            let row = model.recovery().iter().find(|r| &r.rid == id)?;
            if row.busy || row.problem.is_some() {
                return None;
            }
            StartAction::Recover(id.clone())
        }
        Slot::Filter(tag) => StartAction::SetTagFilter(tag.clone()),
        Slot::MoreFilters => return None, // opens its menu (the page does that)
        Slot::View(v) => StartAction::SetView(*v),
        Slot::Card(k) => StartAction::OpenRecent(card_path(model, k)?),
    })
}

fn card_path(model: &StartModel, key: &str) -> Option<std::path::PathBuf> {
    model.cards().iter().find(|c| c.key == key).map(|c| c.path.clone())
}

/// Where Home's thumbnails come from (lane L3's cache; the host implements it). Called on the page's
/// decode worker, never on the UI thread.
pub trait ThumbSource: Send + Sync {
    /// The cached thumbnail file for `key` and whether it is current for a board modified at
    /// `modified` (unix seconds). `None` = no thumbnail: the typographic placeholder.
    fn find(&self, key: &ThumbKey, modified: u64) -> Option<(PathBuf, bool)>;
}

/// Thumbnail textures kept beyond the ones drawn this frame (a small LRU).
const THUMB_SPARE: usize = 24;

type ThumbId = (String, u64);
struct Decoded {
    id: ThumbId,
    image: Option<egui::ColorImage>,
}

/// The page's thumbnail textures: looked up and decoded on one worker thread (never a frame on disk),
/// uploaded on the UI thread when ready (the worker asks for a repaint — an event, not animation),
/// bounded to the cards drawn plus [`THUMB_SPARE`].
#[derive(Default)]
struct ThumbCache {
    source: Option<Arc<dyn ThumbSource>>,
    jobs: Option<mpsc::Sender<(ThumbKey, u64)>>,
    done: Option<mpsc::Receiver<Decoded>>,
    textures: HashMap<ThumbId, (egui::TextureHandle, u64)>,
    asked: HashSet<ThumbId>,
    frame: u64,
    drawn: usize,
}
impl ThumbCache {
    fn begin(&mut self, ctx: &egui::Context, model: &StartModel) {
        self.frame += 1;
        self.drawn = 0;
        let Some(done) = &self.done else {
            return;
        };
        for d in done.try_iter() {
            // a decode that lands after its board left Recent is dropped, never uploaded
            if !board_has_key(model, &d.id.0) {
                self.asked.remove(&d.id);
                continue;
            }
            if let Some(image) = d.image {
                let tex = ctx.load_texture(format!("start-thumb-{}", d.id.0), image, egui::TextureOptions::LINEAR);
                self.textures.insert(d.id, (tex, self.frame));
            }
        }
    }
    fn get(&mut self, ctx: &egui::Context, key: &ThumbKey, modified: u64) -> Option<TextureId> {
        let id = (key.0.clone(), modified);
        if let Some((tex, used)) = self.textures.get_mut(&id) {
            *used = self.frame;
            self.drawn += 1;
            return Some(tex.id());
        }
        if self.source.is_some() && self.asked.insert(id) {
            if self.jobs.is_none() {
                self.spawn(ctx);
            }
            if let Some(jobs) = &self.jobs {
                let _ = jobs.send((key.clone(), modified));
            }
        }
        None
    }
    fn spawn(&mut self, ctx: &egui::Context) {
        let Some(source) = self.source.clone() else {
            return;
        };
        let (jobs, rx) = mpsc::channel::<(ThumbKey, u64)>();
        let (tx, done) = mpsc::channel();
        let ctx = ctx.clone();
        let worker = std::thread::Builder::new().name("varos-start-thumbs".into()).spawn(move || {
            for (key, modified) in rx {
                let image = source.find(&key, modified).and_then(|(path, _fresh)| decode_thumbnail(&path));
                if tx.send(Decoded { id: (key.0, modified), image }).is_err() {
                    return;
                }
                ctx.request_repaint();
            }
        });
        if worker.is_ok() {
            (self.jobs, self.done) = (Some(jobs), Some(done));
        }
    }
    /// Keep the textures drawn this frame plus a few spares; free the rest.
    fn end(&mut self) {
        let keep = self.drawn + THUMB_SPARE;
        while self.textures.len() > keep {
            let oldest = self
                .textures
                .iter()
                .filter(|(_, (_, used))| *used < self.frame)
                .min_by_key(|(_, (_, used))| *used)
                .map(|(id, _)| id.clone());
            match oldest {
                Some(id) => {
                    self.textures.remove(&id);
                }
                None => break,
            }
        }
    }
    /// The cache rewrote `key`'s pixels (or deleted them): drop what the page holds so it asks again.
    fn forget(&mut self, key: &str) {
        self.textures.retain(|(k, _), _| k != key);
        self.asked.retain(|(k, _)| k != key);
    }
    /// Free the textures of boards no longer in Recent.
    fn retain_boards(&mut self, model: &StartModel) {
        self.textures.retain(|(k, _), _| board_has_key(model, k));
        self.asked.retain(|(k, _)| board_has_key(model, k));
    }
}

/// Is `key` a thumbnail key of a board in Recent (its cached key, or its own path — L3's key rule)?
fn board_has_key(model: &StartModel, key: &str) -> bool {
    model.cards().iter().any(|c| c.key == key || c.thumb.as_ref().is_some_and(|t| t.0 == key))
}

/// The largest thumbnail file the page reads (L3 writes ~544 × 246 PNGs, a few hundred KB at most).
pub const THUMB_MAX_BYTES: u64 = 2 * 1024 * 1024;
/// The largest thumbnail the page decodes: twice L3's 544 × 246.
pub const THUMB_MAX_W: u32 = 1088;
pub const THUMB_MAX_H: u32 = 492;
/// The decoder's allocation ceiling (the largest allowed image's RGBA plus headroom).
const THUMB_MAX_ALLOC: u64 = 4 * 1024 * 1024;

/// Read and decode one cached thumbnail, bounded: a file over [`THUMB_MAX_BYTES`] is not read, a header
/// larger than [`THUMB_MAX_W`] × [`THUMB_MAX_H`] is refused before any pixel buffer exists, and the
/// decoder runs under an allocation limit — so a corrupt or planted PNG can never take more than a few
/// MB. Any refusal = `None` (the placeholder), logged once; never a panic.
#[doc(hidden)]
pub fn decode_thumbnail(path: &Path) -> Option<egui::ColorImage> {
    let refuse = |why: &str| {
        static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            eprintln!("start thumbnail refused ({why}): {}", path.display());
        }
        None
    };
    match std::fs::metadata(path) {
        Ok(m) if m.len() <= THUMB_MAX_BYTES => {}
        Ok(_) => return refuse("file too large"),
        Err(_) => return None,
    }
    let Ok(bytes) = std::fs::read(path) else {
        return None;
    };
    let reader = || image::ImageReader::with_format(std::io::Cursor::new(&bytes), image::ImageFormat::Png);
    match reader().into_dimensions() {
        Ok((w, h)) if w <= THUMB_MAX_W && h <= THUMB_MAX_H => {}
        Ok(_) => return refuse("dimensions too large"),
        Err(_) => return refuse("not a PNG"),
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(THUMB_MAX_W);
    limits.max_image_height = Some(THUMB_MAX_H);
    limits.max_alloc = Some(THUMB_MAX_ALLOC);
    let mut decoder = reader();
    decoder.limits(limits);
    let Ok(decoded) = decoder.decode() else {
        return refuse("corrupt PNG");
    };
    let rgba = decoded.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}

/// The tag filter row as drawn: the tabs that fit (with their widths) and the tags behind "+N".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FilterPlan {
    /// `(tag, width)`; `None` = All. The selected tag is always among them.
    pub shown: Vec<(Option<String>, f32)>,
    /// The tags behind the "+N" tab, in the model's order.
    pub hidden: Vec<String>,
}

/// Text derived from the model and the content width, rebuilt only when either changed (keyed by
/// `StartModel::generation`): identical frames format, measure and elide nothing again.
#[derive(Default)]
struct Derived {
    key: Option<(u64, u32)>,
    /// Card key → its folder, `~`-relative and middle-elided to the card (grid) or column (list).
    folders: HashMap<String, String>,
    /// Card key → "2 artboards" / "free".
    facts: HashMap<String, String>,
    numbers: Vec<String>,
    visible: String,
    /// "All" + each tag: (label, count).
    filters: Vec<(String, String)>,
    plan: FilterPlan,
    more_label: String,
    /// The "+N" menu's rows ("studio  1").
    more_rows: Vec<String>,
    /// Recovered rows: (name line, folder).
    recovered: Vec<(String, Option<String>)>,
    shortcuts: [String; 2],
    preset_labels: Vec<String>,
    preset_sizes: Vec<String>,
    version: String,
}

/// The Start page state: keyboard focus and its visibility, the open "…" menu.
pub struct StartPage {
    focus: Slot,
    ring: bool,
    entered: bool,
    menu_for: Option<String>,
    /// ⇧F10 asked for this card's "…" menu: opened where its chip is drawn this frame.
    menu_request: Option<String>,
    /// Enter / Space on the "+N" tab: its menu opens where the tab is drawn this frame.
    more_request: bool,
    last_card: usize,
    thumbs: ThumbCache,
    derived: Derived,
    derived_builds: u64,
    seen_generation: u64,
    home: Option<PathBuf>,
    /// Handle ⌘N / ⌘O here. Off in the app — the host's command keys own them (K2 row 1); the example
    /// gallery turns it on.
    pub command_keys: bool,
    /// Current recovery setting / availability, supplied by the same host as the Review footer.
    pub recovery_status: String,
}
impl Default for StartPage {
    fn default() -> Self {
        Self::new()
    }
}

struct Frame<'a> {
    model: &'a StartModel,
    actions: Vec<StartAction>,
    clicked: Option<Slot>,
    focused_rect: Option<Rect>,
    warning: Option<&'a str>,
}

impl StartPage {
    pub fn new() -> Self {
        Self {
            focus: Slot::New,
            ring: false,
            entered: false,
            menu_for: None,
            menu_request: None,
            more_request: false,
            last_card: 0,
            thumbs: ThumbCache::default(),
            derived: Derived::default(),
            derived_builds: 0,
            seen_generation: 0,
            home: home_dir(),
            command_keys: false,
            recovery_status: String::new(),
        }
    }
    /// Home was (re)entered: focus rests on New board with no ring until the keyboard is used.
    pub fn reset_focus(&mut self) {
        self.focus = Slot::New;
        self.ring = false;
        self.entered = false;
        self.menu_for = None;
    }
    pub fn focus(&self) -> &Slot {
        &self.focus
    }
    /// The focus ring is showing (keyboard modality).
    pub fn ring_visible(&self) -> bool {
        self.ring
    }
    /// The card whose "…" menu is open.
    pub fn menu_for(&self) -> Option<&str> {
        self.menu_for.as_deref()
    }

    /// THE thumbnail seam: the texture for a card's [`ThumbKey`] at the board's cached `modified` time,
    /// or `None` for the typographic placeholder. L3: the source's lookup (key, mtime) → the PNG is
    /// decoded on the page's worker → uploaded once and cached per key + mtime; a Stale file shows its
    /// old pixels (the host re-renders it at the board's next open or save — Home never parses files);
    /// the host calls [`Self::thumb_updated`] on every `ThumbDone`. Never blocks: the first frame draws
    /// the placeholder, the texture appears when its decode lands.
    pub fn thumb_texture(&mut self, ctx: &egui::Context, key: &ThumbKey, modified: u64) -> Option<TextureId> {
        self.thumbs.get(ctx, key, modified)
    }
    /// Where thumbnails come from (startup). Without one every card shows its placeholder.
    pub fn set_thumb_source(&mut self, source: Arc<dyn ThumbSource>) {
        self.thumbs.source = Some(source);
    }
    /// A render landed for `key` (or its file was deleted): the next frame asks the cache again.
    pub fn thumb_updated(&mut self, key: &ThumbKey) {
        self.thumbs.forget(&key.0);
    }
    /// How many thumbnail textures the page holds (tests: the LRU bound).
    pub fn thumb_textures(&self) -> usize {
        self.thumbs.textures.len()
    }
    /// How many times the derived text was rebuilt (tests: identical frames rebuild nothing).
    pub fn derived_builds(&self) -> u64 {
        self.derived_builds
    }
    /// The filter row as last laid out.
    pub fn filter_plan(&self) -> &FilterPlan {
        &self.derived.plan
    }

    /// Draw into everything `ui` has left (the app: the root under the top bar). `warning` is the
    /// Recent list's load problem, shown on the status line.
    pub fn draw(&mut self, ui: &mut Ui, model: &StartModel, warning: Option<&str>) -> Vec<StartAction> {
        let area = ui.available_rect_before_wrap();
        self.draw_in(ui, area, model, warning)
    }

    /// Draw the page in `area`.
    pub fn draw_in(&mut self, ui: &mut Ui, area: Rect, model: &StartModel, warning: Option<&str>) -> Vec<StartAction> {
        let lay = layout(area, &Shape::of(model));
        self.thumbs.begin(ui.ctx(), model);
        if self.seen_generation != model.generation() {
            self.seen_generation = model.generation();
            self.thumbs.retain_boards(model);
            self.carry_focus(model);
        }
        self.derive(ui, &lay, model);
        let mut f = Frame { model, actions: vec![], clicked: None, focused_rect: None, warning };
        let moved = self.keyboard(ui, &lay, &mut f);
        ui.allocate_rect(area, Sense::hover());
        let p = ui.painter().clone();
        p.rect_filled(area, egui::CornerRadius::ZERO, t::SEAM);
        p.rect_filled(lay.board, t::r_box(), t::BG);
        p.rect_stroke(lay.board, t::r_box(), t::hairline(), egui::StrokeKind::Inside);
        status(ui, &lay, &self.derived.version, f.warning, &self.recovery_status);
        let inner = lay.board.shrink(t::KIT_STROKE);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(Align::Min)));
        egui::ScrollArea::vertical().id_salt("start-v2-scroll").auto_shrink([false, false]).show(&mut child, |ui| {
            // the bottom padding is border-inclusive: content that ends on the box's bottom edge fits exactly
            let h = inner.height() + (lay.content_bottom - lay.board.bottom()).max(0.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(inner.width(), h), Sense::hover());
            let l = lay.translated(rect.top() - inner.top());
            self.content(ui, &l, &mut f);
            templates::draw(self, ui, &mut f);
            if moved {
                if let Some(r) = f.focused_rect {
                    ui.scroll_to_rect(r, None);
                }
            }
        });
        self.card_menu(ui.ctx(), &mut f);
        self.more_menu(ui.ctx(), &mut f);
        self.thumbs.end();
        if let Some(slot) = f.clicked.take() {
            self.focus = slot;
            self.entered = true;
        }
        // where the focused card sits, so a rebuild that removes it can focus the card now in its place
        if let Slot::Card(k) = &self.focus {
            if let Some(i) = model.visible_cards().position(|c| &c.key == k) {
                self.last_card = i;
            }
        }
        f.actions
    }

    /// A rebuilt model: keep focus on the same element by key, or the card now in its place.
    fn carry_focus(&mut self, model: &StartModel) {
        let order = tab_order(model, Some(&self.derived.plan));
        if self.menu_for.as_ref().is_some_and(|k| !model.visible_cards().any(|c| &c.key == k)) {
            self.menu_for = None;
        }
        if order.contains(&self.focus) {
            if let Slot::Card(key) = &self.focus {
                self.last_card = model.visible_cards().position(|c| &c.key == key).unwrap_or(self.last_card);
            }
            return;
        }
        let cards: Vec<&BoardCard> = model.visible_cards().collect();
        self.focus = match &self.focus {
            Slot::Card(_) if !cards.is_empty() => Slot::Card(cards[self.last_card.min(cards.len() - 1)].key.clone()),
            Slot::Card(_) => Slot::View(model.filter().view),
            Slot::Filter(_) | Slot::MoreFilters => Slot::Filter(None),
            _ => Slot::New,
        };
    }

    /// Rebuild the derived text when the model or the content width changed.
    fn derive(&mut self, ui: &Ui, lay: &PageLayout, model: &StartModel) {
        let key = (model.generation(), lay.content.width().to_bits());
        if self.derived.key == Some(key) {
            return;
        }
        self.derived_builds += 1;
        let home = self.home.as_deref();
        let d = &mut self.derived;
        d.key = Some(key);
        let list = model.filter().view == StartView::List;
        let folder_w = if list { list_wide_column(lay.content.width()) } else { lay.card_w - t::SB_CARD_PAD_X * 2.0 };
        d.folders.clear();
        d.facts.clear();
        for card in model.visible_cards() {
            let folder = folder_text(&card.path, home);
            let fitted = elide_middle(&folder, |c| width_of(ui, c, roles::MONO) <= folder_w);
            d.folders.insert(card.key.clone(), fitted);
            d.facts.insert(card.key.clone(), facts(card.artboards));
        }
        d.numbers = (1..=model.visible_count()).map(|n| n.to_string()).collect();
        d.visible = model.visible_count().to_string();
        d.filters = std::iter::once(("All".to_string(), model.cards().len().to_string()))
            .chain(model.tags().iter().map(|tc| (tc.tag.clone(), tc.count.to_string())))
            .collect();
        d.recovered =
            model.recovery().iter().map(|r| (format!("Recovered — {}", r.name), r.original_dir.clone())).collect();
        d.shortcuts = [t::shortcut_label("N"), t::shortcut_label("O")];
        d.preset_labels = PRESETS.iter().map(|p| format!("New board with a {} artboard", p.label)).collect();
        d.preset_sizes = PRESETS.iter().map(|p| preset_size_text(p.id)).collect();
        d.version = version_text();
        // the filter row: what fits between the count and the view toggle
        let head = lay.head;
        let title_w = width_of(ui, "Recent boards", roles::H2);
        let start = title_w + t::SB_COUNT_GAP + width_of(ui, &d.visible, roles::MONO) + t::SB_FILTERS_GAP;
        let seg_w = (t::SB_SEG_BTN_W + t::SB_SEG_PAD) * 2.0 + t::SB_SEG_PAD + t::KIT_STROKE * 2.0;
        let avail = head.width() - seg_w - t::SB_FILTERS_GAP - start;
        let tab_w = |label: &str, count: &str| {
            t::SB_FILTER_PAD * 2.0
                + width_of(ui, label, roles::BODY)
                + t::SB_FILTER_INNER
                + width_of(ui, count, roles::MONO)
        };
        let mut widths: Vec<f32> = d.filters.iter().map(|(l, c)| tab_w(l, c)).collect();
        let selected = model.filter().tag.as_deref().map(varos_core::board::fold);
        let is_selected =
            |i: usize| i > 0 && selected.as_deref() == Some(varos_core::board::fold(&d.filters[i].0).as_str());
        let all_fit =
            widths.iter().sum::<f32>() + t::SB_FILTER_SPACING * widths.len().saturating_sub(1) as f32 <= avail;
        let mut show = vec![all_fit; widths.len()];
        if !all_fit {
            let more_w = t::SB_FILTER_PAD * 2.0 + width_of(ui, &format!("+{}", widths.len()), roles::BODY);
            let room = avail - more_w - t::SB_FILTER_SPACING;
            show[0] = true;
            let mut used = widths[0];
            for i in 1..widths.len() {
                if is_selected(i) {
                    // the selected tab is always shown; when it does not fit it shrinks to what is left
                    // (its label elides, the full name is its tooltip) — it never overlaps "+N" or the toggle
                    widths[i] = widths[i].min((room - used - t::SB_FILTER_SPACING).max(0.0));
                    show[i] = true;
                    used += t::SB_FILTER_SPACING + widths[i];
                }
            }
            for i in 1..widths.len() {
                if !show[i] && used + t::SB_FILTER_SPACING + widths[i] <= room {
                    show[i] = true;
                    used += t::SB_FILTER_SPACING + widths[i];
                }
            }
        }
        let tag = |i: usize| (i > 0).then(|| d.filters[i].0.clone());
        d.plan = FilterPlan {
            shown: (0..widths.len()).filter(|&i| show[i]).map(|i| (tag(i), widths[i])).collect(),
            hidden: (1..widths.len()).filter(|&i| !show[i]).map(|i| d.filters[i].0.clone()).collect(),
        };
        d.more_label = format!("+{}", d.plan.hidden.len());
        d.more_rows = (1..widths.len())
            .filter(|&i| !show[i])
            .map(|i| format!("{}  {}", d.filters[i].0, d.filters[i].1))
            .collect();
    }

    /// The page's keyboard (K2 row 2). Returns whether focus moved (to scroll it into view). The page
    /// yields while a kit menu is open.
    fn keyboard(&mut self, ui: &mut Ui, lay: &PageLayout, f: &mut Frame<'_>) -> bool {
        let ctx = ui.ctx().clone();
        let events = ui.input(|i| i.events.clone());
        if events.is_empty() {
            return false;
        }
        let blocked = kit::menu_open(&ctx);
        let order = tab_order(f.model, Some(&self.derived.plan));
        let mut moved = false;
        for event in events {
            match event {
                Event::PointerButton { pressed: true, .. } => self.ring = false,
                Event::Key { key, pressed: true, repeat, modifiers, .. } if !blocked => {
                    if modifiers.command && !modifiers.shift && !modifiers.alt {
                        if !self.command_keys || repeat {
                            continue;
                        }
                        match key {
                            Key::N => f.actions.push(StartAction::NewBoard),
                            Key::O => f.actions.push(StartAction::Open),
                            _ => continue,
                        }
                        ui.input_mut(|i| i.consume_key(modifiers, key));
                        continue;
                    }
                    if modifiers.command || modifiers.ctrl || modifiers.alt {
                        continue;
                    }
                    let handled = match key {
                        Key::Tab => {
                            self.focus = if !self.entered {
                                if modifiers.shift {
                                    order.last().cloned().unwrap_or(Slot::New)
                                } else {
                                    Slot::New
                                }
                            } else {
                                step(&order, &self.focus, if modifiers.shift { -1 } else { 1 })
                            };
                            self.entered = true;
                            true
                        }
                        Key::ArrowLeft | Key::ArrowRight => {
                            let d = if key == Key::ArrowLeft { -1 } else { 1 };
                            let next = step_clamped(&order, &self.focus, d);
                            let list_card = f.model.filter().view == StartView::List && matches!(next, Slot::Card(_));
                            if next.row() == self.focus.row() && !list_card {
                                self.focus = next;
                            }
                            self.entered = true;
                            true
                        }
                        Key::ArrowUp | Key::ArrowDown => {
                            let d: isize = if key == Key::ArrowUp { -1 } else { 1 };
                            self.vertical(f, lay, d);
                            true
                        }
                        Key::Enter | Key::Space if !repeat => {
                            if self.focus == Slot::MoreFilters {
                                self.more_request = true;
                            }
                            f.actions.extend(activate(&self.focus, f.model));
                            true
                        }
                        Key::F10 if modifiers.shift && !repeat => {
                            if let Slot::Card(k) = &self.focus {
                                self.menu_request = Some(k.clone());
                            }
                            true
                        }
                        Key::Delete | Key::Backspace if !repeat => {
                            if let Slot::Card(k) = &self.focus {
                                f.actions.extend(card_path(f.model, k).map(StartAction::RemoveRecent));
                            }
                            true
                        }
                        Key::Enter | Key::Space | Key::Delete | Key::Backspace => true,
                        _ => false,
                    };
                    if handled {
                        self.ring = true;
                        moved = true;
                        ui.input_mut(|i| i.consume_key(modifiers, key));
                    }
                }
                _ => {}
            }
        }
        moved
    }

    /// ↑/↓: a grid row (2D) or a list row; from outside the cards they enter the first card.
    fn vertical(&mut self, f: &Frame<'_>, lay: &PageLayout, d: isize) {
        let count = f.model.visible_count();
        if count == 0 {
            return;
        }
        let at = match &self.focus {
            Slot::Card(k) => f.model.visible_cards().position(|c| &c.key == k),
            _ => None,
        };
        let next = match at {
            None => 0,
            Some(i) => {
                let span = if f.model.filter().view == StartView::Grid { lay.cols as isize } else { 1 };
                let j = i as isize + d * span;
                if j < 0 || j >= count as isize {
                    i
                } else {
                    j as usize
                }
            }
        };
        let Some(card) = f.model.visible_cards().nth(next) else {
            return;
        };
        self.focus = Slot::Card(card.key.clone());
        self.last_card = next;
        self.entered = true;
    }

    fn ring_on(&self, slot: &Slot) -> bool {
        self.ring && self.entered && &self.focus == slot
    }

    fn mark(&self, f: &mut Frame<'_>, slot: &Slot, rect: Rect) {
        if &self.focus == slot {
            f.focused_rect = Some(rect);
        }
    }

    fn content(&mut self, ui: &mut Ui, l: &PageLayout, f: &mut Frame<'_>) {
        if l.first_launch {
            let p = ui.painter().clone();
            let title = text(ui, FIRST_TITLE, roles::H1);
            let tx = l.title.center().x - title.size().x / 2.0;
            kb::galley_in_line(&p, tx, l.title.top(), l.title.height(), title, t::TEXT);
            let lede = text_wrapped(ui, LEDE, roles::LEDE_FIRST, l.lede.width(), 2, Align::Center);
            p.galley(egui::pos2(l.lede.center().x, l.lede.top()), lede, t::MUTED);
            self.actions(ui, l, f);
            keys(ui, l.keys, &[(&["Return"], "New board")], true);
            self.presets(ui, l, f);
            return;
        }
        self.actions(ui, l, f);
        let p = ui.painter().clone();
        let lede = text_wrapped(ui, LEDE, roles::BODY, l.lede.width(), 2, Align::Min);
        p.galley(l.lede.min, lede, t::MUTED);
        keys(ui, l.keys, &[(&["↑", "↓"], "Move"), (&["Return"], "Open board"), (&["Delete"], REMOVE)], false);
        self.presets(ui, l, f);
        for (i, (row, rect)) in f.model.recovery().iter().zip(&l.recovered).enumerate() {
            self.recovered(ui, i, row, *rect, f);
        }
        self.head(ui, l, f);
        if f.model.visible_count() == 0 {
            let copy = f.model.no_match_copy().unwrap_or(NO_RECENT_COPY);
            let g = text(ui, copy, roles::BODY);
            kb::galley_in_line(&p, l.content.left(), l.head.bottom() + t::SB_HEAD_GAP, roles::BODY.line, g, t::MUTED);
            return;
        }
        match f.model.filter().view {
            StartView::Grid => {
                let model = f.model;
                for (card, rect) in model.visible_cards().zip(&l.cards) {
                    self.card(ui, card, *rect, f);
                }
            }
            StartView::List => self.table(ui, l, f),
        }
    }

    fn actions(&mut self, ui: &mut Ui, l: &PageLayout, f: &mut Frame<'_>) {
        for (slot, rect, icon, title, sub, key, primary, action) in [
            (
                Slot::New,
                l.new_board,
                Icon::ArtboardAdd,
                "New board",
                "Free canvas, no size needed",
                0,
                true,
                StartAction::NewBoard,
            ),
            (Slot::Open, l.open, Icon::Open, "Open…", "A .vrs file from disk", 1, false, StartAction::Open),
        ] {
            let b = kb::BigButton {
                icon,
                title: text(ui, title, if primary { roles::BUTTON_PRIMARY } else { roles::BUTTON }),
                sub: text(ui, sub, roles::SMALL),
                shortcut: text(ui, &self.derived.shortcuts[key], roles::MONO),
                primary,
                focused: self.ring_on(&slot),
            };
            let id = if primary { ids::new_board() } else { ids::open() };
            let r = kb::big_button(ui, id, rect, b, title);
            self.mark(f, &slot, rect);
            if r.activated {
                f.actions.push(action);
                f.clicked = Some(slot);
            }
        }
    }

    fn presets(&mut self, ui: &mut Ui, l: &PageLayout, f: &mut Frame<'_>) {
        let p = ui.painter().clone();
        let panel = l.presets;
        p.rect_filled(panel, t::r_box(), t::PANEL);
        let head_bottom = panel.top() + t::KIT_STROKE + t::SB_PANEL_HEAD_H;
        p.hline(panel.x_range(), head_bottom - t::KIT_STROKE / 2.0, t::hairline());
        let head = text(ui, "…or start with an artboard", roles::SMALL_MEDIUM);
        let hw = head.size().x;
        let line_top = panel.top() + t::KIT_STROKE;
        let line_h = t::SB_PANEL_HEAD_H - t::KIT_STROKE;
        let hx = panel.left() + t::KIT_STROKE + t::SB_PANEL_PAD_X;
        kb::galley_in_line(&p, hx, line_top, line_h, head, t::TEXT);
        let sub = text(ui, "— true proportions", roles::SMALL);
        kb::galley_in_line(&p, hx + hw + t::SB_PANEL_HEAD_GAP, line_top, line_h, sub, t::MUTED);
        for (i, (preset, cell)) in PRESETS.iter().zip(&l.preset_cells).enumerate() {
            if i > 0 {
                p.vline(cell.left() - t::KIT_STROKE / 2.0, cell.y_range(), t::hairline());
            }
            let preview = egui::vec2(preset.w, preset.h) * t::SB_PRESET_SCALE;
            let name_top = cell.top() + t::SB_PRESET_TOP + t::SB_PRESET_PV + t::SB_PRESET_NAME_GAP;
            let name = text(ui, preset.label, roles::BODY_MEDIUM);
            let name_y = name_top + (roles::BODY_MEDIUM.line - name.size().y) / 2.0;
            let size_top = name_top + roles::BODY_MEDIUM.line + t::SB_PRESET_SIZE_GAP;
            let size = text(ui, &self.derived.preset_sizes[i], roles::MONO);
            let size_line = roles::MONO.line - t::KIT_STROKE;
            let size_y = size_top + (size_line - size.size().y) / 2.0;
            let slot = Slot::Preset(preset.id);
            let focused = self.ring_on(&slot);
            let id = ids::preset(preset.id);
            let label = &self.derived.preset_labels[i];
            let r = kb::preset_cell(ui, id, *cell, preview, (name, name_y), (size, size_y), focused, label);
            self.mark(f, &slot, *cell);
            if r.activated {
                f.actions.push(StartAction::NewWithPreset(preset.id));
                f.clicked = Some(slot);
            }
        }
        // the border last, over the cell washes
        p.rect_stroke(panel, t::r_box(), t::hairline(), egui::StrokeKind::Inside);
    }

    fn recovered(&mut self, ui: &mut Ui, index: usize, row: &RecoveryRow, rect: Rect, f: &mut Frame<'_>) {
        let p = ui.painter().clone();
        // the one Recovered band painter (shared with the editor's recovery card)
        let x = kb::recovered_band(&p, rect);
        // buttons, right to left: Restore (solid), Discard (ghost) — "Restore" everywhere (owner 2026-10-06)
        let label_r = text(ui, RESTORE, roles::SMALL_MEDIUM);
        let label_d = text(ui, "Discard", roles::SMALL_MEDIUM);
        let w_r = kb::pill_width(&label_r, t::SB_BTN_PAD);
        let w_d = kb::pill_width(&label_d, t::SB_BTN_PAD);
        let top = rect.center().y - t::SB_BTN_H / 2.0;
        let recover =
            Rect::from_min_size(egui::pos2(rect.right() - t::SB_RECOV_PAD_R - w_r, top), egui::vec2(w_r, t::SB_BTN_H));
        let discard = Rect::from_min_size(
            egui::pos2(recover.left() - t::SB_RECOV_ICON_GAP - w_d, top),
            egui::vec2(w_d, t::SB_BTN_H),
        );
        let busy = row.busy.then_some(Availability::Busy("Working…"));
        let rec_av = busy.unwrap_or(row.problem.as_deref().map_or(Availability::Enabled, Availability::Disabled));
        let dis_av = busy.unwrap_or(Availability::Enabled);
        let (sd, sr) = (Slot::Discard(row.rid.clone()), Slot::Recover(row.rid.clone()));
        let ghost = kb::ButtonKind::Ghost;
        let r =
            kb::text_button(ui, ids::discard(&row.rid), discard, label_d, ghost, dis_av, self.ring_on(&sd), "Discard");
        self.mark(f, &sd, discard);
        if r.activated {
            f.actions.push(StartAction::DiscardRecovery(row.rid.clone()));
            f.clicked = Some(sd);
        }
        let solid = kb::ButtonKind::Solid;
        let r =
            kb::text_button(ui, ids::recover(&row.rid), recover, label_r, solid, rec_av, self.ring_on(&sr), RESTORE);
        self.mark(f, &sr, recover);
        if r.activated {
            f.actions.push(StartAction::Recover(row.rid.clone()));
            f.clicked = Some(sr);
        }
        // text: name · when (or the problem) · folder, on one baseline; the folder is elided to fit
        let right = discard.left() - t::SB_RECOV_ICON_GAP;
        let (title, folder) = self.derived.recovered.get(index).cloned().unwrap_or_default();
        let name = text_elided(ui, &title, roles::BODY_MEDIUM, (right - x).max(0.0));
        let nw = name.size().x;
        let line = roles::BODY_MEDIUM.line;
        let nr = kb::galley_in_line(&p, x, rect.center().y - line / 2.0, line, name.clone(), t::TEXT);
        let baseline = nr.top() + kb::first_baseline(&name);
        let mut cx = x + nw + t::SB_RECOV_TEXT_GAP;
        let when = row.problem.as_deref().unwrap_or(&row.saved_at_text);
        if cx < right {
            let g = text_elided(ui, when, roles::BODY, right - cx);
            let w = g.size().x;
            kb::galley_at_baseline(&p, cx, baseline, g, t::MUTED);
            cx += w + t::SB_RECOV_TEXT_GAP;
        }
        if let Some(folder) = folder.as_deref().filter(|_| cx < right) {
            let g = path_galley(ui, folder, right - cx);
            kb::galley_at_baseline(&p, cx, baseline, g, t::MUTED);
        }
    }

    fn head(&mut self, ui: &mut Ui, l: &PageLayout, f: &mut Frame<'_>) {
        let p = ui.painter().clone();
        let h = l.head;
        let title = text(ui, "Recent boards", roles::H2);
        let tw = title.size().x;
        kb::galley_in_line(&p, h.left(), h.top(), h.height(), title, t::TEXT);
        let count = text(ui, &self.derived.visible, roles::MONO);
        let cnt_w = count.size().x;
        let cnt_x = h.left() + tw + t::SB_COUNT_GAP;
        // `.cnt { padding-top: 2px }` inside a centred row: one pixel lower than centre
        kb::galley_in_line(&p, cnt_x, h.top() + t::KIT_STROKE, h.height(), count, t::MUTED);
        // the view toggle, right-aligned
        let seg_w = (t::SB_SEG_BTN_W + t::SB_SEG_PAD) * 2.0 + t::SB_SEG_PAD + t::KIT_STROKE * 2.0;
        let seg_h = t::SB_SEG_BTN_H + (t::SB_SEG_PAD + t::KIT_STROKE) * 2.0;
        let seg =
            Rect::from_min_size(egui::pos2(h.right() - seg_w, h.center().y - seg_h / 2.0), egui::vec2(seg_w, seg_h));
        let views = [StartView::Grid, StartView::List];
        let focused = views.iter().position(|v| self.ring_on(&Slot::View(*v)));
        let current = f.model.filter().view as usize;
        let segments = [(Icon::Grid, "Grid"), (Icon::List, "List")];
        let (chosen, segs) = kb::segmented(ui, ids::view(), seg, &segments, current, focused);
        for (i, v) in views.into_iter().enumerate() {
            self.mark(f, &Slot::View(v), segs[i]);
        }
        if let Some(i) = chosen {
            let v = views[i];
            f.actions.push(StartAction::SetView(v));
            f.clicked = Some(Slot::View(v));
        }
        // the tag filter (`derive` decided what fits): All N · tag n … · "+N"
        let mut x = cnt_x + cnt_w + t::SB_FILTERS_GAP;
        let selected_tag = f.model.filter().tag.as_deref().map(varos_core::board::fold);
        for k in 0..self.derived.plan.shown.len() {
            let (tag, w) = self.derived.plan.shown[k].clone();
            let i = tag.as_ref().map_or(0, |t| self.derived.filters.iter().position(|(l, _)| l == t).unwrap_or(0));
            let (label_s, count_s) = &self.derived.filters[i];
            let count = text(ui, count_s, roles::MONO);
            let label_room = (w - t::SB_FILTER_PAD * 2.0 - t::SB_FILTER_INNER - count.size().x).max(0.0);
            let truncated = width_of(ui, label_s, roles::BODY) > label_room;
            let label = text_elided(ui, label_s, roles::BODY, label_room);
            let rect = Rect::from_min_size(egui::pos2(x, h.top()), egui::vec2(w, h.height()));
            let slot = Slot::Filter(tag.clone());
            let selected = selected_tag.as_deref() == tag.as_deref().map(varos_core::board::fold).as_deref();
            let id = ids::filter(tag.as_deref());
            let r = kb::filter_tab(ui, id, rect, label, count, selected, self.ring_on(&slot), label_s);
            let r = if truncated {
                kit::ControlResponse { response: r.response.on_hover_text(label_s.as_str()), activated: r.activated }
            } else {
                r
            };
            self.mark(f, &slot, rect);
            if r.activated {
                f.actions.push(StartAction::SetTagFilter(tag));
                f.clicked = Some(slot);
            }
            x += w + t::SB_FILTER_SPACING;
        }
        if !self.derived.plan.hidden.is_empty() {
            let label = text(ui, &self.derived.more_label, roles::BODY);
            let w = t::SB_FILTER_PAD * 2.0 + label.size().x;
            let rect = Rect::from_min_size(egui::pos2(x, h.top()), egui::vec2(w, h.height()));
            let empty = text(ui, "", roles::MONO);
            let focused = self.ring_on(&Slot::MoreFilters);
            let r = kb::filter_tab(ui, ids::more_filters(), rect, label, empty, false, focused, "More tags");
            self.mark(f, &Slot::MoreFilters, rect);
            if r.activated || std::mem::take(&mut self.more_request) {
                let owner = ids::more_filters();
                if kit::is_menu_open(ui.ctx(), owner) {
                    kit::close_menu(ui.ctx());
                } else {
                    let pos = egui::pos2(rect.left(), rect.bottom() + t::KIT_MENU_GAP);
                    kit::open_menu(ui.ctx(), owner, pos, Some(rect));
                }
                f.clicked = Some(Slot::MoreFilters);
            }
        }
    }

    fn card(&mut self, ui: &mut Ui, card: &BoardCard, rect: Rect, f: &mut Frame<'_>) {
        let slot = Slot::Card(card.key.clone());
        let focused = self.ring_on(&slot);
        let owner = menu_owner(&card.key);
        let menu_open = self.menu_for.as_deref() == Some(card.key.as_str()) && kit::is_menu_open(ui.ctx(), owner);
        let well = card_well(rect);
        let state = kb::CardState { focused, menu_open };
        let (r, lit) = kb::card(ui, ids::card(&card.key), rect, well, state, &card.name);
        self.mark(f, &slot, rect);
        let p = ui.painter().clone();
        let wp = p.with_clip_rect(well.intersect(p.clip_rect()));
        if card.missing {
            let row_top = well.top() + t::SB_MISS_Y;
            let ix = well.left() + t::SB_MISS_X;
            let ic = egui::pos2(ix + t::ICON_MD / 2.0, row_top + t::ICON_MD / 2.0);
            Icon::FileQuestion.paint(&wp, ic, t::ICON_MD, t::MUTED);
            let g = text(ui, "File not found", roles::SMALL);
            kb::galley_in_line(&wp, ix + t::ICON_MD + t::SB_MISS_GAP, row_top, t::ICON_MD, g, t::MUTED);
        } else {
            well_dots(&wp, well);
            // Recent's key when it has one, else the board's own path (the cache's key rule): a Stale
            // image shows while its refresh renders
            let key = card.thumb.clone().unwrap_or_else(|| ThumbKey(card.key.clone()));
            match self.thumb_texture(ui.ctx(), &key, card.modified) {
                Some(tex) => thumbnail(ui, &wp, well, tex),
                None => placeholder(ui, &wp, well, card),
            }
        }
        // meta
        let x0 = rect.left() + t::SB_CARD_PAD_X;
        let x1 = rect.right() - t::SB_CARD_PAD_X;
        let top = well.bottom() + t::SB_CARD_PAD_TOP;
        let line = roles::NAME.line;
        let (right_w, date) = if card.missing {
            let g = text(ui, "Missing", roles::TAG);
            let w = kb::pill_width(&g, t::SB_MISS_PILL_PAD);
            let pill = Rect::from_min_size(
                egui::pos2(x1 - w, top + (line - t::SB_MISS_PILL_H) / 2.0),
                egui::vec2(w, t::SB_MISS_PILL_H),
            );
            kb::outline_pill(&p, pill, g);
            (w, None)
        } else {
            let g = text(ui, &card.modified_text, roles::MONO);
            (g.size().x, Some(g))
        };
        let name_w = (x1 - x0 - right_w - t::SB_DATE_GAP).max(0.0);
        let name = text_elided(ui, &card.name, roles::NAME, name_w);
        let nr = kb::galley_in_line(&p, x0, top, line, name.clone(), if card.missing { t::MUTED } else { t::TEXT });
        if let Some(date) = date {
            let w = date.size().x;
            kb::galley_at_baseline(&p, x1 - w, nr.top() + kb::first_baseline(&name), date, t::MUTED);
        }
        let mut y = top + line;
        if let Some(desc) = card.description.as_deref().filter(|d| !d.is_empty()) {
            let g = text_wrapped(ui, desc, roles::DESC, x1 - x0, 2, Align::Min);
            p.galley(egui::pos2(x0, y + t::SB_DESC_GAP), g, t::MUTED);
            y += t::SB_DESC_GAP + roles::DESC.line * 2.0;
        }
        let tags_top = y + t::SB_TAGS_GAP;
        // an entry Recent has no board summary for yet: its artboard count is unknown, not 0
        let fw = if card.cached {
            let g = text(ui, self.derived.facts.get(&card.key).map_or("", String::as_str), roles::MONO);
            let fw = g.size().x;
            kb::galley_in_line(&p, x1 - fw, tags_top, t::SB_PILL_H, g, t::MUTED);
            fw + t::SB_FACTS_GAP
        } else {
            0.0
        };
        pills(ui, &p, &card.tags, x0, tags_top, x1 - fw);
        let path_top = rect.bottom() - t::SB_CARD_PAD_BOTTOM - roles::MONO.line;
        let folder = self.derived.folders.get(&card.key).map_or("", String::as_str);
        let g = text(ui, folder, roles::MONO);
        kb::galley_in_line(&p, x0, path_top, roles::MONO.line, g, t::MUTED);
        kb::card_border(ui, rect, lit, focused);
        if r.activated {
            f.actions.push(StartAction::OpenRecent(card.path.clone()));
            f.clicked = Some(slot.clone());
        }
        if r.response.secondary_clicked() {
            let at = r.response.interact_pointer_pos().unwrap_or(rect.center());
            kit::open_menu(ui.ctx(), owner, at, None);
            self.menu_for = Some(card.key.clone());
            f.clicked = Some(slot.clone());
        }
        self.chip(ui, card, card_chip(rect), lit || focused, menu_open, &slot, f);
    }

    /// The "…" chip (on hover, keyboard focus or an open menu) and ⇧F10's request for its menu.
    #[allow(clippy::too_many_arguments)]
    fn chip(
        &mut self,
        ui: &mut Ui,
        card: &BoardCard,
        chip: Rect,
        show: bool,
        open: bool,
        slot: &Slot,
        f: &mut Frame<'_>,
    ) {
        let requested = self.menu_request.as_deref() == Some(card.key.as_str());
        if requested {
            self.menu_request = None;
            let owner = menu_owner(&card.key);
            let pos = egui::pos2(chip.right() - t::SB_MENU_W, chip.bottom() + t::KIT_MENU_GAP);
            kit::open_menu(ui.ctx(), owner, pos, Some(chip));
            self.menu_for = Some(card.key.clone());
        }
        if show || requested {
            let c = kb::more_chip(ui, ids::chip(&card.key), chip, open || requested);
            if c.activated {
                toggle_card_menu(ui.ctx(), &card.key, chip);
                self.menu_for = Some(card.key.clone());
                f.clicked = Some(slot.clone());
            }
        }
    }

    fn table(&mut self, ui: &mut Ui, l: &PageLayout, f: &mut Frame<'_>) {
        let p = ui.painter().clone();
        let x = l.content.left();
        let cw = l.content.width();
        let wide = list_wide_column(cw);
        let name_x = x + t::SB_COL_NUM + t::SB_COL_GAP;
        let fr = (cw - t::SB_COL_NUM - wide * 2.0 - t::SB_COL_DATE - t::SB_COL_GAP * LIST_GAPS).max(0.0);
        let tags_x = name_x + fr + t::SB_COL_GAP;
        let folder_x = tags_x + wide + t::SB_COL_GAP;
        let right = x + cw;
        let th = l.table_head;
        for (label, lx) in [("#", x + t::SB_NUM_PAD), ("Name", name_x), ("Tags", tags_x), ("Folder", folder_x)] {
            let g = text(ui, label, roles::MICRO);
            kb::galley_in_line(&p, lx, th.top(), th.height(), g, t::MUTED);
        }
        let g = text(ui, "Modified", roles::MICRO);
        let w = g.size().x;
        kb::galley_in_line(&p, right - w, th.top(), th.height(), g, t::MUTED);
        p.hline(th.x_range(), th.bottom() - t::KIT_STROKE / 2.0, t::hairline());
        let model = f.model;
        for (i, (card, rect)) in model.visible_cards().zip(&l.rows).enumerate() {
            let rect = *rect;
            let slot = Slot::Card(card.key.clone());
            let focused = self.ring_on(&slot);
            let r = kb::table_row(ui, ids::row(&card.key), rect, focused, &card.name);
            self.mark(f, &slot, rect);
            let owner = menu_owner(&card.key);
            let open = self.menu_for.as_deref() == Some(card.key.as_str()) && kit::is_menu_open(ui.ctx(), owner);
            let chip_shown = r.response.contains_pointer() || focused || open;
            // the "…" chip at the row's right end; the date / Missing pill steps left of it
            let chip = Rect::from_min_size(
                egui::pos2(x + cw - t::SB_CHIP, rect.center().y - t::SB_CHIP / 2.0),
                egui::Vec2::splat(t::SB_CHIP),
            );
            let right = if chip_shown { chip.left() - t::SB_DATE_GAP } else { right };
            let n = text(ui, self.derived.numbers.get(i).map_or("", String::as_str), roles::MONO);
            kb::galley_in_line(&p, x + t::SB_NUM_PAD, rect.top(), rect.height(), n, t::MUTED);
            let ink = if card.missing { t::MUTED } else { t::TEXT };
            let name = text_elided(ui, &card.name, roles::LIST_NAME, fr);
            match card.description.as_deref().filter(|d| !d.is_empty()) {
                Some(desc) => {
                    let block = roles::LIST_NAME.line + t::SB_LIST_DESC_GAP + roles::SMALL.line;
                    let top = rect.center().y - block / 2.0;
                    kb::galley_in_line(&p, name_x, top, roles::LIST_NAME.line, name, ink);
                    let d = text_elided(ui, desc, roles::SMALL, fr);
                    let dt = top + roles::LIST_NAME.line + t::SB_LIST_DESC_GAP;
                    kb::galley_in_line(&p, name_x, dt, roles::SMALL.line, d, t::MUTED);
                }
                None => {
                    kb::galley_in_line(&p, name_x, rect.top(), rect.height(), name, ink);
                }
            }
            pills(ui, &p, &card.tags, tags_x, rect.center().y - t::SB_PILL_H / 2.0, tags_x + wide);
            let folder = self.derived.folders.get(&card.key).map_or("", String::as_str);
            let g = text(ui, folder, roles::MONO);
            kb::galley_in_line(&p, folder_x, rect.top(), rect.height(), g, t::MUTED);
            if card.missing {
                let g = text(ui, "Missing", roles::TAG);
                let w = kb::pill_width(&g, t::SB_MISS_PILL_PAD);
                let pill = Rect::from_min_size(
                    egui::pos2(right - w, rect.center().y - t::SB_MISS_PILL_H / 2.0),
                    egui::vec2(w, t::SB_MISS_PILL_H),
                );
                kb::outline_pill(&p, pill, g);
            } else {
                let g = text(ui, &card.modified_text, roles::MONO);
                let w = g.size().x;
                kb::galley_in_line(&p, right - w, rect.top(), rect.height(), g, t::TEXT);
            }
            if r.activated {
                f.actions.push(StartAction::OpenRecent(card.path.clone()));
                f.clicked = Some(slot.clone());
            }
            if r.response.secondary_clicked() {
                let at = r.response.interact_pointer_pos().unwrap_or(rect.center());
                kit::open_menu(ui.ctx(), owner, at, None);
                self.menu_for = Some(card.key.clone());
                f.clicked = Some(slot.clone());
            }
            self.chip(ui, card, chip, chip_shown, open, &slot, f);
        }
    }

    /// The "+N" tab's kit menu: the tags that do not fit, with their counts.
    fn more_menu(&mut self, ctx: &egui::Context, f: &mut Frame<'_>) {
        let owner = ids::more_filters();
        if !kit::is_menu_open(ctx, owner) {
            return;
        }
        let entries: Vec<MenuEntry<'_>> = self.derived.more_rows.iter().map(|r| MenuEntry::Item(r.as_str())).collect();
        if let Some(i) = kit::menu_with(ctx, owner, &entries, menu_look()) {
            f.actions.push(StartAction::SetTagFilter(self.derived.plan.hidden.get(i).cloned()));
        }
    }

    /// The open card's kit menu: Locate… (Missing only), a hairline, Remove from Recent.
    fn card_menu(&mut self, ctx: &egui::Context, f: &mut Frame<'_>) {
        let Some(key) = self.menu_for.clone() else {
            return;
        };
        let owner = menu_owner(&key);
        let card = f.model.cards().iter().find(|c| c.key == key);
        let (Some(card), true) = (card, kit::is_menu_open(ctx, owner)) else {
            if kit::is_menu_open(ctx, owner) {
                kit::close_menu(ctx);
            }
            self.menu_for = None;
            return;
        };
        let entries = menu_entries(card.missing);
        if let Some(index) = kit::menu_with(ctx, owner, entries, menu_look()) {
            f.actions.push(match entries[index] {
                MenuEntry::Item(LOCATE) => StartAction::Locate(card.path.clone()),
                _ => StartAction::RemoveRecent(card.path.clone()),
            });
            self.menu_for = None;
        }
    }
}

/// The key-hint chips: groups of (keys, label); `center` centres the row in `rect`.
fn keys(ui: &Ui, rect: Rect, groups: &[(&[&str], &str)], center: bool) {
    let p = ui.painter().clone();
    let mut items: Vec<(Arc<Galley>, bool)> = vec![];
    for (keys, label) in groups {
        for k in *keys {
            items.push((text(ui, k, roles::MONO), true));
        }
        items.push((text(ui, label, roles::SMALL), false));
    }
    let chip_w = |g: &Galley| (g.size().x + t::SB_KBD_PAD * 2.0).max(t::SB_KBD_H);
    let mut total = 0.0;
    for (i, (g, is_key)) in items.iter().enumerate() {
        total += if *is_key { chip_w(g) } else { g.size().x };
        if i + 1 < items.len() {
            total += if *is_key { t::SB_KBD_GAP } else { t::SB_KEYS_GAP };
        }
    }
    let mut x = if center { rect.center().x - total / 2.0 } else { rect.left() };
    for (g, is_key) in items {
        if is_key {
            let w = chip_w(&g);
            kb::kbd(&p, Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(w, t::SB_KBD_H)), g);
            x += w + t::SB_KBD_GAP;
        } else {
            let w = g.size().x;
            kb::galley_in_line(&p, x, rect.top(), t::SB_KBD_H, g, t::MUTED);
            x += w + t::SB_KEYS_GAP;
        }
    }
}

/// The status line: version left; the Recent list's load warning after it; recovery right.
fn status(ui: &Ui, l: &PageLayout, version: &str, warning: Option<&str>, recovery_status: &str) {
    let p = ui.painter().clone();
    let s = l.status;
    let left = text(ui, version, roles::MICRO);
    let lw = left.size().x;
    kb::galley_in_line(&p, s.left(), s.top(), s.height(), left, t::MUTED);
    let right = text(ui, recovery_status, roles::MICRO);
    let rw = right.size().x;
    kb::galley_in_line(&p, s.right() - rw, s.top(), s.height(), right, t::MUTED);
    let ix = s.right() - rw - t::SB_STATUS_ICON_GAP - t::SB_ICON_SMALL / 2.0;
    Icon::Shield.paint(&p, egui::pos2(ix, s.center().y), t::SB_ICON_SMALL, t::MUTED);
    if let Some(w) = warning {
        let x = s.left() + lw + t::SB_FILTERS_GAP;
        let room = ix - t::SB_ICON_SMALL - t::SB_FILTERS_GAP - x;
        let g = text_elided(ui, w, roles::MICRO, room.max(0.0));
        kb::galley_in_line(&p, x, s.top(), s.height(), g, t::ERROR);
    }
}

/// The Start page's kit menus: the mockup's 184-wide popup, 28-tall 13 pt rows, LINE2 separator.
fn menu_look() -> MenuLook {
    MenuLook {
        min_width: t::SB_MENU_W - (t::KIT_TEXT_GAP + t::KIT_STROKE) * 2.0,
        row_h: t::SB_MENU_ROW_H,
        pad_x: t::SB_MENU_PAD,
        font: Some(t::body()),
        separator: t::LINE2,
    }
}

/// The "…" menu's entries: Locate… only for a Missing board.
pub fn menu_entries(missing: bool) -> &'static [MenuEntry<'static>] {
    if missing {
        &[MenuEntry::Item(LOCATE), MenuEntry::Separator, MenuEntry::Item(REMOVE)]
    } else {
        &[MenuEntry::Item(REMOVE)]
    }
}

/// The card's menu owner id.
pub fn menu_owner(key: &str) -> Id {
    Id::new(("start-v2-menu", key))
}

fn toggle_card_menu(ctx: &egui::Context, key: &str, chip: Rect) {
    let owner = menu_owner(key);
    if kit::is_menu_open(ctx, owner) {
        kit::close_menu(ctx);
    } else {
        let pos = egui::pos2(chip.right() - t::SB_MENU_W, chip.bottom() + t::KIT_MENU_GAP);
        kit::open_menu(ctx, owner, pos, Some(chip));
    }
}

fn step(order: &[Slot], at: &Slot, d: isize) -> Slot {
    let n = order.len() as isize;
    let i = order.iter().position(|s| s == at).map_or(0, |i| i as isize);
    order[(i + d).rem_euclid(n.max(1)) as usize].clone()
}
fn step_clamped(order: &[Slot], at: &Slot, d: isize) -> Slot {
    let i = order.iter().position(|s| s == at).map_or(0, |i| i as isize);
    order[(i + d).clamp(0, order.len() as isize - 1) as usize].clone()
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(std::path::PathBuf::from)
}

/// Tag pills left to right from `x`, stopping before `limit` (a "+n" pill stands for the rest).
fn pills(ui: &Ui, p: &egui::Painter, tags: &[String], x: f32, top: f32, limit: f32) {
    let mut x = x;
    for (i, tag) in tags.iter().enumerate() {
        let g = text(ui, tag, roles::TAG);
        let w = kb::pill_width(&g, t::SB_PILL_PAD);
        if x + w > limit {
            let more = text(ui, &format!("+{}", tags.len() - i), roles::TAG);
            let mw = kb::pill_width(&more, t::SB_PILL_PAD);
            if x + mw <= limit {
                kb::tag_pill(p, Rect::from_min_size(egui::pos2(x, top), egui::vec2(mw, t::SB_PILL_H)), more);
            }
            return;
        }
        kb::tag_pill(p, Rect::from_min_size(egui::pos2(x, top), egui::vec2(w, t::SB_PILL_H)), g);
        x += w + t::SB_PILL_GAP;
    }
}

/// The editor's dot grid on the well (src.html `.dots`: 12 px cells, a dot in each centre).
fn well_dots(p: &egui::Painter, well: Rect) {
    let half = t::SB_DOT_STEP / 2.0;
    let mut y = well.top() + half;
    while y < well.bottom() {
        let mut x = well.left() + half;
        while x < well.right() {
            p.circle_filled(egui::pos2(x, y), t::SB_DOT_R, t::WELL_DOT);
            x += t::SB_DOT_STEP;
        }
        y += t::SB_DOT_STEP;
    }
}

/// The thumbnail: a whole-well image (lane L3's well-shaped raster) fills the well; anything else is
/// fitted into the 244 × 99 art box, centred on the dots.
fn thumbnail(ui: &Ui, p: &egui::Painter, well: Rect, tex: TextureId) {
    let uv = Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
    let size = ui.ctx().tex_manager().read().meta(tex).map(|m| egui::vec2(m.size[0] as f32, m.size[1] as f32));
    let well_aspect = well.width() / well.height();
    let Some(size) = size.filter(|s| s.x > 0.0 && s.y > 0.0) else {
        p.image(tex, well, uv, IMAGE_TINT);
        return;
    };
    let aspect = size.x / size.y;
    if (aspect / well_aspect - 1.0).abs() <= t::SB_THUMB_WELL_TOLERANCE {
        let wr = egui::CornerRadius { nw: t::SB_WELL_R, ne: t::SB_WELL_R, sw: 0, se: 0 };
        p.add(egui::epaint::RectShape::filled(well, wr, IMAGE_TINT).with_texture(tex, uv));
        return;
    }
    let s = (t::SB_THUMB_W / size.x).min(t::SB_THUMB_H / size.y);
    p.image(tex, Rect::from_center_size(well.center(), size * s), uv, IMAGE_TINT);
}
/// Textures draw untinted.
const IMAGE_TINT: Color32 = Color32::WHITE;

/// No thumbnail yet: the board's initials in an outline per artboard (up to three, offset), or in a
/// dashed outline for a free board.
fn placeholder(ui: &Ui, p: &egui::Painter, well: Rect, card: &BoardCard) {
    let front = Rect::from_center_size(well.center(), egui::vec2(t::SB_PH_W, t::SB_PH_H));
    if card.artboards == 0 {
        kb::dashed_rect(p, front.shrink(t::KIT_STROKE / 2.0), t::MUTED);
    } else {
        let n = (card.artboards as usize).min(t::SB_PH_MAX);
        for k in (0..n).rev() {
            let off = egui::vec2(t::SB_PH_STACK, -t::SB_PH_STACK) * k as f32;
            let r = front.translate(off);
            p.rect_filled(r, egui::CornerRadius::ZERO, t::BG);
            let stroke = egui::Stroke::new(t::KIT_STROKE, t::MUTED);
            p.rect_stroke(r, egui::CornerRadius::ZERO, stroke, egui::StrokeKind::Inside);
        }
    }
    let g = text(ui, &initials(&card.name), roles::H2);
    p.galley(front.center() - g.size() / 2.0, g, t::MUTED);
}

/// Realistic stand-in data (the mockup's ten boards, one Recovered row) for the example gallery and
/// the tests, built through the real Recent cache (`Recents::record_board`) and `StartModel::build`.
#[doc(hidden)]
pub mod demo {
    use crate::start::{RecoveryRow, StartModel};
    use crate::storage::recents::{BoardSummary, Recents};
    use std::path::{Path, PathBuf};

    /// A fixed "now" (unix seconds) so relative dates are stable.
    pub const NOW: u64 = 1_790_000_000;
    pub type DemoBoard = (&'static str, &'static str, &'static [&'static str], &'static str, u64, u32);
    /// (name, description, tags, folder under the home dir, seconds before NOW, artboards), newest first.
    pub const BOARDS: [DemoBoard; 10] = [
        (
            "Ramadan campaign",
            "Key visual for Noor Foods — the portrait poster and its story cut-down.",
            &["client", "ramadan", "social"],
            "Design/Clients/Noor Foods/Ramadan 2026",
            600,
            2,
        ),
        (
            "Logo marks v3",
            "Third round of marks for Atlas Coffee. Free board, no artboards yet.",
            &["client", "logo"],
            "Design/Clients/Atlas Coffee/Identity",
            7_200,
            0,
        ),
        ("Form poster", "", &["personal", "print"], "Design/Personal/Posters", 90_000, 1),
        (
            "Icon set 24",
            "24 px outline icons, 1.5 stroke, for the product app.",
            &["product"],
            "Work/Product/Icons",
            110_000,
            0,
        ),
        ("Story launch", "", &["client", "social"], "Design/Clients/Noor Foods/Social", 200_000, 1),
        (
            "Business card",
            "Front and back, 85 × 55 mm.",
            &["client", "print"],
            "Design/Clients/Atlas Coffee/Print",
            400_000,
            2,
        ),
        (
            "Pattern tiles",
            "Quarter-circle tiles, still exploring the repeat.",
            &["personal"],
            "Design/Explorations",
            600_000,
            0,
        ),
        ("Invoice template", "", &["studio", "print"], "Documents/Studio/Admin", 1_200_000, 1),
        (
            "Eid greetings",
            "Square post for Eid al-Adha, two colourways.",
            &["client", "social"],
            "Design/Clients/Noor Foods/Social",
            1_500_000,
            1,
        ),
        (
            "Type specimen",
            "Display sizes and figures for the studio site.",
            &["personal", "type"],
            "Design/Explorations/Type",
            2_200_000,
            0,
        ),
    ];

    pub fn path(home: &Path, i: usize) -> PathBuf {
        let (name, _, _, folder, _, _) = BOARDS[i];
        home.join(folder).join(format!("{name}.vrs"))
    }

    /// Recent as the app would hold it after opening/saving the ten boards (oldest recorded first).
    pub fn recents(home: &Path) -> Recents {
        let mut r = Recents::default();
        for i in (0..BOARDS.len()).rev() {
            let (name, desc, tags, _, ago, artboards) = BOARDS[i];
            let summary = BoardSummary {
                name: name.into(),
                description: desc.into(),
                tags: tags.iter().map(|t| t.to_string()).collect(),
                artboards,
            };
            r.record_board(&path(home, i), None, NOW - ago, summary, NOW - ago);
        }
        r
    }

    pub fn recovered() -> RecoveryRow {
        RecoveryRow {
            rid: "recovery-menu-card".into(),
            name: "Menu card".into(),
            original_dir: Some("~/Design/Clients/Noor Foods/Menu".into()),
            saved_at_text: "unsaved changes from 11:48 today".into(),
            problem: None,
            busy: false,
        }
    }

    /// The mockup's `inter-recent` state: board `missing` (if any) is Missing, one Recovered row.
    pub fn model(home: &Path, missing: Option<usize>) -> StartModel {
        let gone = missing.map(|i| path(home, i));
        StartModel::build(&recents(home), NOW, |p| gone.as_deref() == Some(p), vec![recovered()])
    }
}
