//! Revision/selection cache for stroke inspection; transient and excluded from document history.
use super::StrokeStyle;
use crate::editor::Editor;
use std::{cell::RefCell, collections::HashSet};
#[derive(Clone, Default)]
pub struct Inspection {
    pub style: StrokeStyle,
    pub mixed: bool,
    pub fields: [bool; 6],
    pub open: bool,
    pub closed: bool,
    pub no_tangent: bool,
}
#[derive(Clone, Default)]
pub struct InspectionCache(RefCell<Option<Entry>>);
#[derive(Clone)]
struct Entry {
    rev: u64,
    objects: HashSet<u32>,
    anchors: HashSet<u32>,
    direct: Option<u32>,
    value: Inspection,
}
impl InspectionCache {
    pub fn read(&self, ed: &Editor) -> Inspection {
        if !ed.transaction_open() && !ed.dirty {
            if let Some(entry) = self.0.borrow().as_ref() {
                if entry.rev == ed.rev
                    && entry.objects == ed.objsel
                    && entry.anchors == ed.selected
                    && entry.direct == ed.dsel_path
                {
                    return entry.value.clone();
                }
            }
        }
        let style = ed.repr_path().map(|i| ed.doc.paths[i].stroke_style.clone()).unwrap_or_default();
        let mut value = Inspection { style, ..Default::default() };
        for id in ed.selected_pids() {
            if let Some(i) = ed.doc.pidx(id) {
                let p = &ed.doc.paths[i];
                let a = &value.style;
                let b = &p.stroke_style;
                value.mixed |= a != b;
                for (mixed, same) in value.fields.iter_mut().zip([
                    a.cap == b.cap,
                    a.join == b.join,
                    a.align == b.align,
                    a.arrows.start == b.arrows.start,
                    a.arrows.end == b.arrows.end,
                    a.arrows.align == b.arrows.align,
                ]) {
                    *mixed |= !same;
                }
                value.open |= !p.closed;
                value.closed |= p.closed;
                // A tangent exists if any authored control/anchor differs from the first point.
                // This avoids curve arc-length integration for a boolean degeneracy check.
                value.no_tangent |= !p.closed && !super::evaluate::has_length(p);
            }
        }
        *self.0.borrow_mut() = if ed.transaction_open() || ed.dirty {
            None
        } else {
            Some(Entry {
                rev: ed.rev,
                objects: ed.objsel.clone(),
                anchors: ed.selected.clone(),
                direct: ed.dsel_path,
                value: value.clone(),
            })
        };
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_inspection_reuses_entry_and_invalidates_on_revision_selection_and_live_edit() {
        let mut ed = Editor::new();
        let doc = crate::format::decode_model(
            include_bytes!("../../tests/fixtures/v5/cap_Butt.json"),
            None,
            &crate::format::Limits::DEFAULT,
        )
        .unwrap()
        .doc;
        ed.replace_doc(doc);
        ed.objsel.insert(10);
        assert!(ed.stroke_inspection.read(&ed).open);
        // A sentinel in the cached entry proves the next idle read did not recompute geometry.
        ed.stroke_inspection.0.borrow_mut().as_mut().unwrap().value.no_tangent = true;
        assert!(ed.stroke_inspection.read(&ed).no_tangent);
        ed.rev += 1;
        assert!(!ed.stroke_inspection.read(&ed).no_tangent);
        ed.objsel.clear();
        assert!(!ed.stroke_inspection.read(&ed).open);
        ed.objsel.insert(10);
        ed.begin();
        let point = ed.doc.paths[0].anchors[0].p;
        for anchor in &mut ed.doc.paths[0].anchors {
            anchor.p = point;
        }
        ed.dirty = true;
        assert!(ed.stroke_inspection.read(&ed).no_tangent);
        assert!(ed.stroke_inspection.0.borrow().is_none());
        ed.commit();
        assert!(ed.stroke_inspection.read(&ed).no_tangent);
        ed.undo();
        assert!(!ed.stroke_inspection.read(&ed).no_tangent);
    }
}
