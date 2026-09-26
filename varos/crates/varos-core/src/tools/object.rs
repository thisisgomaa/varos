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
            let group = ed.doc.top_group_of_path(pid);
            if ed.mods.alt {
                // duplicate the whole selection if the clicked group is part of a multi-selection
                let in_sel = members.iter().any(|m| ed.objsel.contains(m));
                let srcs: Vec<u32> = if in_sel && ed.objsel.len() > 1 {
                    ed.structural_object_paths()
                } else {
                    members.into_iter().filter(|&member| !ed.doc.eff_locked(member)).collect()
                };
                ed.drag = Drag::DupPending { srcs, down: pos, object: true };
                return;
            }
            if ed.mods.shift {
                let all = members.iter().all(|m| ed.objsel.contains(m)); // shift toggles the whole group
                if all {
                    for m in &members {
                        ed.objsel.remove(m);
                    }
                    if let Some(group) = group {
                        ed.group_sel.remove(&group);
                    }
                } else {
                    for &m in &members {
                        ed.objsel.insert(m);
                    }
                    if let Some(group) = group {
                        ed.group_sel.insert(group);
                    }
                }
                ed.refresh_obj_angle(); // selection set changed → single unit shows θ, multi axis-aligns
            } else if !members.iter().any(|m| ed.objsel.contains(m)) {
                ed.objsel.clear();
                ed.group_sel.clear();
                for &m in &members {
                    ed.objsel.insert(m);
                }
                if let Some(group) = group {
                    ed.group_sel.insert(group);
                }
                ed.refresh_obj_angle(); // fresh selection → restore the stored rotation of that unit (A7)
            } // else: re-clicking the selected group → keep selection + frame angle (about to move)
            if let Some(group) = group.filter(|_| members.iter().all(|member| ed.objsel.contains(member))) {
                ed.group_sel.insert(group);
            }
            let (base, base_world, piv_base) = ed.object_move_base();
            ed.drag = Drag::Object { down: pos, base, base_world, piv_base };
            return;
        }
        // empty space → marquee-select objects (Shift keeps the current selection)
        let (base, base_groups): (Vec<u32>, Vec<u32>) = if ed.mods.shift {
            (ed.objsel.iter().copied().collect(), ed.group_sel.iter().copied().collect())
        } else {
            ed.objsel.clear();
            ed.group_sel.clear();
            (Vec::new(), Vec::new())
        };
        ed.refresh_obj_angle();
        ed.drag = Drag::ObjMarquee { start: pos, base, base_groups };
    }
}
