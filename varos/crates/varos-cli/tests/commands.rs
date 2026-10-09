use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
use varos_core::{
    bridge::{json, Value},
    EditCommand, Editor,
};
const FIXTURES: &[&str] = &["v3_board_meta.vrs", "v3_board_meta_pdf.vrs", "v3_boardless.vrs", "v3_boardless_pdf.vrs"];
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures/v3").join(name)
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static SERIAL: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "varos-cli-tests-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn path(&self, s: &str) -> PathBuf {
        self.0.join(s)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn cli(args: &[&std::ffi::OsStr], success: bool) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_varos-cli")).args(args).output().unwrap();
    assert_eq!(output.status.success(), success, "{}", String::from_utf8_lossy(&output.stdout));
    assert!(output.stderr.is_empty(), "unexpected stderr: {}", String::from_utf8_lossy(&output.stderr));
    let value: Value = String::from_utf8(output.stdout).unwrap().parse().unwrap();
    assert_eq!(value["ok"], success);
    if success {
        value["result"].clone()
    } else {
        assert!(value["error"]["reason"].as_str().is_some_and(|s| !s.is_empty()));
        value["error"].clone()
    }
}
#[test]
fn describe_and_full_geometry_for_every_v3_fixture() {
    for f in FIXTURES {
        let input = fixture(f);
        let doc = varos_pdf::load_vrs(&input).unwrap();
        let v = cli(&["describe".as_ref(), input.as_os_str()], true);
        assert_eq!(v["name"], doc.name);
        assert_eq!(v["tags"], json!(doc.tags));
        let count = doc.paths.len()
            + doc.nodes.iter().filter(|n| !matches!(n.kind, varos_core::model::NodeKind::Path(_))).count();
        assert_eq!(v["element_count"], count);
        assert!(!v.to_string().contains("anchors"));
        let id = format!("path:{}", doc.paths[0].id);
        let detail = cli(&["describe".as_ref(), input.as_os_str(), "--detail".as_ref(), id.as_ref()], true);
        assert_eq!(detail["geometry"]["anchors"], json!(doc.paths[0].anchors));
        assert!(detail["world_transform"].is_object());
        cli(&["describe".as_ref(), input.as_os_str(), "--detail".as_ref(), "path:999999".as_ref()], false);
    }
}
#[test]
fn snapshot_is_a_deterministic_cpu_png_for_every_v3_fixture() {
    let dir = Scratch::new();
    for f in FIXTURES {
        let input = fixture(f);
        let out = dir.path("x.png");
        let v = cli(
            &[
                "snapshot".as_ref(),
                input.as_os_str(),
                "--out".as_ref(),
                out.as_os_str(),
                "--size".as_ref(),
                "400".as_ref(),
            ],
            true,
        );
        assert_eq!(v["width"], 400);
        assert_eq!(v["height"], 400);
        let bytes = std::fs::read(&out).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(u32::from_be_bytes(bytes[16..20].try_into().unwrap()), 400);
        let expected = varos_raster::rasterize(std::sync::Arc::new(varos_pdf::load_vrs(&input).unwrap()), [400, 400])
            .encode_png()
            .unwrap();
        assert_eq!(bytes, expected);
    }
}
#[test]
fn pdf_export_is_model_free_and_save_as_reopens_for_every_v3_fixture() {
    let dir = Scratch::new();
    for f in FIXTURES {
        let input = fixture(f);
        let out = dir.path("x.pdf");
        cli(&["export-pdf".as_ref(), input.as_os_str(), "--out".as_ref(), out.as_os_str()], true);
        let bytes = std::fs::read(&out).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        // Reuse the desktop export test's raw-byte privacy assertions.
        for needle in [
            "EmbeddedFile",
            "Filespec",
            "/AF",
            "/Names",
            "VAROS",
            "model.varos.json",
            "\"varos\"",
            "\"paths\"",
            "/Metadata",
            "/Info",
            "/ID",
        ] {
            assert!(!bytes.windows(needle.len()).any(|w| w == needle.as_bytes()), "export contains {needle}");
        }
        assert!(!varos_pdf::has_embedded_model(&bytes));
        let saved = dir.path("saved.vrs");
        cli(&["save-as".as_ref(), input.as_os_str(), "--out".as_ref(), saved.as_os_str()], true);
        assert!(varos_pdf::has_embedded_model(&std::fs::read(&saved).unwrap()));
        assert_eq!(varos_pdf::load_vrs(&saved).unwrap(), varos_pdf::load_vrs(&input).unwrap());
    }
}
#[test]
fn apply_and_diff_for_every_v3_fixture_preserve_the_original() {
    let dir = Scratch::new();
    let batch = dir.path("commands.json");
    for f in FIXTURES {
        let input = fixture(f);
        let original = std::fs::read(&input).unwrap();
        let out = dir.path("new.vrs");
        let doc = varos_pdf::load_vrs(&input).unwrap();
        let id = doc.paths[0].id;
        std::fs::write(&batch,json!({"api":"0.1","commands":[{"SetBoardName":"CLI board"},{"RenamePath":{"path":id,"name":"Renamed"}},{"SelectPaths":[id]},{"SetOpacity":0.4}]}).to_string()).unwrap();
        let v = cli(
            &[
                "apply".as_ref(),
                input.as_os_str(),
                "--batch".as_ref(),
                batch.as_os_str(),
                "--out".as_ref(),
                out.as_os_str(),
            ],
            true,
        );
        assert_eq!(v["commands"], 4);
        assert_eq!(v["changed"], true);
        let result = varos_pdf::load_vrs(&out).unwrap();
        assert_eq!(result.name, "CLI board");
        assert_eq!(result.paths[result.pidx(id).unwrap()].opacity, 0.4);
        let d = cli(&["diff".as_ref(), input.as_os_str(), out.as_os_str()], true);
        assert_eq!(d["added"], json!([]));
        assert_eq!(d["removed"], json!([]));
        assert!(d["changed"].as_array().unwrap().iter().any(|e| e["id"] == format!("path:{id}")));
        assert!(d["document_changes"].as_array().unwrap().contains(&json!("name")));
        let identical = cli(&["diff".as_ref(), input.as_os_str(), input.as_os_str()], true);
        assert_eq!(identical["changed"], json!([]));
        assert_eq!(std::fs::read(&input).unwrap(), original);
    }
}
#[test]
fn new_every_preset_and_compare_to_the_frozen_v3_documents() {
    let dir = Scratch::new();
    for preset in ["free", "square", "portrait", "story", "a4"] {
        let out = dir.path("preset.vrs");
        let v = cli(&["new".as_ref(), "--preset".as_ref(), preset.as_ref(), "--out".as_ref(), out.as_os_str()], true);
        let doc = varos_pdf::load_vrs(&out).unwrap();
        assert_eq!(v["preset"], preset);
        assert!(doc.paths.is_empty());
        assert_eq!(doc.artboards.len(), usize::from(preset != "free"));
        for f in FIXTURES {
            let d = cli(&["diff".as_ref(), fixture(f).as_os_str(), out.as_os_str()], true);
            assert!(!d["removed"].as_array().unwrap().is_empty());
        }
    }
}
#[test]
fn batch_refusal_is_indexed_and_never_writes_or_truncates() {
    let dir = Scratch::new();
    let out = dir.path("sentinel.vrs");
    let batch = dir.path("bad.json");
    let input = fixture(FIXTURES[0]);
    for bad in [
        json!({"RenamePath":{"path":999999,"name":"no"}}),
        json!({"SetBoardName":"x".repeat(121)}),
        json!({"SetOpacity":2}),
        json!({"Bogus":1}),
        json!({"RenamePath":{"path":10,"name":"hi","extra":true}}),
        json!("Undo"),
    ] {
        std::fs::write(&out, b"sentinel").unwrap();
        std::fs::write(&batch, json!({"api":"0.1","commands":[{"SetBoardName":"must roll back"},bad]}).to_string())
            .unwrap();
        let error = cli(
            &[
                "apply".as_ref(),
                input.as_os_str(),
                "--batch".as_ref(),
                batch.as_os_str(),
                "--out".as_ref(),
                out.as_os_str(),
            ],
            false,
        );
        assert_eq!(error["index"], 1);
        assert_eq!(std::fs::read(&out).unwrap(), b"sentinel");
    }
    std::fs::remove_file(&out).unwrap();
    cli(
        &[
            "apply".as_ref(),
            input.as_os_str(),
            "--batch".as_ref(),
            batch.as_os_str(),
            "--out".as_ref(),
            out.as_os_str(),
        ],
        false,
    );
    assert!(!out.exists());
}
#[test]
fn invalid_arguments_and_file_errors_are_json_with_nonzero_exit() {
    let dir = Scratch::new();
    let input = fixture(FIXTURES[0]);
    let out = dir.path("x");
    for args in [
        vec![],
        vec!["bogus".as_ref()],
        vec!["new".as_ref()],
        vec!["new".as_ref(), "--preset".as_ref(), "bogus".as_ref(), "--out".as_ref(), out.as_os_str()],
        vec![
            "snapshot".as_ref(),
            input.as_os_str(),
            "--out".as_ref(),
            out.as_os_str(),
            "--size".as_ref(),
            "0".as_ref(),
        ],
        vec![
            "snapshot".as_ref(),
            input.as_os_str(),
            "--out".as_ref(),
            out.as_os_str(),
            "--size".as_ref(),
            "2049".as_ref(),
        ],
        vec!["describe".as_ref(), out.as_os_str()],
        vec!["describe".as_ref(), input.as_os_str(), "--wat".as_ref()],
        vec!["describe".as_ref(), input.as_os_str(), "--detail".as_ref()],
        vec![
            "describe".as_ref(),
            input.as_os_str(),
            "--detail".as_ref(),
            "path:10".as_ref(),
            "--detail".as_ref(),
            "path:11".as_ref(),
        ],
    ] {
        cli(&args, false);
    }
    let refused = Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures/refused/v4_future.vrs");
    cli(&["describe".as_ref(), refused.as_os_str()], false);
    assert!(!out.exists());
}
#[test]
fn atomic_batch_is_one_undo_step_with_redo_and_preserved_prior_history() {
    for f in FIXTURES {
        let mut ed = Editor::new();
        ed.replace_doc(varos_pdf::load_vrs(&fixture(f)).unwrap());
        let initial = ed.doc.clone();
        ed.execute(EditCommand::SetBoardName("prior history".into()));
        let before = ed.doc.clone();
        let rev = ed.rev;
        ed.execute_batch(vec![
            EditCommand::SetBoardName("batch".into()),
            EditCommand::SetBoardDescription("description".into()),
        ])
        .unwrap();
        let after = ed.doc.clone();
        assert_eq!(ed.rev, rev + 1);
        ed.undo();
        assert_eq!(ed.doc, before);
        ed.redo();
        assert_eq!(ed.doc, after);
        ed.undo();
        ed.undo();
        assert_eq!(ed.doc, initial);
    }
}
#[test]
fn failed_batch_preserves_editor_selection_revision_and_redo() {
    let mut ed = Editor::new();
    ed.replace_doc(varos_pdf::load_vrs(&fixture(FIXTURES[0])).unwrap());
    ed.execute(EditCommand::SetBoardName("redo survives".into()));
    let future = ed.doc.clone();
    ed.undo();
    let original = ed.doc.clone();
    let rev = ed.rev;
    ed.objsel.insert(10);
    let err = ed
        .execute_batch(vec![
            EditCommand::SelectPaths(vec![11]),
            EditCommand::SetBoardName("staged".into()),
            EditCommand::RenamePath { path: 999999, name: "bad".into() },
        ])
        .unwrap_err();
    assert_eq!(err.index, 2);
    assert_eq!(ed.doc, original);
    assert_eq!(ed.rev, rev);
    assert_eq!(ed.objsel, [10].into_iter().collect());
    ed.redo();
    assert_eq!(ed.doc, future);
}
#[test]
fn empty_and_noop_batches_do_not_create_history() {
    let mut ed = Editor::new();
    ed.replace_doc(varos_pdf::load_vrs(&fixture(FIXTURES[0])).unwrap());
    let original = ed.doc.clone();
    let rev = ed.rev;
    ed.execute_batch(vec![]).unwrap();
    ed.execute_batch(vec![EditCommand::SetBoardName(ed.doc.name.clone())]).unwrap();
    assert_eq!(ed.rev, rev);
    ed.undo();
    assert_eq!(ed.doc, original);
}

#[test]
fn diff_reports_added_removed_and_geometry_even_with_unchanged_bounds() {
    let dir = Scratch::new();
    for f in FIXTURES {
        let input = fixture(f);
        let mut doc = varos_pdf::load_vrs(&input).unwrap();
        let id = doc.paths[0].id;
        // A handle-only edit changes full geometry even when its outline bounds stay the same.
        doc.paths[0].anchors[0].smooth = !doc.paths[0].anchors[0].smooth;
        let changed = dir.path("geometry.vrs");
        varos_pdf::save_vrs(&doc, &changed).unwrap();
        let d = cli(&["diff".as_ref(), input.as_os_str(), changed.as_os_str()], true);
        assert!(d["changed"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["id"] == format!("path:{id}") && v["summary"].as_str().unwrap().contains("geometry")));
        let batch = dir.path("replace.json");
        std::fs::write(
            &batch,
            json!({"api":"0.1","commands":[{"SelectPaths":[id]},"Copy",{"Paste":{"offset":[200,0]}},{"SelectPaths":[id]},"DeleteSelected"]})
                .to_string(),
        )
        .unwrap();
        let out = dir.path("replaced.vrs");
        cli(
            &[
                "apply".as_ref(),
                input.as_os_str(),
                "--batch".as_ref(),
                batch.as_os_str(),
                "--out".as_ref(),
                out.as_os_str(),
            ],
            true,
        );
        let d = cli(&["diff".as_ref(), input.as_os_str(), out.as_os_str()], true);
        assert!(d["removed"].as_array().unwrap().contains(&json!(format!("path:{id}"))));
        assert!(d["added"].as_array().unwrap().iter().any(|v| v.as_str().unwrap().starts_with("path:")));
    }
}

#[test]
fn checked_command_ranges_and_structure_fail_without_partial_edits() {
    let mut ed = Editor::new();
    ed.replace_doc(varos_pdf::load_vrs(&fixture(FIXTURES[0])).unwrap());
    let original = ed.doc.clone();
    let bad = [
        EditCommand::SetArtboardCount(usize::MAX),
        EditCommand::SetArtboardCount(0),
        EditCommand::SetActiveArtboard(999),
        EditCommand::SetObjectRotation(f32::INFINITY),
        EditCommand::SetStrokeWidth(-1.0),
        EditCommand::ApplyPaint { target: varos_core::editor::PaintTarget::Fill, color: Some([2.0, 0.0, 0.0, 1.0]) },
        EditCommand::SetObjectBounds {
            x: None,
            y: None,
            width: Some(-1.0),
            height: None,
            anchor_x: 0.0,
            anchor_y: 0.0,
        },
        EditCommand::MoveLayer { sources: vec![15], target: 13, position: varos_core::model::DropPos::Into },
        EditCommand::SelectAnchors(vec![999999]),
    ];
    for command in bad {
        let error = ed
            .execute_batch(vec![
                EditCommand::SelectPaths(vec![10]),
                EditCommand::SetBoardName("staged".into()),
                command,
            ])
            .unwrap_err();
        assert_eq!(error.index, 2);
        assert_eq!(ed.doc, original);
    }
    ed.execute_batch(vec![EditCommand::SelectAnchors(vec![1]), EditCommand::Nudge { x: 3.0, y: 5.0 }]).unwrap();
    assert_ne!(ed.doc, original);
    ed.undo();
    assert_eq!(ed.doc, original);
}

#[test]
fn save_validation_failure_is_attributed_to_the_command_that_created_it() {
    let mut ed = Editor::new();
    let mut doc = varos_pdf::load_vrs(&fixture(FIXTURES[0])).unwrap();
    doc.artboards.resize(1000, doc.artboards[0].clone());
    ed.replace_doc(doc);
    let original = ed.doc.clone();
    let error = ed
        .execute_batch(vec![EditCommand::SetBoardName("staged".into()), EditCommand::DuplicateArtboard(0)])
        .unwrap_err();
    assert_eq!(error.index, 1);
    assert_eq!(ed.doc, original);
}

#[test]
fn output_io_failure_does_not_destroy_the_source() {
    let dir = Scratch::new();
    let input = fixture(FIXTURES[0]);
    let before = std::fs::read(&input).unwrap();
    let out = dir.path("missing/x.pdf");
    cli(&["export-pdf".as_ref(), input.as_os_str(), "--out".as_ref(), out.as_os_str()], false);
    assert_eq!(std::fs::read(&input).unwrap(), before);
    assert!(!out.exists());
}

#[cfg(unix)]
#[test]
fn non_utf8_output_path_returns_json_even_if_the_filesystem_refuses_it() {
    use std::os::unix::ffi::OsStringExt;
    let dir = Scratch::new();
    let out = dir.0.join(std::ffi::OsString::from_vec(b"board-\xff.vrs".to_vec()));
    // Some sandboxed filesystems refuse non-UTF8 paths. Both outcomes must remain JSON responses.
    let output = Command::new(env!("CARGO_BIN_EXE_varos-cli"))
        .args(["new".as_ref(), "--preset".as_ref(), "square".as_ref(), "--out".as_ref(), out.as_os_str()])
        .output()
        .unwrap();
    let value: Value = String::from_utf8(output.stdout).unwrap().parse().unwrap();
    assert_eq!(value["ok"], output.status.success());
    assert!(output.stderr.is_empty());
    if output.status.success() {
        assert!(out.is_file());
        cli(&["describe".as_ref(), out.as_os_str()], true);
    } else {
        assert!(value["error"]["reason"].as_str().is_some_and(|s| !s.is_empty()));
    }
}

#[test]
fn apply_requires_explicit_in_place_for_canonical_input_aliases() {
    let dir = Scratch::new();
    let input = dir.path("input.vrs");
    std::fs::copy(fixture(FIXTURES[0]), &input).unwrap();
    let original = std::fs::read(&input).unwrap();
    let batch = dir.path("batch.json");
    std::fs::write(&batch, r#"{"api":"0.1","commands":[{"SetBoardName":"in place"}]}"#).unwrap();
    let alias = dir.path("./input.vrs");
    let args = [
        "apply".as_ref(),
        input.as_os_str(),
        "--batch".as_ref(),
        batch.as_os_str(),
        "--out".as_ref(),
        alias.as_os_str(),
    ];
    let error = cli(&args, false);
    assert!(error["reason"].as_str().unwrap().contains("--in-place"));
    assert_eq!(std::fs::read(&input).unwrap(), original);
    let mut authorized = args.to_vec();
    authorized.push("--in-place".as_ref());
    cli(&authorized, true);
    assert_eq!(varos_pdf::load_vrs(&input).unwrap().name, "in place");
    cli(&["apply".as_ref(), input.as_os_str(), "--batch".as_ref(), batch.as_os_str(), "--in-place".as_ref()], true);
}

#[cfg(unix)]
#[test]
fn apply_refuses_a_symlink_to_the_input() {
    let dir = Scratch::new();
    let input = dir.path("input.vrs");
    std::fs::copy(fixture(FIXTURES[0]), &input).unwrap();
    let alias = dir.path("alias.vrs");
    std::os::unix::fs::symlink(&input, &alias).unwrap();
    let batch = dir.path("batch.json");
    std::fs::write(&batch, r#"{"api":"0.1","commands":[]}"#).unwrap();
    let error = cli(
        &[
            "apply".as_ref(),
            input.as_os_str(),
            "--batch".as_ref(),
            batch.as_os_str(),
            "--out".as_ref(),
            alias.as_os_str(),
        ],
        false,
    );
    assert!(error["reason"].as_str().unwrap().contains("--in-place"));
    assert!(alias.is_symlink());
    std::fs::write(&batch, r#"{"api":"0.1","commands":[{"SetBoardName":"through alias"}]}"#).unwrap();
    cli(
        &[
            "apply".as_ref(),
            input.as_os_str(),
            "--batch".as_ref(),
            batch.as_os_str(),
            "--out".as_ref(),
            alias.as_os_str(),
            "--in-place".as_ref(),
        ],
        true,
    );
    assert!(alias.is_symlink());
    assert_eq!(varos_pdf::load_vrs(&input).unwrap().name, "through alias");
}

#[test]
fn bridge_envelope_versions_and_stable_wire_names() {
    for api in ["0.1", "0.2"] {
        let bytes = json!({"api":api,"commands":[{"SelectPaths":[10]},{"ApplyPaint":{"target":"Fill","color":[0.1,0.4,0.9,1]}}]}).to_string();
        let parsed = varos_core::bridge::parse_batch(bytes.as_bytes()).unwrap();
        assert_eq!(
            json!(parsed),
            json!([{"SelectPaths":[10]},{"ApplyPaint":{"target":"Fill","color":[0.1_f32,0.4_f32,0.9_f32,1.0_f32]}}])
        );
    }
    for bytes in [r#"{"api":"1.0","commands":[]}"#, r#"{"api":"invalid","commands":[]}"#, "[]", r#"{"commands":[]}"#] {
        let error = varos_core::bridge::parse_batch(bytes.as_bytes()).err().unwrap();
        assert_eq!(error.index, 0);
    }
}

/// Slice 3: `export-pdf --artboard` exports exactly that page by its stable id (format 4 fixture: ids 16, 17).
#[test]
fn export_pdf_artboard_exports_one_page_by_stable_id() {
    let dir = Scratch::new();
    let input = Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures/v4/v4_board_meta.vrs");
    let count = |out: &Path| {
        let text = String::from_utf8_lossy(&std::fs::read(out).unwrap()).into_owned();
        let at = text.find("/Count ").expect("a page tree");
        text[at + 7..].split(|c: char| !c.is_ascii_digit()).next().unwrap().parse::<u32>().unwrap()
    };
    let all = dir.path("all.pdf");
    cli(&["export-pdf".as_ref(), input.as_os_str(), "--out".as_ref(), all.as_os_str()], true);
    assert_eq!(count(&all), 2);
    let mut bytes = vec![];
    for id in ["artboard:16", "artboard:17", "17"] {
        let one = dir.path(&format!("one-{}.pdf", id.replace(':', "_")));
        let args = ["export-pdf".as_ref(), input.as_os_str(), "--out".as_ref(), one.as_os_str()];
        cli(&[&args[..], &["--artboard".as_ref(), id.as_ref()]].concat(), true);
        assert_eq!(count(&one), 1, "{id}");
        bytes.push(std::fs::read(&one).unwrap());
    }
    assert_ne!(bytes[0], bytes[1], "page A (with art) and page B are different pages");
    assert_eq!(bytes[1], bytes[2], "artboard:17 and 17 name the same page");
    for bad in ["artboard:99", "artboard:0", "a1@1"] {
        let out = dir.path("bad.pdf");
        let error = cli(
            &[
                "export-pdf".as_ref(),
                input.as_os_str(),
                "--out".as_ref(),
                out.as_os_str(),
                "--artboard".as_ref(),
                bad.as_ref(),
            ],
            false,
        );
        assert!(error["reason"].as_str().unwrap().contains("artboard"), "{bad}: {error}");
    }
    let out = dir.path("save.vrs");
    cli(
        &[
            "save-as".as_ref(),
            input.as_os_str(),
            "--out".as_ref(),
            out.as_os_str(),
            "--artboard".as_ref(),
            "17".as_ref(),
        ],
        false,
    );
}

/// Review P1 (slice 3): an export never overwrites its editable input — not by the same path, a
/// `..` spelling, a symlink or (Unix) a hard link — and `--in-place` is not an export option.
#[test]
fn export_pdf_refuses_to_overwrite_its_input() {
    let dir = Scratch::new();
    let input = dir.path("board.vrs");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures/v4/v4_board_meta.vrs"),
        &input,
    )
    .unwrap();
    let original = std::fs::read(&input).unwrap();
    let dotted = dir.path("sub");
    std::fs::create_dir(&dotted).unwrap();
    // symlink and hard-link aliases are detected on Unix (canonical path, then device/inode)
    let targets: Vec<PathBuf> = [input.clone(), dotted.join("../board.vrs")]
        .into_iter()
        .chain(unix_aliases(&input, &dir.path("link.vrs"), &dir.path("hard.vrs")))
        .collect();
    for out in &targets {
        for extra in [&[][..], &["--artboard".as_ref(), "17".as_ref()][..]] {
            let args =
                [&["export-pdf".as_ref(), input.as_os_str(), "--out".as_ref(), out.as_os_str()][..], extra].concat();
            let error = cli(&args, false);
            assert!(error["reason"].as_str().unwrap().contains("resolves to the input"), "{out:?}: {error}");
            assert_eq!(std::fs::read(&input).unwrap(), original, "{out:?}: the editable input is untouched");
        }
    }
    let error = cli(&["export-pdf".as_ref(), input.as_os_str(), "--in-place".as_ref()], false);
    assert!(error["reason"].as_str().unwrap().contains("unknown option --in-place"), "{error}");
    assert_eq!(std::fs::read(&input).unwrap(), original);
    // a different, existing destination is still fine (replaced atomically)
    let other = dir.path("other.pdf");
    std::fs::write(&other, b"old").unwrap();
    cli(&["export-pdf".as_ref(), input.as_os_str(), "--out".as_ref(), other.as_os_str()], true);
    assert!(std::fs::read(&other).unwrap().starts_with(b"%PDF-"));
}

#[cfg(unix)]
fn unix_aliases(target: &Path, link: &Path, hard: &Path) -> Vec<PathBuf> {
    std::os::unix::fs::symlink(target, link).unwrap();
    std::fs::hard_link(target, hard).unwrap();
    vec![link.to_path_buf(), hard.to_path_buf()]
}
#[cfg(not(unix))]
fn unix_aliases(_: &Path, _: &Path, _: &Path) -> Vec<PathBuf> {
    vec![]
}

fn nested_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v3_nested_group.vrs")
}

#[test]
fn describe_nested_group_matches_golden_json_and_paint_order() {
    let value = cli(&["describe".as_ref(), nested_fixture().as_os_str()], true);
    let expected: Value = include_str!("fixtures/describe_nested_group.json").parse().unwrap();
    assert_eq!(value, expected);
    let ids: Vec<_> = value["elements"].as_array().unwrap().iter().map(|v| v["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["node:1", "node:15", "node:16", "path:10", "path:11"]);
    assert!(!value.to_string().contains("0.10000000149011612"));
    assert_eq!(value["elements"][3]["fill"], json!([0.1, 0.4, 0.9, 1.0]));
}

#[test]
fn describe_nested_group_detail_has_parent_children_and_world_bounds() {
    let value =
        cli(&["describe".as_ref(), nested_fixture().as_os_str(), "--detail".as_ref(), "node:16".as_ref()], true);
    assert_eq!(value["kind"], "group");
    assert_eq!(value["parent"], "node:15");
    assert_eq!(value["geometry"]["children"], json!([13]));
    assert_eq!(value["bounds"], json!([-10.71, -10.71, 70.71, 70.71]));
}

#[test]
fn diff_arrays_follow_paint_order() {
    let doc = varos_pdf::load_vrs(&nested_fixture()).unwrap();
    let empty = varos_core::board::new_board();
    let added = varos_core::bridge::diff(&empty, &doc);
    assert_eq!(added["added"], json!(["node:15", "node:16", "path:10", "path:11"]));
    let removed = varos_core::bridge::diff(&doc, &empty);
    assert_eq!(removed["removed"], added["added"]);
    let mut changed = doc.clone();
    for path in &mut changed.paths {
        path.opacity = 0.5;
    }
    let diff = varos_core::bridge::diff(&doc, &changed);
    let ids: Vec<_> = diff["changed"].as_array().unwrap().iter().map(|v| v["id"].clone()).collect();
    assert_eq!(ids, [json!("path:10"), json!("path:11")]);
}

#[test]
fn rejected_bridge_major_and_empty_export_preserve_existing_output() {
    let dir = Scratch::new();
    let batch = dir.path("future.json");
    let out = dir.path("existing.vrs");
    std::fs::write(&out, b"sentinel").unwrap();
    std::fs::write(&batch, r#"{"api":"1.0","commands":[{"SetBoardName":"future"}]}"#).unwrap();
    let error = cli(
        &[
            "apply".as_ref(),
            fixture(FIXTURES[0]).as_os_str(),
            "--batch".as_ref(),
            batch.as_os_str(),
            "--out".as_ref(),
            out.as_os_str(),
        ],
        false,
    );
    assert_eq!(error["index"], 0);
    assert!(error["reason"].as_str().unwrap().contains("unsupported Bridge API"));
    assert_eq!(std::fs::read(&out).unwrap(), b"sentinel");
    let empty = dir.path("empty.vrs");
    varos_pdf::save_vrs(&varos_core::board::new_board(), &empty).unwrap();
    cli(&["export-pdf".as_ref(), empty.as_os_str(), "--out".as_ref(), out.as_os_str()], false);
    assert_eq!(std::fs::read(&out).unwrap(), b"sentinel");
}

#[test]
fn diff_keeps_sub_decimal_geometry_changes_despite_describe_rounding() {
    let doc = varos_pdf::load_vrs(&nested_fixture()).unwrap();
    let mut changed = doc.clone();
    changed.paths[0].opacity -= 0.00001;
    assert_eq!(
        varos_core::bridge::describe(&doc, Some("path:10")).unwrap()["opacity"],
        varos_core::bridge::describe(&changed, Some("path:10")).unwrap()["opacity"]
    );
    assert_eq!(varos_core::bridge::diff(&doc, &changed)["changed"][0]["id"], "path:10");
}

#[test]
fn pdf_options_and_headless_print_match_the_writer() {
    let scratch = Scratch::new();
    let input = fixture("v3_board_meta.vrs");
    let doc = varos_pdf::load_vrs(&input).unwrap();
    let plan = varos_pdf::plan_pdf_export(&doc, varos_pdf::default_scope(&doc)).unwrap();
    let mut options = varos_pdf::PdfOptions::preset(varos_pdf::PdfPreset::Press);
    options.image_ppi = 150;
    options.marks.crop = true;
    options.boxes.bleed_override = Some(3.);
    let expected =
        varos_pdf::export_pdf_with_options(&doc, &plan, &options, &std::sync::atomic::AtomicBool::new(false)).unwrap();
    for verb in ["export-pdf", "print"] {
        let out = scratch.path(&format!("{verb}.pdf"));
        let result = cli(
            &[
                verb.as_ref(),
                input.as_os_str(),
                "--out".as_ref(),
                out.as_os_str(),
                "--preset".as_ref(),
                "press".as_ref(),
                "--ppi".as_ref(),
                "150".as_ref(),
                "--marks".as_ref(),
                "crop".as_ref(),
                "--bleed".as_ref(),
                "3".as_ref(),
            ],
            true,
        );
        assert_eq!(std::fs::read(out).unwrap(), expected.0);
        assert_eq!(result["report"], json!(expected.1));
    }
    let out = scratch.path("bad.pdf");
    cli(
        &["export-pdf".as_ref(), input.as_os_str(), "--out".as_ref(), out.as_os_str(), "--ppi".as_ref(), "0".as_ref()],
        false,
    );
    assert!(!out.exists());
}
#[test]
fn headless_clipboard_bundle_matches_the_shared_flavour_writers() {
    let scratch = Scratch::new();
    let input = fixture("v3_board_meta.vrs");
    let doc = varos_pdf::load_vrs(&input).unwrap();
    let id = doc.paths[0].id;
    let out = scratch.path("clipboard");
    cli(
        &[
            "clipboard-out".as_ref(),
            input.as_os_str(),
            "--ids".as_ref(),
            format!("path:{id}").as_ref(),
            "--out".as_ref(),
            out.as_os_str(),
        ],
        true,
    );
    let clip = varos_core::clipboard::Clipboard::capture(&doc, &[id]);
    let vectors = varos_pdf::clipboard_vectors(&doc, &clip).unwrap();
    for (name, bytes) in [
        ("selection.varos.json", vectors.internal),
        ("selection.pdf", vectors.pdf),
        ("selection.svg", vectors.svg),
        ("selection.png", varos_raster::clipboard_png(vectors.document, vectors.rect).unwrap()),
    ] {
        assert_eq!(std::fs::read(out.join(name)).unwrap(), bytes);
    }
}
