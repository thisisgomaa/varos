//! Foundations Stage-0 spine: the document model survives a serde round-trip unchanged.
//! This is the guarantee every later system leans on — Color/Stroke/Transform/Layers/Artboard
//! all assume "it gets saved to the .varos schema". Here we prove the model itself round-trips;
//! the container (zip/OPC) + dialogs are the Save system (§4) on top of this.
//!
//! Pure data — no GPU, no UI. JSON is the readable test format; the on-disk .varos format is
//! chosen in §4 (it just needs the same serde derives).
//!
//! Run with:  cargo test -p varos-core --test serde_roundtrip

use std::collections::HashMap;
use varos_core::model::{Anchor, Artboard, Document, Group, Path, SnapConfig};
use varos_core::units::{DocUnits, Unit};

fn anc(id: u32, p: [f32; 2], hin: Option<[f32; 2]>, hout: Option<[f32; 2]>, smooth: bool) -> Anchor {
    Anchor { id, p, hin, hout, smooth }
}

/// A document that exercises every persisted field: a closed compound path (outer ring + a hole),
/// bezier handles, fill + stroke + stroke_width + opacity + name + locked, plus an open path with
/// no fill, two paths nested in one group, and a non-trivial id counter.
fn sample_doc() -> Document {
    // Compound path: outer rectangle-ish ring with one curved corner, plus a triangular hole.
    let outer = vec![
        anc(1, [0.0, 0.0], None, None, false),
        anc(2, [100.0, 0.0], None, Some([110.0, 10.0]), true),
        anc(3, [100.0, 80.0], Some([105.0, 70.0]), None, true),
        anc(4, [0.0, 80.0], None, None, false),
    ];
    let hole = vec![
        anc(5, [20.0, 20.0], None, None, false),
        anc(6, [40.0, 20.0], None, None, false),
        anc(7, [40.0, 40.0], None, None, false),
    ];
    let mut body = Path::new(10, outer, true, Some([0.05, 0.55, 0.91, 1.0]), Some([0.1, 0.1, 0.1, 1.0]), 2.5);
    body.holes = vec![hole];
    body.opacity = 0.8;
    body.name = Some("Logo body".to_string());
    body.locked = true;

    // Open stroke-only path with handles.
    let mark = Path::new(
        11,
        vec![
            anc(8, [200.0, 10.0], Some([190.0, 5.0]), Some([210.0, 15.0]), true),
            anc(9, [260.0, 60.0], None, None, false),
        ],
        false,
        None,
        Some([1.0, 0.0, 0.0, 1.0]),
        1.0,
    );

    // Both paths in one group.
    let mut group_of = HashMap::new();
    group_of.insert(10u32, 100u32);
    group_of.insert(11u32, 100u32);

    Document {
        paths: vec![body, mark],
        groups: vec![Group { id: 100, name: "Logo".to_string(), parent: None }],
        group_of,
        ids: 100,
        // non-default units + a MULTI-artboard set, so the round-trip exercises every new field:
        // a square white page with bleed + clip, and a transparent page (page_color = None).
        units: DocUnits { ppi: 96.0, display: Unit::Mm },
        artboards: vec![
            Artboard { x: 0.0, y: 0.0, w: 800.0, h: 600.0, name: "Page".to_string(),
                       bleed: 9.0, page_color: Some([1.0, 1.0, 1.0, 1.0]), clip: true },
            Artboard { x: 900.0, y: 0.0, w: 1080.0, h: 1080.0, name: "Logo".to_string(),
                       bleed: 0.0, page_color: None, clip: false },
        ],
        active: 1,
        move_art_with_ab: false,
        // non-default snap config so the round-trip exercises the new fields
        snap: SnapConfig { smart: false, radius_px: 6.0, candidate_max: 5, grid: true, path_intersections: false, ..SnapConfig::default() },
        ruler_origin: [12.0, 34.0],   // non-default ruler zero-point exercises the new field
    }
}

/// The core guarantee: serialize → deserialize yields an identical document (every field, every
/// anchor handle, holes, group membership, ids). Equality is content-based (Document: PartialEq),
/// so HashMap ordering does not matter.
#[test]
fn document_json_round_trips() {
    let doc = sample_doc();
    let json = serde_json::to_string_pretty(&doc).expect("serialize");
    let back: Document = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(doc, back, "the document model must survive a JSON round-trip unchanged");
}

/// An empty/default document round-trips too (the new-document baseline).
#[test]
fn empty_document_round_trips() {
    let doc = Document::default();
    let json = serde_json::to_string(&doc).expect("serialize");
    let back: Document = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(doc, back);
}

/// f32 fields survive exactly (serde_json uses ryu, which round-trips every f32 to itself), so the
/// content-equality above is real equality, not approximate. Guards against a future format swap
/// silently lossy-ing coordinates.
#[test]
fn float_fields_are_exact() {
    let doc = sample_doc();
    let back: Document = serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
    let (a, b) = (&doc.paths[0], &back.paths[0]);
    assert_eq!(a.stroke_width, b.stroke_width);
    assert_eq!(a.opacity, b.opacity);
    assert_eq!(a.anchors[1].hout, b.anchors[1].hout);
    assert_eq!(a.fill, b.fill);
}
