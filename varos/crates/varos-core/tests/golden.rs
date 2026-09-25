//! FROZEN golden `.vrs` fixtures: never regenerate these from current code.
//! Add a new fixture for a new file era instead.

use std::path::PathBuf;

use varos_core::file::{doc_from_blob, doc_to_blob, load_vrs, save_vrs};
use varos_core::model::{Document, NodeKind, Paint, Path as ModelPath};

/// The v1 corpus frozen by DFS S5-E (`tests/fixtures/v1/README.md`): every scenario as raw JSON.
/// Never regenerate; add a new fixture for a new file era instead.
const V1_CORPUS: [&str; 8] = [
    "v1/v1_plain.vrs",
    "v1/v1_boardless.vrs",
    "v1/v1_masked.vrs",
    "v1/v1_rotated.vrs",
    "v1/v1_translucent.vrs",
    "v1/v1_two_artboards.vrs",
    "v1/v1_guides_snap.vrs",
    "v1/v1_unicode_arabic.vrs",
];

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join(name)
}

fn load_fixture(name: &str) -> Document {
    load_vrs(&fixture(name)).unwrap_or_else(|e| panic!("{name} should load through load_vrs: {e}"))
}

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("varos-golden-{}-{name}", std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

fn path_ids(doc: &Document) -> Vec<u32> {
    doc.paths.iter().map(|p| p.id).collect()
}

fn model_path(doc: &Document, id: u32) -> &ModelPath {
    doc.paths.iter().find(|p| p.id == id).unwrap_or_else(|| panic!("missing path {id}"))
}

#[test]
fn every_golden_fixture_obeys_the_full_round_trip_law() {
    let names: Vec<&str> =
        ["ancient_pre_artboards.vrs", "legacy_groups.vrs", "pre_paint_enum.vrs"].into_iter().chain(V1_CORPUS).collect();
    for name in names {
        let loaded = load_fixture(name);
        let bytes_a = doc_to_blob(&loaded).unwrap_or_else(|e| panic!("{name}: first save failed: {e}")).into_bytes();
        let reloaded = doc_from_blob(std::str::from_utf8(&bytes_a).expect("serializer emits UTF-8"))
            .unwrap_or_else(|e| panic!("{name}: reload of bytes A failed: {e}"));

        assert_eq!(reloaded, loaded, "{name}: reloading bytes A must preserve the Document");

        let bytes_b = doc_to_blob(&reloaded).unwrap_or_else(|e| panic!("{name}: second save failed: {e}")).into_bytes();
        assert_eq!(bytes_a, bytes_b, "{name}: consecutive current-format saves must be byte-stable");
    }
}

/// The DFS S5 charter round-trip law (`FOUNDATION_CHARTER.md:45`), exercised on disk over the frozen
/// v1 corpus: load → save → reload must equal, by **content** (`Document::content_eq` — the same rule
/// the app's dirty dot uses), not merely by raw struct equality, so this test would still pass even if
/// a future migration changed a preference field (`ids`, `active`, …) that content_eq deliberately
/// ignores. Round-tripping through real files (not just blobs) also exercises `save_vrs`/`load_vrs`,
/// i.e. the exact path the app takes.
#[test]
fn frozen_v1_corpus_round_trips_on_disk_by_content() {
    for name in V1_CORPUS {
        let loaded = load_fixture(name);
        let p = tmp(&name.replace('/', "_"));
        save_vrs(&loaded, &p).unwrap_or_else(|e| panic!("{name}: save failed: {e}"));
        let reloaded = load_vrs(&p).unwrap_or_else(|e| panic!("{name}: reload failed: {e}"));
        assert!(
            loaded.content_eq(&reloaded),
            "{name}: load -> save -> load must preserve every authored-content field"
        );
        let _ = std::fs::remove_file(&p);
    }
}

#[test]
fn ancient_pre_artboards_loads_with_legacy_bleed_and_exact_art() {
    let doc = load_fixture("ancient_pre_artboards.vrs");

    assert_eq!(doc.artboards.len(), 1);
    assert!(!doc.artboards[0].clip, "pre-artboard files must keep legacy bleed behavior");
    assert_eq!(path_ids(&doc), vec![1, 5]);

    let triangle = model_path(&doc, 1);
    assert_eq!(triangle.fill, Paint::Solid([0.25, 0.5, 0.75, 1.0]));
    assert_eq!(triangle.stroke, Paint::Solid([0.0, 0.0, 0.0, 1.0]));
    assert_eq!(triangle.stroke_width, 3.0);
    assert_eq!(triangle.opacity, 0.875);
    assert_eq!(triangle.anchors[0].p, [10.0, 10.0]);
    assert_eq!(triangle.anchors[2].p, [50.0, 80.0]);

    let curve = model_path(&doc, 5);
    assert_eq!(curve.fill, Paint::None);
    assert_eq!(curve.stroke, Paint::Solid([1.0, 0.25, 0.0, 1.0]));
    assert_eq!(curve.anchors[0].hout, Some([145.0, 5.0]));
    assert_eq!(curve.anchors[1].hin, Some([150.0, 35.0]));
}

#[test]
fn legacy_group_registry_migrates_to_tree_without_changing_z_order() {
    let doc = load_fixture("legacy_groups.vrs");

    assert_eq!(path_ids(&doc), vec![1, 5, 9], "flat storage stays back-to-front");
    assert!(doc.groups.is_empty(), "legacy registry is consumed by migration");
    assert!(doc.group_of.is_empty(), "legacy membership map is consumed by migration");
    assert_eq!(doc.roots.len(), 1);
    assert_eq!(doc.node_paths(doc.roots[0]), vec![1, 5, 9]);
    assert_eq!(doc.group_members(5), vec![5, 9]);
    assert_eq!(doc.group_members(9), vec![5, 9]);
    assert_eq!(doc.group_members(1), vec![1]);

    let top = doc.top_group_of_path(9).expect("nested legacy path should have a migrated top group");
    let top_node = doc.node(top).expect("top group node exists");
    assert!(matches!(top_node.kind, NodeKind::Group));
    assert_eq!(top_node.name, "Legacy pair");
    assert_eq!(doc.node_paths(top), vec![5, 9]);
}

#[test]
fn pre_paint_enum_raw_option_colours_load_as_paint() {
    let doc = load_fixture("pre_paint_enum.vrs");

    assert_eq!(path_ids(&doc), vec![1, 5]);
    assert_eq!(doc.node_paths(doc.roots[0]), vec![1, 5]);

    let solid = model_path(&doc, 1);
    assert_eq!(solid.fill, Paint::Solid([0.25, 0.5, 0.75, 1.0]));
    assert_eq!(solid.stroke, Paint::Solid([1.0, 0.125, 0.0, 0.625]));

    let none = model_path(&doc, 5);
    assert_eq!(none.fill, Paint::None);
    assert_eq!(none.stroke, Paint::None);
}
