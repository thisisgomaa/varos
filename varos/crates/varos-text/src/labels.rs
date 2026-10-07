//! Bounded retained label layouts; raster atlas and textured mesh emission belong to the host.
use crate::{Engine, FaceId, Layout, Request};
use std::{collections::HashMap, sync::Arc};

pub const LABEL_ENTRIES: usize = 4096;
pub const LABEL_BYTES: usize = 4 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LabelKey {
    pub text: String,
    pub face: FaceId,
    pub size_bits: u32,
    pub width_bits: Option<u32>,
    pub role: u32,
    pub ppp_bits: u32,
}
struct Entry {
    layout: Arc<Layout>,
    frame: u64,
    stamp: u64,
    bytes: usize,
}
pub struct LabelCache {
    entries: HashMap<LabelKey, Entry>,
    bytes: usize,
    stamp: u64,
    fonts: [u8; 32],
    last_expired_frame: Option<u64>,
}
impl LabelCache {
    /// A cache belongs to one immutable font snapshot.
    pub fn new(engine: &Engine) -> Self {
        Self {
            entries: HashMap::new(),
            bytes: 0,
            stamp: 0,
            fonts: engine.font_set().snapshot_hash(),
            last_expired_frame: None,
        }
    }
    pub fn stats(&self) -> (usize, usize) {
        (self.entries.len(), self.bytes)
    }
    pub fn get(&mut self, key: &LabelKey, frame: u64) -> Option<Arc<Layout>> {
        let entry = self.entries.get_mut(key)?;
        self.stamp = self.stamp.wrapping_add(1);
        entry.stamp = self.stamp;
        entry.frame = frame;
        Some(entry.layout.clone())
    }
    pub fn expire(&mut self, frame: u64) {
        if self.last_expired_frame == Some(frame) {
            return;
        }
        self.last_expired_frame = Some(frame);
        self.entries.retain(|_, entry| {
            if frame.saturating_sub(entry.frame) > 600 {
                self.bytes -= entry.bytes;
                false
            } else {
                true
            }
        });
    }
    pub fn layout(&mut self, engine: &mut Engine, key: &LabelKey, frame: u64) -> Result<Arc<Layout>, &'static str> {
        if engine.font_set().snapshot_hash() != self.fonts {
            return Err("label cache font snapshot mismatch");
        }
        self.expire(frame);
        if let Some(layout) = self.get(key, frame) {
            return Ok(layout);
        }
        if key.text.len() > 4096 {
            return Err("label display limit: elide first");
        }
        if !f32::from_bits(key.ppp_bits).is_finite() || f32::from_bits(key.ppp_bits) <= 0. {
            return Err("invalid pixels per point");
        }
        let mut req = Request::new(&key.text, f32::from_bits(key.size_bits), key.width_bits.map(f32::from_bits));
        req.face = key.face;
        let layout = Arc::new(engine.layout(&req)?);
        let bytes = retained_bytes(key, &layout);
        if bytes <= LABEL_BYTES {
            while self.entries.len() >= LABEL_ENTRIES || self.bytes + bytes > LABEL_BYTES {
                let oldest = self.entries.iter().min_by_key(|(_, e)| e.stamp).map(|(k, _)| k.clone()).unwrap();
                self.bytes -= self.entries.remove(&oldest).unwrap().bytes;
            }
            self.stamp = self.stamp.wrapping_add(1);
            self.entries.insert(key.clone(), Entry { layout: layout.clone(), frame, stamp: self.stamp, bytes });
            self.bytes += bytes;
        }
        Ok(layout)
    }
}
fn retained_bytes(key: &LabelKey, layout: &Layout) -> usize {
    std::mem::size_of::<(LabelKey, Entry)>()
        + key.text.len()
        + layout.source.capacity()
        + layout.lines.capacity() * std::mem::size_of::<crate::Line>()
        + layout.lines.iter().map(|l| l.glyphs.capacity() * std::mem::size_of::<crate::Glyph>()).sum::<usize>()
        + layout.carets.capacity() * std::mem::size_of::<crate::Caret>()
        + layout.levels.capacity()
        + layout.issues.capacity() * std::mem::size_of::<crate::Issue>()
        + layout
            .issues
            .iter()
            .map(|i| match i {
                crate::Issue::UnsupportedLanguage(s) => s.capacity(),
                _ => 0,
            })
            .sum::<usize>()
}
