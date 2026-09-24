//! S6-A — the native `.vrs` writer stays byte-identical across the write-side refactor, and the pure
//! PDF export (no embedded model, no hidden data) plans and writes the right pages. Everything is
//! inspected through lopdf; no GPU, no window.
//!
//! Run with:  cargo test -p varos-pdf --test export_pdf

use std::path::PathBuf;

use varos_core::model::{Anchor, Artboard, Document, Path, Xform};
use varos_pdf::write_pdf;

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
/// BEFORE the S6-A write-side refactor) by running this test with `VAROS_BLESS_PDF_FIXTURES=1`.
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
