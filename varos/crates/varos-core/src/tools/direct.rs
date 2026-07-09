use super::Tool;
use crate::editor::{Drag, Editor, ANCHOR_R, EDGE_R};
use crate::geom::{dist, sub, Pt};

pub struct Direct;
impl Tool for Direct {
    fn down(&self, ed: &mut Editor, pos: Pt) {
        // handle FIRST — so Alt over a handle BREAKS it (must beat the Alt-duplicate below)
        if let Some(aid) = ed.handle_hit(pos) {
            // A7: reshaping a rotated unit bakes its rotation into geometry first, then edits in world
            // (the simplest consistent Direct-Selection behaviour). `which_handle`/`anchor` read world.
            if let Some(pid) = ed.doc.pid_of_anchor(aid) {
                ed.dirty |= ed.bake_unit_of(pid);
            }
            let out = ed.which_handle(aid, pos);
            let a = ed.doc.anchor(aid).unwrap().clone();
            let hp = if out { a.hout } else { a.hin }.unwrap();
            let couple = !ed.mods.alt && a.hin.is_some() && a.hout.is_some() && {
                let vi = sub(a.hin.unwrap(), a.p);
                let vo = sub(a.hout.unwrap(), a.p);
                let mut d = (vi[1].atan2(vi[0]) - vo[1].atan2(vo[0])).abs();
                if d > std::f32::consts::PI {
                    d = 2.0 * std::f32::consts::PI - d;
                }
                d > std::f32::consts::PI - 0.15
            };
            let opp = if out { a.hin } else { a.hout };
            let opp_len = opp.map_or(0.0, |o| dist(a.p, o));
            if ed.mods.alt {
                if let Some(a) = ed.doc.anchor_mut(aid) {
                    a.smooth = false;
                }
            }
            ed.selected.insert(aid);
            ed.dirty = true;
            ed.drag = Drag::Handle { aid, out, couple, opp_len, grab: sub(hp, pos) };
            return;
        }
        // Alt + anchor/path => duplicate (only once a real drag starts)
        if ed.mods.alt {
            if let Some(aid) = ed.nearest_anchor(pos, ANCHOR_R, false) {
                let (pi, _) = ed.doc.aidx(aid).unwrap();
                let pid = ed.doc.paths[pi].id;
                if !ed.mods.shift {
                    ed.selected.clear();
                }
                ed.selected.insert(aid);
                ed.drag = Drag::DupPending { srcs: vec![pid], down: pos, object: false };
                return;
            }
            if let Some(pid) = ed.path_under(pos) {
                ed.drag = Drag::DupPending { srcs: vec![pid], down: pos, object: false };
                return;
            }
        }
        // an anchor — the white arrow grabs ANY anchor directly (Illustrator), even on an unselected path.
        // Grabbing an already-selected anchor moves the whole selection; an unselected one selects just it.
        if let Some(aid) = ed.nearest_anchor(pos, ANCHOR_R, false) {
            if let Some(pid) = ed.doc.pid_of_anchor(aid) {
                ed.dirty |= ed.bake_unit_of(pid); // A7: bake before editing anchors of a rotated unit
            }
            ed.dsel_path = None;
            if ed.mods.shift {
                if ed.selected.contains(&aid) {
                    ed.selected.remove(&aid);
                } else {
                    ed.selected.insert(aid);
                }
            } else if !ed.selected.contains(&aid) {
                ed.selected.clear();
                ed.selected.insert(aid);
            }
            ed.begin_anchor_drag(pos);
            return;
        }
        // the path body: edge = reshape that segment; fill = PATH-LEVEL select (anchors hollow) + whole-path move
        if let Some(pid) = ed.path_under(pos) {
            // A7: bake the unit BEFORE `nearest_seg`/item capture — those work in local coords, so a rotated
            // unit must first become identity for the segment index + whole-path move to be in world.
            ed.dirty |= ed.bake_unit_of(pid);
            if let Some(pi) = ed.doc.pidx(pid) {
                if let Some((i, _, d)) = ed.doc.nearest_seg(pi, pos) {
                    if d <= EDGE_R / ed.ppu {
                        ed.start_segment(pid, i, pos);
                        return;
                    }
                }
            }
            ed.selected.clear();
            ed.dsel_path = Some(pid);
            // whole-path move: ALL anchors of the compound path (outer + holes), so holes move too
            let items = if let Some(pi) = ed.doc.pidx(pid) {
                ed.doc.paths[pi]
                    .anchors
                    .iter()
                    .chain(ed.doc.paths[pi].holes.iter().flatten())
                    .map(|a| (a.id, a.p, a.hin, a.hout))
                    .collect()
            } else {
                vec![]
            };
            ed.drag = Drag::Anchors { start: pos, items };
            return;
        }
        // empty -> marquee
        if !ed.mods.shift {
            ed.selected.clear();
        }
        ed.dsel_path = None;
        let base: Vec<u32> = ed.selected.iter().copied().collect();
        ed.drag = Drag::Marquee { start: pos, base };
    }
}
