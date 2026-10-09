//! Lane F: shared byte-fed shaping and a retained glyph atlas. No host commands or I/O.
mod painter;
mod raster;
pub use painter::{cell, ShapedPainter, ShapedResponse, ShapedUi};
pub mod editor;
use crate::shell::tokens as t;
use egui::{Color32, FontId, Painter, Pos2, Rect, Vec2};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use unicode_segmentation::UnicodeSegmentation;
use varos_text::{Engine, FaceId, FallbackPolicy, FontFace, FontSet, Layout, Request};

#[derive(Clone)]
pub struct Label {
    pub layout: Arc<Layout>,
    logical: Arc<str>,
    size: Vec2,
}
impl Label {
    pub fn empty() -> Self {
        Self {
            logical: Arc::from(""),
            layout: Arc::new(Layout {
                source: String::new(),
                lines: vec![],
                carets: vec![],
                issues: vec![],
                levels: vec![],
                ink_bounds: None,
            }),
            size: Vec2::ZERO,
        }
    }
    pub fn size(&self) -> Vec2 {
        self.size
    }
}
#[derive(Hash, PartialEq, Eq)]
struct Key {
    text: String,
    size: u32,
    face: usize,
    width: Option<u32>,
    elide: bool,
}
struct Cached {
    label: Label,
    frame: u64,
    bytes: usize,
}
#[derive(Hash, PartialEq, Eq)]
struct GlyphKey {
    face: FaceId,
    id: u16,
    size: u32,
    bin: u8,
}
#[derive(Clone)]
struct Entry {
    texture: egui::TextureId,
    uv: Rect,
    offset: Vec2,
    size: Vec2,
}
#[derive(Clone, Debug)]
pub struct PaintRecord {
    pub text: String,
    pub displayed_text: String,
    pub elided: bool,
    pub rect: Rect,
    pub lines: usize,
    pub clip_rect: Rect,
}
struct Trace {
    frame: u64,
    records: Vec<PaintRecord>,
}
struct System {
    trace: Option<Trace>,
    expired_frame: u64,
    engine: Engine,
    labels: HashMap<Key, Cached>,
    bytes: usize,
    glyphs: HashMap<GlyphKey, Option<Entry>>,
    texture: Option<egui::TextureHandle>,
    retired: Vec<egui::TextureHandle>,
    atlas_frame: u64,
    ppp: f32,
    options: egui::epaint::text::TextOptions,
    x: usize,
    y: usize,
    row: usize,
}
fn fonts() -> Result<FontSet, &'static str> {
    let mut faces = Vec::new();
    for (name, weight, bytes) in [
        ("Inter", 400, include_bytes!("../../../../assets/fonts/Inter-Regular.ttf").as_slice()),
        ("Inter", 500, include_bytes!("../../../../assets/fonts/Inter-Medium.ttf").as_slice()),
        ("Inter", 600, include_bytes!("../../../../assets/fonts/Inter-SemiBold.ttf").as_slice()),
        (
            "IBM Plex Sans Arabic",
            400,
            include_bytes!("../../../../assets/fonts/IBMPlexSansArabic-Regular.ttf").as_slice(),
        ),
        ("JetBrains Mono", 400, include_bytes!("../../../../assets/fonts/JetBrainsMono-Regular.ttf").as_slice()),
        ("Noto Sans Symbols", 400, include_bytes!("../../../../assets/fonts/NotoSansSymbols-Regular.ttf").as_slice()),
        ("Noto Sans Symbols2", 400, include_bytes!("../../../../assets/fonts/NotoSansSymbols2-Regular.ttf").as_slice()),
    ] {
        faces.push(FontFace::new(name, weight, bytes.into())?);
    }
    let defaults = egui::FontDefinitions::default();
    for (name, family, weight) in [
        ("Ubuntu-Light", "Ubuntu", 300),
        ("NotoEmoji-Regular", "Noto Emoji", 400),
        ("emoji-icon-font", "emoji", 400),
        ("Hack", "Hack", 400),
    ] {
        if let Some(data) = defaults.font_data.get(name) {
            faces.push(FontFace::new(family, weight, data.font.as_ref().into())?);
        }
    }
    let fallback = (0..faces.len()).filter(|i| *i != 1 && *i != 2).map(FaceId).collect();
    FontSet::new(faces, FallbackPolicy { common: fallback, scripts: vec![(*b"Arab", vec![FaceId(3)])] })
}
fn system(ctx: &egui::Context) -> Option<Arc<Mutex<System>>> {
    let id = egui::Id::new("lane-f-ui-text");
    if let Some(s) = ctx.data(|d| d.get_temp::<Arc<Mutex<System>>>(id)) {
        return Some(s);
    }
    let engine = Engine::new(fonts().ok()?).ok()?;
    let s = Arc::new(Mutex::new(System {
        trace: None,
        expired_frame: u64::MAX,
        engine,
        labels: HashMap::new(),
        bytes: 0,
        glyphs: HashMap::new(),
        texture: None,
        retired: vec![],
        atlas_frame: 0,
        ppp: 0.0,
        options: Default::default(),
        x: 0,
        y: 0,
        row: 0,
    }));
    ctx.data_mut(|d| d.insert_temp(id, s.clone()));
    Some(s)
}
fn face(font: &FontId) -> FaceId {
    match &font.family {
        egui::FontFamily::Monospace => FaceId(4),
        egui::FontFamily::Name(n) if n.as_ref() == "ui-500" => FaceId(1),
        egui::FontFamily::Name(n) if n.as_ref() == "ui-600" => FaceId(2),
        egui::FontFamily::Name(n) if n.as_ref() == "mono-400" => FaceId(4),
        _ => FaceId(0),
    }
}
pub fn layout(ctx: &egui::Context, text: &str, font: &FontId, width: Option<f32>, elide: bool) -> Option<Label> {
    let bounded =
        text.grapheme_indices(true).map(|(i, _)| i).chain([text.len()]).take_while(|i| *i <= 4096).last().unwrap_or(0);
    let source = if bounded < text.len() { format!("{}…", &text[..bounded]) } else { text.to_owned() };
    let width = width.filter(|w| w.is_finite() && *w > 0.0);
    let key =
        Key { text: source, size: font.size.to_bits(), face: face(font).0, width: width.map(f32::to_bits), elide };
    let shared = system(ctx)?;
    let mut s = shared.lock().ok()?;
    let frame = ctx.cumulative_frame_nr();
    if s.expired_frame != frame {
        let expired: usize = s.labels.values().filter(|c| frame.saturating_sub(c.frame) > 600).map(|c| c.bytes).sum();
        s.labels.retain(|_, c| frame.saturating_sub(c.frame) <= 600);
        s.bytes = s.bytes.saturating_sub(expired);
        s.expired_frame = frame;
    }
    if let Some(cached) = s.labels.get_mut(&key) {
        cached.frame = frame;
        return Some(cached.label.clone());
    }
    let mut request = Request::new(&key.text, font.size, if elide { None } else { width });
    request.face = FaceId(key.face);
    let mut shaped = if elide {
        s.engine.elide(&request, width.unwrap_or(f32::MAX) + t::UI_TEXT_WIDTH_EPSILON)
    } else {
        s.engine.layout(&request)
    }
    .ok()?;
    // Fixed role metrics from the Latin/Arabic union; content never moves the baseline.
    use skrifa::MetadataProvider as _;
    let mut ascent: f32 = 0.0;
    let mut descent: f32 = 0.0;
    for id in [FaceId(key.face), FaceId(3)] {
        if let Some(face) = s.engine.font_set().face(id) {
            if let Ok(font) = skrifa::FontRef::new(&face.bytes) {
                let m =
                    font.metrics(skrifa::instance::Size::new(request.size), skrifa::instance::LocationRef::default());
                ascent = ascent.max(m.ascent);
                descent = descent.max(-m.descent);
            }
        }
    }
    let line_height = (font.size * t::UI_TEXT_LEADING).max(ascent + descent);
    for (i, line) in shaped.lines.iter_mut().enumerate() {
        let baseline = i as f32 * line_height + (line_height - ascent - descent) / 2.0 + ascent;
        let dy = baseline - line.baseline;
        for g in &mut line.glyphs {
            g.y += dy;
        }
        line.baseline = baseline;
        line.ascent = ascent;
        line.descent = descent;
    }
    let height = shaped.lines.len().max(1) as f32 * line_height;
    let size = egui::vec2(shaped.lines.iter().map(|l| l.width).fold(0.0, f32::max), height);
    let bytes = key.text.len() * 2
        + shaped.carets.len() * std::mem::size_of::<varos_text::Caret>()
        + shaped.lines.iter().map(|l| l.glyphs.len() * std::mem::size_of::<varos_text::Glyph>()).sum::<usize>();
    let label = Label { layout: Arc::new(shaped), logical: Arc::from(key.text.as_str()), size };
    while !s.labels.is_empty() && (s.labels.len() >= 4096 || s.bytes + bytes > 4 * 1024 * 1024) {
        let Some(old) = s.labels.iter().min_by_key(|(_, c)| c.frame).map(|(k, _)| Key {
            text: k.text.clone(),
            size: k.size,
            face: k.face,
            width: k.width,
            elide: k.elide,
        }) else {
            break;
        };
        if let Some(c) = s.labels.remove(&old) {
            s.bytes -= c.bytes;
        }
    }
    if bytes <= 4 * 1024 * 1024 {
        s.bytes += bytes;
        s.labels.insert(key, Cached { label: label.clone(), frame, bytes });
    }
    Some(label)
}
impl System {
    fn reset(&mut self, ctx: &egui::Context, ppp: f32, options: egui::epaint::text::TextOptions) {
        self.ppp = ppp;
        self.options = options;
        self.glyphs.clear();
        self.x = 0;
        self.y = 0;
        self.row = 0;
        if let Some(old) = self.texture.take() {
            self.retired.push(old);
        }
        self.texture = Some(ctx.load_texture(
            "varos-ui-glyph-atlas",
            egui::ColorImage::filled([t::UI_ATLAS_SIDE; t::UI_ATLAS_DIMENSIONS], Color32::TRANSPARENT),
            egui::TextureOptions::LINEAR,
        ));
    }
    fn glyph(&mut self, ctx: &egui::Context, g: &varos_text::Glyph, bin: u8) -> Option<Entry> {
        let key = GlyphKey { face: g.face, id: g.id, size: (g.size * self.ppp).to_bits(), bin };
        if let Some(entry) = self.glyphs.get(&key) {
            return entry.clone();
        }
        let result = self
            .engine
            .font_set()
            .face(g.face)
            .and_then(|f| raster::raster(&f.bytes, g.id, g.size * self.ppp, bin, self.options));
        let entry = result.and_then(|b| {
            let [w, h] = b.image.size;
            if w + 1 > t::UI_ATLAS_SIDE || h + 1 > t::UI_ATLAS_SIDE {
                return None;
            }
            if self.x + w + 1 > t::UI_ATLAS_SIDE {
                self.x = 0;
                self.y += self.row + 1;
                self.row = 0;
            }
            if self.y + h + 1 > t::UI_ATLAS_SIDE {
                // Retain the old page until the next frame: earlier meshes still refer to it.
                self.reset(ctx, self.ppp, self.options);
            }
            let texture = self.texture.as_mut()?;
            texture.set_partial([self.x, self.y], b.image, egui::TextureOptions::LINEAR);
            let side = t::UI_ATLAS_SIDE as f32;
            let uv = Rect::from_min_max(
                egui::pos2(self.x as f32 / side, self.y as f32 / side),
                egui::pos2((self.x + w) as f32 / side, (self.y + h) as f32 / side),
            );
            self.x += w + 1;
            self.row = self.row.max(h);
            Some(Entry {
                texture: texture.id(),
                uv,
                offset: b.offset / self.ppp,
                size: egui::vec2(w as f32, h as f32) / self.ppp,
            })
        });
        self.glyphs.insert(key, entry.clone());
        entry
    }
}
pub fn paint(painter: &Painter, pos: Pos2, label: &Label, ink: Color32) {
    let ctx = painter.ctx();
    let Some(shared) = system(ctx) else { return };
    let Ok(mut s) = shared.lock() else { return };
    if let Some(trace) = &mut s.trace {
        let frame = ctx.cumulative_pass_nr();
        if trace.frame != frame {
            trace.frame = frame;
            trace.records.clear();
        }
        trace.records.push(PaintRecord {
            text: label.logical.to_string(),
            displayed_text: label.layout.source.clone(),
            elided: label.logical.as_ref() != label.layout.source,
            rect: Rect::from_min_size(pos, label.size()),
            lines: label.layout.lines.len(),
            clip_rect: painter.clip_rect(),
        });
    }
    let frame = ctx.cumulative_frame_nr();
    if s.atlas_frame != frame {
        s.retired.clear();
        s.atlas_frame = frame;
    }
    let ppp = ctx.pixels_per_point();
    let options = ctx.style_of(ctx.theme()).visuals.text_options;
    if s.texture.is_none() || s.ppp != ppp || s.options != options {
        s.reset(ctx, ppp, options);
    }
    let Some(texture) = &s.texture else { return };
    let mut mesh = egui::Mesh::with_texture(texture.id());
    for g in label.layout.lines.iter().flat_map(|l| &l.glyphs) {
        let px = ((pos.x + g.x + g.offset[0]) * ppp * 4.0).round() as i64;
        let bin = px.rem_euclid(4) as u8;
        if let Some(e) = s.glyph(ctx, g, bin) {
            if mesh.texture_id != e.texture {
                painter.add(egui::Shape::mesh(mesh));
                mesh = egui::Mesh::with_texture(e.texture);
            }
            let origin = egui::pos2(px.div_euclid(4) as f32 / ppp, ((pos.y + g.y + g.offset[1]) * ppp).round() / ppp);
            mesh.add_rect_with_uv(Rect::from_min_size(origin + e.offset, e.size), e.uv, ink);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}
pub fn label(ui: &mut egui::Ui, text: &str, font: FontId, ink: Color32) -> egui::Response {
    let text = crate::i18n::translate(ui.ctx(), text);
    let shaped = layout(ui.ctx(), &text, &font, Some(ui.available_width()), false);
    let measured = shaped.as_ref().map_or(Vec2::ZERO, Label::size);
    // Preserve the kit's existing box advance. The shared Latin/Arabic baseline is centred
    // within that cell; ink and field line metrics remain the shaper's, never char-count metrics.
    let nominal = ui.fonts_mut(|fonts| fonts.row_height(&font).ceil());
    let lines = shaped.as_ref().map_or(1, |s| s.layout.lines.len().max(1));
    let size = egui::vec2(measured.x, nominal * lines as f32);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    if let Some(shaped) = shaped {
        paint(ui.painter(), rect.min + egui::vec2(0.0, (rect.height() - shaped.size().y) / 2.0), &shaped, ink);
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), text.as_ref()));
    response
}

/// Opt-in headless paint evidence. Disabled in normal app operation; no text is retained outside the bounded layout cache.
pub fn enable_trace(ctx: &egui::Context) {
    if let Some(shared) = system(ctx) {
        if let Ok(mut s) = shared.lock() {
            s.trace = Some(Trace { frame: u64::MAX, records: vec![] });
        }
    }
}
pub fn paint_records(ctx: &egui::Context) -> Vec<PaintRecord> {
    system(ctx)
        .and_then(|shared| {
            shared.lock().ok().and_then(|s| {
                s.trace
                    .as_ref()
                    .filter(|t| t.frame.saturating_add(1) == ctx.cumulative_pass_nr())
                    .map(|t| t.records.clone())
            })
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
