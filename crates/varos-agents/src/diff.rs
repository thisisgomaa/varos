//! What one command changed in a document.
//!
//! An edit copies only what it changes: every node it leaves alone is the same `Arc` in the
//! document before and after, and so is every subtree with nothing changed inside. The diff skips
//! those by pointer and compares only along the changed branches, so its cost follows the size of
//! the edit, not of the document. In a long sibling list (a compound path of thousands of parts)
//! the children are walked pairwise from both ends, skipping shared `Arc`s by pointer; only the
//! stretch in between where the lists really differ is indexed by id. A node is matched by its id
//! (ids are never reused), so a node moved to another parent counts as changed, not as removed and
//! created.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use vectorcraft_doc::{Artboard, Document, Node, NodeId, NodeKind, RepeatSpec};

/// The objects and artboards one change created, changed and removed, and the containers whose
/// stacking order changed.
///
/// `created` and `changed` are in paint order (bottom first, a parent before its children);
/// `removed` in the order the document before held them. A container is in `changed` only when
/// something of its own changed (name, appearance, visibility, clip…), not merely because a child
/// changed; a layer is reported like any other node, and callers that draw objects skip layers.
/// A container is in `reordered` when the children it held before and still holds are stacked in
/// another order (Bring to Front, Send Backward…): nothing of the objects themselves changed, so
/// presence shows nothing for it, but the activity log has it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Change {
    pub created: Vec<NodeId>,
    pub changed: Vec<NodeId>,
    pub removed: Vec<NodeId>,
    /// Containers (layers, groups…) whose children's stacking order changed.
    pub reordered: Vec<NodeId>,
    /// The order of the top-level layers changed.
    pub layers_reordered: bool,
    pub artboards_created: Vec<u32>,
    pub artboards_changed: Vec<u32>,
    pub artboards_removed: Vec<u32>,
}

impl Change {
    /// Nothing at all changed in the art, its stacking order or the artboards.
    pub fn is_empty(&self) -> bool {
        self.created.is_empty()
            && self.changed.is_empty()
            && self.removed.is_empty()
            && self.reordered.is_empty()
            && !self.layers_reordered
            && self.artboards_created.is_empty()
            && self.artboards_changed.is_empty()
            && self.artboards_removed.is_empty()
    }

    /// The objects created, then the objects changed, each once: the order presence reveals them in.
    pub fn touched(&self) -> Vec<NodeId> {
        let mut seen = HashSet::new();
        self.created.iter().chain(&self.changed).copied().filter(|id| seen.insert(*id)).collect()
    }

    /// The artboards created or changed, in that order.
    pub fn artboards_touched(&self) -> Vec<u32> {
        let mut seen = HashSet::new();
        self.artboards_created.iter().chain(&self.artboards_changed).copied().filter(|id| seen.insert(*id)).collect()
    }
}

/// The change that turns `before` into `after`.
pub fn diff(before: &Document, after: &Document) -> Change {
    diff_counted(before, after).0
}

/// [`diff`], and how many nodes it looked at beyond a pointer comparison (compared with their old
/// selves, or indexed by id): the work it did, which tests hold to the size of the edit.
pub fn diff_counted(before: &Document, after: &Document) -> (Change, usize) {
    let mut w = Walk::default();
    w.lists(&before.layers, &after.layers, None, None);
    let visited = w.visited;
    let mut change = w.resolve();
    artboards(&before.artboards, &after.artboards, &mut change);
    (change, visited)
}

/// Phase one gathers, along the changed branches only: nodes whose own fields changed, the
/// subtrees that appear (in `after` but not at that level of `before`) or vanish, and the lists
/// whose order changed. Phase two ([`Walk::resolve`]) matches appeared and vanished nodes by id: a
/// node in both moved.
#[derive(Default)]
struct Walk<'a> {
    changed: Vec<NodeId>,
    changed_set: HashSet<NodeId>,
    /// Appeared subtree roots, in `after` order, with their parent's id in `after`.
    appeared: Vec<(&'a Arc<Node>, Option<NodeId>)>,
    /// Vanished subtree roots, in `before` order, with their parent's id in `before`.
    vanished: Vec<(&'a Arc<Node>, Option<NodeId>)>,
    reordered: Vec<NodeId>,
    layers_reordered: bool,
    visited: usize,
}

impl<'a> Walk<'a> {
    fn mark(&mut self, id: NodeId) {
        if self.changed_set.insert(id) {
            self.changed.push(id);
        }
    }

    /// Compare two sibling lists, matched by id: pairwise from the front, then from the back, both
    /// skipping shared `Arc`s by pointer and stopping where the ids part; the stretch left between
    /// is matched through an index of its ids. One edited child of a long list costs one pointer
    /// comparison per sibling and nothing else.
    fn lists(&mut self, before: &'a [Arc<Node>], after: &'a [Arc<Node>], before_parent: Option<NodeId>, after_parent: Option<NodeId>) {
        let mut start = 0;
        while let (Some(b), Some(a)) = (before.get(start), after.get(start)) {
            if !Arc::ptr_eq(b, a) {
                if b.id != a.id {
                    break;
                }
                self.visited += 1;
                self.pair(b, a);
            }
            start += 1;
        }
        let (mut b_end, mut a_end) = (before.len(), after.len());
        while b_end > start && a_end > start {
            let (b, a) = (&before[b_end - 1], &after[a_end - 1]);
            if !Arc::ptr_eq(b, a) {
                if b.id != a.id {
                    break;
                }
                self.visited += 1;
                self.pair(b, a);
            }
            b_end -= 1;
            a_end -= 1;
        }
        let (before, after) = (&before[start..b_end], &after[start..a_end]);
        if before.is_empty() && after.is_empty() {
            return;
        }
        self.visited += before.len() + after.len();
        let old: HashMap<NodeId, &'a Arc<Node>> = before.iter().map(|n| (n.id, n)).collect();
        let kept: HashSet<NodeId> = after.iter().map(|n| n.id).collect();
        for n in after {
            match old.get(&n.id) {
                Some(o) => self.pair(o, n),
                None => self.appeared.push((n, after_parent)),
            }
        }
        for o in before {
            if !kept.contains(&o.id) {
                self.vanished.push((o, before_parent));
            }
        }
        // The children in both lists, in each list's order: a different order is a restack.
        let stayed_before = before.iter().map(|n| n.id).filter(|id| kept.contains(id));
        let stayed_after = after.iter().map(|n| n.id).filter(|id| old.contains_key(id));
        if !stayed_before.eq(stayed_after) {
            match after_parent {
                Some(parent) => self.reordered.push(parent),
                None => self.layers_reordered = true,
            }
        }
    }

    /// Compare one node with its old self.
    fn pair(&mut self, before: &'a Arc<Node>, after: &'a Arc<Node>) {
        if Arc::ptr_eq(before, after) {
            return;
        }
        match (before.children(), after.children()) {
            (Some(bc), Some(ac)) => {
                if !shell_eq(before, after) {
                    self.mark(after.id);
                }
                self.lists(bc, ac, Some(before.id), Some(after.id));
            }
            (None, None) => {
                if **before != **after {
                    self.mark(after.id);
                }
            }
            // A leaf became a container or the other way round (Make Compound Path, Expand…).
            (bc, ac) => {
                self.mark(after.id);
                for c in ac.into_iter().flatten() {
                    self.appeared.push((c, Some(after.id)));
                }
                for c in bc.into_iter().flatten() {
                    self.vanished.push((c, Some(before.id)));
                }
            }
        }
    }

    fn resolve(mut self) -> Change {
        // Every node of the vanished subtrees, with its parent before.
        let mut gone: HashMap<NodeId, (&'a Node, Option<NodeId>)> = HashMap::new();
        let mut gone_order: Vec<NodeId> = Vec::new();
        for (root, parent) in &self.vanished {
            subtree(root, *parent, &mut |n, p| {
                if gone.insert(n.id, (n, p)).is_none() {
                    gone_order.push(n.id);
                }
            });
        }
        let mut created = Vec::new();
        let mut still_here: HashSet<NodeId> = HashSet::new();
        let appeared = std::mem::take(&mut self.appeared);
        for (root, parent) in appeared {
            subtree(root, parent, &mut |n, p| {
                match gone.get(&n.id) {
                    Some((old, old_parent)) => {
                        still_here.insert(n.id);
                        // Moved: a new parent, or something of its own changed on the way.
                        let own_change = match (old.children(), n.children()) {
                            (Some(_), Some(_)) => !shell_eq(old, n),
                            _ => *old != n,
                        };
                        if *old_parent != p || own_change {
                            self.mark(n.id);
                        }
                    }
                    None => created.push(n.id),
                }
            });
        }
        let removed: Vec<NodeId> = gone_order.into_iter().filter(|id| !still_here.contains(id)).collect();
        // A node can't be both created and changed: created wins.
        let made: HashSet<NodeId> = created.iter().copied().collect();
        let changed = self.changed.into_iter().filter(|id| !made.contains(id)).collect();
        Change { created, changed, removed, reordered: self.reordered, layers_reordered: self.layers_reordered, ..Change::default() }
    }
}

/// Walk `root` and everything under it, pre-order (a parent before its children, children bottom
/// first), with each node's parent id.
fn subtree<'a>(root: &'a Node, parent: Option<NodeId>, f: &mut impl FnMut(&'a Node, Option<NodeId>)) {
    f(root, parent);
    for c in root.children().into_iter().flatten() {
        subtree(c, Some(root.id), f);
    }
}

/// Two containers equal in everything but their children, compared field by field without
/// copying anything. Every field is named (no `..`), so a field added to the model later fails to
/// compile here instead of being missed.
fn shell_eq(a: &Node, b: &Node) -> bool {
    let Node { id, name, visible, locked, opacity, blend, isolate, knockout, knockout_shape, appearance, kind, bbox_angle, extras } = a;
    *id == b.id
        && *name == b.name
        && *visible == b.visible
        && *locked == b.locked
        && *opacity == b.opacity
        && *blend == b.blend
        && *isolate == b.isolate
        && *knockout == b.knockout
        && *knockout_shape == b.knockout_shape
        && *appearance == b.appearance
        && *bbox_angle == b.bbox_angle
        && *extras == b.extras
        && kind_shell_eq(kind, &b.kind)
}

/// Two node kinds equal in everything but their children.
fn kind_shell_eq(a: &NodeKind, b: &NodeKind) -> bool {
    use NodeKind as K;
    match (a, b) {
        (
            K::Layer { color, template, printable, children: _, clip, preview, dim_images },
            K::Layer { color: c2, template: t2, printable: p2, children: _, clip: cl2, preview: pv2, dim_images: d2 },
        ) => color == c2 && template == t2 && printable == p2 && clip == cl2 && preview == pv2 && dim_images == d2,
        (K::Group { children: _, clip }, K::Group { children: _, clip: c2 }) => clip == c2,
        (K::Compound { children: _, rule }, K::Compound { children: _, rule: r2 }) => rule == r2,
        (K::Blend { children: _, spec }, K::Blend { children: _, spec: s2 }) => spec == s2,
        (
            K::Envelope { content: _, kind, fidelity, editing, options, frame },
            K::Envelope { content: _, kind: k2, fidelity: f2, editing: e2, options: o2, frame: fr2 },
        ) => kind == k2 && fidelity == f2 && editing == e2 && options == o2 && frame == fr2,
        (K::Repeat(RepeatSpec { kind, source: _ }), K::Repeat(RepeatSpec { kind: k2, source: _ })) => kind == k2,
        (K::Intertwine { children: _ }, K::Intertwine { children: _ }) => true,
        // Kinds without children compare whole; different kinds differ (by their variant alone).
        _ => a == b,
    }
}

fn artboards(before: &[Artboard], after: &[Artboard], change: &mut Change) {
    let old: HashMap<u32, &Artboard> = before.iter().map(|a| (a.id, a)).collect();
    let kept: HashSet<u32> = after.iter().map(|a| a.id).collect();
    for a in after {
        match old.get(&a.id) {
            None => change.artboards_created.push(a.id),
            Some(o) if *o != a => change.artboards_changed.push(a.id),
            Some(_) => {}
        }
    }
    change.artboards_removed = before.iter().filter(|a| !kept.contains(&a.id)).map(|a| a.id).collect();
}
