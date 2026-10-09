//! Replay of the v5 header gate at main b3d39ee, before live-corner typed decoding.
use varos_core::format::{Limits, FORMAT_VERSION};
fn v5_gate(body: &str) -> Result<u32, String> {
    #[derive(serde::Deserialize)]
    struct Head {
        varos: serde_json::Value,
    }
    let head: Head = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let version =
        head.varos.as_u64().filter(|v| *v > 0).and_then(|v| u32::try_from(v).ok()).ok_or("invalid version")?;
    if version > 5 {
        return Err(format!("needs newer Varos: {version}, supported 5"));
    }
    Ok(version)
}
#[test]
fn v5_reader_refuses_next_json_and_embedded_pdf_before_decode() {
    let fixture = include_bytes!("../../varos-core/tests/fixtures/lane_c/next_live_round.json");
    let loaded = varos_core::format::decode_model(fixture, None, &Limits::DEFAULT).unwrap();
    let current = varos_core::format::encode_model(&loaded.doc, &Limits::DEFAULT).unwrap();
    let pdf = lopdf::Document::load_mem(&varos_pdf::write_pdf(&loaded.doc).unwrap()).unwrap();
    let (_, object) = pdf.dereference(pdf.catalog().unwrap().get(b"VAROS_Model").unwrap()).unwrap();
    let embedded = std::str::from_utf8(&object.as_stream().unwrap().content).unwrap();
    let undecodable = format!("{{\"varos\":{FORMAT_VERSION},\"doc\":42}}");
    for body in [current.as_str(), embedded, undecodable.as_str()] {
        assert_eq!(v5_gate(body).unwrap_err(), format!("needs newer Varos: {FORMAT_VERSION}, supported 5"));
    }
    assert_eq!(v5_gate(include_str!("../../varos-core/tests/fixtures/v5/corners.json")).unwrap(), 5);
}
