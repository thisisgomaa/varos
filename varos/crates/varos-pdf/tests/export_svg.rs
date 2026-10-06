//! Binary and JSON frozen corpus coverage: every accepted fixture, including all masks.
use std::sync::atomic::AtomicBool;
use varos_core::{format::Limits, svg::*};

#[test]
fn every_frozen_container_exports_without_panics_and_binary_twins_match_goldens() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures");
    let mut count = 0;
    for version in ["v1", "v2", "v3"] {
        for entry in std::fs::read_dir(root.join(version)).unwrap() {
            let path = entry.unwrap().path();
            if !path.extension().is_some_and(|e| e == "vrs") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            let d = varos_pdf::load_vrs_bytes(&bytes, &Limits::DEFAULT).unwrap().doc;
            for scope in [default_scope(&d), ExportScope::WholeBoard] {
                let plan = plan_svg_export(&d, scope).unwrap();
                for (i, file) in export_svg_files(&d, &plan, &AtomicBool::new(false)).unwrap().iter().enumerate() {
                    let name = path.file_stem().unwrap().to_str().unwrap();
                    if name.ends_with("_pdf") && scope == default_scope(&d) {
                        let golden = root.join("svg").join(format!("{}-{}.svg", name.trim_end_matches("_pdf"), i + 1));
                        assert_eq!(file.bytes, std::fs::read(golden).unwrap(), "{name}");
                    }
                    let tree = resvg::usvg::Tree::from_data(&file.bytes, &resvg::usvg::Options::default()).unwrap();
                    let mut pix = resvg::tiny_skia::Pixmap::new(128, 128).unwrap();
                    let scale = 128.0 / tree.size().width().max(tree.size().height());
                    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pix.as_mut());
                }
            }
            count += 1;
        }
    }
    assert_eq!(count, 25, "new frozen fixtures must be included in this gate");
}
