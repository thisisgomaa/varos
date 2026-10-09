//! Compare frozen PDF objects after normalizing only the format stamps; reserialize lengths/xref.
pub fn normalized(bytes: &[u8]) -> Vec<u8> {
    let mut doc = lopdf::Document::load_mem(bytes).unwrap();
    for object in doc.objects.values_mut() {
        match object {
            lopdf::Object::Dictionary(d) if d.has(b"VAROS_SchemaVersion") => {
                d.set("VAROS_SchemaVersion", 1_i64);
            }
            lopdf::Object::Stream(s) if s.content.starts_with(b"{\"varos\":") => {
                let text = std::str::from_utf8(&s.content).unwrap();
                let comma = text.find(',').unwrap();
                s.set_content(format!("{{\"varos\":1{}", &text[comma..]).into_bytes());
            }
            _ => {}
        }
    }
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    bytes
}
