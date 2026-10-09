use std::sync::{atomic::AtomicBool, Arc};
use varos_core::{
    format::Limits,
    images::{self, Pixels, PlacementMode},
    Editor,
};
fn editor() -> Editor {
    let bytes = images::codec::encode_png(&Pixels {
        budget: None,
        width: 4,
        height: 4,
        rgba: Arc::from([255, 0, 0, 128].repeat(16)),
    })
    .unwrap();
    let mut ed = Editor::new();
    images::links::place_bytes(&mut ed, &bytes, [0.; 2], Some([0., 0., 72., 72.]), PlacementMode::Embed, None).unwrap();
    ed
}
#[test]
fn native_binary_streams_roundtrip_and_missing_original_refuses() {
    let ed = editor();
    let bytes = varos_pdf::images::write_vrs(&ed.doc, &ed.blobs, &Limits::DEFAULT).unwrap();
    let loaded = varos_pdf::load_vrs_bytes(&bytes, &Limits::DEFAULT).unwrap();
    assert_eq!(loaded.doc, ed.doc);
    assert_eq!(loaded.blobs, ed.blobs);
    let mut pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let root = pdf.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let assets = pdf
        .get_object_mut(root)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .get_mut(b"VAROS_Assets")
        .unwrap()
        .as_dict_mut()
        .unwrap();
    assets.get_mut(ed.doc.assets[0].key.0.as_bytes()).unwrap().as_dict_mut().unwrap().remove(b"Original");
    let mut corrupt = vec![];
    pdf.save_to(&mut corrupt).unwrap();
    assert_eq!(
        varos_pdf::load_vrs_bytes(&corrupt, &Limits::DEFAULT).unwrap_err(),
        varos_core::format::LoadError::MalformedPdf("Embedded image original stream missing".into())
    );
    assert!(varos_pdf::write_pdf_checked(&ed.doc, &Limits::DEFAULT).is_err());
}
#[test]
fn pdf_xobject_smask_downsampling_and_no_editable_export() {
    let ed = editor();
    let pages =
        vec![varos_pdf::PageSpec { rect: [0., 0., 72., 72.], background: None, bleed: 0., bleed_edges: [0.; 4] }];
    let (bytes, report) =
        varos_pdf::images::export_pdf(&ed.doc, &ed.blobs, &pages, 2., false, &AtomicBool::new(false)).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    assert!(pdf.catalog().unwrap().get(b"VAROS_Model").is_err());
    let images: Vec<_> = pdf
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .filter(|s| s.dict.get(b"Subtype").is_ok_and(|o| o.as_name().is_ok_and(|n| n == b"Image")))
        .collect();
    assert_eq!(images.len(), 2);
    assert!(images.iter().any(|s| s.dict.has(b"SMask")));
    assert!(images.iter().all(|s| s.dict.get(b"Width").unwrap().as_i64().unwrap() == 2));
    assert!(report.notes.iter().any(|n| n.kind == "image_downsample"));
}
#[test]
fn svg_portable_upright_image_and_clip_validate() {
    let mut ed = editor();
    let id = ed.doc.images[0].id;
    ed.try_execute(varos_core::EditCommand::Image(images::ImageEdit::Crop { id, bounds: [0., 0., 20., 20.] })).unwrap();
    let plan = varos_core::svg::ExportPlan {
        scope: varos_core::svg::ExportScope::WholeBoard,
        pages: vec![varos_core::svg::PageSpec {
            rect: [0., 0., 72., 72.],
            background: None,
            artboard: None,
            name: String::new(),
        }],
    };
    let (files, _) = images::svg::export(&ed.doc, &ed.blobs, &plan, false, &AtomicBool::new(false)).unwrap();
    let text = std::str::from_utf8(&files[0].bytes).unwrap();
    assert!(text.contains("<image "));
    assert!(text.contains("data:image/png;base64,"));
    assert!(text.contains("clipPath"));
    resvg::usvg::Tree::from_data(&files[0].bytes, &resvg::usvg::Options::default()).unwrap();
}

#[test]
fn frozen_image_container_and_svg_are_stable() {
    let data = include_bytes!("../../varos-core/tests/fixtures/v6-images/embedded-crop.vrs");
    let loaded = varos_pdf::load_vrs_bytes(data, &Limits::DEFAULT).unwrap();
    assert_eq!(
        varos_pdf::images::write_vrs(&loaded.doc, &loaded.blobs, &Limits::DEFAULT).unwrap(),
        include_bytes!("fixtures/image-fixed-writer.vrs")
    );
    let plan = varos_core::svg::plan_svg_export(&loaded.doc, varos_core::svg::ExportScope::WholeBoard).unwrap();
    assert_eq!(
        images::svg::export(&loaded.doc, &loaded.blobs, &plan, false, &AtomicBool::new(false)).unwrap().0[0].bytes,
        include_bytes!("../../varos-core/tests/fixtures/v6-images/embedded-crop.svg")
    );
}
#[test]
fn package_relocation_collision_and_modified_identity() {
    let root = std::env::temp_dir().join(format!(
        "w2-package-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let source = root.join("source.png");
    let base = editor();
    let blob = base.blobs.get(&base.doc.images[0].blob).unwrap();
    std::fs::write(&source, blob.original.as_ref().unwrap()).unwrap();
    let mut ed = Editor::new();
    images::links::place_file(&mut ed, &source, [0.; 2], None, PlacementMode::Link, None).unwrap();
    let before = ed.doc.clone();
    let rev = ed.rev;
    let destination = root.join("Package");
    varos_pdf::package::package(&ed.doc, &ed.blobs, &destination).unwrap();
    assert_eq!(ed.doc, before);
    assert_eq!(ed.rev, rev);
    assert!(varos_pdf::package::package(&ed.doc, &ed.blobs, &destination).is_err());
    let relocated = root.join("Moved");
    std::fs::rename(&destination, &relocated).unwrap();
    std::fs::remove_file(&source).unwrap();
    let loaded = varos_pdf::load_vrs_checked(&relocated.join("Document.vrs"), &Limits::DEFAULT).unwrap();
    assert!(loaded.blobs.get(&loaded.doc.images[0].blob).unwrap().original.is_some());
    let path = images::links::resolve(&loaded.doc.images[0], Some(&relocated), None).unwrap();
    let mut changed = std::fs::read(&path).unwrap();
    let end = changed.len() - 1;
    changed[end] ^= 1;
    std::fs::write(&path, &changed).unwrap();
    assert!(images::links::resolve(&loaded.doc.images[0], Some(&relocated), None).is_err());
    assert_eq!(images::links::status(&loaded.doc.images[0], &loaded.blobs), images::links::LinkStatus::Modified);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn image_selection_export_excludes_neighbor_resources() {
    let mut ed = editor();
    let id = ed.doc.images[0].id;
    let bytes = images::codec::encode_png(&Pixels {
        budget: None,
        width: 4,
        height: 4,
        rgba: Arc::from([0, 0, 255, 255].repeat(16)),
    })
    .unwrap();
    images::links::place_bytes(&mut ed, &bytes, [500., 500.], None, Default::default(), None).unwrap();
    let (narrowed, plan) = varos_pdf::plan_selection_export(&ed.doc, &[id].into_iter().collect()).unwrap();
    assert!(plan.pages[0].rect[2] < 100.);
    varos_pdf::images::export_pdf(&narrowed, &ed.blobs, &plan.pages, 300., false, &AtomicBool::new(false)).unwrap();
}
