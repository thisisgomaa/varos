//! OS clipboard output. Original implementation; UTI names follow Apple's pasteboard contracts.
use varos_core::{clipboard::Clipboard, editor::Editor, model::Document, EditCommand};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flavour {
    pub kind: &'static str,
    pub bytes: Vec<u8>,
}
pub trait Pasteboard {
    fn publish(&mut self, flavours: &[Flavour]) -> Result<(), String>;
}
/// The internal detached clipboard is first; public formats contain only visible selected artwork.
#[cfg(test)]
pub fn build(doc: &Document, clipboard: &Clipboard) -> Result<Vec<Flavour>, String> {
    let vectors = varos_pdf::clipboard_vectors(doc, clipboard)?;
    let png = varos_raster::clipboard_png(vectors.document, vectors.rect)?;
    Ok(vec![
        Flavour { kind: "org.varos.clipboard", bytes: vectors.internal },
        Flavour { kind: "com.adobe.pdf", bytes: vectors.pdf },
        Flavour { kind: "public.svg-image", bytes: vectors.svg },
        Flavour { kind: "public.png", bytes: png },
    ])
}

#[cfg(test)]
pub fn publish(doc: &Document, clipboard: &Clipboard, pasteboard: &mut dyn Pasteboard) -> Result<(), String> {
    pasteboard.publish(&build(doc, clipboard)?)
}
pub struct SystemPasteboard;
#[cfg(all(target_os = "macos", not(test)))]
impl Pasteboard for SystemPasteboard {
    fn publish(&mut self, flavours: &[Flavour]) -> Result<(), String> {
        use objc2_app_kit::NSPasteboard;
        use objc2_foundation::{NSArray, NSData, NSString};
        let names: Vec<_> = flavours.iter().map(|f| NSString::from_str(f.kind)).collect();
        let types = NSArray::from_retained_slice(&names);
        let board = NSPasteboard::generalPasteboard();
        // SAFETY: no lazy owner: all declared types receive eagerly owned NSData below.
        unsafe {
            board.declareTypes_owner(&types, None);
        }
        for (flavour, name) in flavours.iter().zip(names) {
            if !board.setData_forType(Some(&NSData::with_bytes(&flavour.bytes)), &name) {
                return Err(format!("Could not publish {} to the clipboard.", flavour.kind));
            }
        }
        Ok(())
    }
}
#[cfg(all(not(target_os = "macos"), not(test)))]
impl Pasteboard for SystemPasteboard {
    fn publish(&mut self, _: &[Flavour]) -> Result<(), String> {
        Err("OS clipboard output is currently available on macOS only.".into())
    }
}
#[cfg(test)]
impl Pasteboard for SystemPasteboard {
    fn publish(&mut self, _: &[Flavour]) -> Result<(), String> {
        Ok(())
    }
}
/// Actual published types and reasons a public flavour was unavailable.
#[derive(Debug, Default, serde::Serialize)]
pub struct ClipboardReport {
    pub published: Vec<&'static str>,
    pub omitted: Vec<String>,
}
fn build_available(doc: &Document, clipboard: &Clipboard) -> Result<(Vec<Flavour>, ClipboardReport), String> {
    let mut flavours =
        vec![Flavour { kind: "org.varos.clipboard", bytes: serde_json::to_vec(clipboard).map_err(|e| e.to_string())? }];
    let mut report = ClipboardReport::default();
    match varos_pdf::clipboard_vectors(doc, clipboard) {
        Ok(vectors) => {
            flavours.push(Flavour { kind: "com.adobe.pdf", bytes: vectors.pdf });
            flavours.push(Flavour { kind: "public.svg-image", bytes: vectors.svg });
            match varos_raster::clipboard_png(vectors.document, vectors.rect) {
                Ok(bytes) => flavours.push(Flavour { kind: "public.png", bytes }),
                Err(reason) => report.omitted.push(format!("public.png: {reason}")),
            }
        }
        Err(reason) => {
            for kind in ["com.adobe.pdf", "public.svg-image", "public.png"] {
                report.omitted.push(format!("{kind}: {reason}"));
            }
        }
    }
    report.published = flavours.iter().map(|f| f.kind).collect();
    Ok((flavours, report))
}
pub fn perform(editor: &mut Editor, cut: bool, pasteboard: &mut dyn Pasteboard) -> Result<ClipboardReport, String> {
    let clipboard = editor.capture_selection_clipboard(cut);
    if clipboard.is_empty() {
        return Err("Select artwork to copy (anchor-only selections cannot be copied).".into());
    }
    let (flavours, report) = build_available(&editor.doc, &clipboard)?;
    pasteboard.publish(&flavours)?;
    if cut {
        // No selection/document mutation occurs between capture and this command, so it deletes
        // exactly the lock-filtered sources just published, with the existing single undo step.
        editor.execute(EditCommand::Cut).map_err(|e| e.to_string())?;
    } else {
        editor.execute(EditCommand::Copy).map_err(|e| e.to_string())?;
    }
    Ok(report)
}
pub fn copy_or_cut(editor: &mut Editor, cut: bool) {
    if editor.capture_selection_clipboard(cut).is_empty() {
        return;
    }
    #[cfg(not(target_os = "macos"))]
    {
        editor.execute_ui(if cut { EditCommand::Cut } else { EditCommand::Copy });
    }
    #[cfg(target_os = "macos")]
    {
        let message = match perform(editor, cut, &mut SystemPasteboard) {
            Err(error) => Some(error),
            Ok(report) if !report.omitted.is_empty() => Some(format!(
                "Artwork captured losslessly for Varos. Omitted public clipboard formats:\n{}",
                report.omitted.join("\n")
            )),
            Ok(_) => None,
        };
        if let Some(message) = message {
            #[cfg(not(test))]
            {
                rfd::MessageDialog::new().set_title("Clipboard").set_description(&message).show();
            }
            #[cfg(test)]
            let _ = message;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{atomic::AtomicBool, Arc};
    use varos_core::model::Artboard;
    fn fixture() -> (Document, Clipboard) {
        let mut editor = Editor::new();
        editor.doc.paths.push(varos_core::model::Path::new(
            2,
            vec![
                varos_core::model::Anchor { id: 3, p: [0., 0.], hin: None, hout: None, smooth: false },
                varos_core::model::Anchor { id: 4, p: [20., 0.], hin: None, hout: None, smooth: false },
                varos_core::model::Anchor { id: 5, p: [0., 10.], hin: None, hout: None, smooth: false },
            ],
            true,
            Some([1., 0., 0., 1.]),
            None,
            0.,
        ));
        editor.doc.ids = 100;
        editor.doc.sync_tree();
        let clipboard = Clipboard::capture(&editor.doc, &[2]);
        (editor.doc, clipboard)
    }
    #[derive(Default)]
    struct RecordingBoard(Vec<Flavour>);
    impl Pasteboard for RecordingBoard {
        fn publish(&mut self, flavours: &[Flavour]) -> Result<(), String> {
            self.0 = flavours.to_vec();
            Ok(())
        }
    }
    fn selected_editor() -> Editor {
        let (doc, _) = fixture();
        let mut editor = Editor::new();
        editor.replace_doc(doc);
        editor.objsel.insert(2);
        editor
    }
    #[test]
    fn direct_path_copy_and_cut_capture_fresh_sources_but_anchors_never_publish_stale_data() {
        for cut in [false, true] {
            let mut editor = selected_editor();
            editor.objsel.clear();
            editor.dsel_path = Some(2);
            let mut board = RecordingBoard::default();
            perform(&mut editor, cut, &mut board).unwrap();
            let captured: Clipboard = serde_json::from_slice(&board.0[0].bytes).unwrap();
            assert_eq!(captured.source_ids().collect::<Vec<_>>(), [2]);
            assert_eq!(editor.clipboard(), &captured);
            assert_eq!(editor.doc.paths.is_empty(), cut);
            if cut {
                editor.execute(EditCommand::Undo).unwrap();
            }
            editor.dsel_path = None;
            editor.selected.insert(3);
            let stale = editor.clipboard().clone();
            let mut untouched_board = RecordingBoard::default();
            assert!(perform(&mut editor, cut, &mut untouched_board).is_err());
            assert!(untouched_board.0.is_empty());
            assert_eq!(editor.clipboard(), &stale);
            assert_eq!(editor.doc.paths.len(), 1);
        }
    }
    #[test]
    fn cut_nonpainting_art_keeps_lossless_payload_reports_omissions_and_undoes_once() {
        for opacity_zero in [false, true] {
            let mut editor = selected_editor();
            if opacity_zero {
                editor.doc.paths[0].opacity = 0.0;
            } else {
                editor.doc.paths[0].fill = varos_core::model::Paint::None;
            }
            let before = editor.doc.clone();
            let mut board = RecordingBoard::default();
            let report = perform(&mut editor, true, &mut board).unwrap();
            assert_eq!(report.published, ["org.varos.clipboard"]);
            assert_eq!(report.omitted.len(), 3);
            let captured: Clipboard = serde_json::from_slice(&board.0[0].bytes).unwrap();
            assert_eq!(editor.clipboard(), &captured);
            assert_eq!(captured.len(), 1);
            assert!(editor.doc.paths.is_empty());
            editor.execute(EditCommand::Undo).unwrap();
            assert_eq!(editor.doc, before);
        }
    }
    #[test]
    fn oversized_cut_omits_only_png_and_still_cuts_losslessly() {
        let mut editor = selected_editor();
        editor.doc.paths[0].anchors[1].p[0] = 9000.0;
        let before = editor.doc.clone();
        let mut board = RecordingBoard::default();
        let report = perform(&mut editor, true, &mut board).unwrap();
        assert_eq!(report.published, ["org.varos.clipboard", "com.adobe.pdf", "public.svg-image"]);
        assert_eq!(report.omitted.len(), 1);
        assert!(report.omitted[0].starts_with("public.png: Selection is too large"));
        assert!(editor.doc.paths.is_empty());
        assert_eq!(editor.clipboard(), &serde_json::from_slice::<Clipboard>(&board.0[0].bytes).unwrap());
        editor.execute(EditCommand::Undo).unwrap();
        assert_eq!(editor.doc, before);
    }
    #[test]
    fn grouped_cut_publishes_exact_unlocked_payload_before_deleting_the_same_sources() {
        let mut editor = selected_editor();
        let mut locked = editor.doc.paths[0].clone();
        locked.id = 6;
        for a in &mut locked.anchors {
            a.id += 10;
            a.p[0] += 40.0;
        }
        editor.doc.paths.push(locked);
        editor.doc.sync_tree();
        editor.objsel.insert(6);
        editor.group_selection();
        let leaf = editor.doc.node_of_path(6).unwrap();
        editor.doc.nodes.iter_mut().find(|n| n.id == leaf).unwrap().locked = true;
        assert_eq!(editor.capture_selection_clipboard(false).len(), 2);
        let expected = editor.capture_selection_clipboard(true);
        assert_eq!(expected.source_ids().collect::<Vec<_>>(), [2]);
        let expected_flavours = build(&editor.doc, &expected).unwrap();
        let before = editor.doc.clone();
        let mut board = RecordingBoard::default();
        perform(&mut editor, true, &mut board).unwrap();
        assert_eq!(board.0, expected_flavours);
        assert_eq!(editor.clipboard(), &expected);
        assert_eq!(editor.doc.paths.iter().map(|p| p.id).collect::<Vec<_>>(), [6]);
        editor.execute(EditCommand::Undo).unwrap();
        assert_eq!(editor.doc, before);
    }
    #[test]
    fn flavours_match_writers_and_fake_pasteboard_never_touches_os() {
        let (doc, clipboard) = fixture();
        let flavours = build(&doc, &clipboard).unwrap();
        assert_eq!(
            flavours.iter().map(|f| f.kind).collect::<Vec<_>>(),
            ["org.varos.clipboard", "com.adobe.pdf", "public.svg-image", "public.png"]
        );
        assert_eq!(serde_json::from_slice::<Clipboard>(&flavours[0].bytes).unwrap(), clipboard);
        let (narrowed, plan) = varos_pdf::plan_selection_export(&doc, &[2].into()).unwrap();
        assert_eq!(flavours[1].bytes, varos_pdf::export_pdf_bytes(&narrowed, &plan, &AtomicBool::new(false)).unwrap());
        let p = plan.pages[0];
        let svg_plan = varos_core::svg::ExportPlan {
            scope: varos_core::svg::ExportScope::WholeBoard,
            pages: vec![varos_core::svg::PageSpec {
                rect: p.rect,
                background: None,
                artboard: None,
                name: String::new(),
            }],
        };
        assert_eq!(
            flavours[2].bytes,
            varos_core::svg::export_svg_files(&narrowed, &svg_plan, &AtomicBool::new(false)).unwrap()[0].bytes
        );
        let mut png_doc = narrowed;
        let [x, y, w, h] = p.rect;
        png_doc.artboards = vec![Artboard { x, y, w, h, page_color: None, ..Artboard::default() }];
        png_doc.active = 0;
        assert_eq!(
            flavours[3].bytes,
            varos_raster::rasterize_artboard(Arc::new(png_doc), 0, [(w * 2.).ceil() as u32, (h * 2.).ceil() as u32])
                .unwrap()
                .encode_png()
                .unwrap()
        );
        #[derive(Default)]
        struct Fake(Vec<Flavour>);
        impl Pasteboard for Fake {
            fn publish(&mut self, f: &[Flavour]) -> Result<(), String> {
                self.0 = f.to_vec();
                Ok(())
            }
        }
        let mut fake = Fake::default();
        publish(&doc, &clipboard, &mut fake).unwrap();
        assert_eq!(fake.0, flavours);
    }
    #[test]
    fn cut_publishes_before_deletion_and_pasteboard_failure_preserves_artwork() {
        let (doc, _) = fixture();
        let mut editor = Editor::new();
        editor.replace_doc(doc);
        editor.objsel.insert(2);
        struct Fake {
            fail: bool,
            flavours: Vec<Flavour>,
        }
        impl Pasteboard for Fake {
            fn publish(&mut self, flavours: &[Flavour]) -> Result<(), String> {
                if self.fail {
                    Err("pasteboard unavailable".into())
                } else {
                    self.flavours = flavours.to_vec();
                    Ok(())
                }
            }
        }
        let before = editor.doc.clone();
        let mut board = Fake { fail: true, flavours: vec![] };
        assert!(perform(&mut editor, true, &mut board).is_err());
        assert_eq!(editor.doc, before);
        board.fail = false;
        perform(&mut editor, true, &mut board).unwrap();
        assert!(editor.doc.paths.is_empty());
        assert!(board.flavours[1].bytes.starts_with(b"%PDF-"));
        editor.execute(EditCommand::Undo).unwrap();
        assert_eq!(editor.doc, before);
    }
}
