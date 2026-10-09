//! Print job preparation is pure; the system hand-off is an explicit host action.
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
use varos_core::model::Document;
use varos_pdf::{ExportScope, PdfOptions};
#[derive(Debug)]
pub struct PrintJob {
    pub bytes: Vec<u8>,
    pub path: PathBuf,
}
pub fn build(
    doc: &Document,
    scope: ExportScope,
    options: &PdfOptions,
    temp: &Path,
    ticket: u64,
) -> Result<PrintJob, String> {
    let plan = varos_pdf::plan_pdf_export(doc, scope).map_err(|e| e.to_string())?;
    let (bytes, _) = varos_pdf::export_pdf_with_options(doc, &plan, options, &AtomicBool::new(false))?;
    Ok(PrintJob { bytes, path: temp.join(format!("varos-print-{}-{ticket}.pdf", std::process::id())) })
}
#[cfg(target_os = "macos")]
pub fn preview_command(path: &Path) -> std::process::Command {
    let mut command = std::process::Command::new("/usr/bin/open");
    command.args(["-a", "Preview"]).arg(path);
    command
}
/// Permitted Preview fallback: opens the prepared PDF; the user chooses Print in Preview.
pub fn hand_off(job: PrintJob) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        varos_core::file::write_atomic(&job.path, &job.bytes)?;
        let status = preview_command(&job.path).status().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Preview could not open the print PDF.".into());
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        drop((job.bytes, job.path));
        Err("Printing is currently available on macOS only.".into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builder_uses_export_bytes_without_printing() {
        let mut doc = Document::default();
        doc.artboards.push(Default::default());
        let options = PdfOptions::default();
        let job = build(&doc, ExportScope::AllVisibleArtboards, &options, Path::new("/tmp"), 42).unwrap();
        let plan = varos_pdf::plan_pdf_export(&doc, ExportScope::AllVisibleArtboards).unwrap();
        assert_eq!(job.bytes, varos_pdf::export_pdf_bytes(&doc, &plan, &AtomicBool::new(false)).unwrap());
        assert_eq!(job.path, Path::new("/tmp").join(format!("varos-print-{}-42.pdf", std::process::id())));
        #[cfg(target_os = "macos")]
        assert_eq!(preview_command(&job.path).get_program(), "/usr/bin/open");
    }
}
