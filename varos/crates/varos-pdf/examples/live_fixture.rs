//! Reproducible Lane E v13 fixture writer, never opens an app/window.
use varos_core::{
    format::Limits,
    live::{self, Action, Axis, Envelope, Kind, Orientation, Repeat, Warp},
    model::ShapeKind,
    EditCommand, Editor,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ed = Editor::new();
    for (i, kind) in [
        Kind::Blend { spine: None, steps: 2, orientation: Orientation::Page },
        Kind::Repeat { repeat: Repeat::Grid { rows: 2, cols: 2, gap: [4., 6.] } },
        Kind::Repeat { repeat: Repeat::Radial { count: 3, radius: 20. } },
        Kind::Repeat { repeat: Repeat::Mirror { axis: Axis::Vertical } },
        Kind::Envelope { envelope: Envelope::Warp { preset: Warp::Arc, bend: 0.25 } },
        Kind::Envelope { envelope: Envelope::Mesh { points: [[250., 0.], [270., 0.], [250., 10.], [270., 20.]] } },
    ]
    .into_iter()
    .enumerate()
    {
        let mut ids = Vec::new();
        for j in 0..if i == 0 { 2 } else { 1 } {
            let id = ed.try_execute_created(EditCommand::AddShape {
                kind: ShapeKind::Rect,
                bounds: [i as f32 * 50. + j as f32 * 20., 0., 10., 10.],
                parent: None,
                fill: Some([0.2, 0.6, 0.9, 1.]),
                stroke: None,
                stroke_width: 0.,
                opacity: 1.,
                name: None,
            })?;
            ids.push(id);
        }
        live::execute(&mut ed, Action::Make { paths: ids, kind })?;
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures/v13");
    let json = varos_core::format::encode_model(&ed.doc, &Limits::DEFAULT)?;
    std::fs::write(root.join("live.json"), &json)?;
    std::fs::write(root.join("live.vrs"), varos_pdf::write_pdf_checked(&ed.doc, &Limits::DEFAULT)?)?;
    std::fs::write(root.join("refused-v12-live.json"), json.replacen("\"varos\":13", "\"varos\":12", 1))?;
    std::fs::write(root.join("refused-newer.json"), "{\"varos\":14,\"doc\":42}")?;
    let mut value: serde_json::Value = serde_json::from_str(&json)?;
    let nodes = value["doc"]["nodes"].as_array_mut().ok_or("nodes")?;
    for node in nodes {
        if node["kind"]["Live"]["effect"] == "repeat" && node["kind"]["Live"]["repeat"]["mode"] == "grid" {
            node["kind"]["Live"]["repeat"]["rows"] = serde_json::json!(0);
            break;
        }
    }
    std::fs::write(root.join("refused-grid-zero.json"), serde_json::to_vec(&value)?)?;
    Ok(())
}
