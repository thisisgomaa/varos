//! Phase 10 native mirrors of the provisional effect sheets.
use super::{Accel, Entry, MenuCmd};
use winit::keyboard::KeyCode;
fn row(id: &str, label: &'static str, accel: Option<Accel>) -> Entry {
    Entry::Item { id: id.into(), label, accel, cmd: MenuCmd::LaneC(label), check: None }
}
pub(super) fn rows() -> Vec<Entry> {
    vec![
        row(
            "effect.apply_last",
            "Apply Last Effect",
            Some(Accel { code: KeyCode::KeyE, shift: true, alt: false, cmd: true }),
        ),
        row("effect.last", "Last Effect…", Some(Accel { code: KeyCode::KeyE, shift: true, alt: true, cmd: true })),
        Entry::Sep,
        Entry::Sub {
            label: "Distort & Transform",
            items: vec![
                row("effect.offset", "Offset…", None),
                row("effect.zigzag", "Zig Zag…", None),
                row("effect.transform", "Transform Effect…", None),
            ],
        },
        Entry::Sub { label: "Warp", items: crate::ui::effects_menu_rows() },
        row("effect.width", "Width Tool", Some(Accel { code: KeyCode::KeyW, shift: true, alt: false, cmd: false })),
        row("effect.profile", "Width Profile…", None),
    ]
}
