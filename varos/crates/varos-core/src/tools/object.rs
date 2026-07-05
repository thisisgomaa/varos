use super::Tool;
use crate::editor::{Drag, Editor};
use crate::geom::Pt;

pub struct Object;
impl Tool for Object {
    fn down(&self, ed: &mut Editor, pos: Pt) {
        if let Some(hit) = ed.transform_hit(pos) {
            ed.start_transform(hit, pos);
            return;
        } // on a handle → transform (keep frame angle)
        if let Some(pid) = ed.path_under(pos) {
            let members = ed.doc.group_members(pid); // a grouped object selects/moves as a whole unit
            if ed.mods.alt {
                ed.obj_angle = 0.0;
                // duplicate the whole selection if the clicked group is part of a multi-selection
                let in_sel = members.iter().any(|m| ed.objsel.contains(m));
                let srcs: Vec<u32> =
                    if in_sel && ed.objsel.len() > 1 { ed.objsel.iter().copied().collect() } else { members };
                ed.drag = Drag::DupPending { srcs, down: pos, object: true };
                return;
            }
            if ed.mods.shift {
                let all = members.iter().all(|m| ed.objsel.contains(m)); // shift toggles the whole group
                if all {
                    for m in &members {
                        ed.objsel.remove(m);
                    }
                } else {
                    for m in members {
                        ed.objsel.insert(m);
                    }
                }
                ed.obj_angle = 0.0; // selection set changed → axis-align
            } else if !members.iter().any(|m| ed.objsel.contains(m)) {
                ed.objsel.clear();
                for m in members {
                    ed.objsel.insert(m);
                }
                ed.obj_angle = 0.0; // fresh selection
            } // else: re-clicking the selected group → keep selection + frame angle (about to move)
            let mut base = vec![];
            for &p in &ed.objsel {
                if let Some(pi) = ed.doc.pidx(p) {
                    for a in ed.doc.paths[pi].anchors.iter().chain(ed.doc.paths[pi].holes.iter().flatten()) {
                        base.push((a.id, a.p, a.hin, a.hout));
                    }
                }
            }
            ed.drag = Drag::Object { down: pos, base };
            return;
        }
        // empty space → marquee-select objects (Shift keeps the current selection)
        let base: Vec<u32> = if ed.mods.shift {
            ed.objsel.iter().copied().collect()
        } else {
            ed.objsel.clear();
            Vec::new()
        };
        ed.obj_angle = 0.0;
        ed.drag = Drag::ObjMarquee { start: pos, base };
    }
}
