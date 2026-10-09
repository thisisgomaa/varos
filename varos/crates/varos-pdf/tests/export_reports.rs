use std::sync::atomic::AtomicBool;
use varos_core::format::Limits;
#[test]
fn every_accepted_fixture_has_empty_reports_and_identical_bytes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures");
    let mut paths = Vec::new();
    for dir in [root.clone(), root.join("v1"), root.join("v2"), root.join("v3"), root.join("v4")] {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|e| e == "vrs") {
                paths.push(p);
            }
        }
    }
    assert!(paths.len() >= 25);
    for p in paths {
        let bytes = std::fs::read(&p).unwrap();
        let d = varos_pdf::load_vrs_bytes(&bytes, &Limits::DEFAULT).unwrap().doc;
        let cancel = AtomicBool::new(false);
        let plan = varos_pdf::plan_pdf_export(&d, varos_pdf::default_scope(&d)).unwrap();
        let (pdf, report) = varos_pdf::export_pdf_bytes_with_report(&d, &plan, &cancel).unwrap();
        assert!(report.notes.is_empty(), "{p:?}");
        assert_eq!(pdf, varos_pdf::export_pdf_bytes(&d, &plan, &cancel).unwrap());
        let plan = varos_core::svg::plan_svg_export(&d, varos_core::svg::default_scope(&d)).unwrap();
        let (svg, report) = varos_core::svg::export_svg_files_with_report(&d, &plan, &cancel).unwrap();
        assert!(report.notes.is_empty(), "{p:?}");
        let legacy = varos_core::svg::export_svg_files(&d, &plan, &cancel).unwrap();
        assert_eq!(
            svg.iter().map(|s| &s.bytes).collect::<Vec<_>>(),
            legacy.iter().map(|s| &s.bytes).collect::<Vec<_>>()
        );
    }
}
