//! The multi-document workspace (DFS S1 §3.3): every tab is a real, independent document — its own
//! `Editor` (document + history + selection), `View`, file path, saved-content checkpoint and dirty
//! state. Pure data: no window, no GPU, no dialogs, no file system (those are the lifecycle's ports).
//!
//! **Dirty rule** (§3.1): a session is dirty when its document's AUTHORED CONTENT differs from the
//! checkpoint taken at New / open / save (`Document::content_eq`), plus an in-flight overlay (a
//! history transaction is open and has changed something). Undo back to the saved state, no-op
//! commits and every view / preference action are therefore clean.
//!
//! **App-wide vs per-tab state** (§6 Q3): the in-app clipboard, the current tool and the recent
//! colours are app-wide — `activate` / `remove` / `add_loaded` hand them from the outgoing tab's
//! editor to the incoming one. Current fill/stroke/weight, rulers/guides visibility and the view stay
//! per tab.
//!
//! API frozen by S1-A — S1-B (lifecycle) and S1-D (host) build on it as is.

use std::cell::Cell;
use std::path::PathBuf;

use varos_core::editor::{AbDrag, Drag, Editor, Mods};
use varos_core::geom::View;
use varos_core::model::Document;

use crate::app_command::{SessionId, TabView};

/// A file's identity for "is this file already open?". Built by `DocStore::key` (absolute +
/// canonical path, plus device/inode on unix so symlinks and case aliases match).
///
/// **Compare with [`FileKey::same_file`], never `==`**: the derived `PartialEq` compares path AND
/// `dev_ino`, so a key taken before a save and one taken after it (the inode changed) are `!=` even
/// though they name the same file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileKey {
    pub path: PathBuf,
    pub dev_ino: Option<(u64, u64)>,
}
impl FileKey {
    /// Same file on disk: the (canonical) paths are equal, OR both keys carry a device/inode and
    /// those are equal. Path first, because every save replaces the inode (`write_atomic` writes a
    /// temp file and renames it over the target), so a key taken before a save and one taken after
    /// it still match by path; the inode catches aliases (symlinks, case variants) of a live file.
    pub fn same_file(&self, o: &FileKey) -> bool {
        self.path == o.path || matches!((self.dev_ino, o.dev_ino), (Some(a), Some(b)) if a == b)
    }
}

/// One open document (one tab).
pub struct DocumentSession {
    pub id: SessionId,
    pub editor: Editor,
    pub view: View,
    /// Where it is saved; `None` until the first Save.
    pub path: Option<PathBuf>,
    /// The file identity of `path` (open dedup / Save-As-onto-another-tab check).
    pub key: Option<FileKey>,
    /// `Some(n)` ⇒ named `Untitled-n` (never saved). Cleared by `mark_saved`.
    pub untitled: Option<u32>,
    /// A fit the host still owes this tab (the `View::fit` pad factor), applied once the Board box
    /// size is known; the host takes it (`= None`) when applied.
    pub fit_pending: Option<f32>,
    /// The saved-content checkpoint (a clone taken at New, open and save).
    saved: Document,
    /// `is_dirty` memo: `(editor.rev, content differs)`, only ever written while no transaction is open.
    memo: Cell<Option<(u64, bool)>>,
}

impl DocumentSession {
    fn untitled(id: SessionId, n: u32) -> Self {
        let editor = Editor::new();
        let saved = editor.doc.clone();
        DocumentSession {
            id,
            editor,
            view: View::identity(),
            path: None,
            key: None,
            untitled: Some(n),
            fit_pending: Some(0.45),
            saved,
            memo: Cell::new(None),
        }
    }
    fn loaded(id: SessionId, doc: Document, path: PathBuf, key: FileKey) -> Self {
        let mut editor = Editor::new();
        editor.replace_doc(doc);
        let saved = editor.doc.clone(); // AFTER replace_doc: the checkpoint is the post-`sync_tree` doc
        DocumentSession {
            id,
            editor,
            view: View::identity(),
            path: Some(path),
            key: Some(key),
            untitled: None,
            fit_pending: Some(0.9),
            saved,
            memo: Cell::new(None),
        }
    }

    /// `Untitled-3`, or the file name WITH its extension (`Logo.vrs`).
    pub fn display_name(&self) -> String {
        if let Some(name) = self.path.as_ref().and_then(|p| p.file_name()) {
            return name.to_string_lossy().into_owned();
        }
        match self.untitled {
            Some(n) => format!("Untitled-{n}"),
            None => "Untitled".into(),
        }
    }

    /// Unsaved changes, for DRAWING (the tab dot, the title `*`): memoised on `editor.rev`, plus the
    /// in-flight overlay. Content changes inside a history transaction (which sets `dirty`) and is
    /// recorded with a `rev` bump when it commits, so the memo holds for every committed transaction
    /// (EditCommands and pointer gestures alike); a content write that skipped `begin`/`dirty`/`commit`
    /// would be missed by the memo only until the next `rev` change, which is why
    /// every Save / Close / Quit decision uses `is_dirty_exact` instead. `mark_saved` resets the memo.
    pub fn is_dirty(&self) -> bool {
        let ed = &self.editor;
        if ed.transaction_open() && ed.dirty {
            return true; // a changed gesture still in flight
        }
        if let Some((rev, dirty)) = self.memo.get() {
            if rev == ed.rev {
                return dirty;
            }
        }
        self.content_dirty()
    }

    /// Unsaved changes, compared fresh (ignores the memo). Every Save / Close / Quit decision uses this.
    pub fn is_dirty_exact(&self) -> bool {
        let ed = &self.editor;
        self.content_dirty() || (ed.transaction_open() && ed.dirty)
    }

    /// Nothing to lose and nowhere saved: an Open may replace this tab instead of adding one.
    pub fn is_pristine(&self) -> bool {
        self.path.is_none() && !self.is_dirty_exact() && !self.editor.transaction_open()
    }

    /// A save to `path` succeeded: the tab takes that path/name, the checkpoint becomes the current
    /// content (clean), and it stops being `Untitled-n`.
    pub fn mark_saved(&mut self, path: PathBuf, key: FileKey) {
        self.path = Some(path);
        self.key = Some(key);
        self.untitled = None;
        self.saved = self.editor.doc.clone();
        self.memo.set((!self.editor.transaction_open()).then_some((self.editor.rev, false)));
    }

    /// Store a freshly computed file key for this tab's path (e.g. `store.key(path)` right after a
    /// successful save, which replaced the file's inode). `mark_saved` takes one too.
    // Frozen S1-A API with no caller in the S1 binary yet (the lifecycle re-keys every tab fresh,
    // `lifecycle::open_tab_of`); its tests use it, and S2/S3 are its planned callers.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn set_key(&mut self, key: FileKey) {
        self.key = Some(key);
    }

    /// Finish a pointer gesture still in flight (a canvas drag or an Artboard-tool drag) the way a
    /// mouse release would, so a lifecycle command never saves / closes / switches mid-gesture.
    ///
    /// Also finishes a gesture whose transaction is open without a drag (a Pen click that added or
    /// deleted an anchor, before its release). Call `Ui::settle` FIRST: it cancels an open colour
    /// picker, whose transaction must be reverted, not committed here.
    pub fn settle(&mut self) {
        let ed = &mut self.editor;
        if !matches!(ed.drag, Drag::None) || !matches!(ed.ab_drag, AbDrag::None) || ed.transaction_open() {
            ed.pointer_up();
            // With the Artboard tool `pointer_up` ends only the board drag; never leave a stale
            // artwork drag behind (its transaction was committed above).
            ed.drag = Drag::None;
        }
    }

    /// Fresh content comparison against the checkpoint; refreshes the memo when that is safe.
    fn content_dirty(&self) -> bool {
        let ed = &self.editor;
        let dirty = !ed.doc.content_eq(&self.saved);
        if !ed.transaction_open() {
            self.memo.set(Some((ed.rev, dirty)));
        }
        dirty
    }
}

/// Move the app-wide state (clipboard, current tool, recent colours) from the outgoing tab's editor
/// to the incoming one, and reset the incoming editor's held keys (their releases went elsewhere).
fn hand_over(from: &mut Editor, to: &mut Editor) {
    to.set_clipboard(from.take_clipboard());
    if to.tool != from.tool {
        to.set_tool(from.tool);
    }
    to.recent_colors = from.recent_colors.clone();
    to.mods = Mods::default();
    to.space = false;
}

/// Two distinct sessions mutably at once.
fn pair_mut(v: &mut [DocumentSession], a: usize, b: usize) -> (&mut DocumentSession, &mut DocumentSession) {
    assert_ne!(a, b, "pair_mut needs two different sessions");
    if a < b {
        let (lo, hi) = v.split_at_mut(b);
        (&mut lo[a], &mut hi[0])
    } else {
        let (lo, hi) = v.split_at_mut(a);
        (&mut hi[0], &mut lo[b])
    }
}

/// All open documents in tab order, plus which one is active.
///
/// The active document is an `Option` in the API (S2's Start page will allow an empty workspace), but
/// in S1 the workspace is NEVER empty: `new` starts with `Untitled-1` and closing the last tab leaves
/// a fresh `Untitled-N`. So in S1 `active_id()` / `active()` / `active_mut()` are always `Some`.
pub struct Workspace {
    sessions: Vec<DocumentSession>,
    active: Option<SessionId>,
    next_id: u64,
    next_untitled: u32,
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

impl Workspace {
    /// Exactly one pristine `Untitled-1` (boardless, clean), with an initial fit pending (0.45).
    pub fn new() -> Self {
        let first = DocumentSession::untitled(SessionId(1), 1);
        Workspace { sessions: vec![first], active: Some(SessionId(1)), next_id: 2, next_untitled: 2 }
    }

    /// The active tab's id (always `Some` in S1).
    pub fn active_id(&self) -> Option<SessionId> {
        self.active
    }
    /// The active tab (always `Some` in S1).
    pub fn active(&self) -> Option<&DocumentSession> {
        self.active_index().map(|i| &self.sessions[i])
    }
    /// The active tab (always `Some` in S1).
    pub fn active_mut(&mut self) -> Option<&mut DocumentSession> {
        self.active_index().map(|i| &mut self.sessions[i])
    }
    pub fn get(&self, id: SessionId) -> Option<&DocumentSession> {
        self.sessions.iter().find(|s| s.id == id)
    }
    pub fn get_mut(&mut self, id: SessionId) -> Option<&mut DocumentSession> {
        self.sessions.iter_mut().find(|s| s.id == id)
    }
    /// The sessions in tab order.
    pub fn sessions(&self) -> &[DocumentSession] {
        &self.sessions
    }
    /// The tab position of `id`.
    pub fn index_of(&self, id: SessionId) -> Option<usize> {
        self.sessions.iter().position(|s| s.id == id)
    }

    fn active_index(&self) -> Option<usize> {
        self.active.and_then(|id| self.index_of(id))
    }
    fn alloc_id(&mut self) -> SessionId {
        let id = SessionId(self.next_id);
        self.next_id += 1;
        id
    }
    fn alloc_untitled(&mut self) -> DocumentSession {
        let id = self.alloc_id();
        let n = self.next_untitled;
        self.next_untitled += 1;
        DocumentSession::untitled(id, n)
    }
    /// Put `incoming` at position `i` in place of the session there, handing the app-wide state
    /// over from it, and make `incoming` active.
    fn replace_at(&mut self, i: usize, mut incoming: DocumentSession) -> SessionId {
        hand_over(&mut self.sessions[i].editor, &mut incoming.editor);
        let id = incoming.id;
        self.sessions[i] = incoming;
        self.active = Some(id);
        id
    }

    /// A new clean, boardless `Untitled-N` tab, appended and activated. Numbers are never reused.
    pub fn new_untitled(&mut self) -> SessionId {
        let s = self.alloc_untitled();
        let id = s.id;
        self.sessions.push(s);
        self.activate(id);
        id
    }

    /// A successfully loaded file becomes a tab: it REPLACES the active tab only if that one is
    /// pristine (an untouched `Untitled`), else it is appended; either way it is activated, with a
    /// fit pending (0.9). The replacement always gets a NEW id (never the pristine tab's id), so the
    /// host's "active id changed" check also sees an in-place replacement. The loaded `Editor` is
    /// `Editor::new()` + `replace_doc(doc)`; its checkpoint is taken after that.
    pub fn add_loaded(&mut self, doc: Document, path: PathBuf, key: FileKey) -> SessionId {
        let id = self.alloc_id();
        let s = DocumentSession::loaded(id, doc, path, key);
        match self.active_index() {
            Some(i) if self.sessions[i].is_pristine() => self.replace_at(i, s),
            _ => {
                self.sessions.push(s);
                self.activate(id);
                id
            }
        }
    }

    /// The open tab holding this file, if any (`FileKey::same_file`).
    // Frozen S1-A API with no caller in the S1 binary yet (the lifecycle re-keys every tab fresh,
    // `lifecycle::open_tab_of`); its tests use it, and S2/S3 are its planned callers.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn find_file(&self, key: &FileKey) -> Option<SessionId> {
        self.sessions.iter().find(|s| s.key.as_ref().is_some_and(|k| k.same_file(key))).map(|s| s.id)
    }

    /// Make `id` the active tab, handing over clipboard + tool + recent colours and resetting the
    /// incoming editor's held modifiers / Space. Returns `false` only when there is no such tab
    /// (activating the already-active tab is a no-op that returns `true`).
    pub fn activate(&mut self, id: SessionId) -> bool {
        let Some(to) = self.index_of(id) else {
            return false;
        };
        if self.active == Some(id) {
            return true;
        }
        match self.active_index() {
            Some(from) => {
                let (outgoing, incoming) = pair_mut(&mut self.sessions, from, to);
                hand_over(&mut outgoing.editor, &mut incoming.editor);
            }
            None => {
                let ed = &mut self.sessions[to].editor;
                ed.mods = Mods::default();
                ed.space = false;
            }
        }
        self.active = Some(id);
        true
    }

    /// Activate the tab `step` places to the right (negative = left), wrapping around. Returns `true`
    /// when the active tab changed (a single tab never changes).
    pub fn activate_relative(&mut self, step: isize) -> bool {
        let n = self.sessions.len();
        if n == 0 {
            return false;
        }
        let Some(cur) = self.active_index() else {
            // no active tab (not reachable in S1): start from the matching end
            let i = if step < 0 { n - 1 } else { 0 };
            return self.activate(self.sessions[i].id);
        };
        let to = (cur as isize + step).rem_euclid(n as isize) as usize;
        if to == cur {
            return false;
        }
        self.activate(self.sessions[to].id)
    }

    /// Move tab `id` to the INSERTION SLOT `to` of the CURRENT order, exactly what the tab strip's
    /// `tab_drop_index` returns: `0` = before the first tab, `k` = between tab `k-1` and tab `k`,
    /// `len` = after the last (larger values clamp to `len`). It is NOT the final index: with tabs
    /// `[A, B, C]`, dropping A into slot 2 (between B and C) gives `[B, A, C]`. Dropping a tab into
    /// its own slot or the next one (either edge of itself) is a no-op. Only the order changes;
    /// identity, the active tab and every session's state are untouched. Returns `true` when the
    /// order changed.
    pub fn reorder(&mut self, id: SessionId, to: usize) -> bool {
        let Some(from) = self.index_of(id) else {
            return false;
        };
        let slot = to.min(self.sessions.len());
        let dest = if slot > from { slot - 1 } else { slot };
        if dest == from {
            return false;
        }
        let s = self.sessions.remove(from);
        self.sessions.insert(dest, s);
        true
    }

    /// Close tab `id` (the caller has already resolved unsaved changes). Closing the active tab
    /// activates its right neighbour, else its left one, handing the clipboard (and tool, recent
    /// colours) over first. S1: closing the LAST tab leaves a fresh pristine `Untitled-N`, or, when
    /// that last tab already was pristine, does nothing (S2's Start page will change this to an empty
    /// workspace). Returns `true` when a tab was removed or replaced.
    pub fn remove(&mut self, id: SessionId) -> bool {
        let Some(i) = self.index_of(id) else {
            return false;
        };
        if self.sessions.len() == 1 {
            if self.sessions[0].is_pristine() {
                return false;
            }
            let fresh = self.alloc_untitled();
            self.replace_at(0, fresh);
            return true;
        }
        if self.active == Some(id) {
            let next = if i + 1 < self.sessions.len() { i + 1 } else { i - 1 };
            let (outgoing, incoming) = pair_mut(&mut self.sessions, i, next);
            hand_over(&mut outgoing.editor, &mut incoming.editor);
            self.active = Some(incoming.id);
        }
        self.sessions.remove(i);
        true
    }

    /// What the tab strip draws, in tab order. Saved tabs with equal file names get
    /// ` — <parent folder>` appended; the tooltip is the full path or `Not saved yet`.
    pub fn tabs(&self) -> Vec<TabView> {
        let names: Vec<String> = self.sessions.iter().map(DocumentSession::display_name).collect();
        self.sessions
            .iter()
            .zip(&names)
            .map(|(s, name)| {
                let mut label = name.clone();
                if let Some(path) = &s.path {
                    let twins =
                        self.sessions.iter().zip(&names).filter(|(o, n)| o.path.is_some() && *n == name).count();
                    if twins > 1 {
                        if let Some(parent) = path.parent().and_then(|p| p.file_name()) {
                            label = format!("{name} — {}", parent.to_string_lossy());
                        }
                    }
                }
                let tooltip = match &s.path {
                    Some(p) => p.display().to_string(),
                    None => "Not saved yet".into(),
                };
                TabView { id: s.id, label, dirty: s.is_dirty(), tooltip }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use varos_core::editor::ToolKind;
    use varos_core::model::{Anchor, Artboard, Path as VPath};
    use varos_core::EditCommand;

    fn sq(id: u32, base: u32, x: f32, y: f32, s: f32, fill: [f32; 4]) -> VPath {
        let a = |i: u32, p: [f32; 2]| Anchor { id: i, p, hin: None, hout: None, smooth: false };
        VPath::new(
            id,
            vec![a(base, [x, y]), a(base + 1, [x + s, y]), a(base + 2, [x + s, y + s]), a(base + 3, [x, y + s])],
            true,
            Some(fill),
            None,
            1.0,
        )
    }

    /// A saved-looking document: two 100×100 boards and one red square on board A.
    fn doc_with_art() -> Document {
        Document {
            artboards: vec![
                Artboard { x: 0.0, y: 0.0, w: 100.0, h: 100.0, name: "A".into(), ..Artboard::default() },
                Artboard { x: 300.0, y: 0.0, w: 100.0, h: 100.0, name: "B".into(), ..Artboard::default() },
            ],
            paths: vec![sq(10, 100, 20.0, 20.0, 30.0, [1.0, 0.0, 0.0, 1.0])],
            ids: 200,
            ..Document::default()
        }
    }

    fn key(p: &str) -> FileKey {
        FileKey { path: PathBuf::from(p), dev_ino: None }
    }

    fn open(ws: &mut Workspace, p: &str) -> SessionId {
        ws.add_loaded(doc_with_art(), PathBuf::from(p), key(p))
    }

    /// Draw a square on the active tab with the Rect tool (a real, committed edit).
    fn draw(ed: &mut Editor, at: [f32; 2]) {
        let prev = ed.tool;
        ed.set_tool(ToolKind::Rect);
        ed.pointer_down(at);
        ed.pointer_move([at[0] + 40.0, at[1] + 40.0]);
        ed.pointer_up();
        ed.set_tool(prev);
    }

    fn labels(ws: &Workspace) -> Vec<String> {
        ws.tabs().into_iter().map(|t| t.label).collect()
    }

    #[test]
    fn starts_with_one_pristine_untitled_1() {
        let ws = Workspace::new();
        assert_eq!(ws.sessions().len(), 1);
        let s = ws.active().unwrap();
        assert_eq!(s.id, ws.active_id().unwrap());
        assert_eq!(s.display_name(), "Untitled-1");
        assert!(s.is_pristine() && !s.is_dirty() && !s.is_dirty_exact());
        assert!(s.path.is_none() && s.key.is_none());
        assert!(s.editor.doc.artboards.is_empty(), "a new document is a boardless free canvas");
        assert_eq!(s.fit_pending, Some(0.45));
        assert_eq!(
            ws.tabs(),
            vec![TabView { id: s.id, label: "Untitled-1".into(), dirty: false, tooltip: "Not saved yet".into() }]
        );
    }

    #[test]
    fn untitled_numbers_increase_and_are_never_reused() {
        let mut ws = Workspace::new();
        let first = ws.active_id().unwrap();
        draw(&mut ws.active_mut().unwrap().editor, [0.0, 0.0]); // tab 1 has art
        let b = ws.new_untitled();
        assert_eq!(ws.active_id().unwrap(), b, "New activates the new tab");
        assert_eq!(ws.active().unwrap().display_name(), "Untitled-2");
        assert!(
            ws.active().unwrap().editor.doc.paths.is_empty() && ws.active().unwrap().editor.doc.artboards.is_empty()
        );
        assert_eq!(ws.get(first).unwrap().editor.doc.paths.len(), 1, "the first tab keeps its art");
        assert!(ws.remove(b));
        let c = ws.new_untitled();
        assert_eq!(ws.get(c).unwrap().display_name(), "Untitled-3", "a closed number is never reused");
        assert!(ws.remove(c));
        assert!(ws.remove(first), "the last (dirty) tab closes to a fresh Untitled");
        assert_eq!(labels(&ws), ["Untitled-4"]);
        assert!(ws.active().unwrap().is_pristine());
        let ids: Vec<u64> = [first, b, c, ws.active_id().unwrap()].iter().map(|s| s.0).collect();
        assert!(ids.windows(2).all(|w| w[0] < w[1]), "ids are never reused either: {ids:?}");
    }

    #[test]
    fn activate_hands_over_clipboard_and_tool() {
        let mut ws = Workspace::new();
        let a = ws.active_id().unwrap();
        draw(&mut ws.active_mut().unwrap().editor, [0.0, 0.0]);
        {
            let ed = &mut ws.active_mut().unwrap().editor;
            ed.objsel = ed.doc.paths.iter().map(|p| p.id).collect();
            ed.execute(EditCommand::Copy);
            assert_eq!(ed.clipboard().len(), 1);
            ed.set_tool(ToolKind::Pen);
            ed.recent_colors = vec![[0.1, 0.2, 0.3, 1.0]];
            ed.cur_fill = Some([0.0, 0.0, 1.0, 1.0]);
            ed.mods.shift = true;
        }
        let b = ws.new_untitled(); // activation #1
        {
            let s = ws.get(b).unwrap();
            assert_eq!(s.editor.clipboard().len(), 1, "the clipboard followed the user to the new tab");
            assert!(s.editor.tool == ToolKind::Pen, "…and the current tool");
            assert_eq!(s.editor.recent_colors, vec![[0.1, 0.2, 0.3, 1.0]], "…and the recent colours");
            assert_ne!(s.editor.cur_fill, Some([0.0, 0.0, 1.0, 1.0]), "the current fill is per tab");
            assert!(ws.get(a).unwrap().editor.clipboard().is_empty(), "exactly one clipboard exists");
        }
        // back to A: a held key on A (stale) is reset on the way in; B's Space too
        ws.get_mut(a).unwrap().editor.space = true;
        ws.get_mut(b).unwrap().editor.set_tool(ToolKind::Direct);
        assert!(ws.activate(a));
        let s = ws.get(a).unwrap();
        assert_eq!(s.editor.clipboard().len(), 1);
        assert!(s.editor.tool == ToolKind::Direct);
        assert!(!s.editor.mods.shift && !s.editor.space, "incoming held keys are reset");
        assert!(ws.activate(a), "activating the active tab is a harmless no-op");
        assert!(!ws.activate(SessionId(999)), "an unknown id does nothing");
        assert_eq!(ws.active_id().unwrap(), a);
        // wrap-around switching
        assert!(ws.activate_relative(1));
        assert_eq!(ws.active_id().unwrap(), b);
        assert!(ws.activate_relative(1));
        assert_eq!(ws.active_id().unwrap(), a, "Ctrl+Tab wraps around");
        assert!(ws.activate_relative(-1));
        assert_eq!(ws.active_id().unwrap(), b, "Ctrl+Shift+Tab wraps around too");
        assert_eq!(ws.active().unwrap().editor.clipboard().len(), 1, "still one clipboard after the round trip");
    }

    #[test]
    fn remove_picks_right_then_left_and_never_leaves_zero_tabs() {
        let mut ws = Workspace::new();
        let a = ws.active_id().unwrap();
        let b = ws.new_untitled();
        let c = ws.new_untitled();
        {
            let ed = &mut ws.get_mut(c).unwrap().editor;
            draw(ed, [0.0, 0.0]);
            ed.objsel = ed.doc.paths.iter().map(|p| p.id).collect();
            ed.execute(EditCommand::Copy);
        }
        // close the active MIDDLE tab → its right neighbour
        assert!(ws.activate(b));
        assert!(ws.remove(b));
        assert_eq!(ws.active_id().unwrap(), c, "right neighbour first");
        assert_eq!(
            ws.active().unwrap().editor.clipboard().len(),
            1,
            "the clipboard survives closing the tab holding it"
        );
        // close the active LAST tab → its left neighbour
        assert!(ws.remove(c));
        assert_eq!(ws.active_id().unwrap(), a, "no right neighbour → the left one");
        assert_eq!(ws.active().unwrap().editor.clipboard().len(), 1, "…still the one clipboard");
        // closing an INACTIVE tab keeps the active one
        let d = ws.new_untitled();
        assert!(ws.activate(a));
        assert!(ws.remove(d));
        assert_eq!(ws.active_id().unwrap(), a);
        // the last tab, already pristine → no-op
        assert!(ws.active().unwrap().is_pristine());
        assert!(!ws.remove(a), "closing the only, pristine tab does nothing");
        assert_eq!(ws.active_id().unwrap(), a);
        assert!(!ws.remove(SessionId(999)));
        // the last tab, not pristine → a fresh Untitled-N takes its place, clipboard kept
        draw(&mut ws.active_mut().unwrap().editor, [0.0, 0.0]);
        assert!(ws.remove(a));
        assert_eq!(ws.sessions().len(), 1, "never zero tabs");
        assert_ne!(ws.active_id().unwrap(), a);
        assert!(ws.active().unwrap().is_pristine());
        assert_eq!(ws.active().unwrap().display_name(), "Untitled-5");
        assert_eq!(ws.active().unwrap().editor.clipboard().len(), 1);
    }

    #[test]
    fn reorder_keeps_identity_and_state() {
        let mut ws = Workspace::new();
        let a = ws.active_id().unwrap();
        draw(&mut ws.active_mut().unwrap().editor, [0.0, 0.0]);
        let b = ws.new_untitled();
        let c = ws.new_untitled();
        let order = |ws: &Workspace| ws.sessions().iter().map(|s| s.id).collect::<Vec<_>>();
        assert_eq!(order(&ws), [a, b, c]);
        let rev_a = ws.get(a).unwrap().editor.rev;
        // drag C before A (slot 0)
        assert!(ws.reorder(c, 0));
        assert_eq!(order(&ws), [c, a, b]);
        assert_eq!(ws.active_id().unwrap(), c, "the active tab stays active");
        // drag C to the end (slot len)
        assert!(ws.reorder(c, 3));
        assert_eq!(order(&ws), [a, b, c]);
        // dropping a tab next to itself changes nothing
        assert!(!ws.reorder(b, 1) && !ws.reorder(b, 2));
        // dragging a tab ONE place to the right = dropping it into slot index+2 (between the next two
        // chips): A (index 0) into slot 2 lands at index 1 — no off-by-one
        assert!(ws.reorder(a, 2));
        assert_eq!(order(&ws), [b, a, c]);
        // …and one place to the left = slot index-1: A (index 1) into slot 0
        assert!(ws.reorder(a, 0));
        assert_eq!(order(&ws), [a, b, c]);
        assert!(ws.reorder(a, 2));
        assert_eq!(order(&ws), [b, a, c]);
        // its own slot (1) and the next one (2) are both no-ops
        assert!(!ws.reorder(a, 1) && !ws.reorder(a, 2));
        assert_eq!(order(&ws), [b, a, c]);
        assert!(ws.reorder(a, 99), "an out-of-range slot clamps to the end");
        assert_eq!(order(&ws), [b, c, a]);
        assert!(!ws.reorder(SessionId(999), 0));
        let sa = ws.get(a).unwrap();
        assert_eq!((sa.editor.rev, sa.editor.doc.paths.len()), (rev_a, 1), "A's document and history are untouched");
        assert_eq!(labels(&ws), ["Untitled-2", "Untitled-3", "Untitled-1"]);
        assert_eq!(ws.active_id().unwrap(), c);
    }

    #[test]
    fn add_loaded_reuses_only_a_pristine_active_tab() {
        let mut ws = Workspace::new();
        let first = ws.active_id().unwrap();
        let x = open(&mut ws, "/docs/x.vrs");
        assert_eq!(ws.sessions().len(), 1, "the pristine Untitled-1 was replaced");
        assert_ne!(x, first, "…by a session with a NEW id");
        assert_eq!(ws.active_id().unwrap(), x);
        let s = ws.active().unwrap();
        assert_eq!(s.display_name(), "x.vrs");
        assert_eq!(s.fit_pending, Some(0.9));
        assert!(!s.is_dirty_exact() && !s.is_pristine(), "a loaded file is clean but not pristine");
        assert_eq!(s.editor.doc.paths.len(), 1);
        // a loaded file is not pristine → the next open appends
        let y = open(&mut ws, "/docs/y.vrs");
        assert_eq!(ws.sessions().len(), 2);
        assert_eq!(ws.active_id().unwrap(), y);
        // a dirty untitled is not pristine either
        let u = ws.new_untitled();
        draw(&mut ws.active_mut().unwrap().editor, [0.0, 0.0]);
        let z = open(&mut ws, "/docs/z.vrs");
        assert_eq!(ws.sessions().len(), 4, "a dirty Untitled survives an open");
        assert!(ws.get(u).is_some());
        // a fresh pristine untitled that is NOT active is kept too (only the ACTIVE one is reused)
        let p = ws.new_untitled();
        assert!(ws.activate(z));
        let w = open(&mut ws, "/docs/w.vrs");
        assert!(ws.get(p).is_some() && ws.get(z).is_some() && ws.get(w).is_some());
        // …but when it IS active it is replaced in place (same tab position)
        assert!(ws.activate(p));
        let at = ws.index_of(p).unwrap();
        let v = open(&mut ws, "/docs/v.vrs");
        assert!(ws.get(p).is_none());
        assert_eq!(ws.index_of(v), Some(at));
        // the checkpoint is the post-load document: clean until edited
        draw(&mut ws.active_mut().unwrap().editor, [200.0, 200.0]);
        assert!(ws.active().unwrap().is_dirty_exact());
    }

    #[test]
    fn find_file_matches_same_file_key() {
        let mut ws = Workspace::new();
        let x = ws.add_loaded(
            doc_with_art(),
            PathBuf::from("/docs/x.vrs"),
            FileKey { path: PathBuf::from("/docs/x.vrs"), dev_ino: Some((1, 42)) },
        );
        let alias = FileKey { path: PathBuf::from("/link/X.VRS"), dev_ino: Some((1, 42)) };
        assert_eq!(ws.find_file(&alias), Some(x), "same device + inode = the same file under another name");
        // ⌘S replaced the file (temp + rename ⇒ a new inode): the same path is still the same file
        let after_save = FileKey { path: PathBuf::from("/docs/x.vrs"), dev_ino: Some((1, 43)) };
        assert_eq!(ws.find_file(&after_save), Some(x), "a new inode at the same path is the same file");
        assert_ne!(after_save, ws.get(x).unwrap().key.clone().unwrap(), "(which is why `==` must not be used)");
        assert_eq!(ws.find_file(&key("/docs/x.vrs")), Some(x), "no inode on one side → compare paths");
        let elsewhere = FileKey { path: PathBuf::from("/docs/y.vrs"), dev_ino: Some((1, 99)) };
        assert_eq!(ws.find_file(&elsewhere), None, "another path with another inode is another file");
        assert_eq!(ws.find_file(&key("/docs/y.vrs")), None);
        assert!(key("/a").same_file(&key("/a")) && !key("/a").same_file(&key("/b")));
        // B refreshes the key after a save
        ws.get_mut(x).unwrap().set_key(after_save.clone());
        assert_eq!(ws.get(x).unwrap().key, Some(after_save));
    }

    /// Review F1: every save swaps the inode (`write_atomic` = temp file + rename), so a key taken
    /// before a save must still match one taken after it. Real files in a temp dir; unix only
    /// (device/inode is how `DocStore::key` identifies aliases there).
    #[cfg(unix)]
    #[test]
    fn same_file_survives_an_atomic_save() {
        use std::os::unix::fs::MetadataExt;
        let dir = std::env::temp_dir().join(format!("varos-s1a-filekey-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.vrs");
        let key_of = |p: &std::path::Path| {
            let canon = std::fs::canonicalize(p).unwrap();
            let m = std::fs::metadata(&canon).unwrap();
            FileKey { path: canon, dev_ino: Some((m.dev(), m.ino())) }
        };
        std::fs::write(&path, b"one").unwrap();
        let before = key_of(&path);
        // what `write_atomic` does: write a sibling temp file, then rename it over the target
        let tmp = path.with_extension("vrs.tmp");
        std::fs::write(&tmp, b"two").unwrap();
        std::fs::rename(&tmp, &path).unwrap();
        let after = key_of(&path);
        assert_ne!(before.dev_ino, after.dev_ino, "the atomic save really replaced the inode");
        assert!(before.same_file(&after) && after.same_file(&before), "…but it is the same file");
        // a symlink alias of the live file matches by inode even though the path differs
        let link = dir.join("alias.vrs");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        let alias = FileKey { path: link.clone(), dev_ino: std::fs::metadata(&link).ok().map(|m| (m.dev(), m.ino())) };
        assert!(alias.same_file(&after), "a symlink to the file is the same file");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tabs_disambiguate_equal_file_names() {
        let mut ws = Workspace::new();
        open(&mut ws, "/work/client/Logo.vrs");
        open(&mut ws, "/work/archive/Logo.vrs");
        open(&mut ws, "/work/archive/Card.vrs");
        ws.new_untitled();
        let tabs = ws.tabs();
        let got: Vec<(&str, &str)> = tabs.iter().map(|t| (t.label.as_str(), t.tooltip.as_str())).collect();
        assert_eq!(
            got,
            [
                ("Logo.vrs — client", "/work/client/Logo.vrs"),
                ("Logo.vrs — archive", "/work/archive/Logo.vrs"),
                ("Card.vrs", "/work/archive/Card.vrs"),
                ("Untitled-2", "Not saved yet"),
            ]
        );
    }

    #[test]
    fn dirty_follows_edit_undo_and_mark_saved() {
        let mut ws = Workspace::new();
        let id = ws.active_id().unwrap();
        draw(&mut ws.active_mut().unwrap().editor, [0.0, 0.0]);
        let s = ws.active().unwrap();
        assert!(s.is_dirty() && s.is_dirty_exact() && !s.is_pristine());
        assert!(ws.tabs()[0].dirty);
        ws.active_mut().unwrap().editor.execute(EditCommand::Undo);
        assert!(
            !ws.active().unwrap().is_dirty() && !ws.active().unwrap().is_dirty_exact(),
            "undo back to the checkpoint is clean"
        );
        ws.active_mut().unwrap().editor.execute(EditCommand::Redo);
        assert!(ws.active().unwrap().is_dirty());
        // save
        ws.active_mut().unwrap().mark_saved(PathBuf::from("/docs/a.vrs"), key("/docs/a.vrs"));
        let s = ws.active().unwrap();
        assert!(!s.is_dirty() && !s.is_dirty_exact(), "saved = clean");
        assert_eq!(s.display_name(), "a.vrs");
        assert_eq!(s.untitled, None);
        assert_eq!(s.key, Some(key("/docs/a.vrs")));
        assert!(!s.is_pristine(), "a saved file is never pristine");
        assert_eq!(ws.find_file(&key("/docs/a.vrs")), Some(id));
        // undo PAST the save point is dirty; redo back is clean
        ws.active_mut().unwrap().editor.execute(EditCommand::Undo);
        assert!(ws.active().unwrap().is_dirty());
        ws.active_mut().unwrap().editor.execute(EditCommand::Redo);
        assert!(!ws.active().unwrap().is_dirty());
        // an in-flight gesture (transaction open + changed) shows dirty before it commits
        {
            let ed = &mut ws.active_mut().unwrap().editor;
            ed.set_tool(ToolKind::Object);
            ed.objsel = ed.doc.paths.iter().map(|p| p.id).collect();
            let c = ed.doc.paths[0].anchors[0].p;
            ed.pointer_down([c[0] + 20.0, c[1] + 20.0]);
            ed.pointer_move([c[0] + 60.0, c[1] + 60.0]);
            assert!(ed.transaction_open() && ed.dirty, "a drag is in flight");
        }
        assert!(
            ws.active().unwrap().is_dirty() && ws.active().unwrap().is_dirty_exact(),
            "an in-flight change shows dirty"
        );
        assert!(!ws.active().unwrap().is_pristine());
        ws.active_mut().unwrap().editor.pointer_up();
        assert!(ws.active().unwrap().is_dirty(), "…and stays dirty once committed");
        // a cancelled picker preview is not an edit (the memo is never written mid-transaction)
        ws.active_mut().unwrap().mark_saved(PathBuf::from("/docs/a.vrs"), key("/docs/a.vrs"));
        {
            let ed = &mut ws.active_mut().unwrap().editor;
            ed.execute(EditCommand::PickerBegin);
            ed.execute(EditCommand::PickerLivePaint {
                target: varos_core::editor::PaintTarget::Fill,
                color: [0.0, 1.0, 0.0, 1.0],
            });
        }
        assert!(ws.active().unwrap().is_dirty(), "the live preview shows dirty while the picker is open");
        ws.active_mut().unwrap().editor.execute(EditCommand::PickerCancel);
        assert!(!ws.active().unwrap().is_dirty() && !ws.active().unwrap().is_dirty_exact(), "Cancel is clean again");
    }

    #[test]
    fn view_only_actions_never_dirty() {
        let mut ws = Workspace::new();
        open(&mut ws, "/docs/a.vrs");
        let clean = |ws: &Workspace, what: &str| {
            let s = ws.active().unwrap();
            assert!(!s.is_dirty(), "{what} made the tab dirty (memo)");
            assert!(!s.is_dirty_exact(), "{what} made the tab dirty (exact)");
            assert!(!ws.tabs()[0].dirty, "{what} put a dot on the tab");
        };
        {
            let s = ws.active_mut().unwrap();
            s.view.pan = [123.0, -45.0];
            s.view.zoom = 3.5;
            s.view = View::fit(0.0, 0.0, 100.0, 100.0, 800.0, 600.0, 0.9);
        }
        clean(&ws, "pan / zoom / fit");
        let ed = &mut ws.active_mut().unwrap().editor;
        ed.execute(EditCommand::SetActiveArtboard(1));
        assert_eq!(ed.doc.active, 1);
        ed.toggle_rulers_visibility();
        ed.toggle_guides_visibility();
        let mut cfg = ed.doc.snap;
        cfg.grid = !cfg.grid;
        ed.execute(EditCommand::SetSnapConfig(cfg));
        ed.execute(EditCommand::ToggleSnapping);
        ed.execute(EditCommand::ToggleSmartGuides);
        ed.execute(EditCommand::ToggleGuidesLocked);
        ed.execute(EditCommand::SetRulerOrigin([7.0, 9.0]));
        let rev = ed.rev;
        ed.execute(EditCommand::CycleUnits);
        ed.execute(EditCommand::SetMoveArtWithArtboard(false));
        assert_eq!(ed.rev, rev + 2, "units and move-art are still history steps");
        ed.execute(EditCommand::RenameArtboard { index: 0, name: "A".into() });
        assert_eq!(ed.rev, rev + 3, "an unchanged board name is still a (no-op) commit");
        // select / deselect
        ed.objsel = ed.doc.paths.iter().map(|p| p.id).collect();
        ed.objsel.clear();
        clean(&ws, "board / ruler / guide / snap / units / move-art / rename / select");
        // Artboard tool: click a board's resize handle without moving
        let ed = &mut ws.active_mut().unwrap().editor;
        ed.set_tool(ToolKind::Artboard);
        ed.ppu = 1.0;
        let rev = ed.rev;
        let ab = ed.doc.active_artboard().cloned().expect("board B is active");
        let corner = [ab.x + ab.w, ab.y + ab.h];
        ed.pointer_down(corner);
        ed.pointer_move(corner);
        ed.pointer_up();
        assert_eq!(ed.doc.active_artboard(), Some(&ab), "the board did not change");
        assert_eq!(ed.rev, rev + 1, "today's handle click is a no-op commit (bumps rev)…");
        clean(&ws, "a board-handle click with no movement");
        // …and clicking a board body just activates it
        let ed = &mut ws.active_mut().unwrap().editor;
        ed.pointer_down([10.0, 90.0]);
        ed.pointer_up();
        assert_eq!(ed.doc.active, 0);
        clean(&ws, "a board-body click");
        // a real edit is still seen
        let ed = &mut ws.active_mut().unwrap().editor;
        ed.execute(EditCommand::RenameArtboard { index: 0, name: "Cover".into() });
        assert!(ws.active().unwrap().is_dirty(), "a real rename is an edit");
    }

    #[test]
    fn settle_finishes_an_in_flight_drag() {
        let mut ws = Workspace::new();
        open(&mut ws, "/docs/a.vrs");
        let s = ws.active_mut().unwrap();
        s.editor.set_tool(ToolKind::Object);
        s.editor.objsel.insert(10);
        let rev = s.editor.rev;
        s.editor.pointer_down([30.0, 30.0]);
        s.editor.pointer_move([70.0, 50.0]);
        assert!(!matches!(s.editor.drag, Drag::None) && s.editor.transaction_open());
        s.settle();
        assert!(matches!(s.editor.drag, Drag::None), "the drag is over");
        assert!(!s.editor.transaction_open(), "its transaction is closed");
        assert_eq!(s.editor.rev, rev + 1, "the gesture committed as one step");
        assert!(s.is_dirty_exact(), "…and it is an edit");
        s.editor.execute(EditCommand::Undo);
        assert!(!s.is_dirty_exact(), "which undoes as one step");
        // an Artboard-tool drag settles too
        s.editor.set_tool(ToolKind::Artboard);
        s.editor.ppu = 1.0;
        s.editor.pointer_down([50.0, 50.0]);
        s.editor.pointer_move([60.0, 60.0]);
        assert!(!matches!(s.editor.ab_drag, AbDrag::None));
        s.settle();
        assert!(matches!(s.editor.ab_drag, AbDrag::None) && !s.editor.transaction_open());
        assert!(s.is_dirty_exact(), "the board moved");
        // a Pen click that added an anchor (transaction open, no drag yet) settles too
        s.editor.set_tool(ToolKind::Pen);
        s.editor.objsel.insert(10);
        let rev = s.editor.rev;
        let n = s.editor.doc.paths.iter().find(|p| p.id == 10).unwrap().anchors.len();
        let sqr = s.editor.doc.paths.iter().find(|p| p.id == 10).unwrap();
        let (a0, a1) = (sqr.anchors[0].p, sqr.anchors[1].p);
        let mid = [(a0[0] + a1[0]) * 0.5, (a0[1] + a1[1]) * 0.5]; // on the square's first segment
        s.editor.pointer_move(mid);
        s.editor.pointer_down(mid);
        assert!(s.editor.transaction_open() && matches!(s.editor.drag, Drag::None), "open, no drag");
        assert_eq!(s.editor.doc.paths.iter().find(|p| p.id == 10).unwrap().anchors.len(), n + 1);
        s.settle();
        assert!(!s.editor.transaction_open(), "the Pen click's transaction is committed");
        assert_eq!(s.editor.rev, rev + 1, "as one undo step");
        // nothing in flight → settle is a no-op
        let rev = s.editor.rev;
        s.settle();
        assert_eq!(s.editor.rev, rev);
    }
}
