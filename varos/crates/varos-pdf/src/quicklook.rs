//! Lane F: optional container preview, independent of the editable model version.
//! Preview revision 1 uses an unfiltered PNG stream under /VAROS_Preview.
use lopdf::{dictionary, Document, Object, Stream};
const MAX_PREVIEW: usize = 2 * 1024 * 1024;
/// Pure next-container migration: preserves all existing page/model objects and adds a preview.
/// Calling twice replaces the catalog pointer; no native model keys or FORMAT_VERSION change.
pub fn embed_preview_next(pdf: &[u8], png: &[u8]) -> Result<Vec<u8>, String> {
    if png.len() > MAX_PREVIEW || !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("Preview must be a PNG up to 2 MiB".into());
    }
    let mut doc = Document::load_mem(pdf).map_err(|e| e.to_string())?;
    let root = doc.trailer.get(b"Root").and_then(Object::as_reference).map_err(|e| e.to_string())?;
    if let Ok(old) = doc.get_dictionary(root).and_then(|d| d.get(b"VAROS_Preview")).and_then(Object::as_reference) {
        doc.objects.remove(&old);
    }
    let id = doc.add_object(Stream::new(
        dictionary! {"Type"=>"EmbeddedFile","Subtype"=>Object::Name(b"image/png".to_vec())},
        png.to_vec(),
    ));
    let catalog = doc.get_dictionary_mut(root).map_err(|e| e.to_string())?;
    catalog.set("VAROS_Preview", id);
    catalog.set("VAROS_PreviewVersion", 1);
    let mut bytes = vec![];
    doc.save_to(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}
pub fn preview(pdf: &[u8]) -> Result<Option<Vec<u8>>, String> {
    let doc = Document::load_mem(pdf).map_err(|e| e.to_string())?;
    let root = doc.trailer.get(b"Root").and_then(Object::as_reference).map_err(|e| e.to_string())?;
    let catalog = doc.get_dictionary(root).map_err(|e| e.to_string())?;
    let Ok(id) = catalog.get(b"VAROS_Preview").and_then(Object::as_reference) else { return Ok(None) };
    if catalog.get(b"VAROS_PreviewVersion").and_then(Object::as_i64).ok() != Some(1) {
        return Err("Unsupported container preview version".into());
    }
    let stream = doc.get_object(id).and_then(Object::as_stream).map_err(|e| e.to_string())?;
    if stream.content.len() > MAX_PREVIEW || stream.dict.has(b"Filter") {
        return Err("Unsupported or oversized preview stream".into());
    }
    Ok(Some(stream.content.clone()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_model_roundtrip_and_preview_refusals() {
        let doc = varos_core::board::new_board();
        let original = crate::write_pdf_checked(&doc, &varos_core::format::Limits::DEFAULT).unwrap();
        assert!(preview(&original).unwrap().is_none());
        let png = b"\x89PNG\r\n\x1a\nfixture";
        let next = embed_preview_next(&original, png).unwrap();
        assert_eq!(preview(&next).unwrap().unwrap(), png);
        assert_eq!(
            varos_core::format::encode_model(
                &crate::load_vrs_bytes(&next, &varos_core::format::Limits::DEFAULT).unwrap().doc,
                &varos_core::format::Limits::DEFAULT
            )
            .unwrap(),
            varos_core::format::encode_model(&doc, &varos_core::format::Limits::DEFAULT).unwrap()
        );
        let twice = embed_preview_next(&next, png).unwrap();
        assert_eq!(preview(&twice).unwrap().unwrap(), png);
        assert!(embed_preview_next(&original, b"not png").is_err());
        assert!(embed_preview_next(b"not pdf", png).is_err());
        assert!(embed_preview_next(&original, &vec![0; MAX_PREVIEW + 1]).is_err());
    }
}
