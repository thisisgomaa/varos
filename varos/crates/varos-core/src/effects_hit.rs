//! Hit authored objects through independently painted evaluated copies.
use crate::{geom::point_in_poly, model::Document, Editor, Pt};
pub fn contains(ed: &Editor, id: u32, pos: Pt, reach: f32) -> bool {
    let Some(i) = ed.doc.pidx(id) else { return false };
    let Ok(parts) = crate::effects_document::resolved_many(&ed.doc.paths[i]) else { return false };
    parts.iter().any(|p| {
        let mut fill = p.anchors.len() >= 3 && point_in_poly(&Document::ring_px(&p.anchors, true, ed.ppu), pos);
        for h in &p.holes {
            if h.len() >= 3 && point_in_poly(&Document::ring_px(h, true, ed.ppu), pos) {
                fill = !fill;
            }
        }
        (fill && p.appearance().fill().resolved_ref(&ed.doc).is_painted())
            || crate::stroke::evaluate(p, 0.25 / f64::from(ed.ppu.max(0.0001)), &|| false)
                .is_ok_and(|c| crate::stroke::evaluate::contains(&c.rings, pos, reach))
    })
}
