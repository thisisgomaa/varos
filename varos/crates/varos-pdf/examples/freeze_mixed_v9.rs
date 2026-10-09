//! One-shot generation of the frozen mixed format-9 fixture (integration w2): a placed image, a
//! gradient-filled path with Live Corners and an editable text box in ONE document. Tests compare
//! against these bytes and never regenerate them.
use std::sync::Arc;
use varos_core::{
    format::{encode_model, Limits},
    images::{codec, links, Pixels, PlacementMode},
    live_corners::{CornerParam, Kind},
    model::{Paint, ShapeKind},
    EditCommand, Editor,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures/v9");
    let mut ed = Editor::new();
    let png =
        codec::encode_png(&Pixels { budget: None, width: 2, height: 2, rgba: Arc::from([255, 0, 0, 255].repeat(4)) })?;
    links::place_bytes(&mut ed, &png, [10., 10.], None, PlacementMode::Embed, None)?;
    ed.try_execute(EditCommand::AddShape {
        kind: ShapeKind::Rect,
        bounds: [40., 40., 140., 100.],
        parent: None,
        fill: Some([0., 0., 1., 1.]),
        stroke: None,
        stroke_width: 0.,
        opacity: 1.,
        name: None,
    })?;
    let pid = ed.doc.paths.last().ok_or("no path")?.id;
    let path = ed.doc.paths.iter_mut().find(|p| p.id == pid).ok_or("no path")?;
    path.fill = Paint::Gradient(varos_core::gradient::Gradient::default());
    path.corners = vec![CornerParam { radius: 8., kind: Kind::Round }; path.anchors.len()];
    let swatch = ed.doc.nid();
    ed.doc.swatches.push(varos_core::swatches::Swatch {
        id: swatch,
        name: "Brand".into(),
        paint: Paint::Solid([0.9, 0.2, 0.1, 1.]),
        global: true,
        group: String::new(),
    });
    let text = varos_text_layout::default_text("Varos v9", [20., 160.])?;
    ed.try_execute(EditCommand::AddText { text, parent: None })?;
    let json = encode_model(&ed.doc, &Limits::DEFAULT)?;
    std::fs::write(root.join("mixed.json"), json.as_bytes())?;
    std::fs::write(root.join("mixed.vrs"), varos_pdf::images::write_vrs(&ed.doc, &ed.blobs, &Limits::DEFAULT)?)?;
    Ok(())
}
