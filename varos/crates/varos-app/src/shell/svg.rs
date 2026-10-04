//! The single SVG rasterizer: Lucide/UI icons and cursor bitmaps go through `render_svg` (resvg →
//! straight-alpha RGBA); callers upload the result as an egui texture or a platform cursor.
use resvg::{tiny_skia, usvg};

/// tiny-skia renders premultiplied RGBA; every cursor consumer (the Win32 DIB builder, winit
/// `CustomCursor`, the macOS PNG → NSBitmapImageRep path) takes straight alpha. In place.
pub fn unpremultiply(d: &mut [u8]) {
    for px in d.chunks_mut(4) {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            px[0] = ((px[0] as u32 * 255) / a) as u8;
            px[1] = ((px[1] as u32 * 255) / a) as u8;
            px[2] = ((px[2] as u32 * 255) / a) as u8;
        }
    }
}

/// Render an arbitrary SVG string to straight-alpha RGBA, fit into a `size`×`size` box.
/// Used for the UI icons and the dev `--preview` mode. `force_black` recolors
/// everything to solid black (many icon sets use currentColor / theme fills).
pub fn render_svg(svg: &str, size: u32, force_black: bool) -> Option<(Vec<u8>, u32, u32)> {
    let svg = if force_black {
        // strip explicit fills/strokes so our wrapper color wins; cheap textual nudge
        svg.replace("currentColor", "#000").replace("fill=\"none\"", "")
    } else {
        svg.to_string()
    };
    let wrapped = if force_black {
        format!("<g fill=\"#000\" stroke=\"none\">{}</g>", inner_of(&svg)).replacen(
            "<g",
            &format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{}\"><g", viewbox_of(&svg)),
            1,
        ) + "</svg>"
    } else {
        svg.clone()
    };
    let tree = usvg::Tree::from_str(&wrapped, &usvg::Options::default())
        .or_else(|_| usvg::Tree::from_str(&svg, &usvg::Options::default()))
        .ok()?;
    let ts = tree.size();
    let sc = (size as f32 / ts.width().max(ts.height())).max(0.01);
    let (w, h) = (((ts.width() * sc).ceil() as u32).max(1), ((ts.height() * sc).ceil() as u32).max(1));
    let mut pm = tiny_skia::Pixmap::new(w, h)?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(sc, sc), &mut pm.as_mut());
    let mut d = pm.data().to_vec();
    unpremultiply(&mut d);
    Some((d, w, h))
}
fn viewbox_of(svg: &str) -> String {
    if let Some(i) = svg.find("viewBox=\"") {
        let r = &svg[i + 9..];
        if let Some(j) = r.find('"') {
            return r[..j].to_string();
        }
    }
    "0 0 24 24".into()
}
fn inner_of(svg: &str) -> String {
    if let (Some(a), Some(b)) = (svg.find('>'), svg.rfind("</svg>")) {
        svg[a + 1..b].to_string()
    } else {
        svg.to_string()
    }
}
