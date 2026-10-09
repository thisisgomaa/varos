//! Lane E host presentation helpers, GPU-independent.
/// Presentation owns canvas keys even when a now-hidden field or picker held focus.
pub fn presentation_key(ed: &mut varos_core::Editor, code: &str, ctrl: bool, shift: bool, alt: bool) -> bool {
    if !ed.view_depth.presentation {
        return false;
    }
    if code == "Escape" || (code == "KeyF" && shift && !ctrl && !alt) {
        ed.execute_ui(varos_core::EditCommand::View(varos_core::editor::view_commands::ViewAction::Depth(
            varos_core::view_depth::DepthAction::ExitPresentation,
        )));
    }
    true
}
pub fn canvas_rgba(rgb: [u8; 3]) -> [f32; 4] {
    [rgb[0] as f32 / 255.0, rgb[1] as f32 / 255.0, rgb[2] as f32 / 255.0, 1.0]
}
pub fn signature(scene: u64, rgb: [u8; 3]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    scene.hash(&mut h);
    rgb.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presentation_owns_edit_keys_and_escape() {
        let mut ed = varos_core::Editor::new();
        ed.view_depth.presentation = true;
        let before = ed.doc.clone();
        crate::apply_key(&mut ed, &mut varos_core::geom::View::identity(), [0.0, 0.0], "Delete", false, false, false);
        assert_eq!(before, ed.doc);
        assert!(presentation_key(&mut ed, "Escape", true, false, false));
        assert!(!ed.view_depth.presentation);
        assert!(!presentation_key(&mut ed, "Delete", false, false, false));
    }
    #[test]
    fn illustrator_mode_shortcuts_reach_commands() {
        let mut ed = varos_core::Editor::new();
        let mut view = varos_core::geom::View::identity();
        for (code, ctrl, shift, alt) in
            [("KeyY", true, false, false), ("KeyY", true, false, true), ("KeyF", false, true, false)]
        {
            crate::apply_key(&mut ed, &mut view, [0.0, 0.0], code, ctrl, shift, alt);
        }
        assert!(ed.view_depth.outline && ed.view_depth.pixel_preview && ed.view_depth.presentation);
        crate::apply_key(&mut ed, &mut view, [0.0, 0.0], "Escape", false, false, false);
        assert!(!ed.view_depth.presentation);
    }
}
