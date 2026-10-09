//! Every image mutation is a checked, undoable EditCommand.
use super::*;
use crate::{
    model::{GroupRole, Node, NodeKind, Path, Xform},
    Editor,
};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub enum ImageEdit {
    Rasterized { target: u32, image: ImageObject },
    ExpandTrace { id: u32, paths: Vec<Path> },
    Add { image: ImageObject, parent: Option<u32> },
    Transform { id: u32, xform: ImageAffine, opacity: f32 },
    Replace { id: u32, image: ImageObject },
    Mode { id: u32, mode: PlacementMode, link: Option<LinkInfo> },
    Delete { id: u32 },
    Crop { id: u32, bounds: [f32; 4] },
    EffectsPpi(f32),
}
fn leaf(id: u32, kind: NodeKind, parent: Option<u32>) -> Node {
    Node {
        id,
        kind,
        name: String::new(),
        parent,
        children: vec![],
        hidden: false,
        locked: false,
        color: None,
        clip_exempt: false,
        xform: Xform::default(),
        role: GroupRole::Normal,
        mask_child: None,
    }
}
pub fn check(ed: &Editor, edit: &ImageEdit) -> Result<(), String> {
    if ed.doc.ids > u32::MAX - 16 {
        return Err("Image edit has no id headroom".into());
    }
    match edit {
        ImageEdit::Add { parent, .. } => {
            let mut current = Some(parent.unwrap_or(ed.doc.active_layer));
            while let Some(id) = current {
                let n = ed.doc.node(id).ok_or("Image parent unavailable")?;
                if n.hidden || n.locked {
                    return Err("Image parent hidden or locked".into());
                }
                current = n.parent;
            }
        }
        ImageEdit::Rasterized { target, .. } => {
            let node = ed
                .doc
                .node_of_path(*target)
                .or_else(|| ed.doc.node(*target).map(|n| n.id))
                .ok_or("Rasterize target unavailable")?;
            let mut current = Some(node);
            while let Some(id) = current {
                let n = ed.doc.node(id).ok_or("Ancestor missing")?;
                if n.hidden || n.locked {
                    return Err("Rasterize target hidden or locked".into());
                }
                current = n.parent;
            }
            if ed.doc.is_mask_source(*target) || ed.doc.nodes.iter().any(|n| n.mask_child == Some(node)) {
                return Err("Cannot rasterize a mask source independently".into());
            }
        }
        ImageEdit::ExpandTrace { id, .. }
        | ImageEdit::Transform { id, .. }
        | ImageEdit::Replace { id, .. }
        | ImageEdit::Mode { id, .. }
        | ImageEdit::Delete { id }
        | ImageEdit::Crop { id, .. }
            if image_locked(&ed.doc, *id) || image_hidden(&ed.doc, *id) =>
        {
            return Err("Image is hidden or locked".into());
        }
        _ => {}
    }
    let mut doc = ed.doc.clone();
    perform(&mut doc, edit.clone(), &ed.blobs)?;
    validate(&doc)?;
    crate::format::validate(&doc, &crate::format::Limits::DEFAULT).map_err(|e| e.to_string())?;
    if doc.nodes.len() > crate::format::Limits::DEFAULT.max_nodes {
        return Err("Image edit exceeds node limit".into());
    }
    for i in &doc.images {
        let b = ed.blobs.get(&i.blob).ok_or("Image resource is unavailable")?;
        if i.placement == PlacementMode::Embed && b.original.is_none() {
            return Err("Cannot embed a proxy-only image".into());
        }
    }
    Ok(())
}
pub fn apply(ed: &mut Editor, edit: ImageEdit) {
    if check(ed, &edit).is_err() {
        return;
    }
    let mut doc = ed.doc.clone();
    if perform(&mut doc, edit, &ed.blobs).is_err() || doc.content_eq(&ed.doc) {
        return;
    }
    ed.begin();
    ed.doc = doc;
    ed.dirty = true;
    ed.commit();
}
fn perform(doc: &mut crate::model::Document, edit: ImageEdit, store: &BlobStore) -> Result<(), String> {
    match edit {
        ImageEdit::Rasterized { target, mut image } => {
            let id = doc
                .node_of_path(target)
                .or_else(|| doc.node(target).map(|n| n.id))
                .ok_or("Rasterize target unavailable")?;
            let old = doc.node(id).cloned().ok_or("Rasterize node unavailable")?;
            if old.kind == NodeKind::Layer {
                return Err("Rasterize a group or object, not a layer".into());
            }
            let mut gone = std::collections::HashSet::new();
            let mut pending = vec![id];
            while let Some(n) = pending.pop() {
                if gone.insert(n) {
                    if let Some(node) = doc.node(n) {
                        pending.extend(node.children.iter().copied());
                    }
                }
            }
            let paths: std::collections::HashSet<_> = doc
                .nodes
                .iter()
                .filter(|n| gone.contains(&n.id))
                .filter_map(|n| if let NodeKind::Path(p) = n.kind { Some(p) } else { None })
                .collect();
            let images: std::collections::HashSet<_> = doc
                .nodes
                .iter()
                .filter(|n| gone.contains(&n.id))
                .filter_map(|n| if let NodeKind::Image(p) = n.kind { Some(p) } else { None })
                .collect();
            doc.nodes.retain(|n| !gone.contains(&n.id));
            doc.paths.retain(|p| !paths.contains(&p.id));
            doc.images.retain(|i| !images.contains(&i.id));
            image.id = if let NodeKind::Image(pid) = old.kind { pid } else { doc.nid() };
            let mut node = leaf(id, NodeKind::Image(image.id), old.parent);
            node.name = old.name;
            doc.nodes.push(node);
            doc.images.push(image);
        }
        ImageEdit::ExpandTrace { id, paths } => {
            let old =
                doc.nodes.iter().find(|n| n.kind == NodeKind::Image(id)).cloned().ok_or("Image node unavailable")?;
            let limits = crate::format::Limits::DEFAULT;
            let count: usize =
                paths.iter().map(|p| p.anchors.len() + p.holes.iter().map(Vec::len).sum::<usize>()).sum();
            let required = count as u64 + paths.len() as u64 * 2;
            if paths.is_empty()
                || doc.paths.len() + paths.len() > limits.max_paths
                || doc.nodes.len() + paths.len() > limits.max_nodes
                || u64::from(doc.ids) + required >= u64::from(u32::MAX)
            {
                return Err("Trace exceeds document capacity".into());
            }
            let mut leaves = vec![];
            for mut p in paths {
                p.id = doc.nid();
                for a in p.anchors.iter_mut().chain(p.holes.iter_mut().flatten()) {
                    a.id = doc.nid();
                }
                let nid = doc.nid();
                leaves.push(nid);
                doc.nodes.push(leaf(nid, NodeKind::Path(p.id), old.parent));
                doc.paths.push(p);
            }
            for n in &mut doc.nodes {
                if let Some(at) = n.children.iter().position(|n| *n == old.id) {
                    n.children.splice(at..=at, leaves.iter().rev().copied());
                }
            }
            if let Some(at) = doc.roots.iter().position(|n| *n == old.id) {
                doc.roots.splice(at..=at, leaves.iter().rev().copied());
            }
            doc.nodes.retain(|n| n.id != old.id);
            doc.images.retain(|i| i.id != id);
        }
        ImageEdit::Add { mut image, parent } => {
            let parent = parent.unwrap_or(doc.active_layer);
            if !doc.node(parent).is_some_and(|n| matches!(n.kind, NodeKind::Layer | NodeKind::Group)) {
                return Err("Invalid image parent".into());
            }
            image.id = doc.nid();
            let id = doc.nid();
            doc.nodes.push(leaf(id, NodeKind::Image(image.id), Some(parent)));
            if let Some(n) = doc.nodes.iter_mut().find(|n| n.id == parent) {
                n.children.insert(0, id);
            }
            doc.images.push(image);
        }
        ImageEdit::EffectsPpi(ppi) => doc.raster_effects_ppi = ppi,
        ImageEdit::Transform { id, xform, opacity } => {
            let i = doc.images.iter_mut().find(|i| i.id == id).ok_or("Image not found")?;
            i.xform = xform;
            i.opacity = opacity;
        }
        ImageEdit::Replace { id, mut image } => {
            let old = doc.images.iter_mut().find(|i| i.id == id).ok_or("Image not found")?;
            image.id = id;
            image.opacity = old.opacity;
            image.replacement = old.replacement;
            image.xform = old.xform;
            if old.replacement == ReplacementPolicy::KeepBounds {
                image.xform.a *= old.px_w as f32 / image.px_w as f32;
                image.xform.b *= old.px_w as f32 / image.px_w as f32;
                image.xform.c *= old.px_h as f32 / image.px_h as f32;
                image.xform.d *= old.px_h as f32 / image.px_h as f32;
            }
            *old = image;
        }
        ImageEdit::Mode { id, mode, link } => {
            let i = doc.images.iter_mut().find(|i| i.id == id).ok_or("Image not found")?;
            i.placement = mode;
            i.link = link;
        }
        ImageEdit::Delete { id } => {
            if !doc.images.iter().any(|i| i.id == id) {
                return Err("Image not found".into());
            }
            let nodes: Vec<_> = doc.nodes.iter().filter(|n| n.kind == NodeKind::Image(id)).map(|n| n.id).collect();
            doc.images.retain(|i| i.id != id);
            doc.nodes.retain(|n| !nodes.contains(&n.id));
            for n in &mut doc.nodes {
                n.children.retain(|n| !nodes.contains(n));
            }
            doc.roots.retain(|n| !nodes.contains(n));
        }
        ImageEdit::Crop { id, bounds: [x, y, w, h] } => {
            if ![x, y, w, h, x + w, y + h].iter().all(|v| v.is_finite()) || w <= 0. || h <= 0. {
                return Err("Invalid crop bounds".into());
            }
            let image_node =
                doc.nodes.iter().find(|n| n.kind == NodeKind::Image(id)).cloned().ok_or("Image not found")?;
            let group = doc.nid();
            let pid = doc.nid();
            let mask = doc.nid();
            let mut corners = [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
            let mut chain = vec![];
            let mut node = image_node.parent;
            while let Some(id) = node {
                let n = doc.node(id).ok_or("Crop ancestor unavailable")?;
                chain.push(n.xform);
                node = n.parent;
            }
            for xf in chain.into_iter().rev() {
                corners = corners.map(|p| xf.inverse_apply(p));
            }
            let anchors = corners
                .into_iter()
                .map(|p| crate::model::Anchor { id: doc.nid(), p, hin: None, hout: None, smooth: false })
                .collect();
            doc.paths.push(Path::new(pid, anchors, true, Some([1.; 4]), None, 0.));
            let mut g = leaf(group, NodeKind::Group, image_node.parent);
            g.name = "Crop".into();
            g.children = vec![mask, image_node.id];
            g.role = GroupRole::Clip;
            g.mask_child = Some(mask);
            for n in &mut doc.nodes {
                if n.id == image_node.id {
                    n.parent = Some(group);
                } else {
                    for c in &mut n.children {
                        if *c == image_node.id {
                            *c = group;
                        }
                    }
                }
            }
            for r in &mut doc.roots {
                if *r == image_node.id {
                    *r = group;
                }
            }
            doc.nodes.push(g);
            doc.nodes.push(leaf(mask, NodeKind::Path(pid), Some(group)));
        }
    }
    doc.assets.retain(|a| doc.images.iter().any(|i| i.blob == a.key));
    for i in &doc.images {
        if !doc.assets.iter().any(|a| a.key == i.blob) {
            let b = store.get(&i.blob).ok_or("Image resource is unavailable")?;
            doc.assets.push(b.meta.clone());
        }
    }
    Ok(())
}
/// Staging resources does not publish a node; callers must roll back the store on refusal.
pub fn stage(
    ed: &mut Editor,
    decoded: codec::Decoded,
    at: [f32; 2],
    mode: PlacementMode,
    link: Option<LinkInfo>,
) -> Result<ImageObject, String> {
    let meta = decoded.blob.meta.clone();
    let ppi = decoded.ppi;
    ed.blobs.insert(decoded.blob)?;
    Ok(ImageObject {
        id: 0,
        blob: meta.key,
        px_w: meta.px_w,
        px_h: meta.px_h,
        ppi,
        xform: ImageAffine { a: 72. / ppi[0], b: 0., c: 0., d: 72. / ppi[1], e: at[0], f: at[1] },
        opacity: 1.,
        placement: mode,
        link,
        replacement: ReplacementPolicy::KeepBounds,
    })
}
