//! Slice 4A routing; mutations use EditCommand.
// Adapted from VectorCraft tools/src/xform/transform.rs, free.rs, wand.rs@a469568
// (MIT OR Apache-2.0), ArtCraft Team 2026.
use crate::{
    editor::{Editor, TfHit, ToolKind},
    geom::Pt,
    select_transform::{SelectMode, Transform},
    EditCommand,
};
pub fn down(ed: &mut Editor, pos: Pt) -> bool {
    if ed.gesture == ToolKind::MagicWand {
        if let Some(source) = ed.path_under(pos) {
            let mode = if ed.mods.alt {
                SelectMode::Subtract
            } else if ed.mods.shift {
                SelectMode::Add
            } else {
                SelectMode::Set
            };
            ed.execute_ui(EditCommand::MagicWand { source, options: ed.select_transform.wand, mode });
        } else if !ed.mods.alt && !ed.mods.shift {
            ed.escape_selection();
        }
        return true;
    }
    let shear_handle = ed.gesture == ToolKind::FreeTransform
        && ed.mods.ctrl
        && matches!(ed.transform_hit(pos), Some(TfHit::Scale(4..=7)));
    if !matches!(ed.gesture, ToolKind::Reflect | ToolKind::Shear) && !shear_handle {
        return false;
    }
    if ed.objsel.is_empty() {
        if let Some(p) = ed.path_under(pos) {
            ed.objsel.insert(p);
        }
    }
    if ed.objsel.is_empty() {
        return true;
    }
    ed.select_transform.free_shear = None;
    if shear_handle {
        if let (Some(TfHit::Scale(h)), Some(handles)) = (ed.transform_hit(pos), ed.frame_handles()) {
            let origin = handles[4 + ((usize::from(h) - 4 + 2) % 4)];
            let axis = ed.obj_angle.to_degrees() + if h == 4 || h == 6 { 0. } else { 90. };
            let (sin, cos) = axis.to_radians().sin_cos();
            let lever = -(pos[0] - origin[0]) * sin + (pos[1] - origin[1]) * cos;
            ed.select_transform.free_shear = Some((origin, axis, lever));
        }
    }
    ed.select_transform.down = Some((pos, ed.mods.alt));
    let mut ids: Vec<_> = ed.selected_pids().into_iter().collect();
    ids.sort_unstable();
    ed.select_transform.preview = Some((ed.doc.clone(), ids));
    true
}
pub fn movement(ed: &mut Editor, pos: Pt) -> bool {
    let Some((down, _)) = ed.select_transform.down else { return false };
    if crate::geom::dist(down, pos) * ed.ppu < 3. {
        return true;
    }
    let mut s = Transform { copy: ed.mods.alt, ..Default::default() };
    if ed.gesture == ToolKind::Reflect {
        let mut angle = (pos[1] - down[1]).atan2(pos[0] - down[0]).to_degrees();
        if ed.mods.shift {
            angle = (angle / 45.).round() * 45.;
        }
        s.reflect = Some(angle);
        s.origin = Some(down);
    } else if let Some((origin, axis, lever)) = ed.select_transform.free_shear {
        let (sin, cos) = axis.to_radians().sin_cos();
        let delta = (pos[0] - down[0]) * cos + (pos[1] - down[1]) * sin;
        if lever.abs() < 1.0e-4 {
            return true;
        }
        s.shear = (delta / lever).atan().to_degrees();
        s.shear_axis = axis;
        s.origin = Some(origin);
    } else {
        let b = ed
            .select_transform
            .preview
            .as_ref()
            .map(|(doc, ids)| {
                ids.iter()
                    .filter_map(|p| doc.pidx(*p))
                    .map(|i| doc.outline_bbox(i))
                    .fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |b, q| {
                        (b.0.min(q.0), b.1.min(q.1), b.2.max(q.2), b.3.max(q.3))
                    })
            })
            .unwrap_or((0., 0., 1., 1.));
        let horizontal = (pos[0] - down[0]).abs() >= (pos[1] - down[1]).abs();
        let origin = ed.pivot.unwrap_or([(b.0 + b.2) * 0.5, (b.1 + b.3) * 0.5]);
        let raw = if horizontal { down[1] - origin[1] } else { origin[0] - down[0] };
        let lever = if raw.abs() > 1.0e-3 {
            raw
        } else if horizontal {
            (b.3 - b.1) * 0.5
        } else {
            (b.2 - b.0) * 0.5
        };
        let d = if horizontal { pos[0] - down[0] } else { pos[1] - down[1] };
        s.shear = (d / if lever.abs() < 1. { 1. } else { lever }).atan().to_degrees();
        s.shear_axis = if horizontal { 0. } else { 90. };
        s.origin = Some(origin);
    }
    ed.execute_ui(EditCommand::TransformLive(s));
    true
}
pub fn up(ed: &mut Editor) -> bool {
    if let Some((down, alt)) = ed.select_transform.down.take() {
        ed.select_transform.free_shear = None;
        let click = crate::geom::dist(down, ed.cursor) * ed.ppu < 3.;
        if click {
            ed.execute_ui(EditCommand::TransformCancel);
            ed.pivot = Some(down);
            if alt {
                ed.select_transform.dialog = Some(ed.tool);
            }
        } else {
            ed.execute_ui(EditCommand::TransformCommit);
        }
        return true;
    }
    if ed.mods.alt && matches!(ed.drag, crate::editor::Drag::TfPending { .. }) {
        ed.select_transform.dialog = Some(ed.tool);
    }
    false
}
