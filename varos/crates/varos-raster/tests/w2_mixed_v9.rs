//! Integration w2 review regressions on the frozen mixed format-9 document (image + gradient +
//! swatch + Live Corners + text): CPU pixels and advanced SVG options for image documents.
use std::sync::{atomic::AtomicBool, Arc};
use varos_core::format::Limits;

const VRS: &[u8] = include_bytes!("../../varos-core/tests/fixtures/v9/mixed.vrs");
const BG: [f32; 3] = [20. / 255., 19. / 255., 19. / 255.];

fn near(a: [f32; 4], b: [f32; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 0.02)
}

/// CPU raster pixels: the placed image is red, the rounded corner (40,40) stays canvas, the gradient
/// interior is painted, and the outlined text draws ink near its baseline.
#[test]
fn mixed_v9_cpu_pixels_show_image_rounded_gradient_and_text() {
    let loaded = varos_pdf::load_vrs_bytes(VRS, &Limits::DEFAULT).unwrap();
    let raster = varos_raster::rasterize_canvas_with_images(&loaded.doc, &loaded.blobs, [200, 200], [0., 0.], 1.)
        .into_result()
        .unwrap();
    let px = |x: f32, y: f32| raster.sample([x, y]).unwrap();
    let red = px(11., 11.);
    assert!(red[0] > 0.9 && red[1] < 0.1 && red[2] < 0.1, "image pixel {red:?}");
    assert!(near(px(40.6, 40.6), BG), "rounded corner is not painted: {:?}", px(40.6, 40.6));
    assert!(!near(px(41.0, 70.0), BG), "the straight left edge is painted");
    // The default gradient runs white (x=0) to black (x=100): the interior at x=50 is mid grey.
    let mid = px(50., 70.);
    assert!(mid[0] > 0.3 && mid[0] < 0.7 && !near(mid, BG), "gradient interior is painted: {mid:?}");
    // "Varos v9" sits on the baseline y=160 around x=20 (default alignment); its black outlines ink there.
    let ink = (140..162)
        .flat_map(|y| (0..40).map(move |x| (x, y)))
        .filter(|&(x, y)| px(x as f32 + 0.5, y as f32 + 0.5)[0] < 0.05)
        .count();
    assert!(ink > 20, "text outlines draw ink ({ink} px)");
    let blank = (165..200)
        .flat_map(|y| (0..200).map(move |x| (x, y)))
        .all(|(x, y)| near(px(x as f32 + 0.5, y as f32 + 0.5), BG));
    assert!(blank, "nothing paints below the text");
}

/// P2: advanced SVG options (precision, minify, ids) apply to image documents too.
#[test]
fn advanced_svg_options_apply_to_image_documents() {
    let loaded = varos_pdf::load_vrs_bytes(VRS, &Limits::DEFAULT).unwrap();
    let doc = Arc::new(loaded.doc);
    let asset = varos_raster::export::plan(&doc, &varos_raster::export::Scope::WholeBoard).unwrap().remove(0);
    let options = varos_raster::export::Options { format: varos_raster::export::Format::Svg, ..Default::default() };
    let svg_options = varos_core::svg::options::Options { decimals: 0, ids: false, minify: true, ..Default::default() };
    let out = varos_raster::export::encode_with_images_and_svg_options(
        &asset,
        &options,
        &loaded.blobs,
        &AtomicBool::new(false),
        &svg_options,
    )
    .unwrap();
    let text = String::from_utf8(out.bytes).unwrap();
    assert!(text.contains("<image "), "image kept");
    assert!(!text.contains('\n'), "minified");
    assert!(!text.contains(" id=\"path-"), "path ids stripped");
    let d = &text[text.find(" d=\"").unwrap() + 4..];
    let d = &d[..d.find('"').unwrap()];
    assert!(!d.contains('.'), "0-decimal precision applied: {d}");
}
