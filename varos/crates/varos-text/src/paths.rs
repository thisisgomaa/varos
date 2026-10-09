//! Core-compatible anchor geometry without a dependency on varos-core.
//! Hosts allocate IDs and carry NonZero on the transient draw item. Do NOT put
//! these contours into today's persisted EvenOdd Path and discard the fill rule.
//! Integration contract: render/export each GlyphPath as an independent NonZero
//! fill. Never concatenate contours across glyphs (font winding can differ).
use crate::{
    outlines::{self, Command, Outline, Point},
    FontSet, Glyph, Layout,
};
#[derive(Clone, Debug, PartialEq)]
pub struct PathAnchor {
    pub p: Point,
    pub hin: Option<Point>,
    pub hout: Option<Point>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Contour {
    pub anchors: Vec<PathAnchor>,
    pub closed: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillRule {
    NonZero,
}
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphPath {
    pub glyph: Glyph,
    /// Keep independent glyphs separate: opposite font winding must not cancel.
    pub contours: Vec<Contour>,
    pub fill_rule: FillRule,
}
pub fn layout_paths(fonts: &FontSet, layout: &Layout) -> Result<Vec<GlyphPath>, &'static str> {
    layout
        .lines
        .iter()
        .flat_map(|l| &l.glyphs)
        .map(|g| {
            Ok(GlyphPath {
                glyph: g.clone(),
                contours: contours(&outlines::glyph_outline(fonts, g)?)?,
                fill_rule: FillRule::NonZero,
            })
        })
        .collect()
}
pub fn contours(outline: &Outline) -> Result<Vec<Contour>, &'static str> {
    let mut result: Vec<Contour> = Vec::new();
    for command in &outline.commands {
        match *command {
            Command::Move(p) => {
                result.push(Contour { anchors: vec![PathAnchor { p, hin: None, hout: None }], closed: false })
            }
            Command::Line(p) => {
                let contour = result.last_mut().ok_or("outline line before move")?;
                if contour.closed {
                    return Err("outline line after close");
                }
                contour.anchors.push(PathAnchor { p, hin: None, hout: None });
            }
            Command::Cubic(a, b, p) => {
                let contour = result.last_mut().ok_or("outline curve before move")?;
                if contour.closed {
                    return Err("outline curve after close");
                }
                contour.anchors.last_mut().ok_or("empty outline contour")?.hout = Some(a);
                contour.anchors.push(PathAnchor { p, hin: Some(b), hout: None });
            }
            Command::Close => {
                let contour = result.last_mut().ok_or("outline close before move")?;
                if contour.closed {
                    return Err("outline repeated close");
                }
                // A closing cubic may repeat the first point. Preserve its
                // incoming control on the first anchor, with no zero-length edge.
                if contour.anchors.len() > 1
                    && contour.anchors[0].p == contour.anchors.last().ok_or("empty outline contour")?.p
                {
                    let last = contour.anchors.pop().ok_or("empty outline contour")?;
                    contour.anchors[0].hin = last.hin;
                }
                contour.closed = true;
            }
        }
    }
    if result
        .iter()
        .flat_map(|c| &c.anchors)
        .any(|a| a.p.iter().chain(a.hin.iter().flatten()).chain(a.hout.iter().flatten()).any(|v| !v.is_finite()))
    {
        return Err("nonfinite outline coordinate");
    }
    Ok(result)
}
