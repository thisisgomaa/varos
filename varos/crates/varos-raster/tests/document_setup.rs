//! The transparency grid and bleed guides are canvas aids, never page export pixels.
use std::sync::Arc;
#[test]
fn page_pixels_ignore_canvas_setup_aids() {
    let mut doc = varos_core::model::Document::default();
    doc.artboards.push(varos_core::model::Artboard { w: 50.0, h: 40.0, page_color: None, ..Default::default() });
    let off = varos_raster::rasterize_artboard(Arc::new(doc.clone()), 0, [100, 100]).unwrap();
    doc.transparency_grid = true;
    doc.artboards[0].bleed_edges = Some([1.0, 2.0, 3.0, 4.0]);
    doc.artboards[0].bleed = 4.0;
    let on = varos_raster::rasterize_artboard(Arc::new(doc), 0, [100, 100]).unwrap();
    assert_eq!(off.pixels, on.pixels);
    assert!(on.pixels.iter().all(|v| *v == 0));
}
