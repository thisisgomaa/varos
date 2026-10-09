//! Lane A: bounded, undoable appearance edits and the one stored mask form.
use crate::{
    appearance::{base_stack, BaseSlot, EntryOpts, Look, StackItem},
    model::{Document, GroupRole, Node, NodeKind, Paint, Path, Xform},
    Editor,
};
use serde::{Deserialize, Serialize};

pub const MAX_ENTRIES: usize = 64;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum AppearanceEdit {
    SetStack { path: u32, stack: Vec<StackItem> },
    AddFill { path: u32, paint: Paint },
    AddStroke { path: u32, paint: Paint, width: f32 },
    SetEntry { path: u32, index: usize, paint: Paint, opacity: f32, visible: bool },
    Reorder { path: u32, from: usize, to: usize },
    Delete { path: u32, index: usize },
    SetLook { node: u32, look: Option<Look> },
    Expand { path: u32 },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum MaskEdit {
    /// Existing vector subtree becomes the mask, wrapping any content row.
    Add {
        node: u32,
        mask: u32,
        alpha: bool,
    },
    /// Empty container for drawing: its mask child is a group, never a Path field.
    Begin {
        node: u32,
        alpha: bool,
    },
    Mode {
        node: u32,
        alpha: bool,
    },
    Release {
        node: u32,
    },
}
fn unit(v: f32) -> bool {
    v.is_finite() && (0.0..=1.0).contains(&v)
}
pub fn validate_stack(path: &Path, stack: &[StackItem]) -> Result<(), String> {
    if stack.is_empty() {
        return Ok(());
    }
    if stack.len() > MAX_ENTRIES {
        return Err("limit_exceeded: at most 64 appearance entries".into());
    }
    for slot in [BaseSlot::Fill, BaseSlot::Stroke] {
        if stack.iter().filter(|e| matches!(e, StackItem::Base(s, _) if *s == slot)).count() != 1 {
            return Err("appearance requires exactly one base fill and base stroke".into());
        }
    }
    for entry in stack {
        if !unit(entry.opts().opacity) {
            return Err("entry opacity must be finite, between 0 and 1".into());
        }
        let paint = match entry {
            StackItem::Base(BaseSlot::Fill, _) => &path.fill,
            StackItem::Base(BaseSlot::Stroke, _) => &path.stroke,
            StackItem::Fill { paint, .. } => paint,
            StackItem::Stroke { paint, style, width, .. } => {
                if !width.is_finite() || *width < 0.0 {
                    return Err("invalid entry stroke width".into());
                }
                style.validate(path.id).map_err(|e| e.to_string())?;
                paint
            }
        };
        match paint {
            Paint::Solid(c) if !c.iter().all(|v| unit(*v)) => return Err("invalid appearance colour".into()),
            Paint::Gradient(g) => g.validate().map_err(|e| e.to_string())?,
            _ => {}
        }
    }
    Ok(())
}
pub fn validate_document(doc: &Document) -> Result<(), String> {
    for p in &doc.paths {
        validate_stack(p, &p.stack)?;
        for entry in &p.stack {
            if let StackItem::Fill { paint, .. } | StackItem::Stroke { paint, .. } = entry {
                crate::swatches::validate_paint(paint, doc)?;
            }
        }
    }
    for n in &doc.nodes {
        if let Some(look) = n.look {
            if !matches!(n.kind, NodeKind::Layer | NodeKind::Group) || !unit(look.opacity) {
                return Err(format!("node {} look requires a container and finite opacity 0..1", n.id));
            }
        }
    }
    Ok(())
}
fn path_id(edit: &AppearanceEdit) -> Option<u32> {
    match edit {
        AppearanceEdit::SetLook { .. } => None,
        AppearanceEdit::SetStack { path, .. }
        | AppearanceEdit::AddFill { path, .. }
        | AppearanceEdit::AddStroke { path, .. }
        | AppearanceEdit::SetEntry { path, .. }
        | AppearanceEdit::Reorder { path, .. }
        | AppearanceEdit::Delete { path, .. }
        | AppearanceEdit::Expand { path } => Some(*path),
    }
}
fn changed_path(p: &Path, edit: &AppearanceEdit) -> Result<Path, String> {
    let mut next = p.clone();
    if next.stack.is_empty() {
        next.stack = base_stack();
    }
    match edit {
        AppearanceEdit::SetStack { stack, .. } => next.stack = stack.clone(),
        AppearanceEdit::AddFill { paint, .. } => {
            next.stack.push(StackItem::Fill { paint: paint.clone(), opts: EntryOpts::default() })
        }
        AppearanceEdit::AddStroke { paint, width, .. } => {
            if !width.is_finite() || *width < 0.0 {
                return Err("stroke width must be finite and nonnegative".into());
            }
            // Each extra stroke owns its width in points and a complete style.
            next.stack.push(StackItem::Stroke {
                paint: paint.clone(),
                width: *width,
                style: next.stroke_style.clone(),
                opts: EntryOpts::default(),
            });
        }
        AppearanceEdit::SetEntry { index, paint, opacity, visible, .. } => {
            let entry = next.stack.get_mut(*index).ok_or("unknown appearance entry")?;
            entry.opts_mut().opacity = *opacity;
            entry.opts_mut().visible = *visible;
            match entry {
                StackItem::Base(BaseSlot::Fill, _) => next.fill = paint.clone(),
                StackItem::Base(BaseSlot::Stroke, _) => next.stroke = paint.clone(),
                StackItem::Fill { paint: p, .. } | StackItem::Stroke { paint: p, .. } => *p = paint.clone(),
            }
        }
        AppearanceEdit::Reorder { from, to, .. } => {
            if *from >= next.stack.len() || *to >= next.stack.len() {
                return Err("unknown appearance entry".into());
            }
            let entry = next.stack.remove(*from);
            next.stack.insert(*to, entry);
        }
        AppearanceEdit::Delete { index, .. } => {
            if matches!(next.stack.get(*index), Some(StackItem::Base(_, _))) {
                return Err("base entries cannot be deleted; set their paint to none".into());
            }
            if *index >= next.stack.len() {
                return Err("unknown appearance entry".into());
            }
            next.stack.remove(*index);
        }
        _ => {}
    }
    validate_stack(&next, &next.stack)?;
    if next.stack == base_stack() {
        next.stack.clear();
    }
    Ok(next)
}
pub fn check(ed: &Editor, edit: &AppearanceEdit) -> Result<(), String> {
    if let Some(id) = path_id(edit) {
        let p = ed.doc.pidx(id).map(|i| &ed.doc.paths[i]).ok_or("unknown path")?;
        if ed.doc.eff_locked(id) || ed.doc.eff_hidden(id) {
            return Err("path is hidden or locked".into());
        }
        let changed = changed_path(p, edit)?;
        crate::swatches::validate_paint(&changed.fill, &ed.doc)?;
        crate::swatches::validate_paint(&changed.stroke, &ed.doc)?;
        for entry in &changed.stack {
            if let StackItem::Fill { paint, .. } | StackItem::Stroke { paint, .. } = entry {
                crate::swatches::validate_paint(paint, &ed.doc)?;
            }
        }
    } else if let AppearanceEdit::SetLook { node, look } = edit {
        let n = ed.doc.node(*node).ok_or("unknown node")?;
        if !matches!(n.kind, NodeKind::Group | NodeKind::Layer) || look.is_some_and(|l| !unit(l.opacity)) {
            return Err("look requires container, opacity 0..1".into());
        }
        if node_locked(&ed.doc, *node) || node_hidden(&ed.doc, *node) {
            return Err("node is hidden or locked".into());
        }
    }
    Ok(())
}
pub fn apply(ed: &mut Editor, edit: AppearanceEdit) {
    if let Err(e) = check(ed, &edit) {
        ed.last_error = Some(crate::EngineError::Internal { what: e });
        return;
    }
    let mut next = ed.doc.clone();
    match &edit {
        AppearanceEdit::SetLook { node, look } => {
            if let Some(n) = next.node_mut(*node) {
                n.look = *look;
            }
        }
        AppearanceEdit::Expand { path } => {
            if let Some(i) = next.pidx(*path) {
                let original = next.paths[i].clone();
                if original.stack.is_empty() {
                    return;
                }
                let Some(node) = next.node_of_path(*path) else { return };
                let entries = crate::appearance_scene::paint_paths(&original);
                let mut children = vec![];
                for mut p in entries {
                    let id = next.nid();
                    p.id = id;
                    for a in p.anchors.iter_mut().chain(p.holes.iter_mut().flatten()) {
                        a.id = next.nid();
                    }
                    let leaf = next.nid();
                    let mut n = container(leaf, Some(node), "");
                    n.kind = NodeKind::Path(id);
                    next.nodes.push(n);
                    children.insert(0, leaf);
                    next.paths.push(p);
                }
                if let Some(n) = next.node_mut(node) {
                    n.kind = NodeKind::Group;
                    n.children = children;
                    n.look = Some(Look { opacity: original.opacity, isolate: true });
                    n.name = "Expanded Appearance".into();
                }
                next.paths.retain(|p| p.id != *path);
                next.sync_tree();
            }
        }
        _ => {
            if let Some(i) = path_id(&edit).and_then(|id| next.pidx(id)) {
                if let Ok(p) = changed_path(&next.paths[i], &edit) {
                    next.paths[i] = p;
                }
            }
        }
    }
    if next == ed.doc {
        return;
    }
    let own = !ed.transaction_open();
    if own {
        ed.begin();
    }
    ed.doc = next;
    ed.dirty = true;
    if own {
        ed.commit();
    }
}
fn mask_node(edit: &MaskEdit) -> u32 {
    match edit {
        MaskEdit::Add { node, .. }
        | MaskEdit::Begin { node, .. }
        | MaskEdit::Mode { node, .. }
        | MaskEdit::Release { node } => *node,
    }
}
fn check_mask_shape(ed: &Editor, edit: &MaskEdit) -> Result<(), String> {
    let id = mask_node(edit);
    let n = ed.doc.node(id).ok_or("unknown content row")?;
    if node_locked(&ed.doc, id) || node_hidden(&ed.doc, id) {
        return Err("row is hidden or locked".into());
    }
    if matches!(edit, MaskEdit::Add { .. } | MaskEdit::Begin { .. })
        && n.kind == NodeKind::Layer
        && n.children.iter().any(|id| ed.doc.node(*id).is_some_and(|n| n.kind == NodeKind::Layer))
    {
        return Err("mask the contents of each sublayer separately".into());
    }
    match edit {
        MaskEdit::Add { mask, .. } => {
            let m = ed.doc.node(*mask).ok_or("unknown mask row")?;
            if m.kind == NodeKind::Layer {
                return Err("mask source must be a vector or group, not a layer".into());
            }
            if *mask == id
                || ed.doc.node_paths(*mask).is_empty()
                || node_locked(&ed.doc, *mask)
                || node_hidden(&ed.doc, *mask)
            {
                return Err("mask must be a separate editable vector subtree".into());
            }
            if m.parent.and_then(|id| ed.doc.node(id)).is_some_and(|p| p.mask_child == Some(*mask)) {
                return Err("move the mask group, not its authoritative mask child".into());
            }
            let mut parent = n.parent;
            while let Some(p) = parent {
                if p == *mask {
                    return Err("mask cannot contain content".into());
                }
                parent = ed.doc.node(p).and_then(|n| n.parent);
            }
            let mut parent = m.parent;
            while let Some(p) = parent {
                if p == id {
                    return Err("content cannot contain mask".into());
                }
                parent = ed.doc.node(p).and_then(|n| n.parent);
            }
        }
        MaskEdit::Mode { .. } | MaskEdit::Release { .. } if !n.role.is_mask_group() => {
            return Err("row is not a mask group".into())
        }
        _ => {}
    }
    Ok(())
}
fn container(id: u32, parent: Option<u32>, name: &str) -> Node {
    Node {
        id,
        kind: NodeKind::Group,
        name: name.into(),
        parent,
        children: vec![],
        hidden: false,
        locked: false,
        color: None,
        clip_exempt: false,
        xform: Xform::default(),
        role: GroupRole::Normal,
        mask_child: None,
        look: None,
    }
}
/// Selecting a mask row restores its authoritative drawing child.
pub fn drawing_target(doc: &Document, node: u32) -> u32 {
    doc.node(node)
        .filter(|n| n.role.is_mask_group())
        .and_then(|n| n.mask_child)
        .filter(|id| doc.node(*id).is_some_and(|n| n.kind == NodeKind::Group))
        .unwrap_or_else(|| doc.layer_ancestor(node))
}
fn staged_mask(ed: &Editor, edit: &MaskEdit) -> Result<Editor, String> {
    check_mask_shape(ed, edit)?;
    let mut staged = ed.clone();
    mutate_mask(&mut staged, edit.clone());
    crate::format::check_structure(&staged.doc, &crate::format::Limits::DEFAULT).map_err(|e| e.to_string())?;
    crate::format::validate(&staged.doc, &crate::format::Limits::DEFAULT).map_err(|e| e.to_string())?;
    Ok(staged)
}
pub fn check_mask(ed: &Editor, edit: &MaskEdit) -> Result<(), String> {
    staged_mask(ed, edit).map(|_| ())
}
pub fn apply_mask(ed: &mut Editor, edit: MaskEdit) {
    let staged = match staged_mask(ed, &edit) {
        Ok(staged) => staged,
        Err(e) => {
            ed.last_error = Some(crate::EngineError::Internal { what: e });
            return;
        }
    };
    let own = !ed.transaction_open();
    if own {
        ed.begin();
    }
    ed.doc = staged.doc;
    ed.group_sel = staged.group_sel;
    ed.dirty = true;
    if own {
        ed.commit();
    }
}
fn mutate_mask(ed: &mut Editor, edit: MaskEdit) {
    let node = mask_node(&edit);
    match edit {
        MaskEdit::Mode { alpha, .. } => {
            if let Some(n) = ed.doc.node_mut(node) {
                n.role = if alpha { GroupRole::MaskAlpha } else { GroupRole::Clip };
                if !alpha && n.look.is_none() {
                    n.look = Some(Look { isolate: true, ..Default::default() });
                }
            }
        }
        MaskEdit::Release { .. } => {
            ed.doc.release_clip(node);
        }
        MaskEdit::Add { mask, alpha, .. } => {
            wrap(ed, node, mask, alpha);
        }
        MaskEdit::Begin { alpha, .. } => {
            let mask = ed.doc.nid();
            ed.doc.nodes.push(container(mask, None, "Mask — draw inside"));
            wrap(ed, node, mask, alpha);
            // Drawing tools read active_layer; target the mask container until another row is selected.
            ed.doc.active_layer = mask;
        }
    }
}
fn wrap(ed: &mut Editor, content: u32, mask: u32, alpha: bool) {
    // Reparenting makes transforms nested; freeze the former world-space units first.
    let mut baked = std::collections::HashSet::new();
    for node in [content, mask] {
        for pid in crate::images::node_items(&ed.doc, node) {
            if ed.doc.unit_of(pid).is_some_and(|unit| baked.insert(unit)) {
                ed.bake_unit_of(pid);
            }
        }
    }
    let Some(mut n) = ed.doc.node(content).cloned() else { return };
    let content = if n.kind == NodeKind::Layer {
        // Layers stay layers; the authoritative mask group is their sole content row.
        let inner = ed.doc.nid();
        let mut group = container(inner, Some(content), "Masked contents");
        group.children = n.children.clone();
        for child in &group.children {
            if let Some(node) = ed.doc.node_mut(*child) {
                node.parent = Some(inner);
            }
        }
        if let Some(layer) = ed.doc.node_mut(content) {
            layer.children = vec![inner];
        }
        n = group.clone();
        ed.doc.nodes.push(group);
        inner
    } else {
        content
    };
    let id = ed.doc.nid();
    if let Some(parent) = ed.doc.node(mask).and_then(|n| n.parent) {
        if let Some(p) = ed.doc.node_mut(parent) {
            p.children.retain(|c| *c != mask);
        }
    } else {
        ed.doc.roots.retain(|c| *c != mask);
    }
    if let Some(parent) = n.parent {
        if let Some(p) = ed.doc.node_mut(parent) {
            for c in &mut p.children {
                if *c == content {
                    *c = id;
                }
            }
            if p.mask_child == Some(content) {
                p.mask_child = Some(id);
            }
        }
    } else {
        for c in &mut ed.doc.roots {
            if *c == content {
                *c = id;
            }
        }
    }
    let mut group = container(id, n.parent, if alpha { "Alpha mask" } else { "Clip mask" });
    group.children = vec![mask, content];
    group.role = if alpha { GroupRole::MaskAlpha } else { GroupRole::Clip };
    group.mask_child = Some(mask);
    // New clip masks opt into recursive composition; v9 clips without looks retain their bytes/semantics.
    if !alpha {
        group.look = Some(Look { isolate: true, ..Default::default() });
    }
    ed.doc.nodes.push(group);
    for child in [content, mask] {
        if let Some(n) = ed.doc.node_mut(child) {
            n.parent = Some(id);
        }
    }
    ed.group_sel.clear();
    ed.group_sel.insert(id);
}

fn node_flag(doc: &Document, id: u32, locked: bool) -> bool {
    let mut next = Some(id);
    while let Some(id) = next {
        let Some(n) = doc.node(id) else { return true };
        if if locked { n.locked } else { n.hidden } {
            return true;
        }
        next = n.parent;
    }
    false
}
fn node_locked(doc: &Document, id: u32) -> bool {
    node_flag(doc, id, true)
}
fn node_hidden(doc: &Document, id: u32) -> bool {
    node_flag(doc, id, false)
}
