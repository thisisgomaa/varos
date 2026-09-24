//! Format migrations (work order §3 step 7, ADR-0008 rule 5): a sequential table of pure, deterministic
//! steps. Each takes a decoded document of format N and returns the same content in format N+1.
//! Migrations run in memory only; the file on disk is untouched until the user saves.

use super::error::{Invalid, LoadError};
use super::limits::Limits;
use super::structure::max_used_id;
use crate::model::{Document, GroupRole};
use std::collections::HashMap;

/// A migration step: format `from` → `from + 1`.
pub type Step = fn(Document, &Limits) -> Result<Document, LoadError>;

/// The sequential table. Loading format N runs every step from N up to `FORMAT_VERSION`, in order.
pub const MIGRATIONS: &[(u32, Step)] = &[(1, migrate_v1_to_v2)];

/// Run the migrations that take a format-`from` document to format `to`, in order.
pub fn migrate(mut doc: Document, from: u32, to: u32, limits: &Limits) -> Result<Document, LoadError> {
    for v in from..to {
        let Some(&(_, step)) = MIGRATIONS.iter().find(|(f, _)| *f == v) else {
            return Err(LoadError::MigrationFailed { from: v, reason: format!("no migration from format {v}") });
        };
        doc = step(doc, limits)?;
    }
    Ok(doc)
}

/// v1 → v2. The v2 model is the v1 model, so this only performs the documented v1 normalizations
/// (`sync_tree`): the legacy group registry becomes tree nodes with z order kept, tree-less paths are
/// adopted by the active layer, empty groups are pruned, nested live transforms are healed. It never
/// repairs authored mask meaning: a clip group that `sync_tree` would demote is refused
/// (`Invalid::BadMask`). The id counter is raised to cover every id in use.
///
/// Requires a document that passed `check_structure` (`decode_model` runs it first): the tree walks in
/// `sync_tree` assume an acyclic, depth-bounded tree.
pub fn migrate_v1_to_v2(doc: Document, _limits: &Limits) -> Result<Document, LoadError> {
    normalize(doc)
}

/// The shared normalizer: migration of v1 files, and the save-side pass on a clone of the editor's
/// document (so this build writes only canonical files). Requires `check_structure` to have passed.
pub(crate) fn normalize(mut doc: Document) -> Result<Document, LoadError> {
    // Raise the counter BEFORE `sync_tree`: its `nid()` would otherwise hand out ids already in use.
    doc.ids = doc.ids.max(max_used_id(&doc));
    let clips: Vec<(u32, Option<u32>)> =
        doc.nodes.iter().filter(|n| n.role == GroupRole::Clip).map(|n| (n.id, n.mask_child)).collect();
    doc.sync_tree();
    if !clips.is_empty() {
        let after: HashMap<u32, (GroupRole, Option<u32>)> =
            doc.nodes.iter().map(|n| (n.id, (n.role, n.mask_child))).collect();
        for (group, mask_child) in &clips {
            if after.get(group) != Some(&(GroupRole::Clip, *mask_child)) {
                // a mask id naming no node is refused earlier, by `check_structure` (Dangling)
                let reason = match mask_child {
                    None => "it has no mask shape",
                    Some(_) => "its mask shape is no longer inside it",
                };
                return Err(Invalid::BadMask { group: *group, reason }.into());
            }
        }
    }
    Ok(doc)
}
