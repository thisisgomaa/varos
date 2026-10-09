use super::*;
use crate::shell::PanelId;
use crate::storage::durable::{Fault, FaultFs, Op, RealFs, Step};
use crate::storage::testdir::TestDir;

fn modified() -> Layout {
    let mut shell = ShellState::standard();
    shell.toggle_panel(PanelId::Layers); // active lower tab
    shell.toggle_panel(PanelId::Align); // close an open panel
    let mut tree = shell.layout_value();
    for tile in tree["tiles"]["tiles"].as_object_mut().unwrap().values_mut() {
        if let Some(shares) = tile.pointer_mut("/Container/Linear/shares/shares").and_then(|s| s.as_object_mut()) {
            for share in shares.values_mut() {
                *share = serde_json::json!(0.5);
            }
        }
    }
    Layout {
        tree,
        show_rail: false,
        show_control_bar: true,
        picker: PickerLayout {
            open: true,
            position: Some([35.0, 104.0]),
            drawer_open: true,
            drawer_tab: 2,
            mode: Default::default(),
            harmony: Default::default(),
        },
    }
}

#[test]
fn modified_tree_sizes_closed_panel_and_active_tab_roundtrip() {
    let d = TestDir::new("layout-roundtrip");
    let path = d.join("layout.json");
    let (mut store, _) = LayoutStore::load(&RealFs, Some(path.clone()), false);
    let layout = modified();
    assert!(layout.valid());
    store.observe(layout.clone(), Instant::now());
    assert!(store.flush(&RealFs, Instant::now()).unwrap());
    assert_eq!(store.next_wake(), None);
    let (mut back, loaded) = LayoutStore::load(&RealFs, Some(path), false);
    assert_eq!(loaded, layout);
    let shell = ShellState::from_layout_value(loaded.tree.clone()).unwrap();
    assert!(!shell.is_open(PanelId::Align));
    assert!(shell.is_open(PanelId::Layers));
    back.observe(loaded, Instant::now());
    assert!(!back.flush(&RealFs, Instant::now()).unwrap());
    assert_eq!(d.names(), ["layout.json"]);
}

#[test]
fn one_write_per_burst_and_no_unchanged_or_reverted_write() {
    let d = TestDir::new("layout-debounce");
    let fs = FaultFs::new(vec![]);
    let (mut store, original) = LayoutStore::load(&fs, Some(d.join("layout.json")), false);
    let now = Instant::now();
    store.observe(original.clone(), now);
    assert!(!store.tick(&fs, now + SETTLE).unwrap());
    for ms in [0, 100, 200] {
        let mut layout = modified();
        layout.show_control_bar = ms != 100;
        store.observe(layout, now + Duration::from_millis(ms));
        assert!(!store.tick(&fs, now + Duration::from_millis(ms)).unwrap());
    }
    assert!(!store.tick(&fs, now + Duration::from_millis(1199)).unwrap());
    assert!(store.tick(&fs, now + Duration::from_millis(1200)).unwrap());
    store.observe(modified(), now + SETTLE * 2);
    assert!(!store.tick(&fs, now + SETTLE * 3).unwrap());
    assert_eq!(fs.log().iter().filter(|(op, _)| *op == Op::Create).count(), 1);
    store.observe(original, now + SETTLE * 4);
    store.observe(modified(), now + SETTLE * 4);
    assert!(!store.flush(&fs, now + SETTLE * 4).unwrap());
}

#[test]
fn corrupt_unknown_panel_old_and_future_version_are_quarantined_silently() {
    let valid = serde_json::to_string(&Envelope { version: 1, app_build: "test".into(), layout: modified() }).unwrap();
    for bytes in [
        "{".to_string(),
        valid.replace("Layers", "FuturePanel"),
        valid.replace("\"version\":1", "\"version\":0"),
        valid.replace("\"version\":1", "\"version\":2"),
    ] {
        let d = TestDir::new("layout-bad");
        let path = d.join("layout.json");
        std::fs::write(&path, &bytes).unwrap();
        let (_, layout) = LayoutStore::load(&RealFs, Some(path.clone()), false);
        assert_eq!(layout, Layout::default());
        assert!(!path.exists());
        assert_eq!(std::fs::read_to_string(d.join("layout.json.bad")).unwrap(), bytes);
    }
}

#[test]
fn reset_restores_standard_removes_file_and_cancels_pending_even_on_quit() {
    let d = TestDir::new("layout-reset");
    let path = d.join("layout.json");
    let (mut store, _) = LayoutStore::load(&RealFs, Some(path.clone()), false);
    store.observe(modified(), Instant::now());
    store.flush(&RealFs, Instant::now()).unwrap();
    store.observe(Layout::default(), Instant::now());
    let standard = store.reset(&RealFs);
    assert_eq!(standard, Layout::default());
    assert!(!path.exists());
    store.observe(standard, Instant::now());
    assert!(!store.flush(&RealFs, Instant::now()).unwrap());
    std::fs::write(&path, "bad").unwrap();
    let (_, standard) = LayoutStore::load(&RealFs, Some(path.clone()), true);
    assert_eq!(standard, Layout::default());
    assert!(!path.exists());
    assert!(!d.join("layout.json.bad").exists());
}

#[test]
fn quit_flushes_before_debounce_expires() {
    let d = TestDir::new("layout-quit");
    let path = d.join("layout.json");
    let (mut store, _) = LayoutStore::load(&RealFs, Some(path.clone()), false);
    let now = Instant::now();
    store.observe(modified(), now);
    assert!(!store.tick(&RealFs, now).unwrap());
    assert!(store.flush(&RealFs, now).unwrap());
    assert_eq!(LayoutStore::load(&RealFs, Some(path), false).1, modified());
}

#[test]
fn failed_atomic_write_keeps_previous_file_and_retries_once_per_second() {
    let d = TestDir::new("layout-failure");
    let path = d.join("layout.json");
    let (mut store, _) = LayoutStore::load(&RealFs, Some(path.clone()), false);
    let now = Instant::now();
    store.observe(modified(), now);
    store.flush(&RealFs, now).unwrap();
    let original = std::fs::read(&path).unwrap();
    store.observe(Layout::default(), now + SETTLE);
    let fs = FaultFs::new(vec![Fault::at(Step::Rename)]);
    assert!(store.tick(&fs, now + SETTLE * 2).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!store.tick(&fs, now + SETTLE * 2).unwrap());
    assert!(store.tick(&fs, now + SETTLE * 3).unwrap());
    assert_eq!(d.names(), ["layout.json"]);
}

#[test]
fn missing_file_and_unavailable_storage_leave_standard_unwritten() {
    let d = TestDir::new("layout-missing");
    for path in [Some(d.join("layout.json")), None] {
        let (mut store, layout) = LayoutStore::load(&RealFs, path, false);
        assert_eq!(layout, Layout::default());
        store.observe(layout, Instant::now());
        assert!(!store.flush(&RealFs, Instant::now()).unwrap());
    }
    assert!(d.names().is_empty());
}

#[test]
fn tree_validation_rejects_empty_cycles_dangling_and_sizes() {
    let original = Layout::default().tree;
    let tiles = original["tiles"]["tiles"].as_object().unwrap();
    let root = original["root"].clone();
    let root_key = root.to_string();
    let tabs_key = tiles.iter().find(|(_, t)| t.pointer("/Container/Tabs").is_some()).unwrap().0.clone();
    let tabs = &tiles[&tabs_key]["Container"]["Tabs"];
    let mut cases = Vec::new();
    let mut value = original.clone();
    value["root"] = serde_json::Value::Null;
    cases.push(value);
    let mut value = original.clone();
    value["tiles"]["tiles"][&root_key]["Container"]["Linear"]["children"][0] = root;
    cases.push(value);
    let mut value = original.clone();
    value["tiles"]["tiles"][&root_key]["Container"]["Linear"]["children"][0] = serde_json::json!(999);
    cases.push(value);
    let mut value = original.clone();
    value["tiles"]["tiles"][&tabs_key]["Container"]["Tabs"]["children"] = serde_json::json!([]);
    cases.push(value);
    let mut value = original.clone();
    value["width"] = serde_json::json!(-1);
    cases.push(value);
    let mut value = original.clone();
    value["tiles"]["invisible"] = serde_json::json!([original["root"]]);
    cases.push(value);
    let mut value = original.clone();
    value["tiles"]["next_tile_id"] = tabs["children"][0].clone();
    cases.push(value);
    let mut value = original.clone();
    let shares = value["tiles"]["tiles"][&root_key]["Container"]["Linear"]["shares"]["shares"].as_object_mut().unwrap();
    *shares.values_mut().next().unwrap() = serde_json::json!(0);
    cases.push(value);
    for (i, tree) in cases.into_iter().enumerate() {
        let d = TestDir::new("layout-invalid-tree");
        let path = d.join("layout.json");
        let layout = Layout { tree, ..Layout::default() };
        let bytes = serde_json::to_vec(&Envelope { version: 1, app_build: "test".into(), layout }).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(LayoutStore::load(&RealFs, Some(path.clone()), false).1, Layout::default(), "case {i}");
        assert!(!path.exists());
        assert_eq!(std::fs::read(d.join("layout.json.bad")).unwrap(), bytes);
    }
}

#[test]
fn ordinary_frames_never_write_and_derived_column_survives_restore() {
    let d = TestDir::new("layout-frames");
    let path = d.join("layout.json");
    let (mut store, layout) = LayoutStore::load(&RealFs, Some(path), false);
    let mut shell = ShellState::from_layout_value(layout.tree).unwrap();
    let ctx = egui::Context::default();
    for _ in 0..3 {
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))),
                ..Default::default()
            },
            |ui| shell.ui(ui),
        );
        store.observe(Layout { tree: shell.layout_value(), ..Layout::default() }, Instant::now());
    }
    assert!(!store.flush(&RealFs, Instant::now()).unwrap());
    assert!(d.names().is_empty());
    let column = shell.side_column_span().unwrap();
    let mut restored = ShellState::from_layout_value(shell.layout_value()).unwrap();
    let _ = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))),
            ..Default::default()
        },
        |ui| restored.ui(ui),
    );
    assert_eq!(restored.side_column_span(), Some(column));
}

#[test]
fn second_bad_file_preserves_both_diagnostics() {
    let d = TestDir::new("layout-bad-twice");
    let path = d.join("layout.json");
    std::fs::write(d.join("layout.json.bad"), "earlier").unwrap();
    std::fs::write(&path, "latest").unwrap();
    LayoutStore::load(&RealFs, Some(path), false);
    assert_eq!(std::fs::read_to_string(d.join("layout.json.bad")).unwrap(), "latest");
    let archived = d.names().into_iter().find(|n| n.starts_with("layout.json.bad.")).unwrap();
    assert_eq!(std::fs::read_to_string(d.join(&archived)).unwrap(), "earlier");
}

#[test]
fn different_app_build_is_compatible() {
    let d = TestDir::new("layout-build");
    let path = d.join("layout.json");
    let layout = modified();
    std::fs::write(
        &path,
        serde_json::to_vec(&Envelope {
            version: 1,
            app_build: "different-future-build".into(),
            layout: layout.clone(),
        })
        .unwrap(),
    )
    .unwrap();
    assert_eq!(LayoutStore::load(&RealFs, Some(path), false).1, layout);
    assert_eq!(d.names(), ["layout.json"]);
}

#[test]
fn read_errors_preserve_layout_and_existing_diagnostic() {
    for kind in [std::io::ErrorKind::PermissionDenied, std::io::ErrorKind::Other] {
        let d = TestDir::new("layout-unreadable");
        let path = d.join("layout.json");
        std::fs::write(&path, "unreadable bytes").unwrap();
        std::fs::write(d.join("layout.json.bad"), "earlier diagnostic").unwrap();
        let fs = FaultFs::new(vec![Fault::at(Step::Read).kind(kind)]);
        assert_eq!(LayoutStore::load(&fs, Some(path.clone()), false).1, Layout::default());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "unreadable bytes");
        assert_eq!(std::fs::read_to_string(d.join("layout.json.bad")).unwrap(), "earlier diagnostic");
        assert!(!fs.log().iter().any(|(op, _)| *op == Op::Rename));
    }
}

#[cfg(unix)]
#[test]
fn read_only_directory_stops_after_three_failures_until_real_change() {
    use std::os::unix::fs::PermissionsExt;
    let d = TestDir::new("layout-readonly-dir");
    let permissions = std::fs::metadata(d.path()).unwrap().permissions();
    let (mut store, _) = LayoutStore::load(&RealFs, Some(d.join("layout.json")), false);
    let now = Instant::now();
    let changed = modified();
    store.observe(changed.clone(), now);
    std::fs::set_permissions(d.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
    // Restore before assertions so a failed test does not leave an unwritable scratch directory.
    let results: Vec<_> = (1..=3).map(|n| store.tick(&RealFs, now + SETTLE * n)).collect();
    std::fs::set_permissions(d.path(), permissions).unwrap();
    assert!(results.iter().all(Result::is_err));
    assert_eq!(store.failures, 3);
    assert_eq!(store.next_wake(), None);
    store.observe(changed.clone(), now + SETTLE * 4);
    assert!(!store.tick(&RealFs, now + SETTLE * 60).unwrap());
    assert!(!store.flush(&RealFs, now + SETTLE * 60).unwrap());
    assert!(d.names().is_empty());
    store.observe(Layout { show_control_bar: false, ..changed }, now + SETTLE * 61);
    assert!(store.tick(&RealFs, now + SETTLE * 62).unwrap());
    assert_eq!(store.next_wake(), None);
}

#[test]
fn non_dockable_panels_and_grids_fall_back_and_quarantine() {
    for panel in ["Swatches", "History", "Assets", "Grid"] {
        let d = TestDir::new("layout-unsupported");
        let path = d.join("layout.json");
        let mut layout = modified();
        if panel == "Grid" {
            let root = layout.tree["root"].to_string();
            let children = layout.tree["tiles"]["tiles"][&root]["Container"]["Linear"]["children"].clone();
            layout.tree["tiles"]["tiles"][&root] = serde_json::json!({"Container": {"Grid": {
                "children": children, "layout": {"Columns": usize::MAX}, "col_shares": [], "row_shares": []
            }}});
        } else {
            let tile = layout.tree["tiles"]["tiles"]
                .as_object_mut()
                .unwrap()
                .values_mut()
                .find(|tile| tile["Pane"] == "Layers")
                .unwrap();
            tile["Pane"] = serde_json::json!(panel);
        }
        let bytes = serde_json::to_vec(&Envelope { version: 1, app_build: "test".into(), layout }).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(LayoutStore::load(&RealFs, Some(path.clone()), false).1, Layout::default());
        assert!(!path.exists());
        assert_eq!(std::fs::read(d.join("layout.json.bad")).unwrap(), bytes);
    }
}

#[test]
fn duplicate_panels_and_stale_active_load_without_quarantine() {
    let d = TestDir::new("layout-repaired");
    let path = d.join("layout.json");
    let mut layout = Layout { show_rail: false, ..Layout::default() };
    let tiles = layout.tree["tiles"]["tiles"].as_object_mut().unwrap();
    for tile in tiles.values_mut() {
        if tile["Pane"] == "Properties" {
            tile["Pane"] = serde_json::json!("Align");
        }
        if let Some(tabs) = tile.pointer_mut("/Container/Tabs") {
            tabs["active"] = serde_json::json!(999);
        }
    }
    std::fs::write(&path, serde_json::to_vec(&Envelope { version: 1, app_build: "test".into(), layout }).unwrap())
        .unwrap();
    let (_, loaded) = LayoutStore::load(&RealFs, Some(path), false);
    assert!(!loaded.show_rail);
    let shell = ShellState::from_layout_value(loaded.tree).unwrap();
    assert!(shell.is_open(PanelId::Align));
    assert!(!shell.is_open(PanelId::Properties));
    assert_eq!(d.names(), ["layout.json"]);
}
