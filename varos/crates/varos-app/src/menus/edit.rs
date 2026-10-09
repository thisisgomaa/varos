//! The Edit menu: undo / redo, the in-app clipboard, selection.

use super::*;

pub(super) fn rows() -> Vec<Entry> {
    use KeyCode as K;
    vec![
        key("edit.undo", "Undo", cmd(K::KeyZ)),
        key("edit.redo", "Redo", cmd_shift(K::KeyZ)),
        Entry::Sep,
        // the in-app clipboard (Astra F04): ⌘C / ⌘X via `apply_key`, ⌘V / ⇧⌘V via the shortcut
        // path's view-centred paste. In a focused text field `forward_shortcut` turns these
        // into egui's own Copy / Cut / Paste events, so field editing keeps working.
        key("edit.cut", "Cut", cmd(K::KeyX)),
        key("edit.copy", "Copy", cmd(K::KeyC)),
        key("edit.paste", "Paste", cmd(K::KeyV)),
        key("edit.pasteinplace", "Paste in Place", cmd_shift(K::KeyV)),
        // click-only: the Delete/Backspace key path, with no key equivalent (so a text field
        // keeps its Backspace)
        item("edit.delete", "Delete", None, MenuCmd::Plain(K::Backspace)),
        Entry::Sep,
        // ⌘A / ⇧⌘A via `apply_key`; in a focused text field ⌘A is handed to the field (select text)
        key("edit.selectall", "Select All", cmd(K::KeyA)),
        key("edit.deselect", "Deselect", cmd_shift(K::KeyA)),
    ]
}
