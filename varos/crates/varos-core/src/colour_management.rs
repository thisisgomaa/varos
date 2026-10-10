//! Lane C: explicit source colours. Screen space remains sRGB; no ambient profile state.
use crate::{
    geom::Rgba,
    model::{Document, Paint},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColourMode {
    #[default]
    Rgb,
    Cmyk,
}
impl ColourMode {
    pub fn is_rgb(&self) -> bool {
        *self == Self::Rgb
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cmyk {
    pub c: f32,
    pub m: f32,
    pub y: f32,
    pub k: f32,
}
impl Cmyk {
    pub fn channels(self) -> [f32; 4] {
        [self.c, self.m, self.y, self.k]
    }
    pub fn from_rgb(rgb: Rgba) -> Self {
        let k = 1. - rgb[0].max(rgb[1]).max(rgb[2]);
        if k >= 1. {
            return Self { c: 0., m: 0., y: 0., k: 1. };
        }
        Self { c: (1. - rgb[0] - k) / (1. - k), m: (1. - rgb[1] - k) / (1. - k), y: (1. - rgb[2] - k) / (1. - k), k }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "model", rename_all = "snake_case", deny_unknown_fields)]
pub enum Colour {
    Rgb { r: f32, g: f32, b: f32 },
    Cmyk { c: f32, m: f32, y: f32, k: f32 },
    Gray { value: f32 },
    Spot { name: String, tint: f32, alt: Cmyk },
}
impl Colour {
    pub fn validate(&self) -> Result<(), String> {
        let values = match self {
            Self::Rgb { r, g, b } => vec![*r, *g, *b],
            Self::Cmyk { c, m, y, k } => vec![*c, *m, *y, *k],
            Self::Gray { value } => vec![*value],
            Self::Spot { name, tint, alt } => {
                if name.trim().is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
                    return Err("invalid spot name".into());
                }
                let mut v = alt.channels().to_vec();
                v.push(*tint);
                v
            }
        };
        if values.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)) {
            return Err("colour channels must be in 0..1".into());
        }
        Ok(())
    }
    /// Unprofiled approximation. Call `to_srgb` to use an explicit ICC source profile.
    pub fn naive_srgb(&self, alpha: f32) -> Rgba {
        match self {
            Self::Rgb { r, g, b } => [*r, *g, *b, alpha],
            Self::Gray { value } => [*value, *value, *value, alpha],
            Self::Cmyk { c, m, y, k } => [(1. - c) * (1. - k), (1. - m) * (1. - k), (1. - y) * (1. - k), alpha],
            Self::Spot { tint, alt, .. } => {
                Self::Cmyk { c: alt.c * tint, m: alt.m * tint, y: alt.y * tint, k: alt.k * tint }.naive_srgb(alpha)
            }
        }
    }
    pub fn to_srgb(&self, alpha: f32, source: Option<&IccProfile>) -> Result<Rgba, String> {
        self.validate()?;
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err("invalid alpha".into());
        }
        let Some(source) = source else {
            return Ok(self.naive_srgb(alpha));
        };
        let p = source.parse()?;
        let (layout, values, space) = match self {
            Self::Rgb { r, g, b } => (moxcms::Layout::Rgb, vec![*r, *g, *b], moxcms::DataColorSpace::Rgb),
            Self::Gray { value } => (moxcms::Layout::Gray, vec![*value], moxcms::DataColorSpace::Gray),
            Self::Cmyk { c, m, y, k } => (moxcms::Layout::Rgba, vec![*c, *m, *y, *k], moxcms::DataColorSpace::Cmyk),
            Self::Spot { tint, alt, .. } => {
                (moxcms::Layout::Rgba, alt.channels().map(|v| v * tint).to_vec(), moxcms::DataColorSpace::Cmyk)
            }
        };
        if p.color_space != space {
            return Err("ICC profile does not match colour model".into());
        }
        let transform = p
            .create_transform_f32(layout, &moxcms::ColorProfile::new_srgb(), moxcms::Layout::Rgb, Default::default())
            .map_err(|e| e.to_string())?;
        let mut rgb = [0.; 3];
        transform.transform(&values, &mut rgb).map_err(|e| e.to_string())?;
        Ok([rgb[0].clamp(0., 1.), rgb[1].clamp(0., 1.), rgb[2].clamp(0., 1.), alpha])
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedColour {
    pub colour: Colour,
    pub alpha: f32,
}
impl ManagedColour {
    pub fn hash_colour<H: std::hash::Hasher>(&self, state: &mut H) {
        use std::hash::Hash;
        std::mem::discriminant(&self.colour).hash(state);
        let channels = match &self.colour {
            Colour::Rgb { r, g, b } => vec![*r, *g, *b],
            Colour::Cmyk { c, m, y, k } => vec![*c, *m, *y, *k],
            Colour::Gray { value } => vec![*value],
            Colour::Spot { name, tint, alt } => {
                name.hash(state);
                let mut c = alt.channels().to_vec();
                c.push(*tint);
                c
            }
        };
        for v in channels.into_iter().chain([self.alpha]) {
            if v == 0. {
                0u32.hash(state);
            } else {
                v.to_bits().hash(state);
            }
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        self.colour.validate()?;
        if !self.alpha.is_finite() || !(0.0..=1.0).contains(&self.alpha) {
            return Err("invalid alpha".into());
        }
        Ok(())
    }
    pub fn rgba(&self) -> Rgba {
        self.colour.naive_srgb(self.alpha)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IccProfile {
    pub name: String,
    /// Hex-encoded ICC metadata; raster byte storage stays outside Document.
    pub data: String,
}
impl IccProfile {
    pub fn new(name: String, bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 4 * 1024 * 1024 {
            return Err("ICC profile exceeds 4 MiB".into());
        }
        let data = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let profile = Self { name, data };
        profile.parse()?;
        Ok(profile)
    }
    pub fn bytes(&self) -> Result<Vec<u8>, String> {
        if self.name.trim().is_empty()
            || self.name.len() > 256
            || self.name.chars().any(char::is_control)
            || self.data.len() > 8 * 1024 * 1024
            || self.data.len() < 256
            || !self.data.len().is_multiple_of(2)
        {
            return Err("invalid ICC profile size/name".into());
        }
        self.data
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| {
                let digit = |b: u8| (b as char).to_digit(16).ok_or("invalid ICC hex data");
                Ok((digit(c[0])? * 16 + digit(c[1])?) as u8)
            })
            .collect::<Result<Vec<_>, &str>>()
            .map_err(str::to_owned)
    }
    pub fn parse(&self) -> Result<moxcms::ColorProfile, String> {
        moxcms::ColorProfile::new_from_slice(&self.bytes()?).map_err(|e| e.to_string())
    }
}
pub fn validate_document(doc: &Document) -> Result<(), String> {
    if let Some(p) = &doc.output_profile {
        let parsed = p.parse()?;
        if !matches!(
            parsed.color_space,
            moxcms::DataColorSpace::Rgb | moxcms::DataColorSpace::Cmyk | moxcms::DataColorSpace::Gray
        ) {
            return Err("unsupported ICC colour space".into());
        }
    }
    let mut spots = std::collections::BTreeMap::new();
    for paint in doc.paths.iter().flat_map(|p| [&p.fill, &p.stroke]).chain(doc.swatches.iter().map(|s| &s.paint)) {
        if let crate::model::Paint::Managed(ManagedColour { colour: Colour::Spot { name, alt, .. }, .. }) = paint {
            if spots.insert(name, alt).is_some_and(|previous| previous != alt) {
                return Err("a spot name must have one consistent alternate colour".into());
            }
        }
    }
    Ok(())
}
/// Diagnostics for formats whose working colour space is sRGB.
pub fn screen_export_notes(doc: &Document, format: &str) -> Vec<crate::ExportNote> {
    let mut notes = Vec::new();
    for p in &doc.paths {
        for paint in [&p.fill, &p.stroke] {
            if let crate::model::Paint::Managed(m) = paint.resolved_ref(doc) {
                notes.push(crate::ExportNote {
                    kind: "colour_conversion".into(),
                    object_id: Some(p.id),
                    message: format!(
                        "{format}: {:?} converted to sRGB ({}); spot identity is not retained.",
                        m.colour,
                        if format == "SVG" || doc.output_profile.is_none() {
                            "unprofiled approximation"
                        } else {
                            "moxcms for matching profile channels; other models use unprofiled approximation"
                        }
                    ),
                });
            }
        }
    }
    notes
}

/// One explicit source-to-sRGB transform per scene, with no global state.
#[derive(Clone)]
pub(crate) struct Screen {
    transform: std::sync::Arc<moxcms::TransformF32Executor>,
    channels: usize,
}
impl Screen {
    pub fn new(profile: &IccProfile) -> Result<Self, String> {
        let p = profile.parse()?;
        let (layout, channels) = match p.color_space {
            moxcms::DataColorSpace::Rgb => (moxcms::Layout::Rgb, 3),
            moxcms::DataColorSpace::Cmyk => (moxcms::Layout::Rgba, 4),
            moxcms::DataColorSpace::Gray => (moxcms::Layout::Gray, 1),
            _ => return Err("unsupported screen profile".into()),
        };
        let transform = p
            .create_transform_f32(layout, &moxcms::ColorProfile::new_srgb(), moxcms::Layout::Rgb, Default::default())
            .map_err(|e| e.to_string())?;
        Ok(Self { transform, channels })
    }
    pub fn paint(&self, paint: Paint) -> Result<Paint, String> {
        let Paint::Managed(m) = &paint else { return Ok(paint) };
        let values = match &m.colour {
            Colour::Rgb { r, g, b } => vec![*r, *g, *b],
            Colour::Gray { value } => vec![*value],
            Colour::Cmyk { c, m, y, k } => vec![*c, *m, *y, *k],
            Colour::Spot { tint, alt, .. } => alt.channels().map(|v| v * tint).to_vec(),
        };
        // The document output profile characterizes matching process colours only.
        if values.len() != self.channels {
            return Ok(paint);
        }
        let mut rgb = [0.; 3];
        self.transform.transform(&values, &mut rgb).map_err(|e| e.to_string())?;
        Ok(Paint::Solid([rgb[0].clamp(0., 1.), rgb[1].clamp(0., 1.), rgb[2].clamp(0., 1.), m.alpha]))
    }
}
