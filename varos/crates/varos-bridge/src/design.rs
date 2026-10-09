//! Validated design operations on isolated core state.
use crate::{
    dto::*,
    service::{paint, resolve},
    MAX_TARGETS,
};
use std::collections::{BTreeMap, BTreeSet};
use varos_core::{
    bridge::{ArtboardError, ArtboardErrorCode, TargetEdit},
    editor::{AlignMode, AlignTarget, DistAxis, Editor, ZOrder},
    model::NodeKind,
    EditCommand,
};
fn fail(reason: impl Into<String>) -> Error {
    let reason = reason.into();
    let code = if reason.contains("limit_exceeded:") {
        "limit_exceeded"
    } else if reason.starts_with("internal error:") {
        "internal"
    } else {
        "invalid_argument"
    };
    Error::new(code, reason)
}
pub(crate) fn canonical(id: &str) -> Result<(&str, u32), Error> {
    let (kind, value) = id.split_once(':').ok_or_else(|| fail("use path:N or node:N"))?;
    let n = value.parse::<u32>().map_err(|_| fail("id suffix must be u32"))?;
    if n == 0 || format!("{kind}:{n}") != id || !["path", "node"].contains(&kind) {
        return Err(fail("object id must be canonical path:N or node:N"));
    }
    Ok((kind, n))
}
pub(crate) fn local_name(name: &str) -> Result<(), Error> {
    let bytes = name.as_bytes();
    if bytes.len() < 2
        || bytes.len() > 64
        || bytes[0] != b'$'
        || !bytes[1].is_ascii_alphabetic()
        || !bytes[1..].iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_')
    {
        return Err(fail("local must be $name (ASCII identifier, at most 64 bytes)"));
    }
    Ok(())
}
fn bind(locals: &mut BTreeMap<String, String>, name: &Option<String>, id: String) -> Result<(), Error> {
    if let Some(name) = name {
        local_name(name)?;
        if locals.insert(name.clone(), id).is_some() {
            return Err(fail("duplicate request-local name"));
        }
    }
    Ok(())
}
fn clean_name(name: &str) -> Result<String, Error> {
    if name.chars().count() > 256 {
        return Err(Error::new("limit_exceeded", "name exceeds 256 characters"));
    }
    let name = varos_core::command::clean_name(name);
    if name.is_empty() {
        return Err(fail("name must not be empty"));
    }
    Ok(name.to_owned())
}
/// `artboard:N` (canonical, N > 0) or a request-local bound to one, resolved to the stable id.
fn artboard_ref(id: &str, locals: &BTreeMap<String, String>) -> Result<u32, Error> {
    let id = if id.starts_with('$') {
        locals.get(id).ok_or_else(|| Error::new("not_found", format!("unknown request-local {id}")))?.as_str()
    } else {
        id
    };
    id.strip_prefix("artboard:")
        .and_then(|n| n.parse::<u32>().ok())
        .filter(|n| *n > 0 && format!("artboard:{n}") == id)
        .ok_or_else(|| fail(format!("{id} is not an artboard; use artboard:N or a request-local bound to one")))
}
fn artboard_error(e: ArtboardError) -> Error {
    Error::new(
        match e.code {
            ArtboardErrorCode::NotFound => "not_found",
            ArtboardErrorCode::LockedTarget => "locked_target",
            ArtboardErrorCode::InvalidArgument => "invalid_argument",
            ArtboardErrorCode::LimitExceeded => "limit_exceeded",
        },
        e.reason,
    )
}
/// The page verbs (slice 3). Identity is the stable artboard id; the active page never changes as a
/// side effect (see `Editor::artboard_delete` for the active-artboard rule).
fn apply_artboard_op(
    ed: &mut Editor,
    op: &Operation,
    locals: &mut BTreeMap<String, String>,
    affected: &mut BTreeSet<String>,
) -> Result<(), Error> {
    match op {
        Operation::AddArtboard { bounds, preset, origin, name, local } => {
            if let Some(local) = local {
                local_name(local)?;
                if locals.contains_key(local) {
                    return Err(fail("duplicate request-local name"));
                }
            }
            let rect = match (bounds, preset) {
                (Some(b), None) => {
                    if origin.is_some() {
                        return Err(fail("origin goes with preset; bounds already place the artboard"));
                    }
                    *b
                }
                (None, Some(p)) => {
                    let p = varos_core::board::preset(match p {
                        ArtboardPreset::Square => varos_core::board::PresetId::Square,
                        ArtboardPreset::Portrait => varos_core::board::PresetId::Portrait,
                        ArtboardPreset::Story => varos_core::board::PresetId::Story,
                        ArtboardPreset::A4 => varos_core::board::PresetId::A4,
                    });
                    let ppi = ed.doc.units.ppi;
                    let (w, h) = (p.w * p.unit.pt_per(ppi), p.h * p.unit.pt_per(ppi));
                    let [x, y] = origin.unwrap_or_else(|| ed.artboard_next_origin());
                    [x, y, w, h]
                }
                _ => return Err(fail("add_artboard needs exactly one of bounds or preset")),
            };
            let name = name.as_ref().map(|n| clean_name(n)).transpose()?;
            let id = ed.artboard_add(rect, name).map_err(artboard_error)?;
            bind(locals, local, format!("artboard:{id}"))?;
        }
        Operation::ResizeArtboard { id, bounds } => {
            ed.artboard_set_rect(artboard_ref(id, locals)?, *bounds).map_err(artboard_error)?
        }
        Operation::RenameArtboard { id, name } => {
            let name = clean_name(name)?;
            ed.artboard_rename(artboard_ref(id, locals)?, name).map_err(artboard_error)?
        }
        Operation::DeleteArtboard { id } => {
            let id = artboard_ref(id, locals)?;
            affected.insert(format!("artboard:{id}"));
            ed.artboard_delete(id).map_err(artboard_error)?
        }
        Operation::SetActiveArtboard { id } => {
            ed.artboard_set_active(artboard_ref(id, locals)?).map_err(artboard_error)?
        }
        Operation::ReorderArtboard { id, position } => {
            ed.artboard_reorder(artboard_ref(id, locals)?, *position).map_err(artboard_error)?
        }
        Operation::DuplicateArtboard { id, with_art, offset, local } => {
            if let Some(n) = local {
                local_name(n)?;
                if locals.contains_key(n) {
                    return Err(fail("duplicate request-local name"));
                }
            }
            let id = ed.artboard_duplicate(artboard_ref(id, locals)?, *with_art, *offset).map_err(artboard_error)?;
            bind(locals, local, format!("artboard:{id}"))?;
        }
        Operation::SetArtboardColor { id, color } => {
            let color = paint(&color.clone().map_or(Paint::None, Paint::Solid))?.flatten();
            ed.artboard_color(artboard_ref(id, locals)?, color).map_err(artboard_error)?;
        }
        Operation::SetArtboardClip { id, clip } => {
            ed.artboard_clip(artboard_ref(id, locals)?, *clip).map_err(artboard_error)?
        }
        _ => unreachable!("not a page verb"),
    }
    Ok(())
}
/// DEPRECATED (slice 3, kept for one slice): the revision-bound `aI@rev` reference of slice 1/2. It
/// names the page at index I of revision `rev` and is valid only when `rev` is the request's revision;
/// it resolves against the staged document, so the service refuses it in any batch that also has a
/// page verb (review P2: an earlier delete would otherwise retarget it). Use `artboard:N` instead.
fn legacy_artboard_ref(target: &str, original_rev: u64) -> Result<usize, Error> {
    let (index, rev) = target.strip_prefix('a').and_then(|s| s.split_once('@')).ok_or_else(|| {
        fail("align target must be selection, artboard:N, an artboard request-local, or the deprecated aN@revision")
    })?;
    let index = index.parse::<usize>().map_err(|_| fail("invalid artboard reference"))?;
    let rev = rev.parse::<u64>().map_err(|_| fail("invalid artboard revision"))?;
    if format!("a{index}@{rev}") != target {
        return Err(fail("artboard reference must be canonical"));
    }
    if rev != original_rev {
        return Err(Error::new("revision_conflict", "artboard reference is revision-bound"));
    }
    Ok(index)
}
fn whole_units(ed: &Editor, paths: &[u32]) -> Result<Vec<u32>, Error> {
    let chosen: BTreeSet<_> = paths.iter().copied().collect();
    let units: BTreeSet<_> = paths.iter().filter_map(|p| ed.doc.unit_of(*p)).collect();
    for unit in &units {
        if ed.doc.node_paths(*unit).iter().any(|p| !chosen.contains(p)) {
            return Err(Error::new("unsupported", "operation requires complete objects or groups"));
        }
    }
    Ok(units.into_iter().collect())
}
pub(crate) fn apply_design_op(
    ed: &mut Editor,
    op: &Operation,
    original_rev: u64,
    locals: &mut BTreeMap<String, String>,
    expanded: &mut usize,
    affected: &mut BTreeSet<String>,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<u32>, Error> {
    // ---- Lane D: resolve locals and cap expanded targets before drawing ----
    if op.drawing() {
        let targets = op
            .ids()
            .iter()
            .map(|id| {
                if id.starts_with('$') {
                    locals.get(id).cloned().ok_or_else(|| Error::new("not_found", "unknown request-local target"))
                } else {
                    Ok(id.clone())
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let paths = crate::service::resolve(
            &ed.doc,
            &targets,
            matches!(
                op,
                Operation::ShapeTool { .. }
                    | Operation::Pencil { .. }
                    | Operation::Curvature { .. }
                    | Operation::DrawingOptions { .. }
            ),
        )?;
        *expanded += paths.len();
        if *expanded > MAX_TARGETS {
            return Err(Error::new("limit_exceeded", "batch expanded targets exceed 1000"));
        }
        return crate::drawing::apply(ed, op, paths, affected);
    }
    // ---- Lane G ----
    if crate::text::apply(ed, op, locals, affected)? {
        return Ok(None);
    }
    // ---- Lane C ----
    if op.lane_c() {
        return crate::path_advanced::apply(ed, op, locals, affected, expanded);
    }
    if op.slice4a() {
        let mut resolved = op.clone();
        let ids = match &mut resolved {
            // ---- w3-cmyk ----
            Operation::ColourManagement { ids, .. }
            | Operation::Colour { ids, .. }
            | Operation::Transform { ids, .. }
            | Operation::MagicWand { ids, .. }
            | Operation::Eyedropper { ids, .. }
            | Operation::Isolation { ids, .. }
            | Operation::Layers { ids, .. } => Some(ids),
            _ => None,
        };
        if let Some(ids) = ids {
            for id in ids {
                if id.starts_with('$') {
                    *id = locals
                        .get(id)
                        .cloned()
                        .ok_or_else(|| Error::new("not_found", "unknown request-local target"))?;
                }
            }
        }
        if let Operation::Eyedropper { source, .. } = &mut resolved {
            if source.starts_with('$') {
                *source = locals
                    .get(source)
                    .cloned()
                    .ok_or_else(|| Error::new("not_found", "unknown request-local source"))?;
            }
        }
        let count = resolved
            .ids()
            .iter()
            .map(|id| {
                let (kind, n) = canonical(id)?;
                Ok(if kind == "path" { 1 } else { ed.doc.node_paths(n).len() })
            })
            .collect::<Result<Vec<usize>, Error>>()?
            .into_iter()
            .sum::<usize>();
        *expanded += count;
        if *expanded > MAX_TARGETS {
            return Err(Error::new("limit_exceeded", "batch expanded targets exceed 1000"));
        }
        return crate::select_transform::apply(ed, &resolved, affected);
    }
    if let Operation::TraceRgba { rgba, width, height, options } = op {
        let (paths, _) = varos_core::trace::trace(rgba, *width, *height, options).map_err(fail)?;
        let before: BTreeSet<_> = ed.doc.paths.iter().map(|p| p.id).collect();
        ed.try_execute(EditCommand::InsertTracedPaths { paths }).map_err(fail)?;
        for p in &ed.doc.paths {
            if !before.contains(&p.id) {
                affected.insert(format!("path:{}", p.id));
            }
        }
        return Ok(None);
    }
    #[cfg(test)]
    if matches!(op, Operation::Rename { name, .. } if name == "__forced_adapter_panic__") {
        ed.doc.name = "corrupted staged document".into();
        panic!("forced adapter panic");
    }
    if let Operation::DocumentSetup { field, value, artboard } = op {
        use varos_core::document_setup as setup;
        let command = match field.as_str() {
            "units" => EditCommand::SetUnits(
                value.as_str().and_then(varos_core::Unit::parse_suffix).ok_or_else(|| fail("unknown units"))?,
            ),
            "ppi" => {
                let ppi = value.as_f64().ok_or_else(|| fail("ppi must be a number"))? as f32;
                if !setup::valid_ppi(ppi) {
                    return Err(fail("ppi must be 1..9600"));
                }
                EditCommand::SetPpi(ppi)
            }
            "transparency_grid" => {
                EditCommand::SetTransparencyGrid(value.as_bool().ok_or_else(|| fail("grid must be boolean"))?)
            }
            "bleed" => {
                let edges: [f32; 4] =
                    serde_json::from_value(value.clone()).map_err(|_| fail("bleed needs top/right/bottom/left"))?;
                if !setup::valid_bleed(edges) {
                    return Err(fail("bleed must be finite 0..7200 pt"));
                }
                let id = artboard.as_deref().ok_or_else(|| fail("bleed needs artboard:N"))?;
                let id = artboard_ref(id, locals)?;
                let index = ed.doc.artboard_index(id).ok_or_else(|| fail("unknown artboard"))?;
                EditCommand::SetBleed { index, edges }
            }
            _ => return Err(fail("unknown setup field")),
        };
        ed.try_execute(command).map_err(fail)?;
        return Ok(None);
    }
    if op.is_page_verb() {
        return apply_artboard_op(ed, op, locals, affected).map(|()| None);
    }
    if let Operation::Object { ids, action, anchors } = op {
        if anchors.is_some() && *action != varos_core::editor::wave::ObjectAction::Average {
            return Err(fail("anchors are only accepted by average"));
        }
        use varos_core::editor::wave::ObjectAction as O;
        if matches!(action, O::UnlockAll | O::ShowAll | O::CleanUp | O::NewLayer | O::NewSublayer) {
            if !ids.is_empty() {
                return Err(fail("global object action must omit targets"));
            }
            if op.destructive() {
                affected.extend(ed.doc.paths.iter().map(|p| format!("path:{}", p.id)));
            }
            ed.try_execute(EditCommand::Object(*action)).map_err(fail)?;
            return Ok(None);
        }
    }
    let mut created = None;
    let ids = op
        .ids()
        .iter()
        .map(|id| {
            if id.starts_with('$') {
                locals.get(id).cloned().ok_or_else(|| Error::new("not_found", format!("unknown request-local {id}")))
            } else {
                canonical(id)?;
                Ok(id.clone())
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let new_style = match op {
        Operation::SetStrokeStyle { stroke_style, .. } => Some(stroke_style),
        Operation::SetPaint { stroke_style, .. } => stroke_style.as_ref(),
        _ => None,
    };
    if new_style.is_some() && ids.iter().any(|id| !id.starts_with("path:")) {
        return Err(Error::new("unsupported", "stroke style targets must be explicit editable paths"));
    }
    let paths = if matches!(op, Operation::AddShape { .. } | Operation::AddPath { .. }) {
        vec![]
    } else if matches!(op, Operation::View { action, .. } if !matches!(action,
        varos_core::editor::view_commands::ViewAction::MakeGuides
        | varos_core::editor::view_commands::ViewAction::ReleaseGuides
        | varos_core::editor::view_commands::ViewAction::ConvertArtboards
        | varos_core::editor::view_commands::ViewAction::FitArtboard { selected: true, .. }))
    {
        if !ids.is_empty() {
            return Err(fail("global view action requires empty ids"));
        }
        if matches!(op, Operation::View { action: varos_core::editor::view_commands::ViewAction::ClearGuides, .. }) {
            affected.extend(ed.doc.guide_paths.iter().map(|p| format!("path:{p}")));
        }
        vec![]
    } else if matches!(op, Operation::Rename { .. }) {
        if ids.is_empty() {
            return Err(fail("explicit targets must not be empty"));
        }
        let mut paths = BTreeSet::new();
        for id in &ids {
            let (kind, n) = canonical(id)?;
            if kind == "node" {
                let node = ed.doc.node(n).ok_or_else(|| Error::new("not_found", format!("unknown {id}")))?;
                if node.locked || node.hidden {
                    return Err(Error::new(
                        if node.locked { "locked_target" } else { "hidden_target" },
                        "node is inert",
                    ));
                }
                if node.kind == NodeKind::Layer && node.children.is_empty() {
                    continue;
                }
            }
            paths.extend(resolve(&ed.doc, std::slice::from_ref(id), false)?);
        }
        paths.into_iter().collect()
    } else {
        resolve(&ed.doc, &ids, false)?
    };
    if let Some(style) = new_style {
        let mut budget = varos_core::stroke::evaluate::StrokeBudget::default();
        for pid in &paths {
            let Some(index) = ed.doc.pidx(*pid) else {
                continue;
            };
            let mut proposed = ed.doc.paths[index].clone();
            proposed.stroke_style = style.clone();
            if let Operation::SetPaint { stroke_width: Some(width), .. } = op {
                proposed.stroke_width = *width;
            }
            let coverage = varos_core::stroke::evaluate(&proposed, 0.01, cancelled).map_err(|e| {
                Error::new(
                    match e {
                        varos_core::stroke::StrokeError::Cancelled => "cancelled",
                        varos_core::stroke::StrokeError::LimitExceeded => "limit_exceeded",
                        _ => "invalid_argument",
                    },
                    e.to_string(),
                )
            })?;
            budget.charge(&coverage).map_err(|e| Error::new("limit_exceeded", e.to_string()))?;
        }
    }
    *expanded += paths.len();
    if *expanded > MAX_TARGETS {
        return Err(Error::new("limit_exceeded", "batch expanded targets exceed 1000"));
    }
    if op.destructive() {
        affected.extend(ids.iter().cloned());
        for pid in &paths {
            affected.insert(format!("path:{pid}"));
            if let Some(n) = ed.doc.node_of_path(*pid) {
                affected.insert(format!("node:{n}"));
            }
        }
        for n in &ed.doc.nodes {
            let descendants = ed.doc.node_paths(n.id);
            if n.kind == NodeKind::Group && !descendants.is_empty() && descendants.iter().all(|p| paths.contains(p)) {
                affected.insert(format!("node:{}", n.id));
            }
        }
    }
    let execute = |ed: &mut Editor, command| ed.try_execute(command).map_err(fail);
    if let Operation::View { ids, action } = op {
        use varos_core::editor::view_commands::ViewAction as V;
        let targeted = matches!(
            action,
            V::MakeGuides | V::ReleaseGuides | V::ConvertArtboards | V::FitArtboard { selected: true, .. }
        );
        if targeted {
            if ids.is_empty() {
                return Err(fail("view action requires explicit artwork targets"));
            }
            whole_units(ed, &paths)?;
            execute(ed, EditCommand::SelectPaths(paths.clone()))?;
        } else if !ids.is_empty() {
            return Err(fail("global view action requires empty ids"));
        }
        execute(ed, EditCommand::View(*action))?;
        return Ok(None);
    }
    match op {
        Operation::TraceRgba { .. } => return Err(fail("trace dispatch failed")),
        Operation::AddShape { parent, local, name, fill, stroke, stroke_width, opacity, .. }
        | Operation::AddPath { parent, local, name, fill, stroke, stroke_width, opacity, .. } => {
            if let Some(local) = local {
                local_name(local)?;
                if locals.contains_key(local) {
                    return Err(fail("duplicate request-local name"));
                }
            }
            let parent = parent
                .as_ref()
                .map(|id| {
                    let (kind, n) = canonical(id)?;
                    if kind != "node" {
                        return Err(fail("parent must be node:N layer"));
                    }
                    let node = ed.doc.node(n).ok_or_else(|| Error::new("not_found", "unknown parent layer"))?;
                    if node.kind != NodeKind::Layer {
                        return Err(Error::new("unsupported", "parent must be a layer"));
                    }
                    if node.hidden || node.locked {
                        return Err(Error::new(
                            if node.locked { "locked_target" } else { "hidden_target" },
                            "parent layer is inert",
                        ));
                    }
                    Ok(n)
                })
                .transpose()?;
            let n = ed
                .doc
                .node(parent.unwrap_or(ed.doc.active_layer))
                .ok_or_else(|| Error::new("not_found", "unknown active layer"))?;
            if n.hidden || n.locked {
                return Err(Error::new(
                    if n.locked { "locked_target" } else { "hidden_target" },
                    "parent layer is inert",
                ));
            }
            let fill = paint(fill)?.unwrap_or(None);
            let stroke = paint(stroke)?.unwrap_or(None);
            if fill.is_none() && stroke.is_none() {
                return Err(fail("paint required: add_shape needs an explicit non-null fill or stroke"));
            }
            if fill.is_none() && stroke_width.unwrap_or(0.0) <= 0.0 {
                return Err(fail("stroke_width must be > 0 when stroke is the only paint"));
            }
            let command = match op {
                Operation::AddPath { anchors, closed, .. } => {
                    if anchors.len() > 1000 {
                        return Err(Error::new("limit_exceeded", "path exceeds 1000 anchors"));
                    }
                    EditCommand::AddPath {
                        anchors: anchors
                            .iter()
                            .map(|a| varos_core::model::Anchor {
                                id: 0,
                                p: a.p,
                                hin: a.hin,
                                hout: a.hout,
                                smooth: a.smooth,
                            })
                            .collect(),
                        closed: *closed,
                        parent,
                        fill,
                        stroke,
                        stroke_width: stroke_width.unwrap_or(0.0),
                        opacity: opacity.unwrap_or(1.0),
                        name: name.as_ref().map(|n| clean_name(n)).transpose()?,
                    }
                }
                Operation::AddShape { kind, bounds, radius, .. } => {
                    let kind = match kind {
                        ShapeKind::Rect => varos_core::model::ShapeKind::Rect,
                        ShapeKind::Ellipse => varos_core::model::ShapeKind::Ellipse,
                    };
                    if radius.is_some() && kind != varos_core::model::ShapeKind::Rect {
                        return Err(fail("radius requires rect"));
                    }
                    if let Some(r) = radius.filter(|r| *r != 0.0) {
                        EditCommand::AddPath {
                            anchors: rounded_rect(*bounds, r)?,
                            closed: true,
                            parent,
                            fill,
                            stroke,
                            stroke_width: stroke_width.unwrap_or(0.0),
                            opacity: opacity.unwrap_or(1.0),
                            name: name.as_ref().map(|n| clean_name(n)).transpose()?,
                        }
                    } else {
                        EditCommand::AddShape {
                            kind,
                            bounds: *bounds,
                            parent,
                            fill,
                            stroke,
                            stroke_width: stroke_width.unwrap_or(0.0),
                            opacity: opacity.unwrap_or(1.0),
                            name: name.as_ref().map(|n| clean_name(n)).transpose()?,
                        }
                    }
                }
                _ => unreachable!(),
            };
            let id = ed.try_execute_created(command).map_err(fail)?;
            created = Some(id);
            bind(locals, local, format!("path:{id}"))?;
        }
        Operation::Move { delta, .. } => {
            ed.apply_targeted_op(&TargetEdit::Move { paths, delta: *delta }, 0).map_err(super::service::target_error)?
        }
        Operation::OutlineStroke { .. }
        | Operation::OffsetPath { .. }
        | Operation::Expand { .. }
        | Operation::LiveCorners { .. }
        | Operation::ScaleStrokes { .. }
        | Operation::NewDocument { .. } => return Err(fail("Lane C dispatch error")),
        Operation::SetStrokeStyle { stroke_style, .. } => {
            ed.try_execute(EditCommand::SetStrokeStyle { ids: paths, style: stroke_style.clone() }).map_err(fail)?;
        }
        Operation::SetPaint { fill, stroke, stroke_width, opacity, stroke_style, .. } => {
            if fill != &crate::dto::Paint::Unchanged
                || stroke != &crate::dto::Paint::Unchanged
                || stroke_width.is_some()
                || opacity.is_some()
            {
                ed.apply_targeted_op(
                    &TargetEdit::Paint {
                        paths,
                        fill: paint(fill)?,
                        stroke: paint(stroke)?,
                        stroke_width: *stroke_width,
                        opacity: *opacity,
                    },
                    0,
                )
                .map_err(super::service::target_error)?;
            }
            if let Some(style) = stroke_style {
                ed.try_execute(EditCommand::SetStrokeStyle {
                    ids: ids.iter().map(|id| canonical(id).map(|(_, n)| n)).collect::<Result<Vec<_>, _>>()?,
                    style: style.clone(),
                })
                .map_err(fail)?;
            }
        }
        Operation::Rename { name, .. } => {
            let name = clean_name(name)?;
            for id in ids {
                let (kind, n) = canonical(&id)?;
                execute(
                    ed,
                    if kind == "path" {
                        EditCommand::RenamePath { path: n, name: name.clone() }
                    } else {
                        let node = ed.doc.node(n).expect("resolved");
                        if let NodeKind::Path(pid) = node.kind {
                            EditCommand::RenamePath { path: pid, name: name.clone() }
                        } else {
                            EditCommand::RenameNode { node: n, name: name.clone() }
                        }
                    },
                )?;
            }
        }
        _ => {
            for id in &ids {
                let (kind, n) = canonical(id)?;
                if kind == "node" && ed.doc.node(n).is_some_and(|n| n.kind == NodeKind::Layer) {
                    return Err(Error::new(
                        "unsupported",
                        "structural layer targets are deferred; use object/group ids",
                    ));
                }
            }
            if let Operation::Rotate { degrees, .. } = op {
                if !degrees.is_finite() {
                    return Err(fail("degrees must be finite"));
                }
                for unit in paths.iter().filter_map(|p| ed.doc.unit_of(*p)) {
                    if ed.doc.node_paths(unit).iter().any(|p| !paths.contains(p)) {
                        return Err(Error::new(
                            "unsupported",
                            format!("partial-group rotate has no independent absolute leaf angle in core; rotate the whole group node:{unit}"),
                        ));
                    }
                }
            }
            let units = if matches!(op, Operation::Resize { .. }) {
                let units: BTreeSet<_> = paths.iter().filter_map(|p| ed.doc.unit_of(*p)).collect();
                if units.iter().any(|u| {
                    !ed.doc.node_xform(*u).is_identity() && ed.doc.node_paths(*u).iter().any(|p| !paths.contains(p))
                }) {
                    return Err(Error::new("unsupported", "partial resize of a rotated group has no independent local size frame in core; use move for leaf placement or resize with the whole group node id"));
                }
                units.into_iter().collect()
            } else {
                if matches!(
                    op,
                    Operation::AnchorType { .. } | Operation::InsertAnchor { .. } | Operation::DeleteAnchor { .. }
                ) {
                    Vec::new()
                } else {
                    whole_units(ed, &paths)?
                }
            };
            ed.try_execute(EditCommand::SelectPaths(paths.clone())).map_err(fail)?;
            match op {
                Operation::Pathfinder { operation, .. } => {
                    let command = construction_command(operation)?;
                    execute(ed, command)?;
                }
                Operation::ShapeBuilder { points, delete, .. } => {
                    execute(ed, EditCommand::ShapeBuilder { points: points.clone(), delete: *delete })?
                }
                Operation::Scissors { segment, t, .. } => {
                    if paths.len() != 1 {
                        return Err(fail("scissors requires one path"));
                    }
                    execute(ed, EditCommand::Scissors { path: paths[0], segment: *segment, t: *t })?;
                }
                Operation::Knife { points, .. } => execute(ed, EditCommand::Knife { points: points.clone() })?,
                Operation::Eraser { points, radius, .. } => {
                    execute(ed, EditCommand::Eraser { points: points.clone(), radius: *radius })?
                }
                Operation::DivideObjectsBelow { .. } => execute(ed, EditCommand::DivideObjectsBelow)?,
                Operation::InsertAnchor { segment, t, .. } => {
                    if paths.len() != 1 {
                        return Err(fail("insert_anchor needs one path"));
                    }
                    execute(ed, EditCommand::InsertAnchor { path: paths[0], segment: *segment, t: *t })?;
                }
                Operation::AnchorType { anchor, smooth, .. } => {
                    if paths.len() != 1 || ed.doc.pid_of_anchor(*anchor) != Some(paths[0]) {
                        return Err(fail("anchor_type needs its one owning path"));
                    }
                    execute(ed, EditCommand::AnchorType { anchor: *anchor, smooth: *smooth })?;
                }
                Operation::DeleteAnchor { anchor, .. } => {
                    if paths.len() != 1 || ed.doc.pid_of_anchor(*anchor) != Some(paths[0]) {
                        return Err(fail("delete_anchor needs its one owning path"));
                    }
                    execute(ed, EditCommand::DeleteAnchor(*anchor))?;
                }
                Operation::DistributeMode { mode, .. } => execute(
                    ed,
                    EditCommand::DistributeMode(match mode {
                        Alignment::Left => AlignMode::Left,
                        Alignment::Center => AlignMode::CenterH,
                        Alignment::Right => AlignMode::Right,
                        Alignment::Top => AlignMode::Top,
                        Alignment::Middle => AlignMode::Middle,
                        Alignment::Bottom => AlignMode::Bottom,
                    }),
                )?,
                Operation::Object { action, anchors, .. } => {
                    if let Some(anchors) = anchors {
                        if *action != varos_core::editor::wave::ObjectAction::Average {
                            return Err(fail("anchors are only accepted by average"));
                        }
                        if anchors.len() > MAX_TARGETS
                            || anchors.iter().any(|id| ed.doc.pid_of_anchor(*id).is_none_or(|p| !paths.contains(&p)))
                        {
                            return Err(fail("average anchors must belong to explicit targets, at most 1000"));
                        }
                        execute(ed, EditCommand::SelectAnchors(anchors.clone()))?;
                    }
                    execute(ed, EditCommand::Object(*action))?;
                }
                Operation::DistributeSpacing { axis, gap, .. } => execute(
                    ed,
                    EditCommand::DistributeSpacing {
                        axis: match axis {
                            Axis::H => DistAxis::Horizontal,
                            Axis::V => DistAxis::Vertical,
                        },
                        gap: *gap,
                    },
                )?,
                Operation::Resize { bounds, .. } => {
                    let b = if units.len() == 1 && !ed.doc.node_xform(units[0]).is_identity() {
                        ed.obj_local_bbox()
                    } else {
                        ed.obj_bbox()
                    }
                    .ok_or_else(|| fail("cannot resize empty bounds"))?;
                    let (width, height) = (b.2 - b.0, b.3 - b.1);
                    if width <= 1e-3 || height <= 1e-3 || bounds[2] / width < 1e-3 || bounds[3] / height < 1e-3 {
                        return Err(fail("resize would hit the core minimum size/scale clamp"));
                    }
                    ed.set_constrain_wh(false);
                    let [x, y, w, h] = *bounds;
                    execute(
                        ed,
                        EditCommand::SetObjectBounds {
                            x: Some(x),
                            y: Some(y),
                            width: Some(w),
                            height: Some(h),
                            anchor_x: 0.0,
                            anchor_y: 0.0,
                        },
                    )?;
                }
                Operation::Rotate { degrees, .. } => {
                    for unit in &units {
                        execute(ed, EditCommand::SelectPaths(ed.doc.node_paths(*unit)))?;
                        execute(ed, EditCommand::SetObjectRotation(*degrees))?;
                    }
                }
                Operation::Delete { .. } => execute(ed, EditCommand::DeleteSelected)?,
                Operation::Align { mode, target, .. } => {
                    let mode = match mode {
                        Alignment::Left => AlignMode::Left,
                        Alignment::Center => AlignMode::CenterH,
                        Alignment::Right => AlignMode::Right,
                        Alignment::Top => AlignMode::Top,
                        Alignment::Middle => AlignMode::Middle,
                        Alignment::Bottom => AlignMode::Bottom,
                    };
                    let active = ed.doc.active;
                    let reference = if target == "selection" {
                        if units.len() < 2 {
                            return Err(fail("selection alignment requires at least two units"));
                        }
                        AlignTarget::Selection
                    } else if target == "key_object" {
                        AlignTarget::KeyObject
                    } else {
                        if target.eq_ignore_ascii_case("auto") {
                            return Err(Error::new("unsupported", "Auto alignment is not enabled"));
                        }
                        let index = if target.starts_with("artboard:") || target.starts_with('$') {
                            // slice 3: the stable id (or a request-local bound to one) — resolved in
                            // the staged document, so a page added earlier in this batch is a target
                            let id = artboard_ref(target, locals)?;
                            ed.doc
                                .artboard_index(id)
                                .ok_or_else(|| Error::new("not_found", format!("unknown artboard:{id}")))?
                        } else {
                            legacy_artboard_ref(target, original_rev)?
                        };
                        let a =
                            ed.doc.artboards.get(index).ok_or_else(|| Error::new("not_found", "unknown artboard"))?;
                        if a.hidden || a.locked {
                            return Err(Error::new(
                                if a.locked { "locked_target" } else { "hidden_target" },
                                "artboard is inert",
                            ));
                        }
                        ed.doc.active = index;
                        AlignTarget::Artboard
                    };
                    let result = ed.align_explicit_units(mode, reference).map_err(fail);
                    ed.doc.active = active;
                    result?;
                }
                Operation::Distribute { axis, .. } => {
                    if paths.len() < 3 {
                        return Err(fail("distribute requires at least three paths"));
                    }
                    if paths.iter().any(|p| ed.doc.top_group_of_path(*p).is_some()) {
                        return Err(Error::new("unsupported", "group distribution is deferred"));
                    }
                    execute(
                        ed,
                        EditCommand::Distribute(match axis {
                            Axis::H => DistAxis::Horizontal,
                            Axis::V => DistAxis::Vertical,
                        }),
                    )?;
                }
                Operation::Group { local, .. } => {
                    if let Some(local) = local {
                        local_name(local)?;
                        if locals.contains_key(local) {
                            return Err(fail("duplicate request-local name"));
                        }
                    }
                    if units.len() < 2 {
                        return Err(fail("group requires at least two complete units"));
                    }
                    let parents: BTreeSet<_> = units.iter().map(|u| ed.doc.node(*u).expect("unit").parent).collect();
                    if parents.len() != 1 {
                        return Err(Error::new("unsupported", "group targets must share a parent"));
                    }
                    let before: BTreeSet<_> = ed.doc.nodes.iter().map(|n| n.id).collect();
                    execute(ed, EditCommand::GroupSelection)?;
                    let group = ed
                        .doc
                        .nodes
                        .iter()
                        .find(|n| n.kind == NodeKind::Group && !before.contains(&n.id))
                        .ok_or_else(|| fail("group did not create a unit"))?;
                    bind(locals, local, format!("node:{}", group.id))?;
                }
                Operation::Clip { .. } => {
                    execute(ed, EditCommand::ClipMake)?;
                }
                Operation::ReleaseClip { .. } => {
                    ed.layer_select_set(&units);
                    execute(ed, EditCommand::ClipRelease)?;
                }
                Operation::Ungroup { .. } => {
                    for id in &ids {
                        let (kind, n) = canonical(id)?;
                        if kind != "node"
                            || ed.doc.node(n).is_none_or(|n| n.kind != NodeKind::Group)
                            || !units.contains(&n)
                        {
                            return Err(Error::new("unsupported", "ungroup requires explicit top-level group ids"));
                        }
                    }
                    execute(ed, EditCommand::UngroupSelection)?;
                }
                Operation::Order { order, .. } => execute(
                    ed,
                    EditCommand::Arrange(match order {
                        Order::Front => ZOrder::Front,
                        Order::Forward => ZOrder::Forward,
                        Order::Backward => ZOrder::Backward,
                        Order::Back => ZOrder::Back,
                    }),
                )?,
                _ => unreachable!(),
            }
        }
    }
    Ok(created)
}

/// Eight tangent anchors, quarter-circle cubics. This is editable path geometry, not a primitive.
fn rounded_rect([x, y, w, h]: [f32; 4], r: f32) -> Result<Vec<varos_core::model::Anchor>, Error> {
    if ![x, y, w, h, r, x + w, y + h].iter().all(|v| v.is_finite())
        || w <= 0.0
        || h <= 0.0
        || r < 0.0
        || x + w <= x
        || y + h <= y
    {
        return Err(fail("radius must be finite and between 0 and half the shorter positive dimension"));
    }
    let r = r.min(w.min(h) / 2.0);
    let k = varos_core::model::K * r;
    let points = [
        [x + r, y],
        [x + w - r, y],
        [x + w, y + r],
        [x + w, y + h - r],
        [x + w - r, y + h],
        [x + r, y + h],
        [x, y + h - r],
        [x, y + r],
    ];
    let incoming = [
        [x + r - k, y],
        [x + w - r, y],
        [x + w, y + r - k],
        [x + w, y + h - r],
        [x + w - r + k, y + h],
        [x + r, y + h],
        [x, y + h - r + k],
        [x, y + r],
    ];
    let outgoing = [
        [x + r, y],
        [x + w - r + k, y],
        [x + w, y + r],
        [x + w, y + h - r + k],
        [x + w - r, y + h],
        [x + r - k, y + h],
        [x, y + h - r],
        [x, y + r - k],
    ];
    let mut anchors: Vec<varos_core::model::Anchor> = (0..8)
        .map(|i| varos_core::model::Anchor {
            id: 0,
            p: points[i],
            hin: (incoming[i] != points[i]).then_some(incoming[i]),
            hout: (outgoing[i] != points[i]).then_some(outgoing[i]),
            smooth: true,
        })
        .collect();
    for i in (1..anchors.len()).rev() {
        if anchors[i].p == anchors[i - 1].p {
            anchors[i - 1].hout = anchors[i].hout;
            anchors.remove(i);
        }
    }
    Ok(anchors)
}

fn construction_command(operation: &str) -> Result<EditCommand, Error> {
    use varos_core::{boolean::BoolOp, planar::PathfinderOp};
    Ok(match operation {
        "unite" => EditCommand::Boolean(BoolOp::Unite),
        "minus_front" => EditCommand::Boolean(BoolOp::MinusFront),
        "intersect" => EditCommand::Boolean(BoolOp::Intersect),
        "exclude" => EditCommand::Boolean(BoolOp::Exclude),
        "divide" => EditCommand::Pathfinder(PathfinderOp::Divide),
        "trim" => EditCommand::Pathfinder(PathfinderOp::Trim),
        "merge" => EditCommand::Pathfinder(PathfinderOp::Merge),
        "crop" => EditCommand::Pathfinder(PathfinderOp::Crop),
        "outline" => EditCommand::Pathfinder(PathfinderOp::Outline),
        "minus_back" => EditCommand::Pathfinder(PathfinderOp::MinusBack),
        _ => return Err(Error::new("invalid_argument", "unknown pathfinder operation")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_rect_half_and_clamp_drop_straights() {
        let half = rounded_rect([0.0, 0.0, 20.0, 20.0], 10.0).unwrap();
        assert_eq!(half.len(), 4);
        assert_eq!(rounded_rect([0.0, 0.0, 20.0, 20.0], 100.0).unwrap(), half);
        let small = rounded_rect([0.0, 0.0, 20.0, 30.0], 5.0).unwrap();
        assert_eq!(small.len(), 8);
        assert!(small[0].hout.is_none());
        assert!(small[1].hin.is_none());
        for a in half {
            assert!(a.hin.is_some() && a.hout.is_some());
        }
    }
    #[test]
    fn rotate_rejects_nonfinite_degrees_before_mutation() {
        let mut ed = Editor::new();
        let id = ed
            .try_execute_created(EditCommand::AddShape {
                kind: varos_core::model::ShapeKind::Rect,
                bounds: [0.0, 0.0, 10.0, 20.0],
                parent: None,
                fill: Some([1.0; 4]),
                stroke: None,
                stroke_width: 0.0,
                opacity: 1.0,
                name: None,
            })
            .unwrap();
        let before = ed.doc.clone();
        let selection = ed.objsel.clone();
        let rev = ed.rev;
        for degrees in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let error = apply_design_op(
                &mut ed,
                &Operation::Rotate { ids: vec![format!("path:{id}")], degrees },
                rev,
                &mut BTreeMap::new(),
                &mut 0,
                &mut BTreeSet::new(),
                &|| false,
            )
            .unwrap_err();
            assert_eq!(error.code, "invalid_argument");
            assert_eq!(error.reason, "degrees must be finite");
            assert_eq!(ed.doc, before);
            assert_eq!(ed.objsel, selection);
            assert_eq!(ed.rev, rev);
        }
    }
}
