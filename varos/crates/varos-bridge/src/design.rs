//! Validated design operations on isolated core state.
use crate::{
    dto::*,
    service::{paint, resolve},
    MAX_TARGETS,
};
use std::collections::{BTreeMap, BTreeSet};
use varos_core::{
    bridge::TargetEdit,
    editor::{AlignMode, AlignTarget, DistAxis, Editor, ZOrder},
    model::NodeKind,
    EditCommand,
};
fn fail(reason: impl Into<String>) -> Error {
    Error::new("invalid_argument", reason)
}
fn canonical(id: &str) -> Result<(&str, u32), Error> {
    let (kind, value) = id.split_once(':').ok_or_else(|| fail("use path:N or node:N"))?;
    let n = value.parse::<u32>().map_err(|_| fail("id suffix must be u32"))?;
    if n == 0 || format!("{kind}:{n}") != id || !["path", "node"].contains(&kind) {
        return Err(fail("object id must be canonical path:N or node:N"));
    }
    Ok((kind, n))
}
fn local_name(name: &str) -> Result<(), Error> {
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
) -> Result<(), Error> {
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
    let paths = if matches!(op, Operation::AddShape { .. }) {
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
    match op {
        Operation::AddShape { kind, bounds, parent, local, name, fill, stroke, stroke_width, opacity, .. } => {
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
            let id = ed
                .try_execute_created(EditCommand::AddShape {
                    kind: match kind {
                        ShapeKind::Rect => varos_core::model::ShapeKind::Rect,
                        ShapeKind::Ellipse => varos_core::model::ShapeKind::Ellipse,
                    },
                    bounds: *bounds,
                    parent,
                    fill: paint(fill)?.unwrap_or(None),
                    stroke: paint(stroke)?.unwrap_or(None),
                    stroke_width: stroke_width.unwrap_or(0.0),
                    opacity: opacity.unwrap_or(1.0),
                    name: name.as_ref().map(|n| clean_name(n)).transpose()?,
                })
                .map_err(fail)?;
            bind(locals, local, format!("path:{id}"))?;
        }
        Operation::Move { delta, .. } => {
            ed.apply_targeted_op(&TargetEdit::Move { paths, delta: *delta }, 0).map_err(super::service::target_error)?
        }
        Operation::SetPaint { fill, stroke, stroke_width, opacity, .. } => ed
            .apply_targeted_op(
                &TargetEdit::Paint {
                    paths,
                    fill: paint(fill)?,
                    stroke: paint(stroke)?,
                    stroke_width: *stroke_width,
                    opacity: *opacity,
                },
                0,
            )
            .map_err(super::service::target_error)?,
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
            let units = whole_units(ed, &paths)?;
            ed.try_execute(EditCommand::SelectPaths(paths.clone())).map_err(fail)?;
            match op {
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
                    } else {
                        if target.eq_ignore_ascii_case("auto") {
                            return Err(Error::new("unsupported", "Auto alignment is not enabled"));
                        }
                        let (index, rev) =
                            target.strip_prefix('a').and_then(|s| s.split_once('@')).ok_or_else(|| {
                                fail("align target must be selection or aN@revision; Auto is unsupported")
                            })?;
                        let index = index.parse::<usize>().map_err(|_| fail("invalid artboard reference"))?;
                        let rev = rev.parse::<u64>().map_err(|_| fail("invalid artboard revision"))?;
                        if format!("a{index}@{rev}") != *target {
                            return Err(fail("artboard reference must be canonical"));
                        }
                        if rev != original_rev {
                            return Err(Error::new("revision_conflict", "artboard reference is revision-bound"));
                        }
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
    Ok(())
}
