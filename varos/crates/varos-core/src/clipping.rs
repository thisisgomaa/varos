//! Illustrator clipping commands on the existing clip container model.
use crate::{
    model::{GroupRole, NodeKind},
    Editor,
};
impl Editor {
    fn clip_mask(&self) -> Option<u32> {
        let units = self
            .objsel
            .iter()
            .filter_map(|p| self.doc.unit_of(*p))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if units.len() < 2 || self.objsel.iter().any(|p| self.doc.eff_hidden(*p) || self.doc.eff_locked(*p)) {
            return None;
        }
        if units.iter().any(|u| self.doc.node_paths(*u).iter().any(|p| !self.objsel.contains(p))) {
            return None;
        }
        let top =
            units.iter().max_by_key(|u| self.doc.node_paths(**u).iter().filter_map(|p| self.doc.pidx(*p)).max())?;
        let node = self.doc.node(*top)?;
        if !matches!(node.kind, NodeKind::Path(_)) {
            return None;
        }
        self.doc.node_paths(*top).first().copied()
    }
    pub fn clip_make_enabled(&self) -> bool {
        self.clip_mask().is_some()
    }
    fn selected_clip_groups(&self) -> Vec<u32> {
        let mut groups = self.group_sel.clone();
        for pid in &self.objsel {
            if let Some(id) = self.doc.unit_of(*pid) {
                if self.doc.node_paths(id).iter().all(|p| self.objsel.contains(p)) {
                    groups.insert(id);
                }
            }
        }
        groups
            .into_iter()
            .filter(|id| self.doc.node(*id).is_some_and(|n| n.role == GroupRole::Clip && !n.hidden && !n.locked))
            .collect()
    }
    pub fn clip_release_enabled(&self) -> bool {
        !self.selected_clip_groups().is_empty()
    }
    pub(crate) fn clip_make(&mut self) {
        let Some(mask) = self.clip_mask() else {
            return;
        };
        self.group_selection_with_clip(Some(mask));
    }
    pub(crate) fn clip_release(&mut self) {
        if !self.clip_release_enabled() {
            return;
        }
        self.begin();
        for id in self.selected_clip_groups() {
            if self.doc.node(id).is_some_and(|n| n.role == GroupRole::Clip) {
                self.doc.release_clip(id);
            }
        }
        self.dirty = true;
        self.commit();
    }
}
