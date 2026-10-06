//! Pure Start view model: Recent rows, optional claimed Recovery rows and keyboard navigation.
//! The host supplies file-existence probes and F2 recovery state outside paint; `start_page` renders
//! the model. Disabled or busy recovery actions never emit commands through keyboard activation.
//!
//! Start v2 (work order `START_V2_BOARDS.md`, the L2↔L4 interface): every Recent entry is also a
//! [`BoardCard`] built from Recent's cached board summary (Home never parses a `.vrs`), with a tag
//! list + counts ([`StartModel::tags`]) and pure filtering ([`StartFilter`]: tag, grid/list).
//! The filter decides which Recent rows keyboard traversal visits; with the default (empty) filter
//! the traversal is exactly the Start v1 contract.
//! Tab/Shift+Tab traverse actions, arrows move within a list, Enter activates and Delete (or Mac
//! delete, which arrives as Backspace) removes a focused Recent entry. Escape policy belongs to the
//! host.
use std::path::{Path, PathBuf};

use crate::storage::recents::Recents;
use crate::storage::time_text;
use varos_core::board::{fold, PresetId};

/// Window/heading title shown while Start is the active view (work order §3.7/§3.9: "Window
/// title \"Varos\" on Start."). E2's heading and host window title share this value.
pub const START_TITLE: &str = "Varos";
/// Exact empty-Recent copy (work order §3.7).
pub const EMPTY_RECENT_COPY: &str = "No recent documents. Create a document or open a .vrs file.";
/// Shown when Recent has boards but the tag filter hides all of them.
pub const NO_MATCH_COPY: &str = "No boards match. Choose All to see every board.";
/// Tag shown next to a recent row whose file can't be found on disk (work order §3.7). No consumer
/// yet: the Start page draws it beside a row whose [`StartRow::missing`] is true.
pub const MISSING_TAG: &str = "Missing";

/// How many characters a recent row's parent-folder text is elided to before the file-path
/// tooltip is the only place to see the full path (work order §3.7: "parent folder MUTED
/// (middle-elided, full path tooltip)"; the exact width is a layout detail left to `start_page.rs`,
/// so this is a reasonable default the host may override by calling [`elide_middle`] itself).
pub const DIR_ELIDE_MAX_CHARS: usize = 40;

/// One row in the Recent list, ready to draw: display strings precomputed, nothing left to derive
/// per frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartRow {
    pub path: PathBuf,
    pub name: String,
    /// Parent folder, middle-elided to [`DIR_ELIDE_MAX_CHARS`] (full path belongs in a tooltip).
    pub dir_elided: String,
    /// "3 min ago" / "Yesterday" / "12 Sep" etc., via [`time_text::relative`].
    pub when_text: String,
    /// True when the build-time probe reported the file can't be found. The row stays in the
    /// list either way — Start never silently drops a Recent entry for being missing.
    pub missing: bool,
}

/// The thumbnail cache key of a board card (filled by the thumbnail lane, L3).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ThumbKey(pub String);

/// One board card (Start v2): what a card or a list-table row draws, precomputed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardCard {
    /// Stable identity for UI ids (the path, as text).
    pub key: String,
    /// The display name (`varos_core::board::display_name`: board name, else the file stem).
    pub name: String,
    /// The board's description, `None` when it has none.
    pub description: Option<String>,
    pub tags: Vec<String>,
    /// Artboards on the board (0 = a free canvas; also 0 while [`Self::cached`] is false).
    pub artboards: u32,
    pub path: PathBuf,
    /// The file's modified time at its last successful open/save (unix seconds).
    pub modified: u64,
    /// The build-time probe says the file can't be found (the card stays; Locate / Remove).
    pub missing: bool,
    pub thumb: Option<ThumbKey>,
    // ── additions beyond the frozen interface, precomputed so nothing is derived per frame ──
    /// Parent folder, middle-elided to [`DIR_ELIDE_MAX_CHARS`] (full path in a tooltip).
    pub folder: String,
    /// `modified` as the card's date (`time_text::board_date`: "Today 14:32", "Yesterday 22:41",
    /// "2 Oct", "17 Sep 2025"; local time).
    pub modified_text: String,
    /// False for an entry Recent has not cached a board summary for yet (a list kept from before
    /// Start v2): tags/description/artboards are unknown, not empty — draw no count.
    pub cached: bool,
}

/// Grid of cards or the numbered list table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StartView {
    #[default]
    Grid,
    List,
}

/// One tag of the filter row with how many boards carry it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagCount {
    pub tag: String,
    pub count: usize,
}

/// The Start filter state: tag (`None` = All), view. Pure; applied by
/// [`StartModel::apply`] and carried across model rebuilds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StartFilter {
    pub tag: Option<String>,
    pub view: StartView,
}
impl StartFilter {
    /// Does `card` pass? Every comparison goes through the ONE core fold (`varos_core::board::fold`:
    /// NFC + full case fold, so ß/SS and σ/ς meet). Tag: folded equality with one of its tags.
    pub fn matches(&self, card: &BoardCard) -> bool {
        self.tag.as_ref().is_none_or(|tag| card.tags.iter().any(|t| fold(t) == fold(tag)))
    }
    /// Apply a filter action (`SetTagFilter`, `SetView`); `true` when the state changed.
    /// Every other action is not a filter action and returns `false`.
    pub fn apply(&mut self, action: &StartAction) -> bool {
        let changed = match action {
            StartAction::SetTagFilter(tag) => self.tag != *tag,
            StartAction::SetView(view) => self.view != *view,
            _ => return false,
        };
        match action {
            StartAction::SetTagFilter(tag) => self.tag.clone_from(tag),
            StartAction::SetView(view) => self.view = *view,
            _ => {}
        }
        changed
    }
}

/// One row in the optional Recovery section (work order §3.7: name · original folder · "saved
/// 14:32" · Recover/Discard). The real data (`rid`, folder, saved time) comes from F2's
/// `storage::recovery::OrphanEntry` scan; this type is defined here so E1 has something concrete
/// to build the focus/action model against without depending on the recovery store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryRow {
    pub rid: String,
    pub name: String,
    pub original_dir: Option<String>,
    /// Pre-formatted "saved 14:32" text (left to the caller — F2 knows whether to use
    /// `time_text::clock_hhmm` or a relative form).
    pub saved_at_text: String,
    pub problem: Option<String>,
    pub busy: bool,
}

/// What activating the currently focused Start element means. The host (E2) turns this into the
/// matching `AppCommand`. `Locate` is not reachable from [`StartModel::activate`]: E2 emits it
/// after the "can't be found" dialog it shows for a `missing` row's click (§3.7; E2 test
/// `locate_validates_before_relocating`), which is outside this pure model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartAction {
    /// "New board" (⌘N): a free canvas with zero artboards.
    NewBoard,
    /// "…or start with an artboard": one artboard from the core preset table.
    NewWithPreset(PresetId),
    /// Filter actions — handled by [`StartModel::apply`], never sent to the host.
    SetTagFilter(Option<String>),
    SetView(StartView),
    Open,
    OpenRecent(PathBuf),
    Locate(PathBuf),
    RemoveRecent(PathBuf),
    ClearRecent,
    Recover(String),
    DiscardRecovery(String),
    Later,
}

/// One focusable element, in traversal order. Private: callers only see [`StartModel::focus`]
/// (the index) and [`StartModel::activate`]/[`StartModel::delete_focused`] (what it means).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FocusTarget {
    NewDocument,
    Open,
    Recover(usize),
    Discard(usize),
    RecoveryLater,
    Recent(usize),
    ClearRecentFooter,
}

/// The Start page's pure view model: everything `start_page.rs` needs to draw, and everything
/// its key handling will need to decide what Tab/arrows/Enter/Delete do — with no `egui` in sight.
///
/// The collections and focus are private so the cached focus order can never drift out of step
/// with them; rebuild the model (from fresh `Recents`/recovery data) to change what it shows.
pub struct StartModel {
    rows: Vec<StartRow>,
    /// One card per row, same order (Start v2).
    cards: Vec<BoardCard>,
    tags: Vec<TagCount>,
    filter: StartFilter,
    /// Indices into `rows`/`cards` that pass `filter`, newest first.
    visible: Vec<usize>,
    recovery: Vec<RecoveryRow>,
    /// Index into the flat focus order (New, Open, [Recover/Discard per recovery row, then one
    /// trailing Later], [one entry per recent row, then a Clear-Recent footer]). Always in
    /// bounds: New and Open make the order never empty, and every setter keeps it in range.
    focus: usize,
    focus_order: Vec<FocusTarget>,
    /// A process-unique stamp, renewed by every build and every change of what is visible: the page
    /// keys its derived text (elided paths, counts) on it, so identical frames rebuild nothing.
    generation: u64,
}

/// The source of [`StartModel::generation`] stamps.
static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl StartModel {
    /// Build from Recents + a per-path "is it missing" probe, with recovery rows (supplied by F2
    /// after the launch scan). `now`/`last_opened` are unix seconds, matching [`Recents`].
    pub fn build(
        recents: &Recents,
        now: u64,
        mut missing: impl FnMut(&Path) -> bool,
        recovery: Vec<RecoveryRow>,
    ) -> Self {
        let mut rows = Vec::with_capacity(recents.entries().len());
        let mut cards = Vec::with_capacity(recents.entries().len());
        for e in recents.entries() {
            let is_missing = missing(&e.path);
            let folder = elide_middle(&parent_dir_display(&e.path), DIR_ELIDE_MAX_CHARS);
            rows.push(StartRow {
                path: e.path.clone(),
                name: e.name.clone(),
                dir_elided: folder.clone(),
                when_text: time_text::relative(now, e.last_opened),
                missing: is_missing,
            });
            let board = e.board.as_ref();
            cards.push(BoardCard {
                key: e.path.to_string_lossy().into_owned(),
                name: e.name.clone(),
                description: board.map(|b| b.description.clone()).filter(|d| !d.is_empty()),
                tags: board.map(|b| b.tags.clone()).unwrap_or_default(),
                artboards: board.map_or(0, |b| b.artboards),
                path: e.path.clone(),
                modified: e.modified,
                missing: is_missing,
                thumb: e.thumb.clone().map(ThumbKey),
                folder,
                modified_text: time_text::board_date(now, e.modified),
                cached: board.is_some(),
            });
        }
        let tags = tag_counts(&cards);
        let mut model = Self {
            rows,
            cards,
            tags,
            filter: StartFilter::default(),
            visible: Vec::new(),
            recovery,
            focus: 0,
            focus_order: Vec::new(),
            generation: 0,
        };
        model.refilter();
        model
    }

    /// Recompute the visible rows and the traversal order after a filter change, keeping focus on
    /// the same element when it is still visible (else the nearest index).
    fn refilter(&mut self) {
        self.generation = GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let current = self.focus_order.get(self.focus).copied();
        self.visible = (0..self.cards.len()).filter(|&i| self.filter.matches(&self.cards[i])).collect();
        let mut focus_order = vec![FocusTarget::NewDocument, FocusTarget::Open];
        for i in 0..self.recovery.len() {
            focus_order.push(FocusTarget::Recover(i));
            focus_order.push(FocusTarget::Discard(i));
        }
        if !self.recovery.is_empty() {
            focus_order.push(FocusTarget::RecoveryLater);
        }
        for &i in &self.visible {
            focus_order.push(FocusTarget::Recent(i));
        }
        if !self.rows.is_empty() {
            focus_order.push(FocusTarget::ClearRecentFooter);
        }
        self.focus = current
            .and_then(|t| focus_order.iter().position(|o| *o == t))
            .unwrap_or(self.focus.min(focus_order.len() - 1));
        self.focus_order = focus_order;
    }

    /// The model's stamp: it changes whenever the cards, the filter or what is visible changed.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Every board card, newest first (unfiltered; one per [`Self::rows`] entry).
    pub fn cards(&self) -> &[BoardCard] {
        &self.cards
    }

    /// The cards that pass the current filter, newest first.
    pub fn visible_cards(&self) -> impl Iterator<Item = &BoardCard> + '_ {
        self.visible.iter().map(|&i| &self.cards[i])
    }

    /// How many cards pass the current filter ("Recent boards 10").
    pub fn visible_count(&self) -> usize {
        self.visible.len()
    }

    /// The tag filter row: each tag (first spelling seen, newest board first) with its board count,
    /// most-used first, then alphabetical (case-insensitive). Counts ignore the current filter.
    pub fn tags(&self) -> &[TagCount] {
        &self.tags
    }

    /// The current filter state.
    pub fn filter(&self) -> &StartFilter {
        &self.filter
    }

    /// Apply a filter action (`SetTagFilter`, `SetView`). `true` when the state changed;
    /// any other action returns `false` and changes nothing.
    pub fn apply(&mut self, action: &StartAction) -> bool {
        if !self.filter.apply(action) {
            return false;
        }
        self.refilter();
        true
    }

    /// The copy for "Recent has boards but none pass the filter", else `None`.
    pub fn no_match_copy(&self) -> Option<&'static str> {
        (!self.rows.is_empty() && self.visible.is_empty()).then_some(NO_MATCH_COPY)
    }

    /// Convenience for a build with no recovery rows (launches without recovery copies).
    pub fn without_recovery(recents: &Recents, now: u64, missing: impl FnMut(&Path) -> bool) -> Self {
        Self::build(recents, now, missing, Vec::new())
    }

    /// The Recent rows, newest first (read-only).
    pub fn rows(&self) -> &[StartRow] {
        &self.rows
    }

    /// The Recovery rows (read-only).
    pub fn recovery(&self) -> &[RecoveryRow] {
        &self.recovery
    }

    /// Index of the focused element in the flat traversal order.
    pub fn focus(&self) -> usize {
        self.focus
    }

    /// Put focus on element `index` (e.g. after a click). Out-of-range indices are ignored and
    /// return `false`, so focus always stays valid.
    pub fn set_focus(&mut self, index: usize) -> bool {
        if index < self.focus_order.len() {
            self.focus = index;
            true
        } else {
            false
        }
    }

    /// Total number of focusable elements (always ≥ 2: New and Open).
    pub fn focus_count(&self) -> usize {
        self.focus_order.len()
    }

    /// Tab: next element in the whole traversal order, wrapping from the last back to New.
    pub fn tab_next(&mut self) {
        let len = self.focus_order.len();
        if len > 0 {
            self.focus = (self.focus + 1) % len;
        }
    }

    /// Shift+Tab: previous element in the whole traversal order, wrapping from New to the last.
    pub fn tab_prev(&mut self) {
        let len = self.focus_order.len();
        if len > 0 {
            self.focus = (self.focus + len - 1) % len;
        }
    }

    /// ↑: previous item of the current list only. Stops at the list's first item and never
    /// leaves the list; a no-op on elements that are not list items (New, Open, Later, Clear
    /// Recent). In the Recovery list focus keeps its column (Recover ↔ Recover, Discard ↔
    /// Discard).
    pub fn arrow_up(&mut self) {
        self.arrow(-1);
    }

    /// ↓: next item of the current list only. Stops at the list's last item and never leaves
    /// the list; a no-op on elements that are not list items.
    pub fn arrow_down(&mut self) {
        self.arrow(1);
    }

    fn arrow(&mut self, delta: isize) {
        let Some(current) = self.focus_order.get(self.focus).copied() else {
            return;
        };
        let wanted = match current {
            // Recent steps through the VISIBLE (filtered) boards, not raw row indices
            FocusTarget::Recent(i) => self
                .visible
                .iter()
                .position(|&v| v == i)
                .and_then(|at| at.checked_add_signed(delta))
                .and_then(|at| self.visible.get(at))
                .map(|&v| FocusTarget::Recent(v)),
            FocusTarget::Recover(i) => i.checked_add_signed(delta).map(FocusTarget::Recover),
            FocusTarget::Discard(i) => i.checked_add_signed(delta).map(FocusTarget::Discard),
            FocusTarget::NewDocument
            | FocusTarget::Open
            | FocusTarget::RecoveryLater
            | FocusTarget::ClearRecentFooter => None,
        };
        // Only move when the neighbour exists in the same list; otherwise stay put (list end).
        if let Some(pos) = wanted.and_then(|t| self.focus_order.iter().position(|o| *o == t)) {
            self.focus = pos;
        }
    }

    /// After a rebuild, keep focus on the same element of `old` by its key (Recent path, recovery
    /// id, or the fixed control). When that element is gone (e.g. its Recent row was removed), focus
    /// lands on the item now in its place in the same list (clamped), else on the nearest index.
    pub fn carry_focus_from(&mut self, old: &StartModel) {
        if self.filter != old.filter {
            self.filter = old.filter.clone(); // a rebuild keeps the user's tag / search / view
            self.refilter();
        }
        let Some(target) = old.focus_order.get(old.focus).copied() else {
            return;
        };
        let rid = |i: usize| old.recovery.get(i).map(|r| r.rid.as_str());
        let same = match target {
            FocusTarget::Recent(i) => {
                old.rows.get(i).and_then(|r| self.rows.iter().position(|n| n.path == r.path)).map(FocusTarget::Recent)
            }
            FocusTarget::Recover(i) => {
                rid(i).and_then(|id| self.recovery.iter().position(|n| n.rid == id)).map(FocusTarget::Recover)
            }
            FocusTarget::Discard(i) => {
                rid(i).and_then(|id| self.recovery.iter().position(|n| n.rid == id)).map(FocusTarget::Discard)
            }
            other => Some(other),
        };
        let clamped = |len: usize, i: usize| (len > 0).then(|| i.min(len - 1));
        let fallback = match target {
            FocusTarget::Recent(i) => clamped(self.rows.len(), i).map(FocusTarget::Recent),
            FocusTarget::Recover(i) => clamped(self.recovery.len(), i).map(FocusTarget::Recover),
            FocusTarget::Discard(i) => clamped(self.recovery.len(), i).map(FocusTarget::Discard),
            _ => None,
        };
        let find = |t: Option<FocusTarget>| t.and_then(|t| self.focus_order.iter().position(|o| *o == t));
        self.focus = find(same).or_else(|| find(fallback)).unwrap_or(old.focus.min(self.focus_order.len() - 1));
    }

    /// Enter: what the focused element does (`None` only if its target no longer resolves).
    pub fn activate(&self) -> Option<StartAction> {
        self.focus_order.get(self.focus).and_then(|t| self.action_for(*t))
    }

    /// Delete: removes the focused Recent row from the list (the work order's "Delete removes
    /// the focused recent row"); `None` anywhere else focus can be.
    pub fn delete_focused(&self) -> Option<StartAction> {
        match self.focus_order.get(self.focus) {
            Some(FocusTarget::Recent(i)) => self.rows.get(*i).map(|r| StartAction::RemoveRecent(r.path.clone())),
            _ => None,
        }
    }

    /// Exact copy for the empty-Recent state ([`EMPTY_RECENT_COPY`]), or `None` once any row
    /// exists.
    pub fn empty_copy(&self) -> Option<&'static str> {
        self.rows.is_empty().then_some(EMPTY_RECENT_COPY)
    }

    /// Resolve a focus target with checked indexing: a target whose row is gone yields `None`.
    fn action_for(&self, target: FocusTarget) -> Option<StartAction> {
        Some(match target {
            FocusTarget::NewDocument => StartAction::NewBoard,
            FocusTarget::Open => StartAction::Open,
            FocusTarget::Recover(i) => {
                let row = self.recovery.get(i)?;
                if row.busy || row.problem.is_some() {
                    return None;
                }
                StartAction::Recover(row.rid.clone())
            }
            FocusTarget::Discard(i) => {
                let row = self.recovery.get(i)?;
                if row.busy {
                    return None;
                }
                StartAction::DiscardRecovery(row.rid.clone())
            }
            FocusTarget::RecoveryLater => StartAction::Later,
            FocusTarget::Recent(i) => StartAction::OpenRecent(self.rows.get(i)?.path.clone()),
            FocusTarget::ClearRecentFooter => StartAction::ClearRecent,
        })
    }
}

/// When the host must rebuild Start's model: only while Home is showing, and only when one of the
/// inputs it is built from moved (Recent, recovery rows, existence answers — the host's generation
/// counters), or Home was just entered. Never after an ordinary edit batch in a document.
#[derive(Default)]
pub struct StartRefresh {
    built: Option<[u64; 3]>,
}
impl StartRefresh {
    pub fn should_rebuild(&mut self, home: bool, inputs: [u64; 3]) -> bool {
        if !home {
            self.built = None; // returning to Home rebuilds once (fresh probe answers)
            return false;
        }
        if self.built == Some(inputs) {
            return false;
        }
        self.built = Some(inputs);
        true
    }
}

/// Tag counts over every card: grouped by the core fold (first spelling seen wins, cards newest
/// first), most-used first, ties by folded spelling.
fn tag_counts(cards: &[BoardCard]) -> Vec<TagCount> {
    let mut out: Vec<(String, TagCount)> = Vec::new();
    for card in cards {
        for tag in &card.tags {
            let folded = fold(tag);
            match out.iter_mut().find(|(f, _)| *f == folded) {
                Some((_, t)) => t.count += 1,
                None => out.push((folded, TagCount { tag: tag.clone(), count: 1 })),
            }
        }
    }
    out.sort_by(|(fa, a), (fb, b)| b.count.cmp(&a.count).then_with(|| fa.cmp(fb)));
    out.into_iter().map(|(_, t)| t).collect()
}

fn parent_dir_display(path: &Path) -> String {
    path.parent().map(|p| p.to_string_lossy().into_owned()).filter(|s| !s.is_empty()).unwrap_or_default()
}

/// Middle-elide `s` to at most `max_chars` **characters** (not bytes), keeping both ends and
/// inserting a single `…`. UTF-8 safe by construction (`chars()` never splits a codepoint mid
/// sequence) and Arabic-safe in the sense the work order asks for: the cut points are always
/// character boundaries, never inside one. Returns `s` unchanged when it already fits.
pub fn elide_middle(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        return s.to_string();
    }
    if max_chars == 0 {
        return String::new();
    }
    if max_chars == 1 {
        return '…'.to_string();
    }
    let budget = max_chars - 1; // one char reserved for the ellipsis
    let head = budget.div_ceil(2);
    let tail = budget - head;
    let mut out = String::with_capacity(max_chars * 4); // headroom for multi-byte chars
    out.extend(chars[..head].iter());
    out.push('…');
    out.extend(chars[chars.len() - tail..].iter());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_790_260_320; // 2026-09-24 14:32:00 UTC, matches storage::time_text tests

    #[test]
    fn elide_middle_keeps_both_ends() {
        assert_eq!(elide_middle("short.vrs", 40), "short.vrs", "under the limit: unchanged");

        let out = elide_middle("abcdefghij", 5);
        assert_eq!(out, "ab…ij");

        // Arabic: never split a codepoint; both ends survive, one ellipsis sits in the middle.
        let arabic = "مجلد-مشاريع-التصميم-الخاصة-بالشعار-الجديد";
        let total = arabic.chars().count();
        assert!(total > 12, "fixture must actually need eliding");
        let out = elide_middle(arabic, 12);
        assert_eq!(out.chars().count(), 12);
        let chars: Vec<char> = arabic.chars().collect();
        let head: String = chars[..6].iter().collect(); // budget=11, head=ceil(11/2)=6
        let tail: String = chars[chars.len() - 5..].iter().collect(); // tail=5
        assert_eq!(out, format!("{head}…{tail}"));
        // Round-trips cleanly through `chars()`: no byte-level corruption.
        assert_eq!(out.chars().count(), out.chars().collect::<Vec<_>>().len());
    }

    #[test]
    fn missing_rows_are_flagged_not_dropped() {
        let mut recents = Recents::default();
        recents.record(Path::new("/docs/present.vrs"), None, 100);
        recents.record(Path::new("/docs/missing.vrs"), None, 200);
        let model = StartModel::without_recovery(&recents, 300, |p| p.ends_with("missing.vrs"));

        assert_eq!(model.rows().len(), 2, "the missing row stays in the list");
        let missing_row = model.rows().iter().find(|r| r.name == "missing").unwrap();
        assert!(missing_row.missing);
        let present_row = model.rows().iter().find(|r| r.name == "present").unwrap();
        assert!(!present_row.missing);
    }

    #[test]
    fn tab_wraps_and_activate_maps_to_actions() {
        let mut recents = Recents::default();
        recents.record(Path::new("/docs/a.vrs"), None, 100);
        recents.record(Path::new("/docs/b.vrs"), None, 200); // newest -> rows[0]
        let mut model = StartModel::without_recovery(&recents, 300, |_| false);

        // Order: New(0), Open(1), Recent(b)(2), Recent(a)(3), ClearRecentFooter(4).
        assert_eq!(model.focus_count(), 5);
        assert_eq!(model.focus(), 0);
        assert_eq!(model.activate(), Some(StartAction::NewBoard));

        model.tab_next();
        assert_eq!(model.focus(), 1);
        assert_eq!(model.activate(), Some(StartAction::Open));

        model.tab_next();
        assert_eq!(model.focus(), 2);
        assert_eq!(model.activate(), Some(StartAction::OpenRecent(PathBuf::from("/docs/b.vrs"))));

        // Shift+Tab from New wraps to the last element (the Clear Recent footer).
        assert!(model.set_focus(0));
        model.tab_prev();
        assert_eq!(model.focus(), 4);
        assert_eq!(model.activate(), Some(StartAction::ClearRecent));

        // Delete only means something on a focused Recent row.
        model.tab_prev(); // -> Recent(a) at index 3
        assert_eq!(model.focus(), 3);
        assert_eq!(model.delete_focused(), Some(StartAction::RemoveRecent(PathBuf::from("/docs/a.vrs"))));

        assert!(model.set_focus(1));
        assert_eq!(model.delete_focused(), None, "Delete does nothing off a recent row");

        // Out-of-range focus requests are refused; focus stays valid.
        assert!(!model.set_focus(5));
        assert_eq!(model.focus(), 1);
    }

    #[test]
    fn tab_from_last_wraps_to_new() {
        let mut recents = Recents::default();
        recents.record(Path::new("/docs/a.vrs"), None, 100);
        let mut model = StartModel::without_recovery(&recents, 200, |_| false);
        let last = model.focus_count() - 1;
        assert!(model.set_focus(last));
        assert_eq!(model.activate(), Some(StartAction::ClearRecent));
        model.tab_next();
        assert_eq!(model.focus(), 0);
        assert_eq!(model.activate(), Some(StartAction::NewBoard));
    }

    #[test]
    fn arrows_stay_inside_the_recent_list() {
        let mut recents = Recents::default();
        recents.record(Path::new("/docs/a.vrs"), None, 100);
        recents.record(Path::new("/docs/b.vrs"), None, 200); // newest -> rows[0]
        let mut model = StartModel::without_recovery(&recents, 300, |_| false);
        // Order: New(0), Open(1), Recent(b)(2), Recent(a)(3), ClearRecentFooter(4).

        assert!(model.set_focus(2)); // first Recent
        model.arrow_up();
        assert_eq!(model.focus(), 2, "↑ on the first Recent stays (no jump to Open)");

        model.arrow_down();
        assert_eq!(model.focus(), 3);
        assert_eq!(model.activate(), Some(StartAction::OpenRecent(PathBuf::from("/docs/a.vrs"))));

        model.arrow_down();
        assert_eq!(model.focus(), 3, "↓ on the last Recent stays (no jump to the footer, no wrap)");

        model.arrow_up();
        assert_eq!(model.focus(), 2);
    }

    #[test]
    fn arrows_do_nothing_outside_lists() {
        let mut recents = Recents::default();
        recents.record(Path::new("/docs/a.vrs"), None, 100);
        let recovery = vec![RecoveryRow {
            rid: "rid-1".to_string(),
            name: "Untitled-1".to_string(),
            original_dir: None,
            saved_at_text: "saved 14:32".to_string(),
            problem: None,
            busy: false,
        }];
        let mut model = StartModel::build(&recents, 200, |_| false, recovery);
        // Order: New(0), Open(1), Recover(2), Discard(3), Later(4), Recent(5), ClearRecent(6).

        model.arrow_up();
        assert_eq!(model.focus(), 0, "↑ on New stays");
        model.arrow_down();
        assert_eq!(model.focus(), 0, "↓ on New stays");

        for idx in [1, 4, 6] {
            assert!(model.set_focus(idx));
            model.arrow_up();
            assert_eq!(model.focus(), idx, "↑ is a no-op off a list item (index {idx})");
            model.arrow_down();
            assert_eq!(model.focus(), idx, "↓ is a no-op off a list item (index {idx})");
        }
    }

    #[test]
    fn arrows_in_recovery_keep_their_column_and_stop_at_ends() {
        let row = |n: u32| RecoveryRow {
            rid: format!("rid-{n}"),
            name: format!("Untitled-{n}"),
            original_dir: None,
            saved_at_text: "saved 14:32".to_string(),
            problem: None,
            busy: false,
        };
        let mut model = StartModel::build(&Recents::default(), 200, |_| false, vec![row(1), row(2)]);
        // Order: New(0), Open(1), Recover(0)(2), Discard(0)(3), Recover(1)(4), Discard(1)(5), Later(6).

        assert!(model.set_focus(3)); // Discard on the first recovery row
        model.arrow_up();
        assert_eq!(model.focus(), 3, "↑ on the first recovery row stays");
        model.arrow_down();
        assert_eq!(model.activate(), Some(StartAction::DiscardRecovery("rid-2".to_string())));
        model.arrow_down();
        assert_eq!(model.focus(), 5, "↓ on the last recovery row stays (never reaches Later)");

        assert!(model.set_focus(4)); // Recover on the second row
        model.arrow_up();
        assert_eq!(model.activate(), Some(StartAction::Recover("rid-1".to_string())));
    }

    #[test]
    fn empty_recent_copy_is_exact() {
        assert_eq!(EMPTY_RECENT_COPY, "No recent documents. Create a document or open a .vrs file.");

        let empty = Recents::default();
        let model = StartModel::without_recovery(&empty, NOW, |_| false);
        assert_eq!(model.empty_copy(), Some(EMPTY_RECENT_COPY));

        let mut one = Recents::default();
        one.record(Path::new("/docs/a.vrs"), None, 1);
        let model = StartModel::without_recovery(&one, NOW, |_| false);
        assert_eq!(model.empty_copy(), None);
    }

    #[test]
    fn recovery_rows_come_first_when_present() {
        let mut recents = Recents::default();
        recents.record(Path::new("/docs/a.vrs"), None, 100);
        let recovery = vec![RecoveryRow {
            rid: "rid-1".to_string(),
            name: "Untitled-1".to_string(),
            original_dir: None,
            saved_at_text: "saved 14:32".to_string(),
            problem: None,
            busy: false,
        }];
        let mut model = StartModel::build(&recents, 200, |_| false, recovery);

        // Order: New(0), Open(1), Recover(0)(2), Discard(0)(3), RecoveryLater(4), Recent(0)(5),
        // ClearRecentFooter(6) -- the recovery section is fully traversed before Recent.
        assert_eq!(model.focus_count(), 7);

        model.tab_next();
        model.tab_next();
        assert_eq!(model.activate(), Some(StartAction::Recover("rid-1".to_string())));

        model.tab_next();
        assert_eq!(model.activate(), Some(StartAction::DiscardRecovery("rid-1".to_string())));

        model.tab_next();
        assert_eq!(model.activate(), Some(StartAction::Later));

        model.tab_next();
        assert_eq!(
            model.activate(),
            Some(StartAction::OpenRecent(PathBuf::from("/docs/a.vrs"))),
            "recent rows follow the recovery section, never precede it"
        );
    }

    #[test]
    fn clearing_collections_never_panics_on_old_focus() {
        let mut recents = Recents::default();
        recents.record(Path::new("/docs/a.vrs"), None, 100);
        let recovery = vec![RecoveryRow {
            rid: "rid-1".to_string(),
            name: "Untitled-1".to_string(),
            original_dir: None,
            saved_at_text: "saved 14:32".to_string(),
            problem: None,
            busy: false,
        }];
        let mut model = StartModel::build(&recents, 200, |_| false, recovery);
        // Focus Recover(0) (index 2), then empty both collections behind the cached order.
        model.focus = 2;
        model.recovery.clear();
        assert_eq!(model.activate(), None, "stale Recover target resolves to None, not a panic");
        model.focus = 5; // Recent(0)
        model.rows.clear();
        assert_eq!(model.activate(), None);
        assert_eq!(model.delete_focused(), None, "stale Recent target resolves to None");
    }

    #[test]
    fn start_refresh_skips_documents_and_unchanged_inputs() {
        let mut refresh = StartRefresh::default();
        assert!(refresh.should_rebuild(true, [0, 0, 0]), "first Home frame builds");
        assert!(!refresh.should_rebuild(true, [0, 0, 0]), "nothing changed: no rebuild");
        for _ in 0..3 {
            assert!(!refresh.should_rebuild(false, [7, 7, 7]), "never while a document is active");
        }
        assert!(refresh.should_rebuild(true, [7, 7, 7]), "re-entering Home builds once");
        assert!(!refresh.should_rebuild(true, [7, 7, 7]));
        assert!(refresh.should_rebuild(true, [8, 7, 7]), "a Recent change rebuilds");
        assert!(refresh.should_rebuild(true, [8, 7, 8]), "a probe answer rebuilds");
    }

    #[test]
    fn focus_survives_rebuilds_and_lands_in_place_after_a_remove() {
        let mut recents = Recents::default();
        for (i, name) in ["/d/c.vrs", "/d/b.vrs", "/d/a.vrs"].iter().enumerate() {
            recents.record(Path::new(name), None, 100 + i as u64); // a newest → rows a, b, c
        }
        let mut old = StartModel::without_recovery(&recents, 300, |_| false);
        assert!(old.set_focus(3)); // Recent(b)
        let mut same = StartModel::without_recovery(&recents, 300, |_| true);
        same.carry_focus_from(&old);
        assert_eq!(same.activate(), Some(StartAction::OpenRecent("/d/b.vrs".into())), "same row by path");
        recents.remove(Path::new("/d/b.vrs"));
        let mut removed = StartModel::without_recovery(&recents, 300, |_| false);
        removed.carry_focus_from(&old);
        assert_eq!(removed.activate(), Some(StartAction::OpenRecent("/d/c.vrs".into())), "next row takes its place");
        assert!(old.set_focus(1));
        let mut empty = StartModel::without_recovery(&Recents::default(), 300, |_| false);
        empty.carry_focus_from(&old);
        assert_eq!(empty.activate(), Some(StartAction::Open));
    }

    use crate::storage::recents::BoardSummary;

    /// Four boards, newest first: c (client, logo), b (personal), a (Client, عربي), d (not cached).
    fn boards() -> Recents {
        let mut r = Recents::default();
        let summary = |name: &str, description: &str, tags: &[&str], artboards: u32| BoardSummary {
            name: name.into(),
            description: description.into(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            artboards,
        };
        r.record(Path::new("/old/d.vrs"), None, 50); // upgraded from store v1: no summary cached
        r.record_board(
            Path::new("/work/client/a.vrs"),
            None,
            100,
            summary("شعار المقهى", "", &["Client", "عربي"], 2),
            90,
        );
        r.record_board(Path::new("/home/b.vrs"), None, 200, summary("", "Birthday card", &["personal"], 0), 190);
        r.record_board(
            Path::new("/work/client/c.vrs"),
            None,
            300,
            summary("Cafe Logo", "Round two", &["client", "logo"], 1),
            290,
        );
        r
    }
    fn names(model: &StartModel) -> Vec<&str> {
        model.visible_cards().map(|c| c.name.as_str()).collect()
    }

    #[test]
    fn board_cards_come_from_the_recent_cache() {
        let model = StartModel::without_recovery(&boards(), 400, |p| p.ends_with("d.vrs"));
        let c = &model.cards()[0];
        assert_eq!(c.name, "Cafe Logo");
        assert_eq!(c.description.as_deref(), Some("Round two"));
        assert_eq!(c.tags, ["client", "logo"]);
        assert_eq!((c.artboards, c.modified, c.cached, c.missing), (1, 290, true, false));
        assert_eq!(c.key, "/work/client/c.vrs");
        assert_eq!(c.folder, "/work/client");
        assert_eq!(c.modified_text, time_text::board_date(400, 290));
        let b = &model.cards()[1];
        assert_eq!((b.name.as_str(), b.artboards), ("b", 0), "no board name → file stem; 0 = a free canvas");
        let d = &model.cards()[3];
        assert_eq!((d.name.as_str(), d.cached, d.missing, d.description.clone()), ("d", false, true, None));
        assert_eq!(model.cards().len(), model.rows().len(), "one card per Recent row");
        assert_eq!(model.visible_count(), 4);
    }

    #[test]
    fn tag_counts_group_case_insensitively_most_used_first() {
        let model = StartModel::without_recovery(&boards(), 400, |_| false);
        let tags: Vec<(&str, usize)> = model.tags().iter().map(|t| (t.tag.as_str(), t.count)).collect();
        assert_eq!(tags, [("client", 2), ("logo", 1), ("personal", 1), ("عربي", 1)], "newest spelling first seen wins");
    }

    #[test]
    fn tag_filter_is_case_insensitive_and_all_restores_every_board() {
        let mut model = StartModel::without_recovery(&boards(), 400, |_| false);
        assert!(model.apply(&StartAction::SetTagFilter(Some("CLIENT".into()))));
        assert_eq!(names(&model), ["Cafe Logo", "شعار المقهى"]);
        assert!(!model.apply(&StartAction::SetTagFilter(Some("CLIENT".into()))));
        model.apply(&StartAction::SetTagFilter(Some("absent".into())));
        assert_eq!(model.no_match_copy(), Some("No boards match. Choose All to see every board."));
        assert_eq!(model.visible_count(), 0);
        model.apply(&StartAction::SetTagFilter(None));
        assert_eq!(names(&model), ["Cafe Logo", "b", "شعار المقهى", "d"]);
        assert_eq!(model.no_match_copy(), None);
        assert!(model.apply(&StartAction::SetView(StartView::List)));
        assert_eq!(model.filter().view, StartView::List);
        assert_eq!(model.visible_count(), 4);
        assert!(!model.apply(&StartAction::NewBoard));
        assert!(!model.apply(&StartAction::OpenRecent("/x".into())));
    }

    #[test]
    fn keyboard_traversal_visits_only_visible_boards_and_filters_survive_rebuilds() {
        let mut model = StartModel::without_recovery(&boards(), 400, |_| false);
        // Order unfiltered: New, Open, c, b, a, d, Clear — the v1 contract
        assert_eq!(model.focus_count(), 7);
        assert!(model.set_focus(4)); // a
        model.apply(&StartAction::SetTagFilter(Some("client".into())));
        // Order: New, Open, c, a, Clear — focus stays on a
        assert_eq!(model.focus_count(), 5);
        assert_eq!(model.activate(), Some(StartAction::OpenRecent("/work/client/a.vrs".into())));
        model.arrow_up();
        assert_eq!(model.activate(), Some(StartAction::OpenRecent("/work/client/c.vrs".into())));
        model.arrow_up();
        assert_eq!(model.activate(), Some(StartAction::OpenRecent("/work/client/c.vrs".into())), "list end");
        // filtering away the focused board moves focus to the nearest index, never out of range
        model.apply(&StartAction::SetTagFilter(Some("personal".into())));
        assert!(model.focus() < model.focus_count());
        assert!(model.activate().is_some());
        // a rebuilt model (Recent changed) keeps the filter and the focused board
        let mut rebuilt = StartModel::without_recovery(&boards(), 500, |_| true);
        rebuilt.carry_focus_from(&model);
        assert_eq!(rebuilt.filter(), model.filter());
        assert_eq!(names(&rebuilt), ["b"]);
        assert_eq!(rebuilt.activate(), model.activate());
    }

    #[test]
    fn filter_counts_and_tags_use_the_one_core_fold() {
        let mut r = Recents::default();
        let board = |tags: &[&str]| BoardSummary {
            name: "Café".into(),
            description: String::new(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            artboards: 0,
        };
        r.record_board(Path::new("/d/a.vrs"), None, 1, board(&["Straße", "σς"]), 1);
        r.record_board(Path::new("/d/b.vrs"), None, 2, board(&["STRASSE", "ΣΣ", "عربي"]), 2);
        let mut model = StartModel::without_recovery(&r, 3, |_| false);
        let counts: Vec<(&str, usize)> = model.tags().iter().map(|t| (t.tag.as_str(), t.count)).collect();
        assert_eq!(counts, [("STRASSE", 2), ("ΣΣ", 2), ("عربي", 1)], "ß/SS and σς/ΣΣ count as one tag");
        model.apply(&StartAction::SetTagFilter(Some("straße".into())));
        assert_eq!(model.visible_count(), 2);
        model.apply(&StartAction::SetTagFilter(None));
        model.apply(&StartAction::SetTagFilter(Some("σσ".into())));
        assert_eq!(model.visible_count(), 2);
    }

    #[test]
    fn new_board_actions_and_filter_actions_are_distinct() {
        use varos_core::board::PresetId;
        let mut filter = StartFilter::default();
        assert!(!filter.apply(&StartAction::NewWithPreset(PresetId::A4)));
        assert!(!filter.apply(&StartAction::NewBoard));
        assert!(filter.apply(&StartAction::SetTagFilter(Some("x".into()))));
        assert_eq!(filter, StartFilter { tag: Some("x".into()), view: StartView::Grid });
    }

    #[test]
    fn relative_time_text_used() {
        let then = NOW - 120;
        let mut recents = Recents::default();
        recents.record(Path::new("/docs/a.vrs"), None, then);
        let model = StartModel::without_recovery(&recents, NOW, |_| false);

        assert_eq!(model.rows()[0].when_text, time_text::relative(NOW, then));
        assert_eq!(model.rows()[0].when_text, "2 min ago");
    }
}
