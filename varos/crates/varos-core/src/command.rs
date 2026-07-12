//! Internal edit-command boundary.
//!
//! This closed enum is application plumbing, not a stable plugin or AI protocol. Adapters,
//! versioning, permissions, compatibility, and query contracts require separate decisions.

use crate::boolean::BoolOp;
use crate::editor::{AlignMode, AlignTarget, DistAxis, Editor, PaintTarget, ZOrder};
use crate::geom::{Pt, Rgba};
use crate::model::{DropPos, SnapConfig};

/// A deterministic edit or history action executed entirely inside `varos-core`.
pub enum EditCommand {
    SetObjectBounds {
        x: Option<f32>,
        y: Option<f32>,
        width: Option<f32>,
        height: Option<f32>,
        anchor_x: f32,
        anchor_y: f32,
    },
    SetObjectRotation(f32),
    SetOpacity(f32),
    SetStrokeWidth(f32),
    SetClipExempt(bool),
    ApplyPaint {
        target: PaintTarget,
        color: Option<Rgba>,
    },
    SwapColors,
    DefaultPaint,
    PickerBegin,
    PickerLivePaint {
        target: PaintTarget,
        color: Rgba,
    },
    PickerLiveArtboard {
        index: usize,
        color: Rgba,
    },
    PickerCommit {
        current: Option<PaintTarget>,
        color: Rgba,
    },
    PickerCancel,
    ToggleNodeHidden(u32),
    ToggleNodeLocked(u32),
    RenameNode {
        node: u32,
        name: String,
    },
    GroupSelection,
    UngroupSelection,
    DeleteLayerSelection,
    MoveLayer {
        sources: Vec<u32>,
        target: u32,
        position: DropPos,
    },
    DuplicateMoveLayer {
        sources: Vec<u32>,
        target: u32,
        position: DropPos,
    },
    MoveLayerToBoard {
        sources: Vec<u32>,
        source_board: Option<usize>,
        target_board: usize,
    },
    Flip(bool),
    Align {
        mode: AlignMode,
        target: AlignTarget,
    },
    Distribute(DistAxis),
    Boolean(BoolOp),
    Arrange(ZOrder),
    TransformAgain,
    DeleteSelected,
    Nudge {
        x: f32,
        y: f32,
    },
    SetActiveArtboard(usize),
    SetArtboardRect {
        index: usize,
        x: Option<f32>,
        y: Option<f32>,
        width: Option<f32>,
        height: Option<f32>,
    },
    RenameArtboard {
        index: usize,
        name: String,
    },
    SetArtboardColor {
        index: usize,
        color: Option<Rgba>,
    },
    ToggleArtboardClip(usize),
    ToggleArtboardHidden(usize),
    ToggleArtboardLocked(usize),
    OrientArtboard(usize),
    AddArtboard,
    DuplicateArtboard(usize),
    DeleteArtboard(usize),
    SetArtboardCount(usize),
    SetMoveArtWithArtboard(bool),
    SetRulerOrigin(Pt),
    CommitGuide,
    CycleUnits,
    SetSnapConfig(SnapConfig),
    ToggleSnapping,
    ToggleGuidesLocked,
    ToggleSmartGuides,
    Undo,
    Redo,
}

impl EditCommand {
    fn apply(self, ed: &mut Editor) {
        match self {
            Self::SetObjectBounds { x, y, width, height, anchor_x, anchor_y } => {
                ed.set_obj_bbox(x, y, width, height, anchor_x, anchor_y)
            }
            Self::SetObjectRotation(degrees) => ed.set_obj_rotation(degrees),
            Self::SetOpacity(opacity) => ed.set_opacity(opacity),
            Self::SetStrokeWidth(width) => set_stroke_width(ed, width),
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
            Self::GroupSelection => ed.group_selection(),
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
            Self::Arrange(order) => ed.arrange(order),
            Self::TransformAgain => ed.transform_again(),
            Self::DeleteSelected => ed.delete_selected(),
            Self::Nudge { x, y } => ed.nudge(x, y),
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
            Self::SetSnapConfig(config) => ed.doc.snap = config,
            Self::ToggleSnapping => ed.doc.snap.enabled = !ed.doc.snap.enabled,
            Self::ToggleGuidesLocked => ed.doc.guides_locked = !ed.doc.guides_locked,
            Self::ToggleSmartGuides => ed.doc.snap.smart = !ed.doc.snap.smart,
            Self::Undo => ed.undo(),
            Self::Redo => ed.redo(),
        }
    }
}

impl Editor {
    /// Execute one deterministic edit through the core-owned command boundary.
    pub fn execute(&mut self, command: EditCommand) {
        command.apply(self);
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
}

fn set_stroke_width(ed: &mut Editor, width: f32) {
    let paths: Vec<u32> = ed.objsel.iter().copied().collect();
    if paths.is_empty() {
        ed.cur_sw = width.max(0.0);
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
