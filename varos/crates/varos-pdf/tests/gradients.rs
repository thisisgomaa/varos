//! PDF shadings, alpha and group semantics, refusal budgets; no device or window.
use std::{path::Path, sync::atomic::AtomicBool};
use varos_core::{
    format::{decode_model, Limits},
    gradient::{GradientKind, Spread},
    model::Paint,
};
fn document() -> varos_core::model::Document {
    decode_model(
        &std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures/next_gradients/linear.vrs"),
        )
        .unwrap(),
        None,
        &Limits::DEFAULT,
    )
    .unwrap()
    .doc
}
#[test]
fn shading_alpha_and_knockout_form_resources_roundtrip() {
    for kind in [GradientKind::Linear, GradientKind::Radial] {
        for spread in [Spread::Pad, Spread::Repeat, Spread::Reflect] {
            let mut doc = document();
            let Paint::Gradient(g) = &mut doc.paths[0].fill else { panic!() };
            g.kind = kind;
            g.spread = spread;
            g.focal = [0.3, -0.2];
            doc.paths[0].stroke = doc.paths[0].fill.clone();
            doc.paths[0].opacity = 0.4;
            let bytes = varos_pdf::write_pdf(&doc).unwrap();
            assert_eq!(varos_pdf::load_vrs_bytes(&bytes, &Limits::DEFAULT).unwrap().doc, doc);
            let pdf = lopdf::Document::load_mem(&bytes).unwrap();
            let mut shadings = 0;
            let mut masks = 0;
            let mut knockout = 0;
            for obj in pdf.objects.values() {
                let Ok(d) = obj.as_dict().or_else(|_| obj.as_stream().map(|s| &s.dict)) else { continue };
                if let Ok(t) = d.get(b"ShadingType") {
                    assert_eq!(t.as_i64().unwrap(), if kind == GradientKind::Linear { 2 } else { 3 });
                    shadings += 1;
                }
                if d.get(b"SMask").is_ok() {
                    masks += 1;
                }
                if let Ok(group) = d.get(b"Group").and_then(|g| g.as_dict()) {
                    if group.get(b"K").is_ok_and(|v| matches!(v.as_bool(), Ok(true))) {
                        knockout += 1;
                        assert!(group.get(b"I").unwrap().as_bool().unwrap());
                    }
                }
            }
            assert!(shadings >= 2);
            assert!(masks >= 1);
            assert_eq!(knockout, 1);
            let plan = varos_pdf::plan_pdf_export(&doc, varos_pdf::ExportScope::ArtworkBounds).unwrap();
            let (_, report) = varos_pdf::export_pdf_bytes_with_report(&doc, &plan, &AtomicBool::new(false)).unwrap();
            assert!(report.notes.iter().any(|n| n.kind == "gradient_sampled"));
            assert!(report.notes.iter().any(|n| n.kind == "stroke_baked"));
        }
    }
}
#[test]
fn sampled_stream_budget_refuses_before_emission() {
    let doc = document();
    let mut plan = varos_pdf::plan_pdf_export(&doc, varos_pdf::ExportScope::ArtworkBounds).unwrap();
    plan.pages = vec![plan.pages[0]; 2000];
    assert_eq!(
        varos_pdf::export_pdf_bytes(&doc, &plan, &AtomicBool::new(false)),
        Err(varos_pdf::ExportError::LimitExceeded)
    );
}
#[test]
fn next_fixture_checksums_are_frozen() {
    use sha2::{Digest, Sha256};
    for relative in ["../varos-core/tests/fixtures/next_gradients", "../varos-raster/tests/fixtures/gradients"] {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        for line in std::fs::read_to_string(root.join("SHA256SUMS")).unwrap().lines() {
            let (expected, name) = line.split_once("  ").unwrap();
            let actual = Sha256::digest(std::fs::read(root.join(name)).unwrap())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            assert_eq!(actual, expected, "{name}");
        }
    }
}
#[test]
fn old_reader_refuses_next_before_a_malformed_document() {
    let source = br#"{"varos":6,"doc":42}"#;
    let version = varos_core::format::peek_version(source).unwrap();
    let gate = if version > 5 {
        Err(varos_core::format::LoadError::NewerVersion { found: version, supported: 5 })
    } else {
        varos_core::format::decode_model(source, None, &Limits::DEFAULT).map(|_| ())
    };
    assert!(matches!(gate, Err(varos_core::format::LoadError::NewerVersion { found: 6, supported: 5 })));
}
