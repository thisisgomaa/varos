//! Lane F: optional preview introduced by the next native format.
//! Preview revision 1 uses an unfiltered PNG stream under /VAROS_Preview.
use lopdf::{dictionary, Document, Object, Stream};
const MAX_PREVIEW: usize = 2 * 1024 * 1024;
/// Pure next-container migration: preserves all existing page/model objects and adds a preview.
/// Upgrades the model envelope and catalog together; authored model content is unchanged.
pub fn embed_preview_next(pdf: &[u8], png: &[u8]) -> Result<Vec<u8>, String> {
    if png.len() > MAX_PREVIEW || !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("Preview must be a PNG up to 2 MiB".into());
    }
    let loaded = crate::load_vrs_bytes(pdf, &varos_core::format::Limits::DEFAULT).map_err(|e| e.to_string())?;
    let mut doc = Document::load_mem(pdf).map_err(|e| e.to_string())?;
    let root = doc.trailer.get(b"Root").and_then(Object::as_reference).map_err(|e| e.to_string())?;
    let model_id = doc
        .get_dictionary(root)
        .and_then(|d| d.get(b"VAROS_Model"))
        .and_then(Object::as_reference)
        .map_err(|e| e.to_string())?;
    let model = varos_core::format::encode_model(&loaded.doc, &varos_core::format::Limits::DEFAULT)
        .map_err(|e| e.to_string())?;
    doc.get_object_mut(model_id)
        .and_then(Object::as_stream_mut)
        .map_err(|e| e.to_string())?
        .set_content(model.into_bytes());
    doc.get_dictionary_mut(root)
        .map_err(|e| e.to_string())?
        .set("VAROS_SchemaVersion", varos_core::format::FORMAT_VERSION as i64);
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
    if pdf.len() as u64 > varos_core::format::Limits::DEFAULT.max_file_bytes {
        return Err("Container exceeds file limit".into());
    }
    let doc = Document::load_mem(pdf).map_err(|e| e.to_string())?;
    let root = doc.trailer.get(b"Root").and_then(Object::as_reference).map_err(|e| e.to_string())?;
    let catalog = doc.get_dictionary(root).map_err(|e| e.to_string())?;
    if !catalog.has(b"VAROS_Preview") && !catalog.has(b"VAROS_PreviewVersion") {
        return Ok(None);
    }
    let id = catalog.get(b"VAROS_Preview").and_then(Object::as_reference).map_err(|_| "Invalid preview reference")?;
    if catalog
        .get(b"VAROS_SchemaVersion")
        .and_then(Object::as_i64)
        .ok()
        .is_none_or(|v| v < varos_core::format::PREVIEW_FORMAT_VERSION as i64)
    {
        return Err("Preview keys require the next native format".into());
    }
    if catalog.get(b"VAROS_PreviewVersion").and_then(Object::as_i64).ok() != Some(1) {
        return Err("Unsupported container preview version".into());
    }
    let stream = doc.get_object(id).and_then(Object::as_stream).map_err(|e| e.to_string())?;
    if stream.content.len() > MAX_PREVIEW || stream.dict.has(b"Filter") {
        return Err("Unsupported or oversized preview stream".into());
    }
    if !stream.content.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("Invalid PNG preview".into());
    }
    Ok(Some(stream.content.clone()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_container_input_and_future_preview_refusal() {
        let original = include_bytes!("../../varos-core/tests/fixtures/v3/v3_boardless_pdf.vrs");
        let png = include_bytes!("../fixtures/quicklook/preview-v1.png");
        let upgraded = embed_preview_next(original, png).unwrap();
        assert_eq!(preview(&upgraded).unwrap().unwrap(), png);
        let old = crate::load_vrs_bytes(original, &varos_core::format::Limits::DEFAULT).unwrap().doc;
        let new = crate::load_vrs_bytes(&upgraded, &varos_core::format::Limits::DEFAULT).unwrap().doc;
        assert!(old.content_eq(&new));
        assert!(embed_preview_next(original, include_bytes!("../fixtures/quicklook/refuse-not-png.bin")).is_err());
        let refused: serde_json::Value =
            serde_json::from_slice(include_bytes!("../fixtures/quicklook/refuse-version.json")).unwrap();
        let mut doc = Document::load_mem(&upgraded).unwrap();
        let root = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        doc.get_dictionary_mut(root)
            .unwrap()
            .set("VAROS_PreviewVersion", refused["VAROS_PreviewVersion"].as_i64().unwrap());
        let mut bytes = vec![];
        doc.save_to(&mut bytes).unwrap();
        assert!(preview(&bytes).unwrap_err().contains("version"));
    }
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

#[cfg(test)]
mod fix_round_tests {
    use super::*;
    #[test]
    fn frozen_next_preview_and_old_stamp_refusal() {
        let next = include_bytes!("../fixtures/quicklook/next-preview.vrs");
        let old = include_bytes!("../../varos-core/tests/fixtures/v5/plain.pdf");
        let loaded = crate::load_vrs_bytes(next, &varos_core::format::Limits::DEFAULT).unwrap();
        assert_eq!(loaded.source_version, varos_core::format::PREVIEW_FORMAT_VERSION);
        assert_eq!(loaded.migrated, varos_core::format::FORMAT_VERSION > loaded.source_version);
        assert!(loaded.doc.content_eq(&crate::load_vrs_bytes(old, &varos_core::format::Limits::DEFAULT).unwrap().doc));
        assert_eq!(preview(next).unwrap().unwrap(), include_bytes!("../fixtures/quicklook/preview-v1.png"));
        // Preview keys under an older stamp are refused: format 5 and (integration w2) format 8, the
        // last format before the preview joined the v9 bump.
        for stamped in [
            include_bytes!("../fixtures/quicklook/refuse-old-stamp.vrs").as_slice(),
            include_bytes!("../fixtures/quicklook/refuse-v8-stamp.vrs").as_slice(),
        ] {
            let error = crate::load_vrs_bytes(stamped, &varos_core::format::Limits::DEFAULT).unwrap_err();
            assert!(error.to_string().contains("preview keys require native format 9"), "{error}");
        }
        // Freeze the previous v5 gate; the next writer is refused before any typed model decode.
        for pdf in [next.as_slice(), include_bytes!("../../varos-core/tests/fixtures/refused/future_v6.pdf").as_slice()]
        {
            let doc = Document::load_mem(pdf).unwrap();
            let v = doc.catalog().unwrap().get(b"VAROS_SchemaVersion").unwrap().as_i64().unwrap();
            assert!(v > 5, "the previous reader must refuse this container");
        }
        let old_future: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../varos-core/tests/fixtures/refused/future_v6.json")).unwrap();
        assert!(old_future["varos"].as_u64().unwrap() > 5);
    }
}
