//! Integration w3: the frozen mixed format-14 document through the CPU raster and SVG writers
//! (image, gradient + corners + appearance stack + live effect, CMYK swatch, live Repeat, styled
//! area text). Pixels are checked on the CPU canvas; the SVG must carry the image and gradient and render.
use std::sync::atomic::AtomicBool;
use varos_core::format::Limits;

const VRS: &[u8] = include_bytes!("../../varos-core/tests/fixtures/v14-mixed/mixed.vrs");
const BG: [f32; 3] = [20. / 255., 19. / 255., 19. / 255.];

fn near(a: [f32; 4], b: [f32; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 0.02)
}

#[test]
fn mixed_v14_cpu_pixels_show_every_wave_three_feature() {
    let loaded = varos_pdf::load_vrs_bytes(VRS, &Limits::DEFAULT).unwrap();
    let raster = varos_raster::rasterize_canvas_with_images(&loaded.doc, &loaded.blobs, [240, 280], [0., 0.], 1.)
        .into_result()
        .unwrap();
    let px = |x: f32, y: f32| raster.sample([x, y]).unwrap();
    let red = px(11., 11.);
    assert!(red[0] > 0.9 && red[1] < 0.1 && red[2] < 0.1, "image pixel {red:?}");
    // The default gradient runs white (x=0) to black (x=100) in page space: x=50 is mid grey.
    let mid = px(50., 70.);
    assert!(!near(mid, BG) && mid[0] > 0.2 && mid[0] < 0.8, "gradient interior under the stack: {mid:?}");
    let cyan = px(190., 70.);
    assert!(cyan[0] < 0.35 && cyan[2] > 0.5, "CMYK swatch previews as cyan: {cyan:?}");
    let green = px(60., 135.);
    assert!(green[1] > 0.4 && green[0] < 0.2, "live repeat source: {green:?}");
    let ink = (175..255)
        .flat_map(|y| (25..215).map(move |x| (x, y)))
        .filter(|&(x, y)| px(x as f32 + 0.5, y as f32 + 0.5)[0] < 0.3)
        .count();
    assert!(ink > 20, "area text draws ink inside its shape ({ink} px)");
    // the live node evaluates to more paths than it authors (the mirrored copy)
    let evaluated = varos_core::live::evaluated_document(&loaded.doc).unwrap().expect("live node");
    assert!(evaluated.paths.len() > loaded.doc.paths.len());
}

#[test]
fn mixed_v14_svg_carries_image_and_gradient_and_renders() {
    let loaded = varos_pdf::load_vrs_bytes(VRS, &Limits::DEFAULT).unwrap();
    let outlined = varos_text_layout::outline_document(&loaded.doc).unwrap();
    let plan = varos_core::svg::plan_svg_export(&outlined, varos_core::svg::ExportScope::WholeBoard).unwrap();
    let (files, _) =
        varos_core::images::svg::export(&outlined, &loaded.blobs, &plan, false, &AtomicBool::new(false)).unwrap();
    let svg = std::str::from_utf8(&files[0].bytes).unwrap();
    assert!(svg.contains("<image "), "image element");
    assert!(svg.contains("linearGradient"), "gradient definition");
    let tree = resvg::usvg::Tree::from_data(&files[0].bytes, &Default::default()).unwrap();
    let size = tree.size().to_int_size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height()).unwrap();
    resvg::render(&tree, resvg::tiny_skia::Transform::identity(), &mut pixmap.as_mut());
    assert!(pixmap.pixels().iter().any(|p| p.alpha() > 0), "the SVG renders");
}
