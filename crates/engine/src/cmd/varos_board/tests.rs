use super::*;
use vectorcraft_geom::{Affine, Point};

fn board() -> Session {
    let mut s = Session::new();
    s.execute("file.newBoard", &json!({})).unwrap();
    s
}
fn art(s: &mut Session) {
    s.execute("shape.rectangle", &json!({"x":-240,"y":-130,"width":80,"height":50})).unwrap();
}
fn bytes(value: &Value) -> Vec<u8> {
    vectorcraft_format::base64_decode(value["dataBase64"].as_str().unwrap()).unwrap()
}

#[test]
fn boardless_renders_hit_tests_and_roundtrips_without_a_page() {
    let mut s = board();
    assert!(s.active().unwrap().doc.artboards.is_empty());
    art(&mut s);
    let doc = &s.active().unwrap().doc;
    let hit = vectorcraft_doc::hit::hit_test(doc, Point::new(-200.0, -100.0), Default::default());
    assert!(hit.is_some());
    let opts = vectorcraft_render::RenderOptions { artboards: false, trim: true, ..Default::default() };
    let image = vectorcraft_render::Renderer::new().render(doc, 160, 100, Affine::translate((260.0, 150.0)), &opts);
    assert!(image.pixels.chunks_exact(4).any(|p| p[3] != 0));
    let data = vectorcraft_format::save(doc, false);
    let restored = vectorcraft_format::load(&data).unwrap();
    assert_eq!(restored.artboards.len(), 0);
    assert_eq!(restored.art_bounds(), doc.art_bounds());
    assert!(vectorcraft_doc::hit::hit_test(&restored, Point::new(-200.0, -100.0), Default::default()).is_some());
    // Existing Artboard tool commits this same engine command.
    s.execute("artboard.new", &json!({"x":-250,"y":-150,"width":200,"height":150})).unwrap();
    assert_eq!(s.active().unwrap().doc.artboards.len(), 1);
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(s.active().unwrap().doc.artboards.is_empty());
}

#[test]
fn boardless_default_png_pdf_and_print_use_art_bounds() {
    let mut s = board();
    art(&mut s);
    let bounds = vectorcraft_render::encode::art_bounds(&s.active().unwrap().doc).unwrap();
    let png = s.execute("document.export", &json!({"format":"png"})).unwrap();
    let image = image::load_from_memory(&bytes(&png)).unwrap();
    assert_eq!(image.width(), bounds.width().ceil() as u32);
    assert_eq!(image.height(), bounds.height().ceil() as u32);
    for (command, params) in [
        ("document.exportPdf", json!({})),
        ("document.export", json!({"format":"pdf"})),
        ("document.export", json!({"format":"pdf","useArtboards":false})),
        ("document.exportPdf", json!({"thumbnails":true,"compatibility":"1.3"})),
    ] {
        let value = s.execute(command, &params).unwrap();
        let pdf = bytes(&value);
        let native = vectorcraft_pdf::editing(&pdf).unwrap();
        assert!(vectorcraft_format::load(&native.data).unwrap().artboards.is_empty(), "{command} {params}");
        let pages = vectorcraft_pdf::info(&pdf, None).unwrap();
        assert_eq!(pages.pages.len(), 1);
        assert!((pages.pages[0].width - bounds.width()).abs() < 0.02);
        assert!((pages.pages[0].height - bounds.height()).abs() < 0.02);
    }
    let pdf = bytes(&s.execute("document.exportPdf", &json!({})).unwrap());
    let native = vectorcraft_pdf::editing(&pdf).unwrap();
    assert!(vectorcraft_format::load(&native.data).unwrap().artboards.is_empty());
    let printed = s.execute("file.print", &json!({})).unwrap();
    assert!(bytes(&printed).starts_with(b"%PDF-"));
    let preview = s.execute("print.preview", &json!({})).unwrap();
    assert!(!preview["sheets"].as_array().unwrap().is_empty(), "{preview}");
    assert!(s.active().unwrap().doc.artboards.is_empty(), "outputs never manufacture a saved board");
    assert!(s.execute("document.export", &json!({"format":"png","artboard":0})).is_err());
    assert!(s.execute("document.exportPdf", &json!({"artboard":0})).is_err());
}

#[test]
fn empty_board_export_reports_an_error_and_old_documents_keep_their_pages() {
    let mut s = board();
    for (command, p) in [("document.export", json!({"format":"png"})), ("document.exportPdf", json!({})), ("print.preview", json!({}))] {
        assert!(s.execute(command, &p).is_err());
    }
    assert!(s.execute("file.newBoard", &json!({"artboards":1})).is_err());
    s.execute("file.new", &json!({"width":123,"height":456,"artboards":2})).unwrap();
    let old = &s.active().unwrap().doc;
    let restored = vectorcraft_format::load(&vectorcraft_format::save(old, false)).unwrap();
    assert_eq!(restored.artboards.iter().map(|a| a.rect).collect::<Vec<_>>(), old.artboards.iter().map(|a| a.rect).collect::<Vec<_>>());
    assert_eq!(restored.artboards.len(), 2);
}

#[test]
fn boardless_pdf_bounds_skip_hidden_art_and_keep_shadow_reach() {
    let mut s = board();
    let visible = s.execute("shape.rectangle", &json!({"x":-200,"y":-100,"width":80,"height":50})).unwrap()["id"].as_u64().unwrap();
    let hidden = s.execute("shape.rectangle", &json!({"x":10000,"y":10000,"width":500,"height":500})).unwrap()["id"].as_u64().unwrap();
    let doc = std::sync::Arc::make_mut(&mut s.active_mut().unwrap().doc);
    doc.node_mut(vectorcraft_doc::NodeId(hidden)).unwrap().visible = false;
    doc.node_mut(vectorcraft_doc::NodeId(visible)).unwrap().appearance.effects.push(vectorcraft_doc::Effect {
        id: "stylize.dropShadow".into(),
        params: json!({"x":20,"y":15,"blur":2,"opacity":100}),
        visible: true,
    });
    let bounds = vectorcraft_render::encode::art_bounds(doc).unwrap();
    assert!(bounds.width() > 80.0 && bounds.width() < 200.0);
    let value = s.execute("document.exportPdf", &json!({})).unwrap();
    let pages = vectorcraft_pdf::info(&bytes(&value), None).unwrap();
    assert!((pages.pages[0].width - bounds.width()).abs() < 0.02);
    assert!((pages.pages[0].height - bounds.height()).abs() < 0.02);
}
