//! Data adapted from VectorCraft tools/src/catalog.rs:20-140 @ a469568
//! (MIT OR Apache-2.0), plus Illustrator's default menu shortcuts.
//! Primary means Command on macOS / Control on Windows. No new chord is invented.
#[derive(Clone, Copy)]
pub struct Binding {
    pub key: &'static str,
    pub primary: bool,
    pub shift: bool,
    pub alt: bool,
    pub illustrator: &'static str,
}
macro_rules! b {
    ($key:literal,$p:literal,$s:literal,$a:literal,$name:literal) => {
        Binding { key: $key, primary: $p, shift: $s, alt: $a, illustrator: $name }
    };
}
pub const BINDINGS: &[Binding] = &[
    b!("KeyN", true, false, false, "New"),
    b!("KeyO", true, false, false, "Open"),
    b!("KeyS", true, false, false, "Save"),
    b!("KeyS", true, true, false, "Save As"),
    b!("KeyS", true, false, true, "Save a Copy"),
    b!("F12", false, false, false, "Revert"),
    b!("KeyW", true, false, false, "Close"),
    b!("KeyW", true, false, true, "Close All"),
    b!("KeyQ", true, false, false, "Quit"),
    b!("KeyZ", true, false, false, "Undo"),
    b!("KeyZ", true, true, false, "Redo"),
    b!("KeyC", true, false, false, "Copy"),
    b!("KeyX", true, false, false, "Cut"),
    b!("KeyV", true, false, false, "Paste"),
    b!("KeyV", true, true, false, "Paste in Place"),
    b!("KeyA", true, false, false, "All"),
    b!("KeyA", true, true, false, "Deselect"),
    b!("KeyA", true, false, true, "All on Active Artboard"),
    b!("Digit6", true, false, false, "Reselect"),
    b!("BracketRight", true, false, true, "Next Object Above"),
    b!("BracketLeft", true, false, true, "Next Object Below"),
    b!("KeyD", true, false, false, "Transform Again"),
    b!("BracketRight", true, true, false, "Bring to Front"),
    b!("BracketRight", true, false, false, "Bring Forward"),
    b!("BracketLeft", true, false, false, "Send Backward"),
    b!("BracketLeft", true, true, false, "Send to Back"),
    b!("KeyG", true, false, false, "Group"),
    b!("KeyG", true, true, false, "Ungroup"),
    b!("Digit2", true, false, false, "Lock Selection"),
    b!("Digit2", true, false, true, "Unlock All"),
    b!("Digit3", true, false, false, "Hide Selection"),
    b!("Digit3", true, false, true, "Show All"),
    b!("KeyJ", true, false, false, "Join"),
    b!("KeyJ", true, false, true, "Average"),
    b!("Digit8", true, false, false, "Make Compound Path"),
    b!("Digit8", true, true, true, "Release Compound Path"),
    b!("Digit0", true, false, true, "Fit All"),
    b!("Digit0", true, false, false, "Fit Artboard"),
    b!("Digit5", true, false, false, "Make Guides"),
    b!("Digit5", true, false, true, "Release Guides"),
    b!("Quote", true, false, false, "Show Grid"),
    b!("Quote", true, true, false, "Snap to Grid"),
    b!("KeyZ", false, false, false, "Zoom"),
    b!("KeyH", false, false, false, "Hand"),
    b!("Numpad0", true, false, false, "Fit Artboard"),
    b!("Digit1", true, false, false, "Actual Size"),
    b!("Equal", true, false, false, "Zoom In"),
    b!("Equal", true, true, false, "Zoom In"),
    b!("NumpadAdd", true, false, false, "Zoom In"),
    b!("Minus", true, false, false, "Zoom Out"),
    b!("NumpadSubtract", true, false, false, "Zoom Out"),
    b!("KeyR", true, false, false, "Rulers"),
    b!("Semicolon", true, false, false, "Guides"),
    b!("Semicolon", true, false, true, "Lock Guides"),
    b!("KeyU", true, false, false, "Smart Guides"),
    b!("KeyV", false, false, false, "Selection"),
    b!("KeyA", false, false, false, "Direct Selection"),
    b!("KeyP", false, false, false, "Pen"),
    b!("KeyQ", false, false, false, "Lasso"),
    b!("KeyC", false, true, false, "Anchor Point"),
    b!("Equal", false, true, false, "Add Anchor"),
    b!("Equal", false, false, false, "Add Anchor"),
    b!("NumpadAdd", false, false, false, "Add Anchor"),
    b!("Minus", false, false, false, "Delete Anchor"),
    b!("NumpadSubtract", false, false, false, "Delete Anchor"),
    b!("KeyM", false, false, false, "Rectangle"),
    b!("KeyL", false, false, false, "Ellipse"),
    b!("KeyR", false, false, false, "Rotate"),
    b!("KeyS", false, false, false, "Scale"),
    b!("KeyI", false, false, false, "Eyedropper"),
    b!("KeyO", false, true, false, "Artboard"),
    b!("KeyX", false, false, false, "Toggle Fill/Stroke"),
    b!("KeyX", false, true, false, "Swap Fill/Stroke"),
    b!("KeyD", false, false, false, "Default Paint"),
    b!("Slash", false, false, false, "None Paint"),
    b!("Tab", true, false, false, "Next Document"),
    b!("Tab", true, true, false, "Previous Document"),
    b!("Space", false, false, false, "Temporary Hand"),
    b!("Escape", false, false, false, "Deselect/Cancel"),
    b!("Enter", false, false, false, "Finish"),
    b!("Delete", false, false, false, "Delete"),
    b!("Backspace", false, false, false, "Delete"),
    b!("ArrowLeft", false, false, false, "Nudge Left"),
    b!("ArrowRight", false, false, false, "Nudge Right"),
    b!("ArrowUp", false, false, false, "Nudge Up"),
    b!("ArrowDown", false, false, false, "Nudge Down"),
    b!("ArrowLeft", false, true, false, "Nudge Left ×10"),
    b!("ArrowRight", false, true, false, "Nudge Right ×10"),
    b!("ArrowUp", false, true, false, "Nudge Up ×10"),
    b!("ArrowDown", false, true, false, "Nudge Down ×10"),
];
pub fn is_bound(key: &str, primary: bool, shift: bool, alt: bool) -> bool {
    BINDINGS
        .iter()
        .any(|b| !b.illustrator.is_empty() && (b.key, b.primary, b.shift, b.alt) == (key, primary, shift, alt))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_menu_chord_has_the_same_illustrator_binding() {
        for row in crate::menus::flat_items(&crate::menus::menus()) {
            if let crate::menus::Entry::Item { id, accel: Some(a), label, .. } = row {
                let code = format!("{:?}", a.code);
                let binding = BINDINGS
                    .iter()
                    .find(|b| (b.key, b.primary, b.shift, b.alt) == (code.as_str(), a.cmd, a.shift, a.alt))
                    .unwrap_or_else(|| panic!("{id} missing {code}"));
                assert!(!binding.illustrator.is_empty(), "{label}");
            }
        }
    }
    #[test]
    fn every_lifecycle_and_tab_binding_has_a_parity_row() {
        use winit::keyboard::KeyCode as K;
        for code in [K::KeyN, K::KeyO, K::KeyS, K::KeyW, K::KeyQ, K::F12, K::Tab] {
            for primary in [false, true] {
                for shift in [false, true] {
                    for alt in [false, true] {
                        if crate::host::lifecycle_key(code, primary, shift, alt).is_some()
                            || crate::host::tab_key(code, primary, shift, alt).is_some()
                        {
                            assert!(is_bound(&format!("{code:?}"), primary, shift, alt));
                        }
                    }
                }
            }
        }
        assert!(is_bound("Space", false, false, false));
    }
    #[test]
    fn bindings_are_unique_and_dispatch_requires_a_listed_chord() {
        let mut seen = std::collections::HashSet::new();
        for b in BINDINGS {
            assert!(seen.insert((b.key, b.primary, b.shift, b.alt)));
            assert!(is_bound(b.key, b.primary, b.shift, b.alt));
        }
        assert!(!is_bound("KeyY", true, false, false), "Illustrator reserves this for Outline, never Redo");
    }
}
