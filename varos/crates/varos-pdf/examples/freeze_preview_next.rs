//! One-shot generation for Lane F's additional fixtures; historical fixtures are untouched.
use lopdf::{Document, Object};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/quicklook");
    let old = include_bytes!("../../varos-core/tests/fixtures/v5/plain.pdf");
    let png = include_bytes!("../fixtures/quicklook/preview-v1.png");
    let upgraded = varos_pdf::quicklook::embed_preview_next(old, png)?;
    std::fs::write(root.join("next-preview.vrs"), &upgraded)?;
    let mut pdf = Document::load_mem(&upgraded)?;
    let catalog_id = pdf.trailer.get(b"Root")?.as_reference()?;
    pdf.get_dictionary_mut(catalog_id)?.set("VAROS_SchemaVersion", 5);
    let model_id = pdf.get_dictionary(catalog_id)?.get(b"VAROS_Model")?.as_reference()?;
    let content = pdf.get_object(model_id)?.as_stream()?.content.clone();
    let current = format!("\"varos\":{}", varos_core::format::FORMAT_VERSION);
    let content = String::from_utf8(content)?.replacen(&current, "\"varos\":5", 1);
    pdf.get_object_mut(model_id)?.as_stream_mut()?.set_content(content.into_bytes());
    pdf.save(root.join("refuse-old-stamp.vrs"))?;
    let mut future = Document::load_mem(old)?;
    let root_id = future.trailer.get(b"Root")?.as_reference()?;
    let id = future.get_dictionary(root_id)?.get(b"VAROS_Model")?.as_reference()?;
    // integration w3: the refused future is format 15 (the wave-3 writer is 14)
    future.get_object_mut(id)?.as_stream_mut()?.set_content(br#"{"varos":15,"doc":42}"#.to_vec());
    future.get_dictionary_mut(root_id)?.set("VAROS_SchemaVersion", Object::Integer(15));
    future.save(root.join("future-v15.pdf"))?;
    std::fs::write(root.join("future-v15.json"), br#"{"varos":15,"doc":42}"#)?;
    Ok(())
}
