//! Start v2 lane L2: a "New board" (zero artboards) and a preset board work through the real native
//! save gate (`write_pdf_checked`), reopen, and export; board metadata (Arabic included) survives the
//! PDF container but never leaks into a deliverable export. Headless — no Renderer, no EventLoop.

use std::sync::atomic::AtomicBool;

use varos_core::board::{new_board, new_board_with_preset, PresetId};
use varos_core::format::Limits;
use varos_core::model::{Anchor, Document, Path};
use varos_pdf::ExportUnavailable;
use varos_pdf::{default_scope, export_pdf_bytes, load_vrs_bytes, plan_pdf_export, write_pdf_checked, ExportScope};

fn with_triangle(mut d: Document) -> Document {
    let a = |id: u32, x: f32, y: f32| Anchor { id, p: [x, y], hin: None, hout: None, smooth: false };
    d.paths.push(Path::new(
        1,
        vec![a(1, 100.0, 100.0), a(2, 300.0, 100.0), a(3, 200.0, 260.0)],
        true,
        Some([0.9, 0.2, 0.1, 1.0]),
        None,
        1.0,
    ));
    d.ids = 3;
    d.sync_tree();
    d
}

fn named(mut d: Document) -> Document {
    d.name = "لوحة جديدة".into();
    d.description = "SECRET-DESCRIPTION وصف".into();
    d.tags = vec!["عربي".into(), "SECRET-TAG".into()];
    d
}

#[test]
fn new_board_with_zero_artboards_saves_reopens_and_exports_its_artwork_bounds() {
    // an EMPTY new board saves and reopens (nothing to export yet — said plainly, never a dummy page)
    let empty = new_board();
    let bytes = write_pdf_checked(&empty, &Limits::DEFAULT).expect("an empty board saves");
    let back = load_vrs_bytes(&bytes, &Limits::DEFAULT).unwrap();
    assert_eq!(back.doc, empty);
    assert!(back.doc.artboards.is_empty());
    assert_eq!(default_scope(&empty), ExportScope::ArtworkBounds);
    assert_eq!(plan_pdf_export(&empty, ExportScope::ArtworkBounds), Err(ExportUnavailable::NothingToExport));
    assert_eq!(plan_pdf_export(&empty, ExportScope::ActiveArtboard), Err(ExportUnavailable::NeedsArtboards));
    assert_eq!(plan_pdf_export(&empty, ExportScope::AllVisibleArtboards), Err(ExportUnavailable::NeedsArtboards));

    // with free artwork and metadata: save → reopen → export one page fitted to the art
    let d = named(with_triangle(new_board()));
    let bytes = write_pdf_checked(&d, &Limits::DEFAULT).expect("a named free board saves");
    let back = load_vrs_bytes(&bytes, &Limits::DEFAULT).unwrap();
    assert_eq!(back.doc, d, "metadata and art survive the PDF container");
    assert_eq!((back.source_version, back.migrated), (varos_core::format::FORMAT_VERSION, false));
    let plan = plan_pdf_export(&back.doc, default_scope(&back.doc)).unwrap();
    assert_eq!(plan.page_count(), 1);
    let rect = plan.pages[0].rect;
    assert_eq!((rect[2], rect[3]), (200.0, 160.0), "the page is the artwork bounds");
    let pdf = export_pdf_bytes(&back.doc, &plan, &AtomicBool::new(false)).unwrap();
    assert!(!varos_pdf::has_embedded_model(&pdf), "an export carries no editable model");
    for secret in ["SECRET-DESCRIPTION", "SECRET-TAG"] {
        assert!(
            !pdf.windows(secret.len()).any(|w| w == secret.as_bytes()),
            "the deliverable export never carries board metadata ({secret})"
        );
    }
}

#[test]
fn preset_boards_save_reopen_and_export_their_one_artboard() {
    for (id, w, h) in [
        (PresetId::Square, 1080.0, 1080.0),
        (PresetId::Portrait, 1080.0, 1350.0),
        (PresetId::Story, 1080.0, 1920.0),
        (PresetId::A4, 595.0, 842.0),
    ] {
        let d = named(new_board_with_preset(id));
        let bytes = write_pdf_checked(&d, &Limits::DEFAULT).unwrap();
        let back = load_vrs_bytes(&bytes, &Limits::DEFAULT).unwrap().doc;
        assert_eq!(back, d, "{id:?}");
        let plan = plan_pdf_export(&back, default_scope(&back)).unwrap();
        assert_eq!(plan.scope, ExportScope::AllVisibleArtboards);
        assert_eq!(plan.page_count(), 1);
        assert_eq!(plan.pages[0].rect, [0.0, 0.0, w, h], "{id:?}");
        assert!(export_pdf_bytes(&back, &plan, &AtomicBool::new(false)).is_ok());
    }
}

#[test]
fn over_bound_metadata_is_refused_by_the_native_save_gate() {
    let mut d = new_board();
    d.tags = (0..17).map(|i| format!("t{i}")).collect();
    let reason = write_pdf_checked(&d, &Limits::DEFAULT).unwrap_err();
    assert_eq!(reason, "This document can't be saved: the board has 17 tags; the limit is 16. It is still open.");
}
