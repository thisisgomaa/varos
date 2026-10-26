//! Varos free canvases use the native document's existing empty artboard list.
use super::*;
use serde_json::json;
use vectorcraft_doc::Document;

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "file.newBoard",
        "New board",
        [],
        None,
        "{created?: Unix seconds|null (default now, recorded in the journal)} create a free canvas with zero artboards; add artboards later with the Artboard tool",
        always,
        new_board
    )]
}

fn new_board(s: &mut Session, p: &Value) -> Result<Value> {
    if !p.is_null() && p.as_object().is_none_or(|o| o.keys().any(|k| k != "created")) {
        return Err(bad("file.newBoard", "expected {created?: Unix seconds|null}"));
    }
    let mut doc = s.default_settings().document();
    doc.artboards.clear();
    doc.title = s.next_untitled();
    doc.grid = s.prefs.grid();
    doc.metadata.created = clock_date(s, "created", date_param(p, "created", "file.newBoard")?);
    let index = s.add_document(doc, None);
    Ok(json!({"index": index, "artboards": 0}))
}

/// Default export bounds on a free canvas; an explicitly selected artboard still validates.
pub(crate) fn export_art(doc: &Document, p: &Value) -> bool {
    doc.artboards.is_empty() && !["artboard", "artboards", "range"].iter().any(|k| p.get(k).is_some_and(|v| !v.is_null()))
}

/// A PDF needs a finite page. Adapt only the output copy, leaving the editable native
/// document (including its empty artboard list) in the PDF's editing attachment.
pub(crate) fn pdf_document(cmd: &str, doc: &Document, p: &Value) -> Result<Option<Document>> {
    if !export_art(doc, p)
        && !(doc.artboards.is_empty()
            && p.get("range").and_then(Value::as_str) == Some("all")
            && ["artboard", "artboards"].iter().all(|k| p.get(k).is_none_or(Value::is_null)))
    {
        return Ok(None);
    }
    let rect = vectorcraft_render::encode::art_bounds(doc).ok_or_else(|| bad(cmd, "nothing to export: the document has no visible art"))?;
    let mut output = doc.clone();
    output.artboards.push(vectorcraft_doc::Artboard {
        id: 1,
        name: String::new(),
        rect,
        locked: false,
        show_center_mark: false,
        show_cross_hairs: false,
        show_video_safe_areas: false,
        video_ruler_pixel_aspect_ratio: 1.0,
        color: None,
    });
    Ok(Some(output))
}

impl Session {
    /// Varos: put the open document with uid `uid` in front for the rest of one agent request
    /// (and the person's back after it, in the same UI turn), without finishing the tool's work as
    /// a tab switch does ([`Session::set_active`]): the tool's state stays the person's, on the
    /// person's document. The host checks first that no tool work is in progress
    /// ([`Session::tool_busy`], text editing, an open interaction). → `false` when it isn't open.
    pub fn varos_lend_front(&mut self, uid: u64) -> bool {
        match self.docs.iter().position(|d| d.uid == uid) {
            Some(i) => {
                self.active = Some(i);
                true
            }
            None => false,
        }
    }

    /// Varos: does the tool hold the person's work in progress (a drag, an open pen path, typing)?
    /// Then another board must not be lent the front ([`Session::varos_lend_front`]).
    pub fn varos_tool_working(&self) -> bool {
        self.tool_busy() || self.tool_wants_text() || self.tool_composing() || self.active().is_some_and(|d| d.interaction.is_some())
    }
}

thread_local! {
    /// Varos: the board (document uid) an agent request is pinned to while it runs on this thread.
    static PIN: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

impl Session {
    /// Varos: pin the commands run on this thread to the open document with uid `uid` (`None`:
    /// unpin). While pinned, every command, each step of a batch included, runs only when that
    /// document is the active one ([`check_pin`]): a step that would run on another document
    /// fails with `target_changed` and the steps after it don't run. The host pins an agent
    /// request that names a board for the turn that runs it.
    #[allow(clippy::unused_self)] // a method so hosts reach it through the session they drive
    pub fn varos_pin(&mut self, uid: Option<u64>) {
        PIN.with(|p| p.set(uid));
    }
}

/// Varos: the pin's check before command `id` runs ([`Session::varos_pin`]).
pub(crate) fn check_pin(s: &Session, id: &str) -> Result<()> {
    let Some(pin) = PIN.with(std::cell::Cell::get) else { return Ok(()) };
    if s.active().map(|d| d.uid) == Some(pin) {
        return Ok(());
    }
    Err(EngineError::Other(format!(
        "target_changed: `{id}` would run on another board than the one the request names (a step left it); it and the steps after it were not run"
    )))
}

#[cfg(test)]
mod tests;
