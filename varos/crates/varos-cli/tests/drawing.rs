//! The offline CLI apply route replays the same deterministic commands as the tools.
use std::process::Command;
use varos_core::{
    drawing::{Action, Options, Shape, ShapeSpec},
    EditCommand, Editor,
};
#[test]
fn headless_cli_replays_shapes_and_all_freehand_behaviours() {
    let dir = std::env::temp_dir().join(format!("varos-lane-d-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("input.vrs");
    let output = dir.join("out.vrs");
    let batch = dir.join("batch.json");
    varos_pdf::save_vrs(&Editor::new().doc, &input).unwrap();
    let commands = vec![
        EditCommand::Drawing(Action::Shape { spec: ShapeSpec { kind: Shape::Star, ..Default::default() } }),
        EditCommand::Drawing(Action::Pencil {
            points: vec![[300., 0.], [350., 20.], [400., 0.]],
            options: Options::default(),
        }),
        EditCommand::Drawing(Action::Curvature { points: vec![[500., 0.], [550., 20.], [600., 0.]], closed: false }),
        EditCommand::Drawing(Action::Smooth { points: vec![[549., 20.], [551., 20.]], options: Options::default() }),
        EditCommand::Drawing(Action::PathErase {
            points: vec![[570., -10.], [570., 30.]],
            options: Options::default(),
        }),
        EditCommand::Drawing(Action::Join { points: vec![[560., -10.], [580., 30.]], options: Options::default() }),
        EditCommand::Drawing(Action::Options { options: Options::default() }),
    ];
    std::fs::write(&batch, serde_json::json!({"api":"1.2","commands":commands}).to_string()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_varos-cli"))
        .arg("apply")
        .arg(&input)
        .arg("--batch")
        .arg(&batch)
        .arg("--out")
        .arg(&output)
        .output()
        .unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stdout));
    assert!(!varos_pdf::load_vrs(&output).unwrap().paths.is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}
