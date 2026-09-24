//! Start page — pure view model (work order `DFS_S2_S3_START_RECENTS_RECOVERY.md` §3.7, piece
//! **E1**).
//!
//! This module is model-only: drawing (`start_ui.rs`) is deferred by the moderator until the
//! UI-system kit (U0) lands, so nothing here touches `egui`, `main.rs`, `ui.rs` or `chrome.rs`.
//! [`StartModel`] is built from [`crate::storage::recents::Recents`] plus a caller-supplied
//! "is this file missing" probe (S3-B design: missing is computed once, at build time, not per
//! frame) and an optional list of [`RecoveryRow`]s (the data shape F2 will fill from
//! `storage::recovery::OrphanEntry`; empty until then). It also carries a flat keyboard-focus
//! model good enough for Tab/Shift+Tab/↑/↓ traversal, Enter-to-activate and Delete-to-remove —
//! the exact widget-level key routing is a job for the host once `start_ui.rs` exists.
use std::path::{Path, PathBuf};

use crate::storage::recents::Recents;
use crate::storage::time_text;

/// Window/heading title shown while Start is the active view (work order §3.7/§3.9: "Window
/// title \"Varos\" on Start.").
pub const START_TITLE: &str = "Varos";
/// Exact empty-Recent copy (work order §3.7).
pub const EMPTY_RECENT_COPY: &str = "No recent documents. Create a document or open a .vrs file.";
/// Tag shown next to a recent row whose file can't be found on disk (work order §3.7).
pub const MISSING_TAG: &str = "Missing";

/// How many characters a recent row's parent-folder text is elided to before the file-path
/// tooltip is the only place to see the full path (work order §3.7: "parent folder MUTED
/// (middle-elided, full path tooltip)"; the exact width is a layout detail left to `start_ui.rs`,
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
}

/// What activating the currently focused Start element means. The host (E2, wave 2) turns this
/// into the matching `AppCommand`. `Locate` is not reachable from [`StartModel::activate`] — it
/// is emitted by the host after the "can't be found" dialog it shows for a `missing` row's click,
/// which is outside this pure model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartAction {
    New,
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

/// The Start page's pure view model: everything `start_ui.rs` will need to draw, and everything
/// its key handling will need to decide what Enter/Delete/Escape do — with no `egui` in sight.
pub struct StartModel {
    pub rows: Vec<StartRow>,
    pub recovery: Vec<RecoveryRow>,
    /// Index into the flat focus order (New, Open, [Recover/Discard per recovery row, then one
    /// trailing Later], [one entry per recent row, then a Clear-Recent footer]). Always in
    /// bounds for a non-empty model; New and Open make the order never empty.
    pub focus: usize,
    focus_order: Vec<FocusTarget>,
}

impl StartModel {
    /// Build from Recents + a per-path "is it missing" probe, with recovery rows (empty until F2
    /// wires the real scan). `now`/`last_opened` are unix seconds, matching [`Recents`].
    pub fn build(
        recents: &Recents,
        now: u64,
        mut missing: impl FnMut(&Path) -> bool,
        recovery: Vec<RecoveryRow>,
    ) -> Self {
        let rows: Vec<StartRow> = recents
            .entries()
            .iter()
            .map(|e| StartRow {
                path: e.path.clone(),
                name: e.name.clone(),
                dir_elided: elide_middle(&parent_dir_display(&e.path), DIR_ELIDE_MAX_CHARS),
                when_text: time_text::relative(now, e.last_opened),
                missing: missing(&e.path),
            })
            .collect();

        let mut focus_order = vec![FocusTarget::NewDocument, FocusTarget::Open];
        for i in 0..recovery.len() {
            focus_order.push(FocusTarget::Recover(i));
            focus_order.push(FocusTarget::Discard(i));
        }
        if !recovery.is_empty() {
            focus_order.push(FocusTarget::RecoveryLater);
        }
        for i in 0..rows.len() {
            focus_order.push(FocusTarget::Recent(i));
        }
        if !rows.is_empty() {
            focus_order.push(FocusTarget::ClearRecentFooter);
        }

        Self { rows, recovery, focus: 0, focus_order }
    }

    /// Convenience for a build with no recovery rows (the common case until F2 lands).
    pub fn without_recovery(recents: &Recents, now: u64, missing: impl FnMut(&Path) -> bool) -> Self {
        Self::build(recents, now, missing, Vec::new())
    }

    /// Total number of focusable elements (always ≥ 2: New and Open).
    pub fn focus_count(&self) -> usize {
        self.focus_order.len()
    }

    /// Move focus by `delta` (Tab = +1, Shift+Tab = -1, ↑/↓ = ∓1), wrapping at both ends.
    pub fn move_focus(&mut self, delta: isize) {
        let len = self.focus_order.len();
        if len == 0 {
            return;
        }
        let len_i = len as isize;
        let mut new = (self.focus as isize + delta) % len_i;
        if new < 0 {
            new += len_i;
        }
        self.focus = new as usize;
    }

    /// Enter: what the focused element does.
    pub fn activate(&self) -> Option<StartAction> {
        self.focus_order.get(self.focus).map(|t| self.action_for(*t))
    }

    /// Delete: removes the focused Recent row from the list (the work order's "Delete removes
    /// the focused recent row"); `None` anywhere else focus can be.
    pub fn delete_focused(&self) -> Option<StartAction> {
        match self.focus_order.get(self.focus) {
            Some(FocusTarget::Recent(i)) => Some(StartAction::RemoveRecent(self.rows[*i].path.clone())),
            _ => None,
        }
    }

    /// Escape: the work order gives Start no modal to back out of, so this is the one sane
    /// default — return focus to the top (New document) rather than leave it stranded on a row
    /// that Tab/Shift+Tab no longer has to pass through once the host repaints.
    pub fn escape(&mut self) {
        self.focus = 0;
    }

    /// Exact copy for the empty-Recent state ([`EMPTY_RECENT_COPY`]), or `None` once any row
    /// exists.
    pub fn empty_copy(&self) -> Option<&'static str> {
        self.rows.is_empty().then_some(EMPTY_RECENT_COPY)
    }

    fn action_for(&self, target: FocusTarget) -> StartAction {
        match target {
            FocusTarget::NewDocument => StartAction::New,
            FocusTarget::Open => StartAction::Open,
            FocusTarget::Recover(i) => StartAction::Recover(self.recovery[i].rid.clone()),
            FocusTarget::Discard(i) => StartAction::DiscardRecovery(self.recovery[i].rid.clone()),
            FocusTarget::RecoveryLater => StartAction::Later,
            FocusTarget::Recent(i) => StartAction::OpenRecent(self.rows[i].path.clone()),
            FocusTarget::ClearRecentFooter => StartAction::ClearRecent,
        }
    }
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

        assert_eq!(model.rows.len(), 2, "the missing row stays in the list");
        let missing_row = model.rows.iter().find(|r| r.name == "missing.vrs").unwrap();
        assert!(missing_row.missing);
        let present_row = model.rows.iter().find(|r| r.name == "present.vrs").unwrap();
        assert!(!present_row.missing);
    }

    #[test]
    fn focus_wraps_and_activate_maps_to_actions() {
        let mut recents = Recents::default();
        recents.record(Path::new("/docs/a.vrs"), None, 100);
        recents.record(Path::new("/docs/b.vrs"), None, 200); // newest -> rows[0]
        let mut model = StartModel::without_recovery(&recents, 300, |_| false);

        // Order: New(0), Open(1), Recent(b)(2), Recent(a)(3), ClearRecentFooter(4).
        assert_eq!(model.focus_count(), 5);
        assert_eq!(model.focus, 0);
        assert_eq!(model.activate(), Some(StartAction::New));

        model.move_focus(1);
        assert_eq!(model.focus, 1);
        assert_eq!(model.activate(), Some(StartAction::Open));

        model.move_focus(1);
        assert_eq!(model.focus, 2);
        assert_eq!(model.activate(), Some(StartAction::OpenRecent(PathBuf::from("/docs/b.vrs"))));

        // Forward wrap: 2 + 3 = 5 -> 0.
        model.move_focus(3);
        assert_eq!(model.focus, 0);
        assert_eq!(model.activate(), Some(StartAction::New));

        // Backward wrap: 0 - 1 -> the last element.
        model.move_focus(-1);
        assert_eq!(model.focus, 4);
        assert_eq!(model.activate(), Some(StartAction::ClearRecent));

        // Delete only means something on a focused Recent row.
        model.move_focus(-1); // -> Recent(a) at index 3
        assert_eq!(model.focus, 3);
        assert_eq!(model.delete_focused(), Some(StartAction::RemoveRecent(PathBuf::from("/docs/a.vrs"))));

        model.move_focus(-2); // -> Open at index 1
        assert_eq!(model.focus, 1);
        assert_eq!(model.delete_focused(), None, "Delete does nothing off a recent row");

        model.move_focus(1);
        model.escape();
        assert_eq!(model.focus, 0, "Escape returns focus to the top");
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
        }];
        let mut model = StartModel::build(&recents, 200, |_| false, recovery);

        // Order: New(0), Open(1), Recover(0)(2), Discard(0)(3), RecoveryLater(4), Recent(0)(5),
        // ClearRecentFooter(6) -- the recovery section is fully traversed before Recent.
        assert_eq!(model.focus_count(), 7);

        model.move_focus(2);
        assert_eq!(model.activate(), Some(StartAction::Recover("rid-1".to_string())));

        model.move_focus(1);
        assert_eq!(model.activate(), Some(StartAction::DiscardRecovery("rid-1".to_string())));

        model.move_focus(1);
        assert_eq!(model.activate(), Some(StartAction::Later));

        model.move_focus(1);
        assert_eq!(
            model.activate(),
            Some(StartAction::OpenRecent(PathBuf::from("/docs/a.vrs"))),
            "recent rows follow the recovery section, never precede it"
        );
    }

    #[test]
    fn relative_time_text_used() {
        let then = NOW - 120;
        let mut recents = Recents::default();
        recents.record(Path::new("/docs/a.vrs"), None, then);
        let model = StartModel::without_recovery(&recents, NOW, |_| false);

        assert_eq!(model.rows[0].when_text, time_text::relative(NOW, then));
        assert_eq!(model.rows[0].when_text, "2 min ago");
    }
}
