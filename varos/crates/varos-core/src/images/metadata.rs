//! ADR-0014 wire values. All strings are bounded identities/locators, never pixel payloads.
// Image/link contracts adapted from VectorCraft doc/{node.rs,links.rs}@a469568.
// Copyright 2026 ArtCraft Team. MIT OR Apache-2.0; see NOTICE.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlobKey(pub String);
impl BlobKey {
    pub fn valid(&self) -> bool {
        self.0.len() == 64 && self.0.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mime {
    Png,
    Jpeg,
    Gif,
    WebP,
    Tiff,
    Bmp,
}
impl Mime {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::WebP => "image/webp",
            Self::Tiff => "image/tiff",
            Self::Bmp => "image/bmp",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetMeta {
    pub key: BlobKey,
    pub mime: Mime,
    pub encoded_len: usize,
    /// Original EXIF normalization recipe; every consumer uses oriented pixels.
    pub orientation: u32,
    pub px_w: u32,
    pub px_h: u32,
    pub proxy_w: u32,
    pub proxy_h: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageAffine {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}
impl ImageAffine {
    pub fn apply(self, [x, y]: [f32; 2]) -> [f32; 2] {
        [self.a * x + self.c * y + self.e, self.b * x + self.d * y + self.f]
    }
    pub fn valid(self) -> bool {
        [self.a, self.b, self.c, self.d, self.e, self.f].iter().all(|v| v.is_finite())
            && (self.a as f64 * self.d as f64 - self.b as f64 * self.c as f64).abs() > 1e-12
    }
    pub fn bounds(self, w: u32, h: u32) -> [f32; 4] {
        let p = [[0., 0.], [w as f32, 0.], [w as f32, h as f32], [0., h as f32]].map(|p| self.apply(p));
        let x0 = p.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
        let y0 = p.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let x1 = p.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max);
        let y1 = p.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);
        [x0, y0, x1 - x0, y1 - y0]
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkInfo {
    pub absolute: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home_relative: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_relative: Option<String>,
    pub accepted_mtime: String,
    pub byte_size: usize,
    pub hash: BlobKey,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlacementMode {
    #[default]
    Embed,
    Link,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplacementPolicy {
    #[default]
    KeepBounds,
    KeepTransform,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageObject {
    pub id: u32,
    pub blob: BlobKey,
    pub px_w: u32,
    pub px_h: u32,
    pub ppi: [f32; 2],
    pub xform: ImageAffine,
    pub opacity: f32,
    pub placement: PlacementMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<LinkInfo>,
    #[serde(default)]
    pub replacement: ReplacementPolicy,
}
impl ImageObject {
    pub fn effective_ppi(&self) -> [f32; 2] {
        [72. / self.xform.a.hypot(self.xform.b), 72. / self.xform.c.hypot(self.xform.d)]
    }
}
pub fn default_effects_ppi() -> f32 {
    300.
}
pub fn is_default_effects_ppi(v: &f32) -> bool {
    *v == 300.
}
