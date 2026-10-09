//! The Window menu: the rail, the control bar, every dockable panel, the colour picker, Reset layout.

use super::*;

pub(super) fn rows() -> Vec<Entry> {
    let mut window = vec![
        Entry::Native(Native::Minimize),
        Entry::Native(Native::Zoom),
        Entry::Sep,
        toggle("win.rail", "Tool rail", MenuCmd::ToggleRail, Check::Rail),
        toggle("win.dock", "Control bar", MenuCmd::ToggleDock, Check::Dock),
        Entry::Sep,
    ];
    for p in PanelId::DOCKABLE {
        window.push(Entry::Item {
            id: format!("win.panel.{}", p.title()),
            label: p.title(),
            accel: None,
            cmd: MenuCmd::TogglePanel(p),
            check: Some(Check::Panel(p)),
        });
    }
    window.extend([
        toggle("win.colour-picker", "Colour", MenuCmd::TogglePicker, Check::Picker),
        Entry::Sep,
        item("win.reset-layout", "Reset layout", None, MenuCmd::ResetLayout),
        Entry::Sep,
        Entry::Native(Native::BringAllToFront),
    ]);
    window.push(Entry::Sub {
        label: "Layers",
        items: [
            "Release to Layers (Sequence)",
            "Release to Layers (Build)",
            "Collect in New Layer",
            "Merge Selected Layers",
            "Flatten Artwork",
            "Locate Object",
            "Hide Others",
            "Lock Others",
        ]
        .into_iter()
        .map(|label| Entry::Item {
            id: format!("4a.{label}"),
            label,
            accel: None,
            cmd: MenuCmd::Slice4a(label),
            check: None,
        })
        .collect(),
    });
    window
}
