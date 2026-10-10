//! Adapted from epaint 0.35.0 src/text/font.rs (MIT OR Apache-2.0),
//! https://github.com/emilk/egui — Skrifa/vello raster recipe and Y-down outline pen.
//! Removal condition: a public epaint API accepting shaped glyph IDs and font bytes.
use egui::{Color32, ColorImage};
use skrifa::{
    instance::{LocationRef, Size},
    outline::{DrawSettings, HintingInstance, OutlinePen},
    MetadataProvider,
};
use vello_cpu::{color, kurbo};

pub struct Bitmap {
    pub image: ColorImage,
    pub offset: egui::Vec2,
}
pub fn raster(
    bytes: &[u8],
    glyph: u16,
    size: f32,
    bin: u8,
    options: egui::epaint::text::TextOptions,
) -> Option<Bitmap> {
    let font = skrifa::FontRef::new(bytes).ok()?;
    let outlines = font.outline_glyphs();
    let outline = outlines.get(skrifa::GlyphId::new(u32::from(glyph)))?;
    let mut path = kurbo::BezPath::new();
    let mut pen = Pen { path: &mut path, x: f64::from(bin) / 4.0 };
    if options.font_hinting {
        let hint = HintingInstance::new(
            &outlines,
            Size::new(size),
            LocationRef::default(),
            skrifa::outline::Target::from(egui::epaint::text::HintingTarget::default()),
        )
        .ok()?;
        outline.draw(DrawSettings::hinted(&hint, false), &mut pen).ok()?;
    } else {
        outline.draw(DrawSettings::unhinted(Size::new(size), LocationRef::default()), &mut pen).ok()?;
    }
    let bounds = path.control_box().expand();
    let (w, h) = (bounds.width() as u16, bounds.height() as u16);
    if w == 0 || h == 0 || w > 1024 || h > 1024 {
        return None;
    }
    let mut context = vello_cpu::RenderContext::new(w, h);
    context.set_transform(kurbo::Affine::translate((-bounds.x0, -bounds.y0)));
    context.set_paint(color::OpaqueColor::<color::Srgb>::WHITE);
    context.fill_path(&path);
    let mut pixels = vello_cpu::Pixmap::new(w, h);
    context.render_to_pixmap(&mut vello_cpu::Resources::new(), &mut pixels);
    let mut image = ColorImage::filled([w as usize, h as usize], Color32::TRANSPARENT);
    for (dst, src) in image.pixels.iter_mut().zip(pixels.data_as_u8_slice().as_chunks::<4>().0) {
        *dst = options
            .color_transfer_function
            .to_atlas_color(Color32::from_rgba_premultiplied(src[0], src[1], src[2], src[3]));
    }
    Some(Bitmap { image, offset: egui::vec2(bounds.x0 as f32, bounds.y0 as f32) })
}
struct Pen<'a> {
    path: &'a mut kurbo::BezPath,
    x: f64,
}
impl OutlinePen for Pen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to((f64::from(x) + self.x, -f64::from(y)));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to((f64::from(x) + self.x, -f64::from(y)));
    }
    fn quad_to(&mut self, x: f32, y: f32, a: f32, b: f32) {
        self.path.quad_to((f64::from(x) + self.x, -f64::from(y)), (f64::from(a) + self.x, -f64::from(b)));
    }
    fn curve_to(&mut self, x: f32, y: f32, a: f32, b: f32, c: f32, d: f32) {
        self.path.curve_to(
            (f64::from(x) + self.x, -f64::from(y)),
            (f64::from(a) + self.x, -f64::from(b)),
            (f64::from(c) + self.x, -f64::from(d)),
        );
    }
    fn close(&mut self) {
        self.path.close_path();
    }
}
