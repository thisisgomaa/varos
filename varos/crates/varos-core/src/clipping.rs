//! Illustrator clipping commands on the existing clip container model.
use crate::{
    model::{GroupRole, NodeKind},
    Editor,
};
#[derive(Clone)]
pub(crate) struct Enablement {
    rev: u64,
    objects: std::collections::HashSet<u32>,
    groups: std::collections::HashSet<u32>,
    result: (bool, bool),
}
// Temporary index confines tree work to cache misses and avoids linear node lookups.
struct ClipIndex<'a> {
    nodes: std::collections::HashMap<u32, &'a crate::model::Node>,
    leaves: std::collections::HashMap<u32, u32>,
}
impl<'a> ClipIndex<'a> {
    fn new(ed: &'a Editor) -> Self {
        Self {
            nodes: ed.doc.nodes.iter().map(|n| (n.id, n)).collect(),
            leaves: ed
                .doc
                .nodes
                .iter()
                .filter_map(|n| match n.kind {
                    NodeKind::Path(pid) | NodeKind::Image(pid) => Some((pid, n.id)),
                    _ => None,
                })
                .collect(),
        }
    }
    fn unit(&self, pid: u32) -> Option<u32> {
        let leaf = *self.leaves.get(&pid)?;
        let mut current = leaf;
        let mut top = leaf;
        for _ in 0..4096 {
            let Some(parent) = self.nodes.get(&current)?.parent else { break };
            if matches!(self.nodes.get(&parent)?.kind, NodeKind::Group) {
                top = parent;
            }
            current = parent;
        }
        Some(top)
    }
    fn paths(&self, id: u32) -> Vec<u32> {
        let mut todo = vec![id];
        let mut seen = std::collections::HashSet::new();
        let mut paths = Vec::new();
        while let Some(id) = todo.pop() {
            if !seen.insert(id) {
                continue;
            }
            if let Some(n) = self.nodes.get(&id) {
                if let NodeKind::Path(pid) | NodeKind::Image(pid) = n.kind {
                    paths.push(pid);
                }
                todo.extend(n.children.iter().copied());
            }
        }
        paths
    }
}
#[cfg(test)]
thread_local! { static CACHE_MISSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
impl Editor {
    /// UI mirrors share cached state for committed revisions and transient selections.
    /// Live transactions bypass the cache; begin/execute invalidate before editing.
    pub fn clipping_enablement(&self) -> (bool, bool) {
        let mut cache = self.clipping_enablement.borrow_mut();
        let cacheable = !self.transaction_open() && !self.dirty;
        if cacheable {
            if let Some(old) = cache.as_ref() {
                if old.rev == self.rev && old.objects == self.objsel && old.groups == self.group_sel {
                    return old.result;
                }
            }
        }
        #[cfg(test)]
        CACHE_MISSES.with(|n| n.set(n.get() + 1));
        let result = (self.clip_make_enabled(), self.clip_release_enabled());
        if cacheable {
            *cache = Some(Enablement {
                rev: self.rev,
                objects: self.objsel.clone(),
                groups: self.group_sel.clone(),
                result,
            });
        }
        result
    }

    fn clip_mask(&self) -> Option<u32> {
        if self.objsel.len() < 2 {
            return None;
        }
        let index = ClipIndex::new(self);
        let units = self
            .objsel
            .iter()
            .filter_map(|p| index.unit(*p))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if units.len() < 2 || self.objsel.iter().any(|p| self.doc.eff_hidden(*p) || self.doc.eff_locked(*p)) {
            return None;
        }
        if units.iter().any(|u| index.paths(*u).iter().any(|p| !self.objsel.contains(p))) {
            return None;
        }
        let ranks: std::collections::HashMap<_, _> = crate::images::paint_order(&self.doc)
            .into_iter()
            .enumerate()
            .filter_map(|(rank, kind)| match kind {
                NodeKind::Path(id) | NodeKind::Image(id) => Some((id, rank)),
                _ => None,
            })
            .collect();
        let top = units.iter().max_by_key(|u| index.paths(**u).iter().filter_map(|p| ranks.get(p)).max().copied())?;
        let node = self.doc.node(*top)?;
        if !matches!(node.kind, NodeKind::Path(_)) {
            return None;
        }
        index.paths(*top).first().copied()
    }
    pub fn clip_make_enabled(&self) -> bool {
        self.clip_mask().is_some()
    }
    fn selected_clip_groups(&self) -> Vec<u32> {
        let mut groups = self.group_sel.clone();
        if self.objsel.is_empty() && groups.is_empty() {
            return Vec::new();
        }
        // ---- w2-images ----
        for id in &self.objsel {
            if self.doc.images.iter().any(|i| i.id == *id) && !self.doc.eff_locked(*id) && !self.doc.eff_hidden(*id) {
                if let Some(group) = self.doc.clip_group_of(*id) {
                    groups.insert(group);
                }
            }
        }
        let index = ClipIndex::new(self);
        // Traverse each selected unit once, even when every descendant is selected.
        let units: std::collections::HashSet<_> = self.objsel.iter().filter_map(|pid| index.unit(*pid)).collect();
        for id in units {
            if index.paths(id).iter().all(|p| self.objsel.contains(p)) {
                groups.insert(id);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{Anchor, Document, Path},
        EditCommand,
    };
    fn large_selection() -> Editor {
        let mut ed = Editor::new();
        let mut doc = Document::default();
        for pid in 1..=512 {
            doc.paths.push(Path::new(
                pid,
                vec![Anchor { id: 1000 + pid, p: [0., 0.], hin: None, hout: None, smooth: false }],
                true,
                Some([1.; 4]),
                None,
                0.,
            ));
        }
        doc.ids = 2000;
        doc.sync_tree();
        let group = doc.clip_group(&(1..=512).collect::<Vec<_>>(), 512).unwrap();
        ed.replace_doc(doc);
        ed.objsel = (1..=512).collect();
        ed.group_sel.insert(group);
        ed
    }
    #[test]
    fn large_selection_mirrors_share_cache_and_selection_revision_invalidate() {
        let mut ed = large_selection();
        CACHE_MISSES.with(|n| n.set(0));
        for _ in 0..100 {
            assert_eq!(ed.clipping_enablement(), (false, true));
        }
        CACHE_MISSES.with(|n| assert_eq!(n.get(), 1));
        ed.group_sel.clear();
        ed.objsel.clear();
        assert_eq!(ed.clipping_enablement(), (false, false));
        CACHE_MISSES.with(|n| assert_eq!(n.get(), 2));
        ed.execute(EditCommand::Undo).unwrap();
        ed.clipping_enablement();
        CACHE_MISSES.with(|n| assert_eq!(n.get(), 3));
        ed.begin();
        ed.clipping_enablement();
        ed.clipping_enablement();
        CACHE_MISSES.with(|n| assert_eq!(n.get(), 5));
    }
    #[test]
    fn cancelled_transaction_does_not_cache_preview_enablement() {
        let mut ed = large_selection();
        assert_eq!(ed.clipping_enablement(), (false, true));
        ed.begin();
        ed.doc.nodes.iter_mut().find(|n| n.role == GroupRole::Clip).unwrap().role = GroupRole::Normal;
        ed.dirty = true;
        assert_eq!(ed.clipping_enablement(), (false, false));
        ed.picker_cancel();
        assert_eq!(ed.clipping_enablement(), (false, true));
    }
    #[test]
    fn menu_cache_follows_release_undo_and_document_replacement() {
        let mut ed = large_selection();
        assert_eq!(ed.clipping_enablement(), (false, true));
        ed.execute(EditCommand::ClipRelease).unwrap();
        assert_eq!(ed.clipping_enablement(), (false, false));
        ed.execute(EditCommand::Undo).unwrap();
        assert_eq!(ed.clipping_enablement(), (false, true));
        ed.replace_doc(Document::default());
        assert_eq!(ed.clipping_enablement(), (false, false));
    }
    #[test]
    fn indexed_unit_deduplicates_large_group_and_partial_release_is_disabled() {
        let mut ed = large_selection();
        let index = ClipIndex::new(&ed);
        let units: std::collections::HashSet<_> = ed.objsel.iter().filter_map(|p| index.unit(*p)).collect();
        assert_eq!(units.len(), 1);
        assert_eq!(index.paths(*units.iter().next().unwrap()).len(), 512);
        assert!(ed.clip_release_enabled());
        ed.group_sel.clear();
        ed.objsel.remove(&1);
        assert!(!ed.clip_release_enabled());
    }
}
