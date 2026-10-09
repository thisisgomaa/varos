//! Lane H: font package inventory with explicit redistribution decisions.
use std::{collections::BTreeSet, path::Path};
use varos_core::model::Document;
use varos_text_layout::{bundled_fonts, font_hash, TextLayout};
pub(crate) fn collect(doc: &Document, stage: &Path) -> Result<String, String> {
    if doc.text_boxes.is_empty() {
        return Ok("Fonts: none\n".into());
    }
    let fonts = bundled_fonts()?;
    let mut engine = TextLayout::new(fonts.clone())?;
    let mut used = BTreeSet::new();
    for t in &doc.text_boxes {
        for r in &doc.typography.resolved(t)?.runs {
            let index = fonts
                .faces()
                .iter()
                .position(|f| font_hash(f.content_hash) == r.style.font.hash)
                .ok_or("Package requires an explicit redistribution licence for this non-bundled font")?;
            used.insert(index);
        }
        for g in engine.compose_document(doc, t, 1.)?.layout.lines.iter().flat_map(|l| &l.glyphs) {
            used.insert(g.face.0);
        }
    }
    std::fs::create_dir(stage.join("Fonts")).map_err(|e| e.to_string())?;
    let mut manifest = Vec::new();
    let mut report = String::new();
    for index in used {
        let face = fonts.faces().get(index).ok_or("missing package font")?;
        crate::font_subset::embedding_allowed(&face.bytes, false)?;
        let hash = font_hash(face.content_hash);
        let name = format!("Fonts/{hash}.ttf");
        super::package::write_new(&stage.join(&name), &face.bytes)?;
        let licence = match index {
            0 => include_str!("../../varos-text/assets/fonts/Inter-OFL.txt"),
            1 => include_str!("../../varos-text/assets/fonts/plex-sans-arabic-LICENSE.txt"),
            _ => return Err("unknown font redistribution licence".into()),
        };
        super::package::write_new(&stage.join(format!("Fonts/{hash}.LICENSE.txt")), licence.as_bytes())?;
        manifest.push(serde_json::json!({"file":name,"sha256":hash,"family":face.family,"weight":face.weight,"face_index":0,"axes":{},"licence":"SIL-OFL-1.1","decision":"redistribution permitted; licence included"}));
        report.push_str(&format!("Font: {} {} — OFL licence included\n", face.family, face.weight));
    }
    super::package::write_new(
        &stage.join("Fonts/manifest.json"),
        &serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )?;
    Ok(report)
}
