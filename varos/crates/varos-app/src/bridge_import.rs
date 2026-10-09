//! Lane H host adapter: typed source boundaries, cancellation, clipboard generations, checked edit.
use crate::workspace::Workspace;
use std::sync::atomic::{AtomicBool, Ordering};
use varos_bridge::{dto::FileEffect, Error, Reply};
pub fn perform(ws: &mut Workspace, verb: &str, request: &FileEffect, cancel: &AtomicBool) -> Result<Reply, Error> {
    let guard =
        || if cancel.load(Ordering::Acquire) { Err(Error::new("cancelled", "Import cancelled")) } else { Ok(()) };
    guard()?;
    let id = crate::app_command::SessionId(
        request
            .board
            .strip_prefix('b')
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| Error::new("invalid_argument", "board must be bN"))?,
    );
    let options: varos_import::ImportOptions = request
        .options
        .clone()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| Error::new("invalid_argument", e.to_string()))?
        .unwrap_or_default();
    if verb == "import_clipboard" {
        let s = ws.get_mut(id).ok_or_else(|| Error::new("not_found", "board closed"))?;
        let mut pasteboard = crate::clipboard_in::SystemPasteboard;
        let staged = crate::clipboard_in::stage(&s.editor, &mut pasteboard, options)
            .map_err(|e| Error::new("invalid_argument", e))?;
        guard()?;
        let report = if let Some((doc, report, generation)) = staged {
            if varos_import::clipboard::Pasteboard::generation(&pasteboard) != generation {
                return Err(Error::new("stale_clipboard", "Clipboard changed"));
            }
            s.editor
                .try_execute(varos_core::EditCommand::PlaceArtwork(Box::new(doc)))
                .map_err(|e| Error::new("invalid_argument", e))?;
            report
        } else {
            if s.editor.clipboard().is_empty() {
                return Err(Error::new("not_found", "Clipboard is empty"));
            }
            s.editor
                .try_execute(varos_core::EditCommand::Paste { offset: None })
                .map_err(|e| Error::new("invalid_argument", e))?;
            varos_import::ImportReport::default()
        };
        return Ok(Reply::success(serde_json::json!({"rev":s.editor.rev,"report":report})));
    }
    let path = std::path::Path::new(
        request.path.as_deref().ok_or_else(|| Error::new("invalid_argument", "source path required"))?,
    );
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| Error::new("invalid_argument", "source extension required"))?;
    let format = varos_import::Format::from_extension(extension).map_err(|e| Error::new("unsupported", e))?;
    if verb == "import_svg" && format != varos_import::Format::Svg {
        return Err(Error::new("invalid_argument", "import_svg requires SVG/SVGZ"));
    }
    let bytes = varos_bridge::files::read_source(path, extension, varos_import::MAX_BYTES)?;
    let (doc, report) = varos_import::worker::isolated_import(&bytes, format, options, cancel)
        .map_err(|e| Error::new("invalid_argument", e))?;
    guard()?;
    let s = ws.get_mut(id).ok_or_else(|| Error::new("not_found", "board closed"))?;
    varos_core::placement::check(&s.editor, &doc).map_err(|e| Error::new("invalid_argument", e))?;
    s.editor
        .try_execute(varos_core::EditCommand::PlaceArtwork(Box::new(doc)))
        .map_err(|e| Error::new("invalid_argument", e))?;
    Ok(Reply::success(serde_json::json!({"rev":s.editor.rev,"report":report})))
}
