//! Lane H: byte-fed font/glyph export seam for PDF and packaging consumers.
pub use varos_text::outlines::{glyph_outline, Command};
pub use varos_text::{FaceId, FontFace, FontSet, Glyph};

pub fn snapshot(doc: &varos_core::model::Document) -> Result<FontSet, String> {
    let bundled = super::bundled_fonts()?;
    let mut supplied = true;
    for text in &doc.text_boxes {
        for run in &doc.typography.resolved(text)?.runs {
            supplied &= bundled.faces().iter().any(|f| super::font_hash(f.content_hash) == run.style.font.hash);
        }
    }
    if supplied {
        Ok(bundled)
    } else {
        super::host_fonts::snapshot()
    }
}
