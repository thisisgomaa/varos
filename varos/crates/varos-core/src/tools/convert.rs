use super::Tool;
use crate::editor::{Drag, Editor, ANCHOR_R, EDGE_R};
use crate::geom::{sub, Pt};

pub struct Convert;
impl Tool for Convert {
    fn down(&self, ed: &mut Editor, pos: Pt) {
        // handle -> break it
        if let Some(aid) = ed.handle_hit(pos) {
            // A7: the Convert tool edits anchors/handles in WORLD (`hp`/`pos` mix local + world). A rotated
            // unit must be baked to identity FIRST, else the raw world write into local storage double-
            // transforms at render (mirror `tools/direct.rs`).
            if let Some(pid) = ed.doc.pid_of_anchor(aid) {
                ed.dirty |= ed.bake_unit_of(pid);
            }
            let (pi, ai) = ed.doc.aidx(aid).unwrap();
            ed.doc.paths[pi].anchors[ai].smooth = false;
            ed.selected.insert(aid);
            let out = ed.which_handle(aid, pos);
            let hp = if out { ed.doc.paths[pi].anchors[ai].hout } else { ed.doc.paths[pi].anchors[ai].hin }.unwrap();
            ed.dirty = true;
            ed.drag = Drag::Handle { aid, out, couple: false, opp_len: 0.0, grab: sub(hp, pos) };
            return;
        }
        // anchor: smooth -> corner (click); corner -> drag to pull handles
        if let Some(aid) = ed.nearest_anchor(pos, ANCHOR_R, true) {
            // A7: `Drag::ConvPull` writes the raw WORLD cursor into `a.hout`/`a.hin` (local storage) → bake
            // the rotated unit first so local == world for the whole gesture.
            if let Some(pid) = ed.doc.pid_of_anchor(aid) {
                ed.dirty |= ed.bake_unit_of(pid);
            }
            let (pi, ai) = ed.doc.aidx(aid).unwrap();
            ed.selected.clear();
            ed.selected.insert(aid);
            if ed.doc.paths[pi].anchors[ai].smooth {
                ed.toggle_type(aid);
                ed.dirty = true;
            } else {
                ed.drag = Drag::ConvPull { aid, down: pos };
            }
            return;
        }
        // segment -> reshape
        if let Some(pid) = ed.path_under(pos) {
            // A7: `nearest_seg`/`start_segment` operate in LOCAL coords — bake the rotated unit so the
            // segment index + reshape are in world (consistent with the other Convert branches).
            ed.dirty |= ed.bake_unit_of(pid);
            if let Some(pi) = ed.doc.pidx(pid) {
                if let Some((i, _, d)) = ed.doc.nearest_seg(pi, pos) {
                    if d <= EDGE_R {
                        ed.start_segment(pid, i, pos);
                    }
                }
            }
        }
    }
}
