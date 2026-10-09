fn main() -> Result<(), Box<dyn std::error::Error>> {
    let loaded = varos_pdf::load_vrs_bytes(
        include_bytes!("../../varos-core/tests/fixtures/v6-images/embedded-crop.vrs"),
        &varos_core::format::Limits::DEFAULT,
    )?;
    let bytes = varos_pdf::images::write_vrs(&loaded.doc, &loaded.blobs, &varos_core::format::Limits::DEFAULT)?;
    std::fs::write("crates/varos-pdf/tests/fixtures/image-fixed-writer.vrs", bytes)?;
    Ok(())
}
