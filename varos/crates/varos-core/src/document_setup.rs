//! Adapted from VectorCraft engine command conventions @ a469568 (MIT OR Apache-2.0).
//! Copyright (c) 2026 ArtCraft Team and the VectorCraft contributors.
//! ADAPT: VectorCraft engine cmd/docsetup.rs and docinfo.rs (a469568), field/report conventions.
//! Independent pure-core implementation; no reference UI or assets copied. See NOTICE.
use crate::model::{Artboard, Document, NodeKind};
use serde_json::{json, Value};

pub fn valid_ppi(ppi: f32) -> bool {
    ppi.is_finite() && (1.0..=9600.0).contains(&ppi)
}
pub fn valid_bleed(edges: [f32; 4]) -> bool {
    edges.iter().all(|v| v.is_finite() && (0.0..=7200.0).contains(v))
}
pub fn bleed(ab: &Artboard) -> [f32; 4] {
    ab.bleed_edges.unwrap_or([ab.bleed; 4])
}
/// Canvas-only guide rectangle, in points. Hidden/zero-bleed pages emit no guide.
pub fn bleed_rect(ab: &Artboard) -> Option<[f32; 4]> {
    let [top, right, bottom, left] = bleed(ab);
    (!ab.hidden && valid_bleed([top, right, bottom, left]) && [top, right, bottom, left].iter().any(|v| *v > 0.0))
        .then_some([ab.x - left, ab.y - top, ab.x + ab.w + right, ab.y + ab.h + bottom])
}
/// Shared with Bridge describe: one definition of object-kind counts.
pub fn counts(doc: &Document) -> Value {
    json!({"paths":doc.paths.len(),"groups":doc.nodes.iter().filter(|n|n.kind==NodeKind::Group).count(),"layers":doc.nodes.iter().filter(|n|n.kind==NodeKind::Layer).count(),"artboards":doc.artboards.len()})
}
pub fn info(doc: &Document) -> Value {
    let mut colours: Vec<[f32; 4]> = Vec::new();
    for c in doc
        .paths
        .iter()
        .flat_map(|p| [p.fill.solid(), p.stroke.solid()])
        .flatten()
        .chain(doc.artboards.iter().filter_map(|a| a.page_color))
    {
        if !colours.contains(&c) {
            colours.push(c);
        }
    }
    json!({"counts":counts(doc),"colours":colours,"artboards":doc.artboards.iter().map(|a|json!({"name":a.name,"size":[a.w,a.h],"ppi":doc.units.ppi})).collect::<Vec<_>>(),"links":"—","fonts":"—"})
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EditCommand, Editor, Unit};
    #[test]
    fn fields_roundtrip_and_undo_separately() {
        let mut ed = Editor::new();
        ed.execute(EditCommand::AddArtboard);
        let before = ed.doc.clone();
        ed.execute(EditCommand::SetUnits(Unit::Mm));
        ed.execute(EditCommand::SetPpi(300.0));
        assert_eq!(crate::units::to_pt(300.0, Unit::Px, ed.doc.units.ppi), 72.0);
        ed.execute(EditCommand::SetBleed { index: 0, edges: [1.0, 2.0, 3.0, 4.0] });
        ed.execute(EditCommand::SetTransparencyGrid(true));
        let back: Document = serde_json::from_slice(&serde_json::to_vec(&ed.doc).unwrap()).unwrap();
        assert_eq!(back, ed.doc);
        ed.execute(EditCommand::Undo);
        assert!(!ed.doc.transparency_grid);
        ed.execute(EditCommand::Undo);
        assert_eq!(bleed(&ed.doc.artboards[0]), [0.0; 4]);
        ed.execute(EditCommand::Undo);
        assert_eq!(ed.doc.units.ppi, 72.0);
        ed.execute(EditCommand::Undo);
        assert_eq!(ed.doc.units, before.units);
    }
    #[test]
    fn guide_geometry_and_info() {
        let ab = Artboard {
            x: 10.0,
            y: 20.0,
            w: 100.0,
            h: 200.0,
            bleed_edges: Some([1.0, 2.0, 3.0, 4.0]),
            ..Default::default()
        };
        assert_eq!(bleed_rect(&ab), Some([6.0, 19.0, 112.0, 223.0]));
        assert!(bleed_rect(&Artboard::default()).is_none());
        let mut doc = Document::default();
        doc.artboards.push(ab);
        assert_eq!(info(&doc)["counts"], counts(&doc));
        assert_eq!(info(&doc)["artboards"][0]["ppi"], 72.0);
        assert!(!valid_ppi(f32::NAN));
        assert!(!valid_bleed([-1.0; 4]));
    }
}

#[cfg(test)]
mod scene_tests {
    use super::*;
    #[test]
    fn checkerboard_is_bounded_canvas_furniture() {
        let mut ed = crate::Editor::new();
        ed.doc.artboards.push(Artboard { w: 100.0, h: 100.0, page_color: None, ..Default::default() });
        let plain = crate::build_scene(&ed, 1.0);
        ed.execute(crate::EditCommand::SetTransparencyGrid(true));
        let checker = crate::build_scene(&ed, 1.0);
        let count = |s: &crate::Scene| s.content.iter().map(|g| g.prims().len()).sum::<usize>();
        assert!(count(&checker) > count(&plain));
        assert!(count(&checker) < 17000);
        ed.doc.artboards[0].page_color = Some([0.0; 4]);
        assert_eq!(count(&crate::build_scene(&ed, 1.0)), count(&plain));
    }
}
