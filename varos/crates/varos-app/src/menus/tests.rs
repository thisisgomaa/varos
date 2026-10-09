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

fn edit_rows() -> Vec<Entry> {
    let m = menus();
    m.into_iter().find(|(t, _)| *t == "Edit").expect("an Edit menu").1
}

#[test]
fn edit_menu_mirrors_select_all_deselect_delete() {
    let rows = edit_rows();
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
    assert_eq!(find("edit.selectall"), ("Select All", all, MenuCmd::Key(all.unwrap())));
    assert_eq!(find("edit.deselect"), ("Deselect", none, MenuCmd::Key(none.unwrap())));
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
        ("Snap to Grid", MenuCmd::Snap(SnapRow::Grid), Some(Check::SnapGrid)),
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
    assert_eq!(titles, ["Varos", "File", "Edit", "Object", "View", "Window"]);
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
    assert_eq!(export.len(), 2, "PDF… and Export Selection…");
    assert!(
        matches!(export[0], Entry::Item { label: "PDF\u{2026}", accel: None, cmd: MenuCmd::File(FileCmd::Export), .. }),
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
        ("file.close", cmd(KeyCode::KeyW).unwrap(), FileCmd::CloseTab),
        ("file.save", cmd(KeyCode::KeyS).unwrap(), FileCmd::Save),
        ("file.saveas", cmd_shift(KeyCode::KeyS).unwrap(), FileCmd::SaveAs),
        // slice 0.6, Illustrator's keys: Close All ⌥⌘W · Save a Copy ⌥⌘S · Revert F12 (no ⌘)
        ("file.closeall", cmd_alt(KeyCode::KeyW).unwrap(), FileCmd::CloseAll),
        ("file.savecopy", cmd_alt(KeyCode::KeyS).unwrap(), FileCmd::SaveCopy),
        ("file.revert", fkey(KeyCode::F12).unwrap(), FileCmd::Revert),
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
const ADDED_AFTER_SPLIT: &[&str] = &["file.closeall", "file.savecopy", "file.revert", "file.exportselection"];

fn without_added(menus: Vec<(&'static str, Vec<Entry>)>) -> Vec<(&'static str, Vec<Entry>)> {
    fn strip(v: Vec<Entry>) -> Vec<Entry> {
        v.into_iter()
            .filter(|e| {
                !matches!(e, Entry::Item { id, .. } if ADDED_AFTER_SPLIT.contains(&id.as_str()))
                    && !matches!(e, Entry::Sub { label: "Transform" | "Layers", .. })
            })
            .map(|e| match e {
                Entry::Sub { label, items } => Entry::Sub { label, items: strip(items) },
                e => e,
            })
            .collect()
    }
    menus.into_iter().map(|(t, v)| (t, strip(v))).collect()
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
            "---",
            "file.close",
            "file.closeall",
            "file.save",
            "file.saveas",
            "file.savecopy",
            "file.revert",
            "---",
            "sub Export",
            "file.exportselection",
        ]
    );
}

/// Only a function key may be a bare (⌘-less) menu key: a bare letter would steal typing from a field.
#[test]
fn only_function_keys_go_without_command() {
    for e in flat_items(&menus()) {
        if let Entry::Item { id, accel: Some(a), .. } = e {
            if !a.cmd {
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
