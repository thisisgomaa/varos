//! Headless tests: documents are built in memory, no window, renderer or engine.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;
use vectorcraft_doc::{Appearance, Artboard, Document, LayerColor, Node, NodeId};
use vectorcraft_geom::{Rect, shapes};

use crate::activity::{Activity, Entry, Since};
use crate::actor::{ActorKey, DEFAULT_LABEL, MAX_LABEL, Registry, sanitize_label};
use crate::diff::{Change, diff};
use crate::presence::{FADE, FINISH, HOLD, Presence, REVEAL_CAP, STAGGER, repaint_after, target_artboard};
use crate::receipts::{Admission, ReceiptError, Receipts};

// ---------------------------------------------------------------- fixture

struct Fx {
    doc: Arc<Document>,
    layer: NodeId,
    r1: NodeId,
    r2: NodeId,
    g: NodeId,
    c1: NodeId,
    c2: NodeId,
}

fn rect(id: NodeId, x: f64, y: f64) -> Arc<Node> {
    Arc::new(Node::path(id, shapes::rectangle(Rect::new(x, y, x + 10.0, y + 10.0)), Appearance::default_art()))
}

/// Artboard 1 at (0,0)-(100,100) and artboard 2 at (200,0)-(300,100); one layer holding r1, r2 and
/// a group g of c1 and c2, all on artboard 1.
fn fixture() -> Fx {
    let mut d = Document::new(100.0, 100.0);
    d.artboards.push(Artboard {
        id: 2,
        name: "Artboard 2".into(),
        rect: Rect::new(200.0, 0.0, 300.0, 100.0),
        show_center_mark: false,
        show_cross_hairs: false,
    });
    let layer = d.alloc_id();
    let r1 = d.alloc_id();
    let r2 = d.alloc_id();
    let g = d.alloc_id();
    let c1 = d.alloc_id();
    let c2 = d.alloc_id();
    let mut l = Node::layer(layer, "Layer 1", LayerColor::Preset(0));
    if let Some(ch) = l.children_mut() {
        ch.push(rect(r1, 10.0, 10.0));
        ch.push(rect(r2, 30.0, 30.0));
        ch.push(Arc::new(Node::group(g, vec![rect(c1, 50.0, 50.0), rect(c2, 70.0, 70.0)])));
    }
    // A new document starts with a layer of its own; the fixture's layer is the only one.
    d.layers = vec![Arc::new(l)];
    Fx { doc: Arc::new(d), layer, r1, r2, g, c1, c2 }
}

/// A copy of `d` changed by `f`, sharing everything `f` leaves alone (as an engine edit does).
fn edit(d: &Arc<Document>, f: impl FnOnce(&mut Document)) -> Arc<Document> {
    let mut next = d.clone();
    f(Arc::make_mut(&mut next));
    next
}

fn layer_children(d: &mut Document) -> &mut Vec<Arc<Node>> {
    Arc::make_mut(&mut d.layers[0]).children_mut().unwrap()
}

/// The children of the layer's (only) group, wherever it now is in the layer.
fn group_children(d: &mut Document) -> &mut Vec<Arc<Node>> {
    let g = layer_children(d).iter_mut().find(|n| n.children().is_some()).unwrap();
    Arc::make_mut(g).children_mut().unwrap()
}

// ---------------------------------------------------------------- diff

#[test]
fn diff_of_a_shared_copy_is_empty() {
    let fx = fixture();
    let copy = Arc::new((*fx.doc).clone());
    assert!(diff(&fx.doc, &copy).is_empty());
}

#[test]
fn diff_lists_created_objects_in_paint_order() {
    let mut fx = fixture();
    let (a, b) = {
        let d = Arc::make_mut(&mut fx.doc);
        (d.alloc_id(), d.alloc_id())
    };
    let after = edit(&fx.doc, |d| {
        layer_children(d).push(rect(a, 0.0, 0.0));
        layer_children(d).push(rect(b, 5.0, 5.0));
    });
    let c = diff(&fx.doc, &after);
    assert_eq!(c.created, vec![a, b]);
    assert!(c.changed.is_empty(), "the layer itself did not change: {c:?}");
    assert!(c.removed.is_empty());
}

#[test]
fn diff_reports_only_the_changed_leaf_not_its_ancestors() {
    let fx = fixture();
    let after = edit(&fx.doc, |d| Arc::make_mut(&mut group_children(d)[1]).opacity = 0.5);
    let c = diff(&fx.doc, &after);
    assert_eq!(c.changed, vec![fx.c2]);
    assert!(c.created.is_empty() && c.removed.is_empty());
}

#[test]
fn diff_ignores_a_new_arc_with_the_same_content() {
    let fx = fixture();
    let after = edit(&fx.doc, |d| {
        let same = (*layer_children(d)[0]).clone();
        layer_children(d)[0] = Arc::new(same);
    });
    assert!(diff(&fx.doc, &after).is_empty());
}

#[test]
fn diff_reports_a_renamed_group_but_not_for_a_child_edit() {
    let fx = fixture();
    let renamed = edit(&fx.doc, |d| Arc::make_mut(&mut layer_children(d)[2]).name = Some("Logo".into()));
    assert_eq!(diff(&fx.doc, &renamed).changed, vec![fx.g]);
}

#[test]
fn diff_removes_a_whole_subtree() {
    let fx = fixture();
    let after = edit(&fx.doc, |d| {
        layer_children(d).remove(2);
    });
    let c = diff(&fx.doc, &after);
    assert_eq!(c.removed, vec![fx.g, fx.c1, fx.c2]);
    assert!(c.created.is_empty() && c.changed.is_empty());
}

#[test]
fn diff_counts_a_move_to_another_parent_as_changed() {
    let fx = fixture();
    let after = edit(&fx.doc, |d| {
        let r1 = layer_children(d).remove(0);
        group_children(d).push(r1);
    });
    let c = diff(&fx.doc, &after);
    assert_eq!(c.changed, vec![fx.r1]);
    assert!(c.created.is_empty() && c.removed.is_empty(), "{c:?}");
}

#[test]
fn diff_reorder_reports_the_restacked_parent_not_its_objects() {
    // Z-order is the parent's list order; the objects themselves are untouched, so presence has
    // nothing to show, but the change is not empty: the activity log sees the restack.
    let fx = fixture();
    let after = edit(&fx.doc, |d| layer_children(d).swap(0, 1));
    let c = diff(&fx.doc, &after);
    assert!(c.created.is_empty() && c.removed.is_empty() && c.changed.is_empty(), "{c:?}");
    assert_eq!(c.reordered, vec![fx.layer]);
    assert!(!c.layers_reordered);
    assert!(!c.is_empty());
    assert!(c.touched().is_empty(), "no object lights up");
    // Inside a group, and Bring to Front of the group's bottom child.
    let after = edit(&fx.doc, |d| {
        let ch = group_children(d);
        let c1 = ch.remove(0);
        ch.push(c1);
    });
    assert_eq!(diff(&fx.doc, &after).reordered, vec![fx.g]);
    // A child removed or added leaves the others' order alone: no restack.
    let after = edit(&fx.doc, |d| {
        layer_children(d).remove(1);
    });
    assert!(diff(&fx.doc, &after).reordered.is_empty());
}

#[test]
fn diff_reports_the_layers_own_order() {
    let mut fx = fixture();
    let second = Arc::make_mut(&mut fx.doc).alloc_id();
    let two_layers = edit(&fx.doc, |d| d.layers.push(Arc::new(Node::layer(second, "Layer 2", LayerColor::Preset(1)))));
    let swapped = edit(&two_layers, |d| d.layers.swap(0, 1));
    let c = diff(&two_layers, &swapped);
    assert!(c.layers_reordered && c.reordered.is_empty(), "{c:?}");
    assert_eq!(diff(&fx.doc, &two_layers).created, vec![second]);
}

/// The owner's compound path of ~13,900 parts: one part edited, one added, one removed or two
/// swapped costs a handful of node visits (compared or indexed by id), not thousands. The pointer
/// comparisons over the siblings are still one per part (O(siblings)); they are not counted here.
/// Counted, not timed, so the test is the same on any machine.
#[test]
fn diff_visits_a_handful_of_nodes_on_a_long_list() {
    const N: usize = 13_900;
    let mut d = Document::new(100.0, 100.0);
    let layer = d.alloc_id();
    let compound = d.alloc_id();
    let parts: Vec<Arc<Node>> = (0..N).map(|i| rect(d.alloc_id(), i as f64, 0.0)).collect();
    let ids: Vec<NodeId> = parts.iter().map(|p| p.id).collect();
    let mut l = Node::layer(layer, "Layer 1", LayerColor::Preset(0));
    let mut c = Node::group(compound, parts);
    c.kind = vectorcraft_doc::NodeKind::Compound { children: std::mem::take(c.children_mut().unwrap()), rule: Default::default() };
    l.children_mut().unwrap().push(Arc::new(c));
    d.layers = vec![Arc::new(l)];
    let doc = Arc::new(d);
    fn parts_of(d: &mut Document) -> &mut Vec<Arc<Node>> {
        let c = Arc::make_mut(&mut d.layers[0]).children_mut().unwrap();
        Arc::make_mut(&mut c[0]).children_mut().unwrap()
    }
    let mut doc2 = doc.clone();
    let extra = Arc::make_mut(&mut doc2).alloc_id();
    let cases: Vec<(&str, Arc<Document>)> = vec![
        ("edit one part", edit(&doc, |d| Arc::make_mut(&mut parts_of(d)[N / 2]).visible = false)),
        ("add one part", edit(&doc2, |d| parts_of(d).insert(N / 3, rect(extra, -5.0, 0.0)))),
        (
            "remove one part",
            edit(&doc, |d| {
                parts_of(d).remove(N / 4);
            }),
        ),
        ("swap two neighbours", edit(&doc, |d| parts_of(d).swap(N / 2, N / 2 + 1))),
    ];
    for (what, after) in cases {
        let before = if what == "add one part" { &doc2 } else { &doc };
        let (change, visited) = crate::diff_counted(before, &after);
        assert!(visited <= 8, "{what}: {visited} nodes visited");
        match what {
            "edit one part" => assert_eq!(change.changed, vec![ids[N / 2]]),
            "add one part" => assert_eq!(change.created, vec![extra]),
            "remove one part" => assert_eq!(change.removed, vec![ids[N / 4]]),
            _ => assert_eq!(change.reordered, vec![compound]),
        }
    }
    // Everything replaced (every part a new Arc, the same ids) is still matched pairwise: no index.
    let all = edit(&doc, |d| {
        for p in parts_of(d).iter_mut() {
            Arc::make_mut(p).locked = true;
        }
    });
    let (change, visited) = crate::diff_counted(&doc, &all);
    assert_eq!(change.changed.len(), N);
    assert!(visited <= N + 4, "{visited}");
}

#[test]
fn diff_grouping_creates_the_group_and_moves_its_members() {
    let mut fx = fixture();
    let ng = Arc::make_mut(&mut fx.doc).alloc_id();
    let after = edit(&fx.doc, |d| {
        let ch = layer_children(d);
        let r1 = ch.remove(0);
        let r2 = ch.remove(0);
        ch.insert(0, Arc::new(Node::group(ng, vec![r1, r2])));
    });
    let c = diff(&fx.doc, &after);
    assert_eq!(c.created, vec![ng]);
    assert_eq!(c.changed, vec![fx.r1, fx.r2]);
    assert!(c.removed.is_empty());
    assert_eq!(c.touched(), vec![ng, fx.r1, fx.r2]);
}

#[test]
fn diff_reports_artboards() {
    let fx = fixture();
    let after = edit(&fx.doc, |d| {
        d.artboards[0].name = "Cover".into();
        d.artboards.remove(1);
        d.artboards.push(Artboard {
            id: 3,
            name: "Story".into(),
            rect: Rect::new(0.0, 200.0, 50.0, 300.0),
            show_center_mark: false,
            show_cross_hairs: false,
        });
    });
    let c = diff(&fx.doc, &after);
    assert_eq!(c.artboards_changed, vec![1]);
    assert_eq!(c.artboards_removed, vec![2]);
    assert_eq!(c.artboards_created, vec![3]);
    assert_eq!(c.artboards_touched(), vec![3, 1]);
}

// ---------------------------------------------------------------- actor

#[test]
fn labels_lose_control_and_bidi_characters_and_stay_short() {
    assert_eq!(sanitize_label("Claude\u{202e} Code\n"), "Claude Code");
    assert_eq!(sanitize_label("\u{200b}\u{2066}  "), DEFAULT_LABEL);
    assert_eq!(sanitize_label(&"x".repeat(100)).chars().count(), MAX_LABEL);
    assert_eq!(sanitize_label("وكيل"), "وكيل");
}

#[test]
fn a_second_session_with_the_same_label_is_numbered() {
    let now = Instant::now();
    let mut r = Registry::new();
    let a = r.admit("s-1", "claude-code", "Claude Code", now);
    let b = r.admit("s-2", "claude-code", "Claude Code", now);
    let c = r.admit("s-3", "codex", "Codex", now);
    assert_ne!(a, b);
    assert_eq!(r.label(a), "Claude Code");
    assert_eq!(r.label(b), "Claude Code 2");
    assert_eq!(r.label(c), "Codex");
    // The same session (a reconnect) keeps its key and label.
    assert_eq!(r.admit("s-2", "claude-code", "Claude Code", now), b);
    // An idle session is forgotten and its number is free again.
    r.admit("s-2", "claude-code", "Claude Code", now + Duration::from_secs(100));
    r.forget_idle(now + Duration::from_secs(100), Duration::from_secs(60));
    assert!(r.get(a).is_none() && r.get(c).is_none());
    let d = r.admit("s-4", "claude-code", "Claude Code", now + Duration::from_secs(101));
    assert_eq!(r.label(d), "Claude Code");
    assert_eq!(r.label(b), "Claude Code 2");
}

/// A label is numbered only while another session with it is connected: after both end, the
/// next one is plain again; one that reconnects keeps its key, renumbered if its number was taken.
#[test]
fn numbers_are_reused_once_sessions_end() {
    let now = Instant::now();
    let mut r = Registry::new();
    let a = r.admit("s-1", "claude-code", "Claude Code", now);
    let b = r.admit("s-2", "claude-code", "Claude Code", now);
    assert_eq!((r.label(a), r.label(b)), ("Claude Code".to_string(), "Claude Code 2".to_string()));
    // Both end (their connections close).
    r.depart("s-1");
    r.depart("s-2");
    let c = r.admit("s-3", "claude-code", "Claude Code", now);
    assert_eq!(r.label(c), "Claude Code", "plain again after both ended");
    // One after another, never numbered (the v7 live test numbered them up to 7).
    for k in 4..10 {
        r.depart(&format!("s-{}", k - 1));
        let s = r.admit(&format!("s-{k}"), "claude-code", "Claude Code", now);
        assert_eq!(r.label(s), "Claude Code", "session {k}");
    }
    // Two at once while the others have ended.
    let x = r.admit("x", "claude-code", "Claude Code", now);
    assert_eq!(r.label(x), "Claude Code 2");
    // The first reconnects: same key; its number 1 is taken by s-9, so it is renumbered.
    assert_eq!(r.admit("s-1", "claude-code", "Claude Code", now), a);
    assert_eq!(r.label(a), "Claude Code 3");
    // A reconnect whose number is free keeps it.
    r.depart("x");
    assert_eq!(r.admit("x", "claude-code", "Claude Code", now), x);
    assert_eq!(r.label(x), "Claude Code 2");
    // Other labels are numbered apart.
    let codex = r.admit("c", "codex", "Codex", now);
    assert_eq!(r.label(codex), "Codex");
    // An unknown session departing changes nothing.
    r.depart("nobody");
    assert_eq!(r.len(), 11);
}

#[test]
fn profiles_are_reduced_to_a_safe_id() {
    let mut r = Registry::new();
    let a = r.admit("s", "../../etc/passwd\u{0}claude", "x", Instant::now());
    assert_eq!(r.get(a).unwrap().profile, "....etcpasswdclaude");
}

// ---------------------------------------------------------------- presence

const BOARD: u64 = 7;
const AGENT: ActorKey = ActorKey(1);
const OTHER: ActorKey = ActorKey(2);

fn change(created: &[NodeId], changed: &[NodeId], removed: &[NodeId]) -> Change {
    Change { created: created.to_vec(), changed: changed.to_vec(), removed: removed.to_vec(), ..Change::default() }
}

fn none() -> HashSet<NodeId> {
    HashSet::new()
}

#[test]
fn nothing_shows_without_agents() {
    let fx = fixture();
    let mut p = Presence::new();
    assert!(!p.is_active());
    assert!(p.frame(BOARD, 1, &fx.doc, &none(), Instant::now()).is_empty());
}

#[test]
fn the_artboard_lights_up_while_the_request_runs() {
    let fx = fixture();
    let now = Instant::now();
    let mut p = Presence::new();
    let _t = p.begin(AGENT, "Claude Code", "claude-code", BOARD, Some(1), now);
    let f = p.frame(BOARD, 1, &fx.doc, &none(), now);
    assert_eq!(f.pages.len(), 1);
    assert_eq!(f.pages[0].artboard, 1);
    assert_eq!(f.pages[0].labels, vec!["Claude Code".to_string()]);
    assert_eq!(f.pages[0].rect, Rect::new(0.0, 0.0, 100.0, 100.0));
    assert!(f.objects.is_empty());
    assert_eq!(f.expiry_after, None, "an outline doesn't expire while its request runs");
    // Another document shows nothing.
    assert!(p.frame(BOARD + 1, 1, &fx.doc, &none(), now).is_empty());
}

#[test]
fn objects_appear_one_by_one_then_fade() {
    let fx = fixture();
    let now = Instant::now();
    let mut p = Presence::new();
    let t = p.begin(AGENT, "Claude Code", "", BOARD, Some(1), now);
    p.finish(t, &change(&[fx.r1, fx.r2], &[fx.c1], &[]), Some(1), now);
    let f = p.frame(BOARD, 2, &fx.doc, &none(), now);
    assert_eq!(f.objects.len(), 1, "only the first has appeared: {f:?}");
    assert_eq!(f.objects[0].id, fx.r1);
    assert_eq!(f.objects[0].alpha, 1.0);
    let delays: Vec<Duration> = f.pending.iter().map(|(_, d)| *d).collect();
    assert_eq!(delays, vec![STAGGER, STAGGER * 2]);
    assert_eq!(f.repaint_after, Some(Duration::ZERO));
    // Halfway through the first fade, all three are out.
    let mid = now + FADE / 2;
    let f = p.frame(BOARD, 2, &fx.doc, &none(), mid);
    assert_eq!(f.objects.iter().map(|o| o.id).collect::<Vec<_>>(), vec![fx.r1, fx.r2, fx.c1]);
    assert!((f.objects[0].alpha - 0.5).abs() < 0.01);
    // Every fade is over; the artboard outline stays until HOLD.
    let later = now + STAGGER * 2 + FADE + Duration::from_millis(1);
    let f = p.frame(BOARD, 2, &fx.doc, &none(), later);
    assert!(f.objects.is_empty() && f.pending.is_empty());
    assert_eq!(f.pages.len(), 1);
    assert_eq!(f.expiry_after, Some(HOLD - (later - now)));
    assert_eq!(f.repaint_after, f.expiry_after);
    // After HOLD, nothing.
    p.retain(|_| true, now + HOLD);
    assert!(!p.is_active());
}

#[test]
fn a_large_batch_appears_within_the_cap() {
    let mut fx = fixture();
    let ids: Vec<NodeId> = (0..300).map(|_| Arc::make_mut(&mut fx.doc).alloc_id()).collect();
    let doc = edit(&fx.doc, |d| {
        for (i, id) in ids.iter().enumerate() {
            layer_children(d).push(rect(*id, (i % 9) as f64 * 10.0, (i / 9 % 9) as f64 * 10.0));
        }
    });
    let now = Instant::now();
    let mut p = Presence::new();
    let t = p.begin(AGENT, "A", "", BOARD, Some(1), now);
    p.finish(t, &change(&ids, &[], &[]), Some(1), now);
    let f = p.frame(BOARD, 2, &doc, &none(), now);
    let last = f.pending.iter().map(|(_, d)| *d).max().unwrap();
    assert!(last < REVEAL_CAP, "{last:?}");
    assert!(last > REVEAL_CAP - Duration::from_millis(10), "{last:?}");
}

#[test]
fn a_failed_request_leaves_no_trace() {
    let fx = fixture();
    let now = Instant::now();
    let mut p = Presence::new();
    let t = p.begin(AGENT, "A", "", BOARD, Some(1), now);
    p.abort(t);
    assert!(!p.is_active());
    // With earlier work showing, a failed request puts that back as it was.
    let t = p.begin(AGENT, "A", "", BOARD, Some(1), now);
    p.finish(t, &change(&[fx.r1], &[], &[]), Some(1), now);
    let t = p.begin(AGENT, "A", "", BOARD, Some(2), now + Duration::from_millis(10));
    p.abort(t);
    let f = p.frame(BOARD, 2, &fx.doc, &none(), now + Duration::from_millis(20));
    assert_eq!(f.pages[0].artboard, 1);
    assert_eq!(f.objects.len(), 1);
}

#[test]
fn new_work_finishes_the_old_fades_quickly() {
    let fx = fixture();
    let now = Instant::now();
    let mut p = Presence::new();
    let t = p.begin(AGENT, "A", "", BOARD, Some(1), now);
    p.finish(t, &change(&[fx.r1], &[], &[]), Some(1), now);
    let next = now + Duration::from_millis(100);
    let _t = p.begin(AGENT, "A", "", BOARD, Some(1), next);
    let f = p.frame(BOARD, 2, &fx.doc, &none(), next + FINISH + Duration::from_millis(1));
    assert!(f.objects.is_empty(), "{f:?}");
}

#[test]
fn removed_objects_stop_showing_for_every_agent() {
    let fx = fixture();
    let now = Instant::now();
    let mut p = Presence::new();
    let t = p.begin(OTHER, "Codex", "", BOARD, Some(1), now);
    p.finish(t, &change(&[fx.r1], &[], &[]), Some(1), now);
    let t = p.begin(AGENT, "Claude Code", "", BOARD, Some(1), now);
    p.finish(t, &change(&[], &[], &[fx.r1]), Some(1), now);
    let f = p.frame(BOARD, 3, &fx.doc, &none(), now);
    assert!(f.objects.is_empty(), "{f:?}");
}

#[test]
fn the_persons_selection_wins_over_orange() {
    let fx = fixture();
    let now = Instant::now();
    let mut p = Presence::new();
    let t = p.begin(AGENT, "A", "", BOARD, Some(1), now);
    p.finish(t, &change(&[fx.r1, fx.c1, fx.g], &[], &[]), Some(1), now);
    let later = now + Duration::from_millis(100);
    // r1 itself selected; c1 inside the selected group g; g with a selected member c1.
    let f = p.frame(BOARD, 2, &fx.doc, &HashSet::from([fx.r1, fx.g]), later);
    assert!(f.objects.is_empty(), "{f:?}");
    let f = p.frame(BOARD, 2, &fx.doc, &HashSet::from([fx.c1]), later);
    assert!(f.objects.iter().all(|o| o.id != fx.g && o.id != fx.c1), "{f:?}");
    assert!(f.objects.iter().any(|o| o.id == fx.r1));
}

#[test]
fn hidden_objects_and_layers_get_no_outline() {
    let fx = fixture();
    let doc = edit(&fx.doc, |d| Arc::make_mut(&mut layer_children(d)[2]).visible = false);
    let now = Instant::now();
    let mut p = Presence::new();
    let t = p.begin(AGENT, "A", "", BOARD, Some(1), now);
    p.finish(t, &change(&[fx.layer, fx.c1], &[fx.r2], &[]), Some(1), now);
    let f = p.frame(BOARD, 2, &doc, &none(), now + Duration::from_millis(100));
    assert_eq!(f.objects.iter().map(|o| o.id).collect::<Vec<_>>(), vec![fx.r2]);
}

#[test]
fn two_agents_on_one_artboard_share_one_outline() {
    let fx = fixture();
    let now = Instant::now();
    let mut p = Presence::new();
    let _a = p.begin(OTHER, "Codex", "b-profile", BOARD, Some(1), now);
    let _b = p.begin(AGENT, "Claude Code", "a-profile", BOARD, Some(1), now);
    let f = p.frame(BOARD, 1, &fx.doc, &none(), now);
    assert_eq!(f.pages.len(), 1);
    assert_eq!(f.pages[0].labels, vec!["Claude Code".to_string(), "Codex".to_string()]);
}

#[test]
fn closing_the_document_drops_its_presence() {
    let fx = fixture();
    let now = Instant::now();
    let mut p = Presence::new();
    let t = p.begin(AGENT, "A", "", BOARD, Some(1), now);
    p.finish(t, &change(&[fx.r1], &[], &[]), Some(1), now);
    p.retain(|b| b != BOARD, now);
    assert!(!p.is_active());
}

#[test]
fn a_deleted_artboard_is_not_outlined() {
    let fx = fixture();
    let doc = edit(&fx.doc, |d| {
        d.artboards.remove(1);
    });
    let now = Instant::now();
    let mut p = Presence::new();
    let _t = p.begin(AGENT, "A", "", BOARD, Some(2), now);
    assert!(p.frame(BOARD, 2, &doc, &none(), now).pages.is_empty());
}

#[test]
fn off_screen_work_does_not_keep_drawing() {
    let fx = fixture();
    let now = Instant::now();
    let mut p = Presence::new();
    let t = p.begin(AGENT, "A", "", BOARD, Some(1), now);
    p.finish(t, &change(&[fx.r1, fx.r2], &[], &[]), Some(1), now);
    let f = p.frame(BOARD, 2, &fx.doc, &none(), now);
    assert_eq!(repaint_after(&f, |_| false), None);
    // Only the pending r2 on screen: wake when it appears.
    let r2 = f.pending[0].0;
    assert_eq!(repaint_after(&f, |r| r == r2), Some(STAGGER));
}

#[test]
fn the_target_artboard_follows_the_work() {
    let fx = fixture();
    let none_changed = Change::default();
    assert_eq!(target_artboard(&fx.doc, &none_changed, Some(2), Some(1)), Some(2), "explicit wins");
    assert_eq!(target_artboard(&fx.doc, &none_changed, Some(99), Some(1)), Some(1), "unknown explicit falls back");
    let boards = Change { artboards_changed: vec![2], ..Change::default() };
    assert_eq!(target_artboard(&fx.doc, &boards, None, Some(1)), Some(2));
    assert_eq!(target_artboard(&fx.doc, &change(&[fx.r1], &[], &[]), None, Some(2)), Some(1), "where the art is");
    // Art off every artboard: the view's artboard.
    let mut fx2 = fixture();
    let far = Arc::make_mut(&mut fx2.doc).alloc_id();
    let doc = edit(&fx2.doc, |d| layer_children(d).push(rect(far, 500.0, 500.0)));
    assert_eq!(target_artboard(&doc, &change(&[far], &[], &[]), None, Some(2)), Some(2));
    assert_eq!(target_artboard(&doc, &change(&[far], &[], &[]), None, None), None);
}

#[test]
fn diff_then_presence_end_to_end() {
    // What the desktop glue does per agent request: diff, target, begin/finish, frame.
    let mut fx = fixture();
    let new = Arc::make_mut(&mut fx.doc).alloc_id();
    let after = edit(&fx.doc, |d| {
        let ch = layer_children(d);
        ch.push(Arc::new(Node::path(new, shapes::rectangle(Rect::new(220.0, 20.0, 240.0, 40.0)), Appearance::default_art())));
    });
    let now = Instant::now();
    let mut p = Presence::new();
    let t = p.begin(AGENT, "Claude Code", "", BOARD, Some(1), now);
    let c = diff(&fx.doc, &after);
    let target = target_artboard(&after, &c, None, Some(1));
    assert_eq!(target, Some(2));
    p.finish(t, &c, target, now);
    let f = p.frame(BOARD, 2, &after, &none(), now);
    assert_eq!(f.pages[0].artboard, 2);
    assert_eq!(f.objects.len(), 1);
    assert_eq!(f.objects[0].id, new);
    assert!(f.objects[0].rect.x0 <= 220.0 && f.objects[0].rect.x1 >= 240.0);
}

// ---------------------------------------------------------------- activity

fn entry(from: u64, to: u64, actor: Option<ActorKey>, c: Change) -> Entry {
    Entry { from, to, actor, label: "A".into(), step: "Edit".into(), change: c, at: Instant::now() }
}

#[test]
fn since_the_current_revision_is_empty() {
    let a = Activity::new();
    assert_eq!(a.since(BOARD, 5, 5), Since::Changes(Change::default()));
    assert_eq!(a.since(BOARD, 4, 5), Since::ResyncRequired);
    assert_eq!(a.since(BOARD, 6, 5), Since::ResyncRequired);
}

#[test]
fn since_folds_the_changes_in_between() {
    let (x, y, z) = (NodeId(1), NodeId(2), NodeId(3));
    let mut a = Activity::new();
    a.record(BOARD, entry(1, 2, Some(AGENT), change(&[x, y], &[], &[])));
    a.record(BOARD, entry(2, 3, None, change(&[], &[x, z], &[])));
    a.record(BOARD, entry(3, 4, Some(AGENT), change(&[], &[], &[y, z])));
    let Since::Changes(c) = a.since(BOARD, 1, 4) else { panic!("resync") };
    assert_eq!(c.created, vec![x], "created then changed stays created; created then removed is nothing");
    assert!(c.changed.is_empty());
    assert_eq!(c.removed, vec![z]);
    let Since::Changes(c) = a.since(BOARD, 2, 4) else { panic!("resync") };
    assert_eq!(c.changed, vec![x]);
    assert_eq!(c.removed, vec![y, z]);
    assert_eq!(a.by_actor(BOARD, AGENT).count(), 2);
}

#[test]
fn a_gap_in_the_revisions_needs_a_resync() {
    let mut a = Activity::new();
    a.record(BOARD, entry(1, 2, None, Change::default()));
    a.record(BOARD, entry(3, 4, None, Change::default()));
    assert_eq!(a.since(BOARD, 1, 4), Since::ResyncRequired);
    assert!(matches!(a.since(BOARD, 3, 4), Since::Changes(_)));
    // An entry that doesn't advance is ignored.
    a.record(BOARD, entry(4, 4, None, Change::default()));
    assert_eq!(a.entries(BOARD).count(), 2);
}

#[test]
fn the_log_is_bounded_and_old_revisions_resync() {
    let mut a = Activity::new();
    for r in 0..(Activity::MAX_ENTRIES as u64 + 10) {
        a.record(BOARD, entry(r, r + 1, None, change(&[NodeId(r)], &[], &[])));
    }
    assert_eq!(a.entries(BOARD).count(), Activity::MAX_ENTRIES);
    assert_eq!(a.since(BOARD, 0, Activity::MAX_ENTRIES as u64 + 10), Since::ResyncRequired);
    assert!(matches!(a.since(BOARD, 20, Activity::MAX_ENTRIES as u64 + 10), Since::Changes(_)));
    a.record(BOARD + 1, entry(0, 1, None, Change::default()));
    a.retain(|doc| doc == BOARD + 1);
    assert_eq!(a.entries(BOARD).count(), 0);
    assert_eq!(a.entries(BOARD + 1).count(), 1);
    a.close(BOARD + 1);
    assert_eq!(a.entries(BOARD + 1).count(), 0);
}

#[test]
fn undo_of_a_delete_reads_as_changed() {
    let x = NodeId(9);
    let mut a = Activity::new();
    a.record(BOARD, entry(1, 2, Some(AGENT), change(&[], &[], &[x])));
    a.record(BOARD, entry(2, 3, None, change(&[x], &[], &[])));
    let Since::Changes(c) = a.since(BOARD, 1, 3) else { panic!("resync") };
    assert_eq!(c.changed, vec![x]);
    assert!(c.created.is_empty() && c.removed.is_empty());
}

/// A change too large for the log's budget by itself is dropped: asking across it resyncs, and the
/// revisions after it are answered as usual.
#[test]
fn an_entry_over_the_budget_is_dropped_and_resyncs() {
    let mut a = Activity::new();
    a.record(BOARD, entry(1, 2, None, change(&[NodeId(1)], &[], &[])));
    let huge: Vec<NodeId> = (0..(Activity::MAX_BYTES / std::mem::size_of::<NodeId>() + 1) as u64).map(NodeId).collect();
    a.record(BOARD, entry(2, 3, Some(AGENT), change(&huge, &[], &[])));
    a.record(BOARD, entry(3, 4, None, change(&[NodeId(2)], &[], &[])));
    assert_eq!(a.entries(BOARD).count(), 2, "the oversized entry is not kept");
    assert_eq!(a.since(BOARD, 1, 4), Since::ResyncRequired);
    assert_eq!(a.since(BOARD, 2, 4), Since::ResyncRequired);
    assert!(matches!(a.since(BOARD, 3, 4), Since::Changes(_)));
}

#[test]
fn since_keeps_restacks() {
    let (g, h) = (NodeId(5), NodeId(6));
    let mut a = Activity::new();
    a.record(BOARD, entry(1, 2, Some(AGENT), Change { reordered: vec![g], ..Change::default() }));
    a.record(BOARD, entry(2, 3, None, Change { layers_reordered: true, ..Change::default() }));
    a.record(BOARD, entry(3, 4, None, Change { created: vec![h], reordered: vec![h], ..Change::default() }));
    let Since::Changes(c) = a.since(BOARD, 1, 4) else { panic!("resync") };
    assert_eq!(c.reordered, vec![g], "a container made in between is reported as made");
    assert!(c.layers_reordered);
    assert_eq!(c.created, vec![h]);
}

// ---------------------------------------------------------------- receipts

#[test]
fn a_committed_request_replays_instead_of_running_again() {
    let mut r = Receipts::new();
    let payload = json!({"board": "b1", "expected_rev": 3, "ops": [{"verb": "move", "ids": [4], "delta": [10, 0]}]});
    let Ok(Admission::Fresh { seq, digest }) = r.admit(AGENT, "r1", &payload) else { panic!("fresh") };
    assert_eq!(seq, 1);
    r.commit(AGENT, seq, digest, json!({"rev": 4}));
    // Same payload, keys in another order: the original receipt.
    let reordered = json!({"ops": [{"delta": [10, 0], "ids": [4], "verb": "move"}], "expected_rev": 3, "board": "b1"});
    assert_eq!(r.admit(AGENT, "r1", &reordered), Ok(Admission::Replay(json!({"rev": 4}))));
    assert_eq!(r.admit(AGENT, "r1", &json!({"board": "b2"})), Err(ReceiptError::PayloadMismatch));
    assert_eq!(r.status(AGENT, "r1"), Some(&json!({"rev": 4})));
    // Another agent's r1 is its own.
    assert!(matches!(r.admit(OTHER, "r1", &payload), Ok(Admission::Fresh { .. })));
}

#[test]
fn a_failed_request_can_be_retried_and_a_later_id_retires_earlier_ones() {
    let mut r = Receipts::new();
    let p = json!({});
    // r1 failed before commit: nothing kept, r1 may run again.
    assert!(matches!(r.admit(AGENT, "r1", &p), Ok(Admission::Fresh { .. })));
    let Ok(Admission::Fresh { seq, digest }) = r.admit(AGENT, "r5", &p) else { panic!() };
    r.commit(AGENT, seq, digest, json!(5));
    assert_eq!(r.admit(AGENT, "r1", &p), Err(ReceiptError::TooOld));
    assert_eq!(r.admit(AGENT, "r1", &p).unwrap_err().code(), "not_found");
}

#[test]
fn only_r_numbers_are_request_ids() {
    let r = Receipts::new();
    for bad in ["", "r", "r0", "r012", "1", "rx", "r-1", "r1.5", "R1", "r99999999999999999999999"] {
        assert_eq!(r.admit(AGENT, bad, &json!({})), Err(ReceiptError::InvalidId), "{bad}");
    }
}

#[test]
fn receipts_are_bounded() {
    let mut r = Receipts::new();
    for n in 1..=(Receipts::KEEP as u64 + 5) {
        let id = format!("r{n}");
        let Ok(Admission::Fresh { seq, digest }) = r.admit(AGENT, &id, &json!(n)) else { panic!() };
        r.commit(AGENT, seq, digest, json!(n));
    }
    assert_eq!(r.status(AGENT, "r1"), None);
    assert_eq!(r.admit(AGENT, "r1", &json!(1)), Err(ReceiptError::TooOld));
    assert!(r.status(AGENT, &format!("r{}", Receipts::KEEP + 5)).is_some());
    for k in 0..Receipts::MAX_CLIENTS as u64 {
        r.commit(ActorKey(100 + k), 1, 0, json!(null));
    }
    assert_eq!(r.admit(ActorKey(999), "r1", &json!({})), Err(ReceiptError::TooManyClients));
}
