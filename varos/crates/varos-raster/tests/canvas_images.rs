//! Integration w2 (view × images): the Navigator / eyedropper canvas raster sees placed images.
use std::sync::Arc;
use varos_core::{images::*, Editor};

#[test]
fn canvas_raster_draws_placed_images_with_lent_resources() {
    let mut ed = Editor::new();
    let png =
        codec::encode_png(&Pixels { budget: None, width: 2, height: 2, rgba: Arc::from([255, 0, 0, 255].repeat(4)) })
            .unwrap();
    links::place_bytes(&mut ed, &png, [0., 0.], None, PlacementMode::Embed, None).unwrap();
    // Without resources the image is a missing-resource error, never silently blank.
    assert!(varos_raster::rasterize_canvas(&ed.doc, [8, 8], [0., 0.], 1.).into_result().is_err());
    let raster =
        varos_raster::rasterize_canvas_with_images(&ed.doc, &ed.blobs, [8, 8], [0., 0.], 1.).into_result().unwrap();
    let c = raster.sample([0.5, 0.5]).unwrap();
    assert!(c[0] > 0.9 && c[1] < 0.1, "{c:?}");
}
