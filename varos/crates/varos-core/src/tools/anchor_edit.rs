//! Explicit Add/Delete Anchor tools use the same deterministic commands as headless callers.
use super::Tool;
use crate::command::EditCommand;
use crate::editor::{Drag, Editor, ANCHOR_R, EDGE_R};
use crate::geom::Pt;
pub struct Add;
pub struct Delete;
pub struct Lasso;
impl Tool for Add {
    fn down(&self, ed: &mut Editor, pos: Pt) {
        let Some(pid) = ed.path_under(pos) else { return };
        let Some(pi) = ed.doc.pidx(pid) else { return };
        let local = ed.doc.unit_xform(pid).inverse_apply(pos);
        if let Some((segment, t, distance)) = ed.doc.nearest_seg(pi, local) {
            if distance <= EDGE_R / ed.ppu {
                let _ = ed.try_execute(EditCommand::InsertAnchor { path: pid, segment, t });
            }
        }
    }
}
impl Tool for Delete {
    fn down(&self, ed: &mut Editor, pos: Pt) {
        if let Some(id) = ed.nearest_anchor(pos, ANCHOR_R, false) {
            let _ = ed.try_execute(EditCommand::DeleteAnchor(id));
        }
    }
}
impl Tool for Lasso {
    fn down(&self, ed: &mut Editor, pos: Pt) {
        ed.drag = Drag::Lasso { points: vec![pos], objects: !ed.objsel.is_empty(), additive: ed.mods.shift };
    }
}
