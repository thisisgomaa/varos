//! Lane H host routing and provisional fidelity confirmation; no foreign fallback from native read.
use std::{io::Read, path::Path};
pub fn bytes(path: &Path) -> Result<Vec<u8>, String> {
    let mut b = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take((varos_import::MAX_BYTES + 1) as u64)
        .read_to_end(&mut b)
        .map_err(|e| e.to_string())?;
    if b.len() > varos_import::MAX_BYTES {
        return Err("Import exceeds byte limit".into());
    }
    Ok(b)
}
pub fn is_foreign(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    match ext.to_ascii_lowercase().as_str() {
        "svg" | "svgz" | "ai" | "dxf" | "dwg" => true,
        "pdf" => bytes(path).is_ok_and(|b| {
            !b.windows(6).any(|w| w == b"VAROS_")
                && !b.windows(b"model.varos.json".len()).any(|w| w == b"model.varos.json")
        }),
        _ => false,
    }
}
pub fn read(path: &Path) -> Result<(varos_core::model::Document, varos_import::ImportReport), String> {
    let ext = path.extension().and_then(|e| e.to_str()).ok_or("Import requires source extension")?;
    varos_import::worker::isolated_import(
        &bytes(path)?,
        varos_import::Format::from_extension(ext)?,
        varos_import::ImportOptions { loss_policy: varos_import::LossPolicy::AllowReported, ..Default::default() },
        &std::sync::atomic::AtomicBool::new(false),
    )
}
pub fn accept_losses(notes: &[String]) -> bool {
    #[cfg(not(test))]
    {
        matches!(
            rfd::MessageDialog::new()
                .set_title("Import fidelity report")
                .set_description(format!("{}\n\nImport this converted artwork?", notes.join("\n")))
                .set_buttons(rfd::MessageButtons::YesNo)
                .show(),
            rfd::MessageDialogResult::Yes
        )
    }
    #[cfg(test)]
    {
        let _ = notes;
        false
    }
}

/// Provisional native choices: explicit one-based PDF page and unitless DXF scale.
pub fn choose_options(path: &Path) -> Option<varos_import::ImportOptions> {
    use rfd::{MessageButtons, MessageDialog, MessageDialogResult};
    let mut options =
        varos_import::ImportOptions { loss_policy: varos_import::LossPolicy::AllowReported, ..Default::default() };
    match path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase().as_str() {
        "pdf" | "ai" => {
            let mut page = 1;
            loop {
                let reply = MessageDialog::new().set_title("Import PDF page (provisional)")
                    .set_description(format!("Import page {page}? Choose Next page to increase the page number. Pages are one-based; unavailable pages refuse without changing artwork."))
                    .set_buttons(MessageButtons::YesNoCancelCustom("Import page".into(), "Next page".into(), "Cancel".into())).show();
                match reply {
                    MessageDialogResult::Custom(v) if v == "Import page" => {
                        options.page = Some(page);
                        break;
                    }
                    MessageDialogResult::Custom(v) if v == "Next page" && page < 100 => page += 1,
                    _ => return None,
                }
            }
        }
        "dxf" => {
            match MessageDialog::new().set_title("DXF units (provisional)")
                .set_description("Use the drawing's declared units? For unitless drawings choose units explicitly. An explicit choice overrides declared units.")
                .set_buttons(MessageButtons::YesNoCancelCustom("Use declared units".into(), "Choose units".into(), "Cancel".into())).show() {
                MessageDialogResult::Custom(v) if v == "Use declared units" => return Some(options),
                MessageDialogResult::Custom(v) if v == "Choose units" => {},
                _ => return None,
            }
            let reply = MessageDialog::new().set_title("Unitless DXF units (provisional)")
                .set_description("Interpret one drawing unit as a millimetre or a point. This explicit choice overrides any declared units.")
                .set_buttons(MessageButtons::YesNoCancelCustom("Millimetres".into(), "Points".into(), "Cancel".into())).show();
            options.points_per_unit = Some(match reply {
                MessageDialogResult::Custom(v) if v == "Millimetres" => 72.0 / 25.4,
                MessageDialogResult::Custom(v) if v == "Points" => 1.0,
                _ => return None,
            });
        }
        _ => {}
    }
    Some(options)
}
