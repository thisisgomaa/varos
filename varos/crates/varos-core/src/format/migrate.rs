//! Format migrations (work order §3 step 7, ADR-0008 rule 5): a sequential table of pure, deterministic
//! steps. Each takes a decoded document of format N and returns the same content in format N+1.
//! Migrations run in memory only; the file on disk is untouched until the user saves.

use super::error::{Invalid, LoadError};
use super::limits::Limits;
use super::structure::max_used_id;
use crate::model::{Document, GroupRole, NodeKind};
use std::collections::{HashMap, HashSet};

/// A migration step: format `from` → `from + 1`.
pub type Step = fn(Document, &Limits) -> Result<Document, LoadError>;

/// The sequential table. Loading format N runs every step from N up to `FORMAT_VERSION`, in order.
pub const MIGRATIONS: &[(u32, Step)] = &[
    (1, migrate_v1_to_v2),
    (2, migrate_v2_to_v3),
    (3, migrate_v3_to_v4),
    (4, migrate_v4_to_v5),
    (5, migrate_v5_to_v6),
    (6, migrate_v6_to_v7),
    (7, migrate_v7_to_v8),
    (8, migrate_v8_to_v9),
];

/// Every format this build reads: each migration start plus the current writer (integration w2: the
/// Bridge 1.2 `readable_vrs` list derives from this, so reserved/skipped numbers are never advertised).
pub fn readable_versions() -> Vec<u32> {
    let mut v: Vec<u32> = MIGRATIONS.iter().map(|(from, _)| *from).collect();
    v.push(super::FORMAT_VERSION);
    v
}

/// Run the migrations that take a format-`from` document to format `to`, in order.
pub fn migrate(mut doc: Document, from: u32, to: u32, limits: &Limits) -> Result<Document, LoadError> {
    let mut v = from;
    while v < to {
        let Some(&(_, step)) = MIGRATIONS.iter().find(|(f, _)| *f == v) else {
            return Err(LoadError::MigrationFailed { from: v, reason: format!("no migration from format {v}") });
        };
        doc = step(doc, limits)?;
        v += 1;
    }
    Ok(doc)
}

/// v7 → v8 (editable text): legacy readers supplied the empty default; never fabricates
/// source/fonts (`text_format::migrate_to_text_boxes`).
pub fn migrate_v7_to_v8(doc: Document, limits: &Limits) -> Result<Document, LoadError> {
    crate::text_format::migrate_to_text_boxes(doc, limits)
}

/// v1 → v2. The v2 model is the v1 model, so this only performs the documented v1 normalizations
/// (`sync_tree`): the legacy group registry becomes tree nodes with z order kept, tree-less paths are
/// adopted by the active layer, empty groups are pruned, nested live transforms are healed. It never
/// repairs additional mask meaning: decode_model already releases the explicitly allowed broken-v1
/// clips, retaining a notice. Any further clip change here is refused (`Invalid::BadMask`).
/// The id counter is raised to cover every id in use.
///
/// Requires a document that passed `check_structure` (`decode_model` runs it first): the tree walks in
/// `sync_tree` assume an acyclic, depth-bounded tree.
pub fn migrate_v1_to_v2(doc: Document, _limits: &Limits) -> Result<Document, LoadError> {
    normalize(doc)
}

/// v2 → v3 (2026-10-04, board metadata). Format 3 adds `doc.name`, `doc.description` and `doc.tags`; a
/// v2 file has none of them (`decode_model` refuses the keys in a v2 file), so the typed decode already
/// gave them their defaults: empty name (= "use the file stem", `board::display_name`), empty
/// description, no tags. Nothing else changes; the input already passed the v2 canonical check.
pub fn migrate_v2_to_v3(doc: Document, _limits: &Limits) -> Result<Document, LoadError> {
    debug_assert!(doc.name.is_empty() && doc.description.is_empty() && doc.tags.is_empty());
    Ok(doc)
}

/// v3 → v4 (2026-10-07, stable artboard ids, Bridge slice 3). Format 4 adds `doc.artboards[].id`; a v3
/// file has none (`decode_model` refuses the key in a v1–v3 file), so every artboard decoded with id 0.
/// Assign ids deterministically, in artboard order, from the document id counter (`Document::nid`,
/// which the v3 canonical pass / v1 normalization already raised to cover every id in use), and clamp a
/// stale `active` index into range (format 4 refuses a dangling one; a v3 reader clamped it on read).
/// Nothing else changes.
pub fn migrate_v3_to_v4(mut doc: Document, _limits: &Limits) -> Result<Document, LoadError> {
    debug_assert!(doc.artboards.iter().all(|a| a.id == 0));
    doc.ids = doc.ids.max(max_used_id(&doc));
    doc.assign_artboard_ids();
    clamp_active(&mut doc);
    Ok(doc)
}

/// The canonical `active` index: within `artboards`, or 0 on a free canvas. Every editor path keeps
/// this already (`ab_delete`, `ab_set_count`, …); the format refuses anything else from format 4.
pub(crate) fn clamp_active(doc: &mut Document) {
    doc.active = doc.active.min(doc.artboards.len().saturating_sub(1));
}

/// The shared normalizer: migration of v1 files, and the save-side pass on a clone of the editor's
/// document (so this build writes only canonical files). Requires `check_structure` to have passed.
pub(crate) fn normalize(mut doc: Document) -> Result<Document, LoadError> {
    // Raise the counter BEFORE `sync_tree`: its `nid()` would otherwise hand out ids already in use.
    doc.ids = doc.ids.max(max_used_id(&doc));
    let clips: Vec<(u32, GroupRole, Option<u32>)> =
        doc.nodes.iter().filter(|n| n.role.is_mask_group()).map(|n| (n.id, n.role, n.mask_child)).collect();
    doc.sync_tree();
    if !clips.is_empty() {
        let after: HashMap<u32, (GroupRole, Option<u32>)> =
            doc.nodes.iter().map(|n| (n.id, (n.role, n.mask_child))).collect();
        for (group, role, mask_child) in &clips {
            if after.get(group) != Some(&(*role, *mask_child)) {
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

/// Owner-approved v1 exception: clear only a Group's broken clip reference, before structural
/// checking/id allocation. Never adopt a dangling mask id or repair another kind of corruption.
/// This pass performs no tree traversal; all other references still pass the strict precheck.
pub(crate) fn release_broken_clips(doc: &mut Document) -> bool {
    let ids: HashSet<u32> = doc.nodes.iter().map(|n| n.id).collect();
    let mut released = false;
    for n in &mut doc.nodes {
        if n.kind == NodeKind::Group
            && n.role == GroupRole::Clip
            && !n.mask_child.is_some_and(|id| ids.contains(&id) && n.children.contains(&id))
        {
            n.role = GroupRole::Normal;
            n.mask_child = None;
            released = true;
        }
    }
    released
}

/// v4 → v5 is identity: omitted styles decode as defaults, without allocation or normalization.
pub fn migrate_v4_to_v5(doc: Document, _limits: &Limits) -> Result<Document, LoadError> {
    Ok(doc)
}

// ---- w2-images ----
/// v5 → v6 (images) is identity: no source bytes or image identities are invented.
pub fn migrate_v5_to_v6(doc: Document, _limits: &Limits) -> Result<Document, LoadError> {
    Ok(doc)
}

// ---- w2-gradients ----
/// v6 → v7 (gradient paints + document swatches) is a pure identity: old paints and absent
/// swatches are already the canonical stored form. No validation here (the loader validates).
pub fn migrate_v6_to_v7(doc: Document, _limits: &Limits) -> Result<Document, LoadError> {
    Ok(doc)
}

// ---- Lane C ----
/// v8 → v9 (Live Corners) is identity: older writers did not emit `Path.corners`. The app lane's
/// embedded preview is container-level (PDF catalog) and needs no model step.
pub fn migrate_v8_to_v9(doc: Document, _limits: &Limits) -> Result<Document, LoadError> {
    Ok(doc)
}
