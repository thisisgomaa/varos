//! Lane A: recursive Appearance/alpha-mask export with original editable curves.
use super::*;
use crate::{
    appearance_scene::paint_paths,
    model::{GroupRole, NodeKind},
};
#[allow(clippy::too_many_arguments)]
pub(super) fn write(
    out: &mut String,
    doc: &Document,
    page: &PageSpec,
    cancel: &AtomicBool,
    decimals: Option<u8>,
    blobs: Option<&crate::images::BlobStore>,
    preview: bool,
) -> Result<(), ExportError> {
    #[allow(clippy::too_many_arguments)]
    fn walk(
        out: &mut String,
        doc: &Document,
        nid: u32,
        page: &PageSpec,
        cancel: &AtomicBool,
        decimals: Option<u8>,
        serial: &mut u32,
        blobs: Option<&crate::images::BlobStore>,
        preview: bool,
    ) -> Result<(), ExportError> {
        cancelled(cancel)?;
        let Some(n) = doc.node(nid) else { return Ok(()) };
        if n.hidden {
            return Ok(());
        }
        match n.kind {
            NodeKind::Path(pid) => {
                let Some(pi) = doc.pidx(pid) else { return Ok(()) };
                let p = &doc.paths[pi];
                if p.hidden {
                    return Ok(());
                }
                if p.stack.is_empty() {
                    if let Some(d) = drawable(doc, pi, p) {
                        paint_drawn(out, &d, doc, decimals)?;
                    }
                } else {
                    out.push_str(&format!("<g opacity=\"{}\">\n", number(p.opacity, decimals)));
                    for mut entry in paint_paths(p) {
                        let Some(d) = drawable(doc, pi, &entry) else { continue };
                        *serial += 1;
                        let (xf, fill, stroke, pad, bbox, clip) = (d.xf, d.fill, d.stroke, d.pad, d.bbox, d.clip);
                        entry.id = u32::MAX - *serial;
                        let d = Drawn { p: &entry, xf, fill, stroke, pad, bbox, clip };
                        paint_drawn(out, &d, doc, decimals)?;
                    }
                    out.push_str("</g>\n");
                }
            }
            NodeKind::Image(id) => {
                let blobs = blobs.ok_or_else(|| ExportError::InvalidDocument("Missing SVG image resources".into()))?;
                crate::images::svg::appearance_element(out, doc, blobs, id, preview)
                    .map_err(ExportError::InvalidDocument)?;
            }
            _ => {
                out.push_str(&format!(
                    "<g id=\"appearance-node-{nid}\" opacity=\"{}\"{}>\n",
                    number(n.look.map_or(1., |l| l.opacity), decimals),
                    if n.look.is_some_and(|l| l.isolate) { " style=\"isolation:isolate\"" } else { "" }
                ));
                if let Some(mask) = n.mask_child {
                    if n.role == GroupRole::MaskAlpha {
                        let [x, y, w, h] = page.rect;
                        out.push_str(&format!("<defs><mask id=\"alpha-{nid}\" maskUnits=\"userSpaceOnUse\" maskContentUnits=\"userSpaceOnUse\" x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" style=\"mask-type:alpha\">\n"));
                        walk(out, doc, mask, page, cancel, decimals, serial, blobs, preview)?;
                        out.push_str(&format!("</mask></defs><g mask=\"url(#alpha-{nid})\">\n"));
                    } else {
                        let mut data = String::new();
                        for pid in doc.node_paths(mask) {
                            if let Some(pi) = doc.pidx(pid) {
                                data.push_str(&path_data(&doc.paths[pi], &doc.unit_xform(pid), decimals));
                            }
                        }
                        out.push_str(&format!("<defs><clipPath id=\"clip-{nid}\" clipPathUnits=\"userSpaceOnUse\"><path d=\"{data}\" clip-rule=\"evenodd\"/></clipPath></defs><g clip-path=\"url(#clip-{nid})\">\n"));
                    }
                }
                for child in n.children.iter().rev().filter(|id| Some(**id) != n.mask_child) {
                    walk(out, doc, *child, page, cancel, decimals, serial, blobs, preview)?;
                }
                if n.mask_child.is_some() {
                    out.push_str("</g>\n");
                }
                out.push_str("</g>\n");
            }
        }
        Ok(())
    }
    let mut serial = 0;
    for nid in doc.roots.iter().rev() {
        walk(out, doc, *nid, page, cancel, decimals, &mut serial, blobs, preview)?;
    }
    Ok(())
}
