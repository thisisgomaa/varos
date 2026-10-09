//! Semantic checks on authored content. Structural checks run first; these checks never walk the
//! tree recursively or repair it. Both load and save check authored values BEFORE normalization,
//! so pruning a group or clearing a nested transform cannot hide an invalid number.

use super::{Invalid, Limits};
use crate::model::{Document, GroupRole, NodeKind};
use std::collections::{HashMap, HashSet};

/// Validate canonical content, after migration/normalization and the structural precheck.
pub fn validate(doc: &Document, _limits: &Limits) -> Result<(), Invalid> {
    before_artboard_ids(doc)?;
    // format 4: every artboard has its id (uniqueness is structural, `check_structure`), and `active`
    // names an artboard (or is 0 on a free canvas)
    if let Some(index) = doc.artboards.iter().position(|a| a.id == 0) {
        return Err(Invalid::MissingArtboardId { index });
    }
    if doc.active >= doc.artboards.len().max(1) {
        return Err(Invalid::ActiveArtboardOutOfRange { active: doc.active, count: doc.artboards.len() });
    }
    Ok(())
}

/// `validate` without the format-4 artboard checks: a v2/v3 file is checked with exactly the strictness
/// it had when its format was current, before the v3→v4 migration assigns ids.
pub(crate) fn before_artboard_ids(doc: &Document) -> Result<(), Invalid> {
    authored(doc)?;
    let leaves: HashSet<u32> = doc
        .nodes
        .iter()
        .filter_map(|n| match n.kind {
            NodeKind::Path(id) => Some(id),
            _ => None,
        })
        .collect();
    if doc.paths.iter().any(|p| !leaves.contains(&p.id)) {
        return Err(Invalid::NotCanonical { what: "path ownership" });
    }
    Ok(())
}

/// The normalizer may adopt tree-less paths, but must not erase invalid authored values or roles.
/// Invariants: model::{move_is_legal,clip_group,release_clip,sync_tree}, Editor::set_opacity,
/// command::set_stroke_width, Editor::ab_set_rect, and the model/units field contracts.
/// Root paths/groups are permitted by move_is_legal(Before/After a root); do not require Layer roots.
/// candidate_max is currently unused (no live-editor bound); do not invent a new file restriction.
pub(crate) fn authored(doc: &Document) -> Result<(), Invalid> {
    let index: HashMap<u32, _> = doc.nodes.iter().map(|n| (n.id, n)).collect();
    let mut leaves = HashSet::with_capacity(doc.paths.len());
    for n in &doc.nodes {
        if let NodeKind::Path(pid) = n.kind {
            if !n.children.is_empty() {
                return Err(Invalid::BadParentage { node: n.id, reason: "a path cannot contain child nodes" });
            }
            if !leaves.insert(pid) {
                return Err(Invalid::BadParentage { node: n.id, reason: "the path already has a leaf" });
            }
        }
        if n.kind == NodeKind::Layer
            && n.parent.and_then(|id| index.get(&id)).is_some_and(|p| p.kind == NodeKind::Group)
        {
            return Err(Invalid::BadParentage { node: n.id, reason: "a layer cannot be inside a group" });
        }
        let bad_mask = |reason| Invalid::BadMask { group: n.id, reason };
        match n.role {
            role if role.is_mask_group() && role != GroupRole::Clip => {
                return Err(bad_mask("soft masks are not supported"))
            }
            GroupRole::Normal if n.mask_child.is_some() => {
                return Err(bad_mask("an ordinary node cannot have a mask shape"))
            }
            GroupRole::Clip => {
                if n.kind != NodeKind::Group {
                    return Err(bad_mask("only a group can clip its children"));
                }
                let mask = n.mask_child.ok_or_else(|| bad_mask("it has no mask shape"))?;
                if !n.children.contains(&mask) || !index.contains_key(&mask) {
                    return Err(bad_mask("its mask shape is no longer inside it"));
                }
            }
            _ => {}
        }
        let label = format!("node {}", n.id);
        finite(n.xform.rot, &label, "rotation")?;
        point(n.xform.piv, &label, "pivot")?;
        if let Some(c) = n.color {
            color(c, &label, "color")?;
        }
    }
    for p in &doc.paths {
        let label = match &p.name {
            Some(name) => format!("path {} ({name})", p.id),
            None => format!("path {}", p.id),
        };
        for a in p.anchors.iter().chain(p.holes.iter().flatten()) {
            point(a.p, &label, "anchor position")?;
            if let Some(pt) = a.hin {
                point(pt, &label, "incoming handle")?;
            }
            if let Some(pt) = a.hout {
                point(pt, &label, "outgoing handle")?;
            }
        }
        nonnegative(p.stroke_width, &label, "stroke width")?;
        unit(p.opacity, &label, "opacity")?;
        if let Some(c) = p.fill.solid() {
            color(c, &label, "fill")?;
        }
        if let Some(c) = p.stroke.solid() {
            color(c, &label, "stroke")?;
        }
    }
    for (i, b) in doc.artboards.iter().enumerate() {
        let label = format!("artboard {} ({})", i + 1, b.name);
        finite(b.x, &label, "x")?;
        finite(b.y, &label, "y")?;
        nonnegative(b.w, &label, "width")?;
        nonnegative(b.h, &label, "height")?;
        nonnegative(b.bleed, &label, "bleed")?;
        if let Some(c) = b.page_color {
            color(c, &label, "page color")?;
        }
    }
    for (i, g) in doc.guides.iter().enumerate() {
        finite(g.pos, &format!("guide {}", i + 1), "position")?;
    }
    point(doc.ruler_origin, "document", "ruler origin")?;
    finite(doc.snap.radius_px, "snapping", "radius")?;
    finite(doc.snap.grid_spacing, "snapping", "grid spacing")?;
    if !(0.01..=1e6).contains(&doc.snap.grid_spacing) {
        return Err(range(doc.snap.grid_spacing, "snapping", "grid spacing"));
    }
    if !(1..=100).contains(&doc.snap.grid_subdivisions) {
        return Err(range(doc.snap.grid_subdivisions as f32, "snapping", "grid subdivisions"));
    }
    crate::board::check_document(doc).map_err(Invalid::Board)?;
    finite(doc.units.ppi, "document", "ppi")?;
    if doc.units.ppi <= 0.0 {
        return Err(range(doc.units.ppi, "document", "ppi"));
    }
    Ok(())
}

fn finite(value: f32, object: &str, field: &str) -> Result<(), Invalid> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(Invalid::NonFinite { what: format!("{object}: {field}") })
    }
}
fn range(value: f32, object: &str, field: &str) -> Invalid {
    Invalid::OutOfRange { what: format!("{object}: {field}"), value: f64::from(value) }
}
fn nonnegative(value: f32, object: &str, field: &str) -> Result<(), Invalid> {
    finite(value, object, field)?;
    if value < 0.0 {
        Err(range(value, object, field))
    } else {
        Ok(())
    }
}
fn unit(value: f32, object: &str, field: &str) -> Result<(), Invalid> {
    finite(value, object, field)?;
    if !(0.0..=1.0).contains(&value) {
        Err(range(value, object, field))
    } else {
        Ok(())
    }
}
fn point(p: [f32; 2], object: &str, field: &str) -> Result<(), Invalid> {
    for v in p {
        finite(v, object, field)?;
    }
    Ok(())
}
fn color(c: [f32; 4], object: &str, field: &str) -> Result<(), Invalid> {
    for v in c {
        unit(v, object, field)?;
    }
    Ok(())
}
