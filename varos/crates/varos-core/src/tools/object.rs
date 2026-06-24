use super::Tool;
use crate::editor::{Drag, Editor};
use crate::geom::Pt;

pub struct Object;
impl Tool for Object {
    fn down(&self, ed: &mut Editor, pos: Pt) {
        if let Some(hit) = ed.transform_hit(pos) { ed.start_transform(hit, pos); return; } // on a handle → transform (keep frame angle)
        if let Some(pid) = ed.path_under(pos) {
            if ed.mods.alt {
                ed.obj_angle = 0.0;
                // duplicate the whole selection if the clicked object is part of a multi-selection
                let srcs: Vec<u32> = if ed.objsel.contains(&pid) && ed.objsel.len() > 1 { ed.objsel.iter().copied().collect() } else { vec![pid] };
                ed.drag = Drag::DupPending { srcs, down: pos, object: true };
                return;
            }
            if ed.mods.shift {
                if ed.objsel.contains(&pid) { ed.objsel.remove(&pid); } else { ed.objsel.insert(pid); } // shift toggles membership
                ed.obj_angle = 0.0;                                          // selection set changed → axis-align
            } else if !ed.objsel.contains(&pid) {
                ed.objsel.clear(); ed.objsel.insert(pid); ed.obj_angle = 0.0; // fresh selection
            } // else: re-clicking a selected object → keep selection + frame angle (about to move)
            let mut base = vec![];
            for &p in &ed.objsel { if let Some(pi) = ed.doc.pidx(p) { for a in ed.doc.paths[pi].anchors.iter().chain(ed.doc.paths[pi].holes.iter().flatten()) { base.push((a.id, a.p, a.hin, a.hout)); } } }
            ed.drag = Drag::Object { down: pos, base };
            return;
        }
        // empty space → marquee-select objects (Shift keeps the current selection)
        let base: Vec<u32> = if ed.mods.shift { ed.objsel.iter().copied().collect() } else { ed.objsel.clear(); Vec::new() };
        ed.obj_angle = 0.0;
        ed.drag = Drag::ObjMarquee { start: pos, base };
    }
}
