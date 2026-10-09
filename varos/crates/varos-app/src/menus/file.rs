//! The File menu (New / Open / Close / Save / Revert / Export) — every row a `MenuCmd::File` command,
//! on Illustrator's keys (slice 0.6: Close All ⌥⌘W, Save a Copy ⌥⌘S, Revert F12, Export Selection…).

use super::*;

pub(super) fn rows() -> Vec<Entry> {
    use KeyCode as K;
    vec![
        file_key("file.new", "New", cmd(K::KeyN), FileCmd::New),
        file_key("file.open", "Open\u{2026}", cmd(K::KeyO), FileCmd::Open),
        Entry::Sub { label: "Open Recent", items: vec![] },
        file_key("file.place.svg", "Place artwork\u{2026}", cmd_shift(K::KeyP), FileCmd::PlaceSvg),
        file_row("file.new-template", "New from Template…", FileCmd::NewTemplate),
        file_row("file.save-template", "Save as Template…", FileCmd::SaveTemplate),
        file_key("file.document-setup", "Document Setup…", cmd_alt(K::KeyP), FileCmd::DocumentSetup),
        Entry::Sep,
        file_key("file.close", "Close Tab", cmd(K::KeyW), FileCmd::CloseTab),
        file_key("file.closeall", "Close All", cmd_alt(K::KeyW), FileCmd::CloseAll),
        file_key("file.save", "Save", cmd(K::KeyS), FileCmd::Save),
        file_key("file.saveas", "Save As\u{2026}", cmd_shift(K::KeyS), FileCmd::SaveAs),
        file_key("file.savecopy", "Save a Copy\u{2026}", cmd_alt(K::KeyS), FileCmd::SaveCopy),
        file_key("file.revert", "Revert", fkey(K::F12), FileCmd::Revert),
        Entry::Sep,
        file_key("file.export", "Export\u{2026}", cmd_alt(K::KeyE), FileCmd::Export),
        Entry::Sub {
            label: "Export",
            items: vec![file_row("file.export.pdf", "PDF\u{2026}", FileCmd::ExportPdfPreset)],
        },
        file_row("file.exportselection", "Export Selection\u{2026}", FileCmd::ExportSelection),
        file_key("file.print", "Print…", cmd(K::KeyP), FileCmd::Print),
    ]
}
