//! Lane F: current command/menu catalog coverage; no GUI or native menu creation.
#[test]
fn every_menu_and_registered_command_has_arabic_copy() {
    fn visit(entries: &[crate::menus::Entry], missing: &mut Vec<String>) {
        for entry in entries {
            match entry {
                crate::menus::Entry::Item { label, .. } => {
                    if !varos_app::i18n::contains(label) {
                        missing.push((*label).into());
                    }
                }
                crate::menus::Entry::Sub { label, items } => {
                    if !varos_app::i18n::contains(label) {
                        missing.push((*label).into());
                    }
                    visit(items, missing);
                }
                _ => {}
            }
        }
    }
    let mut missing = vec![];
    for (title, entries) in crate::chrome::menus() {
        if title != "Varos" && !varos_app::i18n::contains(title) {
            missing.push(title.into());
        }
        visit(&entries, &mut missing);
    }
    for command in crate::command_registry::commands() {
        if !varos_app::i18n::contains(command.label) {
            missing.push(command.label.into());
        }
    }
    missing.sort();
    missing.dedup();
    assert!(missing.is_empty(), "missing Arabic catalog entries: {missing:#?}");
}
