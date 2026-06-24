use super::Tool;
use crate::editor::{Drag, Editor, ANCHOR_R, EDGE_R};
use crate::geom::Pt;
use crate::model::{Anchor, Path};

pub struct Pen;
impl Tool for Pen {
    fn down(&self, ed: &mut Editor, pos: Pt) {
        // on an existing anchor?
        if let Some(aid) = ed.nearest_anchor(pos, ANCHOR_R, true) {
            let (pi, ai) = ed.doc.aidx(aid).unwrap();
            let pid = ed.doc.paths[pi].id;
            let n = ed.doc.paths[pi].anchors.len();
            let is_end = !ed.doc.paths[pi].closed && (ai == 0 || ai == n - 1);
            let tip = ed.active.and_then(|ap| ed.doc.pidx(ap)).and_then(|i| ed.doc.paths[i].anchors.last().map(|a| a.id));
            if is_end {
                if let Some(act) = ed.active {
                    if act == pid {
                        if Some(aid) != tip {
                            if let Some(i) = ed.doc.pidx(pid) { ed.doc.paths[i].closed = true; }
                            ed.dirty = true;
                            ed.drag = Drag::PenClose { aid, down: pos, broken: false };
                        }
                        return;
                    } else { ed.join(act, pid, aid, pos); ed.dirty = true; return; }
                } else { ed.resume(pid, aid); return; }
            }
            // middle anchor -> delete (only if the path is selected/active)
            if ed.is_editable(pid) { ed.delete_anchor(aid); ed.dirty = true; }
            return;
        }
        // on a segment & editable -> add anchor
        if let Some(pid) = ed.path_under(pos) {
            if ed.is_editable(pid) {
                if let Some(pi) = ed.doc.pidx(pid) {
                    if let Some((i, t, d)) = ed.doc.nearest_seg(pi, pos) {
                        if d <= EDGE_R { let nid = ed.add_anchor(pi, i, t); ed.selected.insert(nid); ed.dirty = true; return; }
                    }
                }
            }
        }
        // else: extend the active path, or start a new one
        let pid = match ed.active {
            Some(i) => i,
            None => { let id = ed.doc.nid(); let (f, st, sw) = (ed.cur_fill, ed.cur_stroke, ed.cur_sw); ed.doc.paths.push(Path { id, anchors: vec![], closed: false, fill: f, stroke: st, stroke_width: sw, holes: vec![] }); ed.active = Some(id); ed.selected.clear(); id }
        };
        let aid = ed.doc.nid();
        let pi = ed.doc.pidx(pid).unwrap();
        ed.doc.paths[pi].anchors.push(Anchor { id: aid, p: pos, hin: None, hout: None, smooth: false });
        ed.selected.insert(aid);
        ed.dirty = true;
        ed.drag = Drag::PenNew { aid, down: pos, broken: false };
    }
}
