//! FROZEN golden `.vrs` fixtures: never regenerate these from current code.
//! Add a new fixture for a new file era instead.

use std::path::PathBuf;

use varos_core::file::load_vrs;
use varos_core::model::{Document, NodeKind, Paint, Path as ModelPath};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join(name)
}

fn load_fixture(name: &str) -> Document {
    load_vrs(&fixture(name)).unwrap_or_else(|e| panic!("{name} should load through load_vrs: {e}"))
}

fn path_ids(doc: &Document) -> Vec<u32> {
    doc.paths.iter().map(|p| p.id).collect()
}

fn model_path(doc: &Document, id: u32) -> &ModelPath {
    doc.paths.iter().find(|p| p.id == id).unwrap_or_else(|| panic!("missing path {id}"))
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
