//! Phase 11 live nodes. Source paths stay authored; derived paths never enter history or files.
// Adapted interpolation/container ideas from VectorCraft crates/doc/src/blend.rs:35-380,
// doc/src/live.rs and engine/src/cmd/live.rs:27-117 @ a469568.
// Copyright (c) 2026 ArtCraft Team. MIT OR Apache-2.0; see varos/NOTICE.
mod evaluate;
mod format;
use crate::{
    geom::Pt,
    model::{Document, Node, NodeKind, Path, Xform},
    EditCommand, Editor,
};
pub use evaluate::{Cache, Stats};
pub use format::{migrate_reserved_era, migrate_v12_to_v13, refuse_older_keys};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    Page,
    Path,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    Horizontal,
    Vertical,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Repeat {
    Radial { count: u16, radius: f32 },
    Grid { rows: u16, cols: u16, gap: Pt },
    Mirror { axis: Axis },
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Warp {
    Arc,
    Flag,
    Bulge,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Envelope {
    Warp { preset: Warp, bend: f32 },
    Mesh { points: [Pt; 4] },
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
pub enum Kind {
    Blend { spine: Option<u32>, steps: u16, orientation: Orientation },
    Repeat { repeat: Repeat },
    Envelope { envelope: Envelope },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Make { paths: Vec<u32>, kind: Kind },
    Options { node: u32, kind: Kind },
    Release { node: u32 },
    Expand { node: u32 },
    Isolate { node: Option<u32> },
    Spine { node: u32, path: u32 },
}
pub fn has_live(doc: &Document) -> bool {
    doc.nodes.iter().any(|n| matches!(n.kind, NodeKind::Live(_)))
}
fn live(doc: &Document, id: u32) -> Result<(&Node, Kind), String> {
    let node = doc.node(id).ok_or("Unknown live node")?;
    let NodeKind::Live(kind) = node.kind else {
        return Err("Expected a live node".into());
    };
    Ok((node, kind))
}
fn sources(doc: &Document, node: &Node, kind: Kind) -> Result<Vec<Path>, String> {
    let spine = if let Kind::Blend { spine, .. } = kind { spine } else { None };
    node.children
        .iter()
        .rev()
        .filter_map(|id| {
            let n = doc.node(*id)?;
            match n.kind {
                NodeKind::Path(pid) if Some(pid) != spine => Some(Ok(pid)),
                _ if matches!(n.kind, NodeKind::Path(_)) => None,
                _ => Some(Err("Live sources must be path leaves".to_owned())),
            }
        })
        .map(|pid| {
            let pid = pid?;
            let p = doc.paths.iter().find(|p| p.id == pid).ok_or("Missing live source")?;
            let mut p = crate::live_corners::evaluated(p);
            p.fill = p.appearance().fill().resolved(doc);
            p.stroke = p.appearance().stroke().resolved(doc);
            Ok(p)
        })
        .collect()
}
pub fn validate(doc: &Document) -> Result<(), String> {
    let mut derived = 0usize;
    for node in &doc.nodes {
        if let NodeKind::Live(kind) = node.kind {
            if node.role != crate::model::GroupRole::Normal {
                return Err("Live node cannot own a mask".into());
            }
            let paths = sources(doc, node, kind)?;
            derived = derived.checked_add(evaluate::check(doc, kind, &paths)?).ok_or("Live budget overflow")?;
            if derived > evaluate::MAX_ANCHORS {
                return Err("Live geometry budget exceeded".into());
            }
            if let Kind::Blend { spine: Some(pid), .. } = kind {
                if !node.children.iter().any(|id| doc.node(*id).is_some_and(|n| n.kind == NodeKind::Path(pid))) {
                    return Err("Blend spine must be an owned child".into());
                }
            }
        }
    }
    if derived > 0 && doc.ids.checked_add(derived as u32).and_then(|v| v.checked_add(8192)).is_none() {
        return Err("Live expansion has no ID space".into());
    }
    Ok(())
}
pub fn check(ed: &Editor, action: &Action) -> Result<(), String> {
    if ed.transaction_open() {
        return Err("Finish the current gesture first".into());
    }
    match action {
        Action::Make { paths, kind } => {
            if paths.is_empty() || paths.len() > 1000 {
                return Err("Select 1–1000 paths".into());
            }
            let mut seen = std::collections::HashSet::new();
            let mut source = Vec::new();
            for id in paths {
                if !seen.insert(*id) || ed.doc.eff_hidden(*id) || ed.doc.eff_locked(*id) {
                    return Err("Sources must be distinct visible unlocked paths".into());
                }
                let p = ed.doc.paths.iter().find(|p| p.id == *id).ok_or("Unknown source path")?;
                let nid = ed.doc.node_of_path(*id).ok_or("Source has no tree leaf")?;
                if ed.doc.top_group_of_path(*id).is_some()
                    || ed.doc.node(nid).is_none_or(|n| n.role != crate::model::GroupRole::Normal)
                {
                    return Err("Release source groups/masks before making a live node".into());
                }
                let mut p = crate::live_corners::evaluated(p);
                p.fill = p.appearance().fill().resolved(&ed.doc);
                p.stroke = p.appearance().stroke().resolved(&ed.doc);
                source.push(p);
            }
            if matches!(kind, Kind::Blend { spine: Some(_), .. }) {
                return Err("Use live_spine after Make".into());
            }
            evaluate::check(&ed.doc, *kind, &source)?;
            Ok(())
        }
        Action::Isolate { node } => {
            if let Some(id) = node {
                live(&ed.doc, *id)?;
            }
            Ok(())
        }
        Action::Options { node, kind } => {
            let (n, old) = live(&ed.doc, *node)?;
            if std::mem::discriminant(&old) != std::mem::discriminant(kind) {
                return Err("Options cannot change the live node family".into());
            }
            if let (Kind::Blend { spine: a, .. }, Kind::Blend { spine: b, .. }) = (old, kind) {
                if a != *b {
                    return Err("Use live_spine to replace the spine".into());
                }
            }
            editable(ed, n)?;
            evaluate::check(&ed.doc, *kind, &sources(&ed.doc, n, *kind)?)?;
            Ok(())
        }
        Action::Spine { node, path } => {
            let (n, kind) = live(&ed.doc, *node)?;
            editable(ed, n)?;
            if !matches!(kind, Kind::Blend { .. }) {
                return Err("Spine requires Blend".into());
            }
            let p = ed.doc.paths.iter().find(|p| p.id == *path).ok_or("Unknown spine")?;
            if p.closed
                || p.anchors.len() < 2
                || !p.holes.is_empty()
                || ed.doc.top_group_of_path(*path).is_some()
                || ed.doc.eff_locked(*path)
                || ed.doc.eff_hidden(*path)
            {
                return Err("Spine must be an unlocked open independent path".into());
            }
            Ok(())
        }
        Action::Release { node } | Action::Expand { node } => {
            let (n, _) = live(&ed.doc, *node)?;
            editable(ed, n)
        }
    }
}
fn editable(ed: &Editor, n: &Node) -> Result<(), String> {
    if n.hidden
        || n.locked
        || n.children
            .iter()
            .filter_map(|id| ed.doc.node(*id))
            .any(|n| matches!(n.kind, NodeKind::Path(pid) if ed.doc.eff_hidden(pid) || ed.doc.eff_locked(pid)))
    {
        Err("Live node is hidden or locked".into())
    } else {
        Ok(())
    }
}
pub fn apply(ed: &mut Editor, action: Action) -> Result<(), String> {
    check(ed, &action)?;
    if let Action::Isolate { node } = action {
        ed.isolate(node);
        return Ok(());
    }
    // Stage before history: all evaluation/refusals are atomic.
    let mut doc = ed.doc.clone();
    let selected = match action {
        Action::Make { paths, kind } => {
            let front = paths.iter().filter_map(|id| doc.pidx(*id)).max().ok_or("No sources")?;
            let first = doc.node_of_path(doc.paths[front].id).ok_or("Missing source leaf")?;
            let template = doc.node(first).cloned().ok_or("Missing source")?;
            let order = template
                .parent
                .and_then(|id| doc.node(id))
                .map(|n| n.children.clone())
                .unwrap_or_else(|| doc.roots.clone());
            let src_nodes: Vec<_> = paths.iter().filter_map(|pid| doc.node_of_path(*pid)).collect();
            let slot = order.iter().take_while(|id| **id != first).filter(|id| !src_nodes.contains(id)).count();
            let mut children = Vec::new();
            for pid in &paths {
                let xf = doc.unit_xform(*pid);
                if let Some(i) = doc.pidx(*pid) {
                    let mut p = doc.paths[i].clone();
                    map_path(&mut p, |pt| xf.apply(pt));
                    doc.paths[i] = p;
                }
                let id = doc.node_of_path(*pid).ok_or("Missing source")?;
                let parent = doc.node(id).and_then(|n| n.parent);
                detach(&mut doc, id, parent);
                children.push(id);
            }
            children.sort_by_key(|id| {
                std::cmp::Reverse(
                    doc.node(*id)
                        .and_then(|n| if let NodeKind::Path(pid) = n.kind { doc.pidx(pid) } else { None })
                        .unwrap_or(0),
                )
            });
            let id = doc.nid();
            for c in &children {
                if let Some(n) = doc.node_mut(*c) {
                    n.parent = Some(id);
                    n.xform = Xform::default();
                }
            }
            let parent = template.parent;
            doc.nodes.push(Node {
                id,
                kind: NodeKind::Live(kind),
                children,
                name: "Live".into(),
                xform: Xform::default(),
                ..template
            });
            if let Some(p) = parent.and_then(|p| doc.node_mut(p)) {
                p.children.insert(slot.min(p.children.len()), id);
            } else {
                doc.roots.insert(slot.min(doc.roots.len()), id);
            }
            paths
        }
        Action::Options { node, kind } => {
            if let Some(n) = doc.node_mut(node) {
                n.kind = NodeKind::Live(kind);
            }
            ed.objsel.iter().copied().collect()
        }
        Action::Release { node } => {
            if let Some(n) = doc.node_mut(node) {
                n.kind = NodeKind::Group;
            }
            doc.node_paths(node)
        }
        Action::Expand { node } => {
            let (n, kind) = live(&doc, node)?;
            let paths = ed.live_cache.evaluate(&doc, n, kind)?;
            bake(&mut doc, node, &paths)?;
            doc.node_paths(node)
        }
        Action::Spine { node, path } => {
            let (_, kind) = live(&doc, node)?;
            let target_xf = doc.node_xform(node);
            let Kind::Blend { steps, orientation, spine: old } = kind else {
                return Err("Expected blend".into());
            };
            // Previous spine is released, never silently destroyed.
            if let Some(old) = old {
                if let Some(id) = doc.node_of_path(old) {
                    if let Some(i) = doc.pidx(old) {
                        map_path(&mut doc.paths[i], |p| target_xf.apply(p));
                    }
                    detach(&mut doc, id, Some(node));
                    let parent = doc.node(node).and_then(|n| n.parent);
                    if let Some(n) = doc.node_mut(id) {
                        n.parent = parent;
                    }
                    if let Some(p) = parent.and_then(|p| doc.node_mut(p)) {
                        p.children.insert(0, id);
                    } else {
                        doc.roots.insert(0, id);
                    }
                }
            }
            let source_xf = doc.unit_xform(path);
            if let Some(i) = doc.pidx(path) {
                map_path(&mut doc.paths[i], |p| target_xf.inverse_apply(source_xf.apply(p)));
            }
            let id = doc.node_of_path(path).ok_or("Missing spine leaf")?;
            let parent = doc.node(id).and_then(|n| n.parent);
            detach(&mut doc, id, parent);
            if let Some(n) = doc.node_mut(id) {
                n.parent = Some(node);
                n.xform = Xform::default();
            }
            if let Some(n) = doc.node_mut(node) {
                n.children.push(id);
                n.kind = NodeKind::Live(Kind::Blend { spine: Some(path), steps, orientation });
            }
            Vec::new()
        }
        Action::Isolate { .. } => return Ok(()),
    };
    crate::format::check_structure(&doc, &crate::format::Limits::default()).map_err(|e| e.to_string())?;
    doc.sync_tree();
    validate(&doc)?;
    for n in &doc.nodes {
        if let NodeKind::Live(kind) = n.kind {
            ed.live_cache.evaluate(&doc, n, kind)?;
        }
    }
    ed.begin();
    ed.doc = doc;
    ed.objsel = selected.into_iter().collect();
    ed.dirty = true;
    ed.commit();
    Ok(())
}
fn detach(doc: &mut Document, id: u32, parent: Option<u32>) {
    if let Some(p) = parent.and_then(|p| doc.node_mut(p)) {
        p.children.retain(|c| *c != id);
    } else {
        doc.roots.retain(|c| *c != id);
    }
}
pub(crate) fn map_path(path: &mut Path, f: impl Fn(Pt) -> Pt) {
    for a in path.anchors.iter_mut().chain(path.holes.iter_mut().flatten()) {
        a.p = f(a.p);
        a.hin = a.hin.map(&f);
        a.hout = a.hout.map(&f);
    }
    path.map_gradient_placement(f);
}
fn bake(doc: &mut Document, node: u32, paths: &[Path]) -> Result<(), String> {
    let children = doc.node(node).ok_or("Missing live node")?.children.clone();
    let ids: Vec<_> = children
        .iter()
        .filter_map(|id| doc.node(*id))
        .filter_map(|n| if let NodeKind::Path(pid) = n.kind { Some(pid) } else { None })
        .collect();
    doc.paths.retain(|p| !ids.contains(&p.id));
    doc.guide_paths.retain(|p| !ids.contains(p));
    doc.nodes.retain(|n| !children.contains(&n.id));
    let mut children = Vec::new();
    for p in paths {
        let mut p = p.clone();
        p.id = doc.nid();
        for a in p.anchors.iter_mut().chain(p.holes.iter_mut().flatten()) {
            a.id = doc.nid();
        }
        let id = doc.nid();
        doc.nodes.push(Node {
            id,
            kind: NodeKind::Path(p.id),
            parent: Some(node),
            children: vec![],
            name: String::new(),
            hidden: p.hidden,
            locked: p.locked,
            color: None,
            clip_exempt: false,
            xform: Xform::default(),
            role: crate::model::GroupRole::Normal,
            mask_child: None,
        });
        children.insert(0, id);
        doc.paths.push(p);
    }
    if let Some(n) = doc.node_mut(node) {
        n.kind = NodeKind::Group;
        n.children = children;
    }
    Ok(())
}
pub fn evaluated_document(doc: &Document) -> Result<Option<Document>, String> {
    if !has_live(doc) {
        return Ok(None);
    }
    evaluated_with(doc, &Cache::default(), None).map(Some)
}
fn evaluated_with(doc: &Document, cache: &Cache, isolation: Option<u32>) -> Result<Document, String> {
    crate::format::check_structure(doc, &crate::format::Limits::DEFAULT).map_err(|e| e.to_string())?;
    validate(doc)?;
    let mut out = doc.clone();
    for n in &doc.nodes {
        if let NodeKind::Live(kind) = n.kind {
            if isolation == Some(n.id) {
                if let Some(n) = out.node_mut(n.id) {
                    n.kind = NodeKind::Group;
                }
            } else {
                bake(&mut out, n.id, &cache.evaluate(doc, n, kind)?)?;
            }
        }
    }
    out.sync_tree();
    Ok(out)
}
pub(crate) fn scene_editor(ed: &Editor) -> Editor {
    let mut out = ed.clone();
    match ed.live_cache.document(&ed.doc, ed.select_transform.isolation) {
        Ok(doc) => {
            out.doc = doc.as_ref().clone();
            for n in &ed.doc.nodes {
                if matches!(n.kind, NodeKind::Live(_)) && ed.select_transform.isolation != Some(n.id) {
                    let src = ed.doc.node_paths(n.id);
                    if src.iter().any(|p| ed.objsel.contains(p)) {
                        out.objsel.retain(|id| !src.contains(id));
                        out.objsel.extend(out.doc.node_paths(n.id));
                    }
                }
            }
        }
        Err(_) => {
            // Invalid direct edits must not display stale geometry as valid artwork.
            out.doc.paths.retain(|p| ancestor(&ed.doc, p.id).is_none());
            for n in &mut out.doc.nodes {
                if matches!(n.kind, NodeKind::Live(_)) {
                    n.kind = NodeKind::Group;
                }
            }
            out.doc.sync_tree();
        }
    }
    out
}
/// Nearest live ancestor, including a live object subsequently wrapped in an ordinary group.
pub(crate) fn ancestor(doc: &Document, pid: u32) -> Option<u32> {
    let mut id = doc.node_of_path(pid)?;
    for _ in 0..=doc.nodes.len() {
        let node = doc.node(id)?;
        if matches!(node.kind, NodeKind::Live(_)) {
            return Some(id);
        }
        id = node.parent?;
    }
    None
}
/// Menu target follows selected sources; no second selection identity model.
pub fn selected_node(ed: &Editor) -> Option<u32> {
    ed.objsel.iter().find_map(|pid| ancestor(&ed.doc, *pid))
}
/// The headless host uses exactly the same checked edit command as the desktop and Bridge.
pub fn execute(ed: &mut Editor, action: Action) -> Result<(), String> {
    ed.try_execute(EditCommand::Live(action))
}

/// Blend tool W: two object clicks, creation through the checked command boundary.
pub(crate) fn tool_down(ed: &mut Editor, pos: Pt) {
    let Some(id) = ed.path_under(pos) else {
        ed.objsel.clear();
        return;
    };
    if ed.objsel.len() != 1 {
        ed.objsel.clear();
        ed.objsel.insert(id);
        return;
    }
    let Some(first) = ed.objsel.iter().next().copied() else {
        return;
    };
    if first != id {
        ed.colour_error = execute(
            ed,
            Action::Make {
                paths: vec![first, id],
                kind: Kind::Blend { spine: None, steps: 8, orientation: Orientation::Page },
            },
        )
        .err();
    }
}
/// A hit on a derived copy resolves to an authored source, so normal Object selection and
/// double-click isolation keep using the same tree and undo model.
pub(crate) fn hit(ed: &Editor, node: u32, pos: Pt) -> Option<u32> {
    let (n, kind) = live(&ed.doc, node).ok()?;
    let paths = ed.live_cache.evaluate(&ed.doc, n, kind).ok()?;
    let first = ed.doc.node_paths(node).first().copied()?;
    let lp = ed.doc.unit_xform(first).inverse_apply(pos);
    let doc = Document { paths: paths.as_ref().clone(), ..Default::default() };
    for i in (0..doc.paths.len()).rev() {
        let p = &doc.paths[i];
        if p.hidden || p.locked {
            continue;
        }
        let reach = 4.0 / ed.ppu.max(0.01) + p.stroke_width * 0.5;
        if !matches!(p.fill, crate::model::Paint::None) && doc.point_in_path(i, lp) {
            return Some(first);
        }
        let pts = doc.world_outline_px(i, ed.ppu);
        if pts.windows(2).any(|s| segment_distance(lp, s[0], s[1]) <= reach) {
            return Some(first);
        }
    }
    None
}

fn segment_distance(p: Pt, a: Pt, b: Pt) -> f32 {
    let d = crate::geom::sub(b, a);
    let t = ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / (d[0] * d[0] + d[1] * d[1]).max(1e-12);
    crate::geom::dist(p, crate::geom::add(a, crate::geom::scale(d, t.clamp(0.0, 1.0))))
}

/// Clipboard/duplicate seam: the copied blend never refers back to the original spine.
pub(crate) fn remap_kind(mut kind: Kind, paths: &std::collections::HashMap<u32, u32>) -> Kind {
    if let Kind::Blend { ref mut spine, .. } = kind {
        *spine = spine.and_then(|id| paths.get(&id).copied());
    }
    kind
}

/// Mesh placement follows baked source geometry once per owning node.
pub(crate) fn map_mesh(doc: &mut Document, pid: u32, f: impl Fn(Pt) -> Pt) {
    let Some(leaf) = doc.node_of_path(pid) else { return };
    let Some(owner) = doc.node(leaf).and_then(|n| n.parent) else { return };
    if doc.node_paths(owner).first().copied() != Some(pid) {
        return;
    }
    if let Some(n) = doc.node_mut(owner) {
        if let NodeKind::Live(kind) = n.kind {
            n.kind = NodeKind::Live(map_mesh_kind(kind, f));
        }
    }
}
pub(crate) fn map_mesh_kind(kind: Kind, f: impl Fn(Pt) -> Pt) -> Kind {
    match kind {
        Kind::Envelope { envelope: Envelope::Mesh { points } } => {
            Kind::Envelope { envelope: Envelope::Mesh { points: points.map(f) } }
        }
        kind => kind,
    }
}
/// Recompute pointer transforms from the transaction snapshot, avoiding cumulative drift.
pub(crate) fn map_mesh_gesture(
    doc: &mut Document,
    base: Option<&Document>,
    pids: &[u32],
    world: bool,
    f: impl Fn(Pt) -> Pt,
) {
    let source = base.unwrap_or(doc);
    let mapped: Vec<_> = source
        .nodes
        .iter()
        .filter_map(|n| {
            let NodeKind::Live(kind @ Kind::Envelope { envelope: Envelope::Mesh { .. } }) = n.kind else { return None };
            let members = source.node_paths(n.id);
            if members.is_empty() || !members.iter().all(|pid| pids.contains(pid)) {
                return None;
            }
            let source_xf = source.unit_xform(members[0]);
            let dest_xf = doc.unit_xform(members[0]);
            Some((
                n.id,
                map_mesh_kind(kind, |p| if world { dest_xf.inverse_apply(f(source_xf.apply(p))) } else { f(p) }),
            ))
        })
        .collect();
    for (id, kind) in mapped {
        if let Some(n) = doc.node_mut(id) {
            n.kind = NodeKind::Live(kind);
        }
    }
}
