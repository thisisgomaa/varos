use varos_core::{
    appearance::{BaseSlot, EntryOpts, Look, StackItem},
    format::{decode_model, encode_model, Limits, FORMAT_VERSION},
    model::Path,
};
#[test]
fn appearance_has_one_authoritative_base_and_preserves_legacy_bytes() {
    let mut path = Path::new(7, vec![], true, Some([0.2, 0.4, 0.6, 1.0]), None, 2.0);
    path.opacity = 0.3;
    let before = serde_json::to_vec(&path).unwrap();
    let view = path.appearance();
    assert!(std::ptr::eq(view.fill(), &path.fill));
    assert!(std::ptr::eq(view.stroke(), &path.stroke));
    assert_eq!(view.paint(BaseSlot::Fill), &path.fill);
    assert_eq!(
        view.stack(),
        [
            StackItem::Base(BaseSlot::Fill, EntryOpts::default()),
            StackItem::Base(BaseSlot::Stroke, EntryOpts::default())
        ]
    );
    assert_eq!(view.look(), Look { opacity: 0.3, isolate: false });
    assert_eq!(serde_json::to_vec(&path).unwrap(), before);
}
#[test]
fn frozen_v4_v5_document_subtrees_are_byte_identical_after_read_view() {
    let mut checked = 0;
    for dir in ["v4", "v5"] {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(dir);
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if !matches!(path.extension().and_then(|s| s.to_str()), Some("json" | "vrs")) {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            if bytes.starts_with(b"%PDF") {
                continue;
            }
            let input: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            if input.get("varos").is_none() {
                continue;
            } // stroke library, not a document
            checked += 1;
            let loaded = decode_model(&bytes, None, &Limits::default()).unwrap();
            let encoded = encode_model(&loaded.doc, &Limits::default()).unwrap();
            // Existing v4→v5 is an identity migration; only its wrapper stamp changes.
            let old_stamp =
                encoded.replacen(&format!("\"varos\":{FORMAT_VERSION}"), &format!("\"varos\":{}", input["varos"]), 1);
            assert_eq!(old_stamp, std::str::from_utf8(&bytes).unwrap().trim(), "{}", path.display());
            let before = serde_json::to_vec(&loaded.doc).unwrap();
            for p in &loaded.doc.paths {
                let _ = (p.appearance().stack(), p.appearance().look());
            }
            assert_eq!(before, serde_json::to_vec(&loaded.doc).unwrap(), "{}", path.display());
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&serde_json::to_vec(&loaded.doc).unwrap()).unwrap(),
                input["doc"],
                "{}",
                path.display()
            );
        }
    }
    assert!(checked >= 40, "checked {checked} frozen documents");
}
