//! Glyph outline extraction — turning shaped text into vector paths.
//!
//! This is what makes "convert text to outlines" and SVG export of text
//! possible, and it is the front door for the *Vector AI* dream in
//! `DREAMS.md`: once a glyph is a path, every path tool (boolean ops, stroke
//! outlining, node editing) applies to letters too.
//!
//! Coordinates come straight from the font in its native **y-up** space.
//! Pass a pixel size to scale, or [`None`] for raw font units (matching the
//! `font units` convention the shaper already uses). The document is y-down,
//! so the caller flips Y when placing a glyph on the canvas.

use skrifa::instance::{LocationRef, Size};
use skrifa::metrics::Metrics;
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, GlyphId, MetadataProvider};

/// One drawing command of a glyph contour, in font/em space (y-up).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutlineCmd {
    MoveTo { x: f32, y: f32 },
    LineTo { x: f32, y: f32 },
    /// Quadratic Bézier (TrueType).
    QuadTo { cx: f32, cy: f32, x: f32, y: f32 },
    /// Cubic Bézier (CFF/OpenType).
    CurveTo { c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32 },
    Close,
}

/// The outline of a single glyph: a list of drawing commands plus the font's
/// units-per-em (meaningful when extracted unscaled).
#[derive(Debug, Clone, Default)]
pub struct GlyphOutline {
    pub commands: Vec<OutlineCmd>,
    pub units_per_em: u16,
}

impl GlyphOutline {
    /// True when the glyph has no contours (e.g. a space).
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

/// A [`OutlinePen`] that records commands into a [`GlyphOutline`].
struct Collector<'a>(&'a mut Vec<OutlineCmd>);

impl OutlinePen for Collector<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.push(OutlineCmd::MoveTo { x, y });
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.push(OutlineCmd::LineTo { x, y });
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.0.push(OutlineCmd::QuadTo { cx, cy, x, y });
    }
    fn curve_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        self.0.push(OutlineCmd::CurveTo { c1x, c1y, c2x, c2y, x, y });
    }
    fn close(&mut self) {
        self.0.push(OutlineCmd::Close);
    }
}

/// Extract the outline of `glyph_id`. `size_px = Some(px)` scales to that em
/// size; `None` yields raw font units. Errors if the font won't parse or the
/// glyph has no outline data (bitmap/COLR-only glyphs).
pub fn glyph_outline(
    font_bytes: &[u8],
    glyph_id: u32,
    size_px: Option<f32>,
) -> Result<GlyphOutline, &'static str> {
    let font = FontRef::new(font_bytes).map_err(|_| "failed to parse font")?;
    let units_per_em = Metrics::new(&font, Size::unscaled(), LocationRef::default()).units_per_em;
    let glyphs = font.outline_glyphs();
    let glyph = glyphs.get(GlyphId::new(glyph_id)).ok_or("glyph has no outline")?;

    let size = match size_px {
        Some(px) => Size::new(px),
        None => Size::unscaled(),
    };
    let settings = DrawSettings::unhinted(size, LocationRef::default());

    let mut commands = Vec::new();
    glyph
        .draw(settings, &mut Collector(&mut commands))
        .map_err(|_| "failed to draw glyph outline")?;

    Ok(GlyphOutline { commands, units_per_em })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{shape_run, TextDirection};

    const AMIRI_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Amiri-Regular.ttf");

    /// Shape one Arabic word and return the glyph id of its first glyph.
    fn first_glyph_of(text: &str) -> u32 {
        shape_run(AMIRI_REGULAR, text, TextDirection::RightToLeft)
            .unwrap()
            .glyphs
            .first()
            .unwrap()
            .glyph_id
    }

    #[test]
    fn letter_has_contours() {
        let gid = first_glyph_of("ب");
        let out = glyph_outline(AMIRI_REGULAR, gid, None).unwrap();
        assert!(!out.is_empty(), "a letter must have outline commands");
        assert_eq!(out.units_per_em, 1000, "Amiri is UPM 1000");
        // A contour must start with a MoveTo.
        assert!(matches!(out.commands[0], OutlineCmd::MoveTo { .. }));
    }

    #[test]
    fn outline_closes_its_contours() {
        let gid = first_glyph_of("م");
        let out = glyph_outline(AMIRI_REGULAR, gid, None).unwrap();
        let closes = out.commands.iter().filter(|c| matches!(c, OutlineCmd::Close)).count();
        assert!(closes >= 1, "glyph should close at least one contour");
    }

    #[test]
    fn scaling_shrinks_coordinates() {
        let gid = first_glyph_of("ع");
        let unscaled = glyph_outline(AMIRI_REGULAR, gid, None).unwrap();
        let scaled = glyph_outline(AMIRI_REGULAR, gid, Some(16.0)).unwrap();

        let max_abs = |o: &GlyphOutline| {
            o.commands
                .iter()
                .map(|c| match *c {
                    OutlineCmd::MoveTo { x, y } | OutlineCmd::LineTo { x, y } => x.abs().max(y.abs()),
                    OutlineCmd::QuadTo { x, y, .. } => x.abs().max(y.abs()),
                    OutlineCmd::CurveTo { x, y, .. } => x.abs().max(y.abs()),
                    OutlineCmd::Close => 0.0,
                })
                .fold(0.0_f32, f32::max)
        };
        // 16px em vs 1000-unit em → scaled coords are far smaller.
        assert!(max_abs(&scaled) < max_abs(&unscaled));
    }

    #[test]
    fn missing_glyph_errors() {
        // Glyph id absurdly out of range.
        assert!(glyph_outline(AMIRI_REGULAR, 999_999, None).is_err());
    }
}
