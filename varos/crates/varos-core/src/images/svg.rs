//! Portable SVG images use upright PNGs; original streams remain unchanged in the native file.
use super::*;
use crate::{
    model::{Document, GroupRole, NodeKind},
    svg::{ExportPlan, SvgFile},
    ExportNote, ExportReport,
};
use std::sync::atomic::{AtomicBool, Ordering};
pub fn export(
    doc: &Document,
    store: &BlobStore,
    plan: &ExportPlan,
    preview: bool,
    cancel: &AtomicBool,
) -> Result<(Vec<SvgFile>, ExportReport), String> {
    export_with_options(doc, store, plan, preview, cancel, None)
}
/// Integration w2 (review P1/P2): the image-aware writer follows the vector writer's traversal —
/// callers outline text first; Live Corners are resolved here; gradients and images then paint — and
/// Lane C's advanced SVG options (precision, styling, ids, minify) apply exactly as for vector files.
pub fn export_with_options(
    doc: &Document,
    store: &BlobStore,
    plan: &ExportPlan,
    preview: bool,
    cancel: &AtomicBool,
    options: Option<&crate::svg::options::Options>,
) -> Result<(Vec<SvgFile>, ExportReport), String> {
    if let Some(options) = options {
        options.validate()?;
    }
    let decimals = options.map(|o| o.decimals);
    crate::format::validate(doc, &crate::format::Limits::DEFAULT).map_err(|e| e.to_string())?;
    let resolved = crate::live_corners::document(doc);
    let doc = &resolved;
    let mut report = ExportReport { notes: super::export_notes(doc, store) };
    let mut files = Vec::new();
    for page in &plan.pages {
        let [x, y, w, h] = page.rect;
        if ![x, y, w, h].iter().all(|v| v.is_finite()) || w <= 0. || h <= 0. {
            return Err("Invalid SVG page".into());
        }
        let mut out = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"{x} {y} {w} {h}\">\n"
        );
        if let Some(bg) = page.background {
            out += &format!(
                "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" fill=\"rgb({},{},{})\" opacity=\"{}\"/>\n",
                bg[0] * 255.,
                bg[1] * 255.,
                bg[2] * 255.,
                bg[3]
            );
        }
        for kind in paint_order(doc) {
            if cancel.load(Ordering::Relaxed) {
                return Err("SVG export cancelled".into());
            }
            let id = match kind {
                NodeKind::Image(id) | NodeKind::Path(id) => id,
                _ => continue,
            };
            if doc.eff_hidden(id) || doc.is_mask_source(id) {
                continue;
            }
            let board_clip = if matches!(kind, NodeKind::Image(_)) { board_clips(doc, id) } else { None };
            if let Some(rects) = &board_clip {
                out += &format!("<defs><clipPath id=\"image-board-{id}\" clipPathUnits=\"userSpaceOnUse\">");
                for r in rects {
                    out += &format!(
                        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                        r.0,
                        r.1,
                        r.2 - r.0,
                        r.3 - r.1
                    );
                }
                out += &format!("</clipPath></defs><g clip-path=\"url(#image-board-{id})\">\n");
            }
            let mut clips = vec![];
            let mut node = doc.node_of_path(id);
            while let Some(n) = node {
                let Some(n) = doc.node(n) else { break };
                if n.role == GroupRole::Clip {
                    clips.push(n.id);
                }
                node = n.parent;
            }
            for &clip in &clips {
                let data = crate::svg::image_clip_data(doc, clip);
                out+=&format!("<defs><clipPath id=\"image-clip-{clip}-{id}\" clipPathUnits=\"userSpaceOnUse\"><path d=\"{data}\" clip-rule=\"evenodd\"/></clipPath></defs><g clip-path=\"url(#image-clip-{clip}-{id})\">\n");
            }
            match kind {
                NodeKind::Path(id) => {
                    crate::svg::paint_image_companion(&mut out, doc, id, decimals).map_err(|e| e.to_string())?
                }
                NodeKind::Image(id) => {
                    let image = doc.images.iter().find(|i| i.id == id).ok_or("Missing SVG image metadata")?;
                    let b = store.get(&image.blob).ok_or("Missing SVG original")?;
                    if (b.original.is_none() || b.pixels.width != b.meta.px_w || b.pixels.height != b.meta.px_h)
                        && !preview
                    {
                        return Err("Production SVG requires full image originals".into());
                    }
                    let bytes = codec::encode_png(&b.pixels)?;
                    let c = world_corners(doc, image);
                    let p = &b.pixels;
                    let a = (c[1][0] - c[0][0]) / p.width as f32;
                    let bb = (c[1][1] - c[0][1]) / p.width as f32;
                    let cc = (c[3][0] - c[0][0]) / p.height as f32;
                    let d = (c[3][1] - c[0][1]) / p.height as f32;
                    out+=&format!("<image id=\"image-{id}\" width=\"{}\" height=\"{}\" transform=\"matrix({a} {bb} {cc} {d} {} {})\" opacity=\"{}\" href=\"data:image/png;base64,{}\"/>\n",p.width,p.height,c[0][0],c[0][1],image.opacity,base64(&bytes));
                    if b.original.is_none() {
                        report.notes.push(ExportNote {
                            kind: "image_proxy".into(),
                            object_id: Some(id),
                            message: "Preview SVG uses a labelled proxy-only image".into(),
                        });
                    }
                }
                _ => {}
            }
            for _ in clips {
                out += "</g>\n";
            }
            if board_clip.is_some() {
                out += "</g>\n";
            }
        }
        out += "</svg>\n";
        if let Some(options) = options {
            out = options.apply(&out)?;
        }
        files.push(SvgFile { page: page.clone(), bytes: out.into_bytes() });
    }
    report.notes.push(ExportNote {
        kind: "image_rgb".into(),
        object_id: None,
        message: "Portable SVG embeds upright RGB PNG images; no ICC/CMYK proofing".into(),
    });
    Ok((files, report))
}
fn base64(bytes: &[u8]) -> String {
    const CH: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for p in bytes.chunks(3) {
        let a = p[0] as usize;
        let b = p.get(1).copied().unwrap_or(0) as usize;
        let c = p.get(2).copied().unwrap_or(0) as usize;
        out.push(CH[a >> 2] as char);
        out.push(CH[(a & 3) << 4 | b >> 4] as char);
        out.push(if p.len() > 1 { CH[(b & 15) << 2 | c >> 6] as char } else { '=' });
        out.push(if p.len() > 2 { CH[c & 63] as char } else { '=' });
    }
    out
}
