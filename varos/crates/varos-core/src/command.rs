//! Internal edit-command boundary.
//!
//! This closed enum is application plumbing, not a stable plugin or AI protocol. Adapters,
//! versioning, permissions, compatibility, and query contracts require separate decisions.

use crate::board;
use crate::boolean::BoolOp;
use crate::editor::{AlignMode, AlignTarget, DistAxis, Editor, PaintTarget, ZOrder};
use crate::geom::{Pt, Rgba};
use crate::model::{DropPos, SnapConfig};

/// A deterministic edit or history action executed entirely inside `varos-core`.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub enum EditCommand {
    // ---- Lane D: deterministic drawing boundary ----
    Drawing(crate::drawing::Action),
    // ---- Lane G ----
    AddText {
        text: crate::text::TextBox,
        parent: Option<u32>,
    },
    SetText {
        id: u32,
        text: crate::text::TextBox,
    },
    // ---- w2-images ----
    Image(crate::images::ImageEdit),
    // ---- w2-gradients ----
    Colour(crate::colour_commands::ColourCommand),
    // ---- Lane C ----
    PathAdvanced(crate::path_advanced::Action),
    SetCornersLive {
        path: u32,
        corners: Vec<crate::live_corners::CornerParam>,
    },
    SetCorners {
        path: u32,
        corners: Vec<crate::live_corners::CornerParam>,
    },
    SetScaleStrokes(bool),
    NewDocument(crate::new_document::Settings),
    // ---- Lane F ----
    HistoryJump {
        undo_depth: usize,
    },
    SetWandOptions(crate::select_transform::WandOptions),
    SetEyedropperOptions(crate::select_transform::PickOptions),
    Transform(crate::select_transform::Transform),
    TransformBegin,
    TransformLive(crate::select_transform::Transform),
    TransformCommit,
    TransformCancel,
    MagicWand {
        source: u32,
        options: crate::select_transform::WandOptions,
        mode: crate::select_transform::SelectMode,
    },
    Eyedropper {
        source: u32,
        options: crate::select_transform::PickOptions,
        colour_only: bool,
    },
    Isolate(Option<u32>),
    LayerFamily {
        action: crate::select_transform::LayerAction,
        nodes: Vec<u32>,
    },
    /// Insert a pure trace result as one undoable edit; all IDs are remapped.
    InsertTracedPaths {
        paths: Vec<crate::model::Path>,
    },
    /// Place normalized artwork as one group with fresh ids and one undo entry.
    PlaceArtwork(Box<crate::model::Document>),
    ZoomPercent(f32),
    View(crate::editor::view_commands::ViewAction),
    Selection(crate::editor::wave::Selection),
    Lasso {
        points: Vec<Pt>,
        objects: bool,
        additive: bool,
    },
    InsertAnchor {
        path: u32,
        segment: usize,
        t: f32,
    },
    DeleteAnchor(u32),
    AnchorType {
        anchor: u32,
        smooth: bool,
    },
    DistributeMode(AlignMode),
    SetDistributeGap(f32),
    SetPasteRemembersLayers(bool),
    SetKeyObject(Option<u32>),
    Object(crate::editor::wave::ObjectAction),
    DistributeSpacing {
        axis: DistAxis,
        gap: f32,
    },
    /// Deterministic creation; checked callers use `try_execute_created` for the allocated path id.
    AddShape {
        kind: crate::model::ShapeKind,
        bounds: [f32; 4],
        parent: Option<u32>,
        fill: Option<Rgba>,
        stroke: Option<Rgba>,
        stroke_width: f32,
        opacity: f32,
        name: Option<String>,
    },
    AddPath {
        anchors: Vec<crate::model::Anchor>,
        closed: bool,
        parent: Option<u32>,
        fill: Option<Rgba>,
        stroke: Option<Rgba>,
        stroke_width: f32,
        opacity: f32,
        name: Option<String>,
    },
    /// Explicit headless object selection, using stable path ids.
    #[serde(rename = "SelectPaths")]
    SelectPaths(Vec<u32>),
    /// Explicit direct selection, using stable anchor ids.
    #[serde(rename = "SelectAnchors")]
    SelectAnchors(Vec<u32>),
    /// Transform-panel X/Y/W/H. Edits the object selection, or — when there is none — the Direct
    /// selection (selected anchors / Direct path-level selection; Astra F07).
    #[serde(rename = "SetObjectBounds")]
    SetObjectBounds {
        #[serde(rename = "x")]
        x: Option<f32>,
        #[serde(rename = "y")]
        y: Option<f32>,
        #[serde(rename = "width")]
        width: Option<f32>,
        #[serde(rename = "height")]
        height: Option<f32>,
        #[serde(rename = "anchor_x")]
        anchor_x: f32,
        #[serde(rename = "anchor_y")]
        anchor_y: f32,
    },
    #[serde(rename = "SetObjectRotation")]
    SetObjectRotation(f32),
    #[serde(rename = "SetOpacity")]
    SetOpacity(f32),
    #[serde(rename = "SetStrokeWidth")]
    SetStrokeWidth(f32),
    SetStrokeStyle {
        ids: Vec<u32>,
        style: crate::stroke::StrokeStyle,
    },
    #[serde(rename = "SetClipExempt")]
    SetClipExempt(bool),
    #[serde(rename = "ApplyPaint")]
    ApplyPaint {
        #[serde(rename = "target")]
        target: PaintTarget,
        #[serde(rename = "color")]
        color: Option<Rgba>,
    },
    #[serde(rename = "SwapColors")]
    SwapColors,
    #[serde(rename = "DefaultPaint")]
    DefaultPaint,
    #[serde(rename = "PickerBegin")]
    PickerBegin,
    #[serde(rename = "PickerLivePaint")]
    PickerLivePaint {
        #[serde(rename = "target")]
        target: PaintTarget,
        #[serde(rename = "color")]
        color: Rgba,
    },
    #[serde(rename = "PickerLiveArtboard")]
    PickerLiveArtboard {
        #[serde(rename = "index")]
        index: usize,
        #[serde(rename = "color")]
        color: Rgba,
    },
    #[serde(rename = "PickerCommit")]
    PickerCommit {
        #[serde(rename = "current")]
        current: Option<PaintTarget>,
        #[serde(rename = "color")]
        color: Rgba,
    },
    #[serde(rename = "PickerCancel")]
    PickerCancel,
    #[serde(rename = "ToggleNodeHidden")]
    ToggleNodeHidden(u32),
    #[serde(rename = "ToggleNodeLocked")]
    ToggleNodeLocked(u32),
    #[serde(rename = "RenameNode")]
    RenameNode {
        #[serde(rename = "node")]
        node: u32,
        #[serde(rename = "name")]
        name: String,
    },
    /// Name a path (its Layers row, the inspector header). A path's leaf node does not carry the
    /// displayed name — `Path::name` does — so a `<Path>` row renames through this, not `RenameNode`
    /// (QW3 / Astra F10). The name is trimmed; an empty or unchanged name is a no-op (no undo step,
    /// the document stays clean) — Illustrator keeps the old name when the field is emptied.
    #[serde(rename = "RenamePath")]
    RenamePath {
        #[serde(rename = "path")]
        path: u32,
        #[serde(rename = "name")]
        name: String,
    },
    #[serde(rename = "GroupSelection")]
    GroupSelection,
    ClipMake,
    ClipRelease,
    #[cfg(test)]
    ForcedPanic,
    #[serde(rename = "UngroupSelection")]
    UngroupSelection,
    #[serde(rename = "DeleteLayerSelection")]
    DeleteLayerSelection,
    #[serde(rename = "MoveLayer")]
    MoveLayer {
        #[serde(rename = "sources")]
        sources: Vec<u32>,
        #[serde(rename = "target")]
        target: u32,
        #[serde(rename = "position")]
        position: DropPos,
    },
    #[serde(rename = "DuplicateMoveLayer")]
    DuplicateMoveLayer {
        #[serde(rename = "sources")]
        sources: Vec<u32>,
        #[serde(rename = "target")]
        target: u32,
        #[serde(rename = "position")]
        position: DropPos,
    },
    #[serde(rename = "MoveLayerToBoard")]
    MoveLayerToBoard {
        #[serde(rename = "sources")]
        sources: Vec<u32>,
        #[serde(rename = "source_board")]
        source_board: Option<usize>,
        #[serde(rename = "target_board")]
        target_board: usize,
    },
    #[serde(rename = "Flip")]
    Flip(bool),
    #[serde(rename = "Align")]
    Align {
        #[serde(rename = "mode")]
        mode: AlignMode,
        #[serde(rename = "target")]
        target: AlignTarget,
    },
    #[serde(rename = "Distribute")]
    Distribute(DistAxis),
    #[serde(rename = "Boolean")]
    Boolean(BoolOp),
    #[serde(rename = "Pathfinder")]
    Pathfinder(crate::planar::PathfinderOp),
    #[serde(rename = "ShapeBuilder")]
    ShapeBuilder {
        points: Vec<Pt>,
        delete: bool,
    },
    #[serde(rename = "Scissors")]
    Scissors {
        path: u32,
        segment: usize,
        t: f32,
    },
    #[serde(rename = "Knife")]
    Knife {
        points: Vec<Pt>,
    },
    #[serde(rename = "Eraser")]
    Eraser {
        points: Vec<Pt>,
        radius: f32,
    },
    #[serde(rename = "DivideObjectsBelow")]
    DivideObjectsBelow,

    #[serde(rename = "Arrange")]
    Arrange(ZOrder),
    #[serde(rename = "TransformAgain")]
    TransformAgain,
    #[serde(rename = "DeleteSelected")]
    DeleteSelected,
    /// Edit ▸ Copy: selection → the in-app clipboard. Leaves the document untouched (no history).
    #[serde(rename = "Copy")]
    Copy,
    /// Edit ▸ Cut: Copy + delete the selection, as ONE undo step.
    #[serde(rename = "Cut")]
    Cut,
    /// Edit ▸ Paste / Paste in Place: a fresh copy of the clipboard onto the active layer, selected,
    /// as ONE undo step. `offset` = world translation from the copied position (the app passes the
    /// delta that centres the art in the view — core has no view); `None` = in place (⇧⌘V).
    #[serde(rename = "Paste")]
    Paste {
        #[serde(rename = "offset")]
        offset: Option<Pt>,
    },
    #[serde(rename = "Nudge")]
    Nudge {
        #[serde(rename = "x")]
        x: f32,
        #[serde(rename = "y")]
        y: f32,
    },
    #[serde(rename = "SetActiveArtboard")]
    SetActiveArtboard(usize),
    #[serde(rename = "SetArtboardRect")]
    SetArtboardRect {
        #[serde(rename = "index")]
        index: usize,
        #[serde(rename = "x")]
        x: Option<f32>,
        #[serde(rename = "y")]
        y: Option<f32>,
        #[serde(rename = "width")]
        width: Option<f32>,
        #[serde(rename = "height")]
        height: Option<f32>,
    },
    #[serde(rename = "RenameArtboard")]
    RenameArtboard {
        #[serde(rename = "index")]
        index: usize,
        #[serde(rename = "name")]
        name: String,
    },
    #[serde(rename = "SetArtboardColor")]
    SetArtboardColor {
        #[serde(rename = "index")]
        index: usize,
        #[serde(rename = "color")]
        color: Option<Rgba>,
    },
    #[serde(rename = "ToggleArtboardClip")]
    ToggleArtboardClip(usize),
    #[serde(rename = "ToggleArtboardHidden")]
    ToggleArtboardHidden(usize),
    #[serde(rename = "ToggleArtboardLocked")]
    ToggleArtboardLocked(usize),
    #[serde(rename = "OrientArtboard")]
    OrientArtboard(usize),
    #[serde(rename = "AddArtboard")]
    AddArtboard,
    #[serde(rename = "DuplicateArtboard")]
    DuplicateArtboard(usize),
    #[serde(rename = "DeleteArtboard")]
    DeleteArtboard(usize),
    #[serde(rename = "SetArtboardCount")]
    SetArtboardCount(usize),
    #[serde(rename = "SetMoveArtWithArtboard")]
    SetMoveArtWithArtboard(bool),
    #[serde(rename = "SetRulerOrigin")]
    SetRulerOrigin(Pt),
    #[serde(rename = "CommitGuide")]
    CommitGuide,
    #[serde(rename = "CycleUnits")]
    CycleUnits,
    #[serde(rename = "SetUnits")]
    SetUnits(crate::units::Unit),
    SetPpi(f32),
    SetBleed {
        index: usize,
        edges: [f32; 4],
    },
    SetTransparencyGrid(bool),
    #[serde(rename = "SetSnapConfig")]
    SetSnapConfig(SnapConfig),
    #[serde(rename = "ToggleSnapping")]
    ToggleSnapping,
    #[serde(rename = "ToggleGuidesLocked")]
    ToggleGuidesLocked,
    #[serde(rename = "ToggleSmartGuides")]
    ToggleSmartGuides,
    /// Board metadata (format 3, `crate::board`). Each is ONE undo step and dirties the document; an
    /// unchanged value is a no-op (no undo step, the document stays clean). These variants only ever
    /// carry VALID values: UI input goes through the checked path `Editor::try_set_board_name /
    /// _description / _tags`, which returns the plain-English `board::Reject` (the field keeps focus
    /// and shows it) and builds the command only from valid, cleaned input. The variant re-cleans
    /// (idempotent) and asserts validity in debug builds; a release build still never stores an
    /// invalid value (it is ignored).
    #[serde(rename = "SetBoardName")]
    SetBoardName(String),
    #[serde(rename = "SetBoardDescription")]
    SetBoardDescription(String),
    #[serde(rename = "SetBoardTags")]
    SetBoardTags(Vec<String>),
    #[serde(rename = "Undo")]
    Undo,
    #[serde(rename = "Redo")]
    Redo,
}

impl EditCommand {
    fn apply(self, ed: &mut Editor) {
        match self {
            Self::Drawing(action) => crate::drawing::apply(ed, action),
            Self::Image(edit) => crate::images::apply(ed, edit),
            // Checked colour command dispatch.
            Self::Colour(c) => crate::colour_commands::apply(ed, c),
            // ---- Lane C ----
            Self::PathAdvanced(action) => ed.path_advanced(action),
            Self::SetCornersLive { path, corners } => {
                if let Some(i) = ed.doc.pidx(path) {
                    if crate::live_corners::validate(&ed.doc.paths[i], &corners).is_ok() {
                        ed.doc.paths[i].corners = corners;
                        ed.dirty = true;
                    }
                }
            }
            Self::SetCorners { path, corners } => ed.set_corners(path, corners),
            Self::SetScaleStrokes(on) => ed.select_transform.scale_strokes = on,
            Self::NewDocument(settings) => {
                if let Ok(doc) = settings.document() {
                    ed.begin();
                    ed.doc = doc;
                    ed.dirty = true;
                    ed.commit();
                }
            }
            Self::SetWandOptions(options) => {
                ed.select_transform.wand = options;
                ed.select_transform.options_requested = true;
            }
            Self::SetEyedropperOptions(options) => {
                ed.select_transform.pick = options;
                ed.select_transform.options_requested = true;
            }
            Self::Transform(s) => ed.transform_edit(s),
            Self::TransformBegin => ed.transform_begin(),
            Self::TransformLive(s) => ed.transform_live(s),
            Self::TransformCommit => ed.transform_end(false),
            Self::TransformCancel => ed.transform_end(true),
            Self::MagicWand { source, options, mode } => ed.magic_wand(source, options, mode),
            Self::Eyedropper { source, options, colour_only } => ed.sample_options(source, options, colour_only),
            // ---- Lane G ----
            Self::AddText { text, parent } => {
                let _ = crate::text::add(ed, text, parent);
            }
            Self::SetText { id, text } => {
                let _ = crate::text::set(ed, id, text);
            }
            Self::Isolate(n) => ed.isolate(n),
            Self::LayerFamily { action, nodes } => ed.layer_family(action, nodes),
            Self::InsertTracedPaths { paths } => {
                if paths.is_empty() {
                    return;
                }
                if crate::trace::check_insert(ed, &paths).is_err() {
                    return;
                }
                ed.begin();
                for mut path in paths {
                    path.id = ed.doc.nid();
                    for a in path.anchors.iter_mut().chain(path.holes.iter_mut().flatten()) {
                        a.id = ed.doc.nid();
                    }
                    ed.doc.paths.push(path);
                }
                ed.doc.sync_tree();
                ed.dirty = true;
                ed.commit();
            }
            Self::PlaceArtwork(doc) => crate::placement::place(ed, *doc),
            Self::ZoomPercent(value) => ed.requested_zoom = Some(value),
            Self::View(action) => ed.view_command(action),
            Self::Selection(action) => ed.selection_command(action),
            Self::Lasso { points, objects, additive } => ed.lasso_select(&points, objects, additive),
            Self::InsertAnchor { path, segment, t } => ed.wave_insert_anchor(path, segment, t),
            Self::AnchorType { anchor, smooth } => ed.wave_anchor_type(anchor, smooth),
            Self::DeleteAnchor(id) => ed.wave_delete_anchor(id),
            Self::DistributeMode(mode) => ed.distribute_mode(mode),
            Self::SetPasteRemembersLayers(enabled) => ed.paste_remembers_layers = enabled,
            Self::SetDistributeGap(gap) => ed.distribute_gap = gap,
            Self::SetKeyObject(id) => ed.key_object = id,
            Self::Object(action) => ed.object_command(action),
            Self::DistributeSpacing { axis, gap } => ed.distribute_spacing_wave(axis, gap),
            Self::AddPath { .. } => {
                let _ = ed.try_execute_created(self);
            }
            Self::AddShape { kind, bounds, parent, fill, stroke, stroke_width, opacity, name } => {
                let _ = ed.add_shape(kind, bounds, parent, fill, stroke, stroke_width, opacity, name);
            }
            Self::SelectPaths(paths) => {
                ed.escape_selection();
                ed.tool = crate::editor::ToolKind::Object;
                ed.objsel.extend(paths.into_iter().filter(|p| ed.in_isolation(*p)).collect::<Vec<_>>());
                ed.refresh_obj_angle();
            }
            Self::SelectAnchors(anchors) => {
                ed.escape_selection();
                ed.tool = crate::editor::ToolKind::Direct;
                ed.selected.extend(anchors);
            }
            Self::SetObjectBounds { x, y, width, height, anchor_x, anchor_y } => {
                ed.set_obj_bbox(x, y, width, height, anchor_x, anchor_y)
            }
            Self::SetObjectRotation(degrees) => ed.set_obj_rotation(degrees),
            Self::SetOpacity(opacity) => ed.set_opacity(opacity),
            Self::SetStrokeWidth(width) => set_stroke_width(ed, width),
            Self::SetStrokeStyle { ids, style } => {
                let command = Self::SetStrokeStyle { ids: ids.clone(), style: style.clone() };
                if crate::bridge::check(&command, ed).is_err() {
                    return;
                }
                if !ed.doc.paths.iter().any(|p| ids.contains(&p.id) && p.stroke_style != style) {
                    return;
                }
                ed.begin();
                for p in &mut ed.doc.paths {
                    if ids.contains(&p.id) {
                        p.stroke_style = style.clone();
                    }
                }
                ed.dirty = true;
                ed.commit();
            }
            Self::SetClipExempt(exempt) => ed.set_clip_exempt(exempt),
            Self::ApplyPaint { target, color } => {
                ed.set_paint_target(target);
                ed.apply_paint(color);
            }
            Self::SwapColors => ed.swap_colors(),
            Self::DefaultPaint => ed.default_paint(),
            Self::PickerBegin => ed.picker_begin(),
            Self::PickerLivePaint { target, color } => ed.paint_live(target, Some(color)),
            Self::PickerLiveArtboard { index, color } => ed.ab_color_live(index, Some(color)),
            Self::PickerCommit { current, color } => ed.picker_commit(current, color),
            Self::PickerCancel => ed.picker_cancel(),
            Self::ToggleNodeHidden(node) => ed.layer_toggle_hidden(node),
            Self::ToggleNodeLocked(node) => ed.layer_toggle_locked(node),
            Self::RenameNode { node, name } => ed.layer_rename(node, name),
            Self::RenamePath { path, name } => rename_path(ed, path, name),
            Self::GroupSelection => ed.group_selection(),
            Self::ClipMake => ed.clip_make(),
            Self::ClipRelease => ed.clip_release(),
            #[cfg(test)]
            Self::ForcedPanic => {
                ed.begin();
                ed.doc.paths.clear();
                ed.objsel.clear();
                ed.dirty = true;
                ed.commit();
                panic!("forced command panic");
            }
            Self::UngroupSelection => ed.ungroup_selection(),
            Self::DeleteLayerSelection => ed.layer_delete_selection(),
            Self::MoveLayer { sources, target, position } => ed.layer_move(&sources, target, position),
            Self::DuplicateMoveLayer { sources, target, position } => ed.layer_dup_move(&sources, target, position),
            Self::MoveLayerToBoard { sources, source_board, target_board } => {
                ed.layer_move_to_board(&sources, source_board, target_board)
            }
            Self::Flip(horizontal) => ed.flip(horizontal),
            Self::Align { mode, target } => ed.align(mode, target),
            Self::Distribute(axis) => ed.distribute(axis),
            Self::Boolean(operation) => ed.pathfinder(operation),
            Self::Pathfinder(operation) => ed.planar_pathfinder(operation),
            Self::ShapeBuilder { points, delete } => ed.shape_builder(&points, delete),
            Self::Scissors { path, segment, t } => ed.scissors(path, segment, t),
            Self::Knife { points } => ed.cut_fills(&points, None),
            Self::Eraser { points, radius } => ed.cut_fills(&points, Some(radius)),
            Self::DivideObjectsBelow => ed.divide_objects_below(),
            Self::Arrange(order) => ed.arrange(order),
            Self::TransformAgain => ed.transform_again(),
            Self::DeleteSelected => ed.delete_selected(),
            Self::Copy => ed.copy_selection(),
            Self::Cut => ed.cut_selection(),
            Self::Paste { offset } => ed.paste(offset),
            Self::Nudge { x, y } => {
                // ---- Lane F: one keyboard increment for object and anchor selections ----
                if ed.selected.is_empty() {
                    ed.move_explicit(&ed.objsel.iter().copied().collect::<Vec<_>>(), [x, y]);
                } else {
                    ed.nudge(x, y);
                }
            }
            Self::SetActiveArtboard(index) => ed.ab_set_active(index),
            Self::SetArtboardRect { index, x, y, width, height } => ed.ab_set_rect(index, x, y, width, height),
            Self::RenameArtboard { index, name } => ed.ab_rename(index, name),
            Self::SetArtboardColor { index, color } => ed.ab_set_color(index, color),
            Self::ToggleArtboardClip(index) => ed.ab_toggle_clip(index),
            Self::ToggleArtboardHidden(index) => ed.ab_toggle_hidden(index),
            Self::ToggleArtboardLocked(index) => ed.ab_toggle_locked(index),
            Self::OrientArtboard(index) => ed.ab_orient(index),
            Self::AddArtboard => ed.ab_add(),
            Self::DuplicateArtboard(index) => ed.ab_duplicate(index),
            Self::DeleteArtboard(index) => ed.ab_delete(index),
            Self::SetArtboardCount(count) => ed.ab_set_count(count),
            Self::SetMoveArtWithArtboard(enabled) => ed.ab_set_move_art(enabled),
            Self::SetRulerOrigin(point) => {
                let origin = ed.snap_origin(point);
                ed.doc.ruler_origin = origin;
                ed.origin_preview = Some(origin);
            }
            Self::CommitGuide => ed.commit_guide(),
            Self::CycleUnits => ed.cycle_units(),
            Self::SetUnits(unit) => {
                if ed.doc.units.display != unit {
                    edit_setup(ed, |d| d.units.display = unit);
                }
            }
            Self::SetPpi(ppi) => {
                if crate::document_setup::valid_ppi(ppi) && ed.doc.units.ppi != ppi {
                    edit_setup(ed, |d| d.units.ppi = ppi);
                }
            }
            Self::SetBleed { index, edges } => {
                if crate::document_setup::valid_bleed(edges)
                    && ed.doc.artboards.get(index).is_some_and(|a| crate::document_setup::bleed(a) != edges)
                {
                    edit_setup(ed, |d| {
                        d.artboards[index].bleed = edges.iter().copied().fold(0.0_f32, f32::max);
                        d.artboards[index].bleed_edges = (edges.iter().any(|v| *v != edges[0])).then_some(edges);
                    });
                }
            }
            Self::SetTransparencyGrid(on) => {
                if ed.doc.transparency_grid != on {
                    edit_setup(ed, |d| d.transparency_grid = on);
                }
            }
            Self::SetSnapConfig(config) => ed.doc.snap = config,
            Self::ToggleSnapping => ed.doc.snap.enabled = !ed.doc.snap.enabled,
            Self::ToggleGuidesLocked => ed.doc.guides_locked = !ed.doc.guides_locked,
            Self::ToggleSmartGuides => ed.doc.snap.smart = !ed.doc.snap.smart,
            Self::SetBoardName(name) => {
                let name = board::clean_text(&name);
                if valid_replay(board::check_name(&name)) && name != ed.doc.name {
                    edit_board(ed, |d| d.name = name);
                }
            }
            Self::SetBoardDescription(text) => {
                let text = board::clean_text(&text);
                if valid_replay(board::check_description(&text)) && text != ed.doc.description {
                    edit_board(ed, |d| d.description = text);
                }
            }
            Self::SetBoardTags(tags) => {
                let tags = board::normalize_tags(tags);
                if valid_replay(board::check_tags(&tags)) && tags != ed.doc.tags {
                    edit_board(ed, |d| d.tags = tags);
                }
            }
            Self::HistoryJump { undo_depth } => {
                let _ = ed.history_jump(undo_depth);
            }
            Self::Undo => ed.undo(),
            Self::Redo => ed.redo(),
        }
    }
}

impl Editor {
    /// Checked headless path; interactive callers use the error-retaining `execute_ui` facade.
    pub fn try_execute(&mut self, command: EditCommand) -> Result<(), String> {
        crate::bridge::check(&command, self)?;
        self.execute(command).map_err(|e| e.to_string())
    }

    /// Checked creation returns the actual allocated identity, never a guessed counter.
    pub fn try_execute_created(&mut self, command: EditCommand) -> Result<u32, String> {
        // Immutable history handles bound rollback cost independently of retained artwork.
        let snapshot = self.clone();
        self.clipping_enablement.get_mut().take();
        match crate::guard::catch_panic(|| self.execute_created_inner(command)) {
            Ok(result) => result,
            Err(error) => {
                *self = snapshot;
                Err(error.to_string())
            }
        }
    }
    fn execute_created_inner(&mut self, command: EditCommand) -> Result<u32, String> {
        crate::bridge::check(&command, self)?;
        match command {
            // ---- Lane G ----
            EditCommand::AddText { text, parent } => crate::text::add(self, text, parent),
            EditCommand::AddShape { kind, bounds, parent, fill, stroke, stroke_width, opacity, name } => {
                self.add_shape(kind, bounds, parent, fill, stroke, stroke_width, opacity, name)
            }
            EditCommand::AddPath { mut anchors, closed, parent, fill, stroke, stroke_width, opacity, name } => {
                self.begin();
                let active = self.doc.active_layer;
                self.doc.active_layer = parent.unwrap_or(active);
                let id = self.doc.nid();
                for anchor in &mut anchors {
                    anchor.id = self.doc.nid();
                }
                let mut path = crate::model::Path::new(id, anchors, closed, fill, stroke, stroke_width);
                path.opacity = opacity;
                path.name = name.map(|n| clean_name(&n).to_owned());
                self.doc.paths.push(path);
                self.doc.sync_tree();
                self.doc.active_layer = active;
                self.dirty = true;
                self.commit();
                Ok(id)
            }
            _ => Err("command does not create a path".into()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add_shape(
        &mut self,
        kind: crate::model::ShapeKind,
        bounds: [f32; 4],
        parent: Option<u32>,
        fill: Option<Rgba>,
        stroke: Option<Rgba>,
        stroke_width: f32,
        opacity: f32,
        name: Option<String>,
    ) -> Result<u32, String> {
        let command =
            EditCommand::AddShape { kind, bounds, parent, fill, stroke, stroke_width, opacity, name: name.clone() };
        crate::bridge::check(&command, self)?;
        self.begin(); // reserves above the session high-water mark before any allocation
        let active = self.doc.active_layer;
        self.doc.active_layer = parent.unwrap_or(active);
        let id = self.doc.nid();
        let [x, y, w, h] = bounds;
        let anchors = self.doc.build_shape(kind, [x, y], [x + w, y + h]);
        let mut path = crate::model::Path::new(id, anchors, true, fill, stroke, stroke_width);
        path.opacity = opacity;
        if let Some(name) = name {
            path.name = Some(clean_name(&name).to_owned());
        }
        self.doc.paths.push(path);
        self.doc.sync_tree();
        self.doc.active_layer = active;
        self.dirty = true;
        self.commit();
        Ok(id)
    }

    /// Execute one deterministic edit through the core-owned command boundary.
    pub fn execute_ui(&mut self, command: EditCommand) {
        if let Err(error) = self.execute(command) {
            self.last_error = Some(error);
        }
    }

    /// Fallible command boundary. The interactive facade retains errors for its existing notice path.
    pub fn execute(&mut self, command: EditCommand) -> Result<(), crate::EngineError> {
        // Immutable history handles bound rollback cost independently of retained artwork.
        let snapshot = self.clone();
        let label = crate::command_labels::label(&command);
        let semantic = crate::actions::semantic(&command);
        // ---- Lane F: bind every supported edit to the recording's original selection ----
        if semantic.is_some() {
            self.check_action_targets(&self.objsel.iter().copied().collect::<Vec<_>>());
        }
        let previous_step = std::mem::replace(&mut self.action_commit_step, semantic);
        let before = self.rev;
        self.clipping_enablement.get_mut().take();
        let result = crate::guard::catch_panic(|| {
            command.apply(self);
            // One invariant gate for every command, including commands whose geometry changes artboard
            // membership (and therefore effective hidden/locked state) without touching a node flag.
            self.prune_inert_selection();
        });
        if result.is_err() {
            *self = snapshot;
        } else {
            self.action_commit_step = previous_step;
            self.annotate_history(before, crate::editor::history::Actor::Human, label.into());
        }
        result
    }

    pub fn set_paint_target(&mut self, target: PaintTarget) {
        self.paint = target;
    }

    pub fn set_constrain_wh(&mut self, locked: bool) {
        self.constrain_wh = locked;
    }

    pub fn clear_ruler_origin_preview(&mut self) {
        self.origin_preview = None;
    }

    pub fn toggle_guides_visibility(&mut self) {
        self.guides_hidden = !self.guides_hidden;
    }

    pub fn toggle_rulers_visibility(&mut self) {
        self.show_rulers = !self.show_rulers;
    }

    /// The CHECKED board-name edit the UI field calls: the typed text is edge-cleaned, then bounded.
    /// `Err(reason)` changes nothing (the field shows the reason and keeps focus); `Ok` applies it as one
    /// undo step through `EditCommand::SetBoardName` (no step when unchanged).
    pub fn try_set_board_name(&mut self, typed: &str) -> Result<(), board::Reject> {
        let name = board::clean_text(typed);
        board::check_name(&name)?;
        self.execute_ui(EditCommand::SetBoardName(name));
        Ok(())
    }

    /// The checked board-description edit (see [`Editor::try_set_board_name`]).
    pub fn try_set_board_description(&mut self, typed: &str) -> Result<(), board::Reject> {
        let text = board::clean_text(typed);
        board::check_description(&text)?;
        self.execute_ui(EditCommand::SetBoardDescription(text));
        Ok(())
    }

    /// The checked tag-list edit: typed tags are cleaned and deduplicated (`board::normalize_tags`,
    /// first spelling kept), THEN bounded, so 17 typed tags that clean down to 16 are accepted.
    pub fn try_set_board_tags(&mut self, typed: Vec<String>) -> Result<(), board::Reject> {
        let tags = board::normalize_tags(typed);
        board::check_tags(&tags)?;
        self.execute_ui(EditCommand::SetBoardTags(tags));
        Ok(())
    }
}

/// `EditCommand::SetBoard*` carry only valid values (built by the checked `try_set_board_*`): a
/// debug build stops on a violation; a release build ignores it rather than store an invalid value.
fn valid_replay(check: Result<(), board::Reject>) -> bool {
    debug_assert!(check.is_ok(), "SetBoard* carried an invalid value: {check:?}");
    check.is_ok()
}

/// A user-typed object name with its invisible edges removed: whitespace AND the zero-width
/// direction/format marks an Arabic keyboard or a paste can carry (LRM/RLM U+200E/F, ALM U+061C, the
/// embeddings/overrides U+202A–E, isolates U+2066–9, ZWSP U+200B, BOM U+FEFF). A name made only of
/// those comes back empty, so it can never become a blank-looking row. Marks INSIDE the name are kept.
pub fn clean_name(name: &str) -> &str {
    name.trim_matches(|c: char| {
        c.is_whitespace()
            || matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{061C}' | '\u{FEFF}')
    })
}

/// One board-metadata change as one undo step (the callers have already ruled out a no-op).
fn edit_board(ed: &mut Editor, change: impl FnOnce(&mut crate::model::Document)) {
    ed.begin();
    change(&mut ed.doc);
    ed.dirty = true;
    ed.commit();
}

/// `RenamePath`: only a real change becomes an edit. `Editor::rename_path` itself always opens an undo
/// step (and maps an empty name to the auto-name), so the no-op guards live here.
fn rename_path(ed: &mut Editor, path: u32, name: String) {
    let name = clean_name(&name);
    let Some(pi) = ed.doc.pidx(path) else { return };
    if name.is_empty() || ed.doc.paths[pi].name.as_deref() == Some(name) {
        return;
    }
    ed.rename_path(path, name.to_string());
}

/// The stroke-weight field: like a colour pick (`apply_paint`) and `bump_stroke`, the weight becomes the
/// CURRENT one (the next Pen/shape uses it — Illustrator's last-used appearance) AND lands on the same
/// target set the inspector displays (`selected_pids`: object selection ∪ Direct path ∪ a selected
/// anchor's path, which covers the Pen's in-progress path). Astra 09-24: it used to touch `cur_sw` only
/// when nothing was selected and to read `objsel` alone, so a width-80 stroke left new Pen paths at 2.
fn set_stroke_width(ed: &mut Editor, width: f32) {
    ed.cur_sw = width.max(0.0);
    let paths: Vec<u32> = ed.selected_pids().into_iter().collect();
    if paths.is_empty() {
        return;
    }
    ed.begin();
    for path in paths {
        if let Some(index) = ed.doc.pidx(path) {
            ed.doc.paths[index].stroke_width = width.max(0.0);
        }
    }
    ed.dirty = true;
    ed.commit();
}

/// Setup previews participate in an already-open scrub transaction.
fn edit_setup(ed: &mut Editor, change: impl FnOnce(&mut crate::model::Document)) {
    let own = !ed.transaction_open();
    if own {
        ed.begin();
    }
    change(&mut ed.doc);
    ed.dirty = true;
    if own {
        ed.commit();
    }
}
