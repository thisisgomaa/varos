//! Font extents + actual ink + leading; a shared baseline for mixed faces.
use crate::{FontSet, Layout};
use skrifa::{
    instance::{LocationRef, Size},
    FontRef, MetadataProvider,
};
use unicode_script::{Script, UnicodeScript};
#[derive(Clone, Copy, Debug)]
pub enum LineHeight {
    /// Ascender/descender/lineGap, at least 150% for Arabic body text.
    Font,
    /// Multiple of largest line font; Arabic minimum 130%.
    Multiple(f32),
    /// Absolute request, expanded to contain font extents and mark ink.
    AtLeast(f32),
}
impl Default for LineHeight {
    fn default() -> Self {
        Self::Multiple(1.5)
    }
}
impl LineHeight {
    pub(crate) fn validate(self) -> Result<(), &'static str> {
        match self {
            Self::Font => Ok(()),
            Self::Multiple(v) | Self::AtLeast(v) if v.is_finite() && v > 0. && v <= 100000. => Ok(()),
            _ => Err("invalid line height"),
        }
    }
}
pub(crate) fn apply(
    fonts: &FontSet,
    layout: &mut Layout,
    default_size: f32,
    policy: LineHeight,
) -> Result<(), &'static str> {
    policy.validate()?;
    let mut top = 0.;
    for line in &mut layout.lines {
        let arabic = layout.source[line.range.clone()].chars().any(|c| c.script() == Script::Arabic);
        let mut size = default_size;
        let mut ascent = line.ascent;
        let mut descent = line.descent;
        let mut gap = 0f32;
        for g in &line.glyphs {
            size = size.max(g.size);
            let font = FontRef::new(&fonts.face(g.face).ok_or("invalid face")?.bytes).map_err(|_| "invalid font")?;
            let m = font.metrics(Size::new(g.size), LocationRef::default());
            ascent = ascent.max(m.ascent);
            descent = descent.max(-m.descent);
            gap = gap.max(m.leading);
            if let Some(b) = crate::outlines::glyph_outline(fonts, g)?.ink_bounds() {
                ascent = ascent.max(line.baseline - b[1]);
                descent = descent.max(b[3] - line.baseline);
            }
        }
        let requested = match policy {
            LineHeight::Font => (ascent + descent + gap).max(if arabic { size * 1.5 } else { 0. }),
            LineHeight::Multiple(m) => size * m,
            LineHeight::AtLeast(h) => h,
        };
        let height = requested.max(ascent + descent).max(if arabic { size * 1.3 } else { 0. });
        let leading = (height - ascent - descent) * 0.5;
        let baseline = top + leading + ascent;
        for g in &mut line.glyphs {
            g.y += baseline - line.baseline;
        }
        line.baseline = baseline;
        line.ascent = ascent + leading;
        line.descent = descent + leading;
        top += height;
    }
    Ok(())
}
