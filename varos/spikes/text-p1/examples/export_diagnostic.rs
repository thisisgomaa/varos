//! Diagnostic only: distinguishes path/export geometry from the proposed max-coverage policy.
use varos_text_spike::{outlines::*, *};
fn main() {
    let path = std::env::args().nth(1).expect("PNG output");
    let layout = Engine::default().layout(&Request::new("Logo شعار ۶ السَّلَامُ", 48., Some(600.))).unwrap();
    let mut pixmap = tiny_skia::Pixmap::new(5600, 1200).unwrap();
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);
    for outline in layout_outlines(&layout).unwrap() {
        if let Some(path) = outline.path() {
            pixmap.fill_path(
                &path,
                &paint,
                tiny_skia::FillRule::Winding,
                tiny_skia::Transform::from_scale(8., 8.),
                None,
            );
        }
    }
    proof::downsample_4x(&pixmap).unwrap().save_png(path).unwrap();
}
