use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use varos_core::model::{Anchor, Artboard, Document, Paint, Path, Xform};
use varos_core::svg::*;

fn rect(id: u32, x: f32, y: f32, w: f32, h: f32) -> Path {
    Path::new(
        id,
        [[x, y], [x + w, y], [x + w, y + h], [x, y + h]]
            .into_iter()
            .enumerate()
            .map(|(i, p)| Anchor { id: id * 10 + i as u32, p, hin: None, hout: None, smooth: false })
            .collect(),
        true,
        Some([1.0, 0.0, 0.0, 1.0]),
        None,
        1.0,
    )
}
fn doc(paths: Vec<Path>) -> Document {
    let mut d = Document { paths, ids: 1000, ..Document::default() };
    d.sync_tree();
    d
}
fn svg(d: &Document, scope: ExportScope) -> String {
    let p = plan_svg_export(d, scope).unwrap();
    String::from_utf8(export_svg_files(d, &p, &AtomicBool::new(false)).unwrap().remove(0).bytes).unwrap()
}
fn xml_check(s: &str) {
    let tree = roxmltree::Document::parse(s).unwrap();
    let mut ids = HashSet::new();
    for n in tree.descendants() {
        if let Some(id) = n.attribute("id") {
            assert!(ids.insert(id), "duplicate {id}");
        }
    }
    resvg::usvg::Tree::from_data(s.as_bytes(), &resvg::usvg::Options::default()).unwrap();
}
#[test]
fn plans_visible_artboards_active_and_whole_board_floaters() {
    let mut d = doc(vec![rect(2, -100.0, -50.0, 20.0, 10.0), rect(3, 10.0, 10.0, 30.0, 30.0)]);
    d.artboards = vec![
        Artboard { x: 0.0, y: 0.0, w: 100.0, h: 100.0, ..Artboard::default() },
        Artboard { x: 200.0, hidden: true, ..Artboard::default() },
    ];
    assert_eq!(default_scope(&d), ExportScope::AllVisibleArtboards);
    let p = plan_svg_export(&d, ExportScope::AllVisibleArtboards).unwrap();
    assert_eq!(p.pages.len(), 1);
    let s = svg(&d, ExportScope::ActiveArtboard);
    assert!(s.contains("viewBox=\"0.000 0.000 100.000 100.000\""));
    assert!(!s.contains("id=\"path-2\""));
    assert_bounds(plan_svg_export(&d, ExportScope::WholeBoard).unwrap().pages[0].rect, [-100.0, -50.0, 140.0, 90.0]);
    d.active = 1;
    assert_eq!(plan_svg_export(&d, ExportScope::ActiveArtboard), Err(ExportError::ActiveArtboardHidden));
    d.artboards[0].hidden = true;
    assert_eq!(plan_svg_export(&d, ExportScope::AllVisibleArtboards), Err(ExportError::NoVisibleArtboards));
}
#[test]
fn curves_keep_handles_closing_cubic_holes_and_world_rotation() {
    let mut p = rect(2, 10.0, 10.0, 20.0, 20.0);
    p.anchors[0].hin = Some([5.12345, 8.98765]);
    p.anchors[3].hout = Some([3.33333, 28.0]);
    p.holes = vec![rect(3, 15.0, 15.0, 5.0, 5.0).anchors];
    let mut d = doc(vec![p]);
    let s = svg(&d, ExportScope::WholeBoard);
    assert!(s.contains("C3.333 28.000 5.123 8.988 10.000 10.000 Z"));
    assert_eq!(s.matches("Z ").count(), 2);
    assert!(s.contains("fill-rule=\"evenodd\""));
    let unit = d.unit_of(2).unwrap();
    d.set_node_xform(unit, Xform { rot: std::f32::consts::FRAC_PI_2, piv: [0.0, 0.0] });
    let s = svg(&d, ExportScope::WholeBoard);
    assert!(s.contains("M-10.000 10.000"));
    assert!(s.contains("C-28.000 3.333 -8.988 5.123 -10.000 10.000 Z"));
    xml_check(&s);
}
#[test]
fn open_fill_does_not_close_stroke_and_two_anchors_do_not_fill() {
    let mut p = rect(2, 0.0, 0.0, 20.0, 20.0);
    p.closed = false;
    p.stroke = Paint::Solid([0.0, 0.0, 0.0, 1.0]);
    let s = svg(&doc(vec![p.clone()]), ExportScope::WholeBoard);
    assert!(!s.contains(" Z "));
    assert!(s.contains("fill=\"#ff0000\""));
    p.anchors.truncate(2);
    let s = svg(&doc(vec![p]), ExportScope::WholeBoard);
    assert!(s.contains("fill=\"none\""));
}
#[test]
fn hidden_cascades_locked_exports_and_names_are_safe_unique_ids() {
    let mut a = rect(2, 0.0, 0.0, 20.0, 20.0);
    a.name = Some("اسم & < \" _26_\u{1}".into());
    a.locked = true;
    let mut b = rect(3, 30.0, 0.0, 20.0, 20.0);
    b.name = a.name.clone();
    let mut d = doc(vec![a, b]);
    let s = svg(&d, ExportScope::WholeBoard);
    xml_check(&s);
    assert!(s.contains("&amp; &lt;"));
    assert!(s.contains("id=\"path-2-اسم_x20__x26__x20__x3C__x20__x22__x20__x5F_26_x5F__x01_\""));
    assert!(s.contains("id=\"path-3-"));
    let group = d.group(&[2, 3]);
    d.toggle_node_hidden(group.unwrap());
    assert_eq!(plan_svg_export(&d, ExportScope::WholeBoard), Err(ExportError::NothingToExport));
}
#[test]
fn opacity_isolation_precedes_knockout_and_zero_alpha_stroke_removes_fill() {
    let mut p = rect(2, 0.0, 0.0, 50.0, 50.0);
    p.stroke = Paint::Solid([0.0, 0.0, 0.0, 0.0]);
    p.stroke_width = 10.0;
    let s = svg(&doc(vec![p.clone()]), ExportScope::WholeBoard);
    assert!(s.contains("<mask"));
    assert!(s.contains("stroke-opacity=\"0.000\""));
    p.opacity = 0.5;
    let s = svg(&doc(vec![p]), ExportScope::WholeBoard);
    assert!(!s.contains("<mask"));
    assert!(s.contains("opacity=\"0.500\""));
}
#[test]
fn clip_source_is_geometry_only_even_when_hidden_and_compound_mask_is_one_path() {
    let mut mask = rect(2, 0.0, 0.0, 40.0, 40.0);
    mask.hidden = true;
    mask.holes = vec![rect(4, 10.0, 10.0, 10.0, 10.0).anchors];
    let mut d = doc(vec![mask, rect(3, -20.0, -20.0, 100.0, 100.0)]);
    d.clip_group(&[2, 3], 2).unwrap();
    let s = svg(&d, ExportScope::WholeBoard);
    assert!(s.contains("<clipPath"));
    assert!(s.contains("clip-rule=\"evenodd\""));
    assert!(!s.contains("id=\"path-2\""));
    assert!(s.contains("id=\"path-3\""));
    xml_check(&s);
    assert_bounds(plan_svg_export(&d, ExportScope::WholeBoard).unwrap().pages[0].rect, [0.0, 0.0, 40.0, 40.0]);
}
#[test]
fn cancellation_empty_invalid_geometry_and_cyclic_tree_are_errors() {
    let d = doc(vec![rect(2, 0.0, 0.0, 10.0, 10.0)]);
    let mut plan = plan_svg_export(&d, ExportScope::WholeBoard).unwrap();
    assert_eq!(export_svg_files(&d, &plan, &AtomicBool::new(true)), Err(ExportError::Cancelled));
    plan.pages[0].rect[2] = f32::NAN;
    assert_eq!(export_svg_files(&d, &plan, &AtomicBool::new(false)), Err(ExportError::InvalidPage));
    assert_eq!(plan_svg_export(&Document::default(), ExportScope::WholeBoard), Err(ExportError::NothingToExport));
    let mut bad = d.clone();
    bad.paths[0].anchors[0].p[0] = f32::NAN;
    assert!(matches!(plan_svg_export(&bad, ExportScope::WholeBoard), Err(ExportError::InvalidDocument(_))));
    let mut bad = d;
    bad.nodes[0].parent = Some(bad.nodes[0].id);
    assert!(matches!(plan_svg_export(&bad, ExportScope::WholeBoard), Err(ExportError::InvalidDocument(_))));
}
#[test]
fn stroke_padding_and_degenerate_extent_are_real_pages() {
    let mut p = rect(2, -0.00001, 0.0, 20.0, 0.0);
    p.fill = Paint::None;
    p.stroke = Paint::Solid([0.0, 0.0, 0.0, 1.0]);
    p.stroke_width = 4.0;
    let d = doc(vec![p]);
    let b = plan_svg_export(&d, ExportScope::WholeBoard).unwrap().pages[0].rect;
    assert!((b[0] + 2.0).abs() < 0.001);
    assert_eq!(b[3], 4.0);
}
#[test]
fn frozen_raw_fixtures_match_golden_svg_and_rasterize() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let gold = root.join("svg");
    let bless = std::env::var_os("VAROS_BLESS_SVG_FIXTURES").is_some();
    for version in ["v1", "v2", "v3"] {
        let mut entries: Vec<_> = std::fs::read_dir(root.join(version))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "vrs"))
            .collect();
        entries.sort();
        for path in entries {
            let bytes = std::fs::read(&path).unwrap();
            if !bytes.starts_with(b"{") {
                continue;
            }
            let d = varos_core::file::doc_from_blob(std::str::from_utf8(&bytes).unwrap()).unwrap();
            let plan = plan_svg_export(&d, default_scope(&d)).unwrap();
            let files = export_svg_files(&d, &plan, &AtomicBool::new(false)).unwrap();
            assert_eq!(files, export_svg_files(&d, &plan, &AtomicBool::new(false)).unwrap());
            for (i, file) in files.iter().enumerate() {
                let name = format!("{}-{}.svg", path.file_stem().unwrap().to_str().unwrap(), i + 1);
                if bless {
                    std::fs::create_dir_all(&gold).unwrap();
                    std::fs::write(gold.join(&name), &file.bytes).unwrap();
                }
                assert_eq!(file.bytes, std::fs::read(gold.join(&name)).unwrap(), "{name}");
                xml_check(std::str::from_utf8(&file.bytes).unwrap());
                let tree = resvg::usvg::Tree::from_data(&file.bytes, &resvg::usvg::Options::default()).unwrap();
                let mut pix = resvg::tiny_skia::Pixmap::new(128, 128).unwrap();
                let scale = 128.0 / tree.size().width().max(tree.size().height());
                resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pix.as_mut());
            }
        }
    }
}

fn assert_bounds(a: [f32; 4], b: [f32; 4]) {
    for (a, b) in a.into_iter().zip(b) {
        assert!((a - b).abs() < 0.001, "{a} vs {b}");
    }
}

#[test]
fn ordinary_hidden_paths_are_absent_and_visible_lock_does_not_hide_art() {
    let mut hidden = rect(2, 0.0, 0.0, 20.0, 20.0);
    hidden.hidden = true;
    hidden.name = Some("SECRET".into());
    let mut locked = rect(3, 30.0, 0.0, 20.0, 20.0);
    locked.locked = true;
    let s = svg(&doc(vec![hidden, locked]), ExportScope::WholeBoard);
    assert!(!s.contains("SECRET"));
    assert!(!s.contains("id=\"path-2"));
    assert!(s.contains("id=\"path-3"));
}
#[test]
fn empty_masks_hide_only_their_members_and_do_not_erase_other_bounds() {
    let mut mask = rect(2, 0.0, 0.0, 10.0, 10.0);
    mask.anchors.clear();
    let mut d = doc(vec![mask, rect(3, 0.0, 0.0, 20.0, 20.0), rect(4, 40.0, 40.0, 20.0, 20.0)]);
    d.clip_group(&[2, 3], 2).unwrap();
    let s = svg(&d, ExportScope::WholeBoard);
    xml_check(&s);
    assert!(!s.contains("id=\"path-3"));
    assert!(s.contains("id=\"path-4"));
    assert_bounds(plan_svg_export(&d, ExportScope::WholeBoard).unwrap().pages[0].rect, [40.0, 40.0, 20.0, 20.0]);
}
#[test]
fn nested_nearest_clip_runs_have_unique_ids_and_keep_paint_order() {
    let mut d = doc(vec![
        rect(2, 0.0, 0.0, 90.0, 90.0),
        rect(3, 10.0, 10.0, 60.0, 60.0),
        rect(4, 0.0, 0.0, 100.0, 100.0),
        rect(5, 5.0, 5.0, 20.0, 20.0),
        rect(6, 70.0, 70.0, 20.0, 20.0),
    ]);
    d.clip_group(&[3, 4], 3).unwrap();
    d.clip_group(&[2, 3, 4, 5, 6], 2).unwrap();
    // The nearest outer clip appears before and after the inner clip, in valid flattened paint order.
    let a = d.paths.iter().position(|p| p.id == 5).unwrap();
    let b = d.paths.iter().position(|p| p.id == 4).unwrap();
    d.paths.swap(a, b);
    let s = svg(&d, ExportScope::WholeBoard);
    xml_check(&s);
    for (a, b) in d.paint_list().map(|(_, p)| p.id).collect::<Vec<_>>().windows(2).map(|pair| (pair[0], pair[1])) {
        assert!(s.find(&format!("id=\"path-{a}\"")).unwrap() < s.find(&format!("id=\"path-{b}\"")).unwrap());
    }
}
#[test]
fn overflowing_paint_extents_are_refused_instead_of_emitting_infinity() {
    let mut p = rect(2, 0.0, 0.0, 20.0, 20.0);
    p.anchors[0].p[0] = -f32::MAX;
    p.anchors[1].p[0] = f32::MAX;
    assert!(matches!(plan_svg_export(&doc(vec![p]), ExportScope::WholeBoard), Err(ExportError::InvalidDocument(_))));
}

#[test]
fn viewport_dimensions_colors_and_page_ids_match_svg_pixel_conventions() {
    let mut p = rect(2, 0.0, 0.0, 20.0, 20.0);
    p.fill = Paint::Solid([0.9, 0.5, 0.0, 0.25]);
    p.stroke = Paint::Solid([0.0, 0.25, 1.0, 1.0]);
    let mut d = doc(vec![p]);
    d.artboards = vec![Artboard {
        w: 100.0,
        h: 80.0,
        name: "لوحة١".into(),
        page_color: Some([1.0, 1.0, 1.0, 0.5]),
        ..Artboard::default()
    }];
    let s = svg(&d, ExportScope::ActiveArtboard);
    xml_check(&s);
    let parsed = roxmltree::Document::parse(&s).unwrap();
    let root = parsed.root_element();
    assert_eq!(root.attribute("width"), Some("100.000"));
    assert_eq!(root.attribute("height"), Some("80.000"));
    assert_eq!(root.attribute("id"), Some("artboard-1-لوحة١"));
    assert!(parsed.descendants().any(|n| n.attribute("id") == Some("background")));
    let tree = resvg::usvg::Tree::from_data(s.as_bytes(), &resvg::usvg::Options::default()).unwrap();
    assert_eq!(tree.size().width(), 100.0);
    assert_eq!(tree.size().height(), 80.0);
    assert!(s.contains("fill=\"#e68000\" fill-opacity=\"0.250\""));
    assert!(s.contains("stroke=\"#0040ff\" stroke-opacity=\"1.000\""));
    let s = svg(&d, ExportScope::WholeBoard);
    let parsed = roxmltree::Document::parse(&s).unwrap();
    assert_eq!(parsed.root_element().attribute("id"), Some("board"));
}
