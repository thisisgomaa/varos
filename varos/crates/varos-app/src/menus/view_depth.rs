//! Lane E View-menu mirrors of command-backed view modes.
use super::*;
use varos_core::{
    editor::view_commands::ViewAction,
    view_depth::{DepthAction as D, DepthCheck as C},
};
pub(super) fn rows() -> Vec<Entry> {
    let mut rows = Vec::new();
    for (id, label, action, check, accel) in [
        ("view.outline", "Outline", D::Outline, C::Outline, cmd(KeyCode::KeyY)),
        ("view.pixelpreview", "Pixel Preview", D::PixelPreview, C::PixelPreview, cmd_alt(KeyCode::KeyY)),
        ("view.snappixel", "Snap to Pixel", D::SnapPixel, C::SnapPixel, None),
        ("view.movepixel", "Move Whole Pixels", D::MoveWholePixel, C::MoveWholePixel, None),
        ("view.trim", "Trim View", D::Trim, C::Trim, None),
        (
            "view.presentation",
            "Presentation Mode",
            D::Presentation,
            C::Presentation,
            Some(Accel { code: KeyCode::KeyF, shift: true, alt: false, cmd: false }),
        ),
    ] {
        rows.push(Entry::Item {
            id: id.into(),
            label,
            accel,
            cmd: MenuCmd::View(ViewAction::Depth(action)),
            check: Some(Check::Depth(check)),
        });
    }
    rows.push(Entry::Item {
        id: "view.transparency".into(),
        label: "Transparency Grid",
        accel: None,
        cmd: MenuCmd::View(ViewAction::Depth(D::TransparencyGrid)),
        check: Some(Check::Depth(C::TransparencyGrid)),
    });
    for (id, label, rgb) in [
        ("view.canvas.dark", "Canvas Colour: Dark", varos_app::shell::tokens::CANVAS_DARK),
        ("view.canvas.mid", "Canvas Colour: Medium", varos_app::shell::tokens::CANVAS_MID),
        ("view.canvas.light", "Canvas Colour: Light", varos_app::shell::tokens::CANVAS_LIGHT),
    ] {
        rows.push(item(id, label, None, MenuCmd::View(ViewAction::Depth(D::CanvasColor { rgb }))));
    }
    rows
}
