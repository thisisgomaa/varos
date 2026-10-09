//! Lane F: native accelerators project effective command bindings.
use super::*;
impl MacMenu {
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
                let Some(command) = rows.iter().find(|c| c.alias == item.id().0) else { continue };
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
