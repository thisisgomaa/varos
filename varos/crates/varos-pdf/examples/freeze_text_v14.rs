//! Lane H fixture generator. Run explicitly; never overwrites older-era fixtures.
fn main() -> Result<(), String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures/v14");
    let bytes = std::fs::read(root.join("styles.json")).map_err(|e| e.to_string())?;
    let loaded = varos_core::format::decode_model(&bytes, None, &varos_core::format::Limits::DEFAULT)
        .map_err(|e| e.to_string())?;
    let output = varos_pdf::write_pdf_checked(&loaded.doc, &varos_core::format::Limits::DEFAULT)?;
    std::fs::write(root.join("styles.vrs"), output).map_err(|e| e.to_string())
}
