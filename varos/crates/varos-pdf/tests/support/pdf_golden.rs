//! Compare every parsed object byte payload; only writer version and its stream length may differ.
pub fn assert_same(current: &[u8], frozen: &[u8], version: u32) {
    let mut current = lopdf::Document::load_mem(current).unwrap();
    let frozen = lopdf::Document::load_mem(frozen).unwrap();
    for object in current.objects.values_mut() {
        match object {
            lopdf::Object::Dictionary(d) if d.has(b"VAROS_SchemaVersion") => {
                d.set("VAROS_SchemaVersion", version as i64);
            }
            lopdf::Object::Stream(s) if s.content.starts_with(b"{\"varos\":") => {
                let text = String::from_utf8(s.content.clone()).unwrap();
                s.set_content(
                    text.replacen(
                        &format!("\"varos\":{}", varos_core::format::FORMAT_VERSION),
                        &format!("\"varos\":{version}"),
                        1,
                    )
                    .into_bytes(),
                );
            }
            _ => {}
        }
    }
    assert_eq!(current.objects, frozen.objects);
    assert_eq!(current.trailer, frozen.trailer);
}
