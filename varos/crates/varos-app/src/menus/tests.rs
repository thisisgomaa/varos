//! The menu-table checks (moved verbatim from `chrome.rs`, slice 0.6).

use super::*;

#[test]
fn every_shortcut_item_shows_exactly_the_keystroke_it_sends() {
    for e in flat_items(&menus()) {
        if let Entry::Item { id, accel, cmd: MenuCmd::Key(k), .. } = e {
            assert_eq!(accel, Some(k), "{id}: shown shortcut ≠ sent keystroke");
        }
    }
}

#[test]
fn ids_and_accelerators_are_unique() {
    let items = flat_items(&menus());
    let mut ids = std::collections::HashSet::new();
    let mut accels = std::collections::HashSet::new();
    for e in &items {
        if let Entry::Item { id, accel, .. } = e {
            assert!(ids.insert(id.clone()), "duplicate id {id}");
            if let Some(a) = accel {
                assert!(accels.insert(*a), "{id}: accelerator {a:?} used twice");
            }
        }
    }
}

#[test]
fn every_menu_key_can_be_handed_to_a_text_field() {
    for e in flat_items(&menus()) {
        if let Entry::Item { id, accel: Some(a), .. } = e {
            assert!(egui_key(a.code).is_some(), "{id}: no egui key for {:?}", a.code);
        }
    }
}

#[test]
fn every_clipboard_row_is_its_shortcut() {
    let m = menus();
    let (_, edit) = m.iter().find(|(t, _)| *t == "Edit").expect("an Edit menu");
    let rows: Vec<(String, Accel)> = edit
        .iter()
        .filter_map(|e| match e {
            Entry::Item { id, cmd: MenuCmd::Key(k), .. } => Some((id.clone(), *k)),
            _ => None,
        })
        .collect();
    let want = [
        ("edit.cut", cmd(KeyCode::KeyX)),
        ("edit.copy", cmd(KeyCode::KeyC)),
        ("edit.paste", cmd(KeyCode::KeyV)),
        ("edit.pasteinplace", cmd_shift(KeyCode::KeyV)),
    ];
    for (id, a) in want {
        let a = a.unwrap();
        assert!(rows.iter().any(|(i, k)| i == id && *k == a), "Edit menu misses {id} = {a:?}");
        assert!(egui_key(a.code).is_some(), "{id}: a focused text field must still get the key");
    }
}

#[test]
fn edit_menu_mirrors_select_all_deselect_delete() {
    let rows = flat_items(&menus());
    let find = |want: &str| {
        rows.iter()
            .find_map(|e| match e {
                Entry::Item { id, label, accel, cmd, .. } if id == want => Some((*label, *accel, *cmd)),
                _ => None,
            })
            .unwrap_or_else(|| panic!("Edit menu misses {want}"))
    };
    let all = cmd(KeyCode::KeyA);
    let none = cmd_shift(KeyCode::KeyA);
    assert_eq!(find("select.all"), ("All", all, MenuCmd::Key(all.unwrap())));
    assert_eq!(find("select.deselect"), ("Deselect", none, MenuCmd::Key(none.unwrap())));
    assert_eq!(find("edit.delete"), ("Delete", None, MenuCmd::Plain(KeyCode::Backspace)));
    assert!(egui_key(KeyCode::KeyA).is_some(), "a focused text field must still get ⌘A (select text)");
}

#[test]
fn plain_delete_row_has_no_native_key_equivalent() {
    // every Plain row is click-only: showing a key would let AppKit steal it from a text field
    let items = flat_items(&menus());
    let plain: Vec<_> = items
        .iter()
        .filter_map(|e| match e {
            Entry::Item { id, accel, cmd: MenuCmd::Plain(_), .. } => Some((id.as_str(), *accel)),
            _ => None,
        })
        .collect();
    assert_eq!(plain, [("edit.delete", None)]);
    // …and no row anywhere claims Backspace / Delete as a key equivalent
    for e in &items {
        if let Entry::Item { id, accel: Some(a), .. } = e {
            assert!(!matches!(a.code, KeyCode::Backspace | KeyCode::Delete), "{id} claims {:?}", a.code);
        }
    }
}

/// The band's V mark runs the app menu's item 0 (`mac_menu::show_about`) — it must stay About.
#[test]
fn about_is_the_first_application_menu_row() {
    let m = menus();
    assert_eq!(m[0].0, "Varos");
    assert_eq!(m[0].1[0], Entry::Native(Native::About));
}

/// 4b removed the magnet: every one of its rows lives in View, each a check row on its own flag.
#[test]
fn view_menu_mirrors_every_snapping_row() {
    let m = menus();
    let (_, view) = m.iter().find(|(t, _)| *t == "View").expect("a View menu");
    let rows: Vec<(&str, MenuCmd, Option<Check>)> = view
        .iter()
        .filter_map(|e| match e {
            Entry::Item { label, cmd, check, .. } => Some((*label, *cmd, *check)),
            _ => None,
        })
        .collect();
    let want = [
        ("Smart Guides", MenuCmd::Key(cmd(KeyCode::KeyU).unwrap()), Some(Check::SmartGuides)),
        ("Alignment Guides", MenuCmd::Snap(SnapRow::AlignGuides), Some(Check::AlignGuides)),
        ("Geometric Guides", MenuCmd::Snap(SnapRow::GeomGuides), Some(Check::GeomGuides)),
        ("Snap to Grid", MenuCmd::Key(cmd_shift(KeyCode::Quote).unwrap()), Some(Check::SnapGrid)),
        ("Snap to Point", MenuCmd::Snap(SnapRow::Point), Some(Check::SnapPoint)),
    ];
    for w in want {
        assert!(rows.contains(&w), "View misses {w:?}");
    }
    // the two guide rows sit right under Smart Guides, as they did in the magnet menu
    let at = |l: &str| rows.iter().position(|r| r.0 == l).unwrap();
    assert_eq!((at("Alignment Guides"), at("Geometric Guides")), (at("Smart Guides") + 1, at("Smart Guides") + 2));
}

#[test]
fn the_bar_has_the_standard_mac_menus_and_mirrors_every_dockable_panel() {
    let m = menus();
    let titles: Vec<&str> = m.iter().map(|(t, _)| *t).collect();
    assert_eq!(titles, ["Varos", "File", "Edit", "Select", "Object", "View", "Window", "Help"]);
    let items = flat_items(&m);
    let has = |c: MenuCmd| items.iter().any(|e| matches!(e, Entry::Item { cmd, .. } if *cmd == c));
    for p in PanelId::DOCKABLE {
        assert!(has(MenuCmd::TogglePanel(p)), "Window menu misses {}", p.title());
    }
    assert!(has(MenuCmd::ToggleRail) && has(MenuCmd::ToggleDock));
    assert!(has(MenuCmd::ResetLayout));
    // ⌘Q quits the app; ⌘W closes only the active tab — two DIFFERENT FileCmds (review F5: no
    // longer both folded into one "Close Window" path).
    assert!(has(MenuCmd::File(FileCmd::Quit)), "Varos ▸ Quit is File(FileCmd::Quit)");
    assert!(has(MenuCmd::File(FileCmd::CloseTab)), "File ▸ Close Tab is File(FileCmd::CloseTab)");
    // DFS S6: File ▸ Export ▸ PDF… is the Export command row, with no shortcut (spec: none, R6);
    // slice 0.6: Export Selection… beside it, also without a key (Illustrator has none)
    let export: Vec<&Entry> =
        items.iter().filter(|e| matches!(e, Entry::Item { id, .. } if id.contains("export"))).collect();
    assert_eq!(export.len(), 3, "Export…, PDF… preset and Export Selection…");
    assert!(
        matches!(export[0], Entry::Item { accel: Some(a), cmd: MenuCmd::File(FileCmd::Export), .. } if *a == cmd_alt(KeyCode::KeyE).unwrap())
    );
    let export = &export[1..];
    assert!(
        matches!(
            export[0],
            Entry::Item { label: "PDF\u{2026}", accel: None, cmd: MenuCmd::File(FileCmd::ExportPdfPreset), .. }
        ),
        "{:?}",
        export[0]
    );
    assert!(
        matches!(
            export[1],
            Entry::Item {
                label: "Export Selection\u{2026}",
                accel: None,
                cmd: MenuCmd::File(FileCmd::ExportSelection),
                ..
            }
        ),
        "{:?}",
        export[1]
    );
}

#[test]
fn file_menu_rows_are_new_open_close_save_saveas_on_their_keys() {
    let m = menus();
    let (_, file) = m.iter().find(|(t, _)| *t == "File").expect("a File menu");
    let rows: Vec<(&str, Accel, FileCmd)> = file
        .iter()
        .filter_map(|e| match e {
            Entry::Item { id, accel: Some(a), cmd: MenuCmd::File(fc), .. } => Some((id.as_str(), *a, *fc)),
            _ => None,
        })
        .collect();
    let want = [
        ("file.new", cmd(KeyCode::KeyN).unwrap(), FileCmd::New),
        ("file.open", cmd(KeyCode::KeyO).unwrap(), FileCmd::Open),
        // Lane H's artwork Place now shares the ONE ⇧⌘P Place… (integration w2; "file.place" below)
        ("file.document-setup", cmd_alt(KeyCode::KeyP).unwrap(), FileCmd::DocumentSetup),
        ("file.close", cmd(KeyCode::KeyW).unwrap(), FileCmd::CloseTab),
        ("file.save", cmd(KeyCode::KeyS).unwrap(), FileCmd::Save),
        ("file.saveas", cmd_shift(KeyCode::KeyS).unwrap(), FileCmd::SaveAs),
        // slice 0.6, Illustrator's keys: Close All ⌥⌘W · Save a Copy ⌥⌘S · Revert F12 (no ⌘)
        ("file.closeall", cmd_alt(KeyCode::KeyW).unwrap(), FileCmd::CloseAll),
        ("file.savecopy", cmd_alt(KeyCode::KeyS).unwrap(), FileCmd::SaveCopy),
        ("file.revert", fkey(KeyCode::F12).unwrap(), FileCmd::Revert),
        ("file.export", cmd_alt(KeyCode::KeyE).unwrap(), FileCmd::Export),
        ("file.print", cmd(KeyCode::KeyP).unwrap(), FileCmd::Print),
        ("file.place", cmd_shift(KeyCode::KeyP).unwrap(), FileCmd::Place),
    ];
    for (id, accel, fc) in want {
        assert!(rows.iter().any(|&(i, a, f)| i == id && a == accel && f == fc), "File menu misses {id}");
    }
    assert_eq!(rows.len(), want.len(), "no extra File rows go through MenuCmd::Key any more (review F5)");
    assert!(
        file.iter().all(|e| !matches!(e, Entry::Item { cmd: MenuCmd::Key(_), .. })),
        "File rows never dispatch through the synthetic-key path (spec §4)"
    );
}

/// A stable text rendering of the menu model: one line per entry, indented by depth. Accelerators are
/// written out (`⌘⌥⇧Code`) rather than through `Debug`, so the rendering is about the menu a user sees.
fn snapshot(menus: &[(&'static str, Vec<Entry>)]) -> String {
    use std::fmt::Write;
    fn accel_text(a: &Accel) -> String {
        let cmd = if a.cmd { "\u{2318}" } else { "" };
        format!("{cmd}{}{}{:?}", if a.alt { "\u{2325}" } else { "" }, if a.shift { "\u{21e7}" } else { "" }, a.code)
    }
    fn walk(v: &[Entry], depth: usize, out: &mut String) {
        let pad = "  ".repeat(depth);
        for e in v {
            let _ = match e {
                Entry::Item { id, label, accel, cmd, check } => {
                    let cmd = match cmd {
                        MenuCmd::Key(k) => format!("Key({})", accel_text(k)),
                        other => format!("{other:?}"),
                    };
                    let accel = accel.as_ref().map_or_else(|| "-".to_string(), accel_text);
                    writeln!(out, "{pad}item {id} | {label} | {accel} | {cmd} | {check:?}")
                }
                Entry::Sub { label, items } => {
                    let _ = writeln!(out, "{pad}sub {label}");
                    walk(items, depth + 1, out);
                    Ok(())
                }
                Entry::Native(n) => writeln!(out, "{pad}native {n:?}"),
                Entry::Sep => writeln!(out, "{pad}---"),
            };
        }
    }
    let mut out = String::new();
    for (title, v) in menus {
        let _ = writeln!(out, "menu {title}");
        walk(v, 1, &mut out);
    }
    out
}

/// The rows slice 0.6 added after the split; everything else is the pre-split table.
const ADDED_AFTER_SPLIT: &[&str] = &[
    "view.outline",
    "view.pixelpreview",
    "view.snappixel",
    "view.movepixel",
    "view.trim",
    "view.presentation",
    "view.transparency",
    "view.canvas.dark",
    "view.canvas.mid",
    "view.canvas.light",
    "win.panel.Navigator",
    "file.place",
    "file.package",
    "win.panel.Links",
    "obj.image.trace",
    "obj.image.rasterize",
    "obj.image.crop",
    "file.closeall",
    "file.savecopy",
    "file.revert",
    "file.exportselection",
    "file.place.svg",
    "file.export",
    "file.print",
    "view.fitall",
    "view.makeguides",
    "view.releaseguides",
    "view.clearguides",
    "view.grid",
    "file.new-template",
    "file.save-template",
    "file.document-setup",
    "win.document-info",
];

fn without_added(menus: Vec<(&'static str, Vec<Entry>)>) -> Vec<(&'static str, Vec<Entry>)> {
    fn strip(v: Vec<Entry>) -> Vec<Entry> {
        v.into_iter()
            .filter(|e| {
                !matches!(e, Entry::Sub { label: "Clipping Mask" | "Transform" | "Layers", .. })
                    && !matches!(e, Entry::Item { id, .. } if (ADDED_AFTER_SPLIT.contains(&id.as_str()) || ["app.preferences","app.shortcuts","app.actions","win.panel.History","win.panel.Actions"].contains(&id.as_str())))
            })
            .map(|e| match e {
                Entry::Item { id, accel, cmd, check, .. } if id == "file.new" => {
                    Entry::Item { id, label: "New", accel, cmd, check }
                }
                Entry::Item { id, .. } if id == "view.snapgrid" => {
                    toggle("view.snapgrid", "Snap to Grid", MenuCmd::Snap(SnapRow::Grid), Check::SnapGrid)
                }
                Entry::Sub { label, items } => Entry::Sub { label, items: strip(items) },
                Entry::Item { id, label, accel, cmd: MenuCmd::File(FileCmd::ExportPdfPreset), check } => {
                    Entry::Item { id, label, accel, cmd: MenuCmd::File(FileCmd::Export), check }
                }
                e => e,
            })
            .collect()
    }
    menus
        .into_iter()
        .filter(|(t, _)| *t != "Select" && *t != "Help")
        .map(|(t, v)| {
            let mut v = strip(v);
            if t == "Object" {
                if let Some(i) = v.iter().position(|e| matches!(e,Entry::Item { id,.. } if id == "obj.ungroup")) {
                    v.truncate(i + 1);
                }
            }
            if t == "Edit" {
                v.extend([
                    key("edit.selectall", "Select All", cmd(KeyCode::KeyA)),
                    key("edit.deselect", "Deselect", cmd_shift(KeyCode::KeyA)),
                ]);
            }
            (t, v)
        })
        .collect()
}

/// Slice 0.6: the split of `chrome.rs`'s menu tables into `menus/` was a pure move — the assembled
/// table (minus the rows added afterwards, listed above) is byte-identical to the snapshot taken from
/// the pre-split code at `ba7e6ed`.
#[test]
fn assembled_menus_are_byte_identical_to_the_pre_split_table() {
    let now = snapshot(&without_added(menus()));
    assert_eq!(now, include_str!("pre_split_snapshot.txt"));
}

/// Slice 0.6: the new File rows sit where Illustrator puts them — Close All under Close, Save a Copy
/// and Revert under Save As, Export Selection… after the Export submenu.
#[test]
fn file_menu_order_has_the_slice_0_6_rows_in_illustrator_places() {
    let m = menus();
    let (_, file) = m.iter().find(|(t, _)| *t == "File").expect("a File menu");
    let order: Vec<String> = file
        .iter()
        .map(|e| match e {
            Entry::Item { id, .. } => id.clone(),
            Entry::Sub { label, .. } => format!("sub {label}"),
            Entry::Sep => "---".into(),
            Entry::Native(n) => format!("{n:?}"),
        })
        .collect();
    assert_eq!(
        order,
        [
            "file.new",
            "file.open",
            "sub Open Recent",
            "file.place",
            "file.package",
            "file.place.svg",
            "file.new-template",
            "file.save-template",
            "file.document-setup",
            "---",
            "file.close",
            "file.closeall",
            "file.save",
            "file.saveas",
            "file.savecopy",
            "file.revert",
            "---",
            "file.export",
            "sub Export",
            "file.exportselection",
            "file.print",
        ]
    );
}

/// Only a function key may be a bare (⌘-less) menu key: a bare letter would steal typing from a field.
#[test]
fn only_function_keys_go_without_command() {
    for e in flat_items(&menus()) {
        if let Entry::Item { id, accel: Some(a), .. } = e {
            if !a.cmd {
                if id == "view.presentation" {
                    assert_eq!(a.code, KeyCode::KeyF);
                    assert!(a.shift && !a.alt);
                    continue;
                }
                assert!(matches!(a.code, KeyCode::F12), "{id}: a bare {:?} as a menu key", a.code);
                assert!(!a.shift && !a.alt, "{id}");
            }
        }
    }
}

/// Revert needs a file with unsaved changes, Export Selection a selection, the document rows a
/// document; New / Open / Quit always work (Home included).
#[test]
fn file_rows_enable_from_the_document_state() {
    let home = DocMenuState::default();
    let doc = DocMenuState { active: true, ..home };
    let dirty_file = DocMenuState { can_revert: true, ..doc };
    let selected = DocMenuState { has_selection: true, ..doc };
    for f in [FileCmd::New, FileCmd::Open, FileCmd::Quit] {
        assert!(file_row_enabled(f, home), "{f:?} on Home");
    }
    for f in [FileCmd::CloseTab, FileCmd::CloseAll, FileCmd::Save, FileCmd::SaveAs, FileCmd::SaveCopy, FileCmd::Export]
    {
        assert!(!file_row_enabled(f, home), "{f:?} needs a document");
        assert!(file_row_enabled(f, doc), "{f:?}");
    }
    assert!(!file_row_enabled(FileCmd::Revert, doc), "clean, or no file: nothing to revert to");
    assert!(file_row_enabled(FileCmd::Revert, dirty_file));
    assert!(!file_row_enabled(FileCmd::Revert, DocMenuState { active: false, ..dirty_file }));
    assert!(!file_row_enabled(FileCmd::ExportSelection, doc), "no selection");
    assert!(file_row_enabled(FileCmd::ExportSelection, selected));
}

#[test]
fn artwork_place_row_is_namespaced_and_uses_illustrator_shortcut() {
    // Integration w2: ⇧⌘P belongs to the ONE Place… (images + artwork, routed by file type); the
    // vector-only row keeps its id and command without a second binding.
    let rows = super::file::rows();
    let row = rows.iter().find(|row| matches!(row, Entry::Item { id, .. } if id == "file.place.svg"));
    assert!(matches!(
        row,
        Some(Entry::Item { label: "Place artwork…", accel: None, cmd: MenuCmd::File(FileCmd::PlaceSvg), .. })
    ));
    let place = rows.iter().find(|row| matches!(row, Entry::Item { id, .. } if id == "file.place"));
    assert!(matches!(
        place,
        Some(Entry::Item {
            label: "Place…",
            accel: Some(Accel { code: KeyCode::KeyP, shift: true, alt: false, cmd: true }),
            cmd: MenuCmd::File(FileCmd::Place),
            ..
        })
    ));
}
#[test]
fn clipping_rows_use_illustrator_shortcuts_and_core_commands() {
    let items = flat_items(&menus());
    for (id, alt) in [("obj.clip", false), ("obj.release_clip", true)] {
        let a = items
            .iter()
            .find_map(|e| match e {
                Entry::Item { id: found, accel, .. } if found == id => *accel,
                _ => None,
            })
            .unwrap();
        assert_eq!(a, Accel { code: KeyCode::Digit7, cmd: true, shift: false, alt });
    }
    let mut ed = varos_core::Editor::new();
    for x in [0., 10.] {
        ed.try_execute_created(varos_core::EditCommand::AddShape {
            kind: varos_core::model::ShapeKind::Rect,
            bounds: [x, x, 40., 40.],
            parent: None,
            fill: Some([1.; 4]),
            stroke: None,
            stroke_width: 0.,
            opacity: 1.,
            name: None,
        })
        .unwrap();
    }
    ed.select_all();
    let mut expected = ed.clone();
    expected.try_execute(varos_core::EditCommand::ClipMake).unwrap();
    let mut view = varos_core::geom::View::identity();
    crate::apply_key(&mut ed, &mut view, [0., 0.], "Digit7", true, false, false);
    assert_eq!(ed.doc, expected.doc);
    expected.try_execute(varos_core::EditCommand::ClipRelease).unwrap();
    crate::apply_key(&mut ed, &mut view, [0., 0.], "Digit7", true, false, true);
    assert_eq!(ed.doc, expected.doc);
}
#[test]
fn select_menu_is_between_edit_and_object_and_has_six_same_modes() {
    let m = menus();
    let index = m.iter().position(|(name, _)| *name == "Select").unwrap();
    assert_eq!(m[index - 1].0, "Edit");
    assert_eq!(m[index + 1].0, "Object");
    let same = m[index]
        .1
        .iter()
        .find_map(|e| if let Entry::Sub { label: "Same", items } = e { Some(items) } else { None })
        .unwrap();
    assert_eq!(same.len(), 6);
    assert!(same.iter().all(|e| matches!(e, Entry::Item { cmd: MenuCmd::Selection(_), accel: None, .. })));
}

#[test]
fn redo_uses_only_shift_command_z_across_menu_mirrors() {
    let rows = flat_items(&menus());
    let redo = rows.iter().find(|row| matches!(row, Entry::Item { id, .. } if id == "edit.redo")).unwrap();
    let chord = cmd_shift(KeyCode::KeyZ).unwrap();
    assert!(matches!(redo, Entry::Item { accel: Some(a), cmd: MenuCmd::Key(k), .. } if *a == chord && *k == chord));
    assert!(rows.iter().any(|row| matches!(row, Entry::Item { id, cmd: MenuCmd::View(_), accel: Some(a), .. } if id == "view.outline" && a.code == KeyCode::KeyY && a.cmd && !a.shift && !a.alt)));
}
