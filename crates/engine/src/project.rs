//! Project/session layer. The engine and tools still edit one unchanged DocState.
use crate::{DocState, EngineError, Result, Session};
use serde_json::{Value, json};
use std::ops::{Deref, DerefMut};
use vectorcraft_doc::Document;

#[derive(Clone, Debug)]
pub struct Page {
    pub id: String,
    pub name: String,
    pub doc: DocState,
}

/// One file/tab. `active` and `pages` are private: every constructor and mutation keeps a
/// nonempty vector and a valid active index. Deref leases that page to the existing engine.
#[derive(Clone, Debug)]
pub struct Project {
    pages: Vec<Page>,
    active: usize,
    saved_pages: Vec<(String, String)>,
    file_title: String,
    page_undo: Vec<PageStep>,
    page_redo: Vec<PageStep>,
}
/// Structural snapshots retain deleted canvases, but live canvases are never rolled back.
#[derive(Clone, Debug)]
struct PageStep {
    label: String,
    pages: Vec<Page>,
    active: String,
    boundary: Vec<(String, std::sync::Arc<Document>)>,
}
impl Deref for Project {
    type Target = DocState;
    fn deref(&self) -> &DocState {
        &self.pages[self.active].doc
    }
}
impl DerefMut for Project {
    fn deref_mut(&mut self) -> &mut DocState {
        &mut self.pages[self.active].doc
    }
}
impl From<DocState> for Project {
    fn from(doc: DocState) -> Self {
        let file_title = doc.doc.title.clone();
        let pages = vec![Page { id: "page-1".into(), name: "Page 1".into(), doc }];
        Self { page_undo: Vec::new(), page_redo: Vec::new(), file_title, saved_pages: vec![("page-1".into(), "Page 1".into())], pages, active: 0 }
    }
}
impl Project {
    fn page_step(&self, label: String) -> PageStep {
        PageStep { label, pages: self.pages.clone(), active: self.page_id().to_owned(), boundary: Vec::new() }
    }
    fn boundary(&self) -> Vec<(String, std::sync::Arc<Document>)> {
        self.pages.iter().map(|p| (p.id.clone(), p.doc.doc.clone())).collect()
    }
    /// Structural history takes precedence only after later canvas edits have been undone.
    pub fn page_history_label(&self, redo: bool) -> Option<&str> {
        let step = if redo { self.page_redo.last() } else { self.page_undo.last() }?;
        step.boundary
            .iter()
            .all(|(id, doc)| self.pages.iter().find(|p| &p.id == id).is_none_or(|p| p.doc.same_content(doc)))
            .then_some(step.label.as_str())
    }
    pub(crate) fn turn_page_history(&mut self, redo: bool) -> Option<String> {
        self.page_history_label(redo)?;
        let step = if redo { self.page_redo.pop() } else { self.page_undo.pop() }?;
        let mut inverse = self.page_step(step.label.clone());
        let identity = self.deref().clone();
        let mut live: std::collections::HashMap<_, _> = std::mem::take(&mut self.pages).into_iter().map(|p| (p.id.clone(), p.doc)).collect();
        self.pages = step
            .pages
            .into_iter()
            .map(|mut p| {
                if let Some(doc) = live.remove(&p.id) {
                    p.doc = doc;
                }
                p
            })
            .collect();
        self.active = self.pages.iter().position(|p| p.id == step.active).unwrap_or(0);
        // File state may have changed since the operation (for example Save As).
        for p in &mut self.pages {
            p.doc.path = identity.path.clone();
            p.doc.uid = identity.uid;
            p.doc.format = identity.format;
            p.doc.save_options = identity.save_options.clone();
            p.doc.recovery = identity.recovery.clone();
            p.doc.history.actor = identity.history.actor;
        }
        self.revision = identity.revision.saturating_add(1);
        inverse.boundary = self.boundary();
        let label = step.label;
        if redo {
            self.page_undo.push(inverse);
        } else {
            self.page_redo.push(inverse);
        }
        Some(label)
    }
    pub fn title(&self) -> String {
        if self.path.is_some() || self.converted || self.recovered { self.deref().title() } else { self.file_title.clone() }
    }
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }
    pub fn active_page(&self) -> usize {
        self.active
    }
    pub fn page_id(&self) -> &str {
        &self.pages[self.active].id
    }
    pub fn content_snapshot(&self) -> Vec<(String, String, std::sync::Arc<Document>)> {
        self.pages.iter().map(|p| (p.id.clone(), p.name.clone(), p.doc.doc.clone())).collect()
    }
    pub fn same_pages(&self, saved: &[(String, String, std::sync::Arc<Document>)]) -> bool {
        self.pages.len() == saved.len() && self.pages.iter().zip(saved).all(|(p, s)| p.id == s.0 && p.name == s.1 && p.doc.same_content(&s.2))
    }
    fn signature(&self) -> Vec<(String, String)> {
        self.pages.iter().map(|p| (p.id.clone(), p.name.clone())).collect()
    }
    pub fn is_dirty(&self) -> bool {
        self.signature() != self.saved_pages || self.pages.iter().any(|p| p.doc.is_dirty())
    }
    pub fn mark_saved(&mut self) {
        for p in &mut self.pages {
            p.doc.mark_saved();
        }
        self.saved_pages = self.signature();
    }
    pub fn snapshot(&self) -> vectorcraft_format::Project {
        vectorcraft_format::Project {
            pages: self
                .pages
                .iter()
                .map(|p| {
                    let mut doc = (*p.doc.doc).clone();
                    doc.last_view = p.doc.view.clone();
                    doc.layers_open = p.doc.layers_open.saved(&doc);
                    doc.transparency_grid = p.doc.transparency_grid;
                    vectorcraft_format::Page { id: p.id.clone(), name: p.name.clone(), doc }
                })
                .collect(),
            active: self.active,
        }
    }
    pub(crate) fn saved_snapshot(&mut self, saved: &Project) {
        self.saved_pages = saved.signature();
        for page in &mut self.pages {
            if let Some(old) = saved.pages.iter().find(|p| p.id == page.id) {
                page.doc.mark_saved_as(&old.doc.doc);
            }
        }
    }
    pub(crate) fn page_states_mut(&mut self) -> impl Iterator<Item = &mut DocState> {
        self.pages.iter_mut().map(|page| &mut page.doc)
    }
    pub fn page_state_mut(&mut self, id: &str) -> Option<&mut DocState> {
        self.pages.iter_mut().find(|p| p.id == id).map(|p| &mut p.doc)
    }
    pub fn list_pages(&self) -> Value {
        json!({"pages": self.pages.iter().enumerate().map(|(i,p)| json!({
            "page_id": p.id, "page_name": p.name, "active": i == self.active,
            "index": i, "artboards": p.doc.doc.artboards.len()
        })).collect::<Vec<_>>(), "active_page": self.active})
    }
    pub fn resolve_page(&self, p: &Value, cmd: &str) -> Result<usize> {
        match p.get("page") {
            None => Ok(self.active),
            Some(Value::String(id)) => self.pages.iter().position(|p| &p.id == id).ok_or_else(|| crate::cmd::bad(cmd, "unknown page id")),
            Some(Value::Number(n)) => n
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .filter(|&n| n < self.pages.len())
                .ok_or_else(|| crate::cmd::bad(cmd, "page index out of range")),
            _ => Err(crate::cmd::bad(cmd, "page must be an id or zero-based index")),
        }
    }
    pub(crate) fn install(&mut self, project: vectorcraft_format::Project) -> Result<()> {
        if project.pages.is_empty() || project.active >= project.pages.len() {
            return Err(EngineError::Other("invalid project".into()));
        }
        let previous = self.deref().clone();
        let pages = project
            .pages
            .into_iter()
            .map(|p| {
                let mut doc = p.doc;
                Session::refresh_text_bounds(&mut doc);
                doc.adopt_guides();
                let mut st = DocState::new(doc, previous.path.clone());
                st.uid = previous.uid;
                st.format = previous.format;
                st.converted = previous.converted;
                st.opened_by = previous.opened_by.clone();
                st.save_options = previous.save_options.clone();
                st.imported_from = previous.imported_from.clone();
                st.import_losses = previous.import_losses.clone();
                st.recovered = previous.recovered;
                st.recovery = previous.recovery.clone();
                st.history.limit = previous.history.limit;
                st.history.actor = previous.history.actor;
                Page { id: p.id, name: p.name, doc: st }
            })
            .collect();
        self.page_undo.clear();
        self.page_redo.clear();
        self.pages = pages;
        self.active = project.active;
        self.saved_pages = self.signature();
        Ok(())
    }
    pub fn switch(&mut self, index: usize) -> Result<()> {
        if index >= self.pages.len() {
            return Err(EngineError::Other("unknown page".into()));
        }
        // File identity follows the tab; edit, selection, history and view follow the page.
        let old = self.deref();
        let save_options = old.save_options.clone();
        let imported_from = old.imported_from.clone();
        let import_losses = old.import_losses.clone();
        let (uid, path, format, converted, recovered, recovery, opened_by, revision) =
            (old.uid, old.path.clone(), old.format, old.converted, old.recovered, old.recovery.clone(), old.opened_by.clone(), old.revision);
        self.active = index;
        let st = self.deref_mut();
        st.uid = uid;
        st.path = path;
        st.format = format;
        st.save_options = save_options;
        st.imported_from = imported_from;
        st.import_losses = import_losses;
        st.converted = converted;
        st.recovered = recovered;
        st.recovery = recovery;
        st.opened_by = opened_by;
        st.revision = revision.saturating_add(1);
        Ok(())
    }
}
impl Session {
    pub fn switch_page(&mut self, index: usize) -> Result<()> {
        if index >= self.project()?.pages.len() {
            return Err(EngineError::Other("unknown page".into()));
        }
        if index != self.project()?.active {
            if self.explicit_undo_group.is_some() || self.batch_stash.is_some() {
                return Err(EngineError::Other("finish the command batch or undo group before switching pages".into()));
            }
            self.reset_tool_for_doc_switch();
            self.project_mut()?.switch(index)?;
        }
        Ok(())
    }
}

pub(crate) fn command(s: &mut Session, p: &Value, op: &str) -> Result<Value> {
    let cmd = format!("page.{op}");
    let index = s.project()?.resolve_page(p, &cmd)?;
    if op == "switch" {
        s.switch_page(index)?;
        return Ok(s.project()?.list_pages());
    }
    if s.explicit_undo_group.is_some() || s.batch_stash.is_some() || s.tool_busy() {
        return Err(crate::cmd::bad(&cmd, "finish the current interaction or batch before changing pages"));
    }
    let name = if let Some(v) = p.get("name") {
        let n = v
            .as_str()
            .filter(|n| !n.trim().is_empty() && n.len() <= 4096)
            .ok_or_else(|| crate::cmd::bad(&cmd, "name must be nonempty text (at most 4096 bytes)"))?;
        Some(n.to_owned())
    } else {
        None
    };
    let before = s.project()?.page_step(
        match op {
            "add" => "Add Page",
            "duplicate" => "Duplicate Page",
            "rename" => "Rename Page",
            "delete" => "Delete Page",
            "move" => "Move Page",
            _ => "Page",
        }
        .into(),
    );
    if op == "delete" && s.project()?.pages.len() > 1 && index == s.project()?.active {
        // Deleting the visible canvas is also a document switch: clear idle tool caches.
        s.switch_page(if index == 0 { 1 } else { index - 1 })?;
    }
    let project = s.project_mut()?;
    match op {
        "add" | "duplicate" => {
            if project.pages.len() >= 10_000 {
                return Err(crate::cmd::bad(&cmd, "too many pages"));
            }
            let mut doc = if op == "duplicate" { (*project.pages[index].doc.doc).clone() } else { Document::new(800.0, 600.0) };
            doc.title = project.doc.title.clone();
            let mut state = DocState::new(doc, project.path.clone());
            state.uid = project.uid;
            state.history.limit = project.history.limit;
            state.history.actor = project.history.actor;
            let id = loop {
                let candidate = format!("page-{}", crate::NEXT_DOC_UID.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
                if !project.pages.iter().any(|p| p.id == candidate) {
                    break candidate;
                }
            };
            let name = name.unwrap_or_else(|| {
                if op == "duplicate" {
                    let mut name = project.pages[index].name.clone();
                    while name.len() > 4096 - " copy".len() {
                        name.pop();
                    }
                    name.push_str(" copy");
                    name
                } else {
                    format!("Page {}", project.pages.len() + 1)
                }
            });
            project.pages.push(Page { id, name, doc: state });
        }
        "rename" => project.pages[index].name = name.ok_or_else(|| crate::cmd::bad(&cmd, "missing name"))?,
        "delete" => {
            if project.pages.len() == 1 {
                return Err(crate::cmd::bad(&cmd, "cannot delete the last page"));
            }
            project.pages.remove(index);
            if project.active > index {
                project.active -= 1;
            }
        }
        "move" => {
            let to = p
                .get("index")
                .and_then(Value::as_u64)
                .and_then(|n| usize::try_from(n).ok())
                .filter(|&n| n < project.pages.len())
                .ok_or_else(|| crate::cmd::bad(&cmd, "index out of range"))?;
            let active_id = project.page_id().to_owned();
            let page = project.pages.remove(index);
            project.pages.insert(to, page);
            project.active = project.pages.iter().position(|p| p.id == active_id).unwrap_or(0);
        }
        _ => return Err(crate::cmd::bad(&cmd, "unknown page command")),
    }
    if project.signature() != before.pages.iter().map(|p| (p.id.clone(), p.name.clone())).collect::<Vec<_>>() {
        let mut step = before;
        step.boundary = project.boundary();
        project.page_undo.push(step);
        if project.page_undo.len() > 100 {
            project.page_undo.remove(0);
        }
        project.page_redo.clear();
    }
    project.revision = project.revision.saturating_add(1);
    Ok(project.list_pages())
}
