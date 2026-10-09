//! Bounded transparent 2× clipboard page; no operating-system clipboard access.
use std::sync::Arc;
use varos_core::model::{Artboard, Document};
pub fn clipboard_png(mut doc: Document, rect: [f32; 4]) -> Result<Vec<u8>, String> {
    let [x, y, w, h] = rect;
    if !rect.iter().all(|v| v.is_finite()) || w <= 0.0 || h <= 0.0 {
        return Err("Invalid clipboard bounds.".into());
    }
    let size = [(w * 2.0).ceil() as u32, (h * 2.0).ceil() as u32];
    if size.contains(&0) || size.iter().any(|v| *v > 16384) || u64::from(size[0]) * u64::from(size[1]) > 64_000_000 {
        return Err("Selection is too large for a 2× clipboard PNG.".into());
    }
    // Freeze source-board visibility before replacing page furniture with a transparent page.
    let hidden: std::collections::HashSet<_> =
        doc.paths.iter().filter(|p| doc.eff_hidden(p.id)).map(|p| p.id).collect();
    for path in &mut doc.paths {
        if hidden.contains(&path.id) {
            path.hidden = true;
        }
    }
    doc.artboards = vec![Artboard {
        x,
        y,
        w: size[0] as f32 / 2.0,
        h: size[1] as f32 / 2.0,
        page_color: None,
        ..Artboard::default()
    }];
    doc.active = 0;
    crate::rasterize_artboard(Arc::new(doc), 0, size).ok_or("Invalid clipboard bounds")?.encode_png()
}
