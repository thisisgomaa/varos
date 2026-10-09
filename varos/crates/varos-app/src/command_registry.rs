//! Lane F: migration projection over the incumbent menu tables.
use crate::menus::{self, Accel, DocMenuState, Entry, MenuCmd};
use varos_core::registry::{self, Availability};
#[derive(Clone)]
pub struct Command {
    pub id: String,
    pub alias: String,
    pub label: &'static str,
    pub accel: Option<Accel>,
    pub handler: MenuCmd,
}
pub fn commands() -> Vec<Command> {
    let mut rows = flatten(&menus::menus())
        .into_iter()
        .filter_map(|e| match e {
            Entry::Item { id, label, accel, cmd, .. } => {
                Some(Command { id: registry::menu_id(&id), alias: id, label, accel, handler: cmd })
            }
            _ => None,
        })
        .collect::<Vec<_>>();
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
            alias: id.into(),
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
        MenuCmd::Phase9(_) => true,
        MenuCmd::ToggleRail
        | MenuCmd::ToggleDock
        | MenuCmd::TogglePicker
        | MenuCmd::TogglePanel(_)
        | MenuCmd::ResetLayout => true,
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
        for c in commands() {
            assert!(ids.insert(c.id.clone()), "{}", c.alias);
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

pub fn index(limit: usize, cursor: usize, state: DocMenuState) -> Result<varos_bridge::Reply, varos_bridge::Error> {
    if limit == 0 || limit > 100 {
        return Err(varos_bridge::Error::new("invalid_argument", "limit must be 1–100"));
    }
    let rows = commands();
    if cursor > rows.len() {
        return Err(varos_bridge::Error::new("invalid_argument", "cursor is out of range"));
    }
    let entries=rows.iter().skip(cursor).take(limit).map(|c|{let a=availability(c,state);serde_json::json!({"id":c.id,"aliases":[c.alias],"label":c.label,"enabled":a.enabled,"disabled_reason":a.disabled_reason,"scope":"desktop","shortcut":c.accel.map(crate::shortcut_editor::Chord::from_accel)})}).collect::<Vec<_>>();
    let next = cursor + entries.len();
    Ok(varos_bridge::Reply::success(
        serde_json::json!({"schema_version":1,"commands":entries,"next_cursor":if next<rows.len(){Some(next)}else{None}}),
    ))
}
