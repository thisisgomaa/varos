//! Selection clipboard vectors share the normal export writers, with no OS access.
use std::{collections::HashSet, sync::atomic::AtomicBool};
use varos_core::{clipboard::Clipboard, model::Document};
pub struct ClipboardVectors {
    pub internal: Vec<u8>,
    pub pdf: Vec<u8>,
    pub svg: Vec<u8>,
    pub document: Document,
    pub rect: [f32; 4],
}
pub fn clipboard_vectors(doc: &Document, clipboard: &Clipboard) -> Result<ClipboardVectors, String> {
    let mut selected: HashSet<u32> = clipboard.source_ids().collect();
    // ---- Lane G: public flavours use outlines, internal flavour keeps editable source. ----
    let outlined;
    let doc = if doc.text_boxes.is_empty() {
        doc
    } else {
        outlined = varos_text_layout::outline_document(doc)?;
        for text in &doc.text_boxes {
            if selected.remove(&text.id) {
                if let Some(node) = varos_core::text::node_id(doc, text.id) {
                    selected.extend(outlined.node_paths(node));
                }
            }
        }
        &outlined
    };
    let (narrowed, plan) = crate::plan_selection_export(doc, &selected).map_err(|e| e.to_string())?;
    let cancel = AtomicBool::new(false);
    let pdf = crate::export_pdf_bytes(&narrowed, &plan, &cancel).map_err(|e| e.to_string())?;
    let page = plan.pages[0];
    let svg_plan = varos_core::svg::ExportPlan {
        scope: varos_core::svg::ExportScope::WholeBoard,
        pages: vec![varos_core::svg::PageSpec {
            rect: page.rect,
            background: None,
            artboard: None,
            name: String::new(),
        }],
    };
    let svg = varos_core::svg::export_svg_files(&narrowed, &svg_plan, &cancel)
        .map_err(|e| e.to_string())?
        .into_iter()
        .next()
        .ok_or("No SVG page")?
        .bytes;
    Ok(ClipboardVectors {
        internal: serde_json::to_vec(clipboard).map_err(|e| e.to_string())?,
        pdf,
        svg,
        document: narrowed,
        rect: page.rect,
    })
}
