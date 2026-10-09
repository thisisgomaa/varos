use std::sync::atomic::AtomicBool;
use varos_core::{
    model::{Anchor, Document, Path},
    svg,
};
use varos_raster::export::{self, Format, Options, Scope};

#[test]
fn shared_svg_encoding_retains_requested_precision_and_defaults() {
    let mut doc = Document::default();
    doc.paths.push(Path::new(
        1,
        vec![
            Anchor { id: 2, p: [0.123456, 0.654321], hin: None, hout: None, smooth: false },
            Anchor { id: 3, p: [10.123456, 10.654321], hin: None, hout: None, smooth: false },
        ],
        false,
        None,
        Some([1.; 4]),
        1.,
    ));
    doc.ids = 4;
    doc.sync_tree();
    let asset = export::plan(&doc, &Scope::WholeBoard).unwrap().remove(0);
    let options = Options { format: Format::Svg, ..Default::default() };
    let cancel = AtomicBool::new(false);
    for decimals in 0..=8 {
        let choices = svg::options::Options { decimals, ..Default::default() };
        let shared = export::encode_with_svg_options(&asset, &options, &cancel, &choices).unwrap();
        let plan = svg::ExportPlan { scope: svg::ExportScope::WholeBoard, pages: vec![asset.page.clone()] };
        let direct = svg::export_svg_files_with_options(&asset.doc, &plan, &cancel, &choices).unwrap().0;
        assert_eq!(shared.bytes, direct[0].bytes);
        let text = String::from_utf8(shared.bytes).unwrap();
        let mut expected = format!("{:.prec$}", 0.123456_f32 as f64, prec = decimals as usize);
        if expected.contains('.') {
            expected = expected.trim_end_matches('0').trim_end_matches('.').to_owned();
        }
        assert!(text.contains(&format!("M{expected} ")), "decimals={decimals}: {text}");
        if decimals == 8 {
            assert!(text.contains("M0.123456"));
        }
    }
    let default = export::encode_with_svg_options(&asset, &options, &cancel, &Default::default()).unwrap();
    assert!(String::from_utf8(default.bytes).unwrap().contains("M0.1235 "));
}
