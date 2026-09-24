//! S6-A — the native `.vrs` writer stays byte-identical across the write-side refactor, and the pure
//! PDF export (no embedded model, no hidden data) plans and writes the right pages. Everything is
//! inspected through lopdf; no GPU, no window.
//!
//! Run with:  cargo test -p varos-pdf --test export_pdf

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use varos_core::model::{Anchor, Artboard, Document, GroupRole, Node, NodeKind, Path, Xform};
use varos_pdf::{
    default_scope, export_pdf_bytes, has_embedded_model, load_vrs, plan_pdf_export, save_vrs, write_pdf, ExportError,
    ExportPlan, ExportScope, ExportUnavailable, PageSpec,
};

// ───────────────────────────── document builders ─────────────────────────────

fn anc(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}
fn rect(pid: u32, first_aid: u32, x: f32, y: f32, w: f32, h: f32, fill: Option<[f32; 4]>) -> Path {
    let a = first_aid;
    Path::new(
        pid,
        vec![anc(a, x, y), anc(a + 1, x + w, y), anc(a + 2, x + w, y + h), anc(a + 3, x, y + h)],
        true,
        fill,
        None,
        1.0,
    )
}

/// `container.rs::demo_doc` verbatim: one board, a knockout (fill + translucent stroke) triangle at
/// 0.8 opacity and an open red stroke.
fn demo_doc() -> Document {
    let mut d = Document {
        artboards: vec![Artboard { x: 0.0, y: 0.0, w: 400.0, h: 300.0, ..Default::default() }],
        ..Default::default()
    };
    d.paths.push(Path::new(
        1,
        vec![anc(1, 40.0, 40.0), anc(2, 200.0, 60.0), anc(3, 120.0, 220.0)],
        true,
        Some([0.2, 0.7, 0.3, 1.0]),
        Some([0.0, 0.0, 0.0, 0.5]),
        6.0,
    )); // knockout case
    d.paths.push(Path::new(
        2,
        vec![anc(4, 250.0, 50.0), anc(5, 350.0, 250.0)],
        false,
        None,
        Some([0.9, 0.2, 0.2, 1.0]),
        3.0,
    )); // open stroke
    d.paths[0].opacity = 0.8;
    d.ids = 5;
    d.sync_tree();
    d
}

/// A masked, rotated, translucent two-board document (board B has a translucent page colour), with a
/// curved compound path (hole), a hidden path and a free floater off every page.
fn rich_doc() -> Document {
    let mut d = Document {
        artboards: vec![
            Artboard { x: 0.0, y: 0.0, w: 400.0, h: 300.0, name: "A".into(), ..Default::default() },
            Artboard {
                x: 500.0,
                y: 0.0,
                w: 300.0,
                h: 300.0,
                name: "B".into(),
                page_color: Some([1.0, 0.9, 0.8, 0.5]),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    // board A: a translucent rect + a rotated rect
    let mut p1 = rect(1, 1, 20.0, 20.0, 120.0, 80.0, Some([0.1, 0.3, 0.9, 0.7]));
    p1.opacity = 0.6;
    d.paths.push(p1);
    let mut p2 = rect(2, 5, 200.0, 100.0, 100.0, 40.0, Some([0.9, 0.5, 0.1, 1.0]));
    p2.stroke = varos_core::model::Paint::from_opt(Some([0.0, 0.0, 0.0, 1.0]));
    p2.stroke_width = 2.0;
    d.paths.push(p2);
    // board B: a curved compound path with a hole, knockout stroke
    let mut p3 = Path::new(
        3,
        vec![
            Anchor { id: 9, p: [550.0, 50.0], hin: None, hout: Some([600.0, 20.0]), smooth: false },
            Anchor { id: 10, p: [700.0, 60.0], hin: Some([680.0, 30.0]), hout: None, smooth: false },
            anc(11, 720.0, 200.0),
            anc(12, 560.0, 220.0),
        ],
        true,
        Some([0.3, 0.8, 0.4, 1.0]),
        Some([0.1, 0.1, 0.1, 0.4]),
        8.0,
    );
    p3.holes = vec![vec![anc(13, 600.0, 100.0), anc(14, 650.0, 100.0), anc(15, 625.0, 150.0)]];
    d.paths.push(p3);
    // board B: a clip group — member 4 clipped by mask 5
    d.paths.push(rect(4, 16, 520.0, 180.0, 200.0, 100.0, Some([0.8, 0.1, 0.5, 1.0])));
    d.paths.push(rect(5, 20, 560.0, 200.0, 80.0, 60.0, Some([0.0, 0.0, 0.0, 1.0])));
    // a hidden path on A, and a floater off every page
    let mut p6 = rect(6, 24, 50.0, 200.0, 30.0, 30.0, Some([1.0, 0.0, 0.0, 1.0]));
    p6.hidden = true;
    d.paths.push(p6);
    d.paths.push(rect(7, 28, 2000.0, 2000.0, 10.0, 10.0, Some([0.0, 1.0, 0.0, 1.0])));
    d.ids = 31;
    d.sync_tree();
    let unit = d.unit_of(2).expect("rect 2's unit");
    d.set_node_xform(unit, Xform { rot: 0.5, piv: [250.0, 120.0] });
    d.clip_group(&[4, 5], 5).expect("4 clips to 5");
    d
}

// ───────────────────────────── native byte identity ─────────────────────────────

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join(name)
}

/// The native `.vrs` bytes must not move when the writer is refactored.
///
/// `tests/fixtures/native_demo.pdf` (= `write_pdf(&demo_doc())`) and `tests/fixtures/native_rich.pdf`
/// (= `write_pdf(&rich_doc())`) were captured from commit c9067d4 (branch `claude/sweet-cerf-1sg30t`,
/// BEFORE the S6-A write-side refactor) by running this test with `VAROS_BLESS_PDF_FIXTURES=1`; both
/// still matched after the refactor (commit 69c34c4). `native_rich.pdf` was then re-blessed ONCE, on
/// purpose, when the page loop learned the PDF clip (MASKS_PLAN Stage 5): the only change is board B's
/// content stream gaining `q <mask rect> W* n … Q` around the clipped member (+ its /Length and offsets).
/// The later switch to one clip scope per RUN of members (+ ring culling) left both files unchanged
/// (that document has a single clip member whose mask ring is in reach).
/// `write_pdf` is fully deterministic (pdf-writer writes no timestamp, no /ID and no /Info; the model
/// blob is serde JSON of a document whose only map field, the legacy `group_of`, is empty), so the
/// whole file is compared byte for byte. Re-bless ONLY for an intentional native-format change (e.g.
/// S5's schema version bump) and say so in that PR.
#[test]
fn native_write_is_byte_identical_to_fixture() {
    let bless = std::env::var_os("VAROS_BLESS_PDF_FIXTURES").is_some();
    for (name, doc) in [("native_demo.pdf", demo_doc()), ("native_rich.pdf", rich_doc())] {
        let bytes = write_pdf(&doc).expect("native write");
        assert_eq!(bytes, write_pdf(&doc).unwrap(), "{name}: the native writer is deterministic");
        let path = fixture(name);
        if bless {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &bytes).unwrap();
            continue;
        }
        let want = std::fs::read(&path).unwrap_or_else(|e| panic!("{name}: fixture missing ({e})"));
        assert!(
            bytes == want,
            "{name}: native .vrs bytes changed ({} bytes now, {} in the fixture)",
            bytes.len(),
            want.len()
        );
    }
}

// ───────────────────────────── pure export: helpers ─────────────────────────────

fn plan(doc: &Document, scope: ExportScope) -> ExportPlan {
    plan_pdf_export(doc, scope).unwrap_or_else(|u| panic!("{scope:?} should plan, got {u:?}"))
}
fn export(doc: &Document, scope: ExportScope) -> Vec<u8> {
    let plan = plan(doc, scope);
    export_pdf_bytes(doc, &plan, &AtomicBool::new(false)).expect("exports")
}
fn load(bytes: &[u8]) -> lopdf::Document {
    lopdf::Document::load_mem(bytes).expect("lopdf parses the export")
}
/// Each page's MediaBox, in page order.
fn media_boxes(bytes: &[u8]) -> Vec<[f32; 4]> {
    let pdf = load(bytes);
    pdf.get_pages()
        .values()
        .map(|&pid| {
            let page = pdf.get_dictionary(pid).unwrap();
            let mb = page.get(b"MediaBox").unwrap().as_array().unwrap();
            [0, 1, 2, 3].map(|i| mb[i].as_float().unwrap())
        })
        .collect()
}
/// Each page's decoded content-stream operations, in page order.
fn page_ops(bytes: &[u8]) -> Vec<Vec<lopdf::content::Operation>> {
    let pdf = load(bytes);
    pdf.get_pages()
        .values()
        .map(|&pid| lopdf::content::Content::decode(&pdf.get_page_content(pid).unwrap()).unwrap().operations)
        .collect()
}
fn has_op(ops: &[lopdf::content::Operation], op: &str, operands: &[f32]) -> bool {
    find_op(ops, op, operands).is_some()
}
/// Every stream in the file, decompressed when possible — content streams AND form XObjects.
fn all_streams(bytes: &[u8]) -> Vec<Vec<u8>> {
    load(bytes)
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .map(|s| s.decompressed_content().unwrap_or_else(|_| s.content.clone()))
        .collect()
}
fn close4(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-3)
}
fn contains(hay: &[u8], needle: &str) -> bool {
    hay.windows(needle.len()).any(|w| w == needle.as_bytes())
}

/// Two boards: A (400×300, white) with a blue rect, B (200×250, transparent) with a red rect.
fn two_board_doc() -> Document {
    let mut d = Document {
        artboards: vec![
            Artboard { x: 0.0, y: 0.0, w: 400.0, h: 300.0, name: "A".into(), ..Default::default() },
            Artboard { x: 500.0, y: 0.0, w: 200.0, h: 250.0, name: "B".into(), page_color: None, ..Default::default() },
        ],
        ..Default::default()
    };
    d.paths.push(rect(1, 1, 40.0, 40.0, 100.0, 50.0, Some([0.0, 0.0, 1.0, 1.0])));
    d.paths.push(rect(2, 5, 540.0, 60.0, 50.0, 50.0, Some([1.0, 0.0, 0.0, 1.0])));
    d.ids = 8;
    d.sync_tree();
    d
}

// ───────────────────────────── scope ─────────────────────────────

#[test]
fn default_scope_follows_boards() {
    assert_eq!(default_scope(&two_board_doc()), ExportScope::AllVisibleArtboards);
    assert_eq!(default_scope(&Document::default()), ExportScope::ArtworkBounds);
}

#[test]
fn all_visible_two_boards_two_pages_sized_like_boards() {
    let doc = two_board_doc();
    let p = plan(&doc, ExportScope::AllVisibleArtboards);
    assert_eq!(p.pages.len(), 2);
    let bytes = export(&doc, ExportScope::AllVisibleArtboards);
    assert_eq!(media_boxes(&bytes), vec![[0.0, 0.0, 400.0, 300.0], [0.0, 0.0, 200.0, 250.0]]);
    let ops = page_ops(&bytes);
    // page 1: white background + the blue rect (world 40,40 → page 40, 300−40)
    assert!(has_op(&ops[0], "rg", &[1.0, 1.0, 1.0]) && has_op(&ops[0], "re", &[0.0, 0.0, 400.0, 300.0]));
    assert!(has_op(&ops[0], "m", &[40.0, 260.0]) && has_op(&ops[0], "rg", &[0.0, 0.0, 1.0]));
    // page 2: transparent (no background rect), the red rect in board-local coords (540−500, 250−60)
    assert!(!ops[1].iter().any(|o| o.operator == "re"), "a transparent page draws no background");
    assert!(has_op(&ops[1], "m", &[40.0, 190.0]) && has_op(&ops[1], "rg", &[1.0, 0.0, 0.0]));
    assert!(!has_op(&ops[1], "rg", &[0.0, 0.0, 1.0]), "board A's art is not on board B's page");
}

#[test]
fn hidden_board_excluded() {
    let mut doc = two_board_doc();
    doc.artboards.push(Artboard { x: 0.0, y: 400.0, w: 300.0, h: 100.0, ..Default::default() });
    doc.artboards[1].hidden = true;
    let p = plan(&doc, ExportScope::AllVisibleArtboards);
    assert_eq!(p.pages.len(), 2, "3 boards, 1 hidden → 2 pages");
    let bytes = export(&doc, ExportScope::AllVisibleArtboards);
    assert_eq!(media_boxes(&bytes), vec![[0.0, 0.0, 400.0, 300.0], [0.0, 0.0, 300.0, 100.0]]);
    assert!(page_ops(&bytes).iter().all(|ops| !has_op(ops, "rg", &[1.0, 0.0, 0.0])), "the hidden board's art is gone");
}

#[test]
fn all_hidden_is_unavailable_not_a_dummy_page() {
    let mut doc = two_board_doc();
    for ab in &mut doc.artboards {
        ab.hidden = true;
    }
    assert_eq!(plan_pdf_export(&doc, ExportScope::AllVisibleArtboards), Err(ExportUnavailable::NoVisibleArtboards));
    assert_eq!(plan_pdf_export(&doc, ExportScope::ActiveArtboard), Err(ExportUnavailable::ActiveArtboardHidden));
    assert_eq!(plan_pdf_export(&doc, ExportScope::ArtworkBounds), Err(ExportUnavailable::NotBoardless));
}

#[test]
fn active_scope_one_page() {
    let mut doc = two_board_doc();
    doc.active = 1;
    let p = plan(&doc, ExportScope::ActiveArtboard);
    assert_eq!(p.pages.len(), 1);
    assert_eq!(p.pages[0], PageSpec { rect: [500.0, 0.0, 200.0, 250.0], background: None });
    let bytes = export(&doc, ExportScope::ActiveArtboard);
    assert_eq!(media_boxes(&bytes), vec![[0.0, 0.0, 200.0, 250.0]]);
    let ops = page_ops(&bytes);
    assert!(has_op(&ops[0], "rg", &[1.0, 0.0, 0.0]) && !has_op(&ops[0], "rg", &[0.0, 0.0, 1.0]));
}

#[test]
fn active_hidden_unavailable() {
    let mut doc = two_board_doc();
    doc.active = 1;
    doc.artboards[1].hidden = true;
    assert_eq!(plan_pdf_export(&doc, ExportScope::ActiveArtboard), Err(ExportUnavailable::ActiveArtboardHidden));
    assert_eq!(plan(&doc, ExportScope::AllVisibleArtboards).page_count(), 1, "the other board still exports");
}

#[test]
fn board_scopes_need_boards_and_bounds_needs_none() {
    let mut doc = two_board_doc();
    assert_eq!(plan_pdf_export(&doc, ExportScope::ArtworkBounds), Err(ExportUnavailable::NotBoardless));
    doc.artboards.clear();
    assert_eq!(plan_pdf_export(&doc, ExportScope::AllVisibleArtboards), Err(ExportUnavailable::NeedsArtboards));
    assert_eq!(plan_pdf_export(&doc, ExportScope::ActiveArtboard), Err(ExportUnavailable::NeedsArtboards));
    for u in [
        ExportUnavailable::NoVisibleArtboards,
        ExportUnavailable::ActiveArtboardHidden,
        ExportUnavailable::NotBoardless,
        ExportUnavailable::NeedsArtboards,
        ExportUnavailable::NothingToExport,
    ] {
        assert!(!u.reason().is_empty() && u.reason().ends_with('.'), "{u:?} has user copy");
    }
}

#[test]
fn boardless_artwork_bounds_page_matches_padded_bbox() {
    let mut doc = Document::default(); // a free canvas: no artboards
    doc.paths.push(rect(1, 1, 10.0, 20.0, 100.0, 50.0, Some([0.0, 0.5, 0.0, 1.0]))); // fill only → no pad
    let line = vec![anc(5, 200.0, 100.0), anc(6, 300.0, 100.0)];
    doc.paths.push(Path::new(2, line, false, None, Some([0.0, 0.0, 0.0, 1.0]), 4.0)); // 4-pt stroke → pad 2
    let mut ghost = rect(3, 7, 900.0, 900.0, 10.0, 10.0, Some([1.0, 0.0, 0.0, 1.0]));
    ghost.hidden = true; // hidden art must not stretch the page
    doc.paths.push(ghost);
    doc.ids = 10;
    doc.sync_tree();
    // x: rect 10 … line 300 + 2 (half of the 4-pt stroke); y: rect 20 … line 100 + 2
    let p = plan(&doc, ExportScope::ArtworkBounds);
    assert_eq!(p.pages.len(), 1);
    assert!(close4(p.pages[0].rect, [10.0, 20.0, 292.0, 82.0]) && p.pages[0].background.is_none(), "{:?}", p.pages);
    let bytes = export(&doc, ExportScope::ArtworkBounds);
    let mb = media_boxes(&bytes);
    assert!(mb.len() == 1 && close4(mb[0], [0.0, 0.0, 292.0, 82.0]), "{mb:?}");
    let ops = &page_ops(&bytes)[0];
    assert!(!ops.iter().any(|o| o.operator == "re"), "artwork bounds are transparent: no background");
    // the rect's top-left world (10,20) → page (0, 82); the line starts at world (200,100) → page (190, 2)
    assert!(has_op(ops, "m", &[0.0, 82.0]) && has_op(ops, "m", &[190.0, 2.0]));
}

#[test]
fn boardless_empty_is_nothing_to_export() {
    let mut doc = Document::default();
    assert_eq!(plan_pdf_export(&doc, ExportScope::ArtworkBounds), Err(ExportUnavailable::NothingToExport));
    let mut hidden = rect(1, 1, 0.0, 0.0, 10.0, 10.0, Some([0.0, 0.0, 0.0, 1.0]));
    hidden.hidden = true;
    doc.paths.push(hidden);
    doc.ids = 5;
    doc.sync_tree();
    assert_eq!(plan_pdf_export(&doc, ExportScope::ArtworkBounds), Err(ExportUnavailable::NothingToExport));
    // and an empty hand-built plan never becomes a zero-page PDF
    let empty = ExportPlan { scope: ExportScope::ArtworkBounds, pages: vec![] };
    assert_eq!(
        export_pdf_bytes(&doc, &empty, &AtomicBool::new(false)),
        Err(ExportError::Unavailable(ExportUnavailable::NothingToExport))
    );
}

// ───────────────────────────── privacy ─────────────────────────────

/// Unique coordinates the hidden things carry; none may appear anywhere in the export.
const SECRET_NUMBERS: [&str; 5] = ["277.125", "290.125", "311.375", "1077.625", "77.625"];
const SECRET_NAMES: [&str; 5] = ["SECRET-LAYER", "Board Secret", "Board Public", "Visible Name", "Hidden Name"];

/// Board "Board Public" (visible) + "Board Secret" (hidden); a visible LOCKED named path; a hidden path;
/// a path on a hidden layer; a path standing only on the hidden board.
fn secret_doc() -> Document {
    let mut d = Document {
        artboards: vec![
            Artboard { x: 0.0, y: 0.0, w: 400.0, h: 300.0, name: "Board Public".into(), ..Default::default() },
            Artboard {
                x: 1000.0,
                y: 0.0,
                w: 200.0,
                h: 200.0,
                name: "Board Secret".into(),
                hidden: true,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut visible = rect(1, 1, 40.0, 40.0, 100.0, 50.0, Some([0.0, 0.0, 1.0, 1.0]));
    visible.name = Some("Visible Name".into());
    visible.locked = true;
    d.paths.push(visible);
    let mut hidden = rect(2, 5, 277.125, 40.0, 13.0, 20.0, Some([1.0, 0.0, 0.0, 1.0])); // spans 277.125 … 290.125
    hidden.hidden = true;
    hidden.name = Some("Hidden Name".into());
    d.paths.push(hidden);
    d.paths.push(rect(3, 9, 1077.625, 50.0, 30.0, 30.0, Some([1.0, 0.0, 0.0, 1.0]))); // only on the hidden board
    d.ids = 20;
    d.sync_tree();
    // a hidden layer holding one path at x = 311.375 (on the visible board)
    let layer = d.nid();
    d.nodes.push(Node {
        id: layer,
        kind: NodeKind::Layer,
        name: "SECRET-LAYER".into(),
        parent: None,
        children: vec![],
        hidden: true,
        locked: true,
        color: None,
        clip_exempt: false,
        xform: Xform::default(),
        role: GroupRole::Normal,
        mask_child: None,
    });
    d.roots.insert(0, layer);
    d.active_layer = layer;
    d.paths.push(rect(4, 13, 311.375, 100.0, 20.0, 20.0, Some([1.0, 0.0, 0.0, 1.0])));
    d.sync_tree();
    d
}

#[test]
fn secret_doc_really_hides_what_the_privacy_tests_think_it_hides() {
    // guard the fixture itself: if the model's hiding rules changed, the privacy tests would pass vacuously
    let d = secret_doc();
    assert!(!d.eff_hidden(1) && d.eff_locked(1), "the visible path is visible and locked");
    assert!(d.eff_hidden(2) && d.eff_hidden(3) && d.eff_hidden(4), "hidden path / hidden board / hidden layer");
    let native = write_pdf(&d).unwrap();
    assert!(contains(&native, "SECRET-LAYER") && contains(&native, "1077.625"), "the NATIVE file does carry them");
}

#[test]
fn export_has_no_embedded_file_filespec_af_names_or_varos_keys() {
    let doc = secret_doc();
    let p = plan(&doc, ExportScope::AllVisibleArtboards);
    let bytes = export(&doc, ExportScope::AllVisibleArtboards);
    let pdf = load(&bytes);
    assert_eq!(pdf.get_pages().len(), p.pages.len(), "page count = planned pages");
    assert!(!pdf.trailer.has(b"Info"), "no Info dictionary");
    assert!(!pdf.trailer.has(b"ID"), "no file ID");
    let banned_keys: [&[u8]; 6] = [b"EmbeddedFiles", b"EmbeddedFile", b"EF", b"AF", b"Names", b"Metadata"];
    let banned_types: [&[u8]; 2] = [b"EmbeddedFile", b"Filespec"];
    for (id, obj) in &pdf.objects {
        let dict = match obj {
            lopdf::Object::Dictionary(d) => d,
            lopdf::Object::Stream(s) => &s.dict,
            _ => continue,
        };
        for (k, v) in dict.iter() {
            assert!(!banned_keys.contains(&k.as_slice()), "object {id:?} carries /{}", String::from_utf8_lossy(k));
            assert!(!k.starts_with(b"VAROS"), "object {id:?} carries a private /VAROS_* key");
            if k.as_slice() == b"Type" {
                let t = v.as_name().unwrap_or_default();
                assert!(!banned_types.contains(&t), "object {id:?} is a /{}", String::from_utf8_lossy(t));
            }
        }
    }
    for needle in ["EmbeddedFile", "Filespec", "/AF", "/Names", "VAROS", "model.varos.json", "\"varos\"", "\"paths\""] {
        assert!(!contains(&bytes, needle), "raw export bytes contain {needle:?}");
    }
    assert!(!has_embedded_model(&bytes));
}

#[test]
fn export_leaks_no_hidden_coordinates_or_names() {
    let doc = secret_doc();
    for scope in [ExportScope::AllVisibleArtboards, ExportScope::ActiveArtboard] {
        let bytes = export(&doc, scope);
        let streams = all_streams(&bytes);
        assert!(!streams.is_empty());
        for s in SECRET_NUMBERS.iter().chain(SECRET_NAMES.iter()) {
            assert!(!contains(&bytes, s), "{scope:?}: raw bytes leak {s:?}");
            assert!(streams.iter().all(|st| !contains(st, s)), "{scope:?}: a stream leaks {s:?}");
        }
        // only the visible (blue) art paints; the three hidden red things do not
        let ops = page_ops(&bytes);
        assert_eq!(ops.len(), 1);
        assert!(has_op(&ops[0], "rg", &[0.0, 0.0, 1.0]) && !has_op(&ops[0], "rg", &[1.0, 0.0, 0.0]));
    }
}

#[test]
fn hidden_and_locked_state_change_no_export_byte() {
    // the strongest form of "nothing hidden leaks": the export equals the export of a document where
    // the hidden things never existed, and locking changes nothing at all
    let doc = secret_doc();
    let mut clean = Document {
        artboards: vec![Artboard { x: 0.0, y: 0.0, w: 400.0, h: 300.0, ..Default::default() }],
        ..Default::default()
    };
    clean.paths.push(rect(1, 1, 40.0, 40.0, 100.0, 50.0, Some([0.0, 0.0, 1.0, 1.0])));
    clean.ids = 4;
    clean.sync_tree();
    let a = export(&doc, ExportScope::AllVisibleArtboards);
    assert_eq!(a, export(&clean, ExportScope::AllVisibleArtboards), "hidden things and names leave no trace");
    let mut unlocked = doc.clone();
    unlocked.paths.iter_mut().find(|p| p.id == 1).unwrap().locked = false;
    assert_eq!(a, export(&unlocked, ExportScope::AllVisibleArtboards), "lock state leaves no trace");
}

#[test]
fn exported_pdf_is_refused_by_load_vrs() {
    let bytes = export(&two_board_doc(), ExportScope::AllVisibleArtboards);
    let p = std::env::temp_dir().join(format!("varos-s6a-{}-export.pdf", std::process::id()));
    std::fs::write(&p, &bytes).unwrap();
    let r = load_vrs(&p);
    let _ = std::fs::remove_file(&p);
    assert!(r.is_err(), "a pure export carries no model, so it cannot open as a document");
}

// ───────────────────────────── content & behaviour ─────────────────────────────

#[test]
fn visible_content_present() {
    let bytes = export(&demo_doc(), ExportScope::AllVisibleArtboards);
    let ops = &page_ops(&bytes)[0];
    // the knockout triangle is a Form XObject painted by `Do`; the open red stroke is drawn in place
    assert!(ops.iter().any(|o| o.operator == "Do"), "the knockout unit is painted");
    assert!(has_op(ops, "m", &[250.0, 250.0]) && ops.iter().any(|o| o.operator == "S"), "the open stroke");
    assert!(has_op(ops, "RG", &[0.9, 0.2, 0.2]));
    // the triangle's own ops live in its form XObject: fill f* and stroke, starting at (40, 300−40)
    let forms: Vec<_> = all_streams(&bytes)
        .into_iter()
        .filter_map(|s| lopdf::content::Content::decode(&s).ok())
        .filter(|c| c.operations.iter().any(|o| o.operator == "f*"))
        .collect();
    assert!(forms.iter().any(|c| has_op(&c.operations, "m", &[40.0, 260.0])), "the triangle's fill reaches the file");
}

#[test]
fn opacity_and_rotation_survive_export() {
    // the rotated_export.rs rect: 100×40 at [100,100], spun 90° about [150,120] → its first corner lands
    // at world [170,70] → page (170, 330); object opacity 0.5 → an ExtGState with /ca 0.5
    let mut d = Document {
        artboards: vec![Artboard { x: 0.0, y: 0.0, w: 400.0, h: 400.0, ..Default::default() }],
        ..Default::default()
    };
    let mut r = rect(1, 1, 100.0, 100.0, 100.0, 40.0, Some([0.2, 0.6, 0.9, 1.0]));
    r.opacity = 0.5;
    d.paths.push(r);
    d.ids = 4;
    d.sync_tree();
    let unit = d.unit_of(1).unwrap();
    d.set_node_xform(unit, Xform { rot: std::f32::consts::FRAC_PI_2, piv: [150.0, 120.0] });
    let bytes = export(&d, ExportScope::AllVisibleArtboards);
    let ops = &page_ops(&bytes)[0];
    assert!(has_op(ops, "m", &[170.0, 330.0]), "the rotated corner is on the page");
    assert!(ops.iter().any(|o| o.operator == "gs"), "the opacity graphics state is applied");
    let pdf = load(&bytes);
    let half = pdf.objects.values().filter_map(|o| o.as_dict().ok()).any(|d| {
        d.get(b"Type").and_then(|t| t.as_name()).ok() == Some(b"ExtGState".as_slice())
            && d.get(b"ca").and_then(|v| v.as_float()).is_ok_and(|v| (v - 0.5).abs() < 1e-3)
    });
    assert!(half, "an ExtGState with /ca 0.5 carries the object opacity");
}

/// Walk a content stream's `q`/`Q` nesting and report, for every operation, whether a `W*` clip is in
/// force at that point (set inside the current or an enclosing `q` scope).
fn clip_state(ops: &[lopdf::content::Operation]) -> Vec<bool> {
    let mut stack = vec![false];
    let mut out = Vec::with_capacity(ops.len());
    for o in ops {
        match o.operator.as_str() {
            "q" => {
                let top = *stack.last().unwrap();
                stack.push(top);
            }
            "Q" => {
                stack.pop();
                assert!(!stack.is_empty(), "unbalanced Q");
            }
            "W*" => *stack.last_mut().unwrap() = true,
            _ => {}
        }
        out.push(*stack.last().unwrap());
    }
    assert_eq!(stack.len(), 1, "every q has its Q");
    out
}
/// Index of the first `op` with these operands.
fn find_op(ops: &[lopdf::content::Operation], op: &str, operands: &[f32]) -> Option<usize> {
    ops.iter().position(|o| {
        o.operator == op
            && o.operands.len() == operands.len()
            && o.operands.iter().zip(operands).all(|(a, b)| a.as_float().is_ok_and(|a| (a - b).abs() < 1e-3))
    })
}

#[test]
fn clip_member_is_wrapped_in_w_star_n() {
    // rich_doc: on board B (origin 500,0; 300 tall) rect 4 [520,180 → 720,280] is clipped by mask rect 5
    // [560,200 → 640,260]. Page coords: mask starts at (60, 100), the member at (20, 120).
    let doc = rich_doc();
    let bytes = export(&doc, ExportScope::AllVisibleArtboards);
    let ops = &page_ops(&bytes)[1];
    let clipped = clip_state(ops);
    let w = ops.iter().position(|o| o.operator == "W*").expect("a W* clip on the masked page");
    assert_eq!(ops[w + 1].operator, "n", "W* is followed by n (clip only, the mask itself never paints)");
    let mask_m = find_op(ops, "m", &[60.0, 100.0]).expect("the mask ring is the clip path");
    assert!(mask_m < w, "the mask ring is built before W*");
    let member_m = find_op(ops, "m", &[20.0, 120.0]).expect("the member is drawn");
    assert!(member_m > w && clipped[member_m], "the member (whose art runs outside the mask) paints inside the clip");
    let member_rg = find_op(ops, "rg", &[0.8, 0.1, 0.5]).expect("the member's fill colour");
    assert!(clipped[member_rg]);
    assert!(!has_op(ops, "rg", &[0.0, 0.0, 0.0]), "the mask's own black fill never paints");
    // the unclipped art before it (the knockout compound path) is outside any clip scope
    let first_do = ops.iter().position(|o| o.operator == "Do").expect("board B's knockout path");
    assert!(first_do < w && !clipped[first_do]);
    // board A has no mask, so no clip at all
    assert!(!page_ops(&bytes)[0].iter().any(|o| o.operator == "W*"));
    // the shared page loop gives the native .vrs preview pages the same clip
    let native = page_ops(&write_pdf(&doc).unwrap());
    assert!(native[1].iter().any(|o| o.operator == "W*") && clip_state(&native[1]).iter().any(|c| *c));
}

/// One board; members 1 (overlaps the mask) and 2 (wholly outside it, at x = 341.125) clipped by mask 3.
fn masked_doc() -> Document {
    let mut d = Document {
        artboards: vec![Artboard { x: 0.0, y: 0.0, w: 400.0, h: 300.0, page_color: None, ..Default::default() }],
        ..Default::default()
    };
    d.paths.push(rect(1, 1, 20.0, 20.0, 200.0, 100.0, Some([0.8, 0.1, 0.5, 1.0])));
    d.paths.push(rect(2, 5, 341.125, 200.0, 30.0, 30.0, Some([0.2, 0.9, 0.9, 1.0])));
    d.paths.push(rect(3, 9, 60.0, 40.0, 80.0, 60.0, Some([0.0, 0.0, 0.0, 1.0])));
    d.ids = 12;
    d.sync_tree();
    d.clip_group(&[1, 2, 3], 3).expect("3 masks 1 and 2");
    d
}

#[test]
fn member_outside_mask_is_not_emitted() {
    let doc = masked_doc();
    assert!(doc.clip_group_of(2).is_some() && !doc.eff_hidden(2), "member 2 is a live, visible clip member");
    let bytes = export(&doc, ExportScope::AllVisibleArtboards);
    let ops = &page_ops(&bytes)[0];
    assert!(has_op(ops, "rg", &[0.8, 0.1, 0.5]), "the overlapping member is drawn");
    assert!(!has_op(ops, "rg", &[0.2, 0.9, 0.9]), "the member wholly outside the mask is not emitted");
    assert!(!contains(&bytes, "341.125"), "not even its coordinates reach the file");
    assert_eq!(ops.iter().filter(|o| o.operator == "W*").count(), 1, "one clip, for the one emitted member");
}

#[test]
fn a_mask_with_no_drawable_ring_clips_its_members_to_nothing() {
    // the canvas clips to NOTHING when the mask has no ring; the PDF must not draw the member unclipped
    let mut doc = masked_doc();
    doc.paths.iter_mut().find(|p| p.id == 3).unwrap().anchors.truncate(1); // mask shrinks to one point
    let bytes = export(&doc, ExportScope::AllVisibleArtboards);
    let ops = &page_ops(&bytes)[0];
    assert!(!has_op(ops, "rg", &[0.8, 0.1, 0.5]) && !ops.iter().any(|o| o.operator == "W*"));
}

#[test]
fn boardless_bounds_of_a_clip_are_the_clipped_extent() {
    // no boards: the page is the part of the member the mask lets through (the mask's box here),
    // not the member's full box — and the member outside the mask adds nothing
    let mut doc = masked_doc();
    doc.artboards.clear();
    let p = plan(&doc, ExportScope::ArtworkBounds);
    assert!(close4(p.pages[0].rect, [60.0, 40.0, 80.0, 60.0]), "{:?}", p.pages[0]);
    let ops = &page_ops(&export(&doc, ExportScope::ArtworkBounds))[0];
    assert!(ops.iter().any(|o| o.operator == "W*") && has_op(ops, "rg", &[0.8, 0.1, 0.5]));
}

#[test]
fn clip_off_page_leaves_the_page_unclipped() {
    let mut doc = rich_doc();
    doc.active = 0; // board A: the clip group stands on board B only
    let bytes = export(&doc, ExportScope::ActiveArtboard);
    assert_eq!(media_boxes(&bytes), vec![[0.0, 0.0, 400.0, 300.0]]);
    let ops = &page_ops(&bytes)[0];
    assert!(!ops.iter().any(|o| o.operator == "W*") && !has_op(ops, "rg", &[0.8, 0.1, 0.5]));
    assert!(has_op(ops, "rg", &[0.1, 0.3, 0.9]) && has_op(ops, "rg", &[0.9, 0.5, 0.1]), "board A's art is");
}

#[test]
fn export_is_deterministic() {
    let doc = two_board_doc();
    let a = export(&doc, ExportScope::AllVisibleArtboards);
    assert_eq!(a, export(&doc.clone(), ExportScope::AllVisibleArtboards), "a fresh plan + write, same bytes");
}

#[test]
fn cancel_before_first_page_returns_cancelled() {
    let doc = two_board_doc();
    let p = plan(&doc, ExportScope::AllVisibleArtboards);
    assert_eq!(export_pdf_bytes(&doc, &p, &AtomicBool::new(true)), Err(ExportError::Cancelled));
}

#[test]
fn has_embedded_model_true_for_native_false_for_export() {
    let doc = demo_doc();
    assert!(has_embedded_model(&write_pdf(&doc).unwrap()), "a native .vrs carries the model");
    assert!(!has_embedded_model(&export(&doc, ExportScope::AllVisibleArtboards)), "the export does not");
    assert!(!has_embedded_model(b"not a pdf at all"));
    // a byte scan, not a parse: a damaged native file (header gone) is still recognised
    let native = write_pdf(&doc).unwrap();
    assert!(has_embedded_model(&native[64..]));
    assert!(!has_embedded_model(b""));
}

#[test]
fn clipped_native_file_round_trips() {
    // MASKS_PLAN Stage 5's own check: a clipped document saves and reopens with its clip intact
    let doc = rich_doc();
    let p = std::env::temp_dir().join(format!("varos-s6a-{}-clipped.vrs", std::process::id()));
    save_vrs(&doc, &p).unwrap();
    let loaded = load_vrs(&p);
    let _ = std::fs::remove_file(&p);
    let loaded = loaded.unwrap();
    assert_eq!(loaded, doc, "the native container round-trips a clipped document");
    assert_eq!(loaded.clip_group_of(4), doc.clip_group_of(4));
}

/// The `m` (subpath start) ops inside the first clip scope, before its `W*`, as page coordinates.
fn clip_ring_starts(ops: &[lopdf::content::Operation]) -> Vec<[f32; 2]> {
    let w = ops.iter().position(|o| o.operator == "W*").expect("a clip");
    let q = ops[..w].iter().rposition(|o| o.operator == "q").expect("the clip's q");
    assert!(
        ops[q + 1..w].iter().all(|o| !["f", "f*", "B", "B*", "S", "Do"].contains(&o.operator.as_str())),
        "nothing paints while the clip path is built"
    );
    ops[q + 1..w]
        .iter()
        .filter(|o| o.operator == "m")
        .map(|o| [o.operands[0].as_float().unwrap(), o.operands[1].as_float().unwrap()])
        .collect()
}

#[test]
fn hidden_mask_path_far_away_never_reaches_the_file() {
    // a mask group of {m2 on the visible board, m3 HIDDEN and standing only on a hidden board at
    // x = 1077.625}; the member is on the visible board. m3 cannot affect anything the member paints
    // (its box is far away), so its outline must not be written — the "no hidden data" contract.
    let mut d = Document {
        artboards: vec![
            Artboard { x: 0.0, y: 0.0, w: 400.0, h: 300.0, page_color: None, ..Default::default() },
            Artboard { x: 1000.0, y: 0.0, w: 200.0, h: 200.0, hidden: true, ..Default::default() },
        ],
        ..Default::default()
    };
    d.paths.push(rect(1, 1, 20.0, 20.0, 200.0, 100.0, Some([0.8, 0.1, 0.5, 1.0])));
    d.paths.push(rect(2, 5, 60.0, 40.0, 80.0, 60.0, Some([0.0, 0.0, 0.0, 1.0])));
    let mut m3 = rect(3, 9, 1077.625, 50.0, 30.0, 30.0, Some([0.0, 0.0, 0.0, 1.0]));
    m3.hidden = true;
    d.paths.push(m3);
    d.ids = 12;
    d.sync_tree();
    d.group(&[2, 3]).expect("the two mask paths form one mask unit");
    let clip = d.clip_group(&[1, 2], 2).expect("1 clips to the {2,3} mask");
    let mc = d.node_mask_child(clip).unwrap();
    assert!(d.node_paths(mc).contains(&3), "the hidden far path really is part of the mask");

    for bytes in [export(&d, ExportScope::AllVisibleArtboards), export(&d, ExportScope::ActiveArtboard)] {
        assert!(!contains(&bytes, "1077.625"), "raw bytes leak the hidden mask path");
        assert!(all_streams(&bytes).iter().all(|st| !contains(st, "1077.625") && !contains(st, "77.625")));
        let ops = &page_ops(&bytes)[0];
        assert_eq!(clip_ring_starts(ops), vec![[60.0, 260.0]], "only the mask ring that matters is written");
        assert!(has_op(ops, "rg", &[0.8, 0.1, 0.5]), "and the member still paints, clipped");
    }
}

#[test]
fn clip_uses_every_mask_ring_even_odd() {
    // the mask is TWO paths, one with a hole: A = [40,40 → 120,120] minus [60,60 → 100,100], B = [200,40 →
    // 280,120]. One big member covers them all. Every ring (A outer, A hole, B) builds the W* clip path.
    let mut d = Document {
        artboards: vec![Artboard { x: 0.0, y: 0.0, w: 400.0, h: 300.0, page_color: None, ..Default::default() }],
        ..Default::default()
    };
    d.paths.push(rect(1, 1, 20.0, 20.0, 300.0, 180.0, Some([0.8, 0.1, 0.5, 1.0])));
    let mut a = rect(2, 5, 40.0, 40.0, 80.0, 80.0, Some([0.0, 0.0, 0.0, 1.0]));
    a.holes = vec![rect(0, 9, 60.0, 60.0, 40.0, 40.0, None).anchors];
    d.paths.push(a);
    d.paths.push(rect(3, 13, 200.0, 40.0, 80.0, 80.0, Some([0.0, 0.0, 0.0, 1.0])));
    d.ids = 20;
    d.sync_tree();
    d.group(&[2, 3]).unwrap();
    d.clip_group(&[1, 2], 2).unwrap();
    let ops = &page_ops(&export(&d, ExportScope::AllVisibleArtboards))[0];
    let mut starts = clip_ring_starts(ops);
    starts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(starts, vec![[40.0, 260.0], [60.0, 240.0], [200.0, 260.0]], "A outer + A hole + B");
    let w = ops.iter().position(|o| o.operator == "W*").unwrap();
    assert_eq!(ops[w + 1].operator, "n");
    let member = find_op(ops, "m", &[20.0, 280.0]).expect("the member");
    assert!(member > w && clip_state(ops)[member]);
}

#[test]
fn nested_clip_uses_nearest_mask_only() {
    // member 1 is clipped by inner mask 2; that clip group is itself clipped by outer mask 3 (x = 33.375).
    // The canvas supports one level and uses the NEAREST clip; the PDF must do the same.
    let mut d = Document {
        artboards: vec![Artboard { x: 0.0, y: 0.0, w: 400.0, h: 300.0, page_color: None, ..Default::default() }],
        ..Default::default()
    };
    d.paths.push(rect(1, 1, 20.0, 20.0, 200.0, 100.0, Some([0.8, 0.1, 0.5, 1.0])));
    d.paths.push(rect(2, 5, 60.0, 40.0, 80.0, 60.0, Some([0.0, 0.0, 0.0, 1.0])));
    d.paths.push(rect(3, 9, 33.375, 30.0, 300.0, 200.0, Some([0.0, 0.0, 0.0, 1.0])));
    d.ids = 12;
    d.sync_tree();
    let inner = d.clip_group(&[1, 2], 2).unwrap();
    let outer = d.clip_group(&[1, 3], 3).unwrap();
    assert!(inner != outer && d.node(outer).unwrap().children.contains(&inner), "a real nested clip");
    assert_eq!(d.clip_group_of(1), Some(inner));
    let bytes = export(&d, ExportScope::AllVisibleArtboards);
    let ops = &page_ops(&bytes)[0];
    assert_eq!(ops.iter().filter(|o| o.operator == "W*").count(), 1);
    assert_eq!(clip_ring_starts(ops), vec![[60.0, 260.0]], "the inner mask only");
    assert!(!contains(&bytes, "33.375"), "the outer mask ring is not written");
}

#[test]
fn one_clip_scope_per_run_of_members() {
    // three members under ONE mask (one of them a knockout, drawn with `Do`) + a free path in front:
    // the mask is written once, every member paints inside that one scope, the free path outside it
    let mut d = Document {
        artboards: vec![Artboard { x: 0.0, y: 0.0, w: 400.0, h: 300.0, page_color: None, ..Default::default() }],
        ..Default::default()
    };
    d.paths.push(rect(1, 1, 20.0, 20.0, 100.0, 60.0, Some([0.8, 0.1, 0.5, 1.0])));
    d.paths.push(rect(2, 5, 70.0, 50.0, 100.0, 60.0, Some([0.1, 0.8, 0.5, 1.0])));
    let mut k = rect(3, 9, 50.0, 70.0, 80.0, 50.0, Some([0.1, 0.5, 0.8, 1.0]));
    k.stroke = varos_core::model::Paint::from_opt(Some([0.0, 0.0, 0.0, 0.5])); // translucent → knockout
    k.stroke_width = 4.0;
    d.paths.push(k);
    d.paths.push(rect(4, 13, 60.0, 40.0, 80.0, 60.0, Some([0.0, 0.0, 0.0, 1.0]))); // the mask
    d.ids = 20;
    d.sync_tree();
    d.clip_group(&[1, 2, 3, 4], 4).unwrap();
    d.paths.push(rect(5, 21, 300.0, 200.0, 20.0, 20.0, Some([0.9, 0.9, 0.1, 1.0]))); // free, in front
    d.ids = 30;
    d.sync_tree();
    let ops = &page_ops(&export(&d, ExportScope::AllVisibleArtboards))[0];
    let clipped = clip_state(ops);
    assert_eq!(ops.iter().filter(|o| o.operator == "W*").count(), 1, "one clip scope for the whole run");
    let mask_starts = ops.iter().filter(|o| has_op(std::slice::from_ref(*o), "m", &[60.0, 260.0])).count();
    assert_eq!(mask_starts, 1, "the mask ring is written once, not once per member");
    for colour in [[0.8, 0.1, 0.5], [0.1, 0.8, 0.5]] {
        assert!(clipped[find_op(ops, "rg", &colour).expect("member drawn")], "{colour:?} is clipped");
    }
    let do_idx = ops.iter().position(|o| o.operator == "Do").expect("the knockout member");
    assert!(clipped[do_idx], "the knockout XObject paints inside the clip");
    assert!(!clipped[find_op(ops, "rg", &[0.9, 0.9, 0.1]).expect("the free path")], "the free path is unclipped");
}

#[test]
fn artwork_bounds_follow_the_curve_not_its_handles() {
    // an arch from (0,0) to (100,0) with both handles 100 up: the curve peaks at y = −75 (t = ½), the
    // handles reach −100. A 2-pt stroke pads 1. The page hugs the curve, not the handle box.
    let mut doc = Document::default();
    let arch = vec![
        Anchor { id: 1, p: [0.0, 0.0], hin: None, hout: Some([0.0, -100.0]), smooth: false },
        Anchor { id: 2, p: [100.0, 0.0], hin: Some([100.0, -100.0]), hout: None, smooth: false },
    ];
    doc.paths.push(Path::new(1, arch, false, None, Some([0.0, 0.0, 0.0, 1.0]), 2.0));
    doc.ids = 3;
    doc.sync_tree();
    let r = plan(&doc, ExportScope::ArtworkBounds).pages[0].rect;
    assert!(close4(r, [-1.0, -76.0, 102.0, 77.0]), "page {r:?} hugs the curve, not the handles (−101)");
    // and the export's single page has exactly that size
    let mb = media_boxes(&export(&doc, ExportScope::ArtworkBounds));
    assert!(mb.len() == 1 && (mb[0][2] - 102.0).abs() < 1e-3 && (mb[0][3] - 77.0).abs() < 1e-3, "{mb:?}");
}
