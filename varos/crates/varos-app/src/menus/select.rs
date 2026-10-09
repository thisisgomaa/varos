//! Adapted from VectorCraft engine/src/cmd/select.rs:22-145 @ a469568 (MIT OR Apache-2.0).
use super::*;
use varos_core::editor::wave::{Same, Selection};
pub(super) fn rows() -> Vec<Entry> {
    use KeyCode as K;
    vec![
        key("select.all", "All", cmd(K::KeyA)),
        key("select.deselect", "Deselect", cmd_shift(K::KeyA)),
        key("select.reselect", "Reselect", cmd(K::Digit6)),
        item("select.inverse", "Inverse", None, MenuCmd::Selection(Selection::Inverse)),
        key("select.above", "Next Object Above", cmd_alt(K::BracketRight)),
        key("select.below", "Next Object Below", cmd_alt(K::BracketLeft)),
        key("select.artboard", "All on Active Artboard", cmd_alt(K::KeyA)),
        Entry::Sub {
            label: "Same",
            items: [
                ("select.same.fill", "Fill Color", Same::Fill),
                ("select.same.fillstroke", "Fill & Stroke", Same::FillStroke),
                ("select.same.stroke", "Stroke Color", Same::Stroke),
                ("select.same.weight", "Stroke Weight", Same::StrokeWeight),
                ("select.same.opacity", "Opacity", Same::Opacity),
                ("select.same.appearance", "Appearance", Same::Appearance),
            ]
            .into_iter()
            .map(|(id, label, mode)| item(id, label, None, MenuCmd::Selection(Selection::Same(mode))))
            .collect(),
        },
    ]
}
