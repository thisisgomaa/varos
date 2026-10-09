//! Headless cross-backend paint goldens; never instantiate a device or event loop.
use super::*;
use std::{path::Path, sync::atomic::AtomicBool};
use varos_core::{
    format::{decode_model, Limits},
    svg::{export_svg_files, plan_svg_export, ExportScope},
};
#[test]
fn paint_goldens_json_svg_pdf_and_cpu() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures/next_gradients");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gradients");
    for name in ["linear", "radial", "swatch"] {
        let source = std::fs::read(core.join(format!("{name}.vrs"))).unwrap();
        let loaded = decode_model(&source, None, &Limits::DEFAULT).unwrap();
        let json = varos_core::format::encode_model(&loaded.doc, &Limits::DEFAULT).unwrap();
        let plan = plan_svg_export(&loaded.doc, ExportScope::WholeBoard).unwrap();
        let svg = export_svg_files(&loaded.doc, &plan, &AtomicBool::new(false)).unwrap().remove(0);
        let pdf = varos_pdf::write_pdf(&loaded.doc).unwrap();
        assert_eq!(varos_pdf::load_vrs_bytes(&pdf, &Limits::DEFAULT).unwrap().doc, loaded.doc);
        let [x, y, w, h] = svg.page.rect;
        let size = 128;
        let mut e = Editor::new();
        e.replace_doc(loaded.doc);
        let scene = build_scene(&e, size as f32 / w);
        assert!(scene.errors.is_empty());
        let mut cpu = Pixmap::new(size, size).unwrap();
        draw_groups(
            &scene.content,
            &mut cpu,
            Transform::from_row(size as f32 / w, 0., 0., size as f32 / h, -x * size as f32 / w, -y * size as f32 / h),
        );
        let png = cpu.encode_png().unwrap();
        // Frozen v7 sources and the v6-era PDF goldens: only version stamps follow the current writer
        // (integration w2; single-digit stamps keep every xref offset).
        let fv = varos_core::format::FORMAT_VERSION;
        assert_eq!(
            String::from_utf8(std::fs::read(core.join(format!("{name}.vrs"))).unwrap()).unwrap().replacen(
                "\"varos\":7",
                &format!("\"varos\":{fv}"),
                1
            ),
            json
        );
        let pdf_golden = std::fs::read(root.join(format!("{name}.pdf"))).unwrap();
        pdf_objects::assert_same(&pdf, &pdf_golden, 6);
        for (ext, data) in [("svg", svg.bytes.as_slice()), ("png", png.as_slice())] {
            assert_eq!(std::fs::read(root.join(format!("{name}.{ext}"))).unwrap(), data, "{name}.{ext}");
        }
        let tree = resvg::usvg::Tree::from_data(&svg.bytes, &resvg::usvg::Options::default()).unwrap();
        let mut vector = Pixmap::new(size, size).unwrap();
        resvg::render(
            &tree,
            Transform::from_scale(size as f32 / tree.size().width(), size as f32 / tree.size().height()),
            &mut vector.as_mut(),
        );
        let mean = cpu.data().iter().zip(vector.data()).map(|(a, b)| a.abs_diff(*b) as f64).sum::<f64>()
            / (size * size * 4) as f64;
        assert!(mean < 0.35, "{name}: SVG/CPU mean channel error {mean}");
    }
}

#[path = "../../varos-pdf/tests/support/pdf_golden.rs"]
mod pdf_objects;
