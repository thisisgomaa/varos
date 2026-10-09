use std::sync::Arc;
use varos_core::{
    images::{self, ImageAffine, ImageEdit, Pixels},
    EditCommand, Editor,
};
fn place(ed: &mut Editor, colour: [u8; 4], at: [f32; 2]) -> u32 {
    let b =
        images::codec::encode_png(&Pixels { budget: None, width: 4, height: 4, rgba: Arc::from(colour.repeat(16)) })
            .unwrap();
    images::links::place_bytes(ed, &b, at, Some([at[0], at[1], 72., 72.]), Default::default(), None).unwrap().0
}
#[test]
fn rasterize_only_target_bakes_opacity_rotation_and_one_undo() {
    let mut ed = Editor::new();
    let id = place(&mut ed, [255, 0, 0, 255], [10., 20.]);
    place(&mut ed, [0, 0, 255, 255], [10., 20.]);
    let xf = ImageAffine { a: 0., b: 18., c: -18., d: 0., e: 82., f: 20. };
    ed.try_execute(EditCommand::Image(ImageEdit::Transform { id, xform: xf, opacity: 0.5 })).unwrap();
    let before = ed.doc.clone();
    let rev = ed.rev;
    varos_raster::images::rasterize_object(&mut ed, id, 72., None).unwrap();
    assert_eq!(ed.rev, rev + 1);
    let image = ed.doc.images.iter().find(|i| i.id == id).unwrap();
    assert_eq!(image.opacity, 1.);
    let pixels = &ed.blobs.get(&image.blob).unwrap().pixels;
    let p = &pixels.rgba[((36 * 72 + 36) * 4)..][..4];
    assert_eq!(p[0], 255);
    assert_eq!(p[2], 0);
    assert!((126..=129).contains(&p[3]));
    ed.undo();
    assert_eq!(ed.doc, before);
    ed.redo();
    assert_eq!(ed.doc.images.len(), 2);
}
#[test]
fn proxy_fitted_snapshot_and_crop_alpha() {
    let mut ed = Editor::new();
    let id = place(&mut ed, [255, 0, 0, 128], [10., 20.]);
    ed.try_execute(EditCommand::Image(ImageEdit::Crop { id, bounds: [10., 20., 36., 72.] })).unwrap();
    let raster = varos_raster::images::fitted(&ed.doc, &ed.blobs, [72, 72], None).unwrap();
    assert_eq!(&raster.pixels[(36 * 72 + 36) * 4..][..4], &[128, 0, 0, 128]);
    assert_eq!(&raster.pixels[(36 * 72 + 4) * 4..][..4], &[0; 4]);
}
#[test]
fn board_clip_and_crop_mask_both_cut_image_pixels() {
    let mut ed = Editor::new();
    let id = place(&mut ed, [255, 0, 0, 255], [0.; 2]);
    ed.doc.artboards.push(varos_core::model::Artboard {
        w: 24.,
        h: 24.,
        clip: true,
        page_color: None,
        ..Default::default()
    });
    ed.doc.assign_artboard_ids();
    ed.try_execute(EditCommand::Image(ImageEdit::Crop { id, bounds: [0., 0., 48., 16.] })).unwrap();
    let raster = varos_raster::images::rasterize_with_images(&ed.doc, &ed.blobs, [72, 72], [0.; 2], 1., None).unwrap();
    let pixel = |x: usize, y: usize| &raster.pixels[(y * 72 + x) * 4..][..4];
    assert_eq!(pixel(8, 8), &[255, 0, 0, 255]);
    assert_eq!(pixel(32, 8), &[0; 4]);
    assert_eq!(pixel(8, 20), &[0; 4]);
}
