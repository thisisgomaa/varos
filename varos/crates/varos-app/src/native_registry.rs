//! Lane F: native accelerators project effective command bindings.
use super::*;
impl MacMenu {
    pub fn sync_registry(&self, state: crate::menus::DocMenuState, editor: Option<&varos_core::Editor>) {
        fn walk(
            items: Vec<muda::MenuItemKind>,
            rows: &[crate::command_registry::Command],
            state: crate::menus::DocMenuState,
            editor: Option<&varos_core::Editor>,
        ) {
            for item in items {
                if let muda::MenuItemKind::Submenu(sub) = &item {
                    walk(sub.items(), rows, state, editor);
                    continue;
                }
                let Some(command) = rows.iter().find(|c| c.aliases.contains(&item.id().0)) else { continue };
                let enabled = crate::command_registry::resolved(command, state, editor).enabled;
                match item {
                    muda::MenuItemKind::MenuItem(row) if row.is_enabled() != enabled => row.set_enabled(enabled),
                    muda::MenuItemKind::Check(row) if row.is_enabled() != enabled => row.set_enabled(enabled),
                    _ => {}
                }
            }
        }
        walk(self.menu.items(), &crate::command_registry::commands(), state, editor);
    }
    pub fn sync_shortcuts(&self, overrides: &crate::shortcut_editor::Overrides) {
        fn walk(
            items: Vec<muda::MenuItemKind>,
            overrides: &crate::shortcut_editor::Overrides,
            rows: &[crate::command_registry::Command],
        ) {
            for item in items {
                if let muda::MenuItemKind::Submenu(sub) = &item {
                    walk(sub.items(), overrides, rows);
                    continue;
                }
                let Some(command) = rows.iter().find(|c| c.aliases.contains(&item.id().0)) else { continue };
                let effective = overrides
                    .effective(command)
                    .and_then(|chord| {
                        crate::shortcut_editor::key_code(&chord.key).map(|code| chrome::Accel {
                            code,
                            shift: chord.shift,
                            alt: chord.alt,
                            cmd: chord.primary,
                        })
                    })
                    .and_then(accelerator);
                match item {
                    muda::MenuItemKind::MenuItem(row) => {
                        let _ = row.set_accelerator(effective);
                    }
                    muda::MenuItemKind::Check(row) => {
                        let _ = row.set_accelerator(effective);
                    }
                    _ => {}
                }
            }
        }
        walk(self.menu.items(), overrides, &crate::command_registry::commands());
    }
}
