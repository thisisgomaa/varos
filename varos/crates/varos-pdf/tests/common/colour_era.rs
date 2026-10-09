pub fn stamp(bytes: &[u8], version: u32) -> Vec<u8> {
    if bytes.starts_with(b"%PDF") {
        let mut pdf = lopdf::Document::load_mem(bytes).unwrap();
        restamp(&mut pdf, version);
        let mut output = Vec::new();
        pdf.save_to(&mut output).unwrap();
        output
    } else {
        let mut v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        v["varos"] = serde_json::json!(version);
        serde_json::to_vec(&v).unwrap()
    }
}

#[allow(dead_code)] // images uses only stamp; refusal suites also use future
pub fn future(bytes: &[u8]) -> Vec<u8> {
    stamp(bytes, varos_core::format::FORMAT_VERSION + 1)
}

#[allow(dead_code)] // Only the frozen PDF-object corpus needs xref position normalization.
pub fn objects(mut pdf: lopdf::Document) -> std::collections::BTreeMap<lopdf::ObjectId, lopdf::Object> {
    for object in pdf.objects.values_mut() {
        if let lopdf::Object::Stream(stream) = object {
            stream.start_position = None;
        }
    }
    pdf.objects
}

#[allow(dead_code)]
pub fn restamp(pdf: &mut lopdf::Document, version: u32) {
    let root = pdf.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let model = pdf.catalog().unwrap().get(b"VAROS_Model").unwrap().as_reference().unwrap();
    let stream = pdf.get_object_mut(model).unwrap().as_stream_mut().unwrap();
    let text = std::str::from_utf8(&stream.content).unwrap();
    let end = text.find(",").unwrap();
    stream.set_content(format!("{{\"varos\":{version}{}", &text[end..]).into_bytes());
    pdf.get_object_mut(root).unwrap().as_dict_mut().unwrap().set("VAROS_SchemaVersion", version);
}
