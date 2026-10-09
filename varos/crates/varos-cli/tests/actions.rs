//! Lane F: real offline CLI Actions adapter, file preservation and target rebinding.
use std::{path::PathBuf, process::Command};
use varos_core::{
    actions::{Actions, Step},
    Editor,
};
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "varos-cli-actions-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn apply_actions_rebinds_selection_and_refuses_without_writing() {
    let dir = Scratch::new();
    let input = dir.path("input.vrs");
    let batch = dir.path("recorded.vrs-actions");
    let output = dir.path("output.vrs");
    let mut ed = Editor::new();
    Actions {
        version: 1,
        name: "Seed".into(),
        steps: vec![Step::Rectangle { local: "r".into(), bounds_pt: [0., 0., 10., 10.], fill: None }],
    }
    .replay(&mut ed)
    .unwrap();
    let id = ed.doc.paths[0].id;
    let before = ed.doc.paths[0].anchors[0].p;
    let original = varos_pdf::write_pdf(&ed.doc).unwrap();
    std::fs::write(&input, &original).unwrap();
    let action = Actions { version: 1, name: "Move".into(), steps: vec![Step::Nudge { delta_pt: [7., 9.] }] };
    std::fs::write(&batch, serde_json::to_vec(&action).unwrap()).unwrap();
    let args = [
        "apply".as_ref(),
        input.as_os_str(),
        "--batch".as_ref(),
        batch.as_os_str(),
        "--out".as_ref(),
        output.as_os_str(),
    ];
    let failed = Command::new(env!("CARGO_BIN_EXE_varos-cli")).args(args).output().unwrap();
    assert!(!failed.status.success());
    assert!(!output.exists());
    assert_eq!(std::fs::read(&input).unwrap(), original);
    let passed =
        Command::new(env!("CARGO_BIN_EXE_varos-cli")).args(args).args(["--ids", &id.to_string()]).output().unwrap();
    assert!(passed.status.success(), "{}", String::from_utf8_lossy(&passed.stdout));
    let after = varos_pdf::load_vrs(&output).unwrap();
    assert_eq!(after.paths[0].anchors[0].p, [before[0] + 7., before[1] + 9.]);
    assert_eq!(std::fs::read(&input).unwrap(), original);
    std::fs::write(&batch, br#"{"version":99,"name":"bad","steps":[]}"#).unwrap();
    std::fs::remove_file(&output).unwrap();
    let failed = Command::new(env!("CARGO_BIN_EXE_varos-cli")).args(args).output().unwrap();
    assert!(!failed.status.success());
    assert!(!output.exists());
}
