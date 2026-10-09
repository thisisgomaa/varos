//! Phase 3 bounded import worker and revision-checked publication. Provisional UI owner review pending.
use crate::{app_command::SessionId, file_jobs::CancelFlag, workspace::Workspace};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use varos_core::images::{self, LinkInfo, PlacementMode};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    #[serde(default)]
    pub mode: PlacementMode,
    #[serde(default)]
    pub at: [f32; 2],
    #[serde(default)]
    pub bounds: Option<[f32; 4]>,
    #[serde(default)]
    pub ppi: Option<[f32; 2]>,
    #[serde(default)]
    pub xform: Option<images::ImageAffine>,
}
impl Default for Options {
    fn default() -> Self {
        Self { mode: Default::default(), at: [0.; 2], bounds: None, ppi: None, xform: None }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    pub sid: SessionId,
    pub ticket: u64,
    pub expected_rev: u64,
    pub path: PathBuf,
    pub replace: Option<u32>,
    pub bytes: Option<std::sync::Arc<[u8]>>,
    pub options: Options,
    pub bridge: bool,
    pub cancel: CancelFlag,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Done {
    pub job: Job,
    pub result: Result<(images::codec::Decoded, Option<LinkInfo>), String>,
}
pub fn execute(job: Job) -> Done {
    let result = (|| {
        if job.cancel.flag().load(std::sync::atomic::Ordering::Relaxed) {
            return Err("Image placement cancelled".into());
        }
        let bytes = if let Some(bytes) = &job.bytes {
            bytes.to_vec()
        } else if job.bridge {
            let extension = job.path.extension().and_then(|e| e.to_str()).ok_or("Image source extension required")?;
            if !["png", "jpg", "jpeg", "gif", "webp", "tif", "tiff", "bmp"]
                .contains(&extension.to_ascii_lowercase().as_str())
            {
                return Err("Unsupported image extension".into());
            }
            varos_bridge::files::read_source(&job.path, extension, images::MAX_ORIGINAL).map_err(|e| e.reason)?
        } else {
            images::links::read_original(&job.path)?
        };
        let decoded = images::codec::decode(&bytes)?;
        let home = varos_bridge::files::account_home().ok();
        let link =
            if job.bytes.is_none() { Some(images::links::locator(&job.path, &bytes, home.as_deref())?) } else { None };
        Ok((decoded, link))
    })();
    Done { job, result }
}
pub fn complete(done: Done, ws: &mut Workspace) -> Result<varos_bridge::Reply, String> {
    let job = &done.job;
    if job.cancel.flag().load(std::sync::atomic::Ordering::Relaxed) {
        return Err("Image placement cancelled".into());
    }
    let s = ws.get_mut(job.sid).ok_or("Image target tab closed")?;
    if s.editor.rev != job.expected_rev
        || !matches!(s.editor.drag, varos_core::editor::Drag::None)
        || !matches!(s.editor.ab_drag, varos_core::editor::AbDrag::None)
        || s.editor.transaction_open()
    {
        return Err("Image target changed; place again after finishing the gesture".into());
    }
    let (decoded, link) = done.result?;
    let link = link.map(|info| images::links::relative_locator(info, &s.editor.blobs));
    let notes = decoded.notes.clone();
    let before = s.editor.clone();
    let result = (|| {
        let mut image = images::stage(&mut s.editor, decoded, job.options.at, job.options.mode, link)?;
        if let Some(ppi) = job.options.ppi {
            image.ppi = ppi;
            image.xform.a = 72. / ppi[0];
            image.xform.d = 72. / ppi[1];
        }
        if let Some([x, y, w, h]) = job.options.bounds {
            image.xform =
                images::ImageAffine { a: w / image.px_w as f32, b: 0., c: 0., d: h / image.px_h as f32, e: x, f: y };
        }
        if let Some(xform) = job.options.xform {
            image.xform = xform;
        }
        let edit = if let Some(id) = job.replace {
            images::ImageEdit::Replace { id, image }
        } else {
            images::ImageEdit::Add { image, parent: None }
        };
        s.editor.try_execute(varos_core::EditCommand::Image(edit))?;
        Ok(())
    })();
    if let Err(e) = result {
        s.editor = before;
        return Err(e);
    }
    Ok(varos_bridge::Reply::success(
        serde_json::json!({"rev":s.editor.rev,"image":s.editor.doc.images.last().map(|i|format!("image:{}",i.id)),"report":notes}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn done(ws: &Workspace, options: Options) -> Done {
        let sid = ws.active_id().unwrap();
        let bytes = images::codec::encode_png(&images::Pixels {
            budget: None,
            width: 2,
            height: 3,
            rgba: std::sync::Arc::from([255, 0, 0, 255].repeat(6)),
        })
        .unwrap();
        Done {
            job: Job {
                sid,
                ticket: 1,
                expected_rev: ws.get(sid).unwrap().editor.rev,
                path: "/tmp/source.png".into(),
                replace: None,
                bytes: None,
                options,
                bridge: false,
                cancel: Default::default(),
            },
            result: Ok((
                images::codec::decode(&bytes).unwrap(),
                Some(LinkInfo {
                    absolute: "/tmp/source.png".into(),
                    home_relative: None,
                    document_relative: None,
                    accepted_mtime: String::new(),
                    byte_size: bytes.len(),
                    hash: images::content_key(&bytes),
                }),
            )),
        }
    }
    #[test]
    fn placement_one_undo_and_ppi_override() {
        let mut ws = Workspace::new();
        let d = done(&ws, Options { ppi: Some([144., 288.]), ..Default::default() });
        complete(d, &mut ws).unwrap();
        let ed = &mut ws.active_mut().unwrap().editor;
        assert_eq!(ed.doc.images[0].ppi, [144., 288.]);
        assert_eq!(ed.doc.images[0].xform.a, 0.5);
        ed.undo();
        assert!(ed.doc.images.is_empty());
        ed.redo();
        assert_eq!(ed.doc.images.len(), 1);
    }
    #[test]
    fn stale_closed_cancelled_and_bad_geometry_never_publish() {
        let mut ws = Workspace::new();
        let mut d = done(&ws, Options::default());
        d.job.expected_rev += 1;
        assert!(complete(d, &mut ws).is_err());
        let d = done(&ws, Options::default());
        d.job.cancel.flag().store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(complete(d, &mut ws).is_err());
        let d = done(&ws, Options { bounds: Some([0., 0., 0., 10.]), ..Default::default() });
        assert!(complete(d, &mut ws).is_err());
        assert!(ws.active().unwrap().editor.doc.images.is_empty());
        assert_eq!(ws.active().unwrap().editor.blobs.retained_bytes(), 0);
        let d = done(&ws, Options::default());
        ws.new_untitled();
        ws.remove(d.job.sid);
        assert!(complete(d, &mut ws).is_err());
    }
    #[test]
    fn replacement_is_one_undo_and_retains_original() {
        let mut ws = Workspace::new();
        let d = done(&ws, Options::default());
        complete(d, &mut ws).unwrap();
        let id = ws.active().unwrap().editor.doc.images[0].id;
        let before = ws.active().unwrap().editor.doc.clone();
        let mut d = done(&ws, Options::default());
        d.job.replace = Some(id);
        let (decoded, _) = d.result.as_mut().unwrap();
        let bytes = images::codec::encode_png(&images::Pixels {
            budget: None,
            width: 4,
            height: 2,
            rgba: std::sync::Arc::from([0, 255, 0, 255].repeat(8)),
        })
        .unwrap();
        *decoded = images::codec::decode(&bytes).unwrap();
        d.result.as_mut().unwrap().1.as_mut().unwrap().hash = images::content_key(&bytes);
        d.result.as_mut().unwrap().1.as_mut().unwrap().byte_size = bytes.len();
        complete(d, &mut ws).unwrap();
        let ed = &mut ws.active_mut().unwrap().editor;
        assert_eq!(ed.doc.images[0].px_w, 4);
        ed.undo();
        assert_eq!(ed.doc, before);
        assert!(ed.blobs.get(&before.images[0].blob).is_some());
    }
}
