//! Provisional headless contracts. No file I/O, UI or renderer dependencies.
//! Bridge API 0.x spellings are pinned in `EditCommand`'s serde table.
use crate::{
    board,
    command::EditCommand,
    editor::Editor,
    format,
    model::{Artboard, NodeKind},
};
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
    if matches!(
        command,
        AddPath { .. }
            | AddShape { .. }
            | GroupSelection
            | Boolean(_)
            | Pathfinder(_)
            | ShapeBuilder { .. }
            | Scissors { .. }
            | Knife { .. }
            | Eraser { .. }
            | DivideObjectsBelow
            | Paste { .. }
            | DuplicateMoveLayer { .. }
            | DuplicateArtboard(_)
    ) {
        // Reserve an entire format-sized arena before an allocating edit. Existing allocators use
        // u32 ids; refusing near exhaustion is safer than overflowing before post-validation.
        let limits = format::Limits::DEFAULT;
        let reserve = (limits.max_nodes + limits.max_paths + limits.max_anchors) as u64;
        if u64::from(ed.allocation_floor()) + reserve >= u64::from(u32::MAX) {
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
        Pathfinder(_) => {
            selection()?;
            ed.pathfinder_enabled().map_err(str::to_owned)
        }
        ShapeBuilder { points, .. } | Knife { points } | Eraser { points, .. } => {
            selection()?;
            if points.is_empty() || points.len() > 1000 {
                return Err("gesture needs 1..1000 points".into());
            }
            for p in points {
                for v in p {
                    finite(*v)?;
                    finite(*v + *v)?;
                }
            }
            if let Eraser { radius, .. } = command {
                dimension(*radius)?;
            }
            Ok(())
        }
        Scissors { path: pid, segment, t } => {
            path(*pid)?;
            finite(*t)?;
            let p = ed.doc.pidx(*pid).map(|i| &ed.doc.paths[i]).ok_or("unknown path")?;
            if !p.holes.is_empty() || ed.doc.is_mask_source(*pid) {
                return Err("scissors does not support compound contours or mask sources".into());
            }
            let count = if p.closed { p.anchors.len() } else { p.anchors.len().saturating_sub(1) };
            if *segment >= count || !(0.0..=1.0).contains(t) {
                Err("invalid scissors segment or parameter".into())
            } else {
                Ok(())
            }
        }
        DivideObjectsBelow => selection(),
        AddPath { anchors, parent, fill, stroke, stroke_width, opacity, name, .. } => {
            if !(2..=1000).contains(&anchors.len()) {
                return Err("path needs 2..1000 anchors".into());
            }
            for a in anchors {
                for p in std::iter::once(&a.p).chain(a.hin.iter()).chain(a.hout.iter()) {
                    for v in p {
                        finite(*v)?;
                        finite(*v + *v)?;
                    }
                }
            }
            check(
                &AddShape {
                    kind: crate::model::ShapeKind::Rect,
                    bounds: [0.0, 0.0, 1.0, 1.0],
                    parent: *parent,
                    fill: *fill,
                    stroke: *stroke,
                    stroke_width: *stroke_width,
                    opacity: *opacity,
                    name: name.clone(),
                },
                ed,
            )?;
            let count: usize =
                ed.doc.paths.iter().map(|p| p.anchors.len() + p.holes.iter().map(Vec::len).sum::<usize>()).sum();
            if count + anchors.len() > format::Limits::DEFAULT.max_anchors {
                return Err("path would exceed anchor limit".into());
            }
            Ok(())
        }
        AddShape { kind, bounds, parent, fill, stroke, stroke_width, opacity, name } => {
            if !matches!(kind, crate::model::ShapeKind::Rect | crate::model::ShapeKind::Ellipse) {
                return Err("only rect and ellipse are supported".into());
            }
            let [x, y, w, h] = *bounds;
            finite(x)?;
            finite(y)?;
            dimension(w)?;
            dimension(h)?;
            finite(x + w)?;
            finite(y + h)?;
            if x + w <= x || y + h <= y {
                return Err("bounds collapse at f32 precision".into());
            }
            finite(x + (x + w))?;
            finite(y + (y + h))?;
            let limits = format::Limits::DEFAULT;
            if ed.doc.paths.len() >= limits.max_paths
                || ed.doc.nodes.len() >= limits.max_nodes
                || ed
                    .doc
                    .paths
                    .iter()
                    .map(|p| p.anchors.len() + p.holes.iter().map(Vec::len).sum::<usize>())
                    .sum::<usize>()
                    > limits.max_anchors - 4
            {
                return Err("shape would exceed document limits".into());
            }
            if let Some(c) = fill {
                color(c)?;
            }
            if let Some(c) = stroke {
                color(c)?;
            }
            finite(*stroke_width)?;
            if *stroke_width < 0.0 || !opacity.is_finite() || !(0.0..=1.0).contains(opacity) {
                return Err("invalid stroke width or opacity".into());
            }
            if let Some(n) = name {
                nonempty(n)?;
            }
            let n = ed.doc.node(parent.unwrap_or(ed.doc.active_layer)).ok_or("unknown parent layer")?;
            if n.kind != NodeKind::Layer {
                return Err("parent must be a layer".into());
            }
            if n.hidden || n.locked {
                return Err("parent is hidden or locked".into());
            }
            Ok(())
        }
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
        SetMoveArtWithArtboard(_) | CycleUnits | SetUnits(_) => Ok(()),
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

/// Deliberately separate from wire DTOs and the interactive command enum.
#[derive(Clone, Debug)]
pub enum TargetEdit {
    Move {
        paths: Vec<u32>,
        delta: [f32; 2],
    },
    Paint {
        paths: Vec<u32>,
        fill: Option<Option<[f32; 4]>>,
        stroke: Option<Option<[f32; 4]>>,
        stroke_width: Option<f32>,
        opacity: Option<f32>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TargetErrorCode {
    NotFound,
    LockedTarget,
    HiddenTarget,
    InvalidArgument,
    Busy,
    Cancelled,
}
#[derive(Debug)]
pub struct TargetError {
    pub code: TargetErrorCode,
    pub index: usize,
    pub ids: Vec<u32>,
    pub reason: String,
}
impl Editor {
    /// Replace Bridge selection without escaping or switching the human's tool.
    pub fn bridge_select(&mut self, paths: Vec<u32>) -> Result<(), String> {
        check(&EditCommand::SelectPaths(paths.clone()), self)?;
        self.objsel = paths.into_iter().collect();
        self.selected.clear();
        self.group_sel.clear();
        self.dsel_path = None;
        self.absel.clear();
        self.refresh_obj_angle();
        Ok(())
    }
    /// Explicit targets, checked staging, one publication and no implicit human selection change.
    pub fn execute_targeted_batch(&mut self, ops: Vec<TargetEdit>) -> Result<(), TargetError> {
        self.execute_targeted_batch_cancellable(ops, || false)
    }
    pub fn execute_targeted_batch_cancellable(
        &mut self,
        ops: Vec<TargetEdit>,
        cancelled: impl Fn() -> bool,
    ) -> Result<(), TargetError> {
        if self.transaction_open() {
            return Err(TargetError {
                code: TargetErrorCode::Busy,
                index: 0,
                ids: vec![],
                reason: "active gesture".into(),
            });
        }
        let mut staged = self.batch_stage();
        for (index, op) in ops.iter().enumerate() {
            if cancelled() {
                return Err(TargetError {
                    code: TargetErrorCode::Cancelled,
                    index,
                    ids: vec![],
                    reason: "cancelled before commit".into(),
                });
            }
            staged.apply_targeted_op(op, index)?;
            staged.clear_batch_history();
        }
        if cancelled() {
            return Err(TargetError {
                code: TargetErrorCode::Cancelled,
                index: 0,
                ids: vec![],
                reason: "cancelled before commit".into(),
            });
        }
        if validate_targeted_stage(&staged).is_err() {
            // Full validation/encoding runs once on success; replay only failures for attribution.
            let mut replay = self.batch_stage();
            for (index, op) in ops.iter().enumerate() {
                replay.apply_targeted_op(op, index)?;
                if let Err(reason) = validate_targeted_stage(&replay) {
                    let ids = match op {
                        TargetEdit::Move { paths, .. } | TargetEdit::Paint { paths, .. } => paths.clone(),
                    };
                    return Err(TargetError { code: TargetErrorCode::InvalidArgument, index, ids, reason });
                }
                replay.clear_batch_history();
            }
        }
        if cancelled() {
            return Err(TargetError {
                code: TargetErrorCode::Cancelled,
                index: 0,
                ids: vec![],
                reason: "cancelled before commit".into(),
            });
        }
        staged.doc.active_layer = self.doc.active_layer;
        self.publish_batch(staged, true);
        Ok(())
    }
    pub fn apply_targeted_op(&mut self, op: &TargetEdit, index: usize) -> Result<(), TargetError> {
        let paths = match op {
            TargetEdit::Move { paths, .. } | TargetEdit::Paint { paths, .. } => paths,
        };
        let fail = |code, reason: String| TargetError { code, index, ids: paths.clone(), reason };
        if paths.is_empty() {
            return Err(fail(TargetErrorCode::InvalidArgument, "targets must not be empty".into()));
        }
        for id in paths {
            if self.doc.pidx(*id).is_none() {
                return Err(fail(TargetErrorCode::NotFound, format!("unknown path:{id}")));
            }
            if self.doc.eff_locked(*id) {
                return Err(fail(TargetErrorCode::LockedTarget, format!("path:{id} is locked")));
            }
            if self.doc.eff_hidden(*id) {
                return Err(fail(TargetErrorCode::HiddenTarget, format!("path:{id} is hidden")));
            }
        }
        let result = (|| -> Result<(), String> {
            self.try_execute(EditCommand::SelectPaths(paths.clone()))?;
            match op {
                TargetEdit::Move { delta, .. } => {
                    if !delta.iter().all(|v| v.is_finite()) {
                        return Err("delta must be finite".into());
                    }
                    self.move_explicit(paths, *delta);
                }
                TargetEdit::Paint { fill, stroke, stroke_width, opacity, .. } => {
                    if fill.is_none() && stroke.is_none() && stroke_width.is_none() && opacity.is_none() {
                        return Err("paint needs at least one property".into());
                    }
                    if let Some(color) = fill {
                        self.try_execute(EditCommand::ApplyPaint {
                            target: crate::editor::PaintTarget::Fill,
                            color: *color,
                        })?;
                    }
                    if let Some(color) = stroke {
                        self.try_execute(EditCommand::ApplyPaint {
                            target: crate::editor::PaintTarget::Stroke,
                            color: *color,
                        })?;
                    }
                    if let Some(width) = stroke_width {
                        self.try_execute(EditCommand::SetStrokeWidth(*width))?;
                    }
                    if let Some(opacity) = opacity {
                        self.try_execute(EditCommand::SetOpacity(*opacity))?;
                    }
                }
            }
            Ok(())
        })();
        result.map_err(|reason| fail(TargetErrorCode::InvalidArgument, reason))?;
        Ok(())
    }
}

fn validate_targeted_stage(editor: &Editor) -> Result<(), String> {
    check_document(editor)?;
    crate::format::encode_model(&editor.doc, &format::Limits::DEFAULT).map_err(|e| e.to_string())?;
    Ok(())
}

/// Opaque staged document; only checked staging can construct it.
pub struct PreparedDesignBatch {
    editor: Editor,
    revision: u64,
}
impl PreparedDesignBatch {
    pub fn document(&self) -> &crate::model::Document {
        &self.editor.doc
    }
}
impl Editor {
    /// Resolve and validate each operation against isolated state; publish separately after host authorization.
    pub fn prepare_design_batch<E>(
        &self,
        count: usize,
        mut apply: impl FnMut(&mut Editor, usize) -> Result<(), E>,
        error: impl Fn(usize, String) -> E,
        cancelled: impl Fn() -> bool,
    ) -> Result<PreparedDesignBatch, E> {
        if self.transaction_open() {
            return Err(error(0, "active gesture".into()));
        }
        let mut staged = self.batch_stage();
        for index in 0..count {
            if cancelled() {
                return Err(error(index, "cancelled before commit".into()));
            }
            apply(&mut staged, index)?;
            staged.clear_batch_history();
        }
        if validate_targeted_stage(&staged).is_err() {
            let mut replay = self.batch_stage();
            for index in 0..count {
                if cancelled() {
                    return Err(error(index, "cancelled before commit".into()));
                }
                apply(&mut replay, index)?;
                validate_targeted_stage(&replay).map_err(|reason| error(index, reason))?;
                replay.clear_batch_history();
            }
        }
        if cancelled() {
            return Err(error(0, "cancelled before commit".into()));
        }
        staged.doc.active_layer = self.doc.active_layer;
        // `active` is NOT reset to the human's index: every staged operation keeps the active page by
        // stable id across artboard insertions/removals, and only `artboard_set_active` changes it.
        Ok(PreparedDesignBatch { editor: staged, revision: self.rev })
    }
    /// The owning-thread service rechecks cancellation and consumes grants before this single publication.
    pub fn publish_design_batch(&mut self, batch: PreparedDesignBatch) -> Result<(), String> {
        if self.transaction_open() || self.rev != batch.revision {
            return Err("staged revision changed".into());
        }
        // The human's artboard multi-selection is index-based: carry it across by stable id.
        let before = self.doc.active_artboard().map(|a| a.id);
        let picked: Vec<u32> = self.absel.iter().filter_map(|&i| self.doc.artboards.get(i).map(|a| a.id)).collect();
        let active = batch.editor.doc.active;
        self.publish_batch(batch.editor, true);
        // a set-active-only batch is no content change, so `publish_batch` kept this document: apply
        // the staged active index (same artboards, so the same index) as the navigation preference it is
        self.doc.active = active.min(self.doc.artboards.len().saturating_sub(1));
        let after = self.doc.active_artboard().map(|a| a.id);
        self.absel = if after != before {
            after.map(|_| self.doc.active).into_iter().collect()
        } else {
            picked.into_iter().filter_map(|id| self.doc.artboard_index(id)).collect()
        };
        Ok(())
    }
}

/// Why a checked artboard operation was refused (Bridge slice 3). The adapter maps the code to its
/// stable external error code; the reason is plain English.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArtboardErrorCode {
    NotFound,
    LockedTarget,
    InvalidArgument,
    LimitExceeded,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtboardError {
    pub code: ArtboardErrorCode,
    pub reason: String,
}
fn ab_error(code: ArtboardErrorCode, reason: impl Into<String>) -> ArtboardError {
    ArtboardError { code, reason: reason.into() }
}

/// A page rect for a checked artboard operation: finite, at least 1 pt on each side (the editor's own
/// minimum page size, `ab_set_rect`), and representable once added up — refused, never clamped.
fn check_page_rect(rect: [f32; 4]) -> Result<(), ArtboardError> {
    let [x, y, w, h] = rect;
    let bad = |reason: &str| Err(ab_error(ArtboardErrorCode::InvalidArgument, reason));
    if !rect.iter().all(|v| v.is_finite()) || !(x + w).is_finite() || !(y + h).is_finite() {
        return bad("artboard bounds must be finite");
    }
    if w < 1.0 || h < 1.0 {
        return bad("artboard width and height must be at least 1 pt");
    }
    if x + w <= x || y + h <= y {
        return bad("artboard bounds collapse at f32 precision");
    }
    Ok(())
}

/// Checked, id-addressed artboard operations for the Bridge (slice 3). Each is one ordinary
/// `begin`/`commit` edit, so inside the isolated staging editor a batch still publishes one undo step.
/// None of them changes the active artboard as a side effect: the active page is the human's
/// navigation, kept by id across insertions and removals. `artboard_set_active` is the only way to
/// change it, and the delete rule below refuses to leave it without an explicit choice.
impl Editor {
    fn artboard_checked(&self, id: u32) -> Result<usize, ArtboardError> {
        self.doc
            .artboard_index(id)
            .ok_or_else(|| ab_error(ArtboardErrorCode::NotFound, format!("unknown artboard:{id}")))
    }
    /// Where `artboard_add` places a page of size `w`×`h` when no position is given: to the right of
    /// the right-most page with the standard gap, top-aligned with the active page (the same slot as
    /// the Artboards panel's "+"), or at the origin on a free canvas.
    pub fn artboard_next_origin(&self) -> [f32; 2] {
        let right = self.doc.artboards.iter().map(|a| a.x + a.w).fold(f32::MIN, f32::max);
        let y = self.doc.active_artboard().map_or(0.0, |a| a.y);
        [if right > f32::MIN { right + crate::editor::AB_GAP } else { 0.0 }, y]
    }
    /// Append a page with `rect = [x, y, w, h]` and an optional (already cleaned, non-empty) name; the
    /// default name is "Artboard N". Returns the new page's stable id. The active page is unchanged.
    pub fn artboard_add(&mut self, rect: [f32; 4], name: Option<String>) -> Result<u32, ArtboardError> {
        check_page_rect(rect)?;
        if self.doc.artboards.len() >= format::Limits::DEFAULT.max_artboards {
            return Err(ab_error(ArtboardErrorCode::LimitExceeded, "artboard limit (1000) reached"));
        }
        let limits = format::Limits::DEFAULT;
        let reserve = (limits.max_nodes + limits.max_paths + limits.max_anchors) as u64;
        if u64::from(self.allocation_floor()) + reserve >= u64::from(u32::MAX) {
            return Err(ab_error(ArtboardErrorCode::LimitExceeded, "not enough stable ids remain"));
        }
        let active = self.doc.active_artboard().map(|a| a.id);
        self.begin();
        let id = self.doc.nid();
        let n = self.doc.artboards.len() + 1;
        let [x, y, w, h] = rect;
        let name = name.unwrap_or_else(|| format!("Artboard {n}"));
        self.doc.artboards.push(Artboard { id, x, y, w, h, name, ..Artboard::default() });
        self.keep_active(active);
        self.dirty = true;
        self.commit();
        Ok(id)
    }
    /// Set a page's rect exactly (artwork does not move with it — the panel's X/Y/W/H behaviour).
    /// A locked page is refused: it locks what stands on it, and resizing changes that membership.
    pub fn artboard_set_rect(&mut self, id: u32, rect: [f32; 4]) -> Result<(), ArtboardError> {
        let i = self.artboard_checked(id)?;
        check_page_rect(rect)?;
        if self.doc.artboards[i].locked {
            return Err(ab_error(ArtboardErrorCode::LockedTarget, format!("artboard:{id} is locked")));
        }
        self.begin();
        let [x, y, w, h] = rect;
        let ab = &mut self.doc.artboards[i];
        (ab.x, ab.y, ab.w, ab.h) = (x, y, w, h);
        self.dirty = true;
        self.commit();
        Ok(())
    }
    /// Rename a page; `name` must already be cleaned and non-empty (the adapter cleans it).
    pub fn artboard_rename(&mut self, id: u32, name: String) -> Result<(), ArtboardError> {
        let i = self.artboard_checked(id)?;
        if name.trim().is_empty() {
            return Err(ab_error(ArtboardErrorCode::InvalidArgument, "name must not be empty"));
        }
        self.begin();
        self.doc.artboards[i].name = name;
        self.dirty = true;
        self.commit();
        Ok(())
    }
    /// Remove one page; its artwork stays where it is (it becomes a floater, or stays on the other pages
    /// it overlaps — the same as the panel's delete). THE ACTIVE-ARTBOARD RULE: deleting the active page
    /// while other pages remain is refused — choose the new active page first with
    /// `artboard_set_active`; deleting the last page leaves a free canvas (`active = 0`, never indexed).
    /// A locked page is refused.
    pub fn artboard_delete(&mut self, id: u32) -> Result<(), ArtboardError> {
        let i = self.artboard_checked(id)?;
        if self.doc.artboards[i].locked {
            return Err(ab_error(ArtboardErrorCode::LockedTarget, format!("artboard:{id} is locked")));
        }
        let active = self.doc.active_artboard().map(|a| a.id);
        if active == Some(id) && self.doc.artboards.len() > 1 {
            return Err(ab_error(
                ArtboardErrorCode::InvalidArgument,
                format!(
                    "artboard:{id} is the active artboard; set_active_artboard to another artboard earlier in \
                     the batch, then delete it"
                ),
            ));
        }
        self.begin();
        self.doc.artboards.remove(i);
        self.keep_active(active);
        self.dirty = true;
        self.commit();
        Ok(())
    }
    /// Move a page to a zero-based position; active navigation follows its stable id.
    pub fn artboard_reorder(&mut self, id: u32, position: usize) -> Result<(), ArtboardError> {
        let i = self.artboard_checked(id)?;
        if position >= self.doc.artboards.len() {
            return Err(ab_error(ArtboardErrorCode::InvalidArgument, "position out of range"));
        }
        let active = self.doc.active_artboard().map(|a| a.id);
        self.begin();
        let page = self.doc.artboards.remove(i);
        self.doc.artboards.insert(position, page);
        self.keep_active(active);
        self.dirty = true;
        self.commit();
        Ok(())
    }
    /// Desktop membership/copy semantics, with explicit artwork and offset choices.
    pub fn artboard_duplicate(
        &mut self,
        id: u32,
        with_art: bool,
        offset: Option<[f32; 2]>,
    ) -> Result<u32, ArtboardError> {
        let i = self.artboard_checked(id)?;
        let src = self.doc.artboards[i].clone();
        if src.locked {
            return Err(ab_error(ArtboardErrorCode::LockedTarget, "page is locked"));
        }
        let d = offset.unwrap_or([src.w + crate::editor::AB_GAP, 0.0]);
        let active = self.doc.active_artboard().map(|a| a.id);
        check_page_rect([src.x + d[0], src.y + d[1], src.w, src.h])?;
        let limits = format::Limits::DEFAULT;
        if self.doc.artboards.len() >= limits.max_artboards
            || u64::from(self.allocation_floor()) + (limits.max_nodes + limits.max_paths + limits.max_anchors) as u64
                >= u64::from(u32::MAX)
        {
            return Err(ab_error(ArtboardErrorCode::LimitExceeded, "page or id limit reached"));
        }
        self.begin();
        let created = self.doc.nid();
        if with_art {
            self.copy_artboard_art(i, d);
        }
        let mut page = src;
        page.id = created;
        page.x += d[0];
        page.y += d[1];
        page.name = format!("{} copy", page.name);
        self.doc.artboards.insert(i + 1, page);
        self.keep_active(active);
        self.dirty = true;
        self.commit();
        Ok(created)
    }
    pub fn artboard_color(&mut self, id: u32, color: Option<[f32; 4]>) -> Result<(), ArtboardError> {
        let i = self.artboard_checked(id)?;
        if color.is_some_and(|c| c.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))) {
            return Err(ab_error(ArtboardErrorCode::InvalidArgument, "invalid color"));
        }
        self.begin();
        self.doc.artboards[i].page_color = color;
        self.dirty = true;
        self.commit();
        Ok(())
    }
    pub fn artboard_clip(&mut self, id: u32, clip: bool) -> Result<(), ArtboardError> {
        let i = self.artboard_checked(id)?;
        self.begin();
        self.doc.artboards[i].clip = clip;
        self.dirty = true;
        self.commit();
        Ok(())
    }
    /// Make a page the active one (a navigation preference: no undo step on its own).
    pub fn artboard_set_active(&mut self, id: u32) -> Result<(), ArtboardError> {
        self.doc.active = self.artboard_checked(id)?;
        Ok(())
    }
    /// Re-point `active` at the page with stable id `id` after an insertion/removal (free canvas → 0).
    fn keep_active(&mut self, id: Option<u32>) {
        self.doc.active = id
            .and_then(|id| self.doc.artboard_index(id))
            .unwrap_or_else(|| self.doc.active.min(self.doc.artboards.len().saturating_sub(1)));
    }
}

#[cfg(test)]
mod slice4_tests {
    use super::*;
    #[test]
    fn explicit_path_refuses_nonfinite_points_and_handles_without_allocation() {
        let mut ed = Editor::new();
        let before = ed.doc.clone();
        for (p, hin, hout) in [
            ([f32::NAN, 0.0], None, None),
            ([3.4e38, 0.0], None, None),
            ([0.0, 0.0], Some([-3.4e38, 0.0]), None),
            ([0.0, 0.0], None, Some([3.4e38, 0.0])),
            ([0.0, 0.0], Some([0.0, f32::INFINITY]), None),
            ([0.0, 0.0], None, Some([f32::NEG_INFINITY, 0.0])),
        ] {
            let a = crate::model::Anchor { id: 123, p, hin, hout, smooth: false };
            let cmd = EditCommand::AddPath {
                anchors: vec![a.clone(), a],
                closed: false,
                parent: None,
                fill: Some([1.0; 4]),
                stroke: None,
                stroke_width: 0.0,
                opacity: 1.0,
                name: None,
            };
            assert!(ed.try_execute_created(cmd).is_err());
            assert_eq!(ed.doc, before);
            assert!(!ed.history_available(false));
        }
    }
}
