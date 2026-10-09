//! Slice 4A: baked affine edits, transaction previews and transient selection scope.
// Adapted from VectorCraft tools/src/xform/transform.rs, wand.rs and
// engine/src/cmd/layerpanel.rs@a469568 (MIT OR Apache-2.0), ArtCraft Team 2026.
use crate::{
    editor::Editor,
    geom::Pt,
    model::{Document, DropPos, GroupRole, Node, NodeKind, Xform},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Transform {
    pub scale: Pt,
    pub movement: Pt,
    pub angle: f32,
    /// Axis angle in screen coordinates, degrees; None leaves handedness unchanged.
    pub reflect: Option<f32>,
    pub shear: f32,
    pub shear_axis: f32,
    pub origin: Option<Pt>,
    pub each: bool,
    pub random: bool,
    pub seed: u32,
    pub copy: bool,
}
impl Default for Transform {
    fn default() -> Self {
        Self {
            scale: [1., 1.],
            movement: [0., 0.],
            angle: 0.,
            reflect: None,
            shear: 0.,
            shear_axis: 0.,
            origin: None,
            each: false,
            random: false,
            seed: 1,
            copy: false,
        }
    }
}
impl Transform {
    pub fn check(&self) -> Result<(), String> {
        let mut values = vec![
            self.scale[0],
            self.scale[1],
            self.movement[0],
            self.movement[1],
            self.angle,
            self.shear,
            self.shear_axis,
        ];
        values.extend(self.reflect);
        values.extend(self.origin.into_iter().flatten());
        if values.iter().any(|v| !v.is_finite() || v.abs() > 1.0e6)
            || self.scale.iter().any(|s| s.abs() < 1.0e-4)
            || self.shear.abs() >= 89.9
        {
            return Err(
                "invalid transform: finite bounded values, nonzero scale and shear below 89.9 degrees required".into(),
            );
        }
        Ok(())
    }
    fn map(self, p: Pt, origin: Pt) -> Pt {
        let rotate = |p: Pt, a: f32| {
            let (s, c) = a.to_radians().sin_cos();
            [c * p[0] - s * p[1], s * p[0] + c * p[1]]
        };
        let mut q = [(p[0] - origin[0]) * self.scale[0], (p[1] - origin[1]) * self.scale[1]];
        if let Some(a) = self.reflect {
            q = rotate(q, -a);
            q[1] = -q[1];
            q = rotate(q, a);
        }
        q = rotate(q, -self.shear_axis);
        q[0] += self.shear.to_radians().tan() * q[1];
        q = rotate(q, self.shear_axis + self.angle);
        [q[0] + origin[0] + self.movement[0], q[1] + origin[1] + self.movement[1]]
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PickOptions {
    pub fill: bool,
    pub stroke: bool,
    pub weight: bool,
    pub opacity: bool,
}
impl Default for PickOptions {
    fn default() -> Self {
        Self { fill: true, stroke: true, weight: true, opacity: true }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WandOptions {
    pub pick: PickOptions,
    pub colour: f32,
    pub weight: f32,
    pub opacity: f32,
}
impl Default for WandOptions {
    fn default() -> Self {
        Self {
            pick: PickOptions { fill: true, stroke: false, weight: false, opacity: false },
            colour: 0.,
            weight: 0.,
            opacity: 0.,
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectMode {
    Set,
    Add,
    Subtract,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerAction {
    ReleaseSequence,
    ReleaseBuild,
    Collect,
    Merge,
    Flatten,
    Locate,
    HideOthers,
    LockOthers,
}
#[derive(Default)]
pub struct State {
    pub isolation: Option<u32>,
    pub selection_requested: bool,
    pub options_requested: bool,
    pub located: Option<u32>,
    pub wand: WandOptions,
    pub pick: PickOptions,
    pub dialog: Option<crate::editor::ToolKind>,
    pub preview: Option<(Document, Vec<u32>)>,
    pub down: Option<(Pt, bool)>,
    pub free_shear: Option<(Pt, f32, f32)>,
}
impl Editor {
    pub fn in_isolation(&self, pid: u32) -> bool {
        let Some(scope) = self.select_transform.isolation else { return true };
        let mut at = self.doc.node_of_path(pid);
        for _ in 0..4096 {
            let Some(id) = at else { return false };
            if id == scope {
                return true;
            }
            at = self.doc.node(id).and_then(|n| n.parent);
        }
        false
    }
    pub fn isolate(&mut self, node: Option<u32>) {
        if node.is_some_and(|n| self.doc.node(n).is_none_or(|n| n.kind != NodeKind::Group)) {
            return;
        }
        self.select_transform.isolation = node;
        self.select_transform.selection_requested = true;
        self.escape_selection();
    }
    pub fn escape_selection(&mut self) {
        self.objsel.clear();
        self.selected.clear();
        self.group_sel.clear();
        self.dsel_path = None;
        self.active = None;
        self.drag = crate::editor::Drag::None;
    }
    pub fn transform_begin(&mut self) {
        if self.transaction_open() || self.select_transform.preview.is_some() {
            return;
        }
        self.begin();
        let mut ids: Vec<_> = self.selected_pids().into_iter().collect();
        ids.sort_unstable();
        self.select_transform.preview = Some((self.doc.clone(), ids));
    }
    pub fn transform_live(&mut self, spec: Transform) {
        if spec.check().is_err() {
            return;
        }
        let Some((doc, ids)) = self.select_transform.preview.clone() else { return };
        self.doc = doc;
        self.objsel = ids.into_iter().collect();
        self.transform_geometry(spec);
    }
    pub fn transform_end(&mut self, cancel: bool) {
        let Some((doc, ids)) = self.select_transform.preview.take() else { return };
        if cancel {
            self.doc = doc;
            self.objsel = ids.into_iter().collect();
            self.dirty = false;
        } else {
            self.dirty = self.doc != doc;
        }
        self.commit();
        self.select_transform.dialog = None;
        self.refresh_obj_angle();
    }
    pub fn transform_edit(&mut self, spec: Transform) {
        if spec.check().is_err() || self.selected_pids().is_empty() || self.transaction_open() {
            return;
        }
        self.begin();
        self.transform_geometry(spec);
        self.commit();
    }
    fn transform_geometry(&mut self, spec: Transform) {
        if !spec.copy
            && spec.scale == [1., 1.]
            && spec.movement == [0., 0.]
            && spec.angle == 0.
            && spec.reflect.is_none()
            && spec.shear == 0.
        {
            self.dirty = false;
            return;
        }
        let mut ids: Vec<_> = self.selected_pids().into_iter().collect();
        ids.sort_unstable();
        ids.retain(|p| self.in_isolation(*p) && !self.doc.eff_hidden(*p) && !self.doc.eff_locked(*p));
        if ids.is_empty() {
            return;
        }
        let origin =
            spec.origin.or_else(|| self.obj_bbox().map(|b| [(b.0 + b.2) * 0.5, (b.1 + b.3) * 0.5])).unwrap_or([0., 0.]);
        let mut units: Vec<_> = ids.iter().filter_map(|p| self.doc.unit_of(*p)).collect();
        units.sort_unstable();
        units.dedup();
        let mut origins = std::collections::BTreeMap::new();
        let mut randoms = std::collections::BTreeMap::new();
        let mut seed = spec.seed;
        for unit in &units {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let r = (seed >> 8) as f32 / 16777215.;
            let mut b = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            let members: Vec<_> = self.doc.node_paths(*unit).into_iter().filter(|p| ids.contains(p)).collect();
            for pid in &members {
                if let Some(i) = self.doc.pidx(*pid) {
                    let q = self.doc.outline_bbox(i);
                    b = (b.0.min(q.0), b.1.min(q.1), b.2.max(q.2), b.3.max(q.3));
                }
            }
            for pid in members {
                origins.insert(pid, [(b.0 + b.2) * 0.5, (b.1 + b.3) * 0.5]);
                randoms.insert(pid, r);
            }
        }
        if spec.copy {
            let active = self.select_transform.isolation.unwrap_or(self.doc.active_layer);
            let source = ids.clone();
            ids = self.doc.dup_paths(&ids);
            let mut copied_units: Vec<_> = ids.iter().filter_map(|p| self.doc.unit_of(*p)).collect();
            copied_units.sort_unstable();
            copied_units.dedup();
            for unit in copied_units {
                self.doc.move_node_to(unit, active, DropPos::Into);
            }
            for (old, new) in source.into_iter().zip(&ids) {
                if let Some(o) = origins.get(&old).copied() {
                    origins.insert(*new, o);
                    if let Some(r) = randoms.get(&old).copied() {
                        randoms.insert(*new, r);
                    }
                }
            }
            self.objsel = ids.iter().copied().collect();
            self.group_sel.clear();
        }
        // Bake only the units being edited. A Copy retains the originals' stored transforms.
        let mut edit_units: Vec<_> = ids.iter().filter_map(|p| self.doc.unit_of(*p)).collect();
        edit_units.sort_unstable();
        edit_units.dedup();
        for unit in edit_units {
            if let Some(pid) = self.doc.node_paths(unit).first() {
                self.bake_unit_of(*pid);
            }
        }
        for pid in ids {
            let Some(pi) = self.doc.pidx(pid) else { continue };
            let o = if spec.each { origins.get(&pid).copied().unwrap_or(origin) } else { origin };
            let mut s = spec;
            if spec.random {
                let r = randoms.get(&pid).copied().unwrap_or(1.);
                s.scale = [1. + (s.scale[0] - 1.) * r, 1. + (s.scale[1] - 1.) * r];
                s.movement = s.movement.map(|v| v * r);
                s.angle *= r;
                s.shear *= r;
            }
            let path = &mut self.doc.paths[pi];
            for a in path.anchors.iter_mut().chain(path.holes.iter_mut().flatten()) {
                a.p = s.map(a.p, o);
                a.hin = a.hin.map(|p| s.map(p, o));
                a.hout = a.hout.map(|p| s.map(p, o));
            }
        }
        self.refresh_obj_angle();
        self.dirty = true;
    }
    pub fn magic_wand(&mut self, source: u32, options: WandOptions, mode: SelectMode) {
        let Some(p) = self.doc.paths.iter().find(|p| p.id == source) else { return };
        let close = |a: crate::model::Paint, b: crate::model::Paint| match (a.solid(), b.solid()) {
            (None, None) => true,
            (Some(a), Some(b)) => a.iter().zip(b).all(|(x, y)| (*x - y).abs() <= options.colour),
            _ => false,
        };
        let ids: Vec<_> = self
            .doc
            .paths
            .iter()
            .filter(|q| {
                self.in_isolation(q.id)
                    && !self.doc.eff_hidden(q.id)
                    && !self.doc.eff_locked(q.id)
                    && (!options.pick.fill || close(p.fill, q.fill))
                    && (!options.pick.stroke || close(p.stroke, q.stroke))
                    && (!options.pick.weight || (p.stroke_width - q.stroke_width).abs() <= options.weight)
                    && (!options.pick.opacity || (p.opacity - q.opacity).abs() <= options.opacity)
            })
            .map(|p| p.id)
            .collect();
        self.select_transform.selection_requested = true;
        if matches!(mode, SelectMode::Set) {
            self.escape_selection();
        }
        for p in ids {
            if matches!(mode, SelectMode::Subtract) {
                self.objsel.remove(&p);
            } else {
                self.objsel.insert(p);
            }
        }
        self.refresh_obj_angle();
    }
    pub fn sample_options(&mut self, source: u32, pick: PickOptions, colour_only: bool) {
        let Some(p) = self.doc.paths.iter().find(|p| p.id == source).cloned() else { return };
        let ids = self.selected_pids();
        if colour_only {
            let colour = p.fill.solid().or(p.stroke.solid());
            self.apply_paint(colour);
            return;
        }
        if pick.fill {
            self.cur_fill = p.fill.solid();
        }
        if pick.stroke {
            self.cur_stroke = p.stroke.solid();
        }
        if pick.weight {
            self.cur_sw = p.stroke_width;
        }
        let changed = ids.iter().filter_map(|id| self.doc.pidx(*id)).any(|i| {
            let q = &self.doc.paths[i];
            (pick.fill && q.fill != p.fill)
                || (pick.stroke && q.stroke != p.stroke)
                || (pick.weight && q.stroke_width != p.stroke_width)
                || (pick.opacity && q.opacity != p.opacity)
        });
        if !changed {
            return;
        }
        let open = self.transaction_open();
        if !open {
            self.begin();
        }
        for id in ids {
            if let Some(i) = self.doc.pidx(id) {
                let q = &mut self.doc.paths[i];
                if pick.fill {
                    q.fill = p.fill;
                }
                if pick.stroke {
                    q.stroke = p.stroke;
                }
                if pick.weight {
                    q.stroke_width = p.stroke_width;
                }
                if pick.opacity {
                    q.opacity = p.opacity;
                }
            }
        }
        self.dirty = true;
        if !open {
            self.commit();
        }
    }
    pub fn layer_family(&mut self, action: LayerAction, nodes: Vec<u32>) {
        if matches!(action, LayerAction::Locate) {
            self.select_transform.located = nodes.first().copied();
            return;
        }
        if nodes.is_empty() && !matches!(action, LayerAction::Flatten) {
            return;
        }
        if matches!(action, LayerAction::ReleaseBuild)
            && self.check_release_build(&nodes, crate::format::Limits::DEFAULT).is_err()
        {
            return;
        }
        // Avoid opening a transaction for repeated Hide/Lock Others (preserves redo).
        if matches!(action, LayerAction::HideOthers | LayerAction::LockOthers) {
            let mut keep = std::collections::BTreeSet::new();
            for n in &nodes {
                let mut root = *n;
                while let Some(parent) = self.doc.node(root).and_then(|n| n.parent) {
                    root = parent;
                }
                keep.insert(root);
            }
            if !self.doc.roots.iter().filter(|n| !keep.contains(n)).any(|n| {
                self.doc.node(*n).is_some_and(|row| {
                    if matches!(action, LayerAction::HideOthers) {
                        !row.hidden
                    } else {
                        !row.locked
                    }
                })
            }) {
                return;
            }
        }
        let z_order: std::collections::BTreeMap<_, _> =
            self.doc.paths.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
        self.begin();
        match action {
            LayerAction::HideOthers | LayerAction::LockOthers => {
                let keep: Vec<_> = nodes
                    .iter()
                    .map(|n| {
                        let mut layer = self.doc.layer_ancestor(*n);
                        while let Some(parent) = self.doc.node(layer).and_then(|n| n.parent) {
                            layer = parent;
                        }
                        layer
                    })
                    .collect();
                let roots = self.doc.roots.clone();
                for root in roots {
                    if let Some(p) = self.doc.node_mut(root) {
                        if !keep.contains(&p.id) {
                            if matches!(action, LayerAction::HideOthers) {
                                p.hidden = true;
                            } else {
                                p.locked = true;
                            }
                        }
                    }
                }
            }
            LayerAction::ReleaseSequence | LayerAction::ReleaseBuild => {
                for node in nodes {
                    let children = self.doc.node(node).map(|n| n.children.clone()).unwrap_or_default();
                    if self.doc.node(node).is_none_or(|n| n.kind != NodeKind::Layer) {
                        continue;
                    }
                    for (i, child) in children.iter().rev().enumerate() {
                        let layer = self.tool_layer(format!("Layer {}", i + 1), Some(node));
                        if matches!(action, LayerAction::ReleaseBuild) {
                            let paths: Vec<_> =
                                children.iter().rev().take(i + 1).flat_map(|n| self.doc.node_paths(*n)).collect();
                            let copies = self.doc.dup_paths(&paths);
                            let mut units: Vec<_> = copies.iter().filter_map(|p| self.doc.unit_of(*p)).collect();
                            units.sort_unstable();
                            units.dedup();
                            for unit in units {
                                self.doc.move_node_to(unit, layer, DropPos::Into);
                            }
                        } else {
                            self.doc.move_node_to(*child, layer, DropPos::Into);
                        }
                    }
                    if matches!(action, LayerAction::ReleaseBuild) {
                        for child in children {
                            self.tool_remove_subtree(child);
                        }
                    }
                }
            }
            LayerAction::Collect | LayerAction::Merge | LayerAction::Flatten => {
                let mut sources = if matches!(action, LayerAction::Flatten) { self.doc.roots.clone() } else { nodes };
                sources.sort_by_key(|n| {
                    std::cmp::Reverse(
                        self.doc.node_paths(*n).into_iter().filter_map(|p| self.doc.pidx(p)).max().unwrap_or(0),
                    )
                });
                for n in &sources {
                    for p in self.doc.node_paths(*n) {
                        self.bake_unit_of(p);
                    }
                }
                let target = if matches!(action, LayerAction::Merge) {
                    sources[0]
                } else {
                    self.tool_layer("Collected Artwork".into(), None)
                };
                for src in sources.iter().rev().copied().filter(|n| *n != target) {
                    if matches!(action, LayerAction::Merge | LayerAction::Flatten) {
                        self.tool_merge_layer_children(src, target);
                    } else {
                        self.doc.move_node_to(src, target, DropPos::Into);
                    }
                }
                if matches!(action, LayerAction::Flatten) {
                    loop {
                        let nested =
                            self.doc.nodes.iter().find(|n| n.kind == NodeKind::Layer && n.id != target).map(|n| n.id);
                        let Some(n) = nested else { break };
                        self.tool_merge_layer_children(n, target);
                    }
                }
                let mut children = self.doc.node(target).map(|n| n.children.clone()).unwrap_or_default();
                children.sort_by_key(|n| {
                    std::cmp::Reverse(
                        self.doc.node_paths(*n).iter().filter_map(|p| z_order.get(p)).copied().max().unwrap_or(0),
                    )
                });
                if let Some(n) = self.doc.node_mut(target) {
                    n.children = children;
                }
                self.doc.active_layer = target;
            }
            LayerAction::Locate => {}
        }
        self.dirty = true;
        self.commit();
        self.prune_inert_selection();
    }
    /// Bound peak output, including originals still alive while cumulative copies are built.
    /// Counts saturate so even hostile requests are refused before cloning or allocating ids.
    pub(crate) fn check_release_build(&self, nodes: &[u32], limits: crate::format::Limits) -> Result<(), String> {
        let targets: std::collections::BTreeSet<_> = nodes.iter().copied().collect();
        if targets.len() != nodes.len() {
            return Err("Release Build targets must be unique".into());
        }
        for id in nodes {
            let mut at = self.doc.node(*id).and_then(|n| n.parent);
            while let Some(parent) = at {
                if targets.contains(&parent) {
                    return Err("Release Build targets must not overlap ancestors".into());
                }
                at = self.doc.node(parent).and_then(|n| n.parent);
            }
        }
        let mut paths = self.doc.paths.len();
        let mut anchors =
            self.doc.paths.iter().map(|p| p.anchors.len() + p.holes.iter().map(Vec::len).sum::<usize>()).sum::<usize>();
        let mut tree = self.doc.nodes.len();
        let mut allocations = 0usize;
        for id in nodes {
            let Some(layer) = self.doc.node(*id).filter(|n| n.kind == NodeKind::Layer) else { continue };
            tree = tree.saturating_add(layer.children.len());
            allocations = allocations.saturating_add(layer.children.len());
            for (index, child) in layer.children.iter().rev().enumerate() {
                let multiplicity = layer.children.len() - index;
                let mut groups = std::collections::BTreeSet::new();
                for pid in self.doc.node_paths(*child) {
                    let Some(i) = self.doc.pidx(pid) else { continue };
                    let p = &self.doc.paths[i];
                    let count = p.anchors.len() + p.holes.iter().map(Vec::len).sum::<usize>();
                    paths = paths.saturating_add(multiplicity);
                    anchors = anchors.saturating_add(count.saturating_mul(multiplicity));
                    tree = tree.saturating_add(multiplicity);
                    allocations = allocations.saturating_add((count + 2).saturating_mul(multiplicity));
                    let mut at = self.doc.node_of_path(pid).and_then(|n| self.doc.node(n)).and_then(|n| n.parent);
                    while let Some(g) = at {
                        let Some(n) = self.doc.node(g).filter(|n| n.kind == NodeKind::Group) else { break };
                        groups.insert(g);
                        at = n.parent;
                    }
                }
                tree = tree.saturating_add(groups.len().saturating_mul(multiplicity));
                allocations = allocations.saturating_add(groups.len().saturating_mul(multiplicity));
                if paths > limits.max_paths || anchors > limits.max_anchors || tree > limits.max_nodes {
                    return Err("Release Build would exceed document paths, anchors or nodes limits".into());
                }
            }
        }
        if (self.allocation_floor() as usize).saturating_add(allocations) >= u32::MAX as usize {
            return Err("Release Build would exhaust stable ids".into());
        }
        Ok(())
    }
    fn tool_merge_layer_children(&mut self, source: u32, target: u32) {
        let Some(row) = self.doc.node(source) else { return };
        let (hidden, locked) = (row.hidden, row.locked);
        let children = row.children.clone();
        for child in children.into_iter().rev() {
            // Materialize only the flags of the ancestor being removed. Nested removals
            // propagate them again, preserving effective state without changing unrelated nodes.
            if let Some(n) = self.doc.node_mut(child) {
                n.hidden |= hidden;
                n.locked |= locked;
            }
            self.doc.move_node_to(child, target, DropPos::Into);
        }
        self.doc.remove_node(source);
    }
    fn tool_remove_subtree(&mut self, id: u32) {
        let children = self.doc.node(id).map(|n| n.children.clone()).unwrap_or_default();
        for child in children {
            self.tool_remove_subtree(child);
        }
        if let Some(NodeKind::Path(pid)) = self.doc.node(id).map(|n| n.kind) {
            self.doc.paths.retain(|p| p.id != pid);
        }
        self.doc.remove_node(id);
    }
    fn tool_layer(&mut self, name: String, parent: Option<u32>) -> u32 {
        let id = self.doc.nid();
        self.doc.nodes.push(Node {
            id,
            kind: NodeKind::Layer,
            name,
            parent,
            children: vec![],
            hidden: false,
            locked: false,
            color: None,
            clip_exempt: false,
            xform: Xform::default(),
            role: GroupRole::Normal,
            mask_child: None,
        });
        if let Some(p) = parent {
            if let Some(n) = self.doc.node_mut(p) {
                n.children.insert(0, id);
            }
        } else {
            self.doc.roots.insert(0, id);
        }
        id
    }
}

#[cfg(test)]
mod fix_tests {
    use super::*;
    #[test]
    fn release_build_preflights_anchors_holes_groups_nodes_and_ids() {
        let mut e = Editor::new();
        e.execute(crate::EditCommand::AddShape {
            kind: crate::model::ShapeKind::Rect,
            bounds: [0., 0., 10., 20.],
            parent: None,
            fill: None,
            stroke: None,
            stroke_width: 1.,
            opacity: 1.,
            name: None,
        });
        let id = e.doc.paths[0].id;
        let copy = e.doc.clone_path(id);
        let other = copy.id;
        e.doc.paths.push(copy);
        e.doc.sync_tree();
        e.doc.paths[0].holes = vec![e.doc.paths[0].anchors.clone()];
        e.execute(crate::EditCommand::SelectPaths(vec![id, other]));
        e.execute(crate::EditCommand::GroupSelection);
        let layer = e.doc.active_layer;
        let default = crate::format::Limits::DEFAULT;
        assert!(e.check_release_build(&[layer], default).is_ok());
        assert!(e.check_release_build(&[layer, layer], default).is_err());
        for limits in [
            crate::format::Limits { max_paths: 1, ..default },
            crate::format::Limits { max_anchors: 15, ..default },
            crate::format::Limits { max_nodes: e.doc.nodes.len() + 2, ..default },
        ] {
            assert!(e.check_release_build(&[layer], limits).is_err());
        }
        e.doc.ids = u32::MAX - 3;
        assert!(e.check_release_build(&[layer], default).is_err());
    }
}
