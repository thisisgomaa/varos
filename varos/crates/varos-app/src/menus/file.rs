//! The File menu (New / Open / Close / Save / Revert / Export) — every row a `MenuCmd::File` command,
//! on Illustrator's keys (slice 0.6: Close All ⌥⌘W, Save a Copy ⌥⌘S, Revert F12, Export Selection…).

use super::*;

pub(super) fn rows() -> Vec<Entry> {
    use KeyCode as K;
    vec![
        file_key("file.new", "New", cmd(K::KeyN), FileCmd::New),
        file_key("file.open", "Open\u{2026}", cmd(K::KeyO), FileCmd::Open),
        Entry::Sub { label: "Open Recent", items: vec![] },
        file_row("file.place.svg", "Place SVG\u{2026}", FileCmd::PlaceSvg),
        Entry::Sep,
        file_key("file.close", "Close Tab", cmd(K::KeyW), FileCmd::CloseTab),
        file_key("file.closeall", "Close All", cmd_alt(K::KeyW), FileCmd::CloseAll),
        file_key("file.save", "Save", cmd(K::KeyS), FileCmd::Save),
        file_key("file.saveas", "Save As\u{2026}", cmd_shift(K::KeyS), FileCmd::SaveAs),
        file_key("file.savecopy", "Save a Copy\u{2026}", cmd_alt(K::KeyS), FileCmd::SaveCopy),
        file_key("file.revert", "Revert", fkey(K::F12), FileCmd::Revert),
        Entry::Sep,
        Entry::Sub { label: "Export", items: vec![file_row("file.export.pdf", "PDF\u{2026}", FileCmd::Export)] },
        file_row("file.exportselection", "Export Selection\u{2026}", FileCmd::ExportSelection),
    ]
}
