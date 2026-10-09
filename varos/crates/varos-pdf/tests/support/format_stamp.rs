// Lane H: normalize only version-derived PDF fields; preserve all other object bytes.
pub fn objects(bytes: &[u8], from: u32, to: u32) -> std::collections::BTreeMap<lopdf::ObjectId, lopdf::Object> {
    let mut pdf = lopdf::Document::load_mem(bytes).unwrap();
    let catalog = pdf.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let model =
        pdf.get_object(catalog).unwrap().as_dict().unwrap().get(b"VAROS_Model").unwrap().as_reference().unwrap();
    let stream = pdf.get_object_mut(model).unwrap().as_stream_mut().unwrap();
    let source = String::from_utf8(stream.content.clone()).unwrap();
    let old = format!("\"varos\":{from},");
    assert_eq!(source.matches(&old).count(), 1);
    stream.set_content(source.replacen(&old, &format!("\"varos\":{to},"), 1).into_bytes());
    let len = stream.content.len() as i64;
    stream.dict.set("Length", len);
    if let Ok(params) = stream.dict.get_mut(b"Params") {
        let params = params.as_dict_mut().unwrap();
        if params.has(b"Size") {
            params.set("Size", len);
        }
    }
    pdf.get_object_mut(catalog).unwrap().as_dict_mut().unwrap().set("VAROS_SchemaVersion", to as i64);
    pdf.objects
}
