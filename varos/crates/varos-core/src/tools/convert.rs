use super::Tool;
use crate::editor::{Drag, Editor, ANCHOR_R, EDGE_R};
use crate::geom::{sub, Pt};

pub struct Convert;
impl Tool for Convert {
    fn down(&self, ed: &mut Editor, pos: Pt) {
        // handle -> break it
        if let Some(aid) = ed.handle_hit(pos) {
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
            let (pi, ai) = ed.doc.aidx(aid).unwrap();
            ed.selected.clear(); ed.selected.insert(aid);
            if ed.doc.paths[pi].anchors[ai].smooth { ed.toggle_type(aid); ed.dirty = true; }
            else { ed.drag = Drag::ConvPull { aid, down: pos }; }
            return;
        }
        // segment -> reshape
        if let Some(pid) = ed.path_under(pos) {
            if let Some(pi) = ed.doc.pidx(pid) {
                if let Some((i, _, d)) = ed.doc.nearest_seg(pi, pos) { if d <= EDGE_R { ed.start_segment(pid, i, pos); } }
            }
        }
    }
}
