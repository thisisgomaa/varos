//! Lane H: snapshot on owning thread, conversion on file worker, revision-bound publication.
use crate::{
    import_jobs::{Job, Target},
    workspace::Workspace,
};
use varos_bridge::{dto::FileEffect, Error};
pub fn stage(ws: &Workspace, verb: &str, request: &FileEffect) -> Result<Option<Job>, Error> {
    let id = crate::app_command::SessionId(
        request
            .board
            .strip_prefix('b')
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| Error::new("invalid_argument", "board must be bN"))?,
    );
    let s = ws.get(id).ok_or_else(|| Error::new("not_found", "board closed"))?;
    let options = request
        .options
        .clone()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| Error::new("invalid_argument", e.to_string()))?
        .unwrap_or_default();
    let mut job = Job::new(
        request.path.as_deref().unwrap_or("Clipboard").into(),
        Target::Place { sid: id, rev: request.expected_rev },
    );
    job.options = options;
    if verb == "import_clipboard" {
        job.clipboard = crate::clipboard_in::capture(&s.editor, &mut crate::clipboard_in::SystemPasteboard)
            .map_err(|e| Error::new("invalid_argument", e))?
            .map(Box::new);
        if job.clipboard.is_none() {
            return Ok(None);
        }
    } else {
        let ext = job
            .path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| Error::new("invalid_argument", "source extension required"))?;
        let format = varos_import::Format::from_extension(ext).map_err(|e| Error::new("unsupported", e))?;
        if verb == "import_svg" && format != varos_import::Format::Svg {
            return Err(Error::new("invalid_argument", "import_svg requires SVG/SVGZ"));
        }
    }
    Ok(Some(job))
}
