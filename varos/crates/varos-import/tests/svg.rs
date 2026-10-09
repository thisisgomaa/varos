use varos_core::{model::NodeKind, EditCommand, Editor};
use varos_import::import_svg;
fn svg(body: &str) -> Vec<u8> {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100">{body}</svg>"#).into_bytes()
}
#[test]
fn shapes_paints_and_board() {
    let (doc, report) = import_svg(&svg(r##"<rect x="10" y="20" width="40" height="30" fill="#f00" stroke="#00f" stroke-width="3" opacity=".5"/><ellipse cx="90" cy="50" rx="10" ry="20"/><polygon points="120,20 150,50 120,80"/>"##)).unwrap();
    assert_eq!(report.paths, 3);
    assert_eq!((doc.artboards[0].w, doc.artboards[0].h), (200., 100.));
    let p = &doc.paths[0];
    assert_eq!(p.stroke_width, 3.);
    // usvg represents element opacity with an isolated wrapper group.
    assert_eq!(p.opacity, 0.5);
    assert_eq!(p.fill.solid(), Some([1., 0., 0., 1.]));
    assert_eq!(doc.paths[1].anchors.len(), 4);
    varos_core::format::check_structure(&doc, &varos_core::format::Limits::DEFAULT).unwrap();
}
#[test]
fn nested_transforms_are_baked_and_groups_survive() {
    let (doc, _) = import_svg(&svg(
        r#"<g id="outer" transform="translate(10 20)"><g transform="scale(2)"><rect width="10" height="5"/></g></g>"#,
    ))
    .unwrap();
    assert_eq!(doc.paths[0].anchors[0].p, [10., 20.]);
    assert_eq!(doc.paths[0].anchors[1].p, [30., 20.]);
    assert_eq!(doc.nodes.iter().filter(|n| n.kind == NodeKind::Group).count(), 2);
}
#[test]
fn evenodd_holes_and_curves() {
    let (doc, report) = import_svg(&svg(
        r#"<path fill-rule="evenodd" d="M0 0 H100 V100 H0 Z M20 20 H80 V80 H20 Z"/><path d="M110 10 Q150 80 190 10"/>"#,
    ))
    .unwrap();
    assert_eq!(doc.paths[0].holes.len(), 1);
    assert!(!doc.point_in_path(0, [50., 50.]));
    assert!(doc.point_in_path(0, [10., 10.]));
    assert!(doc.paths[1].anchors[0].hout.is_some());
    assert!(report.loss_notes.is_empty(), "{:?}", report);
}
#[test]
fn unsupported_features_are_reported_even_when_normalization_drops_them() {
    let (_, report) = import_svg(&svg(r##"<defs><linearGradient id="g"><stop stop-color="red"/></linearGradient><filter id="f"/></defs><rect width="10" height="10" fill="url(#g)" filter="url(#f)"/><text>hello</text><image href="file:///etc/passwd" width="20" height="20"/>"##)).unwrap();
    for feature in ["Gradient", "Filters", "Text", "Images"] {
        assert!(report.loss_notes.iter().any(|n| n.contains(feature)), "{feature}: {report:?}");
    }
}
#[test]
fn viewbox_origin_and_explicit_viewport_use_viewbox_units() {
    let (doc, _) = import_svg(br#"<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="500" viewBox="10 20 200 100"><rect x="10" y="20" width="50" height="25"/></svg>"#).unwrap();
    assert_eq!((doc.artboards[0].w, doc.artboards[0].h), (200., 100.));
    assert_eq!(doc.paths[0].anchors[0].p, [0., 0.]);
}
#[test]
fn export_import_roundtrip_preserves_editable_hole_geometry_and_paints() {
    let (doc, _) = import_svg(&svg(
        r##"<path fill="#12ab45" fill-rule="evenodd" d="M0 0 H100 V100 H0 Z M20 20 H80 V80 H20 Z"/>"##,
    ))
    .unwrap();
    let plan = varos_core::svg::plan_svg_export(&doc, varos_core::svg::ExportScope::ActiveArtboard).unwrap();
    let files = varos_core::svg::export_svg_files(&doc, &plan, &std::sync::atomic::AtomicBool::new(false)).unwrap();
    let (back, report) = import_svg(&files[0].bytes).unwrap();
    assert!(report.loss_notes.is_empty(), "{:?}", report);
    assert_eq!(back.paths.len(), 1);
    assert_eq!(back.paths[0].holes.len(), 1);
    assert_eq!(back.paths[0].fill, doc.paths[0].fill);
    assert_eq!(
        back.paths[0].anchors.iter().map(|a| a.p).collect::<Vec<_>>(),
        doc.paths[0].anchors.iter().map(|a| a.p).collect::<Vec<_>>()
    );
}
#[test]
fn placement_remaps_ids_preserves_groups_and_undoes_atomically() {
    let (doc, _) =
        import_svg(&svg(r#"<g><rect width="20" height="20"/><ellipse cx="50" cy="50" rx="10" ry="10"/></g>"#)).unwrap();
    let mut ed = Editor::new();
    ed.execute(EditCommand::PlaceArtwork(Box::new(doc.clone()))).expect("test edit succeeds");
    ed.execute(EditCommand::PlaceArtwork(Box::new(doc))).expect("test edit succeeds");
    assert_eq!(ed.doc.paths.len(), 4);
    varos_core::format::check_structure(&ed.doc, &varos_core::format::Limits::DEFAULT).unwrap();
    ed.execute(EditCommand::Undo).expect("test edit succeeds");
    assert_eq!(ed.doc.paths.len(), 2);
    ed.execute(EditCommand::Undo).expect("test edit succeeds");
    assert!(ed.doc.paths.is_empty());
    ed.execute(EditCommand::Redo).expect("test edit succeeds");
    assert_eq!(ed.doc.paths.len(), 2);
}
#[test]
fn svgz_and_malformed_inputs() {
    use std::io::Write;
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip.write_all(&svg(r#"<rect width="10" height="10"/>"#)).unwrap();
    assert_eq!(import_svg(&gzip.finish().unwrap()).unwrap().0.paths.len(), 1);
    for bytes in [b"<svg".as_slice(), b"<html/>", b"\xff", br#"<svg viewBox="0 0 -10 10"/>"#] {
        assert!(import_svg(bytes).is_err());
    }
}

#[test]
fn disjoint_contours_open_subpaths_and_nested_islands_remain_selectable() {
    let (doc, _) = import_svg(&svg(r#"<path fill-rule="evenodd" d="M0 0 H100 V100 H0 Z M10 10 H90 V90 H10 Z M20 20 H80 V80 H20 Z M120 0 H160 V40 H120 Z"/><path fill="none" stroke="red" d="M0 0 L5 5 M10 10 L15 15"/>"#)).unwrap();
    assert_eq!(doc.paths.len(), 5);
    assert_eq!(doc.paths[0].holes.len(), 1);
    assert!(doc.paths.iter().enumerate().any(|(i, _)| doc.point_in_path(i, [140., 20.])));
    assert!(doc.paths.iter().enumerate().any(|(i, _)| doc.point_in_path(i, [50., 50.])));
    assert!(!doc.paths[3].closed && !doc.paths[4].closed);
}
#[test]
fn prefixed_svg_and_oversize_or_deep_inputs_are_checked() {
    assert!(import_svg(
        br#"<s:svg xmlns:s="http://www.w3.org/2000/svg" viewBox="0 0 20 10"><s:rect width="10" height="10"/></s:svg>"#
    )
    .is_ok());
    assert!(import_svg(&vec![b' '; varos_import::MAX_BYTES + 1]).is_err());
    let deep = svg(&format!("{}<rect width=\"1\" height=\"1\"/>{}", "<g>".repeat(130), "</g>".repeat(130)));
    assert!(import_svg(&deep).is_err());
}

#[test]
fn intersecting_or_touching_compound_contours_are_explicitly_refused() {
    for data in [
        // Reviewer repro: both starts are inside the other contour.
        "M100 100 H0 V0 H100 Z M50 50 H150 V150 H50 Z",
        // Same intersection, starts outside.
        "M0 0 H100 V100 H0 Z M150 150 H50 V50 H150 Z",
        // Inner vertices inside a concave outer, but edges cross its notch.
        "M0 0 H100 V100 H60 V40 H40 V100 H0 Z M20 20 H80 V80 H20 Z",
        // Shared edge, shared vertex, coincident boundaries.
        "M0 0 H100 V100 H0 Z M0 20 H50 V50 H0 Z",
        "M0 0 H100 V100 H0 Z M100 100 H150 V150 H100 Z",
        "M0 0 H100 V100 H0 Z M0 0 H100 V100 H0 Z",
    ] {
        let err = import_svg(&svg(&format!(r#"<path fill-rule="evenodd" d="{data}"/>"#))).unwrap_err();
        assert!(err.contains("intersecting or touching compound contours"), "{data}: {err}");
    }
}

#[test]
fn disjoint_concave_contours_with_overlapping_bounds_are_preserved() {
    let (doc, report) =
        import_svg(&svg(r#"<path fill-rule="evenodd" d="M0 0 H100 V20 H20 V100 H0 Z M40 40 H80 V80 H40 Z"/>"#))
            .unwrap();
    assert_eq!(doc.paths.len(), 2);
    assert!(doc.paths.iter().all(|p| p.holes.is_empty()));
    assert!(report.loss_notes.is_empty(), "{report:?}");
    assert!(doc.point_in_path(0, [10., 50.]));
    assert!(doc.point_in_path(1, [50., 50.]));
}

#[test]
fn stroke_loss_matches_destination_round_caps_and_joins() {
    for attrs in [
        "", // SVG defaults to butt/miter.
        r#"stroke-linecap="butt" stroke-linejoin="miter""#,
        r#"stroke-linecap="square" stroke-linejoin="bevel""#,
        r#"stroke-linecap="round" stroke-linejoin="round" stroke-dasharray="2 3""#,
    ] {
        let (_, report) =
            import_svg(&svg(&format!(r#"<path fill="none" stroke="red" {attrs} d="M0 0 L20 0 L20 20"/>"#))).unwrap();
        assert!(report.loss_notes.iter().any(|n| n.contains("round caps/joins")), "{attrs}: {report:?}");
    }
    let (doc, report) = import_svg(&svg(
        r#"<path fill="none" stroke="red" stroke-linecap="round" stroke-linejoin="round" d="M0 0 L20 0 L20 20"/>"#,
    ))
    .unwrap();
    assert_eq!(doc.paths.len(), 1);
    assert!(report.loss_notes.is_empty(), "{report:?}");
    let plan = varos_core::svg::plan_svg_export(&doc, varos_core::svg::ExportScope::ActiveArtboard).unwrap();
    let files = varos_core::svg::export_svg_files(&doc, &plan, &std::sync::atomic::AtomicBool::new(false)).unwrap();
    assert!(import_svg(&files[0].bytes).unwrap().1.loss_notes.is_empty());
}

#[test]
fn placement_refuses_an_active_transaction_without_mutation() {
    let (doc, _) = import_svg(&svg(r#"<rect width="20" height="20"/>"#)).unwrap();
    let mut ed = Editor::new();
    ed.begin();
    let before = ed.doc.clone();
    let rev = ed.rev;
    assert!(varos_core::placement::check(&ed, &doc).unwrap_err().contains("active transaction"));
    ed.execute(EditCommand::PlaceArtwork(Box::new(doc))).expect("test edit succeeds");
    assert_eq!(ed.doc, before);
    assert_eq!(ed.rev, rev);
    assert!(ed.transaction_open());
}
