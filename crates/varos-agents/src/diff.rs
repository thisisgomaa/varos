//! What one command changed in a document.
//!
//! An edit copies only what it changes: every node it leaves alone is the same `Arc` in the
//! document before and after, and so is every subtree with nothing changed inside. The diff skips
//! those by pointer and compares only along the changed branches, so its cost follows the size of
//! the edit, not of the document. A node is matched by its id (ids are never reused), so a node
//! moved to another parent counts as changed, not as removed and created.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use vectorcraft_doc::{Artboard, Document, Node, NodeId};

/// The objects and artboards one change created, changed and removed.
///
/// `created` and `changed` are in paint order (bottom first, a parent before its children);
/// `removed` in the order the document before held them. A container is in `changed` only when
/// something of its own changed (name, appearance, visibility, clip…), not merely because a child
/// changed; a layer is reported like any other node, and callers that draw objects skip layers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Change {
    pub created: Vec<NodeId>,
    pub changed: Vec<NodeId>,
    pub removed: Vec<NodeId>,
    pub artboards_created: Vec<u32>,
    pub artboards_changed: Vec<u32>,
    pub artboards_removed: Vec<u32>,
}

impl Change {
    /// Nothing at all changed in the art or the artboards.
    pub fn is_empty(&self) -> bool {
        self.created.is_empty()
            && self.changed.is_empty()
            && self.removed.is_empty()
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
    let mut w = Walk::default();
    w.lists(&before.layers, &after.layers, None, None);
    let mut change = w.resolve();
    artboards(&before.artboards, &after.artboards, &mut change);
    change
}

/// Phase one gathers, along the changed branches only: nodes whose own fields changed, and the
/// subtrees that appear (in `after` but not at that level of `before`) or vanish. Phase two
/// ([`Walk::resolve`]) matches appeared and vanished nodes by id: a node in both moved.
#[derive(Default)]
struct Walk<'a> {
    changed: Vec<NodeId>,
    changed_set: HashSet<NodeId>,
    /// Appeared subtree roots, in `after` order, with their parent's id in `after`.
    appeared: Vec<(&'a Arc<Node>, Option<NodeId>)>,
    /// Vanished subtree roots, in `before` order, with their parent's id in `before`.
    vanished: Vec<(&'a Arc<Node>, Option<NodeId>)>,
}

impl<'a> Walk<'a> {
    fn mark(&mut self, id: NodeId) {
        if self.changed_set.insert(id) {
            self.changed.push(id);
        }
    }

    /// Compare two sibling lists, matched by id.
    fn lists(&mut self, before: &'a [Arc<Node>], after: &'a [Arc<Node>], before_parent: Option<NodeId>, after_parent: Option<NodeId>) {
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
        let removed = gone_order.into_iter().filter(|id| !still_here.contains(id)).collect();
        // A node can't be both created and changed: created wins.
        let made: HashSet<NodeId> = created.iter().copied().collect();
        let changed = self.changed.into_iter().filter(|id| !made.contains(id)).collect();
        Change { created, changed, removed, ..Change::default() }
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

/// Two containers equal in everything but their children.
fn shell_eq(a: &Node, b: &Node) -> bool {
    let mut a = a.clone();
    let mut b = b.clone();
    if let Some(c) = a.children_mut() {
        c.clear();
    }
    if let Some(c) = b.children_mut() {
        c.clear();
    }
    a == b
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
