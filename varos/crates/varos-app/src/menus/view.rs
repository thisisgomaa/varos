//! The View menu: zoom, rulers / guides, snapping rows.

use super::*;

pub(super) fn rows() -> Vec<Entry> {
    use KeyCode as K;
    vec![
        key("view.fit", "Fit in Window", cmd(K::Digit0)),
        key("view.actual", "Actual Size", cmd(K::Digit1)),
        key("view.zoomin", "Zoom In", cmd(K::Equal)),
        key("view.zoomout", "Zoom Out", cmd(K::Minus)),
        Entry::Sep,
        key_check("view.rulers", "Rulers", cmd(K::KeyR), Check::Rulers),
        key_check("view.guides", "Guides", cmd(K::Semicolon), Check::Guides),
        key_check("view.lockguides", "Lock Guides", cmd_alt(K::Semicolon), Check::GuidesLocked),
        key_check("view.smart", "Smart Guides", cmd(K::KeyU), Check::SmartGuides),
        // 4b: the magnet's two guide rows have no other home once the band drops the magnet
        toggle("view.alignguides", "Alignment Guides", MenuCmd::Snap(SnapRow::AlignGuides), Check::AlignGuides),
        toggle("view.geomguides", "Geometric Guides", MenuCmd::Snap(SnapRow::GeomGuides), Check::GeomGuides),
        Entry::Sep,
        toggle("view.snapgrid", "Snap to Grid", MenuCmd::Snap(SnapRow::Grid), Check::SnapGrid),
        toggle("view.snappoint", "Snap to Point", MenuCmd::Snap(SnapRow::Point), Check::SnapPoint),
        Entry::Sep,
        Entry::Native(Native::Fullscreen),
    ]
}
