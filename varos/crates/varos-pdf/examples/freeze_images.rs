//! Explicit maintainer tool for the next image format corpus; never called by a test.
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};
use varos_core::{
    format::Limits,
    images::{self, ImageEdit, Pixels},
    EditCommand, Editor,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures/v6-images");
    std::fs::create_dir_all(&root)?;
    let mut ed = Editor::new();
    let p = Pixels { budget: None, width: 4, height: 3, rgba: Arc::from([255, 0, 0, 128].repeat(12)) };
    let source = images::codec::encode_png(&p)?;
    let (id, _) =
        images::links::place_bytes(&mut ed, &source, [10., 20.], Some([10., 20., 72., 54.]), Default::default(), None)?;
    ed.try_execute(EditCommand::Image(ImageEdit::Crop { id, bounds: [20., 25., 40., 30.] }))?;
    let json = varos_core::format::encode_model(&ed.doc, &Limits::DEFAULT)?;
    std::fs::write(root.join("embedded-crop.json"), &json)?;
    std::fs::write(
        root.join("embedded-crop.vrs"),
        varos_pdf::images::write_vrs(&ed.doc, &ed.blobs, &Limits::DEFAULT)?,
    )?;
    let plan = varos_core::svg::plan_svg_export(&ed.doc, varos_core::svg::ExportScope::WholeBoard)?;
    std::fs::write(
        root.join("embedded-crop.svg"),
        images::svg::export(&ed.doc, &ed.blobs, &plan, false, &AtomicBool::new(false))?.0[0].bytes.clone(),
    )?;
    let value: serde_json::Value = serde_json::from_str(&json)?;
    for (name, v) in [
        ("missing-asset", {
            let mut v = value.clone();
            v["doc"]["assets"] = serde_json::json!([]);
            v
        }),
        ("singular-affine", {
            let mut v = value.clone();
            v["doc"]["images"][0]["xform"]["a"] = 0.into();
            v
        }),
        ("unknown-pixels", {
            let mut v = value.clone();
            v["doc"]["images"][0]["pixels"] = serde_json::json!([1, 2, 3]);
            v
        }),
        ("future", serde_json::json!({"varos":7,"doc":42})),
    ] {
        std::fs::write(root.join(format!("refused-{name}.json")), serde_json::to_vec(&v)?)?;
    }
    Ok(())
}
