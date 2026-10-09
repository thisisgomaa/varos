//! Integration w3 (2026-10-10): cross-lane contracts that no single wave-3 lane could test alone.
//! Evaluation order on a derived path: live → effects (→ corners → stroke → appearance stack).
use std::sync::Arc;
use varos_core::{
    effects::Effect,
    live::{self, Action, Axis, Kind, Repeat},
    model::{Anchor, NodeKind, Path},
    stroke::StrokeJoin,
    EditCommand, Editor,
};

fn rectangle(ed: &mut Editor, x: f32) -> u32 {
    let id = ed.doc.nid();
    let mut anchors = Vec::new();
    for p in [[x, 0.], [x + 10., 0.], [x + 10., 10.], [x, 10.]] {
        anchors.push(Anchor { id: ed.doc.nid(), p, hin: None, hout: None, smooth: false });
    }
    ed.doc.paths.push(Path::new(id, anchors, true, Some([1., 0., 0., 1.]), None, 0.));
    ed.doc.sync_tree();
    id
}

fn evaluated(ed: &Editor, node: u32) -> Arc<Vec<Path>> {
    let n = ed.doc.node(node).unwrap();
    let NodeKind::Live(kind) = n.kind else { panic!("live node") };
    ed.live_cache.evaluate(&ed.doc, n, kind).unwrap()
}

#[test]
fn live_sources_keep_live_effects_and_effects_evaluate_after_the_live_node() {
    let mut ed = Editor::new();
    let a = rectangle(&mut ed, 0.);
    let offset = Effect::Offset { delta: 2., join: StrokeJoin::Miter, miter: 4. };
    let i = ed.doc.pidx(a).unwrap();
    ed.doc.paths[i].effects = vec![offset.clone()];
    ed.try_execute(EditCommand::Live(Action::Make {
        paths: vec![a],
        kind: Kind::Repeat { repeat: Repeat::Mirror { axis: Axis::Vertical } },
    }))
    .unwrap();
    let node = live::selected_node(&ed).unwrap();
    // Make never bakes a live effect into its source (it stays editable) ...
    let source = ed.doc.node(node).unwrap().children.iter().find_map(|c| match ed.doc.node(*c)?.kind {
        NodeKind::Path(pid) => ed.doc.paths.iter().find(|p| p.id == pid),
        _ => None,
    });
    assert_eq!(source.unwrap().effects, vec![offset.clone()]);
    // ... and every derived copy carries it, so the shared effects pass runs on the live result.
    let derived = evaluated(&ed, node);
    assert!(derived.len() >= 2);
    assert!(derived.iter().all(|p| p.effects == vec![offset.clone()]));
    let resolved = varos_core::effects::evaluated(&derived[0]);
    let xs: Vec<f32> = resolved.anchors.iter().map(|a| a.p[0]).collect();
    let min = xs.iter().copied().fold(f32::MAX, f32::min);
    assert!(min < derived[0].anchors.iter().map(|a| a.p[0]).fold(f32::MAX, f32::min), "offset grew the copy");
}
