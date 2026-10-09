use std::sync::{atomic::AtomicBool, Arc};
use varos_core::{EditCommand, Editor};
#[test]
fn cpu_and_svg_share_outlines_and_report() {
    let mut ed = Editor::new();
    ed.try_execute_created(EditCommand::AddText {
        text: varos_text_layout::default_text("العربية O", [20., 80.]).unwrap(),
        parent: None,
    })
    .unwrap();
    let out = varos_text_layout::outline_document(&ed.doc).unwrap();
    let a = varos_raster::rasterize(Arc::new(ed.doc.clone()), [320, 160]);
    let b = varos_raster::rasterize(Arc::new(out), [320, 160]);
    assert!(a.errors.is_empty());
    assert_eq!(a.pixels, b.pixels);
    let assets = varos_raster::export::plan(&ed.doc, &varos_raster::export::Scope::WholeBoard).unwrap();
    let options = varos_raster::export::Options { format: varos_raster::export::Format::Svg, ..Default::default() };
    let result = varos_raster::export::encode(&assets[0], &options, &AtomicBool::new(false)).unwrap();
    let svg = String::from_utf8(result.bytes).unwrap();
    assert!(svg.contains("<path"));
    assert!(!svg.contains("<text"));
    assert!(result.report.notes.iter().any(|n| n.message == "text exported as outlines"));
}
