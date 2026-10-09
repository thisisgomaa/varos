//! Explicit foreign import entry point. Native readers never call this module.
use crate::{ImportReport, MAX_BYTES};
use varos_core::model::Document;
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Format {
    Svg,
    Pdf,
    Ai,
    Dxf,
    Bitmap,
    Dwg,
}
impl Format {
    pub fn from_extension(extension: &str) -> Result<Self, String> {
        match extension.to_ascii_lowercase().as_str() {
            "svg" | "svgz" => Ok(Self::Svg),
            "pdf" => Ok(Self::Pdf),
            "ai" => Ok(Self::Ai),
            "dxf" => Ok(Self::Dxf),
            "png" | "tiff" | "tif" => Ok(Self::Bitmap),
            "dwg" => Ok(Self::Dwg),
            _ => Err("Unsupported foreign format; .vrs belongs to the native reader".into()),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LossPolicy {
    #[default]
    Refuse,
    AllowReported,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImportOptions {
    pub loss_policy: LossPolicy,
    /// One-based PDF page; multi-page files require an explicit selection.
    pub page: Option<u32>,
    /// Required for unitless DXF. Native world points per source drawing unit.
    pub points_per_unit: Option<f32>,
}
pub(crate) fn convert(
    bytes: &[u8],
    format: Format,
    options: ImportOptions,
) -> Result<(Document, ImportReport), String> {
    if bytes.len() > MAX_BYTES {
        return Err("Import exceeds 16 MiB limit".into());
    }
    let (doc, mut report) = match format {
        Format::Svg => crate::import_svg(bytes)?,
        Format::Pdf | Format::Ai => crate::pdf::read(bytes, options)?,
        Format::Dxf => crate::dxf::read(bytes, options)?,
        Format::Dwg => return Err("DWG is unsupported. Export an ASCII DXF from your CAD application.".into()),
        Format::Bitmap => return Err("Bitmap import unavailable: native image/blob node prerequisite is absent".into()),
    };
    if format == Format::Ai {
        report.loss("AI private editing data, layers and live effects are not reconstructed");
    }
    if options.loss_policy == LossPolicy::Refuse && !report.loss_notes.is_empty() {
        return Err(format!("Import losses require explicit acceptance: {}", report.loss_notes.join("; ")));
    }
    let limits = varos_core::format::Limits::DEFAULT;
    varos_core::format::check_structure(&doc, &limits).map_err(|e| e.to_string())?;
    varos_core::format::validate(&doc, &limits).map_err(|e| e.to_string())?;
    varos_core::format::encode_model(&doc, &limits).map_err(|e| e.to_string())?;
    Ok((doc, report))
}

pub fn import_file(bytes: &[u8], format: Format, options: ImportOptions) -> Result<(Document, ImportReport), String> {
    crate::import_cancellable(bytes, format, options, std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)))
}
