use std::sync::atomic::AtomicBool;
use varos_core::{
    effects::{Effect, WarpStyle},
    format::Limits,
    model::{Anchor, Document, Path},
    stroke::StrokeJoin,
    width_profile::WidthProfile,
};
fn doc(effect: Effect) -> Document {
    let mut d = Document::default();
    let mut p = Path::new(
        10,
        [[20., 20.], [120., 20.], [120., 100.], [20., 100.]]
            .into_iter()
            .enumerate()
            .map(|(i, p)| Anchor { id: 11 + i as u32, p, hin: None, hout: None, smooth: false })
            .collect(),
        true,
        Some([0., 0.5, 1., 1.]),
        Some([1., 0., 0., 1.]),
        5.,
    );
    p.effects.push(effect);
    p.stroke_style.width_profile = Some(WidthProfile::lens());
    d.paths.push(p);
    d.ids = 30;
    d.sync_tree();
    d
}
fn baked(d: &Document) -> Document {
    varos_core::effects_document::document(d).unwrap().into_owned()
}
#[test]
fn pdf_and_svg_live_effects_equal_baked_geometry_per_effect() {
    for effect in [
        Effect::Offset { delta: 5., join: StrokeJoin::Miter, miter: 10. },
        Effect::ZigZag { size: 3., ridges: 3, smooth: true },
        Effect::Transform { copies: 1, movement: [200., 0.], scale: [1., 1.], rotate: 0., reflect: [false, false] },
        Effect::Warp { style: WarpStyle::Arch, bend: 35., h: 10., v: 0. },
    ] {
        let d = doc(effect);
        let q = baked(&d);
        let cancel = AtomicBool::new(false);
        let plan = varos_pdf::plan_pdf_export(&d, varos_pdf::ExportScope::ArtworkBounds).unwrap();
        let qplan = varos_pdf::plan_pdf_export(&q, varos_pdf::ExportScope::ArtworkBounds).unwrap();
        assert_eq!(plan, qplan);
        assert_eq!(
            varos_pdf::export_pdf_bytes(&d, &plan, &cancel).unwrap(),
            varos_pdf::export_pdf_bytes(&q, &qplan, &cancel).unwrap()
        );
        let plan = varos_core::svg::plan_svg_export(&d, varos_core::svg::ExportScope::WholeBoard).unwrap();
        let qplan = varos_core::svg::plan_svg_export(&q, varos_core::svg::ExportScope::WholeBoard).unwrap();
        assert_eq!(plan, qplan);
        assert_eq!(
            varos_core::svg::export_svg_files(&d, &plan, &cancel).unwrap(),
            varos_core::svg::export_svg_files(&q, &qplan, &cancel).unwrap()
        );
        let bytes = varos_pdf::write_pdf_checked(&d, &Limits::DEFAULT).unwrap();
        let loaded = varos_pdf::load_vrs_bytes(&bytes, &Limits::DEFAULT).unwrap();
        assert_eq!(loaded.doc.paths, d.paths);
    }
}
#[test]
fn frozen_v11_embedded_pdf_and_old_reader_refusal() {
    let d = varos_core::format::decode_model(
        include_bytes!("../../varos-core/tests/fixtures/w3-effects/v11-effects.json"),
        None,
        &Limits::DEFAULT,
    )
    .unwrap()
    .doc;
    let bytes = varos_pdf::write_pdf_checked(&d, &Limits::DEFAULT).unwrap();
    let loaded = varos_pdf::load_vrs_bytes(&bytes, &Limits::DEFAULT).unwrap();
    assert_eq!(loaded.source_version, 11);
    let expected = Err(varos_core::format::LoadError::NewerVersion { found: 11, supported: 9 });
    let json = include_bytes!("../../varos-core/tests/fixtures/w3-effects/v11-effects.json");
    assert_eq!(frozen_v9_header_gate(json), expected);
    assert_eq!(frozen_v9_header_gate(br#"{"varos":11,"doc":"invalid"}"#), expected);
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let catalog = pdf.catalog().unwrap();
    let version = catalog.get(b"VAROS_SchemaVersion").unwrap().as_i64().unwrap() as u32;
    assert_eq!(frozen_v9_version_gate(version), expected);
    let (_, model) = pdf.dereference(catalog.get(b"VAROS_Model").unwrap()).unwrap();
    assert_eq!(frozen_v9_header_gate(&model.as_stream().unwrap().content), expected);
    assert_eq!(frozen_v9_header_gate(br#"{"varos":9,"doc":"invalid"}"#), Ok(9));
    assert_eq!(frozen_v9_version_gate(9), Ok(9));
}
// Frozen v9 predecode header/catalog gates adapted from that reader, not an old binary.
fn frozen_v9_version_gate(found: u32) -> Result<u32, varos_core::format::LoadError> {
    if found > 9 {
        Err(varos_core::format::LoadError::NewerVersion { found, supported: 9 })
    } else {
        Ok(found)
    }
}
fn frozen_v9_header_gate(bytes: &[u8]) -> Result<u32, varos_core::format::LoadError> {
    #[derive(serde::Deserialize)]
    struct Head {
        varos: u32,
    }
    let head: Head = serde_json::from_slice(bytes).unwrap();
    frozen_v9_version_gate(head.varos)
}
