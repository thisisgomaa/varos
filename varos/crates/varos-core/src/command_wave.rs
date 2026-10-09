//! Adapted from VectorCraft engine/src/cmd/select.rs:22-145 and object.rs:85-176,
//! pathops/src/edit.rs:299-346 @ a469568 (MIT OR Apache-2.0).
//! Matching and editing use Varos' model, transform and history boundaries.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Same {
    Fill,
    FillStroke,
    Stroke,
    StrokeWeight,
    Opacity,
    Appearance,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Selection {
    All,
    Deselect,
    Reselect,
    Inverse,
    Above,
    Below,
    Artboard,
    Same(Same),
    KeyObject(u32),
    Group(u32),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectAction {
    Lock,
    UnlockAll,
    Hide,
    ShowAll,
    ExpandTransform,
    Reverse,
    Average,
    AddAnchors,
    CleanUp,
    Join,
    CompoundMake,
    CompoundRelease,
    NewLayer,
    NewSublayer,
    SendToCurrentLayer,
}

impl Editor {
    pub fn selection_command(&mut self, action: Selection) {
        if let Selection::Group(pid) = action {
            self.wave_group_selection(pid);
            return;
        }
        if let Selection::KeyObject(id) = action {
            if self.objsel.contains(&id) {
                self.key_object = Some(id);
            }
            return;
        }
        let old: Vec<u32> = self.selected_pids().into_iter().collect();
        if action == Selection::Reselect {
            if let Some(state) = self.reselect_state.clone() {
                self.escape();
                self.objsel.extend(state.paths.into_iter().filter(|id| {
                    self.doc.pidx(*id).is_some() && !self.doc.eff_hidden(*id) && !self.doc.eff_locked(*id)
                }));
                self.selected.extend(state.anchors.into_iter().filter(|id| {
                    self.doc
                        .pid_of_anchor(*id)
                        .is_some_and(|pid| !self.doc.eff_hidden(pid) && !self.doc.eff_locked(pid))
                }));
                self.group_sel.extend(state.groups.into_iter().filter(|id| self.doc.node(*id).is_some()));
                self.dsel_path = state.direct.filter(|id| {
                    self.doc.pidx(*id).is_some() && !self.doc.eff_hidden(*id) && !self.doc.eff_locked(*id)
                });
                self.refresh_obj_angle();
                return;
            }
        } else if !old.is_empty() {
            self.reselect_state = Some(SelectionState {
                paths: self.objsel.iter().copied().collect(),
                anchors: self.selected.iter().copied().collect(),
                groups: self.group_sel.iter().copied().collect(),
                direct: self.dsel_path,
            });
        }
        if action == Selection::All {
            self.select_all();
            return;
        }
        if action == Selection::Deselect {
            if !old.is_empty() {
                self.reselect = old;
            }
            self.escape();
            return;
        }
        let pickable: Vec<u32> = self
            .doc
            .paint_list()
            .filter_map(|(_, p)| (!self.doc.eff_hidden(p.id) && !self.doc.eff_locked(p.id)).then_some(p.id))
            .collect();
        let ids = match action {
            Selection::Reselect => self.reselect.iter().copied().filter(|id| pickable.contains(id)).collect(),
            Selection::Inverse => pickable.iter().copied().filter(|id| !old.contains(id)).collect(),
            Selection::Artboard => pickable
                .iter()
                .copied()
                .filter(|id| self.doc.pidx(*id).is_some_and(|pi| self.doc.path_boards(pi).contains(&self.doc.active)))
                .collect(),
            Selection::Above | Selection::Below => {
                let mut seen = HashSet::new();
                let objects: Vec<u32> =
                    pickable.iter().copied().filter(|id| seen.insert(self.doc.unit_of(*id))).collect();
                let positions: Vec<usize> = objects
                    .iter()
                    .enumerate()
                    .filter_map(|(i, id)| self.doc.group_members(*id).iter().any(|id| old.contains(id)).then_some(i))
                    .collect();
                let next = if action == Selection::Above {
                    positions.last().and_then(|i| objects.get(i + 1))
                } else {
                    positions.first().and_then(|i| i.checked_sub(1)).and_then(|i| objects.get(i))
                };
                next.map_or_else(
                    || old.clone(),
                    |id| self.doc.group_members(*id).into_iter().filter(|id| pickable.contains(id)).collect(),
                )
            }
            Selection::Same(mode) => {
                let reference = self.doc.paint_list().find(|(_, p)| old.contains(&p.id)).map(|(_, p)| p);
                let Some(a) = reference else { return };
                pickable
                    .iter()
                    .copied()
                    .filter(|id| {
                        self.doc.pidx(*id).is_some_and(|i| {
                            let b = &self.doc.paths[i];
                            let fill = a.appearance().fill().clone() == b.appearance().fill().clone();
                            let stroke = a.appearance().stroke().clone() == b.appearance().stroke().clone();
                            match mode {
                                Same::Fill => fill,
                                Same::Stroke => stroke,
                                Same::FillStroke => fill && stroke,
                                Same::StrokeWeight => a.stroke_width == b.stroke_width,
                                Same::Opacity => a.opacity == b.opacity,
                                Same::Appearance => {
                                    fill && stroke && a.stroke_width == b.stroke_width && a.opacity == b.opacity
                                }
                            }
                        })
                    })
                    .collect()
            }
            Selection::All | Selection::Deselect | Selection::KeyObject(_) | Selection::Group(_) => return,
        };
        if !old.is_empty() {
            self.reselect = old;
        }
        self.escape();
        self.key_object = None;
        self.objsel.extend(ids);
        if matches!(action, Selection::Above | Selection::Below) {
            for id in &self.objsel {
                if let Some(group) = self.doc.top_group_of_path(*id) {
                    self.group_sel.insert(group);
                }
            }
        }
        self.refresh_obj_angle();
    }
    /// Option-Direct clicks select the leaf, then one containing group per click.
    fn wave_group_selection(&mut self, pid: u32) {
        if self.doc.eff_hidden(pid) || self.doc.eff_locked(pid) {
            return;
        }
        let Some(leaf) = self.doc.node_of_path(pid) else { return };
        let mut target = leaf;
        let already = self.objsel.contains(&pid)
            || self.dsel_path == Some(pid)
            || self.selected.iter().any(|id| self.doc.pid_of_anchor(*id) == Some(pid));
        if already {
            let mut chain = vec![leaf];
            let mut current = leaf;
            while let Some(parent) = self.doc.node(current).and_then(|n| n.parent) {
                if !self.doc.node(parent).is_some_and(|n| matches!(n.kind, NodeKind::Group)) {
                    break;
                }
                chain.push(parent);
                current = parent;
            }
            let level = chain.iter().position(|id| self.group_sel.contains(id)).unwrap_or(0);
            target = chain[(level + 1).min(chain.len() - 1)];
        }
        let paths = self.doc.node_paths(target);
        self.escape();
        self.objsel.extend(paths);
        if target != leaf {
            self.group_sel.insert(target);
        }
        self.refresh_obj_angle();
    }
    /// Adapted from VectorCraft tools/src/xform/wand.rs:49,67 (MIT OR Apache-2.0).
    /// Even-odd polygon test in world coordinates; the boundary counts as inside.
    pub fn lasso_select(&mut self, points: &[Pt], objects: bool, additive: bool) {
        if points.len() < 3 {
            return;
        }
        let inside = |p: Pt| {
            let mut hit = false;
            for (a, b) in points.iter().zip(points.iter().cycle().skip(1)).take(points.len()) {
                let v = sub(*b, *a);
                let w = sub(p, *a);
                let cross = v[0] * w[1] - v[1] * w[0];
                let dot = w[0] * v[0] + w[1] * v[1];
                let length = v[0] * v[0] + v[1] * v[1];
                if length > 0.0 && cross.abs() <= 1e-5 && dot >= 0.0 && dot <= length {
                    return true;
                }
                if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
                    hit = !hit;
                }
            }
            hit
        };
        if !additive {
            self.escape();
        }
        for p in &self.doc.paths {
            if self.doc.eff_hidden(p.id) || self.doc.eff_locked(p.id) {
                continue;
            }
            let xf = self.doc.unit_xform(p.id);
            let anchors = p.anchors.iter().chain(p.holes.iter().flatten());
            if objects {
                if anchors.clone().any(|a| inside(xf.apply(a.p))) {
                    self.objsel.insert(p.id);
                }
            } else {
                self.selected.extend(anchors.filter(|a| inside(xf.apply(a.p))).map(|a| a.id));
            }
        }
        self.refresh_obj_angle();
    }
    pub fn wave_insert_anchor(&mut self, path: u32, segment: usize, t: f32) {
        let Some(pi) = self.doc.pidx(path) else { return };
        let own = self.pending.is_none();
        if own {
            self.begin();
        }
        let id = self.add_anchor(pi, segment, t);
        self.selected.insert(id);
        self.dirty = true;
        if own {
            self.commit_wave();
        }
    }
    pub fn wave_delete_anchor(&mut self, id: u32) {
        if self.doc.anchor_address(id).is_none() {
            return;
        }
        let own = self.pending.is_none();
        if own {
            self.begin();
        }
        self.delete_anchor_reconnect(id);
        self.selected.remove(&id);
        self.dirty = true;
        if own {
            self.commit_wave();
        }
    }
    pub fn wave_anchor_type(&mut self, id: u32, smooth: bool) {
        let Some(pid) = self.doc.pid_of_anchor(id) else { return };
        if self.doc.anchor(id).is_some_and(|a| a.smooth == smooth) {
            return;
        }
        let own = self.pending.is_none();
        if own {
            self.begin();
        }
        self.bake_unit_of(pid);
        self.toggle_type(id);
        self.dirty = true;
        if own {
            self.commit_wave();
        }
    }
    /// A selection-level flag edit is one undo step; all also clears ancestor node flags.
    pub fn object_command(&mut self, action: ObjectAction) {
        if matches!(action, ObjectAction::NewLayer | ObjectAction::NewSublayer) {
            self.create_layer(action == ObjectAction::NewSublayer);
            return;
        }
        if action == ObjectAction::SendToCurrentLayer {
            let units = self.objsel_units();
            self.layer_move(&units, self.doc.active_layer, crate::model::DropPos::Into);
            return;
        }
        if action == ObjectAction::ExpandTransform {
            self.expand_transform();
            return;
        }
        let ids = self.selected_pids();
        if matches!(action, ObjectAction::Lock | ObjectAction::Hide) && ids.is_empty() {
            return;
        }
        match action {
            ObjectAction::Lock | ObjectAction::Hide | ObjectAction::UnlockAll | ObjectAction::ShowAll => {
                let all = matches!(action, ObjectAction::UnlockAll | ObjectAction::ShowAll);
                let lock = matches!(action, ObjectAction::Lock | ObjectAction::UnlockAll);
                let changed =
                    self.doc.paths.iter().any(|p| {
                        (all || ids.contains(&p.id)) && if lock { p.locked != !all } else { p.hidden != !all }
                    }) || (all && self.doc.nodes.iter().any(|n| if lock { n.locked } else { n.hidden }));
                if !changed {
                    return;
                }
                self.begin();
                for p in &mut self.doc.paths {
                    if all || ids.contains(&p.id) {
                        if lock {
                            p.locked = !all;
                        } else {
                            p.hidden = !all;
                        }
                    }
                }
                if all {
                    for n in &mut self.doc.nodes {
                        if lock {
                            n.locked = false;
                        } else {
                            n.hidden = false;
                        }
                    }
                }
                self.dirty = true;
                self.commit_wave();
                self.prune_inert_selection();
            }
            ObjectAction::Average => {
                let anchors: Vec<u32> = self.selected.iter().copied().collect();
                if anchors.len() < 2 {
                    return;
                }
                self.begin();
                for id in &ids {
                    self.bake_unit_of(*id);
                }
                let points: Vec<Pt> = anchors.iter().filter_map(|id| self.doc.anchor(*id).map(|a| a.p)).collect();
                let centre =
                    points.iter().fold([0.0, 0.0], |a, p| [a[0] + p[0], a[1] + p[1]]).map(|v| v / points.len() as f32);
                for id in anchors {
                    if let Some(a) = self.doc.anchor_mut(id) {
                        let d = sub(centre, a.p);
                        a.p = centre;
                        a.hin = a.hin.map(|p| add(p, d));
                        a.hout = a.hout.map(|p| add(p, d));
                    }
                }
                self.dirty = true;
                self.commit_wave();
            }
            ObjectAction::Reverse | ObjectAction::AddAnchors => {
                if ids.is_empty() {
                    return;
                }
                self.begin();
                for id in ids {
                    self.bake_unit_of(id);
                    if let Some(pi) = self.doc.pidx(id) {
                        if action == ObjectAction::Reverse {
                            self.reverse(pi);
                            for ring in &mut self.doc.paths[pi].holes {
                                ring.reverse();
                                for a in ring {
                                    std::mem::swap(&mut a.hin, &mut a.hout);
                                }
                            }
                        } else {
                            let n = self.doc.paths[pi].anchors.len();
                            let segments = if self.doc.paths[pi].closed { n } else { n.saturating_sub(1) };
                            for i in (0..segments).rev() {
                                self.add_anchor(pi, i, 0.5);
                            }
                            for hi in 0..self.doc.paths[pi].holes.len() {
                                let mut ring = std::mem::take(&mut self.doc.paths[pi].holes[hi]);
                                std::mem::swap(&mut ring, &mut self.doc.paths[pi].anchors);
                                let closed = self.doc.paths[pi].closed;
                                self.doc.paths[pi].closed = true;
                                let n = self.doc.paths[pi].anchors.len();
                                for i in (0..n).rev() {
                                    self.add_anchor(pi, i, 0.5);
                                }
                                self.doc.paths[pi].closed = closed;
                                std::mem::swap(&mut ring, &mut self.doc.paths[pi].anchors);
                                self.doc.paths[pi].holes[hi] = ring;
                            }
                        }
                    }
                }
                self.dirty = true;
                self.commit_wave();
            }
            ObjectAction::CleanUp => {
                let remove: Vec<u32> = self
                    .doc
                    .paths
                    .iter()
                    .filter(|p| {
                        !self.doc.eff_locked(p.id)
                            && !self.doc.eff_hidden(p.id)
                            && (p.anchors.len() < 2
                                || (p.appearance().fill().clone() == crate::model::Paint::None
                                    && p.appearance().stroke().clone() == crate::model::Paint::None))
                    })
                    .map(|p| p.id)
                    .collect();
                if remove.is_empty() {
                    return;
                }
                self.begin();
                self.doc.paths.retain(|p| !remove.contains(&p.id));
                self.doc.sync_tree();
                self.dirty = true;
                self.commit_wave();
            }
            ObjectAction::Join => self.join_selection(),
            ObjectAction::CompoundMake => self.compound_make(),
            ObjectAction::CompoundRelease => self.compound_release(),
            ObjectAction::ExpandTransform
            | ObjectAction::NewLayer
            | ObjectAction::NewSublayer
            | ObjectAction::SendToCurrentLayer => {}
        }
    }
    fn join_selection(&mut self) {
        let ids = self.selected_pids();
        let mut paths: Vec<u32> = self
            .doc
            .paths
            .iter()
            .filter(|p| ids.contains(&p.id) && !p.closed && p.holes.is_empty() && p.anchors.len() >= 2)
            .map(|p| p.id)
            .collect();
        if paths.is_empty() {
            return;
        }
        // Direct selection must name exactly two endpoints. Whole-path selection uses nearest ends.
        let explicit = !self.selected.is_empty() && self.objsel.is_empty();
        if explicit {
            let endpoints: Vec<_> = paths
                .iter()
                .filter_map(|id| self.doc.pidx(*id))
                .flat_map(|pi| [self.doc.paths[pi].anchors.first(), self.doc.paths[pi].anchors.last()])
                .flatten()
                .filter(|a| self.selected.contains(&a.id))
                .collect();
            if self.selected.len() != 2 || endpoints.len() != 2 || paths.len() > 2 {
                return;
            }
        }
        self.begin();
        for id in &paths {
            self.bake_unit_of(*id);
        }
        let first = paths.remove(0);
        if paths.is_empty() {
            if let Some(pi) = self.doc.pidx(first) {
                self.doc.paths[pi].closed = true;
            }
        } else {
            for id in paths {
                let (Some(a), Some(b)) = (self.doc.pidx(first), self.doc.pidx(id)) else { continue };
                let left = &self.doc.paths[a].anchors;
                let right = &self.doc.paths[b].anchors;
                let mut best = (f32::INFINITY, false, false);
                for ra in [false, true] {
                    for rb in [false, true] {
                        let pa = if ra { left.first() } else { left.last() };
                        let pb = if rb { right.last() } else { right.first() };
                        if let (Some(pa), Some(pb)) = (pa, pb) {
                            if explicit && (!self.selected.contains(&pa.id) || !self.selected.contains(&pb.id)) {
                                continue;
                            }
                            let d = dist(pa.p, pb.p);
                            if d < best.0 {
                                best = (d, ra, rb);
                            }
                        }
                    }
                }
                if best.1 {
                    self.reverse(a);
                }
                if best.2 {
                    self.reverse(b);
                }
                let mut append = self.doc.paths[b].anchors.clone();
                if best.0 < 0.001 && !append.is_empty() {
                    let merged = append.remove(0);
                    if let Some(last) = self.doc.paths[a].anchors.last_mut() {
                        last.hout = merged.hout;
                    }
                }
                self.doc.paths[a].anchors.extend(append);
                self.doc.paths.retain(|p| p.id != id);
            }
        }
        self.doc.sync_tree();
        self.objsel = [first].into_iter().collect();
        self.selected.clear();
        self.dirty = true;
        self.commit_wave();
    }
    fn compound_make(&mut self) {
        let ids = self.selected_pids();
        let parts: Vec<u32> = self.doc.paths.iter().filter(|p| ids.contains(&p.id) && p.closed).map(|p| p.id).collect();
        if self.doc.paths.iter().any(|p| ids.contains(&p.id) && !p.closed) {
            return;
        }
        if parts.len() < 2 {
            return;
        }
        self.begin();
        for id in &parts {
            self.bake_unit_of(*id);
        }
        let first = parts[0];
        let rings: Vec<Vec<Anchor>> = self
            .doc
            .paths
            .iter()
            .filter(|p| parts[1..].contains(&p.id))
            .flat_map(|p| std::iter::once(p.anchors.clone()).chain(p.holes.clone()))
            .collect();
        if let Some(pi) = self.doc.pidx(first) {
            self.doc.paths[pi].holes.extend(rings);
        }
        self.doc.paths.retain(|p| !parts[1..].contains(&p.id));
        self.doc.sync_tree();
        self.objsel = [first].into_iter().collect();
        self.dirty = true;
        self.commit_wave();
    }
    fn compound_release(&mut self) {
        let ids = self.selected_pids();
        if !self.doc.paths.iter().any(|p| ids.contains(&p.id) && !p.holes.is_empty()) {
            return;
        }
        self.begin();
        for id in &ids {
            self.bake_unit_of(*id);
        }
        for id in ids {
            let Some(pi) = self.doc.pidx(id) else { continue };
            let original_node = self.doc.node_of_path(id).and_then(|leaf| self.doc.node(leaf)).cloned();
            let parent = original_node.as_ref().and_then(|n| n.parent);
            let template = self.doc.paths[pi].clone();
            let rings = std::mem::take(&mut self.doc.paths[pi].holes);
            for anchors in rings {
                let mut p = template.clone();
                p.id = self.doc.nid();
                p.anchors = anchors;
                p.holes.clear();
                let new_id = p.id;
                self.objsel.insert(new_id);
                self.doc.paths.push(p);
                self.doc.sync_tree();
                if let (Some(parent), Some(leaf)) = (parent, self.doc.node_of_path(new_id)) {
                    self.doc.move_node_to(leaf, parent, crate::model::DropPos::Into);
                    if let Some(n) = self.doc.nodes.iter_mut().find(|n| n.id == leaf) {
                        n.clip_exempt = original_node.as_ref().is_some_and(|n| n.clip_exempt);
                    }
                }
            }
        }
        self.doc.sync_tree();
        self.dirty = true;
        self.commit_wave();
    }
}

impl Editor {
    pub fn align_to_key(&mut self, mode: AlignMode) {
        let Some(key) = self.key_object.filter(|id| self.objsel.contains(id)) else { return };
        let Some(key_unit) = self.doc.unit_of(key) else { return };
        let Some(reference) = self.wave_unit_bbox(key_unit) else { return };
        let units = self.objsel_units();
        if units.len() < 2 {
            return;
        }
        self.begin();
        self.bake_selected_units();
        for unit in units {
            if unit == key_unit {
                continue;
            }
            let pids = self.doc.node_paths(unit);
            let bounds = pids
                .iter()
                .filter_map(|id| self.doc.pidx(*id))
                .map(|pi| self.doc.outline_bbox(pi))
                .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)));
            if let Some(bounds) = bounds {
                let delta = align_delta(mode, reference, bounds);
                for id in pids {
                    if let Some(pi) = self.doc.pidx(id) {
                        // Flattened curve bounds can differ by a few float ulps on reapplication.
                        if delta[0].abs() > 1.0e-4 || delta[1].abs() > 1.0e-4 {
                            self.translate_path(pi, delta);
                        }
                    }
                }
            }
        }
        self.refresh_obj_angle();
        self.dirty = true;
        self.commit_wave();
    }
    /// Distribute top-level units by matching edges, preserving group geometry.
    pub fn distribute_mode(&mut self, mode: AlignMode) {
        if !self.selected.is_empty() {
            self.distribute_anchors(if matches!(mode, AlignMode::Left | AlignMode::CenterH | AlignMode::Right) {
                DistAxis::Horizontal
            } else {
                DistAxis::Vertical
            });
            return;
        }
        let mut items: Vec<_> = self
            .objsel_units()
            .into_iter()
            .filter_map(|unit| {
                let pids = self.doc.node_paths(unit);
                let b = pids
                    .iter()
                    .filter_map(|id| self.doc.pidx(*id))
                    .map(|pi| self.doc.outline_bbox(pi))
                    .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))?;
                let c = match mode {
                    AlignMode::Left => b.0,
                    AlignMode::CenterH => (b.0 + b.2) * 0.5,
                    AlignMode::Right => b.2,
                    AlignMode::Top => b.1,
                    AlignMode::Middle => (b.1 + b.3) * 0.5,
                    AlignMode::Bottom => b.3,
                };
                Some((unit, c, pids))
            })
            .collect();
        if items.len() < 3 {
            return;
        }
        items.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        let step = (items[items.len() - 1].1 - items[0].1) / (items.len() - 1) as f32;
        let start = items[0].1;
        self.begin();
        self.bake_selected_units();
        for (i, (_, c, pids)) in items.iter().enumerate().skip(1).take(items.len() - 2) {
            let d = start + step * i as f32 - c;
            let delta = if matches!(mode, AlignMode::Left | AlignMode::CenterH | AlignMode::Right) {
                [d, 0.0]
            } else {
                [0.0, d]
            };
            for id in pids {
                if let Some(pi) = self.doc.pidx(*id) {
                    // Flattened curve bounds can differ by a few float ulps on reapplication.
                    if delta[0].abs() > 1.0e-4 || delta[1].abs() > 1.0e-4 {
                        self.translate_path(pi, delta);
                    }
                }
            }
        }
        self.refresh_obj_angle();
        self.dirty = true;
        self.commit_wave();
    }
}
impl Editor {
    pub fn distribute_spacing_wave(&mut self, axis: DistAxis, gap: f32) {
        let horizontal = matches!(axis, DistAxis::Horizontal);
        let mut items: Vec<_> = self
            .objsel_units()
            .into_iter()
            .filter_map(|unit| {
                let paths = self.doc.node_paths(unit);
                let b = paths
                    .iter()
                    .filter_map(|id| self.doc.pidx(*id))
                    .map(|pi| self.doc.outline_bbox(pi))
                    .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))?;
                Some((unit, if horizontal { b.0 } else { b.1 }, if horizontal { b.2 - b.0 } else { b.3 - b.1 }, paths))
            })
            .collect();
        if items.len() < 2 || !gap.is_finite() {
            return;
        }
        items.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        let key = self.key_object.filter(|id| self.objsel.contains(id)).and_then(|id| self.doc.unit_of(id));
        let anchor = items.iter().position(|item| Some(item.0) == key).unwrap_or(0);
        let mut cur = items[anchor].1 - items[..anchor].iter().map(|item| item.2 + gap).sum::<f32>();
        self.begin();
        self.bake_selected_units();
        for (unit, lo, len, paths) in items {
            let delta = if horizontal { [cur - lo, 0.0] } else { [0.0, cur - lo] };
            if Some(unit) != key {
                for id in paths {
                    if let Some(pi) = self.doc.pidx(id) {
                        // Flattened curve bounds can differ by a few float ulps on reapplication.
                        if delta[0].abs() > 1.0e-4 || delta[1].abs() > 1.0e-4 {
                            self.translate_path(pi, delta);
                        }
                    }
                }
            }
            cur += len + gap;
        }
        self.refresh_obj_angle();
        self.dirty = true;
        self.commit_wave();
    }
}

impl Editor {
    pub fn create_layer(&mut self, sublayer: bool) {
        use crate::model::{GroupRole, Node, NodeKind};
        let parent = sublayer.then_some(self.doc.active_layer);
        if sublayer && self.doc.node(self.doc.active_layer).is_none() {
            return;
        }
        self.begin();
        let id = self.doc.nid();
        let name = format!("Layer {}", self.doc.nodes.iter().filter(|n| n.kind == NodeKind::Layer).count() + 1);
        self.doc.nodes.push(Node {
            id,
            kind: NodeKind::Layer,
            name,
            parent,
            children: Vec::new(),
            hidden: false,
            locked: false,
            color: None,
            clip_exempt: false,
            xform: Default::default(),
            role: GroupRole::Normal,
            mask_child: None,
        });
        if let Some(parent) = parent {
            if let Some(n) = self.doc.nodes.iter_mut().find(|n| n.id == parent) {
                n.children.insert(0, id);
            }
        } else {
            self.doc.roots.insert(0, id);
        }
        self.doc.active_layer = id;
        self.dirty = true;
        self.commit_wave();
    }
}

impl Editor {
    pub(super) fn wave_unit_bbox(&self, unit: u32) -> Option<(f32, f32, f32, f32)> {
        self.doc
            .node_paths(unit)
            .iter()
            .filter_map(|id| self.doc.pidx(*id))
            .map(|pi| self.doc.outline_bbox(pi))
            .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
    }
}
