//! Private Stage 0 bridge: one native-selected Arabic line, normal positioned outlines.
use kurbo::{Affine, BezPath};
use std::{ops::Range, sync::Arc};
pub use varos_text;
use varos_text::{
    Direction, Engine, FallbackPolicy, FontFace, FontSet, Issue, Request,
    composer::{Alignment, ParagraphOptions},
    kashida::Kashida,
    outlines::{self, Command},
};

/// External spike input; no font binary is copied into VectorCraft.
pub const FALLBACK_FONT: &[u8] = include_bytes!("~/Documents/AI workspace/varos/varos/crates/varos-text/assets/fonts/NotoSansArabic.ttf");
pub const FALLBACK_FAMILY: &str = "Noto Sans Arabic";

#[derive(Debug)]
pub struct Glyph {
    pub outline: BezPath,
    pub cluster: Range<usize>,
    pub gid: u32,
    /// Baseline-relative pen and outline offsets, in points, Y down.
    pub x: f64,
    pub y: f64,
    pub offset: [f64; 2],
    pub advance: f64,
    pub rtl: bool,
}
#[derive(Debug)]
pub struct Line {
    pub glyphs: Vec<Glyph>,
    pub width: f64,
    pub ascent: f64,
    pub descent: f64,
    pub insertions: usize,
    pub used_fallback: bool,
}

/// Exact conversion lifted from the proven standalone renderer adapter.
pub fn bezpath(outline: &outlines::Outline) -> BezPath {
    let point = |p: [f32; 2]| (f64::from(p[0]), f64::from(p[1]));
    let mut path = BezPath::new();
    for command in &outline.commands {
        match *command {
            Command::Move(p) => path.move_to(point(p)),
            Command::Line(p) => path.line_to(point(p)),
            Command::Cubic(a, b, c) => path.curve_to(point(a), point(b), point(c)),
            Command::Close => path.close_path(),
        }
    }
    path
}

// Varos requires &'static str metadata. Bounded spike table avoids leaking dynamic names.
fn family_name(family: &str) -> Option<&'static str> {
    [FALLBACK_FAMILY, "Noto Naskh Arabic", "Amiri", "Arial", "SF Arabic"].into_iter().find(|name| *name == family)
}

/// Try the native face, then Noto for collections, unsupported metadata or rejected shaping.
/// Returns an error rather than changing the native-selected line breaks.
pub fn justify(text: &str, font: &[u8], family: &str, weight: u16, size: f64, width: f64) -> Result<Line, &'static str> {
    if text.len() > 16_384
        || text.is_empty()
        || text.contains(['\n', '\r', '\t'])
        || !size.is_finite()
        || !(0.1..=10_000.0).contains(&size)
        || !width.is_finite()
        || !(0.1..=1_000_000.0).contains(&width)
    {
        return Err("unsupported Arabic line input");
    }
    if let Some(family) = family_name(family)
        && !font.starts_with(b"ttcf")
        && let Ok(line) = compose(text, font, family, weight, size as f32, width as f32, false)
    {
        return Ok(line);
    }
    compose(text, FALLBACK_FONT, FALLBACK_FAMILY, 400, size as f32, width as f32, true)
}

fn compose(text: &str, font: &[u8], family: &'static str, weight: u16, size: f32, width: f32, used_fallback: bool) -> Result<Line, &'static str> {
    let fonts = FontSet::new(vec![FontFace::new(family, weight, Arc::from(font))?], FallbackPolicy::default())?;
    let mut engine = Engine::new(fonts.clone())?;
    let mut request = Request::new(text, size, Some(width));
    request.direction = Direction::Rtl;
    request.language = "ar";
    request.script = Some("Arab".parse()?);
    let options = ParagraphOptions { alignment: Alignment::JustifyFull, kashida: Kashida::Balanced, ..Default::default() };
    let layout = engine.compose(&request, &options)?;
    let line = layout.lines.first().ok_or("Arabic composition produced no line")?;
    if layout.lines.len() != 1
        || layout.source != text
        || line.glyphs.iter().any(|g| g.id == 0)
        || !line.width.is_finite()
        || (line.width - width).abs() > 0.01
    {
        return Err("Arabic composition changed line breaks, has missing glyphs or residual width");
    }
    let insertions = layout
        .issues
        .iter()
        .filter_map(|issue| match issue {
            Issue::KashidaInserted { count, .. } => Some(*count),
            _ => None,
        })
        .sum();
    let mut glyphs = Vec::with_capacity(line.glyphs.len());
    for g in &line.glyphs {
        if text.get(g.cluster.clone()).is_none()
            || !g.x.is_finite()
            || !g.y.is_finite()
            || !g.advance.is_finite()
            || !g.offset.iter().all(|v| v.is_finite())
        {
            return Err("invalid Arabic glyph placement or source mapping");
        }
        let mut outline = bezpath(&outlines::glyph_outline(&fonts, g)?);
        outline.apply_affine(Affine::translate((0.0, -f64::from(line.baseline))));
        glyphs.push(Glyph {
            outline,
            cluster: g.cluster.clone(),
            gid: u32::from(g.id),
            x: f64::from(g.x),
            y: f64::from(g.y - line.baseline),
            offset: g.offset.map(f64::from),
            advance: f64::from(g.advance),
            rtl: g.level % 2 == 1,
        });
    }
    Ok(Line { glyphs, width: f64::from(line.width), ascent: f64::from(line.ascent), descent: f64::from(line.descent), insertions, used_fallback })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn justified_arabic_600_has_kashida() {
        let text = "بسم الله الرحمن الرحيم";
        let line = justify(text, FALLBACK_FONT, FALLBACK_FAMILY, 400, 48.0, 600.0).unwrap();
        assert!(line.insertions >= 1);
        assert!((line.width - 600.0).abs() <= 0.01);
        assert!((line.glyphs.iter().map(|g| g.advance).sum::<f64>() - 600.0).abs() <= 0.01);
        assert!(!line.used_fallback);
        assert!(line.glyphs.iter().any(|g| !g.outline.is_empty()));
        eprintln!("insertions={}, width={}, glyphs={}", line.insertions, line.width, line.glyphs.len());
    }
    #[test]
    fn rejected_font_falls_back_and_invalid_sizes_fail() {
        let line = justify("بسم الله الرحمن الرحيم", b"ttcf", "Collection", 400, 48.0, 600.0).unwrap();
        assert!(line.used_fallback && line.insertions > 0);
        assert!(justify("بسم", FALLBACK_FONT, FALLBACK_FAMILY, 400, f64::NAN, 600.0).is_err());
    }
}
