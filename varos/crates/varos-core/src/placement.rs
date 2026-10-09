//! Insert already-normalized vector artwork; no foreign-format parser belongs in core.
use crate::{
    editor::Editor,
    model::{Document, GroupRole, Node, NodeKind, Xform},
};
use std::collections::HashMap;

/// Checked before allocating ids or opening a transaction.
pub fn check(ed: &Editor, source: &Document) -> Result<(), String> {
    if ed.transaction_open() {
        return Err("cannot place artwork during an active transaction".into());
    }
    let limits = crate::format::Limits::DEFAULT;
    crate::format::check_structure(source, &limits).map_err(|e| e.to_string())?;
    crate::format::validate(source, &limits).map_err(|e| e.to_string())?;
    if source.nodes.iter().any(|n| n.kind == NodeKind::Layer && !source.roots.contains(&n.id)) {
        return Err("placed artwork must have root layers only".into());
    }
    if source.roots.iter().any(|id| source.node(*id).is_none_or(|n| n.kind != NodeKind::Layer)) {
        return Err("placed artwork roots must be layers".into());
    }
    if source.nodes.iter().any(|n| !n.xform.is_identity()) {
        return Err("placed artwork must have baked transforms".into());
    }
    let layer =
        ed.doc.node(ed.doc.active_layer).filter(|n| n.kind == NodeKind::Layer).ok_or("active layer unavailable")?;
    if layer.locked || layer.hidden {
        return Err("cannot place artwork into a locked/hidden target layer".into());
    }
    let mut target_depth = 1usize;
    let mut parent = layer.parent;
    while let Some(id) = parent {
        target_depth += 1;
        if ed.doc.node(id).is_some_and(|n| n.locked || n.hidden) {
            return Err("cannot place artwork under a locked/hidden ancestor".into());
        }
        parent = ed.doc.node(id).and_then(|n| n.parent);
        if target_depth > limits.max_tree_depth {
            return Err("destination tree too deep".into());
        }
    }
    let mut stack: Vec<_> = source.roots.iter().map(|id| (*id, 1usize)).collect();
    while let Some((id, depth)) = stack.pop() {
        if depth + target_depth + 1 > limits.max_tree_depth {
            return Err("placed artwork exceeds tree depth".into());
        }
        if let Some(n) = source.node(id) {
            stack.extend(n.children.iter().map(|id| (*id, depth + 1)));
        }
    }
    let anchors = |d: &Document| {
        d.paths.iter().map(|p| p.anchors.len() + p.holes.iter().map(Vec::len).sum::<usize>()).sum::<usize>()
    };
    if ed.doc.paths.len() + source.paths.len() > limits.max_paths
        || ed.doc.nodes.len() + source.nodes.len() + 1 > limits.max_nodes
        || anchors(&ed.doc) + anchors(source) > limits.max_anchors
    {
        return Err("placed artwork exceeds document limits".into());
    }
    let needed = source.nodes.len() + source.paths.len() + anchors(source) + 1;
    if (ed.doc.ids as u64) + (needed as u64) >= u32::MAX as u64 {
        return Err("placed artwork exceeds id headroom".into());
    }
    Ok(())
}

/// One undo step, with every incoming id remapped into the destination's namespace.
pub fn place(ed: &mut Editor, source: Document) {
    if check(ed, &source).is_err() {
        return;
    }
    if source.paths.is_empty() {
        return;
    }
    ed.begin();
    let mut ids = HashMap::new();
    for id in source
        .nodes
        .iter()
        .map(|n| n.id)
        .chain(source.paths.iter().map(|p| p.id))
        .chain(source.paths.iter().flat_map(|p| p.anchors.iter().chain(p.holes.iter().flatten()).map(|a| a.id)))
    {
        ids.entry(id).or_insert_with(|| ed.doc.nid());
    }
    let group = ed.doc.nid();
    let layer = ed.doc.active_layer;
    // ---- Lane H: imported root layers become named groups, retaining hidden/locked state ----
    let children: Vec<u32> = source.roots.iter().filter_map(|id| ids.get(id).copied()).collect();
    ed.doc.nodes.push(Node {
        id: group,
        kind: NodeKind::Group,
        name: "Placed artwork".into(),
        parent: Some(layer),
        children,
        hidden: false,
        locked: false,
        color: None,
        clip_exempt: false,
        xform: Xform::default(),
        role: GroupRole::Normal,
        // ---- Lane A ----
        look: None,
        mask_child: None,
    });
    if let Some(n) = ed.doc.nodes.iter_mut().find(|n| n.id == layer) {
        n.children.insert(0, group);
    }
    ed.objsel.clear();
    for mut path in source.paths {
        path.id = ids[&path.id];
        for a in path.anchors.iter_mut().chain(path.holes.iter_mut().flatten()) {
            a.id = ids[&a.id];
        }
        ed.objsel.insert(path.id);
        ed.doc.paths.push(path);
    }
    for mut n in source.nodes {
        let is_root = source.roots.contains(&n.id);
        if is_root {
            n.kind = NodeKind::Group;
        }
        n.id = ids[&n.id];
        n.parent = Some(if is_root { group } else { n.parent.and_then(|p| ids.get(&p).copied()).unwrap_or(group) });
        n.children = n.children.iter().filter_map(|id| ids.get(id).copied()).collect();
        if let NodeKind::Path(p) = n.kind {
            n.kind = NodeKind::Path(ids[&p]);
        }
        n.mask_child = n.mask_child.and_then(|id| ids.get(&id).copied());
        ed.doc.nodes.push(n);
    }
    ed.doc.sync_tree();
    ed.dirty = true;
    ed.commit();
}
