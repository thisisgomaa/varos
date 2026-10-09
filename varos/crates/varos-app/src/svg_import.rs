//! Host-side SVG input; the native document reader remains exclusively `.vrs`/PDF.
use std::{io::Read, path::Path};
pub fn is_svg(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg") || e.eq_ignore_ascii_case("svgz"))
}
pub fn read(path: &Path) -> Result<(varos_core::model::Document, varos_import::ImportReport), String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take((varos_import::MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    varos_import::import_svg(&bytes)
}
