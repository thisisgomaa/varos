//! Lane A explicit fixture authoring: only the new v10 corpus is written.
use std::sync::atomic::AtomicBool;
use varos_core::{
    appearance::Look,
    appearance_edits::{AppearanceEdit as A, MaskEdit as M},
    format::{encode_model, Limits},
    model::{Anchor, Document, Paint, Path},
    EditCommand, Editor,
};
fn square(id: u32, x: f32, size: f32, opacity: f32) -> varos_core::model::Path {
    let mut p = Path::new(
        id,
        [[x, 0.], [x + size, 0.], [x + size, size], [x, size]]
            .into_iter()
            .enumerate()
            .map(|(i, p)| Anchor { id: id * 10 + i as u32, p, hin: None, hout: None, smooth: false })
            .collect(),
        true,
        Some([1., 0., 0., 1.]),
        None,
        2.,
    );
    p.opacity = opacity;
    p
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new("crates/varos-core/tests/fixtures/v10");
    std::fs::create_dir_all(root)?;
    let mut d =
        Document { paths: vec![square(10, 0., 40., 0.8), square(20, 10., 20., 0.5)], ids: 1000, ..Default::default() };
    d.sync_tree();
    let mut ed = Editor::new();
    ed.replace_doc(d);
    freeze(root, "plain", &ed)?;
    ed.try_execute(EditCommand::Appearance(A::AddFill { path: 10, paint: Paint::Solid([0., 0., 1., 1.]) }))?;
    ed.try_execute(EditCommand::Appearance(A::AddStroke {
        path: 10,
        paint: Paint::Solid([0., 1., 0., 1.]),
        width: 8.,
    }))?;
    freeze(root, "multiple", &ed)?;
    let node = ed.doc.node_of_path(10).ok_or("content missing")?;
    let mask = ed.doc.node_of_path(20).ok_or("mask missing")?;
    ed.try_execute(EditCommand::Mask(M::Add { node, mask, alpha: true }))?;
    freeze(root, "alpha", &ed)?;
    ed.try_execute(EditCommand::Appearance(A::SetLook { node: 1, look: Some(Look { opacity: 0.6, isolate: true }) }))?;
    freeze(root, "isolated_alpha", &ed)?;
    let valid = encode_model(&ed.doc, &Limits::DEFAULT)?;
    for (name, text) in [
        ("refused_v9_stack", valid.replacen("\"varos\":10", "\"varos\":9", 1)),
        ("refused_future", valid.replacen("\"varos\":10", "\"varos\":11", 1)),
    ] {
        std::fs::write(root.join(format!("{name}.json")), text)?;
    }
    Ok(())
}
fn freeze(root: &std::path::Path, name: &str, ed: &Editor) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::write(root.join(format!("{name}.json")), encode_model(&ed.doc, &Limits::DEFAULT)?)?;
    std::fs::write(root.join(format!("{name}.pdf")), varos_pdf::write_pdf(&ed.doc)?)?;
    let plan = varos_core::svg::plan_svg_export(&ed.doc, varos_core::svg::ExportScope::WholeBoard)?;
    let files = varos_core::svg::export_svg_files(&ed.doc, &plan, &AtomicBool::new(false))?;
    std::fs::write(root.join(format!("{name}.svg")), &files[0].bytes)?;
    Ok(())
}
