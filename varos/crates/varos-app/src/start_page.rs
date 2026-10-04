//! Start v2 — Boards: the page (lane L4). Draws a [`StartView`] to the design of record
//! (design-reference/mockups/start-v2/: `inter-recent`, `inter-empty`, `inter-hover`, `inter-list`;
//! numbers from its `src.html`) and returns [`StartIntent`]s. Presentation only: no files, no
//! filtering, no commands. Hand-painted with the kit (`shell/kit/board.rs`); tokens only
//! (`shell/tokens.rs` `SB_*`, text roles in [`style`]); nothing animates.
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
//! "…" menu. The ring is the kit's 2 px azure outside ring with a 1 px gap, shown for keyboard only.
use std::path::Path;
use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{Align, Color32, Event, Galley, Id, Key, Rect, Sense, Ui};

use crate::shell::kit::{self, board as kb, Availability, Icon, MenuEntry, MenuLook};
use crate::shell::tokens as t;
use crate::start_view::{BoardCard, PresetId, RecoveredRow, StartIntent, StartView, ViewMode};

pub mod style {
    //! Text roles of the Start page — REPLACE WITH THE TYPE TOKENS FROM LANE L1 (`shell/tokens.rs`
    //! type section: h1 30/600, h2 18/600, button 15/500, name 14/600, body 13/400, small 12/400,
    //! tag 11/500, mono 11/400) when L1 lands. Sizes, line boxes and tracking are the mockup's; the
    //! `weight` is recorded for L1 but today's bundled faces are Regular only, so it is not drawn yet.
    use egui::{FontFamily, FontId};

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Role {
        pub size: f32,
        pub weight: u16,
        pub mono: bool,
        /// Letter-spacing in em (src.html `--ls-h` on headings, names and the hero titles).
        pub tracking_em: f32,
        /// The CSS line box this role is set in.
        pub line: f32,
    }
    impl Role {
        pub fn font(self) -> FontId {
            FontId::new(self.size, if self.mono { FontFamily::Monospace } else { FontFamily::Proportional })
        }
    }
    const HEADING: f32 = -0.02;
    const PLAIN: f32 = 0.0;
    pub const H1: Role = Role { size: 30.0, weight: 600, mono: false, tracking_em: HEADING, line: 40.0 };
    pub const H2: Role = Role { size: 18.0, weight: 600, mono: false, tracking_em: HEADING, line: 28.0 };
    pub const BUTTON_PRIMARY: Role = Role { size: 15.0, weight: 600, mono: false, tracking_em: HEADING, line: 20.0 };
    pub const BUTTON: Role = Role { size: 15.0, weight: 500, mono: false, tracking_em: HEADING, line: 20.0 };
    pub const NAME: Role = Role { size: 14.0, weight: 600, mono: false, tracking_em: HEADING, line: 20.0 };
    pub const LIST_NAME: Role = Role { size: 14.0, weight: 600, mono: false, tracking_em: HEADING, line: 19.0 };
    pub const LEDE_FIRST: Role = Role { size: 15.0, weight: 400, mono: false, tracking_em: PLAIN, line: 24.0 };
    pub const BODY: Role = Role { size: 13.0, weight: 400, mono: false, tracking_em: PLAIN, line: 20.0 };
    pub const BODY_MEDIUM: Role = Role { size: 13.0, weight: 500, mono: false, tracking_em: PLAIN, line: 16.0 };
    pub const SMALL: Role = Role { size: 12.0, weight: 400, mono: false, tracking_em: PLAIN, line: 16.0 };
    pub const SMALL_MEDIUM: Role = Role { size: 12.0, weight: 500, mono: false, tracking_em: PLAIN, line: 16.0 };
    pub const DESC: Role = Role { size: 12.0, weight: 400, mono: false, tracking_em: PLAIN, line: 17.0 };
    pub const TAG: Role = Role { size: 11.0, weight: 500, mono: false, tracking_em: PLAIN, line: 20.0 };
    pub const XS: Role = Role { size: 11.0, weight: 400, mono: false, tracking_em: PLAIN, line: 16.0 };
    pub const MONO: Role = Role { size: 11.0, weight: 400, mono: true, tracking_em: PLAIN, line: 16.0 };
}
use style::Role;

/// Copy (owner-approved mockup text).
pub const LEDE: &str = "A board is a free canvas with a name, a short description and tags. Artboards inside are optional — add one when a piece needs a fixed size.";
pub const FIRST_TITLE: &str = "Start with a board";
pub const RECOVERY_STATUS: &str = "Recovery on · copies every 30 seconds";
pub const SEARCH_HINT: &str = "Search boards";
const LOCATE: &str = "Locate…";
const REMOVE: &str = "Remove from Recent";

/// Stable ids of the page's controls (tests and the host read their responses).
pub mod ids {
    use crate::start_view::{PresetId, ViewMode};
    use egui::Id;
    pub fn new_board() -> Id {
        Id::new("start-v2-new")
    }
    pub fn open() -> Id {
        Id::new("start-v2-open")
    }
    pub fn preset(p: PresetId) -> Id {
        Id::new(("start-v2-preset", p))
    }
    pub fn recover(id: &str) -> Id {
        Id::new(("start-v2-recover", id))
    }
    pub fn discard(id: &str) -> Id {
        Id::new(("start-v2-discard", id))
    }
    pub fn filter(tag: Option<&str>) -> Id {
        Id::new(("start-v2-filter", tag))
    }
    pub fn view() -> Id {
        Id::new("start-v2-view")
    }
    pub fn view_segment(v: ViewMode) -> Id {
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
    pub fn search() -> Id {
        Id::new("start-v2-search")
    }
}

// ───────────────────────────── pure layout ─────────────────────────────

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
pub fn layout(area: Rect, view: &StartView) -> PageLayout {
    let board = Rect::from_min_max(
        area.min + egui::vec2(t::SEAM_GAP, t::SEAM_GAP),
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
        first_launch: view.is_first_launch(),
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
        let lede_h = style::LEDE_FIRST.line * 2.0;
        let block = style::H1.line
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
        l.title = Rect::from_min_size(egui::pos2(x, y), egui::vec2(cw, style::H1.line));
        y += style::H1.line + t::SB_FIRST_LEDE_GAP;
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
        l.lede = Rect::from_min_size(egui::pos2(x, lede_top), egui::vec2(t::SB_LEDE_W.min(cw), style::BODY.line * 2.0));
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
        for i in 0..view.recovered.len() {
            y += if i == 0 { t::SB_SECTION_GAP } else { t::SB_GUTTER };
            l.recovered.push(Rect::from_min_size(egui::pos2(x, y), egui::vec2(cw, t::SB_RECOV_H)));
            y += t::SB_RECOV_H;
        }
        y += t::SB_SECTION_GAP;
        l.head = Rect::from_min_size(egui::pos2(x, y), egui::vec2(cw, t::SB_HEAD_H));
        y += t::SB_HEAD_H + t::SB_HEAD_GAP;
        let mut bottom = y + style::BODY.line;
        match view.view {
            ViewMode::Grid => {
                for i in 0..view.cards.len() {
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
            ViewMode::List => {
                l.table_head = Rect::from_min_size(egui::pos2(x, y), egui::vec2(cw, t::SB_TH_H));
                bottom = l.table_head.bottom();
                for _ in &view.cards {
                    let row = Rect::from_min_size(egui::pos2(x, bottom), egui::vec2(cw, t::SB_ROW_H));
                    bottom = row.bottom();
                    l.rows.push(row);
                }
            }
        }
        l.content_bottom = (bottom + t::SB_PAD_BOTTOM).max(board.bottom());
    }
    let inner = l.presets.shrink(t::KIT_STROKE);
    let cell_w = inner.width() / PresetId::ALL.len() as f32;
    let row_top = inner.top() + t::SB_PANEL_HEAD_H;
    l.preset_cells = (0..PresetId::ALL.len())
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

/// "Varos 0.1 α" from the crate version.
pub fn version_text() -> String {
    let v = env!("CARGO_PKG_VERSION");
    let short: Vec<&str> = v.split('.').take(2).collect();
    format!("Varos {} α", short.join("."))
}

// ───────────────────────────── text ─────────────────────────────

fn format(role: Role) -> TextFormat {
    TextFormat {
        font_id: role.font(),
        color: Color32::PLACEHOLDER,
        extra_letter_spacing: role.size * role.tracking_em,
        ..Default::default()
    }
}
fn text(ui: &Ui, s: &str, role: Role) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    job.append(s, 0.0, format(role));
    ui.fonts_mut(|f| f.layout_job(job))
}
fn text_elided(ui: &Ui, s: &str, role: Role, width: f32) -> Arc<Galley> {
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
fn path_galley(ui: &Ui, folder: &str, width: f32) -> Arc<Galley> {
    let fitted = elide_middle(folder, |c| width_of(ui, c, style::MONO) <= width);
    text(ui, &fitted, style::MONO)
}

// ───────────────────────────── focus ─────────────────────────────

/// A keyboard stop. Cards are keyed, so focus survives a rebuild of the view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Slot {
    New,
    Open,
    Preset(PresetId),
    Discard(String),
    Recover(String),
    Filter(Option<String>),
    View(ViewMode),
    Card(String),
}
impl Slot {
    /// The row a slot belongs to (←/→ move inside one row).
    fn row(&self) -> (u8, &str) {
        match self {
            Slot::New | Slot::Open => (0, ""),
            Slot::Preset(_) => (1, ""),
            Slot::Discard(id) | Slot::Recover(id) => (2, id),
            Slot::Filter(_) => (3, ""),
            Slot::View(_) => (4, ""),
            Slot::Card(_) => (5, ""),
        }
    }
}

/// The page's Tab order for `view` (disabled actions are skipped; only the filters that fit).
pub fn tab_order(view: &StartView, visible_filters: usize) -> Vec<Slot> {
    let mut slots = vec![Slot::New, Slot::Open];
    slots.extend(PresetId::ALL.iter().map(|p| Slot::Preset(*p)));
    if view.is_first_launch() {
        return slots;
    }
    for r in &view.recovered {
        if !r.busy {
            slots.push(Slot::Discard(r.id.clone()));
            if r.problem.is_none() {
                slots.push(Slot::Recover(r.id.clone()));
            }
        }
    }
    let filters = std::iter::once(None).chain(view.tags.iter().map(|(tag, _)| Some(tag.clone())));
    slots.extend(filters.take(visible_filters.max(1)).map(Slot::Filter));
    slots.extend([Slot::View(ViewMode::Grid), Slot::View(ViewMode::List)]);
    slots.extend(view.cards.iter().map(|c| Slot::Card(c.key.clone())));
    slots
}

/// The Start v2 page state: keyboard focus and its visibility, the open "…" menu, the search buffer.
pub struct StartPage {
    focus: Slot,
    ring: bool,
    entered: bool,
    menu_for: Option<String>,
    last_card: usize,
    visible_filters: usize,
    search: String,
    /// Handle ⌘N / ⌘O here. Off in the app (the host's command keys own them, K2 row 1); the example
    /// gallery turns it on.
    pub command_keys: bool,
}
impl Default for StartPage {
    fn default() -> Self {
        Self::new()
    }
}

struct Frame<'a> {
    view: &'a StartView,
    intents: Vec<StartIntent>,
    clicked: Option<Slot>,
    focused_rect: Option<Rect>,
}

impl StartPage {
    pub fn new() -> Self {
        Self {
            focus: Slot::New,
            ring: false,
            entered: false,
            menu_for: None,
            last_card: 0,
            visible_filters: usize::MAX,
            search: String::new(),
            command_keys: false,
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

    /// Draw into everything `ui` has left (the app: the root under the top bar).
    pub fn draw(&mut self, ui: &mut Ui, view: &StartView) -> Vec<StartIntent> {
        let area = ui.available_rect_before_wrap();
        self.draw_in(ui, area, view)
    }

    /// The top bar's "Search boards" pill at `rect` (the host places it; mockup 200 × 24). Emits
    /// `Search` on every change; Esc restores the text it had when it took the keyboard.
    pub fn search_box(&mut self, ui: &mut Ui, rect: Rect, view: &StartView) -> Option<StartIntent> {
        let id = ids::search();
        if !ui.ctx().memory(|m| m.has_focus(id)) {
            self.search.clone_from(&view.search);
        }
        let changed = kb::search_pill(ui, id, rect, &mut self.search, style::SMALL.font(), SEARCH_HINT);
        changed.then(|| StartIntent::Search(self.search.clone()))
    }

    /// Draw the page in `area`.
    pub fn draw_in(&mut self, ui: &mut Ui, area: Rect, view: &StartView) -> Vec<StartIntent> {
        let lay = layout(area, view);
        self.carry_focus(view);
        let mut f = Frame { view, intents: vec![], clicked: None, focused_rect: None };
        let moved = self.keyboard(ui, view, &lay, &mut f);
        ui.allocate_rect(area, Sense::hover());
        let p = ui.painter().clone();
        p.rect_filled(area, egui::CornerRadius::ZERO, t::SEAM);
        p.rect_filled(lay.board, t::r_box(), t::BG);
        p.rect_stroke(lay.board, t::r_box(), t::hairline(), egui::StrokeKind::Inside);
        self.status(ui, &lay);
        let inner = lay.board.shrink(t::KIT_STROKE);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(Align::Min)));
        egui::ScrollArea::vertical().id_salt("start-v2-scroll").auto_shrink([false, false]).show(&mut child, |ui| {
            // the bottom padding is border-inclusive: content that ends on the box's bottom edge fits exactly
            let h = inner.height() + (lay.content_bottom - lay.board.bottom()).max(0.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(inner.width(), h), Sense::hover());
            let l = lay.translated(rect.top() - inner.top());
            self.content(ui, &l, &mut f);
            if moved {
                if let Some(r) = f.focused_rect {
                    ui.scroll_to_rect(r, None);
                }
            }
        });
        self.card_menu(ui.ctx(), &mut f);
        if let Some(slot) = f.clicked.take() {
            self.focus = slot;
            self.entered = true;
        }
        f.intents
    }

    /// A rebuilt view: keep focus on the same element by key, or the card now in its place.
    fn carry_focus(&mut self, view: &StartView) {
        let order = tab_order(view, self.visible_filters);
        if order.contains(&self.focus) {
            if let Slot::Card(key) = &self.focus {
                self.last_card = view.cards.iter().position(|c| &c.key == key).unwrap_or(self.last_card);
            }
            return;
        }
        self.focus = match &self.focus {
            Slot::Card(_) if !view.cards.is_empty() => {
                Slot::Card(view.cards[self.last_card.min(view.cards.len() - 1)].key.clone())
            }
            Slot::Card(_) => Slot::View(view.view),
            Slot::Filter(_) => Slot::Filter(None),
            _ => Slot::New,
        };
        if self.menu_for.as_ref().is_some_and(|k| !view.cards.iter().any(|c| &c.key == k)) {
            self.menu_for = None;
        }
    }

    /// The page's keyboard (K2 row 2). Returns whether focus moved (to scroll it into view).
    fn keyboard(&mut self, ui: &mut Ui, view: &StartView, lay: &PageLayout, f: &mut Frame<'_>) -> bool {
        let ctx = ui.ctx().clone();
        let events = ui.input(|i| i.events.clone());
        let blocked = kit::menu_open(&ctx) || ctx.memory(|m| m.focused().is_some());
        let order = tab_order(view, self.visible_filters);
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
                            Key::N => f.intents.push(StartIntent::NewBoard),
                            Key::O => f.intents.push(StartIntent::Open),
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
                            if next.row() == self.focus.row()
                                && !(view.view == ViewMode::List && matches!(next, Slot::Card(_)))
                            {
                                self.focus = next;
                            }
                            self.entered = true;
                            true
                        }
                        Key::ArrowUp | Key::ArrowDown => {
                            let d: isize = if key == Key::ArrowUp { -1 } else { 1 };
                            self.vertical(view, lay, d);
                            true
                        }
                        Key::Enter | Key::Space if !repeat => {
                            if let Some(i) = activate(&self.focus, view) {
                                f.intents.push(i);
                            }
                            true
                        }
                        Key::Delete | Key::Backspace if !repeat => {
                            if let Slot::Card(k) = &self.focus {
                                f.intents.push(StartIntent::Remove(k.clone()));
                            }
                            true
                        }
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
    fn vertical(&mut self, view: &StartView, lay: &PageLayout, d: isize) {
        let keys: Vec<&str> = view.cards.iter().map(|c| c.key.as_str()).collect();
        if keys.is_empty() {
            return;
        }
        let at = match &self.focus {
            Slot::Card(k) => keys.iter().position(|c| c == k),
            _ => None,
        };
        let next = match at {
            None => 0,
            Some(i) => {
                let span = if view.view == ViewMode::Grid { lay.cols as isize } else { 1 };
                let j = i as isize + d * span;
                if j < 0 || j >= keys.len() as isize {
                    i
                } else {
                    j as usize
                }
            }
        };
        self.focus = Slot::Card(keys[next].to_string());
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
            let title = text(ui, FIRST_TITLE, style::H1);
            kb::galley_in_line(
                &p,
                l.title.center().x - title.size().x / 2.0,
                l.title.top(),
                l.title.height(),
                title,
                t::TEXT,
            );
            let lede = text_wrapped(ui, LEDE, style::LEDE_FIRST, l.lede.width(), 2, Align::Center);
            p.galley(egui::pos2(l.lede.center().x, l.lede.top()), lede, t::MUTED);
            self.actions(ui, l, f);
            self.keys(ui, l.keys, &[(&["Return"], "New board")], true);
            self.presets(ui, l, f);
            return;
        }
        self.actions(ui, l, f);
        let p = ui.painter().clone();
        let lede = text_wrapped(ui, LEDE, style::BODY, l.lede.width(), 2, Align::Min);
        p.galley(l.lede.min, lede, t::MUTED);
        self.keys(ui, l.keys, &[(&["↑", "↓"], "Move"), (&["Return"], "Open board"), (&["Delete"], REMOVE)], false);
        self.presets(ui, l, f);
        for (row, rect) in f.view.recovered.iter().zip(&l.recovered) {
            self.recovered(ui, row, *rect, f);
        }
        self.head(ui, l, f);
        if f.view.cards.is_empty() {
            let copy = if f.view.total == 0 { "No recent boards yet." } else { "No boards match." };
            let g = text(ui, copy, style::BODY);
            kb::galley_in_line(&p, l.content.left(), l.head.bottom() + t::SB_HEAD_GAP, style::BODY.line, g, t::MUTED);
            return;
        }
        match f.view.view {
            ViewMode::Grid => {
                for (card, rect) in f.view.cards.iter().zip(&l.cards) {
                    self.card(ui, card, *rect, f);
                }
            }
            ViewMode::List => self.table(ui, l, f),
        }
    }

    fn actions(&mut self, ui: &mut Ui, l: &PageLayout, f: &mut Frame<'_>) {
        for (slot, rect, icon, title, sub, key, primary, intent) in [
            (
                Slot::New,
                l.new_board,
                Icon::ArtboardAdd,
                "New board",
                "Free canvas, no size needed",
                "N",
                true,
                StartIntent::NewBoard,
            ),
            (Slot::Open, l.open, Icon::Open, "Open…", "A .vrs file from disk", "O", false, StartIntent::Open),
        ] {
            let b = kb::BigButton {
                icon,
                title: text(ui, title, if primary { style::BUTTON_PRIMARY } else { style::BUTTON }),
                sub: text(ui, sub, style::SMALL),
                shortcut: text(ui, &t::shortcut_label(key), style::MONO),
                primary,
                focused: self.ring_on(&slot),
            };
            let id = if primary { ids::new_board() } else { ids::open() };
            let r = kb::big_button(ui, id, rect, b, title);
            self.mark(f, &slot, rect);
            if r.activated {
                f.intents.push(intent);
                f.clicked = Some(slot);
            }
        }
    }

    /// The key-hint chips: groups of (keys, label); `center` centres the row in `rect`.
    fn keys(&self, ui: &Ui, rect: Rect, groups: &[(&[&str], &str)], center: bool) {
        let p = ui.painter().clone();
        let mut items: Vec<(Arc<Galley>, bool)> = vec![];
        for (keys, label) in groups {
            for k in *keys {
                items.push((text(ui, k, style::MONO), true));
            }
            items.push((text(ui, label, style::SMALL), false));
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

    fn presets(&mut self, ui: &mut Ui, l: &PageLayout, f: &mut Frame<'_>) {
        let p = ui.painter().clone();
        let panel = l.presets;
        p.rect_filled(panel, t::r_box(), t::PANEL);
        let head_bottom = panel.top() + t::KIT_STROKE + t::SB_PANEL_HEAD_H;
        p.hline(panel.x_range(), head_bottom - t::KIT_STROKE / 2.0, t::hairline());
        let head = text(ui, "…or start with an artboard", style::SMALL_MEDIUM);
        let hw = head.size().x;
        let line_top = panel.top() + t::KIT_STROKE;
        let hx = panel.left() + t::KIT_STROKE + t::SB_PANEL_PAD_X;
        kb::galley_in_line(&p, hx, line_top, t::SB_PANEL_HEAD_H - t::KIT_STROKE, head, t::TEXT);
        let sub = text(ui, "— true proportions", style::SMALL);
        kb::galley_in_line(
            &p,
            hx + hw + t::SB_PANEL_HEAD_GAP,
            line_top,
            t::SB_PANEL_HEAD_H - t::KIT_STROKE,
            sub,
            t::MUTED,
        );
        for (i, (preset, cell)) in PresetId::ALL.iter().zip(&l.preset_cells).enumerate() {
            if i > 0 {
                p.vline(cell.left() - t::KIT_STROKE / 2.0, cell.y_range(), t::hairline());
            }
            let preview = match preset.size() {
                Some((w, h, _)) => kb::PresetPreview::Outline(egui::vec2(w as f32, h as f32) * t::SB_PRESET_SCALE),
                None => kb::PresetPreview::Custom,
            };
            let name_top = cell.top() + t::SB_PRESET_TOP + t::SB_PRESET_PV + t::SB_PRESET_NAME_GAP;
            let name = text(ui, preset.label(), style::BODY_MEDIUM);
            let name_y = name_top + (style::BODY_MEDIUM.line - name.size().y) / 2.0;
            let size_top = name_top + style::BODY_MEDIUM.line + t::SB_PRESET_SIZE_GAP;
            let size = text(ui, &preset.size_text(), style::MONO);
            let size_line = style::MONO.line - t::KIT_STROKE;
            let size_y = size_top + (size_line - size.size().y) / 2.0;
            let slot = Slot::Preset(*preset);
            let label = format!("New board with a {} artboard", preset.label());
            let r = kb::preset_cell(
                ui,
                ids::preset(*preset),
                *cell,
                preview,
                (name, name_y),
                (size, size_y),
                self.ring_on(&slot),
                &label,
            );
            self.mark(f, &slot, *cell);
            if r.activated {
                f.intents.push(StartIntent::NewWithPreset(*preset));
                f.clicked = Some(slot);
            }
        }
        // the border last, over the cell washes
        p.rect_stroke(panel, t::r_box(), t::hairline(), egui::StrokeKind::Inside);
    }

    fn recovered(&mut self, ui: &mut Ui, row: &RecoveredRow, rect: Rect, f: &mut Frame<'_>) {
        let p = ui.painter().clone();
        p.rect_filled(rect, t::r_box(), t::PANEL);
        p.rect_stroke(rect, t::r_box(), egui::Stroke::new(t::KIT_STROKE, t::LINE2), egui::StrokeKind::Inside);
        let icon_x = rect.left() + t::SB_RECOV_PAD_L;
        Icon::History.paint(&p, egui::pos2(icon_x + t::SB_ICON_HERO / 2.0, rect.center().y), t::SB_ICON_HERO, t::TEXT);
        // buttons, right to left: Recover (solid), Discard (ghost)
        let label_r = text(ui, "Recover", style::SMALL_MEDIUM);
        let label_d = text(ui, "Discard", style::SMALL_MEDIUM);
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
        let (sd, sr) = (Slot::Discard(row.id.clone()), Slot::Recover(row.id.clone()));
        let r = kb::text_button(
            ui,
            ids::discard(&row.id),
            discard,
            label_d,
            kb::ButtonKind::Ghost,
            dis_av,
            self.ring_on(&sd),
            "Discard",
        );
        self.mark(f, &sd, discard);
        if r.activated {
            f.intents.push(StartIntent::Discard(row.id.clone()));
            f.clicked = Some(sd);
        }
        let r = kb::text_button(
            ui,
            ids::recover(&row.id),
            recover,
            label_r,
            kb::ButtonKind::Solid,
            rec_av,
            self.ring_on(&sr),
            "Recover",
        );
        self.mark(f, &sr, recover);
        if r.activated {
            f.intents.push(StartIntent::Recover(row.id.clone()));
            f.clicked = Some(sr);
        }
        // text: name · when (or the problem) · folder, on one baseline; the folder is elided to fit
        let x = icon_x + t::SB_ICON_HERO + t::SB_RECOV_ICON_GAP;
        let right = discard.left() - t::SB_RECOV_ICON_GAP;
        let name = text_elided(ui, &format!("Recovered — {}", row.name), style::BODY_MEDIUM, (right - x).max(0.0));
        let nw = name.size().x;
        let nr = kb::galley_in_line(
            &p,
            x,
            rect.center().y - style::BODY_MEDIUM.line / 2.0,
            style::BODY_MEDIUM.line,
            name.clone(),
            t::TEXT,
        );
        let baseline = nr.top() + kb::first_baseline(&name);
        let mut cx = x + nw + t::SB_RECOV_TEXT_GAP;
        let when = row.problem.as_deref().unwrap_or(&row.when);
        if cx < right {
            let g = text_elided(ui, when, style::BODY, right - cx);
            let w = g.size().x;
            kb::galley_at_baseline(&p, cx, baseline, g, t::MUTED);
            cx += w + t::SB_RECOV_TEXT_GAP;
        }
        if let Some(folder) = row.folder.as_deref().filter(|_| cx < right) {
            let g = path_galley(ui, folder, right - cx);
            kb::galley_at_baseline(&p, cx, baseline, g, t::MUTED);
        }
    }

    fn head(&mut self, ui: &mut Ui, l: &PageLayout, f: &mut Frame<'_>) {
        let p = ui.painter().clone();
        let h = l.head;
        let title = text(ui, "Recent boards", style::H2);
        let tw = title.size().x;
        kb::galley_in_line(&p, h.left(), h.top(), h.height(), title, t::TEXT);
        let count = text(ui, &f.view.total.to_string(), style::MONO);
        let cnt_w = count.size().x;
        let cnt_x = h.left() + tw + t::SB_COUNT_GAP;
        // `.cnt { padding-top: 2px }` inside a centred row: one pixel lower than centre
        kb::galley_in_line(&p, cnt_x, h.top() + t::KIT_STROKE, h.height(), count, t::MUTED);
        // the view toggle, right-aligned
        let seg_w = (t::SB_SEG_BTN_W + t::SB_SEG_PAD) * 2.0 + t::SB_SEG_PAD + t::KIT_STROKE * 2.0;
        let seg_h = t::SB_SEG_BTN_H + (t::SB_SEG_PAD + t::KIT_STROKE) * 2.0;
        let seg =
            Rect::from_min_size(egui::pos2(h.right() - seg_w, h.center().y - seg_h / 2.0), egui::vec2(seg_w, seg_h));
        let focused = [ViewMode::Grid, ViewMode::List].iter().position(|v| self.ring_on(&Slot::View(*v)));
        let (chosen, segs) = kb::segmented(
            ui,
            ids::view(),
            seg,
            &[(Icon::Grid, "Grid"), (Icon::List, "List")],
            f.view.view as usize,
            focused,
        );
        for (i, v) in [ViewMode::Grid, ViewMode::List].into_iter().enumerate() {
            self.mark(f, &Slot::View(v), segs[i]);
        }
        if let Some(i) = chosen {
            let v = if i == 0 { ViewMode::Grid } else { ViewMode::List };
            f.intents.push(StartIntent::SetView(v));
            f.clicked = Some(Slot::View(v));
        }
        // the tag filter: All N · tag n …, as many as fit before the toggle
        let mut x = cnt_x + cnt_w + t::SB_FILTERS_GAP;
        let limit = seg.left() - t::SB_FILTERS_GAP;
        let entries = std::iter::once((None, f.view.total))
            .chain(f.view.tags.iter().map(|(tag, n)| (Some(tag.as_str()), *n)))
            .collect::<Vec<_>>();
        let mut shown = 0;
        for (tag, n) in entries {
            let label = text(ui, tag.unwrap_or("All"), style::BODY);
            let count = text(ui, &n.to_string(), style::MONO);
            let w = t::SB_FILTER_PAD * 2.0 + label.size().x + t::SB_FILTER_INNER + count.size().x;
            if x + w > limit {
                break;
            }
            let rect = Rect::from_min_size(egui::pos2(x, h.top()), egui::vec2(w, h.height()));
            let slot = Slot::Filter(tag.map(str::to_string));
            let selected = f.view.filter.as_deref() == tag;
            let name = tag.unwrap_or("All").to_string();
            let r = kb::filter_tab(ui, ids::filter(tag), rect, label, count, selected, self.ring_on(&slot), &name);
            self.mark(f, &slot, rect);
            if r.activated {
                f.intents.push(StartIntent::SetTagFilter(tag.map(str::to_string)));
                f.clicked = Some(slot);
            }
            x += w + t::SB_FILTER_SPACING;
            shown += 1;
        }
        self.visible_filters = shown;
    }

    fn card(&mut self, ui: &mut Ui, card: &BoardCard, rect: Rect, f: &mut Frame<'_>) {
        let slot = Slot::Card(card.key.clone());
        let focused = self.ring_on(&slot);
        let menu_open =
            self.menu_for.as_deref() == Some(card.key.as_str()) && kit::is_menu_open(ui.ctx(), menu_owner(&card.key));
        let well = card_well(rect);
        let state = kb::CardState { focused, menu_open };
        let (r, lit) = kb::card(ui, ids::card(&card.key), rect, well, state, &card.name);
        self.mark(f, &slot, rect);
        let p = ui.painter().clone();
        let wp = p.with_clip_rect(well.intersect(p.clip_rect()));
        if card.missing {
            let row_top = well.top() + t::SB_MISS_Y;
            let ix = well.left() + t::SB_MISS_X;
            Icon::FileQuestion.paint(
                &wp,
                egui::pos2(ix + t::ICON_MD / 2.0, row_top + t::ICON_MD / 2.0),
                t::ICON_MD,
                t::MUTED,
            );
            let g = text(ui, "File not found", style::SMALL);
            kb::galley_in_line(&wp, ix + t::ICON_MD + t::SB_MISS_GAP, row_top, t::ICON_MD, g, t::MUTED);
        } else {
            well_dots(&wp, well);
            match card.thumb {
                Some(tex) => thumbnail(ui, &wp, well, tex),
                None => placeholder(ui, &wp, well, card),
            }
        }
        // meta
        let x0 = rect.left() + t::SB_CARD_PAD_X;
        let x1 = rect.right() - t::SB_CARD_PAD_X;
        let top = well.bottom() + t::SB_CARD_PAD_TOP;
        let line = style::NAME.line;
        let (right_w, baseline_item) = if card.missing {
            let g = text(ui, "Missing", style::TAG);
            let w = kb::pill_width(&g, t::SB_MISS_PILL_PAD);
            let pill = Rect::from_min_size(
                egui::pos2(x1 - w, top + (line - t::SB_MISS_PILL_H) / 2.0),
                egui::vec2(w, t::SB_MISS_PILL_H),
            );
            kb::outline_pill(&p, pill, g);
            (w, None)
        } else {
            let g = text(ui, &card.modified, style::MONO);
            (g.size().x, Some(g))
        };
        let name_w = (x1 - x0 - right_w - t::SB_DATE_GAP).max(0.0);
        let name = text_elided(ui, &card.name, style::NAME, name_w);
        let nr = kb::galley_in_line(&p, x0, top, line, name.clone(), if card.missing { t::MUTED } else { t::TEXT });
        if let Some(date) = baseline_item {
            let w = date.size().x;
            kb::galley_at_baseline(&p, x1 - w, nr.top() + kb::first_baseline(&name), date, t::MUTED);
        }
        let mut y = top + line;
        if let Some(desc) = card.description.as_deref().filter(|d| !d.is_empty()) {
            let g = text_wrapped(ui, desc, style::DESC, x1 - x0, 2, Align::Min);
            p.galley(egui::pos2(x0, y + t::SB_DESC_GAP), g, t::MUTED);
            y += t::SB_DESC_GAP + style::DESC.line * 2.0;
        }
        let tags_top = y + t::SB_TAGS_GAP;
        let facts_g = text(ui, &facts(card.artboards), style::MONO);
        let fw = facts_g.size().x;
        kb::galley_in_line(&p, x1 - fw, tags_top, t::SB_PILL_H, facts_g, t::MUTED);
        pills(ui, &p, &card.tags, x0, tags_top, x1 - fw - t::SB_FACTS_GAP);
        let path_top = rect.bottom() - t::SB_CARD_PAD_BOTTOM - style::MONO.line;
        let folder = folder_text(&card.path, home_dir().as_deref());
        let g = path_galley(ui, &folder, x1 - x0);
        kb::galley_in_line(&p, x0, path_top, style::MONO.line, g, t::MUTED);
        kb::card_border(ui, rect, lit, focused);
        if r.activated {
            f.intents.push(StartIntent::OpenBoard(card.key.clone()));
            f.clicked = Some(slot.clone());
        }
        if r.response.secondary_clicked() {
            let at = r.response.interact_pointer_pos().unwrap_or(rect.center());
            kit::open_menu(ui.ctx(), menu_owner(&card.key), at, None);
            self.menu_for = Some(card.key.clone());
            f.clicked = Some(slot.clone());
        }
        if lit {
            let chip = card_chip(rect);
            let c = kb::more_chip(ui, ids::chip(&card.key), chip, menu_open);
            if c.activated {
                toggle_card_menu(ui.ctx(), &card.key, chip);
                self.menu_for = Some(card.key.clone());
                f.clicked = Some(slot);
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
            let g = text(ui, label, style::XS);
            kb::galley_in_line(&p, lx, th.top(), th.height(), g, t::MUTED);
        }
        let g = text(ui, "Modified", style::XS);
        let w = g.size().x;
        kb::galley_in_line(&p, right - w, th.top(), th.height(), g, t::MUTED);
        p.hline(th.x_range(), th.bottom() - t::KIT_STROKE / 2.0, t::hairline());
        for (i, (card, rect)) in f.view.cards.iter().zip(&l.rows).enumerate() {
            let rect = *rect;
            let slot = Slot::Card(card.key.clone());
            let r = kb::table_row(ui, ids::row(&card.key), rect, self.ring_on(&slot), &card.name);
            self.mark(f, &slot, rect);
            let n = text(ui, &(i + 1).to_string(), style::MONO);
            kb::galley_in_line(&p, x + t::SB_NUM_PAD, rect.top(), rect.height(), n, t::MUTED);
            let ink = if card.missing { t::MUTED } else { t::TEXT };
            let name = text_elided(ui, &card.name, style::LIST_NAME, fr);
            match card.description.as_deref().filter(|d| !d.is_empty()) {
                Some(desc) => {
                    let block = style::LIST_NAME.line + t::SB_LIST_DESC_GAP + style::SMALL.line;
                    let top = rect.center().y - block / 2.0;
                    kb::galley_in_line(&p, name_x, top, style::LIST_NAME.line, name, ink);
                    let d = text_elided(ui, desc, style::SMALL, fr);
                    kb::galley_in_line(
                        &p,
                        name_x,
                        top + style::LIST_NAME.line + t::SB_LIST_DESC_GAP,
                        style::SMALL.line,
                        d,
                        t::MUTED,
                    );
                }
                None => {
                    kb::galley_in_line(&p, name_x, rect.top(), rect.height(), name, ink);
                }
            }
            pills(ui, &p, &card.tags, tags_x, rect.center().y - t::SB_PILL_H / 2.0, tags_x + wide);
            let folder = folder_text(&card.path, home_dir().as_deref());
            let g = path_galley(ui, &folder, wide);
            kb::galley_in_line(&p, folder_x, rect.top(), rect.height(), g, t::MUTED);
            if card.missing {
                let g = text(ui, "Missing", style::TAG);
                let w = kb::pill_width(&g, t::SB_MISS_PILL_PAD);
                let pill = Rect::from_min_size(
                    egui::pos2(right - w, rect.center().y - t::SB_MISS_PILL_H / 2.0),
                    egui::vec2(w, t::SB_MISS_PILL_H),
                );
                kb::outline_pill(&p, pill, g);
            } else {
                let g = text(ui, &card.modified, style::MONO);
                let w = g.size().x;
                kb::galley_in_line(&p, right - w, rect.top(), rect.height(), g, t::TEXT);
            }
            if r.activated {
                f.intents.push(StartIntent::OpenBoard(card.key.clone()));
                f.clicked = Some(slot.clone());
            }
            if r.response.secondary_clicked() {
                let at = r.response.interact_pointer_pos().unwrap_or(rect.center());
                kit::open_menu(ui.ctx(), menu_owner(&card.key), at, None);
                self.menu_for = Some(card.key.clone());
                f.clicked = Some(slot);
            }
        }
    }

    fn status(&self, ui: &Ui, l: &PageLayout) {
        let p = ui.painter().clone();
        let s = l.status;
        let left = text(ui, &version_text(), style::XS);
        kb::galley_in_line(&p, s.left(), s.top(), s.height(), left, t::MUTED);
        let right = text(ui, RECOVERY_STATUS, style::XS);
        let rw = right.size().x;
        kb::galley_in_line(&p, s.right() - rw, s.top(), s.height(), right, t::MUTED);
        let ix = s.right() - rw - t::SB_STATUS_ICON_GAP - t::SB_ICON_SMALL / 2.0;
        Icon::Shield.paint(&p, egui::pos2(ix, s.center().y), t::SB_ICON_SMALL, t::MUTED);
    }

    /// The open card's kit menu: Locate… (Missing only), a hairline, Remove from Recent.
    fn card_menu(&mut self, ctx: &egui::Context, f: &mut Frame<'_>) {
        let Some(key) = self.menu_for.clone() else {
            return;
        };
        let owner = menu_owner(&key);
        let card = f.view.cards.iter().find(|c| c.key == key);
        let (Some(card), true) = (card, kit::is_menu_open(ctx, owner)) else {
            if kit::is_menu_open(ctx, owner) {
                kit::close_menu(ctx);
            }
            self.menu_for = None;
            return;
        };
        let entries = menu_entries(card.missing);
        let look = MenuLook {
            min_width: t::SB_MENU_W - (t::KIT_TEXT_GAP + t::KIT_STROKE) * 2.0,
            row_h: t::SB_MENU_ROW_H,
            pad_x: t::SB_MENU_PAD,
            text_size: Some(style::BODY.size),
            separator: t::LINE2,
        };
        if let Some(index) = kit::menu_with(ctx, owner, entries, look) {
            f.intents.push(match entries[index] {
                MenuEntry::Item(LOCATE) => StartIntent::Locate(key),
                _ => StartIntent::Remove(key),
            });
            self.menu_for = None;
        }
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

/// What Enter / Space on `slot` asks for.
pub fn activate(slot: &Slot, view: &StartView) -> Option<StartIntent> {
    Some(match slot {
        Slot::New => StartIntent::NewBoard,
        Slot::Open => StartIntent::Open,
        Slot::Preset(p) => StartIntent::NewWithPreset(*p),
        Slot::Discard(id) => StartIntent::Discard(id.clone()),
        Slot::Recover(id) => {
            let row = view.recovered.iter().find(|r| &r.id == id)?;
            if row.busy || row.problem.is_some() {
                return None;
            }
            StartIntent::Recover(id.clone())
        }
        Slot::Filter(tag) => StartIntent::SetTagFilter(tag.clone()),
        Slot::View(v) => StartIntent::SetView(*v),
        Slot::Card(k) => StartIntent::OpenBoard(k.clone()),
    })
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(std::path::PathBuf::from)
}

/// Tag pills left to right from `x`, stopping before `limit` (a "+n" pill stands for the rest).
fn pills(ui: &Ui, p: &egui::Painter, tags: &[String], x: f32, top: f32, limit: f32) {
    let mut x = x;
    for (i, tag) in tags.iter().enumerate() {
        let g = text(ui, tag, style::TAG);
        let w = kb::pill_width(&g, t::SB_PILL_PAD);
        if x + w > limit {
            let more = text(ui, &format!("+{}", tags.len() - i), style::TAG);
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
fn thumbnail(ui: &Ui, p: &egui::Painter, well: Rect, tex: egui::TextureId) {
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
            p.rect_stroke(
                r,
                egui::CornerRadius::ZERO,
                egui::Stroke::new(t::KIT_STROKE, t::MUTED),
                egui::StrokeKind::Inside,
            );
        }
    }
    let g = text(ui, &initials(&card.name), style::H2);
    p.galley(front.center() - g.size() / 2.0, g, t::MUTED);
}
