//! A7 live-transform — Stage 1 field + seam (identity plumbing).
//!
//! Proves the two things the foundation must guarantee before any behaviour is wired up (Stage 4):
//!  1. the new `Node.xform` field PERSISTS through a serde round-trip (a manually-set NON-identity
//!     transform survives save/load byte-for-byte), so a future live rotation is durable; and
//!  2. `Document::unit_xform` — THE seam every reader funnels through — returns IDENTITY for a plain,
//!     un-transformed path, so with no op writing a transform the whole app is behaviour-neutral.
//!
//! Rotation is deliberately NOT wired to anything here (that's Stage 4). Pure data + one seam lookup.

use varos_core::model::{Document, Node, NodeKind, Path, Xform};

#[test]
fn xform_field_round_trips_and_unit_xform_defaults_to_identity() {
    // (1) a Node carrying a NON-identity transform survives serde unchanged (the field persists).
    let mut n = Node {
        id: 7,
        kind: NodeKind::Group,
        name: "rotated group".into(),
        parent: None,
        children: vec![],
        hidden: false,
        locked: false,
        color: None,
        clip_exempt: false,
        xform: Xform::default(),
    };
    n.xform = Xform { rot: 0.6, piv: [12.0, -3.5] };
    assert!(!n.xform.is_identity(), "a 0.6 rad rotation is not identity");

    let json = serde_json::to_string(&n).expect("serialize");
    let back: Node = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(n, back, "a node with a non-identity xform round-trips byte-for-byte");
    assert_eq!(back.xform.rot, 0.6, "rotation angle preserved");
    assert_eq!(back.xform.piv, [12.0, -3.5], "pivot preserved");

    // (2) a plain path (its own leaf is the unit) has an IDENTITY transform through the seam — so with
    //     no op writing a transform, `unit_xform` composes to a no-op everywhere.
    let mut doc = Document::default();
    let pid = 100;
    doc.paths.push(Path::new(
        pid,
        vec![
            varos_core::model::Anchor { id: 1, p: [0.0, 0.0], hin: None, hout: None, smooth: false },
            varos_core::model::Anchor { id: 2, p: [10.0, 0.0], hin: None, hout: None, smooth: false },
            varos_core::model::Anchor { id: 3, p: [10.0, 10.0], hin: None, hout: None, smooth: false },
        ],
        true,
        Some([0.5, 0.5, 0.5, 1.0]),
        None,
        1.0,
    ));
    doc.ids = 100;
    doc.sync_tree(); // wrap the loose path into a leaf node under the active layer

    let xf = doc.unit_xform(pid);
    assert!(xf.is_identity(), "a plain path's unit transform is identity");
    // and `apply`/`inverse_apply` are exact no-ops at identity (byte-for-byte)
    let p = [3.25, -7.75];
    assert_eq!(xf.apply(p), p, "identity apply returns the point untouched");
    assert_eq!(xf.inverse_apply(p), p, "identity inverse_apply returns the point untouched");
}
