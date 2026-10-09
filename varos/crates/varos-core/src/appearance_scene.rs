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
/// Compose the tree once; leaf evaluation shares the scene's culling, geometry caches and budgets.
pub fn compose(
    ed: &Editor,
    leaf: &impl Fn(NodeKind) -> Vec<Group>,
    mask_rings: &impl Fn(u32) -> Vec<Vec<crate::Pt>>,
) -> Vec<Group> {
    fn walk(
        ed: &Editor,
        id: u32,
        leaf: &impl Fn(NodeKind) -> Vec<Group>,
        mask_rings: &impl Fn(u32) -> Vec<Vec<crate::Pt>>,
    ) -> Vec<Group> {
        let Some(n) = ed.doc.node(id) else { return vec![] };
        if n.hidden {
            return vec![];
        }
        let mut members = match n.kind {
            NodeKind::Path(_) | NodeKind::Image(_) | NodeKind::Text(_) => leaf(n.kind),
            _ => n
                .children
                .iter()
                .rev()
                .filter(|id| Some(**id) != n.mask_child)
                .flat_map(|id| walk(ed, *id, leaf, mask_rings))
                .collect(),
        };
        if members.is_empty() {
            return vec![];
        }
        if let Some(mask) = n.mask_child {
            if n.role == GroupRole::MaskAlpha {
                members =
                    vec![Group::Composite { opacity: 1.0, members, mask: Some(walk(ed, mask, leaf, mask_rings)) }];
            } else if n.role == GroupRole::Clip {
                let rings = mask_rings(id);
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
    ed.doc.roots.iter().rev().flat_map(|id| walk(ed, *id, leaf, mask_rings)).collect()
}
