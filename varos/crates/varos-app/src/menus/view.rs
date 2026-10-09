//! The View menu: zoom, rulers / guides, snapping rows.

use super::*;

pub(super) fn rows() -> Vec<Entry> {
    use KeyCode as K;
    vec![
        key("view.fitall", "Fit All", cmd_alt(K::Digit0)),
        key("view.fit", "Fit in Window", cmd(K::Digit0)),
        key("view.actual", "Actual Size", cmd(K::Digit1)),
        key("view.zoomin", "Zoom In", cmd(K::Equal)),
        key("view.zoomout", "Zoom Out", cmd(K::Minus)),
        Entry::Sep,
        key_check("view.rulers", "Rulers", cmd(K::KeyR), Check::Rulers),
        key_check("view.guides", "Guides", cmd(K::Semicolon), Check::Guides),
        key_check("view.lockguides", "Lock Guides", cmd_alt(K::Semicolon), Check::GuidesLocked),
        key("view.makeguides", "Make Guides", cmd(K::Digit5)),
        key("view.releaseguides", "Release Guides", cmd_alt(K::Digit5)),
        item(
            "view.clearguides",
            "Clear Guides",
            None,
            MenuCmd::View(varos_core::editor::view_commands::ViewAction::ClearGuides),
        ),
        key_check("view.grid", "Show Grid", cmd(K::Quote), Check::Grid),
        key_check("view.smart", "Smart Guides", cmd(K::KeyU), Check::SmartGuides),
        // 4b: the magnet's two guide rows have no other home once the band drops the magnet
        toggle("view.alignguides", "Alignment Guides", MenuCmd::Snap(SnapRow::AlignGuides), Check::AlignGuides),
        toggle("view.geomguides", "Geometric Guides", MenuCmd::Snap(SnapRow::GeomGuides), Check::GeomGuides),
        Entry::Sep,
        key_check("view.snapgrid", "Snap to Grid", cmd_shift(K::Quote), Check::SnapGrid),
        toggle("view.snappoint", "Snap to Point", MenuCmd::Snap(SnapRow::Point), Check::SnapPoint),
        Entry::Sep,
        Entry::Native(Native::Fullscreen),
    ]
}
