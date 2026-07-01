//! The transform tools — Rotate (R), Scale (S), Reflect (O) — share one press handler and a movable
//! pivot. A plain CLICK relocates the pivot; a DRAG transforms the selection around it (Alt = a copy,
//! Shift = constrain). Which transform runs is decided by the active tool in the `TfPending` move arm.
//! Mirrors Illustrator's Rotate/Scale/Reflect tools.

use super::Tool;
use crate::editor::{Drag, Editor};
use crate::geom::Pt;

pub struct Transform;
impl Tool for Transform {
    fn down(&self, ed: &mut Editor, pos: Pt) {
        // the tool acts on a selection — with none, grab the object under the cursor (else nothing to do)
        if ed.objsel.is_empty() {
            match ed.path_under(pos) {
                Some(pid) => { for m in ed.doc.group_members(pid) { ed.objsel.insert(m); } ed.objsel.insert(pid); }
                None => return,
            }
        }
        let pivot = ed.pivot_point().unwrap_or(pos);
        ed.drag = Drag::TfPending { pivot, down: pos };
    }
}
