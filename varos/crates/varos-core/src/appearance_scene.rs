//! Lane A: recursive composition; legacy scenes bypass this module entirely.
use crate::{
    appearance::{BaseSlot, StackItem},
    model::{Document, GroupRole, NodeKind, Paint, Path},
    scene::Group,
    Editor,
};
use std::hash::{Hash, Hasher};
pub fn needed(doc: &Document) -> bool {
    doc.paths.iter().any(|p| !p.stack.is_empty())
        || doc.nodes.iter().any(|n| n.look.is_some() || n.role == GroupRole::MaskAlpha)
}
pub fn hash(doc: &Document, state: &mut impl Hasher) {
    // Float bits and every paint/style serialized together; committed and live changes share identity.
    for p in &doc.paths {
        if !p.stack.is_empty() {
            p.id.hash(state);
            if let Ok(bytes) = serde_json::to_vec(&p.stack) {
                bytes.hash(state);
            }
        }
    }
    for n in &doc.nodes {
        if n.role.is_mask_group() || n.mask_child.is_some() {
            n.id.hash(state);
            std::mem::discriminant(&n.role).hash(state);
            n.mask_child.hash(state);
        }
        if let Some(look) = n.look {
            n.id.hash(state);
            look.opacity.to_bits().hash(state);
            look.isolate.hash(state);
        }
    }
}
pub fn paint_paths(path: &Path) -> Vec<Path> {
    path.appearance()
        .stack()
        .iter()
        .filter_map(|item| {
            if !item.opts().visible || item.opts().opacity == 0.0 {
                return None;
            }
            let mut p = path.clone();
            p.stack.clear();
            p.fill = Paint::None;
            p.stroke = Paint::None;
            p.opacity = item.opts().opacity;
            match item {
                StackItem::Base(BaseSlot::Fill, _) => p.fill = path.fill.clone(),
                StackItem::Base(BaseSlot::Stroke, _) => p.stroke = path.stroke.clone(),
                StackItem::Fill { paint, .. } => p.fill = paint.clone(),
                StackItem::Stroke { paint, width, style, .. } => {
                    p.stroke = paint.clone();
                    p.stroke_width = *width;
                    p.stroke_style = style.clone();
                }
            }
            Some(p)
        })
        .collect()
}
/// Each leaf reuses the mature scene evaluator (including live corners, gradient/stroke caches,
/// board clips and view culling). The outer tree owns grouping and masks once.
pub fn compose(ed: &Editor, ppu: f32, errors: &mut Vec<String>, canvas: bool) -> Vec<Group> {
    fn walk(ed: &Editor, id: u32, ppu: f32, errors: &mut Vec<String>, canvas: bool) -> Vec<Group> {
        let Some(n) = ed.doc.node(id) else { return vec![] };
        if n.hidden {
            return vec![];
        }
        let mut members = match n.kind {
            NodeKind::Path(pid) => {
                if canvas && crate::view_depth_scene::outlined(ed, pid) {
                    return vec![];
                }
                let Some(i) = ed.doc.pidx(pid) else { return vec![] };
                let source = &ed.doc.paths[i];
                if source.hidden {
                    return vec![];
                }
                let mut groups = Vec::new();
                let paints = if source.stack.is_empty() { vec![source.clone()] } else { paint_paths(source) };
                for p in paints {
                    let mut child = ed.clone();
                    // Keep the tree for transforms/board membership, disable only composition metadata.
                    for node in &mut child.doc.nodes {
                        node.look = None;
                        node.role = GroupRole::Normal;
                        node.mask_child = None;
                    }
                    for path in &mut child.doc.paths {
                        path.stack.clear();
                        if path.id != pid {
                            path.hidden = true;
                        }
                    }
                    child.doc.paths[i] = p;
                    child.doc.images.clear();
                    let scene = crate::scene::build_scene_for_export(&child, ppu);
                    errors.extend(scene.errors);
                    groups.extend(scene.content);
                }
                if !source.stack.is_empty() && source.opacity < 1.0 {
                    vec![Group::Composite { opacity: source.opacity, members: groups, mask: None }]
                } else {
                    groups
                }
            }
            NodeKind::Image(pid) => {
                if canvas && crate::view_depth_scene::outlined(ed, pid) {
                    return vec![];
                }
                let mut child = ed.clone();
                for node in &mut child.doc.nodes {
                    node.look = None;
                    node.role = GroupRole::Normal;
                    node.mask_child = None;
                }
                for p in &mut child.doc.paths {
                    p.hidden = true;
                    p.stack.clear();
                }
                child.doc.images.retain(|i| i.id == pid);
                let scene = crate::scene::build_scene_for_export(&child, ppu);
                errors.extend(scene.errors);
                scene.content
            }
            NodeKind::Text(_) => vec![], // text outlines are supplied by the existing text adapter
            _ => n
                .children
                .iter()
                .rev()
                .filter(|id| Some(**id) != n.mask_child)
                .flat_map(|id| walk(ed, *id, ppu, errors, canvas))
                .collect(),
        };
        if members.is_empty() {
            return vec![];
        }
        if let Some(mask) = n.mask_child {
            if n.role == GroupRole::MaskAlpha {
                members =
                    vec![Group::Composite { opacity: 1.0, members, mask: Some(walk(ed, mask, ppu, errors, canvas)) }];
            } else if n.role == GroupRole::Clip {
                let rings = ed
                    .doc
                    .node_paths(mask)
                    .iter()
                    .filter_map(|pid| ed.doc.pidx(*pid))
                    .flat_map(|pi| {
                        let g = crate::flatten::flatten_path(&ed.doc, pi, ppu);
                        std::iter::once(g.outline).chain(g.holes).collect::<Vec<_>>()
                    })
                    .collect();
                members = vec![Group::Clip { mask_rings: rings, members }];
            }
        }
        if let Some(look) = n.look {
            if look.opacity < 1.0 || (look.isolate && !n.role.is_mask_group()) {
                members = vec![Group::Composite { opacity: look.opacity, members, mask: None }];
            }
        }
        members
    }
    ed.doc.roots.iter().rev().flat_map(|id| walk(ed, *id, ppu, errors, canvas)).collect()
}

// Renderer/session cache only: never part of Document or the undo snapshots.
#[derive(Clone, Debug, Default)]
pub struct Cache(std::sync::Arc<std::sync::Mutex<Cached>>);
#[derive(Debug, Default)]
struct Cached {
    key: u64,
    groups: Vec<Group>,
    errors: Vec<String>,
    builds: u64,
}
impl Cache {
    pub fn builds(&self) -> u64 {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).builds
    }
}
pub fn compose_cached(ed: &Editor, ppu: f32, errors: &mut Vec<String>, canvas: bool) -> Vec<Group> {
    let mut key = std::collections::hash_map::DefaultHasher::new();
    if let Ok(bytes) = serde_json::to_vec(&ed.doc) {
        bytes.hash(&mut key);
    }
    crate::flatten::zoom_bucket(ppu).hash(&mut key);
    ed.select_transform.isolation.hash(&mut key);
    canvas.hash(&mut key);
    if canvas {
        ed.view_depth.hash(&mut key);
    }
    let key = key.finish();
    {
        let cached = ed.appearance_cache.0.lock().unwrap_or_else(|e| e.into_inner());
        if cached.builds > 0 && cached.key == key {
            errors.extend(cached.errors.clone());
            return cached.groups.clone();
        }
    }
    let mut own_errors = vec![];
    let groups = compose(ed, crate::flatten::bucket_ppu(crate::flatten::zoom_bucket(ppu)), &mut own_errors, canvas);
    errors.extend(own_errors.clone());
    fn points(groups: &[Group]) -> usize {
        groups
            .iter()
            .map(|g| match g {
                Group::Clip { mask_rings, members } => mask_rings.iter().map(Vec::len).sum::<usize>() + points(members),
                Group::Composite { members, mask, .. } => points(members) + mask.as_ref().map_or(0, |m| points(m)),
                _ => g
                    .prims()
                    .iter()
                    .map(|p| match p {
                        crate::Prim::Fill { rings, .. }
                        | crate::Prim::GradientFill { rings, .. }
                        | crate::Prim::StrokeCoverage { rings, .. } => rings.iter().map(Vec::len).sum(),
                        crate::Prim::Stroke { pts, .. } | crate::Prim::Dashed { pts, .. } => pts.len(),
                        _ => 4,
                    })
                    .sum(),
            })
            .sum()
    }
    let mut cached = ed.appearance_cache.0.lock().unwrap_or_else(|e| e.into_inner());
    cached.builds += 1;
    if points(&groups) <= 262_144 {
        cached.key = key;
        cached.groups = groups.clone();
        cached.errors = own_errors;
    } else {
        cached.key = 0;
        cached.groups.clear();
    }
    groups
}
