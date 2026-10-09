//! Image Trace expands accepted oriented pixels into editable paths, atomically.
use super::{image_hidden, image_locked, ImageEdit};
use crate::{
    trace::{TraceOptions, TraceReport},
    EditCommand, Editor,
};
pub fn expand(ed: &mut Editor, id: u32, options: &TraceOptions) -> Result<TraceReport, String> {
    if image_hidden(&ed.doc, id) || image_locked(&ed.doc, id) {
        return Err("Image is hidden or locked".into());
    }
    let image = ed.doc.images.iter().find(|i| i.id == id).ok_or("Image not found")?.clone();
    let blob = ed.blobs.get(&image.blob).ok_or("Image resource unavailable")?;
    if blob.original.is_none() {
        return Err("Trace requires the full original".into());
    }
    let (mut paths, report) = crate::trace::trace(&blob.pixels.rgba, blob.pixels.width, blob.pixels.height, options)?;
    if paths.is_empty() {
        return Err("Trace produced no paths; image retained".into());
    }
    for path in &mut paths {
        path.opacity = image.opacity;
        for a in path.anchors.iter_mut().chain(path.holes.iter_mut().flatten()) {
            a.p = image.xform.apply(a.p);
            a.hin = a.hin.map(|p| image.xform.apply(p));
            a.hout = a.hout.map(|p| image.xform.apply(p));
        }
    }
    ed.try_execute(EditCommand::Image(ImageEdit::ExpandTrace { id, paths }))?;
    Ok(report)
}
