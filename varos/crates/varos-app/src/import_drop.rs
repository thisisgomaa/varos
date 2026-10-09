//! Finder dragging need not emit CursorMoved. Query AppKit's current client-view position at drop.
use varos_core::geom::Pt;
#[cfg(target_os = "macos")]
pub fn position(window: &winit::window::Window, fallback: Pt) -> Pt {
    use objc2_app_kit::NSView;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let point = (|| {
        let RawWindowHandle::AppKit(handle) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        // SAFETY: winit owns the live NSView; this runs on its main-thread window-event handler.
        let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        let native = view.window()?;
        let p = view.convertPoint_fromView(native.mouseLocationOutsideOfEventStream(), None);
        let y = if view.isFlipped() { p.y } else { view.bounds().size.height - p.y };
        let scale = window.scale_factor();
        Some([(p.x * scale) as f32, (y * scale) as f32])
    })();
    point.unwrap_or(fallback)
}
#[cfg(not(target_os = "macos"))]
pub fn position(_: &winit::window::Window, fallback: Pt) -> Pt {
    fallback
}
pub fn on_canvas(point: Pt, canvas: egui::Rect) -> bool {
    point.iter().all(|v| v.is_finite()) && canvas.contains(egui::pos2(point[0], point[1]))
}
#[cfg(test)]
mod tests {
    #[test]
    fn drop_admission_uses_canvas_not_chrome() {
        let canvas = egui::Rect::from_min_max(egui::pos2(20., 40.), egui::pos2(400., 300.));
        assert!(super::on_canvas([200., 100.], canvas));
        assert!(!super::on_canvas([10., 10.], canvas));
        assert!(!super::on_canvas([f32::NAN, 100.], canvas));
    }
}
