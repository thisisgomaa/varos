//! Resolution/baking preserves each Transform copy as an independent painted leaf.
use crate::{
    effects,
    model::{Document, GroupRole, Node, NodeKind, Path},
    Editor,
};
use std::borrow::Cow;
use std::cell::RefCell;
#[derive(Default)]
struct Cache {
    entries: Vec<(Path, Vec<Path>)>,
    anchors: usize,
}
thread_local! {static CACHE:RefCell<Cache>=RefCell::new(Cache::default());}
fn count(p: &Path) -> usize {
    p.anchors.len() + p.holes.iter().map(Vec::len).sum::<usize>()
}
pub fn resolved_many(p: &Path) -> Result<Vec<Path>, String> {
    if p.effects.is_empty() {
        return Ok(vec![crate::live_corners::corners_only(p)]);
    }
    if let Some(parts) =
        CACHE.with(|c| c.borrow().entries.iter().find(|(key, _)| key == p).map(|(_, parts)| parts.clone()))
    {
        return Ok(parts);
    }
    let parts = effects::evaluate_many(p)?;
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        c.entries.retain(|(key, _)| key.id != p.id);
        c.anchors = c.entries.iter().map(|(key, parts)| count(key) + parts.iter().map(count).sum::<usize>()).sum();
        let cost = count(p) + parts.iter().map(count).sum::<usize>();
        while !c.entries.is_empty() && (c.anchors + cost > 200_000 || c.entries.len() >= 128) {
            let (key, parts) = c.entries.remove(0);
            c.anchors -= count(&key) + parts.iter().map(count).sum::<usize>();
        }
        c.anchors += cost;
        c.entries.push((p.clone(), parts.clone()));
    });
    Ok(parts)
}
fn materialize(doc: &mut Document, id: u32, mut parts: Vec<Path>) -> Result<(), String> {
    let pi = doc.pidx(id).ok_or("unknown path")?;
    let leaf = doc.node_of_path(id).ok_or("missing path leaf")?;
    if parts.len() > 1 && doc.is_mask_source(id) {
        return Err("Transform copies cannot replace a clipping-mask source".into());
    }
    for (i, p) in parts.iter_mut().enumerate() {
        p.id = if i == 0 { id } else { doc.nid() };
        for a in p.anchors.iter_mut().chain(p.holes.iter_mut().flatten()) {
            if i > 0 || a.id == 0 {
                a.id = doc.nid();
            }
        }
    }
    if parts.len() == 1 {
        doc.paths[pi] = parts.remove(0);
        return Ok(());
    }
    let original = doc.node(leaf).cloned().ok_or("missing path leaf")?;
    let mut children = vec![];
    for p in &parts {
        let nid = doc.nid();
        children.push(nid);
        doc.nodes.push(Node {
            id: nid,
            kind: NodeKind::Path(p.id),
            name: original.name.clone(),
            parent: Some(leaf),
            children: vec![],
            hidden: false,
            locked: false,
            color: original.color,
            clip_exempt: false,
            xform: Default::default(),
            role: GroupRole::Normal,
            mask_child: None,
        });
    }
    // Children are front-first; authored original precedes its subsequent copies in paint order.
    children.reverse();
    if let Some(node) = doc.node_mut(leaf) {
        node.kind = NodeKind::Group;
        node.children = children;
    }
    doc.paths.remove(pi);
    doc.paths.extend(parts);
    Ok(())
}
pub fn document(source: &Document) -> Result<Cow<'_, Document>, String> {
    if source.paths.iter().all(|p| p.corners.is_empty() && p.effects.is_empty()) {
        return Ok(Cow::Borrowed(source));
    }
    let mut doc = source.clone();
    let prepared = source.paths.iter().map(|p| Ok((p.id, resolved_many(p)?))).collect::<Result<Vec<_>, String>>()?;
    let limits = crate::format::Limits::DEFAULT;
    let paths = prepared.iter().map(|(_, p)| p.len()).sum::<usize>();
    let anchors = prepared.iter().flat_map(|(_, p)| p).map(count).sum::<usize>();
    let nodes = doc.nodes.len() + prepared.iter().filter(|(_, p)| p.len() > 1).map(|(_, p)| p.len()).sum::<usize>();
    if paths > limits.max_paths || anchors > limits.max_anchors || nodes > limits.max_nodes {
        return Err("resolved live-object document exceeds format geometry limits".into());
    }
    for (id, parts) in prepared {
        materialize(&mut doc, id, parts)?;
    }
    doc.sync_tree();
    Ok(Cow::Owned(doc))
}
pub fn expand(ed: &mut Editor, ids: &[u32]) {
    let prepared = ids
        .iter()
        .filter_map(|id| ed.doc.pidx(*id).map(|i| (*id, &ed.doc.paths[i])))
        .map(|(id, p)| Ok((id, resolved_many(p)?)))
        .collect::<Result<Vec<_>, String>>();
    let Ok(prepared) = prepared else { return };
    let untouched = ed.doc.paths.iter().filter(|p| !ids.contains(&p.id));
    let paths = untouched.clone().count() + prepared.iter().map(|(_, p)| p.len()).sum::<usize>();
    let anchors = untouched.map(count).sum::<usize>() + prepared.iter().flat_map(|(_, p)| p).map(count).sum::<usize>();
    let nodes = ed.doc.nodes.len() + prepared.iter().filter(|(_, p)| p.len() > 1).map(|(_, p)| p.len()).sum::<usize>();
    let limits = crate::format::Limits::DEFAULT;
    if paths > limits.max_paths || anchors > limits.max_anchors || nodes > limits.max_nodes {
        return;
    }
    let mut doc = ed.doc.clone();
    for (id, parts) in prepared {
        if materialize(&mut doc, id, parts).is_err() {
            return;
        }
    }
    doc.sync_tree();
    if doc.content_eq(&ed.doc) {
        return;
    }
    ed.begin();
    ed.doc = doc;
    ed.dirty = true;
    ed.commit();
}

/// Object > Expand resolves all live copies before outlining their strokes, in one publication.
pub fn expand_appearance(ed: &mut Editor) {
    let ids: Vec<_> = ed.selected_pids().into_iter().collect();
    let mut stage = ed.clone();
    expand(&mut stage, &ids);
    let targets: Vec<_> = ids.iter().flat_map(|id| stage.doc.group_members(*id)).collect();
    stage.objsel = targets.into_iter().collect();
    stage.path_advanced(crate::path_advanced::Action::Expand);
    if stage.doc.content_eq(&ed.doc) {
        return;
    }
    ed.begin();
    ed.doc = stage.doc;
    ed.dirty = true;
    ed.commit();
}
