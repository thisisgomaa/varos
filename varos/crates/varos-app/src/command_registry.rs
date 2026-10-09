//! Lane F: migration projection over the incumbent menu tables.
use crate::menus::{self, Accel, DocMenuState, Entry, MenuCmd};
use varos_core::registry::{self, Availability};
#[derive(Clone)]
pub struct Command {
    pub id: String,
    pub aliases: Vec<String>,
    pub label: &'static str,
    pub accel: Option<Accel>,
    pub handler: MenuCmd,
}
pub fn commands() -> Vec<Command> {
    let mut rows = flatten(&menus::menus())
        .into_iter()
        .filter_map(|e| match e {
            Entry::Item { id, label, accel, cmd, .. } => {
                Some(Command { id: registry::menu_id(&id), aliases: vec![id.clone()], label, accel, handler: cmd })
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut unique: Vec<Command> = vec![];
    for row in rows.drain(..) {
        if let Some(existing) = unique.iter_mut().find(|c| c.id == row.id) {
            existing.aliases.extend(row.aliases);
        } else {
            unique.push(row);
        }
    }
    rows = unique;
    for binding in crate::shortcuts::parity::BINDINGS {
        let Some(code) = crate::shortcut_editor::key_code(binding.key) else { continue };
        let a = Accel { code, cmd: binding.primary, shift: binding.shift, alt: binding.alt };
        if rows.iter().any(|c| c.accel == Some(a) || c.label.trim_end_matches('…') == binding.illustrator) {
            continue;
        }
        let Some(id) = crate::parity_registry::id(binding.illustrator) else { continue };
        if rows.iter().any(|c| c.id == id) {
            continue;
        }
        rows.push(Command {
            id: id.into(),
            aliases: vec![id.into()],
            label: binding.illustrator,
            accel: Some(a),
            handler: MenuCmd::Key(a),
        });
    }
    rows
}

pub fn availability(c: &Command, state: DocMenuState) -> Availability {
    let enabled = match c.handler {
        MenuCmd::File(f) => menus::file_row_enabled(f, state),
        // ---- Lane G ----
        MenuCmd::Release(_) => true,
        MenuCmd::Phase9(_) => true,
        MenuCmd::ToggleRail
        | MenuCmd::ToggleDock
        | MenuCmd::TogglePicker
        | MenuCmd::TogglePanel(_)
        | MenuCmd::ResetLayout => true,
        MenuCmd::Key(a)
            if matches!(
                a.code,
                winit::keyboard::KeyCode::KeyC
                    | winit::keyboard::KeyCode::KeyX
                    | winit::keyboard::KeyCode::KeyG
                    | winit::keyboard::KeyCode::Digit7
            ) =>
        {
            state.active && state.has_selection
        }
        MenuCmd::Plain(winit::keyboard::KeyCode::Backspace) => state.active && state.has_selection,
        _ => state.active,
    };
    if enabled {
        Availability::enabled()
    } else {
        Availability::disabled("Open a document or satisfy the command's target requirements")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_menu_row_has_unique_registry_identity() {
        let mut ids = std::collections::HashSet::new();
        let rows = commands();
        for entry in flatten(&menus::menus()) {
            if let Entry::Item { id, .. } = entry {
                assert!(rows.iter().any(|c| c.aliases.contains(&id)), "orphan menu {id}");
            }
        }
        for c in rows {
            assert!(ids.insert(c.id.clone()), "{}", c.id);
            assert!(c.id.bytes().all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b)));
        }
    }
    #[test]
    fn no_document_reasons() {
        for c in commands() {
            let a = availability(&c, DocMenuState::default());
            assert_eq!(a.enabled, a.disabled_reason.is_none());
        }
    }
}

fn flatten(rows: &[(&'static str, Vec<Entry>)]) -> Vec<Entry> {
    fn walk(entries: &[Entry], out: &mut Vec<Entry>) {
        for entry in entries {
            match entry {
                Entry::Sub { items, .. } => walk(items, out),
                Entry::Item { .. } => out.push(entry.clone()),
                _ => {}
            }
        }
    }
    let mut out = vec![];
    for (_, entries) in rows {
        walk(entries, &mut out)
    }
    out
}

pub fn index(
    limit: usize,
    cursor: usize,
    state: DocMenuState,
    editor: Option<&varos_core::Editor>,
) -> Result<varos_bridge::Reply, varos_bridge::Error> {
    if limit == 0 || limit > 100 {
        return Err(varos_bridge::Error::new("invalid_argument", "limit must be 1–100"));
    }
    let rows = commands();
    if cursor > rows.len() {
        return Err(varos_bridge::Error::new("invalid_argument", "cursor is out of range"));
    }
    let entries=rows.iter().skip(cursor).take(limit).map(|c|{let a=resolved(c,state,editor);serde_json::json!({"id":c.id,"aliases":c.aliases,"label":c.label,"enabled":a.enabled,"disabled_reason":a.disabled_reason,"scope":"desktop","shortcut":shortcut(c)})}).collect::<Vec<_>>();
    let next = cursor + entries.len();
    Ok(varos_bridge::Reply::success(
        serde_json::json!({"schema_version":1,"commands":entries,"next_cursor":if next<rows.len(){Some(next)}else{None}}),
    ))
}

/// Resolve the same advisory state used by desktop menu projections; execution still rechecks.
pub fn resolved(c: &Command, state: DocMenuState, editor: Option<&varos_core::Editor>) -> Availability {
    let base = availability(c, state);
    if !base.enabled {
        return base;
    }
    let Some(ed) = editor.filter(|_| state.active) else { return base };
    if c.id == "history.undo" && !ed.history_available(false) {
        return Availability::disabled("nothing_to_undo");
    }
    if c.id == "history.redo" && !ed.history_available(true) {
        return Availability::disabled("nothing_to_redo");
    }
    if matches!(c.id.as_str(), "edit.clip" | "edit.release_clip") {
        let (make, release) = ed.clipping_enablement();
        if !(if c.id == "edit.clip" { make } else { release }) {
            return Availability::disabled("invalid_clipping_selection");
        }
    }
    if ed.transaction_open()
        && !matches!(
            c.handler,
            MenuCmd::Phase9(_)
                | MenuCmd::TogglePanel(_)
                | MenuCmd::ToggleRail
                | MenuCmd::ToggleDock
                | MenuCmd::TogglePicker
        )
    {
        return Availability::disabled("busy");
    }
    base
}
thread_local! { static EFFECTIVE: std::cell::RefCell<crate::shortcut_editor::Overrides> = std::cell::RefCell::new(crate::shortcut_editor::Overrides::default()); }
pub fn publish_shortcuts(overrides: crate::shortcut_editor::Overrides) {
    EFFECTIVE.with(|current| *current.borrow_mut() = overrides);
}
pub fn shortcut(c: &Command) -> Option<crate::shortcut_editor::Chord> {
    EFFECTIVE.with(|current| current.borrow().effective(c))
}
pub fn shortcut_text(c: &Command) -> String {
    shortcut(c)
        .map(|chord| {
            chord
                .text()
                .replace("Primary+", if cfg!(target_os = "macos") { "⌘" } else { "Ctrl+" })
                .replace("Shift+", if cfg!(target_os = "macos") { "⇧" } else { "Shift+" })
                .replace("Alt+", if cfg!(target_os = "macos") { "⌥" } else { "Alt+" })
                .replace("Key", "")
                .replace("Digit", "")
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod context_tests {
    use super::*;
    #[test]
    fn aliases_share_identity_and_busy_history_has_a_reason() {
        let rows = commands();
        let shortcuts = rows.iter().find(|c| c.id == "app.shortcuts").unwrap();
        assert!(shortcuts.aliases.contains(&"help.shortcuts".into()));
        let undo = rows.iter().find(|c| c.id == "history.undo").unwrap();
        let state = DocMenuState { active: true, ..Default::default() };
        let mut editor = varos_core::Editor::new();
        assert_eq!(resolved(undo, state, Some(&editor)).disabled_reason.as_deref(), Some("nothing_to_undo"));
        editor.begin();
        editor.doc.name = "Changed".into();
        editor.dirty = true;
        editor.commit();
        assert!(resolved(undo, state, Some(&editor)).enabled);
        editor.begin();
        assert_eq!(resolved(undo, state, Some(&editor)).disabled_reason.as_deref(), Some("busy"));
    }
}
