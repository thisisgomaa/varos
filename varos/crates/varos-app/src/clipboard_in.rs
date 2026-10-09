//! NSPasteboard input; snapshots, generation guard, staged losses and one checked undo step.
use varos_core::{editor::Editor, geom::Pt};
use varos_import::clipboard::{Pasteboard, Snapshot};
#[cfg(test)]
use varos_import::ImportOptions;
pub struct SystemPasteboard;
#[cfg(all(target_os = "macos", not(test)))]
impl Pasteboard for SystemPasteboard {
    fn snapshot(&mut self) -> Result<Snapshot, String> {
        use objc2_app_kit::NSPasteboard;
        use objc2_foundation::NSString;
        let board = NSPasteboard::generalPasteboard();
        let generation = board.changeCount() as i64;
        let mut flavours = Vec::new();
        for kind in varos_import::clipboard::TYPES {
            if let Some(data) = board.dataForType(&NSString::from_str(kind)) {
                if data.length() > varos_import::MAX_BYTES {
                    return Err("Clipboard exceeds byte limit".into());
                }
                flavours.push(((*kind).into(), data.to_vec()));
            }
        }
        if board.changeCount() as i64 != generation {
            return Err("Clipboard changed during snapshot".into());
        }
        Ok(Snapshot { generation, flavours })
    }
    fn generation(&self) -> i64 {
        objc2_app_kit::NSPasteboard::generalPasteboard().changeCount() as i64
    }
}
#[cfg(any(not(target_os = "macos"), test))]
impl Pasteboard for SystemPasteboard {
    fn snapshot(&mut self) -> Result<Snapshot, String> {
        Ok(Snapshot::default())
    }
    fn generation(&self) -> i64 {
        0
    }
}
/// Only bounded snapshotting and internal-flavour validation happen on the UI thread.
pub fn capture(editor: &Editor, board: &mut dyn Pasteboard) -> Result<Option<Snapshot>, String> {
    let snapshot = board.snapshot()?;
    if snapshot.flavours.iter().any(|(_, b)| b.len() > varos_import::MAX_BYTES) {
        return Err("Clipboard exceeds byte limit".into());
    }
    if board.generation() != snapshot.generation {
        return Err("Clipboard changed during snapshot".into());
    }
    if let Some(bytes) = snapshot.find("org.varos.clipboard") {
        let trusted = serde_json::to_value(editor.clipboard()).map_err(|e| e.to_string())?;
        let supplied: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if supplied == trusted {
            return Ok(None);
        }
        return Err("Unknown/malformed Varos internal clipboard; foreign fallback requires explicit choice".into());
    }
    if snapshot.flavours.is_empty() {
        return Ok(None);
    }
    Ok(Some(snapshot))
}
/// None keeps the trusted in-app clipboard. A foreign preferred flavour failure never falls back.
#[cfg(test)]
pub fn stage(
    editor: &Editor,
    board: &mut dyn Pasteboard,
    options: ImportOptions,
) -> Result<Option<(varos_core::model::Document, varos_import::ImportReport, i64)>, String> {
    let Some(snapshot) = capture(editor, board)? else { return Ok(None) };
    let (doc, report, _) = varos_import::clipboard::stage(&snapshot, options)?;
    varos_core::placement::check(editor, &doc)?;
    if board.generation() != snapshot.generation {
        return Err("Clipboard changed during import".into());
    }
    Ok(Some((doc, report, snapshot.generation)))
}
pub fn centre(doc: &mut varos_core::model::Document, target: Pt) {
    if let Some(bounds) =
        doc.paths.iter().flat_map(|p| varos_core::model::Document::ring(&p.anchors, p.closed, 24)).fold(
            None,
            |b: Option<[f32; 4]>, p| {
                Some(b.map_or([p[0], p[1], p[0], p[1]], |b| {
                    [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]
                }))
            },
        )
    {
        let offset = [target[0] - (bounds[0] + bounds[2]) / 2., target[1] - (bounds[1] + bounds[3]) / 2.];
        for p in &mut doc.paths {
            for a in p.anchors.iter_mut().chain(p.holes.iter_mut().flatten()) {
                for pt in std::iter::once(&mut a.p).chain(a.hin.iter_mut()).chain(a.hout.iter_mut()) {
                    pt[0] += offset[0];
                    pt[1] += offset[1];
                }
            }
        }
    }
}

/// Keyboard/menu entry: own the source bytes now, submit conversion, leave the editor untouched.
pub fn queue(
    ws: &mut crate::workspace::Workspace,
    ui: &mut dyn crate::host::DocUi,
    canvas: egui::Rect,
    dialogs: &mut dyn crate::lifecycle::Dialogs,
    jobs: &mut dyn crate::host::FileJobs,
    in_place: bool,
) -> crate::host::Ran {
    if ws.on_home() {
        return crate::host::Ran::default();
    }
    if let Some(s) = ws.active_mut() {
        if !ui.settle_fields(&mut s.editor) {
            return crate::host::Ran { held: true, ..Default::default() };
        }
        match crate::clipboard_in::capture(&s.editor, &mut crate::clipboard_in::SystemPasteboard) {
            Ok(Some(snapshot)) => {
                let mut job = crate::import_jobs::Job::new(
                    "Clipboard".into(),
                    crate::import_jobs::Target::Place { sid: s.id, rev: s.editor.rev },
                );
                let Some(options) = dialogs.import_options(std::path::Path::new(
                    if snapshot.find("com.adobe.pdf").is_some() || snapshot.find("application/pdf").is_some() {
                        "Clipboard.pdf"
                    } else {
                        "Clipboard.svg"
                    },
                )) else {
                    return crate::host::Ran::default();
                };
                job.options = options;
                job.clipboard = Some(Box::new(snapshot));
                if !in_place {
                    let c = canvas.center();
                    job.centre = Some(s.view.s2w([c.x, c.y]));
                }
                if jobs.submit(crate::file_jobs::FileJob::Import(job)).is_err() {
                    dialogs.open_failed("Clipboard", "Import worker unavailable");
                }
            }
            Ok(None) => crate::paste_key(&mut s.editor, &s.view, [canvas.center().x, canvas.center().y], in_place),
            Err(e) => dialogs.open_failed("Clipboard", &e),
        }
    }
    crate::host::Ran::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use varos_core::EditCommand;
    struct Fake {
        snapshot: Snapshot,
        generation: i64,
    }
    impl Pasteboard for Fake {
        fn snapshot(&mut self) -> Result<Snapshot, String> {
            Ok(self.snapshot.clone())
        }
        fn generation(&self) -> i64 {
            self.generation
        }
    }
    fn svg() -> Vec<u8> {
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><path d="M0 0L20 0L0 10Z" fill="red"/></svg>"#.to_vec()
    }
    #[test]
    fn preferred_flavour_generation_and_locked_destination_are_atomic() {
        let mut editor = Editor::new();
        let before = editor.doc.clone();
        let snapshot = Snapshot { generation: 7, flavours: vec![("public.svg-image".into(), svg())] };
        let mut fake = Fake { snapshot: snapshot.clone(), generation: 8 };
        assert!(stage(&editor, &mut fake, ImportOptions::default()).unwrap_err().contains("changed"));
        assert_eq!(editor.doc, before);
        fake.generation = 7;
        let (doc, report, _) = stage(&editor, &mut fake, ImportOptions::default()).unwrap().unwrap();
        assert!(report.loss_notes.is_empty());
        editor.try_execute(EditCommand::PlaceArtwork(Box::new(doc))).unwrap();
        assert_eq!(editor.doc.paths.len(), 1);
        editor.execute_ui(EditCommand::Undo);
        assert_eq!(editor.doc, before);
        editor.doc.nodes.iter_mut().find(|n| n.id == editor.doc.active_layer).unwrap().locked = true;
        assert!(stage(&editor, &mut fake, ImportOptions::default()).is_err());
        fake.snapshot.flavours[0].1 = b"invalid SVG".to_vec();
        fake.snapshot.flavours.push(("public.png".into(), vec![1]));
        assert!(stage(&Editor::new(), &mut fake, ImportOptions::default()).is_err());
    }
    #[test]
    fn trusted_internal_copy_wins_and_unknown_internal_refuses() {
        let editor = Editor::new();
        let snapshot = Snapshot {
            generation: 1,
            flavours: vec![
                ("org.varos.clipboard".into(), serde_json::to_vec(editor.clipboard()).unwrap()),
                ("public.svg-image".into(), b"bad".to_vec()),
            ],
        };
        let mut fake = Fake { snapshot, generation: 1 };
        assert!(stage(&editor, &mut fake, ImportOptions::default()).unwrap().is_none());
        fake.snapshot.flavours[0].1 = b"{}".to_vec();
        assert!(stage(&editor, &mut fake, ImportOptions::default()).is_err());
    }
}
