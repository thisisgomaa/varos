//! Native-menu presentation adapter; no actions, shortcuts or document access.
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSMenu};
use objc2_foundation::NSString;
use vectorcraft_ui_egui::native_menu::{MenuBar, Node};

pub(crate) fn decorate(bar: &MenuBar) {
    let Some(main_thread) = MainThreadMarker::new() else { return };
    let Some(menu) = NSApplication::sharedApplication(main_thread).mainMenu() else { return };
    // muda creates new NSMenu descendants when attaching a submenu. Only the installed
    // tree is authoritative; modifying Submenu::ns_menu() would decorate a separate copy.
    for (item, model) in menu.itemArray().into_iter().zip(&bar.menus) {
        if let Some(submenu) = item.submenu() {
            decorate_children(&submenu, &model.children);
        }
    }
}

fn decorate_children(menu: &NSMenu, nodes: &[Node]) {
    for (item, node) in menu.itemArray().into_iter().zip(nodes) {
        if let Some(tip) = vectorcraft_ui_egui::native_menu::soon_tooltip(node) {
            item.setToolTip(Some(&NSString::from_str(&tip)));
            item.setEnabled(false);
        }
        if let Node::Submenu { children, .. } = node
            && let Some(submenu) = item.submenu()
        {
            decorate_children(&submenu, children);
        }
    }
}
