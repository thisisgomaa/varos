//! Explicit fixture authoring tool; tests never re-bless this corpus.
use std::sync::atomic::AtomicBool;
use varos_core::{
    format::{encode_model, Limits},
    model::{Anchor, Document, Path},
    stroke::{ArrowHead, StrokeAlign, StrokeCap, StrokeJoin, StrokeStyle},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new("crates/varos-core/tests/fixtures/v5");
    std::fs::create_dir_all(root)?;
    // Explicit fix-round refresh: only PDF fixtures whose emitted artwork changed.
    if std::env::args().any(|arg| arg == "--pdf-only") {
        for name in std::fs::read_to_string(root.join("INDEX"))?.lines() {
            let json = std::fs::read(root.join(format!("{name}.json")))?;
            let loaded = varos_core::format::decode_model(&json, None, &Limits::DEFAULT).map_err(|e| e.to_string())?;
            let path = root.join(format!("{name}.pdf"));
            let bytes = varos_pdf::write_pdf(&loaded.doc)?;
            if std::fs::read(&path)? != bytes {
                std::fs::write(path, bytes)?;
                println!("updated {name}.pdf");
            }
        }
        return Ok(());
    }

    let heads: Vec<_> = ArrowHead::ALL
        .iter()
        .map(|head| {
            let (geometry, inset) = varos_core::stroke::heads::geometry(*head);
            serde_json::json!({"head":head, "outline":geometry.to_svg(), "inset":inset})
        })
        .collect();
    std::fs::write(root.join("head_library.json"), serde_json::to_vec_pretty(&heads)?)?;
    let mut cases = vec![("plain".to_owned(), StrokeStyle::default())];
    for cap in [StrokeCap::Butt, StrokeCap::Round, StrokeCap::Square] {
        cases.push((format!("cap_{cap:?}"), StrokeStyle { cap, ..Default::default() }));
    }
    for join in [StrokeJoin::Miter, StrokeJoin::Round, StrokeJoin::Bevel] {
        cases.push((format!("join_{join:?}"), StrokeStyle { join, ..Default::default() }));
    }
    for dash in 1..=3 {
        cases.push((
            format!("dash_{dash}"),
            StrokeStyle {
                dash: [6.0, 3.0].repeat(dash),
                dash_phase: if dash == 2 { -2.0 } else { 2.0 },
                ..Default::default()
            },
        ));
    }
    cases.push((
        "corners".into(),
        StrokeStyle { dash: vec![6.0, 3.0], align_dashes_to_corners: true, ..Default::default() },
    ));
    for align in [StrokeAlign::Inside, StrokeAlign::Outside] {
        cases.push((format!("align_{align:?}"), StrokeStyle { align, ..Default::default() }));
    }
    cases
        .push(("holes".into(), StrokeStyle { align: StrokeAlign::Inside, dash: vec![6.0, 3.0], ..Default::default() }));
    for head in ArrowHead::ALL {
        let mut s = StrokeStyle::default();
        s.arrows.start = Some(head);
        s.arrows.end = Some(head);
        s.arrows.scale_start = 0.5;
        s.arrows.scale_end = 1.5;
        cases.push((format!("head_{head:?}"), s));
    }
    let mut index = String::new();
    for (name, style) in cases {
        let mut d = Document { ids: 100, ..Default::default() };
        let mut p = Path::new(
            10,
            [(11, [20.0, 20.0]), (12, [120.0, 20.0]), (13, [120.0, 100.0])]
                .into_iter()
                .map(|(id, p)| Anchor { id, p, hin: None, hout: None, smooth: false })
                .collect(),
            style.align != StrokeAlign::Center,
            Some([0.5, 0.7, 0.9, 1.0]),
            Some([0.0, 0.0, 0.0, 0.7]),
            4.0,
        );
        if name == "holes" {
            p.anchors = [(11, [20.0, 20.0]), (12, [120.0, 20.0]), (13, [120.0, 100.0]), (14, [20.0, 100.0])]
                .into_iter()
                .map(|(id, p)| Anchor { id, p, hin: None, hout: None, smooth: false })
                .collect();
            p.holes.push(
                [(21, [50.0, 40.0]), (22, [90.0, 40.0]), (23, [90.0, 80.0]), (24, [50.0, 80.0])]
                    .into_iter()
                    .map(|(id, p)| Anchor { id, p, hin: None, hout: None, smooth: false })
                    .collect(),
            );
        }
        p.stroke_style = style;
        p.opacity = 0.8;
        d.paths.push(p);
        d.sync_tree();
        std::fs::write(
            root.join(format!("{name}.json")),
            encode_model(&d, &Limits::DEFAULT).map_err(|e| e.to_string())?,
        )?;
        std::fs::write(root.join(format!("{name}.pdf")), varos_pdf::write_pdf(&d)?)?;
        let plan = varos_core::svg::plan_svg_export(&d, varos_core::svg::ExportScope::WholeBoard)?;
        let files = varos_core::svg::export_svg_files(&d, &plan, &AtomicBool::new(false))?;
        if let Some(file) = files.first() {
            std::fs::write(root.join(format!("{name}.svg")), &file.bytes)?;
        }
        index.push_str(&format!("{name}\n"));
    }
    std::fs::write(root.join("INDEX"), index)?;
    let refused = std::path::Path::new("crates/varos-core/tests/fixtures/refused");
    std::fs::write(refused.join("future_v6.json"), br#"{"varos":6,"doc":42}"#)?;
    // Freeze matching catalog/wrapper future stamps while retaining deliberately undecodable payload.
    let json = std::fs::read(root.join("plain.json"))?;
    let loaded = varos_core::format::decode_model(&json, None, &Limits::DEFAULT).map_err(|e| e.to_string())?;
    let bytes = varos_pdf::write_pdf(&loaded.doc)?;
    let mut pdf = lopdf::Document::load_mem(&bytes)?;
    for object in pdf.objects.values_mut() {
        if let Ok(dict) = object.as_dict_mut() {
            if dict.has(b"VAROS_SchemaVersion") {
                dict.set("VAROS_SchemaVersion", 6i64);
            }
        }
        if let Ok(stream) = object.as_stream_mut() {
            if stream.content.starts_with(b"{\"varos\":5") {
                stream.set_content(br#"{"varos":6,"doc":42}"#.to_vec());
            }
        }
    }
    pdf.save(refused.join("future_v6.pdf"))?;
    Ok(())
}
