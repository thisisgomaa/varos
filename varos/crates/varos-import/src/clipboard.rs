//! Detached pasteboard snapshot. Highest supported flavour never falls back after failure.
use crate::{Format, ImportOptions, ImportReport, MAX_BYTES};
use varos_core::model::Document;
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub generation: i64,
    pub flavours: Vec<(String, Vec<u8>)>,
}
pub trait Pasteboard {
    fn snapshot(&mut self) -> Result<Snapshot, String>;
    fn generation(&self) -> i64;
}
impl Snapshot {
    pub fn find(&self, kind: &str) -> Option<&[u8]> {
        self.flavours.iter().find(|(k, _)| k == kind).map(|(_, b)| b.as_slice())
    }
}
pub const TYPES: &[&str] = &[
    "org.varos.clipboard",
    "public.svg-image",
    "image/svg+xml",
    "com.adobe.pdf",
    "application/pdf",
    "public.png",
    "public.tiff",
];
/// The host validates its own internal flavour before calling foreign import.
pub fn stage(snapshot: &Snapshot, options: ImportOptions) -> Result<(Document, ImportReport, String), String> {
    if snapshot.flavours.iter().any(|(_, b)| b.len() > MAX_BYTES) {
        return Err("Clipboard exceeds import byte limit".into());
    }
    for (k, f) in [
        ("public.svg-image", Format::Svg),
        ("image/svg+xml", Format::Svg),
        ("com.adobe.pdf", Format::Pdf),
        ("application/pdf", Format::Pdf),
        ("public.png", Format::Bitmap),
        ("public.tiff", Format::Bitmap),
    ] {
        if let Some(bytes) = snapshot.find(k) {
            let (doc, mut report) =
                crate::worker::isolated_import(bytes, f, options, &std::sync::atomic::AtomicBool::new(false))?;
            report.source_flavour = Some(k.into());
            return Ok((doc, report, k.into()));
        }
    }
    Err("Clipboard has no supported artwork flavour".into())
}
