use super::Tool;
use crate::editor::{Drag, Editor};
use crate::geom::Pt;
use crate::model::Path;

pub struct Shapes;
impl Tool for Shapes {
    fn down(&self, ed: &mut Editor, pos: Pt) {
        ed.selected.clear();
        ed.objsel.clear();
        let kind = ed.gesture.shape();
        let id = ed.doc.nid();
        let (f, st, sw) = (ed.cur_fill, ed.cur_stroke, ed.cur_sw);
        let anchors = ed.doc.build_shape(kind, pos, pos);
        ed.doc.paths.push(Path::new(id, anchors, true, f, st, sw));
        ed.dirty = true;
        ed.drag = Drag::Shape { start: pos, pid: id, kind };
    }
}
