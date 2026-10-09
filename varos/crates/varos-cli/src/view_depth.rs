//! Lane E headless command inspection. Attached desktop edits use `bridge edit` with verb `view`.
use serde_json::{json, Value};
use std::{ffi::OsString, io::Read, path::PathBuf};
use varos_core::{editor::view_commands::ViewAction, view_depth::DepthAction, EditCommand, Editor};
pub fn inspect(ed: &mut Editor, actions: Vec<DepthAction>) -> Result<Value, String> {
    if actions.len() > 100 {
        return Err("view batch exceeds 100 actions".into());
    }
    for action in &actions {
        varos_core::view_depth::validate(ed, *action)?;
    }
    for action in actions {
        ed.try_execute(EditCommand::View(ViewAction::Depth(action)))?;
    }
    Ok(
        json!({"outline":ed.view_depth.outline,"pixel_preview":ed.view_depth.pixel_preview,"trim":ed.view_depth.trim,"presentation":ed.view_depth.presentation,"outline_nodes":ed.view_depth.outline_nodes,"pixel_step_pt":varos_core::view_depth::pixel_step(ed.doc.units.ppi),"snap_pixel":ed.doc.snap.force_pixel_align,"move_whole_pixel":ed.doc.snap.move_whole_px,"navigator_bounds":varos_core::view_depth::navigator_bounds(ed),"requested_center":ed.requested_pan,"requested_zoom_percent":ed.requested_zoom,"requested_canvas_color":ed.requested_canvas,"transparency_grid":ed.doc.transparency_grid}),
    )
}
pub fn run(args: Vec<OsString>) -> Result<Value, String> {
    if args.len() != 3 || args[1] != "--batch" {
        return Err("view-depth FILE.vrs --batch DEPTH_ACTIONS.json (read-only inspection)".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(PathBuf::from(&args[2]))
        .map_err(|e| e.to_string())?
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err("view batch exceeds 64 KiB".into());
    }
    let actions = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let mut ed = Editor::new();
    ed.replace_doc(varos_pdf::load_vrs(&PathBuf::from(&args[0]))?);
    inspect(&mut ed, actions)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn headless_view_actions_have_observable_output() {
        let mut ed = Editor::new();
        ed.doc.units.ppi = 144.0;
        let v = inspect(
            &mut ed,
            vec![
                DepthAction::Outline,
                DepthAction::PixelPreview,
                DepthAction::SnapPixel,
                DepthAction::NavigatorPan { center: [10.0, 20.0] },
            ],
        )
        .unwrap();
        assert_eq!(v["pixel_step_pt"], 0.5);
        assert_eq!(v["outline"], true);
        assert_eq!(v["requested_center"], json!([10.0, 20.0]));
    }
}
