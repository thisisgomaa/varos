//! Provisional headless contracts. No file I/O, UI or renderer dependencies.
//! Bridge API 0.x spellings are pinned in `EditCommand`'s serde table.
use crate::{board, command::EditCommand, editor::Editor, format, model::NodeKind};
pub use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq)]
pub struct BatchError {
    pub index: usize,
    pub reason: String,
}

/// Parse each command separately so shape errors carry the same zero-based index as edit errors.
pub fn parse_batch(bytes: &[u8]) -> Result<Vec<EditCommand>, BatchError> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Envelope {
        api: String,
        commands: Vec<Value>,
    }
    let envelope: Envelope = serde_json::from_slice(bytes)
        .map_err(|e| BatchError { index: 0, reason: format!("expected Bridge batch envelope: {e}") })?;
    let version = envelope.api.split('.').collect::<Vec<_>>();
    if version.len() != 2 || version[0] != "0" || version[1].parse::<u32>().is_err() {
        return Err(BatchError {
            index: 0,
            reason: format!("unsupported Bridge API {}; expected major 0 (0.1)", envelope.api),
        });
    }
    let values = envelope.commands;
    values
        .into_iter()
        .enumerate()
        .map(|(index, v)| serde_json::from_value(v).map_err(|e| BatchError { index, reason: e.to_string() }))
        .collect()
}

/// Preconditions for the headless command path. Interactive callers retain `execute` unchanged.
pub(crate) fn check(command: &EditCommand, ed: &Editor) -> Result<(), String> {
    use EditCommand::*;
    if matches!(command, GroupSelection | Boolean(_) | Paste { .. } | DuplicateMoveLayer { .. } | DuplicateArtboard(_))
    {
        // Reserve an entire format-sized arena before an allocating edit. Existing allocators use
        // u32 ids; refusing near exhaustion is safer than overflowing before post-validation.
        let limits = format::Limits::DEFAULT;
        let reserve = (limits.max_nodes + limits.max_paths + limits.max_anchors) as u64;
        if u64::from(ed.doc.ids) + reserve >= u64::from(u32::MAX) {
            return Err("not enough stable ids remain for an allocating command".into());
        }
    }
    let finite = |v: f32| if v.is_finite() { Ok(()) } else { Err("number must be finite".to_owned()) };
    let dimension = |v: f32| {
        finite(v)?;
        if v > 0.0 {
            Ok(())
        } else {
            Err("dimension must be positive".to_owned())
        }
    };
    let color = |c: &[f32; 4]| {
        if c.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)) {
            Ok(())
        } else {
            Err("RGBA channels must be between 0 and 1".to_owned())
        }
    };
    let path = |id: u32| {
        if ed.doc.pidx(id).is_none() {
            Err(format!("unknown path id {id}"))
        } else if ed.doc.eff_hidden(id) || ed.doc.eff_locked(id) {
            Err(format!("path {id} is hidden or locked"))
        } else {
            Ok(())
        }
    };
    let node = |id: u32| ed.doc.node(id).map(|_| ()).ok_or_else(|| format!("unknown node id {id}"));
    let artboard = |i: usize| ed.doc.artboards.get(i).map(|_| ()).ok_or_else(|| format!("unknown artboard index {i}"));
    let selection = || {
        for id in &ed.objsel {
            path(*id)?;
        }
        for id in &ed.selected {
            let pid = ed.doc.pid_of_anchor(*id).ok_or_else(|| format!("unknown anchor id {id}"))?;
            path(pid)?;
        }
        if let Some(id) = ed.dsel_path {
            path(id)?;
        }
        if ed.selected_pids().is_empty() {
            Err("command requires a selection; use SelectPaths first".to_owned())
        } else {
            Ok(())
        }
    };
    match command {
        SelectPaths(ids) => {
            for id in ids {
                path(*id)?;
            }
            Ok(())
        }
        SelectAnchors(ids) => {
            for id in ids {
                let pid = ed.doc.pid_of_anchor(*id).ok_or_else(|| format!("unknown anchor id {id}"))?;
                path(pid)?;
            }
            Ok(())
        }
        Nudge { x, y } => {
            if ed.selected.is_empty() {
                return Err("Nudge requires SelectAnchors (use SetObjectBounds for objects)".into());
            }
            finite(*x)?;
            finite(*y)
        }
        Copy | Cut | DeleteLayerSelection => {
            selection()?;
            if ed.objsel.is_empty() && ed.dsel_path.is_none() {
                Err("command requires whole paths, not a bare anchor selection".into())
            } else {
                Ok(())
            }
        }
        Paste { offset } => {
            if ed.clipboard().is_empty() {
                return Err("clipboard is empty; use Copy or Cut in the batch first".into());
            }
            if let Some(p) = offset {
                finite(p[0])?;
                finite(p[1])?;
            }
            Ok(())
        }
        SetBoardName(s) => board::check_name(&board::clean_text(s)).map_err(|e| e.to_string()),
        SetBoardDescription(s) => board::check_description(&board::clean_text(s)).map_err(|e| e.to_string()),
        SetBoardTags(t) => board::check_tags(&board::normalize_tags(t.clone())).map_err(|e| e.to_string()),
        RenamePath { path: id, name } => {
            path(*id)?;
            nonempty(name)
        }
        RenameNode { node: id, name } => {
            node(*id)?;
            nonempty(name)
        }
        ToggleNodeHidden(id) | ToggleNodeLocked(id) => node(*id),
        SetOpacity(v) => {
            selection()?;
            if v.is_finite() && (0.0..=1.0).contains(v) {
                Ok(())
            } else {
                Err("opacity must be between 0 and 1".into())
            }
        }
        SetStrokeWidth(v) => {
            selection()?;
            finite(*v)?;
            if *v < 0.0 {
                Err("stroke width must be non-negative".into())
            } else {
                Ok(())
            }
        }
        SetObjectRotation(v) => {
            selection()?;
            finite(*v)
        }
        SetObjectBounds { x, y, width, height, anchor_x, anchor_y } => {
            selection()?;
            for v in [x, y].into_iter().flatten() {
                finite(*v)?;
            }
            for v in [width, height].into_iter().flatten() {
                dimension(*v)?;
            }
            for v in [anchor_x, anchor_y] {
                if !v.is_finite() || !(0.0..=1.0).contains(v) {
                    return Err("bounds anchor must be between 0 and 1".into());
                }
            }
            Ok(())
        }
        ApplyPaint { color: c, .. } => {
            selection()?;
            if let Some(c) = c {
                color(c)?;
            }
            Ok(())
        }
        Boolean(_) => ed.pathfinder_enabled().map_err(str::to_owned),
        GroupSelection => {
            selection()?;
            if ed.objsel.len() < 2 {
                Err("group requires at least two selected paths".into())
            } else {
                Ok(())
            }
        }
        UngroupSelection => {
            selection()?;
            if ed.objsel.iter().all(|id| ed.doc.top_group_of_path(*id).is_none()) {
                Err("selection contains no group".into())
            } else {
                Ok(())
            }
        }
        Distribute(_) => {
            selection()?;
            if (if ed.tool == crate::editor::ToolKind::Direct { ed.selected.len() } else { ed.objsel.len() }) < 3 {
                Err("distribute requires at least three selected paths".into())
            } else {
                Ok(())
            }
        }
        Align { .. } | Arrange(_) | Flip(_) | SetClipExempt(_) | DeleteSelected | SwapColors | DefaultPaint => {
            selection()
        }
        SetActiveArtboard(i)
        | ToggleArtboardClip(i)
        | ToggleArtboardHidden(i)
        | ToggleArtboardLocked(i)
        | OrientArtboard(i)
        | DuplicateArtboard(i)
        | DeleteArtboard(i) => artboard(*i),
        RenameArtboard { index, name } => {
            artboard(*index)?;
            nonempty(name)
        }
        SetArtboardColor { index, color: c } => {
            artboard(*index)?;
            if let Some(c) = c {
                color(c)?;
            }
            Ok(())
        }
        SetArtboardRect { index, x, y, width, height } => {
            artboard(*index)?;
            for v in [x, y].into_iter().flatten() {
                finite(*v)?;
            }
            for v in [width, height].into_iter().flatten() {
                dimension(*v)?;
            }
            Ok(())
        }
        SetArtboardCount(count) => {
            if *count == 0 || *count > format::Limits::DEFAULT.max_artboards {
                Err("artboard count must be between 1 and 1000".into())
            } else {
                Ok(())
            }
        }
        AddArtboard => {
            if ed.doc.artboards.len() >= format::Limits::DEFAULT.max_artboards {
                Err("artboard limit reached".into())
            } else {
                Ok(())
            }
        }
        SetMoveArtWithArtboard(_) | CycleUnits => Ok(()),
        DuplicateMoveLayer { sources, target, position } => {
            node(*target)?;
            if sources.is_empty() {
                return Err("sources must not be empty".into());
            }
            if *position == crate::model::DropPos::Into
                && matches!(ed.doc.node(*target).unwrap().kind, NodeKind::Path(_))
            {
                return Err("cannot duplicate into a path".into());
            }
            for id in sources {
                node(*id)?;
                let paths = ed.doc.node_paths(*id);
                if paths.is_empty() {
                    return Err(format!("node {id} contains no paths"));
                }
                for id in paths {
                    path(id)?;
                }
            }
            Ok(())
        }
        MoveLayerToBoard { sources, source_board, target_board } => {
            artboard(*target_board)?;
            if let Some(i) = source_board {
                artboard(*i)?;
            }
            if sources.is_empty() {
                return Err("sources must not be empty".into());
            }
            for id in sources {
                node(*id)?;
                let paths = ed.doc.node_paths(*id);
                if paths.is_empty() {
                    return Err(format!("node {id} contains no paths"));
                }
                for id in paths {
                    path(id)?;
                }
            }
            Ok(())
        }
        MoveLayer { sources, target, position } => {
            node(*target)?;
            if sources.is_empty() {
                return Err("sources must not be empty".into());
            }
            for id in sources {
                node(*id)?;
                if !ed.doc.move_is_legal(*id, *target, *position) {
                    return Err(format!("illegal layer move from {id} to {target}"));
                }
            }
            Ok(())
        }
        PickerBegin
        | PickerLivePaint { .. }
        | PickerLiveArtboard { .. }
        | PickerCommit { .. }
        | PickerCancel
        | CommitGuide
        | TransformAgain => Err("command requires an interactive gesture session".into()),
        Undo | Redo => Err("history commands cannot be nested inside a document batch".into()),
        // Preferences survive normal undo; pretending they are undoable would break the law.
        SetRulerOrigin(_) | SetSnapConfig(_) | ToggleSnapping | ToggleGuidesLocked | ToggleSmartGuides => {
            Err("preference commands are not undoable document edits".into())
        }
    }
}
fn nonempty(s: &str) -> Result<(), String> {
    if crate::command::clean_name(s).is_empty() {
        Err("name must not be empty".into())
    } else {
        Ok(())
    }
}

fn bounds(doc: &crate::model::Document, ids: &[u32]) -> Value {
    let mut b: Option<[f32; 4]> = None;
    for id in ids {
        if let Some(i) = doc.pidx(*id) {
            if doc.paths[i].anchors.is_empty() {
                continue;
            }
            let (x0, y0, x1, y1) = doc.outline_bbox(i);
            b = Some(match b {
                None => [x0, y0, x1, y1],
                Some(a) => [a[0].min(x0), a[1].min(y0), a[2].max(x1), a[3].max(y1)],
            });
        }
    }
    json!(b)
}

/// All tree elements; path ids and node ids occupy separate namespaces in the existing format.
/// Prefixes make the public ids unambiguous (`path:10`, `node:15`). Artboards use index, not fake ids.
pub fn elements(doc: &crate::model::Document, detail: bool) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for p in &doc.paths {
        let id = format!("path:{}", p.id);
        let n = doc.node_of_path(p.id).and_then(|id| doc.node(id));
        let mut v = json!({"id":id,"kind":"path","name":p.name,"bounds":bounds(doc,&[p.id]),
            "fill":p.fill,"stroke":{"paint":p.stroke,"width":p.stroke_width},"opacity":p.opacity,
            "hidden":doc.eff_hidden(p.id),"locked":doc.eff_locked(p.id),
            "parent":n.and_then(|n| n.parent).map(|id|format!("node:{id}"))});
        if detail {
            v["geometry"] = json!(p);
            v["node"] = json!(n);
            v["world_transform"] = json!(doc.unit_xform(p.id));
        }
        out.insert(id, v);
    }
    for n in &doc.nodes {
        if matches!(n.kind, NodeKind::Path(_)) {
            continue;
        }
        let id = format!("node:{}", n.id);
        let mut v = json!({"id":id,"kind":if n.kind==NodeKind::Layer {"layer"} else {"group"},
            "name":n.name,"bounds":bounds(doc,&doc.node_paths(n.id)),"fill":null,"stroke":null,
            "hidden":n.hidden,"locked":n.locked,"parent":n.parent.map(|id|format!("node:{id}"))});
        if detail {
            v["geometry"] = json!(n);
        }
        out.insert(id, v);
    }
    out
}

// Tree siblings are front-first; queries expose the corresponding back-to-front paint order.
fn ordered_ids(doc: &crate::model::Document) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack: Vec<_> = doc.roots.clone();
    while let Some(id) = stack.pop() {
        let Some(node) = doc.node(id) else { continue };
        out.push(match node.kind {
            NodeKind::Path(pid) => format!("path:{pid}"),
            _ => format!("node:{id}"),
        });
        stack.extend(&node.children);
    }
    out
}

/// Match the SVG exporter's three-decimal precision rather than widening f32 noise in JSON.
fn round_numbers(value: &mut Value) {
    match value {
        Value::Number(n) if n.is_f64() => {
            if let Some(n) = n.as_f64() {
                *value = json!((n * 1000.0).round() / 1000.0);
            }
        }
        Value::Array(values) => values.iter_mut().for_each(round_numbers),
        Value::Object(fields) => fields.values_mut().for_each(round_numbers),
        _ => {}
    }
}

pub fn describe(doc: &crate::model::Document, detail: Option<&str>) -> Result<Value, String> {
    if let Some(id) = detail {
        let mut value = elements(doc, true).remove(id).ok_or_else(|| format!("unknown element id {id}"))?;
        round_numbers(&mut value);
        return Ok(value);
    }
    let mut by_id = elements(doc, false);
    let elements = ordered_ids(doc).iter().filter_map(|id| by_id.remove(id)).collect::<Vec<_>>();
    let mut value = json!({"name":doc.name,"description":doc.description,"tags":doc.tags,
        "artboards":doc.artboards,"element_count":elements.len(),"elements":elements});
    round_numbers(&mut value);
    Ok(value)
}

pub fn diff(a: &crate::model::Document, b: &crate::model::Document) -> Value {
    let (aa, bb) = (elements(a, true), elements(b, true));
    let added: Vec<_> = ordered_ids(b).into_iter().filter(|id| !aa.contains_key(id)).collect();
    let removed: Vec<_> = ordered_ids(a).into_iter().filter(|id| !bb.contains_key(id)).collect();
    let changed: Vec<_> = ordered_ids(b)
        .into_iter()
        .filter_map(|id| {
            let v = bb.get(&id)?;
            let old = aa.get(&id)?;
            if old == v {
                return None;
            }
            let fields: Vec<_> =
                v.as_object().unwrap().keys().filter(|key| old[*key] != v[*key]).map(String::as_str).collect();
            Some(json!({"id":id,"summary":format!("changed {}",fields.join(", "))}))
        })
        .collect();
    let mut document_changes = Vec::new();
    // Compare ALL document fields outside the element arenas, including artboards and z order.
    let (av, bv) = (json!(a), json!(b));
    for (key, v) in bv.as_object().unwrap() {
        if !["paths", "nodes", "groups", "group_of", "ids"].contains(&key.as_str()) && av[key] != *v {
            document_changes.push(key);
        }
    }
    json!({"added":added,"removed":removed,"changed":changed,"document_changes":document_changes})
}

pub(crate) fn check_document(ed: &Editor) -> Result<(), String> {
    format::check_structure(&ed.doc, &format::Limits::DEFAULT).map_err(|e| e.to_string())?;
    format::validate(&ed.doc, &format::Limits::DEFAULT).map_err(|e| e.to_string())
}
