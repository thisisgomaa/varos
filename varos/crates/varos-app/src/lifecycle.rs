//! The document-lifecycle coordinator (DFS S1 §3.4): New / Open / Save / Save As / Close / Quit and
//! tab switching, run over the `Workspace` through two PORTS — `Dialogs` (every question the user is
//! asked) and `DocStore` (every file read/write). The host plugs in the real rfd + disk ports
//! (`file_ports.rs`); tests plug in scripted fakes. No rfd, fs or egui in this file.
//!
//! API frozen by S1-A; the rule bodies of `Lifecycle::run` are S1-B's.
// S1-A: nothing calls this module until S1-D wires the host; S1-D removes this allow. (Plain `allow`,
// not `cfg_attr(not(test), ..)`: test builds would otherwise fail `clippy --all-targets -D warnings`.)
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use varos_core::model::Document;

use crate::app_command::{AppCommand, SessionId};
use crate::workspace::{FileKey, Workspace};

/// The answer to “Save changes to “name”?”. Escape / closing the prompt = `Cancel`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveDecision {
    Save,
    DontSave,
    Cancel,
}

/// The answer to “Couldn't save “name”.”.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveFailChoice {
    TryAgain,
    SaveAs,
    Cancel,
}

/// Every question the lifecycle asks the user. Blocking; the host implements it with native
/// dialogs, tests with a scripted queue.
pub trait Dialogs {
    /// The Open dialog (multi-select). Empty = cancelled.
    fn pick_open(&mut self) -> Vec<PathBuf>;
    /// The Save dialog, pre-filled with `suggested` in `dir`. `None` = cancelled.
    fn pick_save(&mut self, suggested: &str, dir: Option<&Path>) -> Option<PathBuf>;
    /// “Save changes to “name”?” — `progress` = `Some((i, n))` during Quit (“Document i of n”).
    fn ask_save_changes(&mut self, name: &str, progress: Option<(usize, usize)>) -> SaveDecision;
    /// “Couldn't save “name”.” with the reason.
    fn save_failed(&mut self, name: &str, reason: &str) -> SaveFailChoice;
    /// “Couldn't open “name”.” with the reason.
    fn open_failed(&mut self, name: &str, reason: &str);
    /// “Replace “name”?” — asked only when WE changed the chosen name (appended `.vrs`) and that
    /// file exists; the Save dialog already asked about the name the user typed.
    fn confirm_replace(&mut self, name: &str) -> bool;
    /// A plain notice (e.g. “… is open in another tab…”).
    fn notice(&mut self, title: &str, body: &str);
}

/// Every file operation the lifecycle performs.
pub trait DocStore {
    fn load(&mut self, path: &Path) -> Result<Document, String>;
    fn save(&mut self, doc: &Document, path: &Path) -> Result<(), String>;
    /// The file's identity: absolute + canonical path (the parent canonicalised for a file that does
    /// not exist yet), plus device/inode on unix.
    fn key(&self, path: &Path) -> FileKey;
    fn exists(&self, path: &Path) -> bool;
}

/// What the host must do after a command.
#[derive(Debug, Default, PartialEq)]
pub struct Effect {
    /// Every tab was resolved by a Quit — save the window state and exit.
    pub exit: bool,
}

/// One lifecycle command run over the workspace and the ports.
///
/// The host settles first (`Ui::settle`, then the active `DocumentSession::settle`), so no command
/// runs mid-gesture or with a colour-picker preview open. The lifecycle does not settle by itself: it
/// cannot reach the Ui, and settling a session before the picker is cancelled would COMMIT the
/// preview.
pub struct Lifecycle<'a> {
    pub ws: &'a mut Workspace,
    pub dialogs: &'a mut dyn Dialogs,
    pub store: &'a mut dyn DocStore,
}

impl Lifecycle<'_> {
    /// Run one command. `AppCommand::Window(_)` is ignored here (host-owned).
    pub fn run(&mut self, cmd: AppCommand) -> Effect {
        match cmd {
            AppCommand::NewDocument => {
                self.ws.new_untitled();
            }
            AppCommand::OpenDialog => {
                let picked = self.dialogs.pick_open();
                self.open_paths(picked);
            }
            AppCommand::OpenPaths(paths, _origin) => self.open_paths(paths),
            AppCommand::Save(id) => {
                self.save(id, false);
            }
            AppCommand::SaveAs(id) => {
                self.save(id, true);
            }
            AppCommand::CloseDocument(id) => self.close(id),
            AppCommand::Quit => return Effect { exit: self.quit() },
            AppCommand::ActivateDocument(id) => {
                self.ws.activate(id);
            }
            AppCommand::ActivateNext => {
                self.ws.activate_relative(1);
            }
            AppCommand::ActivatePrevious => {
                self.ws.activate_relative(-1);
            }
            AppCommand::ReorderDocument(id, slot) => {
                self.ws.reorder(id, slot);
            }
            AppCommand::Window(_) => {}
        }
        Effect::default()
    }

    /// Open: each path gets a FRESH key. A file that is already open (same path, or same
    /// device/inode = an alias) focuses its tab and is never reloaded, even when that tab is dirty.
    /// Otherwise the file is read into a candidate: success → `add_loaded` (which reuses a pristine
    /// active `Untitled`); failure → “Couldn't open …”, and no tab, path, selection or history changes.
    fn open_paths(&mut self, paths: Vec<PathBuf>) {
        for path in paths {
            let key = self.store.key(&path);
            if let Some(id) = self.open_tab_of(&key, None) {
                self.ws.activate(id);
                continue;
            }
            match self.store.load(&path) {
                // The tab takes the store's normalised path (absolute, symlinks resolved), so a later
                // Save replaces the real file, never a symlink standing in for it.
                Ok(doc) => {
                    let at = key.path.clone();
                    self.ws.add_loaded(doc, at, key);
                }
                Err(reason) => self.dialogs.open_failed(&file_name(&path), &reason),
            }
        }
    }

    /// The open tab (other than `except`) holding the file `key` names. Each tab's key is computed
    /// FRESH from its path (one stat per tab): a stored key's inode goes stale when another app
    /// rewrites the file atomically, and an alias opened after that would otherwise slip past.
    fn open_tab_of(&self, key: &FileKey, except: Option<SessionId>) -> Option<SessionId> {
        let store = &*self.store;
        self.ws
            .sessions()
            .iter()
            .filter(|s| Some(s.id) != except)
            .find(|s| s.path.as_deref().is_some_and(|p| store.key(p).same_file(key)))
            .map(|s| s.id)
    }

    /// Save tab `id` (with `save_as`, or when it has no `.vrs` path yet: Save As). `true` only when
    /// the document is on disk and the tab took the new checkpoint. A cancelled dialog, or a failure
    /// the user did not resolve, leaves the path, the name and the checkpoint exactly as they were.
    fn save(&mut self, id: SessionId, save_as: bool) -> bool {
        let Some(s) = self.ws.get(id) else {
            return false;
        };
        // Save writes native `.vrs` only: a `.pdf`-opened or never-saved document goes through Save As.
        let mut target = match (&s.path, save_as) {
            (Some(p), false) if is_vrs(p) => Some(p.clone()),
            _ => None,
        };
        loop {
            let dest = match target.take() {
                Some(p) => p,
                None => match self.choose_save_path(id) {
                    Some(p) => p,
                    None => return false, // Save As cancelled
                },
            };
            match self.write(id, &dest) {
                Ok(()) => return true,
                Err(reason) => {
                    let name = self.name_of(id);
                    match self.dialogs.save_failed(&name, &reason) {
                        SaveFailChoice::TryAgain => target = Some(dest),
                        SaveFailChoice::SaveAs => {} // `target` stays None → the Save dialog again
                        SaveFailChoice::Cancel => return false,
                    }
                }
            }
        }
    }

    /// The Save As dialog for tab `id`. It suggests `<name>.vrs` in the file's folder. When the
    /// chosen name lacks `.vrs` it is appended (no part of the name is dropped), and replacing an
    /// existing file under that appended name is confirmed first. A file another open tab holds is
    /// refused (two writers for one file). A refusal returns to the dialog; `None` = cancelled.
    fn choose_save_path(&mut self, id: SessionId) -> Option<PathBuf> {
        let s = self.ws.get(id)?;
        let suggested = format!("{}.vrs", stem_of(s.path.as_deref(), &s.display_name()));
        let dir = s.path.as_deref().and_then(Path::parent).map(Path::to_path_buf);
        loop {
            let picked = self.dialogs.pick_save(&suggested, dir.as_deref())?;
            let (dest, appended) = if is_vrs(&picked) { (picked, false) } else { (with_vrs(picked), true) };
            // The Save dialog already asked about the name the user typed; only a name WE changed
            // needs its own “Replace?”.
            if appended && self.store.exists(&dest) && !self.dialogs.confirm_replace(&file_name(&dest)) {
                continue;
            }
            let key = self.store.key(&dest);
            if let Some(other) = self.open_tab_of(&key, Some(id)) {
                let other = self.name_of(other);
                self.dialogs.notice(
                    &format!("“{other}” is open in another tab."),
                    "Saving here would replace that document. Choose another name, or close that tab first.",
                );
                continue;
            }
            return Some(dest);
        }
    }

    /// Write tab `id` to `dest`. Only after the store succeeded does the tab take the path, the name
    /// and the new checkpoint, with a key recomputed AFTER the write (an atomic save replaces the
    /// file's inode).
    fn write(&mut self, id: SessionId, dest: &Path) -> Result<(), String> {
        let s = self.ws.get(id).ok_or_else(|| "The document is no longer open.".to_string())?;
        self.store.save(&s.editor.doc, dest)?;
        let key = self.store.key(dest);
        if let Some(s) = self.ws.get_mut(id) {
            s.mark_saved(dest.to_path_buf(), key);
        }
        Ok(())
    }

    /// Close tab `id`. Clean → closed at once. Dirty → ask about THAT tab by name without activating
    /// it: Save (possibly through Save As) closes only when the write completed, Don't Save closes,
    /// Cancel keeps it.
    fn close(&mut self, id: SessionId) {
        let Some(s) = self.ws.get(id) else {
            return;
        };
        if s.is_dirty_exact() && !self.resolve(id, None) {
            return;
        }
        self.ws.remove(id);
    }

    /// The quit transaction over the dirty tabs, in tab order, each activated behind its prompt
    /// (“Document i of n”). Save must complete or the quit aborts; Don't Save counts only for this
    /// run; Cancel aborts and keeps every tab (earlier saves stay saved). `true` = every tab was
    /// resolved and the app may exit. Nothing is removed either way: the host exits.
    fn quit(&mut self) -> bool {
        let dirty: Vec<SessionId> = self.ws.sessions().iter().filter(|s| s.is_dirty_exact()).map(|s| s.id).collect();
        let n = dirty.len();
        for (i, id) in dirty.into_iter().enumerate() {
            self.ws.activate(id);
            if !self.resolve(id, Some((i + 1, n))) {
                return false;
            }
        }
        true
    }

    /// Ask “Save changes to …?” about tab `id` and carry the answer out. `true` = resolved (saved,
    /// or Don't Save); `false` = Cancel, or a Save that did not complete.
    fn resolve(&mut self, id: SessionId, progress: Option<(usize, usize)>) -> bool {
        let name = self.name_of(id);
        match self.dialogs.ask_save_changes(&name, progress) {
            SaveDecision::Save => self.save(id, false),
            SaveDecision::DontSave => true,
            SaveDecision::Cancel => false,
        }
    }

    fn name_of(&self, id: SessionId) -> String {
        self.ws.get(id).map(|s| s.display_name()).unwrap_or_default()
    }
}

/// `true` when the path ends in `.vrs` (any case).
fn is_vrs(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("vrs"))
}

/// The chosen name + `.vrs`, keeping every part of it (`logo.pdf` → `logo.pdf.vrs`).
fn with_vrs(p: PathBuf) -> PathBuf {
    let mut s = p.into_os_string();
    s.push(".vrs");
    PathBuf::from(s)
}

/// The Save As suggestion's stem: the file's stem (`Logo.pdf` → `Logo`), else the tab name.
fn stem_of(path: Option<&Path>, display_name: &str) -> String {
    path.and_then(Path::file_stem).map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| display_name.into())
}

/// The name a prompt shows for a path (its file name, else the whole path).
fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

#[cfg(test)]
mod tests {
    //! Headless lifecycle tests: a scripted `FakeDialogs` (an answer queue that records every
    //! prompt) and an in-memory `FakeStore` (per-path load/save failure, aliases, an inode that
    //! changes on every save). No rfd, no file system, no GPU, no EventLoop.
    use super::*;
    use crate::app_command::{OpenOrigin, WindowCmd};
    use crate::workspace::DocumentSession;
    use std::collections::{HashMap, HashSet, VecDeque};
    use varos_core::editor::{Editor, ToolKind};
    use varos_core::geom::Rgba;
    use varos_core::model::{Anchor, Artboard, Path as VPath};
    use varos_core::EditCommand;

    const RED: Rgba = [1.0, 0.0, 0.0, 1.0];
    const BLUE: Rgba = [0.0, 0.0, 1.0, 1.0];

    // ───────────── fake ports ─────────────

    /// One scripted answer, consumed in order by the prompt of the same kind.
    #[derive(Debug)]
    enum Ans {
        Open(Vec<PathBuf>),
        Pick(Option<PathBuf>),
        Decide(SaveDecision),
        Fail(SaveFailChoice),
        Replace(bool),
    }

    #[derive(Default)]
    struct FakeDialogs {
        answers: VecDeque<Ans>,
        /// Every prompt shown, in order (`open`, `save-as <suggested> in <dir>`, `ask <name> [i/n]`,
        /// `failed <name>: <reason>`, `open-failed <name>: <reason>`, `replace <name>`, `notice <title>`).
        log: Vec<String>,
    }
    impl FakeDialogs {
        fn next(&mut self, prompt: String) -> Ans {
            self.log.push(prompt.clone());
            self.answers.pop_front().unwrap_or_else(|| panic!("unscripted prompt `{prompt}`; log: {:?}", self.log))
        }
    }
    impl Dialogs for FakeDialogs {
        fn pick_open(&mut self) -> Vec<PathBuf> {
            match self.next("open".into()) {
                Ans::Open(v) => v,
                a => panic!("the Open dialog got {a:?}"),
            }
        }
        fn pick_save(&mut self, suggested: &str, dir: Option<&Path>) -> Option<PathBuf> {
            let dir = dir.map_or("-".into(), |d| d.display().to_string());
            match self.next(format!("save-as {suggested} in {dir}")) {
                Ans::Pick(p) => p,
                a => panic!("the Save dialog got {a:?}"),
            }
        }
        fn ask_save_changes(&mut self, name: &str, progress: Option<(usize, usize)>) -> SaveDecision {
            let prompt = match progress {
                Some((i, n)) => format!("ask {name} {i}/{n}"),
                None => format!("ask {name}"),
            };
            match self.next(prompt) {
                Ans::Decide(d) => d,
                a => panic!("“Save changes?” got {a:?}"),
            }
        }
        fn save_failed(&mut self, name: &str, reason: &str) -> SaveFailChoice {
            match self.next(format!("failed {name}: {reason}")) {
                Ans::Fail(c) => c,
                a => panic!("“Couldn't save” got {a:?}"),
            }
        }
        fn open_failed(&mut self, name: &str, reason: &str) {
            self.log.push(format!("open-failed {name}: {reason}"));
        }
        fn confirm_replace(&mut self, name: &str) -> bool {
            match self.next(format!("replace {name}")) {
                Ans::Replace(b) => b,
                a => panic!("“Replace?” got {a:?}"),
            }
        }
        fn notice(&mut self, title: &str, _body: &str) {
            self.log.push(format!("notice {title}"));
        }
    }

    /// An in-memory disk. Keys are `(path, (7, inode))`; every save gives the file a NEW inode
    /// (like `write_atomic`'s temp + rename). An alias keeps its own path but shares the target's
    /// inode (a case variant / hard link: only the inode can tell).
    #[derive(Default)]
    struct FakeStore {
        files: HashMap<PathBuf, Document>,
        inodes: HashMap<PathBuf, u64>,
        next_ino: u64,
        aliases: HashMap<PathBuf, PathBuf>,
        fail_load: HashSet<PathBuf>,
        /// Save failures still to come per path (`u32::MAX` = always).
        fail_save: HashMap<PathBuf, u32>,
        loads: Vec<PathBuf>,
        saves: Vec<PathBuf>,
    }
    impl FakeStore {
        fn target(&self, p: &Path) -> PathBuf {
            self.aliases.get(p).cloned().unwrap_or_else(|| p.to_path_buf())
        }
        fn put(&mut self, p: &str, doc: Document) {
            self.files.insert(PathBuf::from(p), doc);
            self.rewrite(p);
        }
        /// Another app rewrote the file atomically: same content, a new inode.
        fn rewrite(&mut self, p: &str) {
            self.next_ino += 1;
            self.inodes.insert(PathBuf::from(p), self.next_ino);
        }
        fn doc(&self, p: &str) -> &Document {
            self.files.get(Path::new(p)).unwrap_or_else(|| panic!("no file {p}"))
        }
    }
    impl DocStore for FakeStore {
        fn load(&mut self, path: &Path) -> Result<Document, String> {
            let t = self.target(path);
            self.loads.push(t.clone());
            if self.fail_load.contains(&t) {
                return Err("The file is damaged".into());
            }
            self.files.get(&t).cloned().ok_or_else(|| "No such file".into())
        }
        fn save(&mut self, doc: &Document, path: &Path) -> Result<(), String> {
            let t = self.target(path);
            if let Some(left) = self.fail_save.get_mut(&t) {
                if *left > 0 {
                    if *left != u32::MAX {
                        *left -= 1;
                    }
                    return Err("The disk is not writable".into());
                }
            }
            self.saves.push(t.clone());
            self.files.insert(t.clone(), doc.clone());
            self.next_ino += 1;
            self.inodes.insert(t, self.next_ino);
            Ok(())
        }
        fn key(&self, path: &Path) -> FileKey {
            let t = self.target(path);
            FileKey { path: path.to_path_buf(), dev_ino: self.inodes.get(&t).map(|i| (7, *i)) }
        }
        fn exists(&self, path: &Path) -> bool {
            self.files.contains_key(&self.target(path))
        }
    }

    // ───────────── rig + helpers ─────────────

    struct Rig {
        ws: Workspace,
        d: FakeDialogs,
        s: FakeStore,
    }
    impl Rig {
        fn new() -> Self {
            Rig { ws: Workspace::new(), d: FakeDialogs::default(), s: FakeStore::default() }
        }
        fn run(&mut self, cmd: AppCommand) -> Effect {
            Lifecycle { ws: &mut self.ws, dialogs: &mut self.d, store: &mut self.s }.run(cmd)
        }
        fn script(&mut self, answers: impl IntoIterator<Item = Ans>) {
            self.d.answers.extend(answers);
        }
        /// Every scripted answer was used; returns (and clears) the prompt log.
        fn prompts(&mut self) -> Vec<String> {
            assert!(self.d.answers.is_empty(), "unused answers: {:?}", self.d.answers);
            std::mem::take(&mut self.d.log)
        }
        fn open(&mut self, p: &str) -> SessionId {
            self.run(AppCommand::OpenPaths(vec![PathBuf::from(p)], OpenOrigin::CommandLine));
            self.ws.active_id().unwrap()
        }
        fn get(&self, id: SessionId) -> &DocumentSession {
            self.ws.get(id).expect("the tab is open")
        }
        fn ed(&mut self, id: SessionId) -> &mut Editor {
            &mut self.ws.get_mut(id).expect("the tab is open").editor
        }
        fn active(&self) -> SessionId {
            self.ws.active_id().unwrap()
        }
        fn ids(&self) -> Vec<SessionId> {
            self.ws.sessions().iter().map(|s| s.id).collect()
        }
        fn names(&self) -> Vec<String> {
            self.ws.sessions().iter().map(|s| s.display_name()).collect()
        }
    }

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    /// A saved-looking file: one 100×100 board and one square of `color`.
    fn art(color: Rgba) -> Document {
        let a = |i: u32, p: [f32; 2]| Anchor { id: i, p, hin: None, hout: None, smooth: false };
        let sq = VPath::new(
            10,
            vec![a(100, [20.0, 20.0]), a(101, [50.0, 20.0]), a(102, [50.0, 50.0]), a(103, [20.0, 50.0])],
            true,
            Some(color),
            None,
            1.0,
        );
        Document {
            artboards: vec![Artboard { x: 0.0, y: 0.0, w: 100.0, h: 100.0, name: "A".into(), ..Artboard::default() }],
            paths: vec![sq],
            ids: 200,
            ..Document::default()
        }
    }

    /// Draw a square in `color` with the Rect tool (a real, committed edit).
    fn draw(ed: &mut Editor, color: Rgba) {
        let prev = ed.tool;
        ed.cur_fill = Some(color);
        ed.set_tool(ToolKind::Rect);
        ed.pointer_down([200.0, 200.0]);
        ed.pointer_move([240.0, 240.0]);
        ed.pointer_up();
        ed.set_tool(prev);
    }

    fn fills(ed: &Editor) -> Vec<Option<Rgba>> {
        ed.doc.paths.iter().map(|p| p.fill.solid()).collect()
    }

    /// The fills of the art stored in the fake file at `path`.
    fn file_fills(s: &FakeStore, path: &str) -> Vec<Option<Rgba>> {
        s.doc(path).paths.iter().map(|p| p.fill.solid()).collect()
    }

    // ───────────── New / Open ─────────────

    #[test]
    fn new_command_adds_clean_boardless_untitled() {
        let mut r = Rig::new();
        let first = r.active();
        draw(r.ed(first), RED);
        assert_eq!(r.run(AppCommand::NewDocument), Effect::default());
        let b = r.active();
        assert_ne!(b, first);
        assert_eq!(r.names(), ["Untitled-1", "Untitled-2"]);
        let s = r.get(b);
        assert!(s.is_pristine() && !s.is_dirty_exact(), "a new tab is clean");
        assert!(s.editor.doc.artboards.is_empty() && s.editor.doc.paths.is_empty(), "…and boardless");
        assert_eq!(r.get(first).editor.doc.paths.len(), 1, "New never clears the other tab");
        // close it and ⌘N again: the number is never reused
        r.run(AppCommand::CloseDocument(b));
        r.run(AppCommand::NewDocument);
        assert_eq!(r.names(), ["Untitled-1", "Untitled-3"]);
        assert!(r.prompts().is_empty(), "New and closing a clean tab ask nothing");
    }

    #[test]
    fn open_lands_in_a_new_tab_and_dirty_tab_survives() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let first = r.active();
        draw(r.ed(first), RED);
        r.script([Ans::Open(vec![p("/d/a.vrs")])]);
        r.run(AppCommand::OpenDialog);
        assert_eq!(r.prompts(), ["open"], "no discard prompt: nothing is replaced");
        assert_eq!(r.names(), ["Untitled-1", "a.vrs"]);
        let a = r.active();
        assert_ne!(a, first);
        assert_eq!(fills(&r.get(a).editor), [Some(BLUE)]);
        assert!(!r.get(a).is_dirty_exact());
        assert_eq!(r.get(a).path.as_deref(), Some(Path::new("/d/a.vrs")));
        assert!(r.get(first).is_dirty_exact(), "the edited Untitled survives, still dirty");
        assert_eq!(fills(&r.get(first).editor), [Some(RED)]);
        // a cancelled Open dialog does nothing
        r.script([Ans::Open(vec![])]);
        r.run(AppCommand::OpenDialog);
        assert_eq!(r.prompts(), ["open"]);
        assert_eq!(r.ids(), [first, a]);
    }

    #[test]
    fn open_reuses_pristine_untitled() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        assert_eq!(r.names(), ["a.vrs"], "the untouched Untitled-1 was replaced, not kept");
        assert_eq!(r.ids(), [a]);
        assert!(!r.get(a).is_dirty_exact());
        assert!(r.prompts().is_empty());
    }

    #[test]
    fn open_already_open_file_focuses_without_reload() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        draw(r.ed(a), RED); // dirty
        let rev = r.get(a).editor.rev;
        let other = {
            r.run(AppCommand::NewDocument);
            r.active()
        };
        assert_eq!(r.s.loads.len(), 1);
        // the same path again → focus, no reload, the unsaved edit is kept
        r.open("/d/a.vrs");
        assert_eq!(r.active(), a);
        assert_eq!(r.s.loads.len(), 1, "never reloaded");
        assert_eq!(r.get(a).editor.rev, rev);
        assert!(r.get(a).is_dirty_exact(), "a dirty tab is focused, not replaced");
        // an alias (another spelling, same inode) → the same tab
        r.s.aliases.insert(p("/Link/A.VRS"), p("/d/a.vrs"));
        r.run(AppCommand::ActivateDocument(other));
        r.open("/Link/A.VRS");
        assert_eq!(r.active(), a, "an alias focuses the open tab");
        // another app rewrote the file (new inode): the alias still finds the tab, because each tab's
        // key is computed fresh on Open (not the stale stored key)
        r.s.rewrite("/d/a.vrs");
        r.run(AppCommand::ActivateDocument(other));
        r.open("/Link/A.VRS");
        assert_eq!(r.active(), a, "…even after an external atomic rewrite");
        assert_eq!(r.s.loads.len(), 1);
        assert_eq!(r.ids(), [a, other], "no second tab for the same file");
        assert!(r.prompts().is_empty());
    }

    #[test]
    fn open_failure_changes_nothing() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        r.s.put("/d/broken.vrs", art(RED));
        r.s.fail_load.insert(p("/d/broken.vrs"));
        // a pristine Untitled survives a failed open under the same id
        let first = r.active();
        r.open("/d/broken.vrs");
        assert_eq!(r.ids(), [first]);
        assert!(r.get(first).is_pristine());
        assert_eq!(r.prompts(), ["open-failed broken.vrs: The file is damaged"]);
        // an edited file with a selection: nothing about it changes
        let a = r.open("/d/a.vrs");
        draw(r.ed(a), RED);
        {
            let ed = r.ed(a);
            ed.objsel = ed.doc.paths.iter().map(|p| p.id).collect();
        }
        let (doc, rev, sel) = {
            let s = r.get(a);
            (s.editor.doc.clone(), s.editor.rev, s.editor.objsel.clone())
        };
        r.script([Ans::Open(vec![p("/d/broken.vrs"), p("/d/missing.vrs")])]);
        r.run(AppCommand::OpenDialog);
        assert_eq!(
            r.prompts(),
            ["open", "open-failed broken.vrs: The file is damaged", "open-failed missing.vrs: No such file"]
        );
        assert_eq!(r.ids(), [a]);
        assert_eq!(r.active(), a);
        let s = r.get(a);
        assert_eq!(s.path.as_deref(), Some(Path::new("/d/a.vrs")));
        assert!(s.editor.doc == doc && s.editor.rev == rev && s.editor.objsel == sel);
        assert!(s.is_dirty_exact());
        r.ed(a).execute(EditCommand::Undo);
        assert!(!r.get(a).is_dirty_exact(), "the history is intact: undo still returns to the saved state");
    }

    #[test]
    fn open_multiple_paths_mixed_success() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        r.s.put("/d/b.pdf", art(RED));
        r.s.put("/d/bad.vrs", art(RED));
        r.s.fail_load.insert(p("/d/bad.vrs"));
        r.script([Ans::Open(vec![p("/d/a.vrs"), p("/d/bad.vrs"), p("/d/b.pdf"), p("/d/a.vrs")])]);
        r.run(AppCommand::OpenDialog);
        assert_eq!(r.prompts(), ["open", "open-failed bad.vrs: The file is damaged"]);
        assert_eq!(r.names(), ["a.vrs", "b.pdf"], "the pristine Untitled-1 took the first file");
        assert_eq!(r.s.loads.len(), 3, "the repeated a.vrs was focused, not loaded twice");
        assert_eq!(r.active(), r.ids()[0], "…and ends up active");
        assert!(r.ws.sessions().iter().all(|s| !s.is_dirty_exact()));
    }

    // ───────────── Save / Save As ─────────────

    #[test]
    fn save_untitled_goes_through_save_as() {
        let mut r = Rig::new();
        let id = r.active();
        draw(r.ed(id), RED);
        r.script([Ans::Pick(Some(p("/d/Logo.vrs")))]);
        r.run(AppCommand::Save(id));
        assert_eq!(r.prompts(), ["save-as Untitled-1.vrs in -"]);
        let s = r.get(id);
        assert_eq!(s.path.as_deref(), Some(Path::new("/d/Logo.vrs")));
        assert_eq!((s.display_name().as_str(), s.untitled), ("Logo.vrs", None));
        assert!(!s.is_dirty_exact());
        assert!(r.s.doc("/d/Logo.vrs").content_eq(&s.editor.doc));
        assert_eq!(s.key, Some(r.s.key(Path::new("/d/Logo.vrs"))), "the key is taken after the write");
        // the next Save writes in place without asking
        draw(r.ed(id), BLUE);
        r.run(AppCommand::Save(id));
        assert!(r.prompts().is_empty());
        assert_eq!(r.s.saves, [p("/d/Logo.vrs"), p("/d/Logo.vrs")]);
        assert_eq!(file_fills(&r.s, "/d/Logo.vrs"), [Some(RED), Some(BLUE)]);
    }

    #[test]
    fn save_as_cancel_changes_nothing() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        draw(r.ed(a), RED);
        r.script([Ans::Pick(None)]);
        r.run(AppCommand::SaveAs(a));
        assert_eq!(r.prompts(), ["save-as a.vrs in /d"]);
        assert_eq!(r.get(a).path.as_deref(), Some(Path::new("/d/a.vrs")));
        assert!(r.get(a).is_dirty_exact());
        assert!(r.s.saves.is_empty());
        // an Untitled whose first Save is cancelled stays Untitled and dirty
        r.run(AppCommand::NewDocument);
        let u = r.active();
        draw(r.ed(u), RED);
        r.script([Ans::Pick(None)]);
        r.run(AppCommand::Save(u));
        assert_eq!(r.prompts(), ["save-as Untitled-2.vrs in -"]);
        let s = r.get(u);
        assert!(s.path.is_none() && s.untitled == Some(2) && s.is_dirty_exact());
        assert!(r.s.saves.is_empty());
    }

    #[test]
    fn save_failure_keeps_path_and_dirty_then_try_again_succeeds() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        draw(r.ed(a), RED);
        r.s.fail_save.insert(p("/d/a.vrs"), 2);
        r.script([Ans::Fail(SaveFailChoice::Cancel)]);
        r.run(AppCommand::Save(a));
        assert_eq!(r.prompts(), ["failed a.vrs: The disk is not writable"]);
        assert!(r.get(a).is_dirty_exact(), "a failed save keeps the tab dirty");
        assert_eq!(r.get(a).path.as_deref(), Some(Path::new("/d/a.vrs")));
        assert_eq!(file_fills(&r.s, "/d/a.vrs"), [Some(BLUE)], "the old file is intact");
        // fails once more, then Try Again succeeds
        r.script([Ans::Fail(SaveFailChoice::TryAgain)]);
        r.run(AppCommand::Save(a));
        assert_eq!(r.prompts(), ["failed a.vrs: The disk is not writable"]);
        assert!(!r.get(a).is_dirty_exact());
        assert_eq!(file_fills(&r.s, "/d/a.vrs"), [Some(BLUE), Some(RED)]);
    }

    #[test]
    fn save_failure_save_as_route() {
        let mut r = Rig::new();
        r.s.put("/ro/a.vrs", art(BLUE));
        r.s.fail_save.insert(p("/ro/a.vrs"), u32::MAX);
        let a = r.open("/ro/a.vrs");
        draw(r.ed(a), RED);
        r.script([Ans::Fail(SaveFailChoice::SaveAs), Ans::Pick(Some(p("/d/copy.vrs")))]);
        r.run(AppCommand::Save(a));
        assert_eq!(r.prompts(), ["failed a.vrs: The disk is not writable", "save-as a.vrs in /ro"]);
        let s = r.get(a);
        assert_eq!(s.path.as_deref(), Some(Path::new("/d/copy.vrs")));
        assert!(!s.is_dirty_exact());
        assert_eq!(file_fills(&r.s, "/ro/a.vrs"), [Some(BLUE)], "the read-only original is untouched");
        assert_eq!(r.s.saves, [p("/d/copy.vrs")]);
    }

    #[test]
    fn save_as_moves_path_only_after_success() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        let old_key = r.get(a).key.clone();
        draw(r.ed(a), RED);
        r.s.fail_save.insert(p("/d/b.vrs"), 1);
        r.script([Ans::Pick(Some(p("/d/b.vrs"))), Ans::Fail(SaveFailChoice::Cancel)]);
        r.run(AppCommand::SaveAs(a));
        assert_eq!(r.prompts(), ["save-as a.vrs in /d", "failed a.vrs: The disk is not writable"]);
        let s = r.get(a);
        assert_eq!(s.path.as_deref(), Some(Path::new("/d/a.vrs")), "a failed Save As keeps the old path");
        assert_eq!(s.key, old_key);
        assert!(s.is_dirty_exact());
        r.script([Ans::Pick(Some(p("/d/b.vrs")))]);
        r.run(AppCommand::SaveAs(a));
        assert_eq!(r.prompts(), ["save-as a.vrs in /d"]);
        let s = r.get(a);
        assert_eq!((s.path.as_deref(), s.display_name().as_str()), (Some(Path::new("/d/b.vrs")), "b.vrs"));
        assert!(!s.is_dirty_exact());
        assert_eq!(file_fills(&r.s, "/d/a.vrs"), [Some(BLUE)], "the old file is left intact");
        assert_eq!(file_fills(&r.s, "/d/b.vrs"), [Some(BLUE), Some(RED)]);
        // the old file is no longer "open": opening it adds a tab
        r.open("/d/a.vrs");
        assert_eq!(r.names(), ["b.vrs", "a.vrs"]);
    }

    #[test]
    fn save_as_onto_other_open_tab_is_refused() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        r.s.put("/d/b.vrs", art(RED));
        let a = r.open("/d/a.vrs");
        let b = r.open("/d/b.vrs");
        draw(r.ed(b), BLUE);
        // onto A's file (also through an alias of it) → refused, back to the dialog, then cancelled
        r.s.aliases.insert(p("/Link/A.vrs"), p("/d/a.vrs"));
        r.script([Ans::Pick(Some(p("/d/a.vrs"))), Ans::Pick(Some(p("/Link/A.vrs"))), Ans::Pick(None)]);
        r.run(AppCommand::SaveAs(b));
        assert_eq!(
            r.prompts(),
            [
                "save-as b.vrs in /d",
                "notice “a.vrs” is open in another tab.",
                "save-as b.vrs in /d",
                "notice “a.vrs” is open in another tab.",
                "save-as b.vrs in /d",
            ]
        );
        assert!(r.s.saves.is_empty(), "A's file was never overwritten");
        assert_eq!(r.get(b).path.as_deref(), Some(Path::new("/d/b.vrs")));
        assert!(r.get(b).is_dirty_exact());
        assert_eq!(r.active(), b, "the refusal does not switch tabs");
        // onto its OWN file is fine
        r.script([Ans::Pick(Some(p("/d/b.vrs")))]);
        r.run(AppCommand::SaveAs(b));
        assert_eq!(r.prompts(), ["save-as b.vrs in /d"]);
        assert!(!r.get(b).is_dirty_exact());
        assert!(!r.get(a).is_dirty_exact());
    }

    #[test]
    fn save_on_pdf_path_offers_vrs() {
        let mut r = Rig::new();
        r.s.put("/d/Old.pdf", art(BLUE));
        let a = r.open("/d/Old.pdf");
        draw(r.ed(a), RED);
        r.script([Ans::Pick(Some(p("/d/Old.vrs")))]);
        r.run(AppCommand::Save(a));
        assert_eq!(r.prompts(), ["save-as Old.vrs in /d"], "Save on a .pdf goes through Save As, suggesting .vrs");
        assert_eq!(r.get(a).path.as_deref(), Some(Path::new("/d/Old.vrs")));
        assert!(!r.get(a).is_dirty_exact());
        assert_eq!(r.s.saves, [p("/d/Old.vrs")], "the .pdf was never written");
    }

    #[test]
    fn appended_extension_asks_before_replacing() {
        let mut r = Rig::new();
        r.s.put("/d/logo.vrs", art(BLUE));
        r.s.put("/d/mark.VRS", art(BLUE));
        let u = r.active();
        draw(r.ed(u), RED);
        // "logo" → logo.vrs exists → asked; No → back to the dialog; "logo.pdf" → logo.pdf.vrs (new)
        r.script([Ans::Pick(Some(p("/d/logo"))), Ans::Replace(false), Ans::Pick(Some(p("/d/logo.pdf")))]);
        r.run(AppCommand::SaveAs(u));
        assert_eq!(r.prompts(), ["save-as Untitled-1.vrs in -", "replace logo.vrs", "save-as Untitled-1.vrs in -"]);
        assert_eq!(r.get(u).path.as_deref(), Some(Path::new("/d/logo.pdf.vrs")), "no part of the name is dropped");
        assert_eq!(file_fills(&r.s, "/d/logo.vrs"), [Some(BLUE)], "logo.vrs was not replaced");
        // "logo" again, Replace → written
        draw(r.ed(u), RED);
        r.script([Ans::Pick(Some(p("/d/logo"))), Ans::Replace(true)]);
        r.run(AppCommand::SaveAs(u));
        assert_eq!(r.prompts(), ["save-as logo.pdf.vrs in /d", "replace logo.vrs"]);
        assert_eq!(r.get(u).path.as_deref(), Some(Path::new("/d/logo.vrs")));
        // a name the user typed WITH .vrs (any case) was already confirmed by the Save dialog
        r.script([Ans::Pick(Some(p("/d/mark.VRS")))]);
        r.run(AppCommand::SaveAs(u));
        assert_eq!(r.prompts(), ["save-as logo.vrs in /d"]);
        assert_eq!(r.get(u).path.as_deref(), Some(Path::new("/d/mark.VRS")));
    }

    #[test]
    fn open_after_save_still_focuses_the_same_tab() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        r.s.aliases.insert(p("/Link/A.VRS"), p("/d/a.vrs"));
        let a = r.open("/d/a.vrs");
        let before = r.get(a).key.clone().unwrap();
        draw(r.ed(a), RED);
        r.run(AppCommand::Save(a));
        let after = r.get(a).key.clone().unwrap();
        assert_ne!(before.dev_ino, after.dev_ino, "the save replaced the inode");
        assert_eq!(after, r.s.key(Path::new("/d/a.vrs")), "the tab's key was refreshed after the save");
        r.run(AppCommand::NewDocument);
        r.open("/d/a.vrs");
        assert_eq!(r.active(), a, "the same path after a save is the same tab");
        r.run(AppCommand::ActivateNext);
        r.open("/Link/A.VRS");
        assert_eq!(r.active(), a, "…and so is an alias (the refreshed inode)");
        assert_eq!(r.s.loads.len(), 1);
        assert_eq!(r.ws.sessions().len(), 2);
        assert!(r.prompts().is_empty());
    }

    // ───────────── dirty + history across tabs ─────────────

    #[test]
    fn undo_back_to_saved_is_clean_end_to_end() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        draw(r.ed(a), RED);
        r.run(AppCommand::Save(a));
        draw(r.ed(a), BLUE);
        assert!(r.get(a).is_dirty() && r.ws.tabs()[0].dirty);
        r.ed(a).execute(EditCommand::Undo);
        assert!(!r.get(a).is_dirty() && !r.ws.tabs()[0].dirty, "undo back to the saved content: no dot");
        // …so Close and Quit ask nothing
        assert_eq!(r.run(AppCommand::Quit), Effect { exit: true });
        r.run(AppCommand::CloseDocument(a));
        assert!(r.prompts().is_empty());
        assert_eq!(
            r.names(),
            ["Untitled-2"],
            "closed without a prompt (Untitled-1 was the pristine tab the open replaced)"
        );
        assert_eq!(r.s.saves.len(), 1);
    }

    #[test]
    fn two_tabs_have_independent_history() {
        let mut r = Rig::new();
        let a = r.active();
        draw(r.ed(a), RED);
        r.run(AppCommand::NewDocument);
        let b = r.active();
        draw(r.ed(b), BLUE);
        let rev_b = r.get(b).editor.rev;
        r.run(AppCommand::ActivateDocument(a));
        assert_eq!(r.active(), a);
        assert_eq!(fills(&r.get(a).editor), [Some(RED)], "each tab shows only its own art");
        assert_eq!(fills(&r.get(b).editor), [Some(BLUE)]);
        r.ed(a).execute(EditCommand::Undo);
        assert!(r.get(a).editor.doc.paths.is_empty(), "⌘Z in A undoes A's square");
        assert_eq!(fills(&r.get(b).editor), [Some(BLUE)], "…and only A's");
        assert_eq!(r.get(b).editor.rev, rev_b);
        assert!(!r.get(a).is_dirty_exact() && r.get(b).is_dirty_exact());
        r.run(AppCommand::ActivateDocument(b));
        r.ed(b).execute(EditCommand::Undo);
        assert!(r.get(b).editor.doc.paths.is_empty());
        r.ed(a).execute(EditCommand::Redo);
        assert_eq!(fills(&r.get(a).editor), [Some(RED)], "A's redo stack survived the switches");
    }

    // ───────────── Close ─────────────

    #[test]
    fn close_clean_tab_no_prompt() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        r.s.put("/d/b.vrs", art(RED));
        let a = r.open("/d/a.vrs");
        let b = r.open("/d/b.vrs");
        r.run(AppCommand::CloseDocument(a));
        assert!(r.prompts().is_empty());
        assert_eq!(r.ids(), [b]);
        assert_eq!(r.active(), b);
        r.run(AppCommand::CloseDocument(b));
        assert_eq!(r.names(), ["Untitled-2"], "the last tab closes to a fresh Untitled");
        r.run(AppCommand::CloseDocument(SessionId(999)));
        assert!(r.prompts().is_empty() && r.s.saves.is_empty());
    }

    #[test]
    fn close_dirty_tab_save_dont_save_cancel() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        r.s.put("/d/b.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        let b = r.open("/d/b.vrs");
        draw(r.ed(a), RED);
        draw(r.ed(b), RED);
        // Cancel → kept, dirty
        r.script([Ans::Decide(SaveDecision::Cancel)]);
        r.run(AppCommand::CloseDocument(b));
        assert_eq!(r.prompts(), ["ask b.vrs"]);
        assert_eq!(r.ids(), [a, b]);
        assert!(r.get(b).is_dirty_exact());
        // Save → written, then closed
        r.script([Ans::Decide(SaveDecision::Save)]);
        r.run(AppCommand::CloseDocument(b));
        assert_eq!(r.prompts(), ["ask b.vrs"]);
        assert_eq!(r.ids(), [a]);
        assert_eq!(file_fills(&r.s, "/d/b.vrs"), [Some(BLUE), Some(RED)]);
        // Don't Save → closed, nothing written
        r.script([Ans::Decide(SaveDecision::DontSave)]);
        r.run(AppCommand::CloseDocument(a));
        assert_eq!(r.prompts(), ["ask a.vrs"]);
        assert_eq!(r.names(), ["Untitled-2"]);
        assert_eq!(file_fills(&r.s, "/d/a.vrs"), [Some(BLUE)]);
        assert_eq!(r.s.saves, [p("/d/b.vrs")]);
    }

    #[test]
    fn close_inactive_dirty_tab_saves_that_tab_only() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        r.s.put("/d/b.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        let b = r.open("/d/b.vrs");
        draw(r.ed(a), RED);
        draw(r.ed(b), RED);
        assert_eq!(r.active(), b);
        r.script([Ans::Decide(SaveDecision::Save)]);
        r.run(AppCommand::CloseDocument(a));
        assert_eq!(r.prompts(), ["ask a.vrs"], "the prompt names the tab being closed");
        assert_eq!(r.s.saves, [p("/d/a.vrs")], "that tab is the one saved");
        assert_eq!(r.ids(), [b]);
        assert_eq!(r.active(), b, "the active tab never changed");
        assert!(r.get(b).is_dirty_exact(), "the active tab was not saved");
    }

    #[test]
    fn close_save_as_cancel_keeps_tab() {
        let mut r = Rig::new();
        let u = r.active();
        draw(r.ed(u), RED);
        r.run(AppCommand::NewDocument);
        r.run(AppCommand::ActivateDocument(u));
        r.script([Ans::Decide(SaveDecision::Save), Ans::Pick(None)]);
        r.run(AppCommand::CloseDocument(u));
        assert_eq!(r.prompts(), ["ask Untitled-1", "save-as Untitled-1.vrs in -"]);
        assert_eq!(r.names(), ["Untitled-1", "Untitled-2"], "a cancelled Save As keeps the tab");
        assert_eq!(r.active(), u, "…and the focus");
        assert!(r.get(u).is_dirty_exact());
        // a save that fails and is cancelled keeps it too
        r.s.fail_save.insert(p("/ro/x.vrs"), u32::MAX);
        r.script([Ans::Decide(SaveDecision::Save), Ans::Pick(Some(p("/ro/x.vrs"))), Ans::Fail(SaveFailChoice::Cancel)]);
        r.run(AppCommand::CloseDocument(u));
        assert_eq!(
            r.prompts(),
            ["ask Untitled-1", "save-as Untitled-1.vrs in -", "failed Untitled-1: The disk is not writable"]
        );
        assert_eq!(r.ws.sessions().len(), 2);
        assert!(r.get(u).is_dirty_exact() && r.get(u).path.is_none());
    }

    #[test]
    fn close_inactive_untitled_suggests_its_own_name() {
        let mut r = Rig::new();
        let one = r.active();
        draw(r.ed(one), RED);
        r.run(AppCommand::NewDocument);
        let two = r.active();
        draw(r.ed(two), BLUE);
        r.script([Ans::Decide(SaveDecision::Save), Ans::Pick(Some(p("/d/one.vrs")))]);
        r.run(AppCommand::CloseDocument(one));
        assert_eq!(r.prompts(), ["ask Untitled-1", "save-as Untitled-1.vrs in -"]);
        assert_eq!(file_fills(&r.s, "/d/one.vrs"), [Some(RED)], "Untitled-1's art was saved");
        assert_eq!(r.ids(), [two]);
        assert!(r.get(two).is_dirty_exact() && r.get(two).path.is_none(), "Untitled-2 untouched");
    }

    // ───────────── Quit ─────────────

    #[test]
    fn quit_clean_asks_nothing() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        r.open("/d/a.vrs");
        r.run(AppCommand::NewDocument);
        assert_eq!(r.run(AppCommand::Quit), Effect { exit: true });
        assert!(r.prompts().is_empty() && r.s.saves.is_empty());
    }

    #[test]
    fn quit_cancel_on_second_keeps_everything_and_first_stays_saved() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        r.s.put("/d/b.vrs", art(BLUE));
        r.s.put("/d/c.vrs", art(BLUE));
        let a = r.open("/d/a.vrs");
        let b = r.open("/d/b.vrs");
        let c = r.open("/d/c.vrs");
        draw(r.ed(b), RED);
        draw(r.ed(a), RED);
        assert_eq!(r.active(), c, "the clean tab is active when ⌘Q is pressed");
        r.script([Ans::Decide(SaveDecision::Save), Ans::Decide(SaveDecision::Cancel)]);
        assert_eq!(r.run(AppCommand::Quit), Effect { exit: false });
        assert_eq!(r.prompts(), ["ask a.vrs 1/2", "ask b.vrs 2/2"], "tab order, Document i of n");
        assert_eq!(r.ids(), [a, b, c], "Cancel keeps every tab");
        assert!(!r.get(a).is_dirty_exact(), "the first document stays saved");
        assert!(r.get(b).is_dirty_exact(), "the second is still dirty");
        assert_eq!(r.s.saves, [p("/d/a.vrs")]);
        assert_eq!(r.active(), b, "the document behind the last prompt is active");
    }

    #[test]
    fn quit_dont_save_all_exits_and_writes_nothing() {
        let mut r = Rig::new();
        r.s.put("/d/a.vrs", art(BLUE));
        let u = r.active();
        draw(r.ed(u), RED);
        let a = r.open("/d/a.vrs");
        draw(r.ed(a), RED);
        r.script([Ans::Decide(SaveDecision::DontSave), Ans::Decide(SaveDecision::DontSave)]);
        assert_eq!(r.run(AppCommand::Quit), Effect { exit: true });
        assert_eq!(r.prompts(), ["ask Untitled-1 1/2", "ask a.vrs 2/2"]);
        assert!(r.s.saves.is_empty(), "Don't Save writes nothing");
        assert_eq!(file_fills(&r.s, "/d/a.vrs"), [Some(BLUE)]);
        assert_eq!(r.ids(), [u, a], "no tab is destroyed by the lifecycle: the host exits");
    }

    #[test]
    fn quit_save_failure_aborts() {
        let mut r = Rig::new();
        r.s.put("/ro/a.vrs", art(BLUE));
        r.s.put("/d/b.vrs", art(BLUE));
        r.s.fail_save.insert(p("/ro/a.vrs"), u32::MAX);
        let a = r.open("/ro/a.vrs");
        let b = r.open("/d/b.vrs");
        draw(r.ed(a), RED);
        draw(r.ed(b), RED);
        r.script([Ans::Decide(SaveDecision::Save), Ans::Fail(SaveFailChoice::Cancel)]);
        assert_eq!(r.run(AppCommand::Quit), Effect { exit: false });
        assert_eq!(r.prompts(), ["ask a.vrs 1/2", "failed a.vrs: The disk is not writable"], "b is never asked");
        assert!(r.get(a).is_dirty_exact() && r.get(b).is_dirty_exact());
        assert!(r.s.saves.is_empty());
        // Try Again that keeps failing, then Save As elsewhere, lets the quit continue
        r.script([
            Ans::Decide(SaveDecision::Save),
            Ans::Fail(SaveFailChoice::TryAgain),
            Ans::Fail(SaveFailChoice::SaveAs),
            Ans::Pick(Some(p("/d/a.vrs"))),
            Ans::Decide(SaveDecision::DontSave),
        ]);
        assert_eq!(r.run(AppCommand::Quit), Effect { exit: true });
        assert_eq!(
            r.prompts(),
            [
                "ask a.vrs 1/2",
                "failed a.vrs: The disk is not writable",
                "failed a.vrs: The disk is not writable",
                "save-as a.vrs in /ro",
                "ask b.vrs 2/2",
            ]
        );
        assert_eq!(r.s.saves, [p("/d/a.vrs")]);
    }

    #[test]
    fn quit_untitled_save_as_cancel_aborts() {
        let mut r = Rig::new();
        let u = r.active();
        draw(r.ed(u), RED);
        r.script([Ans::Decide(SaveDecision::Save), Ans::Pick(None)]);
        assert_eq!(r.run(AppCommand::Quit), Effect { exit: false });
        assert_eq!(r.prompts(), ["ask Untitled-1 1/1", "save-as Untitled-1.vrs in -"]);
        assert!(r.get(u).is_dirty_exact() && r.get(u).path.is_none());
    }

    // ───────────── app-wide state + routing ─────────────

    #[test]
    fn clipboard_survives_close_of_active_tab() {
        let mut r = Rig::new();
        let a = r.active();
        draw(r.ed(a), RED);
        {
            let ed = r.ed(a);
            ed.objsel = ed.doc.paths.iter().map(|p| p.id).collect();
            ed.execute(EditCommand::Copy);
        }
        r.run(AppCommand::NewDocument);
        let b = r.active();
        assert_eq!(r.get(b).editor.clipboard().len(), 1, "the clipboard follows a tab switch");
        r.run(AppCommand::ActivateDocument(a));
        r.script([Ans::Decide(SaveDecision::DontSave)]);
        r.run(AppCommand::CloseDocument(a));
        assert_eq!(r.prompts(), ["ask Untitled-1"]);
        assert_eq!(r.active(), b);
        assert_eq!(r.get(b).editor.clipboard().len(), 1, "…and survives closing the tab that held it");
        r.ed(b).execute(EditCommand::Paste { offset: None });
        assert_eq!(fills(&r.get(b).editor), [Some(RED)], "pasting into the other tab works");
    }

    #[test]
    fn switch_reorder_and_window_commands_route_to_the_workspace() {
        let mut r = Rig::new();
        let a = r.active();
        r.run(AppCommand::NewDocument);
        let b = r.active();
        r.run(AppCommand::NewDocument);
        let c = r.active();
        r.run(AppCommand::ActivateNext);
        assert_eq!(r.active(), a, "Ctrl+Tab wraps around");
        r.run(AppCommand::ActivatePrevious);
        assert_eq!(r.active(), c);
        r.run(AppCommand::ReorderDocument(c, 0));
        assert_eq!(r.ids(), [c, a, b]);
        assert_eq!(r.run(AppCommand::Window(WindowCmd::Minimize)), Effect::default());
        assert_eq!(r.ids(), [c, a, b]);
        assert_eq!(r.active(), c);
        assert!(r.prompts().is_empty());
    }
}
