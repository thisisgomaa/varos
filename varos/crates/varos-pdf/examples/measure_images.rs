//! Headless RSS probe; never initializes a GPU or app event loop.
use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};
use varos_core::{
    format::Limits,
    images::{self, Pixels},
    Editor,
};
fn main() -> Result<(), String> {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "save".into());
    let pixels =
        Pixels { budget: None, width: 2048, height: 2048, rgba: Arc::from([255, 0, 0, 128].repeat(2048 * 2048)) };
    let source = images::codec::encode_png(&pixels)?;
    drop(pixels);
    let now = Instant::now();
    let mut ed = Editor::new();
    images::links::place_bytes(&mut ed, &source, [0.; 2], None, Default::default(), None)?;
    eprintln!(
        "mode={mode} encoded={} resource={} decode_ms={}",
        source.len(),
        ed.blobs.retained_bytes(),
        now.elapsed().as_millis()
    );
    if mode == "save" {
        let bytes = varos_pdf::images::write_vrs(&ed.doc, &ed.blobs, &Limits::DEFAULT)?;
        let loaded = varos_pdf::load_vrs_bytes(&bytes, &Limits::DEFAULT).map_err(|e| e.to_string())?;
        eprintln!("native bytes={} reopened images={}", bytes.len(), loaded.doc.images.len());
    } else if mode == "export" {
        let pages = vec![varos_pdf::PageSpec {
            rect: [0., 0., 2048., 2048.],
            background: None,
            bleed: 0.,
            bleed_edges: [0.; 4],
        }];
        let (bytes, report) =
            varos_pdf::images::export_pdf(&ed.doc, &ed.blobs, &pages, 300., false, &AtomicBool::new(false))?;
        eprintln!("PDF bytes={} notes={}", bytes.len(), report.notes.len());
    }
    Ok(())
}
