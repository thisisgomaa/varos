//! Lane F: session-only authored history. No document format changes.
use super::Editor;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Actor {
    Human,
    Agent { profile_id: String, label: String },
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Summary {
    pub created: u32,
    pub changed: u32,
    pub removed: u32,
    pub verbs: Vec<String>,
    pub artboards: Vec<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub rev_after: u64,
    pub actor: Actor,
    pub label: String,
    pub summary: Summary,
    pub at: u64,
}
#[derive(Clone, Default)]
pub(super) struct Log {
    pub undo: Vec<HistoryEntry>,
    pub redo: Vec<HistoryEntry>,
    pub limit: usize,
}
impl Log {
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
    pub fn push(&mut self, rev: u64, before: &crate::model::Document, after: &crate::model::Document) {
        let old: std::collections::HashMap<_, _> = before.paths.iter().map(|p| (p.id, p)).collect();
        let new: std::collections::HashMap<_, _> = after.paths.iter().map(|p| (p.id, p)).collect();
        let summary = Summary {
            created: new.keys().filter(|id| !old.contains_key(id)).count() as u32,
            removed: old.keys().filter(|id| !new.contains_key(id)).count() as u32,
            changed: new.iter().filter(|(id, p)| old.get(id).is_some_and(|old| old != *p)).count() as u32,
            verbs: vec![],
            artboards: after.artboards.iter().map(|a| a.id).collect(),
        };
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis().min(u64::MAX as u128) as u64);
        self.undo.push(HistoryEntry { rev_after: rev, actor: Actor::Human, label: "Edit artwork".into(), summary, at });
        self.redo.clear();
    }
}
impl Editor {
    pub fn history_redo_entries(&self)->&[HistoryEntry]{&self.history_log.redo}
    pub fn history_entries(&self) -> &[HistoryEntry] {
        &self.history_log.undo
    }
    pub fn history_depths(&self) -> (usize, usize) {
        (self.undo.len(), self.redo.len())
    }
    pub fn set_history_depth(&mut self, limit: usize) -> Result<(), String> {
        if !(5..=1000).contains(&limit) {
            return Err("History depth must be 5–1000".into());
        }
        if self.transaction_open() {
            return Err("Finish the current edit before changing history depth".into());
        }
        self.history_log.limit = limit;
        self.trim_history();
        Ok(())
    }
    pub(super) fn trim_history(&mut self) {
        let limit = self.history_log.limit;
        let drop = self.undo.len().saturating_sub(limit);
        self.undo.drain(..drop);
        self.history_log.undo.drain(..drop);
        let drop = self.redo.len().saturating_sub(limit);
        self.redo.drain(..drop);
        self.history_log.redo.drain(..drop);
    }
    /// Annotate only a successfully published new step, never a failed/no-op batch.
    pub fn annotate_history(&mut self, before_rev: u64, actor: Actor, label: String) {
        if self.rev > before_rev {
            if let Some(e) = self.history_log.undo.last_mut() {
                if e.rev_after == self.rev {
                    e.actor = actor;
                    e.summary.verbs = vec![label.clone()];
                    e.label = label;
                }
            }
        }
    }
    pub fn history_jump(&mut self, undo_depth: usize) -> Result<(), String> {
        if self.transaction_open() {
            return Err("Finish the current edit before jumping history".into());
        }
        if undo_depth > self.undo.len() + self.redo.len() {
            return Err("History position is no longer available".into());
        }
        while self.undo.len() > undo_depth {
            self.undo();
        }
        while self.undo.len() < undo_depth {
            self.redo();
        }
        Ok(())
    }
    pub fn undo_agent(&mut self, profile_id: &str) -> Result<(), String> {
        if !self
            .history_entries()
            .last()
            .is_some_and(|e| matches!(&e.actor,Actor::Agent{profile_id:id,..} if id==profile_id))
        {
            return Err("This agent does not own the latest history step".into());
        }
        self.history_jump(self.undo.len() - 1)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_tracks_caps_jumps_and_agent_top_rule() {
        let mut e = Editor::new();
        for i in 0..8 {
            e.begin();
            e.doc.units.ppi = 72.0 + i as f32;
            e.dirty = true;
            e.commit();
        }
        assert_eq!(e.history_entries().len(), 8);
        let rev = e.rev;
        e.annotate_history(
            rev - 1,
            Actor::Agent { profile_id: "a".into(), label: "Claude".into() },
            "Agent batch".into(),
        );
        assert!(e.undo_agent("b").is_err());
        e.undo_agent("a").unwrap();
        assert_eq!(e.history_depths(), (7, 1));
        e.redo();
        e.set_history_depth(5).unwrap();
        assert_eq!(e.history_entries().len(), 5);
        e.history_jump(0).unwrap();
        assert_eq!(e.history_depths(), (0, 5));
        e.history_jump(5).unwrap();
        assert_eq!(e.history_entries().len(), 5);
        e.replace_doc(e.doc.clone());
        assert!(e.history_entries().is_empty());
    }
    #[test]
    fn open_edit_refuses_jump_and_trim() {
        let mut e = Editor::new();
        e.begin();
        assert!(e.history_jump(0).is_err());
        assert!(e.set_history_depth(5).is_err());
    }
}
