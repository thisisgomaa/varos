//! Adapted from VectorCraft color/src/swatch.rs:9-45@a469568 (MIT OR Apache-2.0).
//! Flat document table with stable ids and named groups; refs never form cycles.
use crate::{
    geom::Rgba,
    model::{Document, Paint},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Swatch {
    pub id: u32,
    pub name: String,
    pub paint: Paint,
    #[serde(default)]
    pub global: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
}
pub fn validate_paint(p: &Paint, doc: &Document) -> Result<(), String> {
    match p {
        Paint::None => Ok(()),
        Paint::Solid(c) if c.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)) => Ok(()),
        Paint::Solid(_) => Err("colour channels must be in 0..1".into()),
        Paint::Gradient(g) => g.validate(),
        Paint::SwatchRef { id }
            if doc.swatches.iter().any(|s| s.id == *id && !matches!(s.paint, Paint::SwatchRef { .. })) =>
        {
            Ok(())
        }
        Paint::SwatchRef { .. } => Err("unresolved or recursive swatch reference".into()),
    }
}
pub fn validate_document(doc: &Document) -> Result<(), String> {
    if doc.swatches.len() > 4096 {
        return Err("too many swatches".into());
    }
    let mut ids = std::collections::HashSet::new();
    for s in &doc.swatches {
        if s.id == 0
            || !ids.insert(s.id)
            || s.name.trim().is_empty()
            || s.name.chars().any(char::is_control)
            || s.group.chars().any(char::is_control)
            || s.name.len() > 256
            || s.group.len() > 256
            || matches!(s.paint, Paint::SwatchRef { .. })
        {
            return Err("invalid swatch table".into());
        }
        validate_paint(&s.paint, doc)?;
    }
    for p in &doc.paths {
        for paint in [p.appearance().fill(), p.appearance().stroke()] {
            if !matches!(paint, Paint::Solid(_)) {
                validate_paint(paint, doc)?;
            }
        }
    }
    Ok(())
}
impl Paint {
    pub fn is_painted(&self) -> bool {
        !matches!(self, Self::None)
    }
    pub fn resolved_ref<'a>(&'a self, doc: &'a Document) -> &'a Paint {
        match self {
            Self::SwatchRef { id } => doc.swatches.iter().find(|s| s.id == *id).map_or(self, |s| &s.paint),
            _ => self,
        }
    }
    pub fn resolved(&self, doc: &Document) -> Paint {
        self.resolved_ref(doc).clone()
    }
    pub fn sample(&self, point: crate::geom::Pt, doc: &Document) -> Option<Rgba> {
        match self.resolved(doc) {
            Self::Solid(c) => Some(c),
            Self::Gradient(g) => Some(g.sample_point(point)),
            _ => None,
        }
    }
}

impl crate::model::Path {
    pub fn map_gradient_placement(&mut self, f: impl Fn(crate::Pt) -> crate::Pt) {
        for paint in [&mut self.fill, &mut self.stroke] {
            if let Paint::Gradient(g) = paint {
                *g = g.mapped(&f);
            }
        }
    }
}
