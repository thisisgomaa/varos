//! Structural precheck (work order §3 step 6). Runs on every decoded model BEFORE anything walks the
//! tree (`sync_tree`, `collect_paths`, `attach_front`, `is_mask_source` all assume an acyclic, bounded
//! tree), and on every document before it is saved. Iterative and HashMap-indexed: linear in the size
//! of the document, never recursive, so hostile input can neither hang it nor overflow the stack.

use super::error::{Invalid, LoadError};
use super::limits::{LimitKind, Limits};
use crate::model::{Document, NodeKind};
use std::collections::{HashMap, HashSet};

/// Refuse a document whose shape would make the tree code misbehave: over a count limit, duplicate ids
/// within a kind, dangling references, a node with no single owner, a cycle (node tree or legacy group
/// registry), a tree deeper than `max_tree_depth`, or an id counter without headroom.
///
/// Cross-kind id reuse is legal (a path, an anchor and a node may share an id), and anchor ids are not
/// required to be unique (R10) — they only feed the id headroom.
pub fn check_structure(doc: &Document, limits: &Limits) -> Result<(), LoadError> {
    check_counts(doc, limits)?;

    // ── ids unique within each kind ──
    let mut path_ids: HashSet<u32> = HashSet::with_capacity(doc.paths.len());
    for p in &doc.paths {
        if !path_ids.insert(p.id) {
            return Err(Invalid::DuplicateId { kind: "path", id: p.id }.into());
        }
    }
    let mut index: HashMap<u32, usize> = HashMap::with_capacity(doc.nodes.len());
    for (i, n) in doc.nodes.iter().enumerate() {
        if index.insert(n.id, i).is_some() {
            return Err(Invalid::DuplicateId { kind: "node", id: n.id }.into());
        }
    }
    let mut group_ids: HashSet<u32> = HashSet::with_capacity(doc.groups.len());
    for g in &doc.groups {
        if !group_ids.insert(g.id) {
            return Err(Invalid::DuplicateId { kind: "legacy group", id: g.id }.into());
        }
    }

    // ── no dangling references ──
    for n in &doc.nodes {
        if let NodeKind::Path(pid) = n.kind {
            if !path_ids.contains(&pid) {
                return Err(Invalid::Dangling { from: "node", id: n.id, missing: pid }.into());
            }
        }
        if let Some(p) = n.parent {
            if !index.contains_key(&p) {
                return Err(Invalid::Dangling { from: "node", id: n.id, missing: p }.into());
            }
        }
        for &c in &n.children {
            if !index.contains_key(&c) {
                return Err(Invalid::Dangling { from: "node", id: n.id, missing: c }.into());
            }
        }
        if let Some(m) = n.mask_child.filter(|m| !index.contains_key(m)) {
            return Err(Invalid::Dangling { from: "node", id: n.id, missing: m }.into());
        }
    }
    for &r in &doc.roots {
        if !index.contains_key(&r) {
            return Err(Invalid::Dangling { from: "root list", id: r, missing: r }.into());
        }
    }

    // ── exactly one owner per node, and the parent link agrees with it ──
    // owner[i] = None (unseen) | Some(None) (listed in `roots`) | Some(Some(p)) (listed in p.children)
    let mut owner: Vec<Option<Option<u32>>> = vec![None; doc.nodes.len()];
    let mut claim = |child: u32, by: Option<u32>| -> Result<(), LoadError> {
        let slot = &mut owner[index[&child]];
        if slot.is_some() {
            return Err(Invalid::BadParentage { node: child, reason: "it is listed in more than one place" }.into());
        }
        *slot = Some(by);
        Ok(())
    };
    for &r in &doc.roots {
        claim(r, None)?;
    }
    for n in &doc.nodes {
        for &c in &n.children {
            claim(c, Some(n.id))?;
        }
    }
    for (n, own) in doc.nodes.iter().zip(&owner) {
        match own {
            None => {
                let reason = if n.parent.is_some() {
                    "its parent does not list it as a child"
                } else {
                    "it has no parent and is not a root layer"
                };
                return Err(Invalid::BadParentage { node: n.id, reason }.into());
            }
            Some(by) if *by != n.parent => {
                let reason = if by.is_none() {
                    "a root layer must not have a parent"
                } else {
                    "its parent link and its parent's children list disagree"
                };
                return Err(Invalid::BadParentage { node: n.id, reason }.into());
            }
            Some(_) => {}
        }
    }

    // ── acyclic + depth: walk down from the roots. Every node has exactly one owner, so a node the walk
    //    never reaches sits on (or hangs below) a cycle of parent links. ──
    let mut seen = vec![false; doc.nodes.len()];
    let mut stack: Vec<(u32, usize)> = doc.roots.iter().map(|&r| (r, 1)).collect();
    let mut deepest = 0usize;
    while let Some((id, depth)) = stack.pop() {
        let i = index[&id];
        if seen[i] {
            return Err(Invalid::Cycle { node: id }.into()); // unreachable after the owner check; defensive
        }
        seen[i] = true;
        deepest = deepest.max(depth);
        for &c in &doc.nodes[i].children {
            stack.push((c, depth + 1));
        }
    }
    if let Some(i) = seen.iter().position(|s| !s) {
        return Err(Invalid::Cycle { node: doc.nodes[i].id }.into());
    }
    if deepest > limits.max_tree_depth {
        return Err(too_large(LimitKind::TreeDepth, deepest, limits.max_tree_depth));
    }

    check_legacy_groups(doc, limits)?;

    // ── id headroom: the counter must be able to hand out every id normalization may allocate, plus one
    //    for the next object the user draws, without overflowing ──
    let base = u64::from(doc.ids.max(max_used_id(doc)));
    if base + ids_normalization_may_allocate(doc) + 1 > u64::from(u32::MAX) {
        return Err(Invalid::IdExhausted.into());
    }
    Ok(())
}

fn too_large(limit: LimitKind, found: usize, max: usize) -> LoadError {
    LoadError::TooLarge { limit, found: found as u64, max: max as u64 }
}

fn check_counts(doc: &Document, limits: &Limits) -> Result<(), LoadError> {
    let nodes = doc.nodes.len().saturating_add(doc.groups.len()); // legacy groups become nodes
    if nodes > limits.max_nodes {
        return Err(too_large(LimitKind::Nodes, nodes, limits.max_nodes));
    }
    if doc.paths.len() > limits.max_paths {
        return Err(too_large(LimitKind::Paths, doc.paths.len(), limits.max_paths));
    }
    let anchors = doc.paths.iter().fold(0usize, |acc, p| {
        p.holes.iter().fold(acc.saturating_add(p.anchors.len()), |a, h| a.saturating_add(h.len()))
    });
    if anchors > limits.max_anchors {
        return Err(too_large(LimitKind::Anchors, anchors, limits.max_anchors));
    }
    if doc.artboards.len() > limits.max_artboards {
        return Err(too_large(LimitKind::Artboards, doc.artboards.len(), limits.max_artboards));
    }
    Ok(())
}

/// The pre-tree registry: `groups[].parent` must not loop (the legacy migration's `attach_front` climbs
/// it recursively), and its chains count against the tree depth (each group becomes a nested node under
/// the host layer). A parent id that names no legacy group is legal v1 data: the migration hangs that
/// group on the host layer.
fn check_legacy_groups(doc: &Document, limits: &Limits) -> Result<(), LoadError> {
    if doc.groups.is_empty() {
        return Ok(());
    }
    let parent: HashMap<u32, Option<u32>> = doc.groups.iter().map(|g| (g.id, g.parent)).collect();
    // depth[g] = number of legacy groups from g up to its top (1 = directly under the host layer)
    let mut depth: HashMap<u32, usize> = HashMap::with_capacity(doc.groups.len());
    for g in &doc.groups {
        let mut chain: Vec<u32> = vec![];
        let mut on_chain: HashSet<u32> = HashSet::new();
        let mut cur = g.id;
        let base = loop {
            if let Some(&d) = depth.get(&cur) {
                break d;
            }
            if !on_chain.insert(cur) {
                return Err(Invalid::Cycle { node: cur }.into());
            }
            chain.push(cur);
            match parent.get(&cur).copied().flatten() {
                Some(p) if parent.contains_key(&p) => cur = p,
                _ => break 0, // top of the chain (no parent, or one the migration maps to the host)
            }
        };
        for (k, &id) in chain.iter().rev().enumerate() {
            depth.insert(id, base + k + 1);
        }
    }
    // host layer (1) + the deepest group chain + the path leaf (1)
    let deepest = depth.values().copied().max().unwrap_or(0) + 2;
    if deepest > limits.max_tree_depth {
        return Err(too_large(LimitKind::TreeDepth, deepest, limits.max_tree_depth));
    }
    Ok(())
}

/// The highest id in use by any kind: paths, anchors (outer + holes), nodes, legacy groups.
pub(crate) fn max_used_id(doc: &Document) -> u32 {
    let mut m = 0u32;
    for p in &doc.paths {
        m = m.max(p.id);
        for a in p.anchors.iter().chain(p.holes.iter().flatten()) {
            m = m.max(a.id);
        }
    }
    for n in &doc.nodes {
        m = m.max(n.id);
    }
    for g in &doc.groups {
        m = m.max(g.id);
    }
    m
}

/// An upper bound on the ids `Document::sync_tree` can allocate through `nid()` for this document:
/// the legacy migration (a host layer, one node per legacy group, one leaf per path), leaves for paths
/// no node refers to, and a replacement "Layer 1".
fn ids_normalization_may_allocate(doc: &Document) -> u64 {
    let legacy = !doc.groups.is_empty() || !doc.group_of.is_empty();
    let leaves = if legacy {
        doc.paths.len()
    } else {
        let known: HashSet<u32> =
            doc.nodes.iter().filter_map(|n| if let NodeKind::Path(p) = n.kind { Some(p) } else { None }).collect();
        doc.paths.iter().filter(|p| !known.contains(&p.id)).count()
    };
    let legacy_nodes = if legacy { 1 + doc.groups.len() } else { 0 };
    (legacy_nodes + leaves + 1) as u64
}
