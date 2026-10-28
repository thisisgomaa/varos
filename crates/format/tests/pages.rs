#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use vectorcraft_doc::Document;
use vectorcraft_format::*;
#[test]
fn pages_migration_fixtures_refusals_and_byte_stable_round_trip() {
    let old = include_bytes!("../../../fixtures/pages_v4.varos");
    let file = load_file(old).unwrap();
    assert_eq!(file.project.pages.len(), 1);
    assert_eq!(file.project.pages[0].name, "Page 1");
    let current = load_file(include_bytes!("../../../fixtures/pages_v5.varos")).unwrap();
    assert_eq!(current.project.pages.len(), 2);
    let opts = SaveOptions { pretty: false, ..Default::default() };
    let once = save_project(&current.project, &opts).unwrap();
    // The generator includes the current build SHA; stability is checked after its first rewrite.
    let twice = save_project(&load_file(&once).unwrap().project, &opts).unwrap();
    assert_eq!(once, twice);
    let migrated = save_project(&file.project, &opts).unwrap();
    let value: serde_json::Value = serde_json::from_slice(json_part(&migrated)).unwrap();
    assert_eq!(value["version"], 5);
    assert!(value.get("document").is_none());
    assert_eq!(value["pages"][0]["name"], "Page 1");
    assert!(matches!(load_file(include_bytes!("../../../fixtures/pages_future.varos")), Err(FormatError::TooNew(6))));
    for input in
        [r#"{"format":"vectorcraft","version":5,"pages":[],"active_page":0}"#, r#"{"format":"vectorcraft","version":5,"pages":[],"active_page":999}"#]
    {
        assert!(load_file(input.as_bytes()).is_err());
    }
    assert!(save_project(&current.project, &SaveOptions { version: 4, ..opts }).is_err());
}
#[test]
fn pages_compression_and_legacy_extensions_keep_all_documents() {
    let mut project = migrate_v4_to_v5_pages(Document::new(200.0, 100.0));
    project.pages.push(Page { id: "second".into(), name: "Second".into(), doc: Document::new(99.0, 88.0) });
    project.active = 1;
    for compress in [false, true] {
        let bytes = save_project(&project, &SaveOptions { compress, ..Default::default() }).unwrap();
        let back = load_file(&bytes).unwrap().project;
        assert_eq!(back.active, 1);
        assert_eq!(back.pages.len(), 2);
        assert_eq!(back.pages[1].doc.artboards[0].rect.width(), 99.0);
    }
    for ext in ["varos", "vectorcraft", "drawcraft"] {
        assert!(is_native_ext(ext));
    }
}

#[test]
fn pages_keep_identical_resource_keys_independent_and_refuse_duplicate_ids() {
    use vectorcraft_doc::{ImageBlob, ImageObject, Node, NodeKind};
    use vectorcraft_geom::Affine;
    let mut project = migrate_v4_to_v5_pages(Document::new(20.0, 10.0));
    project.pages.push(Page { id: "other".into(), name: "Other".into(), doc: Document::new(20.0, 10.0) });
    for (i, page) in project.pages.iter_mut().enumerate() {
        let doc = &mut page.doc;
        doc.images.insert("shared-key".into(), ImageBlob::new("image/png", vec![i as u8; 4]));
        let node = Node::new(
            doc.alloc_id(),
            NodeKind::Image(Box::new(ImageObject {
                key: "shared-key".into(),
                width: 1,
                height: 1,
                xf: Affine::IDENTITY,
                link: None,
                placement: Default::default(),
                pixels: None,
            })),
        );
        doc.insert(Some(doc.layers[0].id), 0, node).unwrap();
    }
    let bytes = save_project(&project, &SaveOptions::default()).unwrap();
    let back = load_file(&bytes).unwrap().project;
    assert_eq!(&*back.pages[0].doc.images["shared-key"].bytes, &[0; 4]);
    assert_eq!(&*back.pages[1].doc.images["shared-key"].bytes, &[1; 4]);
    project.pages[1].id = project.pages[0].id.clone();
    assert!(save_project(&project, &SaveOptions::default()).is_err());
}
