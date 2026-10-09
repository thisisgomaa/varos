//! Pure CPU rasterisation of the core's renderer-independent scene description.

pub mod export;

mod clipboard;
pub use clipboard::clipboard_png;
use std::sync::Arc;
use tiny_skia::{
    FillRule, LineCap, LineJoin, Mask, MaskType, Paint, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform,
};
use varos_core::{build_scene, editor::Editor, geom::Rgba, model::Document, Group, Prim};

pub const WIDTH: u32 = 544;
pub const HEIGHT: u32 = 246;
const PAD: f32 = 22.0;
const GRID_PITCH: f32 = 24.0;

/// Premultiplied RGBA8 pixels, row-major. Tiny-skia provides deterministic antialiasing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub errors: Vec<String>,
}

fn failed_raster(errors: Vec<String>) -> Raster {
    Raster { width: 0, height: 0, pixels: vec![], errors }
}
impl Raster {
    /// Refuse scene failures before a caller can publish incomplete output.
    pub fn into_result(self) -> Result<Self, String> {
        if self.errors.is_empty() {
            Ok(self)
        } else {
            Err(self.errors.join("; "))
        }
    }
    pub fn encode_png(&self) -> Result<Vec<u8>, String> {
        if !self.errors.is_empty() {
            return Err(self.errors.join("; "));
        }
        Pixmap::from_vec(self.pixels.clone(), tiny_skia::IntSize::from_wh(self.width, self.height).ok_or("bad size")?)
            .ok_or_else(|| "bad pixel buffer".to_string())?
            .encode_png()
            .map_err(|e| e.to_string())
    }
}

/// Render artwork once at the physical canvas size, without selection overlays.
pub fn rasterize_canvas(snapshot: &Document, size: [u32; 2], pan: [f32; 2], ppu: f32) -> Raster {
    let mut editor = Editor::new();
    editor.replace_doc(snapshot.clone());
    let scene = build_scene(&editor, ppu);
    if !scene.errors.is_empty() {
        return failed_raster(scene.errors);
    }
    let mut pixmap = Pixmap::new(size[0].max(1), size[1].max(1)).expect("non-zero canvas size");
    pixmap.fill(tiny_skia::Color::from_rgba8(20, 19, 19, 255));
    draw_groups(&scene.content, &mut pixmap, Transform::from_row(ppu, 0.0, 0.0, ppu, pan[0], pan[1]));
    Raster { width: pixmap.width(), height: pixmap.height(), pixels: pixmap.take(), errors: vec![] }
}

impl Raster {
    /// Read a cached pixel; no scene construction or raster work occurs here.
    pub fn sample(&self, pixel: [f32; 2]) -> Option<Rgba> {
        if !self.errors.is_empty() {
            return None;
        }
        if !pixel.iter().all(|v| v.is_finite() && *v >= 0.0)
            || pixel[0] >= self.width as f32
            || pixel[1] >= self.height as f32
        {
            return None;
        }
        let at = (pixel[1] as usize * self.width as usize + pixel[0] as usize) * 4;
        let c = &self.pixels[at..at + 4];
        let alpha = c[3] as f32 / 255.0;
        let channel = |v: u8| if alpha == 0.0 { 0.0 } else { v as f32 / 255.0 / alpha };
        Some([channel(c[0]), channel(c[1]), channel(c[2]), alpha])
    }
}

#[cfg(test)]
fn sample_canvas(editor: &Editor, world: [f32; 2], ppu: f32) -> Rgba {
    rasterize_canvas(&editor.doc, [3, 3], [1.5 - world[0] * ppu, 1.5 - world[1] * ppu], ppu).sample([1.0, 1.0]).unwrap()
}

/// Render an immutable document snapshot to a dotted `#141313` well, fitting visible scene bounds.
pub fn rasterize(snapshot: Arc<Document>, size: [u32; 2]) -> Raster {
    let (w, h) = (size[0].max(1), size[1].max(1));
    let mut editor = Editor::new();
    editor.replace_doc(Arc::try_unwrap(snapshot).unwrap_or_else(|snapshot| (*snapshot).clone()));
    let scene = build_scene(&editor, 1.0);
    if !scene.errors.is_empty() {
        return failed_raster(scene.errors);
    }
    let bounds = scene_bounds(&scene.content);
    let (scale, ox, oy) = bounds.map_or((1.0, 0.0, 0.0), |b| fit(b, w, h));
    let xf = Transform::from_row(scale, 0.0, 0.0, scale, ox, oy);
    let mut pixmap = Pixmap::new(w, h).expect("non-zero raster size");
    pixmap.fill(tiny_skia::Color::from_rgba8(0x14, 0x13, 0x13, 0xff));
    draw_grid(&mut pixmap, scale);
    draw_groups(&scene.content, &mut pixmap, xf);
    Raster { width: w, height: h, pixels: pixmap.take(), errors: vec![] }
}

/// Render ONE page of an immutable document snapshot (Bridge slice 3): the image is exactly the page
/// rect `artboards[index]` (trim box, no bleed) at the page's aspect ratio, scaled to fit inside
/// `max_size`; the background is the page colour (transparent for a transparent page), and anything
/// outside the page is outside the image. The caller's document is not modified; other pages' papers
/// still paint in scene order, as on canvas. `None` for an unknown index or a degenerate page.
pub fn rasterize_artboard(snapshot: Arc<Document>, index: usize, max_size: [u32; 2]) -> Option<Raster> {
    rasterize_artboard_checked(snapshot, index, max_size).ok().flatten()
}

/// Checked page rendering for exporters: scene failures retain their diagnostic.
pub fn rasterize_artboard_checked(
    snapshot: Arc<Document>,
    index: usize,
    max_size: [u32; 2],
) -> Result<Option<Raster>, String> {
    let Some(page) = snapshot.artboards.get(index).cloned() else {
        return Ok(None);
    };
    let Some((size, scale)) = page_fit([page.w, page.h], max_size) else {
        return Ok(None);
    };
    let mut doc = Arc::try_unwrap(snapshot).unwrap_or_else(|snapshot| (*snapshot).clone());
    // a transparent page paints a faint ghost paper on canvas (`scene::AB_GHOST`) so it reads on the
    // dark board; a page image has no board behind it, so that canvas aid is dropped from this copy
    for ab in &mut doc.artboards {
        ab.page_color.get_or_insert([0.0; 4]);
    }
    // the background is painted ONCE, by the fill below; the scene's own paper for this page is made
    // fully transparent in the copy, or a translucent page colour would composite over itself (review
    // P2). The fill also keeps the background for a hidden page, whose paper the scene skips.
    doc.artboards[index].page_color = Some([0.0; 4]);
    let mut editor = Editor::new();
    editor.replace_doc(doc);
    let scene = build_scene(&editor, 1.0);
    if !scene.errors.is_empty() {
        return Err(scene.errors.join("; "));
    }
    let xf = Transform::from_row(scale, 0.0, 0.0, scale, -page.x * scale, -page.y * scale);
    let Some(mut pixmap) = Pixmap::new(size[0], size[1]) else {
        return Err("raster allocation failed".into());
    };
    if let Some(c) = page.page_color {
        if let Some(color) = tiny_skia::Color::from_rgba(c[0], c[1], c[2], c[3]) {
            pixmap.fill(color);
        }
    }
    draw_groups(&scene.content, &mut pixmap, xf);
    Ok(Some(Raster { width: size[0], height: size[1], pixels: pixmap.take(), errors: vec![] }))
}

/// The pixel size of a `page` (w, h in points) fitted inside `max` at its own aspect ratio, and the
/// points→pixels scale. Each side is at least 1 pixel.
fn page_fit(page: [f32; 2], max: [u32; 2]) -> Option<([u32; 2], f32)> {
    let [pw, ph] = page;
    if !(pw.is_finite() && ph.is_finite() && pw > 0.0 && ph > 0.0) {
        return None;
    }
    let (mw, mh) = (max[0].max(1) as f32, max[1].max(1) as f32);
    let scale = (mw / pw).min(mh / ph);
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let w = (pw * scale).round().clamp(1.0, mw) as u32;
    let h = (ph * scale).round().clamp(1.0, mh) as u32;
    Some(([w, h], scale))
}

fn fit(b: [f32; 4], w: u32, h: u32) -> (f32, f32, f32) {
    if !b.iter().all(|v| v.is_finite()) {
        return (1.0, 0.0, 0.0);
    }
    let bw = (b[2] - b[0]).abs().clamp(1.0e-6, 1.0e30);
    let bh = (b[3] - b[1]).abs().clamp(1.0e-6, 1.0e30);
    let avail_w = (w as f32 - PAD * 2.0).max(1.0);
    let avail_h = (h as f32 - PAD * 2.0).max(1.0);
    let s = (avail_w / bw).min(avail_h / bh).clamp(1.0e-12, 1.0e6);
    let ox = (w as f32 - bw * s) * 0.5 - b[0] * s;
    let oy = (h as f32 - bh * s) * 0.5 - b[1] * s;
    if [s, ox, oy].iter().all(|v| v.is_finite()) {
        (s, ox, oy)
    } else {
        (1.0, 0.0, 0.0)
    }
}

fn draw_grid(dst: &mut Pixmap, scale: f32) {
    let pitch = (GRID_PITCH * scale.clamp(0.5, 2.0)).clamp(12.0, 48.0);
    let mut paint = Paint::default();
    paint.set_color_rgba8(255, 255, 255, 12);
    let mut y = pitch * 0.5;
    while y < dst.height() as f32 {
        let mut x = pitch * 0.5;
        while x < dst.width() as f32 {
            let mut dot = PathBuilder::new();
            dot.push_circle(x, y, 0.65);
            if let Some(dot) = dot.finish() {
                dst.fill_path(&dot, &paint, FillRule::Winding, Transform::identity(), None);
            }
            x += pitch;
        }
        y += pitch;
    }
}

fn draw_groups(groups: &[Group], dst: &mut Pixmap, xf: Transform) {
    for group in groups {
        match group {
            Group::Opaque(prims) => draw_prims(prims, dst, xf),
            Group::Knockout(prims) => draw_knockout(prims, dst, xf),
            Group::Isolated { opacity, prims } => {
                let mut layer = Pixmap::new(dst.width(), dst.height()).unwrap();
                if prims.iter().any(|p| matches!(p,Prim::StrokeCoverage {color,..} if color[3]<0.999))
                    && prims.iter().any(|p| matches!(p, Prim::Fill { .. }))
                {
                    draw_knockout(prims, &mut layer, xf);
                } else {
                    draw_prims(prims, &mut layer, xf);
                }
                let paint = PixmapPaint { opacity: *opacity, ..PixmapPaint::default() };
                dst.draw_pixmap(0, 0, layer.as_ref(), &paint, Transform::identity(), None);
            }
            Group::Clip { mask_rings, members } => {
                let mut layer = Pixmap::new(dst.width(), dst.height()).unwrap();
                draw_groups(members, &mut layer, xf);
                let mut mask = Mask::new(dst.width(), dst.height()).unwrap();
                if let Some(path) = rings_path(mask_rings, false) {
                    mask.fill_path(&path, FillRule::EvenOdd, true, xf);
                }
                layer.apply_mask(&mask);
                dst.draw_pixmap(0, 0, layer.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
            }
        }
    }
}

fn draw_knockout(prims: &[Prim], dst: &mut Pixmap, xf: Transform) {
    let mut fill_layer = Pixmap::new(dst.width(), dst.height()).unwrap();
    let fills: Vec<_> =
        prims.iter().filter(|p| !matches!(p, Prim::Stroke { .. } | Prim::StrokeCoverage { .. })).cloned().collect();
    draw_prims(&fills, &mut fill_layer, xf);

    let coverage = stroke_coverage(prims, dst.width(), dst.height(), xf, None);
    let mut inverse = Mask::from_pixmap(coverage.as_ref(), MaskType::Alpha);
    inverse.invert();
    fill_layer.apply_mask(&inverse);
    dst.draw_pixmap(0, 0, fill_layer.as_ref(), &PixmapPaint::default(), Transform::identity(), None);

    let mut colors: Vec<Rgba> = Vec::new();
    for color in prims.iter().filter_map(|p| match p {
        Prim::Stroke { color, .. } | Prim::StrokeCoverage { color, .. } => Some(*color),
        _ => None,
    }) {
        if !colors.contains(&color) {
            colors.push(color);
        }
    }
    for color in colors {
        let coverage = stroke_coverage(prims, dst.width(), dst.height(), xf, Some(color));
        let mut layer = Pixmap::new(dst.width(), dst.height()).unwrap();
        layer.fill(tiny_skia::Color::TRANSPARENT);
        let Some(rect) = tiny_skia::Rect::from_xywh(0.0, 0.0, dst.width() as f32, dst.height() as f32) else {
            continue;
        };
        layer.fill_rect(
            rect,
            &paint(color),
            Transform::identity(),
            Some(&Mask::from_pixmap(coverage.as_ref(), MaskType::Alpha)),
        );
        dst.draw_pixmap(0, 0, layer.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
    }
}

fn stroke_coverage(prims: &[Prim], width: u32, height: u32, xf: Transform, only: Option<Rgba>) -> Pixmap {
    let mut coverage = Pixmap::new(width, height).unwrap();
    let mut white = Paint::default();
    white.set_color_rgba8(255, 255, 255, 255);
    for prim in prims {
        if let Prim::StrokeCoverage { rings, color, clip, native } = prim {
            if only.is_some_and(|wanted| wanted != *color) {
                continue;
            }
            let mask = clip.and_then(|r| rect_mask(width, height, r, xf));
            styled_coverage(&mut coverage, rings, native.as_ref(), &white, xf, mask.as_ref());
            continue;
        }
        let Prim::Stroke { pts, width: stroke_width, color, clip } = prim else { continue };
        if only.is_some_and(|wanted| wanted != *color) {
            continue;
        }
        let Some(path) = line_path(pts) else { continue };
        let stroke =
            Stroke { width: *stroke_width, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Stroke::default() };
        let mask = clip.and_then(|r| rect_mask(width, height, r, xf));
        coverage.stroke_path(&path, &white, &stroke, xf, mask.as_ref());
    }
    coverage
}

fn draw_prims(prims: &[Prim], dst: &mut Pixmap, xf: Transform) {
    for prim in prims {
        match prim {
            Prim::Fill { rings, color } => {
                if let Some(path) = rings_path(rings, false) {
                    dst.fill_path(&path, &paint(*color), FillRule::EvenOdd, xf, None);
                }
            }
            Prim::StrokeCoverage { rings, color, clip, native } => {
                let mask = clip.and_then(|r| rect_mask(dst.width(), dst.height(), r, xf));
                styled_coverage(dst, rings, native.as_ref(), &paint(*color), xf, mask.as_ref());
            }
            Prim::Stroke { pts, width, color, clip } => {
                if let Some(path) = line_path(pts) {
                    let stroke = Stroke {
                        width: *width,
                        line_cap: LineCap::Round,
                        line_join: LineJoin::Round,
                        ..Stroke::default()
                    };
                    let mask = clip.and_then(|r| rect_mask(dst.width(), dst.height(), r, xf));
                    dst.stroke_path(&path, &paint(*color), &stroke, xf, mask.as_ref());
                }
            }
            Prim::Dashed { pts, width, color } => {
                if let Some(path) = line_path(pts) {
                    let stroke = Stroke {
                        width: *width,
                        line_cap: LineCap::Round,
                        line_join: LineJoin::Round,
                        ..Stroke::default()
                    };
                    dst.stroke_path(&path, &paint(*color), &stroke, xf, None);
                }
            }
            Prim::Square { c, half, color } => {
                if let Some(r) = tiny_skia::Rect::from_xywh(c[0] - half, c[1] - half, half * 2.0, half * 2.0) {
                    dst.fill_rect(r, &paint(*color), xf, None);
                }
            }
            Prim::Disc { c, r, color } => {
                let mut pb = PathBuilder::new();
                pb.push_circle(c[0], c[1], *r);
                if let Some(path) = pb.finish() {
                    dst.fill_path(&path, &paint(*color), FillRule::Winding, xf, None);
                }
            }
            Prim::Tri { a, b, c, color } => {
                let mut pb = PathBuilder::new();
                pb.move_to(a[0], a[1]);
                pb.line_to(b[0], b[1]);
                pb.line_to(c[0], c[1]);
                pb.close();
                if let Some(path) = pb.finish() {
                    dst.fill_path(&path, &paint(*color), FillRule::Winding, xf, None);
                }
            }
        }
    }
}

fn rect_mask(width: u32, height: u32, rect: [f32; 4], xf: Transform) -> Option<Mask> {
    let rings = vec![vec![[rect[0], rect[1]], [rect[2], rect[1]], [rect[2], rect[3]], [rect[0], rect[3]]]];
    let mut mask = Mask::new(width, height)?;
    mask.fill_path(&rings_path(&rings, false)?, FillRule::Winding, true, xf);
    Some(mask)
}

fn paint(c: Rgba) -> Paint<'static> {
    let mut p = Paint::default();
    if let Some(color) = tiny_skia::Color::from_rgba(c[0], c[1], c[2], c[3]) {
        p.set_color(color);
    }
    p.anti_alias = true;
    p
}

fn rings_path(rings: &[Vec<[f32; 2]>], open: bool) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    for ring in rings {
        let Some(first) = ring.first() else { continue };
        pb.move_to(first[0], first[1]);
        for p in &ring[1..] {
            pb.line_to(p[0], p[1]);
        }
        if !open {
            pb.close();
        }
    }
    pb.finish()
}

fn line_path(pts: &[[f32; 2]]) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    let first = pts.first()?;
    pb.move_to(first[0], first[1]);
    for p in &pts[1..] {
        pb.line_to(p[0], p[1]);
    }
    pb.finish()
}

fn scene_bounds(groups: &[Group]) -> Option<[f32; 4]> {
    fn visit(group: &Group, out: &mut Option<[f32; 4]>) {
        let prims = match group {
            Group::Opaque(p) | Group::Knockout(p) | Group::Isolated { prims: p, .. } => p,
            Group::Clip { mask_rings, members } => {
                let mut member_bounds = None;
                members.iter().for_each(|g| visit(g, &mut member_bounds));
                let mut mask_bounds = None;
                mask_rings.iter().flatten().for_each(|p| include(&mut mask_bounds, *p));
                if let (Some(a), Some(m)) = (member_bounds, mask_bounds) {
                    let clipped = [a[0].max(m[0]), a[1].max(m[1]), a[2].min(m[2]), a[3].min(m[3])];
                    if clipped[0] <= clipped[2] && clipped[1] <= clipped[3] {
                        include(out, [clipped[0], clipped[1]]);
                        include(out, [clipped[2], clipped[3]]);
                    }
                }
                return;
            }
        };
        for prim in prims {
            let pts: Box<dyn Iterator<Item = &[f32; 2]> + '_> = match prim {
                Prim::Fill { rings, .. } | Prim::StrokeCoverage { rings, .. } => Box::new(rings.iter().flatten()),
                Prim::Stroke { pts, .. } | Prim::Dashed { pts, .. } => Box::new(pts.iter()),
                Prim::Square { c, .. } | Prim::Disc { c, .. } => Box::new(std::iter::once(c)),
                Prim::Tri { a, b, c, .. } => Box::new([a, b, c].into_iter()),
            };
            for p in pts {
                include(out, *p);
            }
        }
    }
    let mut out = None;
    groups.iter().for_each(|g| visit(g, &mut out));
    out
}

fn include(b: &mut Option<[f32; 4]>, p: [f32; 2]) {
    if !p.iter().all(|v| v.is_finite()) {
        return;
    }
    *b = Some(match *b {
        Some(v) => [v[0].min(p[0]), v[1].min(p[1]), v[2].max(p[0]), v[3].max(p[1])],
        None => [p[0], p[1], p[0], p[1]],
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use varos_core::model::{Anchor, Artboard, Path};

    fn rect(id: u32, xy: [f32; 2], wh: [f32; 2], color: Rgba) -> Path {
        let points = [[xy[0], xy[1]], [xy[0] + wh[0], xy[1]], [xy[0] + wh[0], xy[1] + wh[1]], [xy[0], xy[1] + wh[1]]];
        Path::new(
            id,
            points
                .into_iter()
                .enumerate()
                .map(|(i, p)| Anchor { id: id * 10 + i as u32, p, hin: None, hout: None, smooth: false })
                .collect(),
            true,
            Some(color),
            None,
            0.0,
        )
    }

    fn pixel(r: &Raster, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * r.width + x) * 4) as usize;
        r.pixels[i..i + 4].try_into().unwrap()
    }

    fn white_board() -> Document {
        let mut d = Document::default();
        d.artboards.push(Artboard { x: 0.0, y: 0.0, w: 100.0, h: 100.0, ..Artboard::default() });
        d
    }

    #[test]
    fn canvas_eyedropper_samples_rendered_fixture_not_selection_overlay() {
        let mut doc = white_board();
        doc.paths.push(rect(2, [10.0, 10.0], [30.0, 30.0], [1.0, 0.0, 0.0, 1.0]));
        let mut translucent = rect(3, [20.0, 20.0], [10.0, 10.0], [0.0, 0.0, 1.0, 1.0]);
        translucent.opacity = 0.5;
        doc.paths.push(translucent);
        let mut ed = Editor::new();
        ed.replace_doc(doc);
        ed.objsel.insert(2);
        for zoom in [0.5, 1.0, 2.0, 4.0] {
            assert_eq!(sample_canvas(&ed, [15.0, 15.0], zoom), [1.0, 0.0, 0.0, 1.0]);
            assert_eq!(sample_canvas(&ed, [60.0, 60.0], zoom), [1.0; 4]);
            let blended = sample_canvas(&ed, [25.0, 25.0], zoom);
            assert!((blended[0] - 0.5).abs() < 0.01 && (blended[2] - 0.5).abs() < 0.01, "{blended:?}");
            assert_eq!(blended[1], 0.0);
        }
        ed.doc.paths[1].hidden = true;
        assert_eq!(sample_canvas(&ed, [25.0, 25.0], 2.0), [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn red_square_on_white_artboard_has_expected_pixels() {
        let mut d = white_board();
        d.paths.push(rect(2, [20.0, 20.0], [20.0, 20.0], [1.0, 0.0, 0.0, 1.0]));
        let r = rasterize(Arc::new(d), [100, 100]);
        assert_eq!(pixel(&r, 25, 25), [255, 255, 255, 255]);
        let red = pixel(&r, 38, 38);
        assert!(red[0] > 245 && red[1] < 10 && red[2] < 10, "{red:?}");
    }

    #[test]
    fn clip_group_hides_outside_pixels() {
        let mut d = Document::default();
        d.paths.push(rect(2, [0.0, 0.0], [50.0, 100.0], [1.0, 1.0, 1.0, 1.0]));
        d.paths.push(rect(3, [0.0, 0.0], [100.0, 100.0], [1.0, 0.0, 0.0, 1.0]));
        d.sync_tree();
        d.clip_group(&[2, 3], 2).unwrap();
        let r = rasterize(Arc::new(d), [100, 100]);
        assert!(pixel(&r, 50, 50)[0] > 240);
        assert_eq!(pixel(&r, 90, 50)[0..3], [20, 19, 19]);
    }

    #[test]
    fn hidden_object_is_absent() {
        let mut d = white_board();
        let mut p = rect(2, [0.0, 0.0], [100.0, 100.0], [1.0, 0.0, 0.0, 1.0]);
        p.hidden = true;
        d.paths.push(p);
        assert_eq!(pixel(&rasterize(Arc::new(d), [100, 100]), 50, 50), [255, 255, 255, 255]);
    }

    #[test]
    fn zero_artboard_fits_free_artwork() {
        let mut d = Document::default();
        d.paths.push(rect(2, [500.0, -300.0], [20.0, 10.0], [0.0, 1.0, 0.0, 1.0]));
        let p = pixel(&rasterize(Arc::new(d), [100, 100]), 50, 50);
        assert!(p[1] > 245 && p[0] < 10, "{p:?}");
    }

    #[test]
    fn empty_board_is_only_dotted_well() {
        let r = rasterize(Arc::new(Document::default()), [100, 100]);
        assert_eq!(pixel(&r, 1, 1), [20, 19, 19, 255]);
        assert_ne!(pixel(&r, 12, 12), [20, 19, 19, 255]);
    }

    #[test]
    fn raster_and_png_are_deterministic() {
        let d = white_board();
        let a = rasterize(Arc::new(d.clone()), [100, 100]);
        let b = rasterize(Arc::new(d), [100, 100]);
        assert_eq!(a, b);
        assert_eq!(a.encode_png().unwrap(), b.encode_png().unwrap());
    }

    #[test]
    fn knockout_stroke_blends_with_backdrop_not_its_fill() {
        let ring = vec![[20.0, 20.0], [80.0, 20.0], [80.0, 80.0], [20.0, 80.0]];
        let mut edge = ring.clone();
        edge.push(ring[0]);
        let group = Group::Knockout(vec![
            Prim::Fill { rings: vec![ring], color: [1.0, 1.0, 1.0, 1.0] },
            Prim::Stroke { pts: edge, width: 20.0, color: [1.0, 0.0, 0.0, 0.5], clip: None },
        ]);
        let mut pixmap = Pixmap::new(100, 100).unwrap();
        pixmap.fill(tiny_skia::Color::from_rgba8(20, 19, 19, 255));
        draw_groups(&[group], &mut pixmap, Transform::identity());
        let r = Raster { width: 100, height: 100, pixels: pixmap.take(), errors: vec![] };
        assert_eq!(pixel(&r, 50, 50), [255, 255, 255, 255]);
        let band = pixel(&r, 20, 50);
        assert!(band[0] > 125 && band[0] < 145 && band[1] < 15, "{band:?}");
    }

    #[test]
    fn artboard_raster_is_the_page_at_its_ratio_with_its_background() {
        let mut d = Document::default();
        d.artboards.push(Artboard { x: 100.0, y: 50.0, w: 1080.0, h: 1920.0, ..Artboard::default() });
        d.artboards.push(Artboard { x: 2000.0, w: 100.0, h: 100.0, page_color: None, ..Artboard::default() });
        // a red square in the page's top-left quarter, and a green one off every page
        d.paths.push(rect(1, [100.0, 50.0], [540.0, 960.0], [1.0, 0.0, 0.0, 1.0]));
        d.paths.push(rect(2, [-500.0, -500.0], [100.0, 100.0], [0.0, 1.0, 0.0, 1.0]));
        d.ids = 100;
        let r = rasterize_artboard(Arc::new(d.clone()), 0, [1024, 1024]).unwrap();
        assert_eq!((r.width, r.height), (576, 1024), "9:16 story fitted inside 1024×1024");
        assert_eq!(pixel(&r, 10, 10), [255, 0, 0, 255], "art at the page's top-left");
        assert_eq!(pixel(&r, 500, 900), [255, 255, 255, 255], "white page background");
        let r = rasterize_artboard(Arc::new(d.clone()), 1, [64, 32]).unwrap();
        assert_eq!((r.width, r.height), (32, 32));
        assert_eq!(pixel(&r, 16, 16), [0, 0, 0, 0], "a transparent page has a transparent background");
        // a half-transparent page colour is painted once: alpha 0.5, not 0.75 (review P2)
        let mut half = d.clone();
        half.artboards[1].page_color = Some([1.0, 0.0, 0.0, 0.5]);
        let r = rasterize_artboard(Arc::new(half.clone()), 1, [32, 32]).unwrap();
        let [red, g, b, a] = pixel(&r, 16, 16);
        assert!((127..=128).contains(&a) && (127..=128).contains(&red) && g == 0 && b == 0, "{:?}", [red, g, b, a]);
        // and the opaque page keeps exactly its colour with art on top unchanged
        assert_eq!(
            pixel(&rasterize_artboard(Arc::new(half.clone()), 0, [1024, 1024]).unwrap(), 500, 900),
            [255, 255, 255, 255]
        );
        // a hidden page still gets its background
        half.artboards[1].hidden = true;
        assert_eq!(pixel(&rasterize_artboard(Arc::new(half), 1, [32, 32]).unwrap(), 16, 16)[3], a);
        assert!(rasterize_artboard(Arc::new(d), 2, [64, 64]).is_none());
        assert_eq!(page_fit([f32::NAN, 1.0], [10, 10]), None);
        assert_eq!(page_fit([1.0, 1.0e9], [10, 10]).unwrap().0, [1, 10]);
    }

    #[test]
    fn fit_rejects_nonfinite_and_bounds_extreme_values() {
        assert_eq!(fit([f32::NAN, 0.0, 1.0, 1.0], 100, 100), (1.0, 0.0, 0.0));
        for bounds in [[0.0, 0.0, 0.0, 0.0], [-1.0e30, -1.0e30, 1.0e30, 1.0e30]] {
            let fitted = fit(bounds, 1, 1);
            assert!([fitted.0, fitted.1, fitted.2].into_iter().all(f32::is_finite), "{fitted:?}");
        }
    }

    #[test]
    #[ignore = "writes moderator review artifacts to the requested scratchpad"]
    fn render_three_frozen_fixture_samples() {
        let fixtures = ["v1/v1_plain.vrs", "v1/v1_masked.vrs", "v2/v2_boardless.vrs"];
        let out = std::env::var_os("VAROS_THUMB_SAMPLE_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("varos-thumb-samples"));
        std::fs::create_dir_all(&out).unwrap();
        let core = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures");
        for (i, fixture) in fixtures.iter().enumerate() {
            let doc = varos_pdf::load_vrs(&core.join(fixture)).unwrap();
            let png = rasterize(Arc::new(doc), [WIDTH, HEIGHT]).encode_png().unwrap();
            std::fs::write(out.join(format!("thumbs-sample-{}.png", i + 1)), png).unwrap();
        }
    }

    #[test]
    #[ignore = "performance probe; run with --release --ignored --nocapture"]
    fn rasterizes_two_thousand_paths_under_release_budget() {
        let mut d = Document::default();
        for i in 0..2_000 {
            d.paths.push(rect(i + 2, [(i % 50) as f32 * 3.0, (i / 50) as f32 * 3.0], [2.0, 2.0], [1.0, 0.2, 0.1, 1.0]));
        }
        let start = Instant::now();
        let _ = rasterize(Arc::new(d), [WIDTH, HEIGHT]);
        let elapsed = start.elapsed();
        eprintln!("2,000-path thumbnail: {elapsed:?}");
        assert!(elapsed.as_secs_f32() < 1.5, "{elapsed:?}");
    }

    #[test]
    #[ignore = "headless 5k-path clone/raster/encode timing probe; run --release --ignored --nocapture"]
    fn snapshot_five_thousand_paths_timings() {
        let mut doc = Document::default();
        for i in 0..5_000 {
            doc.paths.push(rect(
                i * 10 + 2,
                [(i % 100) as f32 * 3.0, (i / 100) as f32 * 3.0],
                [2.0, 2.0],
                [1.0, 0.2, 0.1, 1.0],
            ));
        }
        doc.sync_tree();
        for size in [[544, 246], [1024, 1024]] {
            let _ = rasterize(Arc::new(doc.clone()), size).encode_png().unwrap();
            for sample in 0..5 {
                let start = Instant::now();
                let snapshot = Arc::new(doc.clone());
                let clone_time = start.elapsed();
                let raster_start = Instant::now();
                let raster = rasterize(snapshot, size);
                let raster_time = raster_start.elapsed();
                let encode_start = Instant::now();
                let png = raster.encode_png().unwrap();
                let encode_time = encode_start.elapsed();
                eprintln!("snapshot {}x{} sample={sample} clone_ms={:.3} raster_ms={:.3} encode_ms={:.3} total_ms={:.3} bytes={}", size[0], size[1], clone_time.as_secs_f64()*1000.0, raster_time.as_secs_f64()*1000.0, encode_time.as_secs_f64()*1000.0, start.elapsed().as_secs_f64()*1000.0, png.len());
            }
        }
    }

    /// For the Start snapshot only: `VAROS_START_THUMBS=<dir>` writes real thumbnails (this rasteriser,
    /// the cache's 544 × 246) for four of the demo boards as `<index>.png`. Not a gate.
    #[test]
    fn start_snapshot_thumbnails() {
        let Some(dir) = std::env::var_os("VAROS_START_THUMBS") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let navy = [0.086, 0.137, 0.239, 1.0];
        let amber = [0.914, 0.769, 0.416, 1.0];
        let cream = [0.925, 0.894, 0.839, 1.0];
        let orange = [0.894, 0.341, 0.180, 1.0];
        let ink = [0.078, 0.075, 0.075, 1.0];
        let green = [0.122, 0.302, 0.227, 1.0];
        let gold = [0.890, 0.690, 0.294, 1.0];
        type Boards = Vec<([f32; 2], [f32; 2])>;
        type Rects = Vec<([f32; 2], [f32; 2], Rgba)>;
        let boards: [(usize, Boards, Rects); 4] = [
            (
                0,
                vec![([0.0, 0.0], [1080.0, 1350.0]), ([1240.0, 0.0], [1080.0, 1920.0])],
                vec![
                    ([0.0, 0.0], [1080.0, 1350.0], navy),
                    ([560.0, 220.0], [300.0, 300.0], amber),
                    ([110.0, 820.0], [700.0, 120.0], cream),
                    ([1240.0, 0.0], [1080.0, 1920.0], navy),
                    ([1700.0, 360.0], [420.0, 420.0], amber),
                    ([1350.0, 1680.0], [420.0, 80.0], amber),
                ],
            ),
            (
                1,
                vec![],
                vec![
                    ([50.0, 70.0], [500.0, 500.0], cream),
                    ([690.0, 70.0], [540.0, 500.0], orange),
                    ([1410.0, 110.0], [420.0, 420.0], cream),
                    ([2040.0, 130.0], [520.0, 100.0], cream),
                    ([2040.0, 290.0], [380.0, 100.0], cream),
                    ([2040.0, 450.0], [240.0, 100.0], orange),
                ],
            ),
            (
                2,
                vec![([0.0, 0.0], [842.0, 1191.0])],
                vec![
                    ([0.0, 0.0], [842.0, 1191.0], cream),
                    ([300.0, 160.0], [480.0, 480.0], orange),
                    ([70.0, 660.0], [470.0, 440.0], ink),
                ],
            ),
            (
                8,
                vec![([0.0, 0.0], [1080.0, 1080.0])],
                vec![
                    ([0.0, 0.0], [1080.0, 1080.0], green),
                    ([370.0, 260.0], [340.0, 340.0], gold),
                    ([260.0, 820.0], [560.0, 90.0], cream),
                ],
            ),
        ];
        for (index, artboards, rects) in boards {
            let mut d = Document::default();
            for (xy, wh) in artboards {
                d.artboards.push(Artboard { x: xy[0], y: xy[1], w: wh[0], h: wh[1], ..Artboard::default() });
            }
            for (i, (xy, wh, color)) in rects.into_iter().enumerate() {
                d.paths.push(rect(2 + i as u32, xy, wh, color));
            }
            let r = rasterize(Arc::new(d), [WIDTH, HEIGHT]);
            std::fs::write(dir.join(format!("{index}.png")), r.encode_png().unwrap()).unwrap();
        }
    }
}

#[cfg(test)]
#[path = "svg_tests.rs"]
mod svg_tests;

#[test]
fn clip_preserves_object_opacity_expected_blend() {
    use varos_core::model::{Anchor, Path};
    let square = |id, size| {
        Path::new(
            id,
            [[0., 0.], [size, 0.], [size, size], [0., size]]
                .into_iter()
                .enumerate()
                .map(|(i, p)| Anchor { id: id * 10 + i as u32, p, hin: None, hout: None, smooth: false })
                .collect(),
            true,
            Some([1., 0., 0., 1.]),
            None,
            0.,
        )
    };
    let mut doc = Document::default();
    let mut art = square(10, 20.);
    art.opacity = 0.5;
    art.stroke = varos_core::model::Paint::Solid([0., 0., 1., 1.]);
    art.stroke_width = 2.;
    doc.paths = vec![art, square(11, 10.)];
    doc.ids = 200;
    doc.sync_tree();
    doc.clip_group(&[10, 11], 11).unwrap();
    let mut editor = Editor::new();
    editor.replace_doc(doc.clone());
    let scene = build_scene(&editor, 1.);
    assert!(
        matches!(&scene.content[0],Group::Clip {members,..} if matches!(members[0],Group::Isolated {opacity:0.5,..}))
    );
    let clipped = rasterize_canvas(&doc, [25, 25], [0., 0.], 1.);
    let p = clipped.sample([5., 5.]).unwrap();
    assert!((p[0] - (0.5 + 20. / 255. * 0.5)).abs() < 2. / 255.);
    assert!((p[1] - 19. / 255. * 0.5).abs() < 2. / 255.);
    // Compare with the same isolated object outside the mask container.
    doc.release_clip(doc.clip_group_of(10).unwrap());
    doc.paths.retain(|p| p.id == 10);
    doc.sync_tree();
    let outside = rasterize_canvas(&doc, [25, 25], [0., 0.], 1.);
    // The clip's extra premultiplied intermediate can round by one RGBA8 quantum.
    for (a, b) in clipped.sample([5., 5.]).unwrap().into_iter().zip(outside.sample([5., 5.]).unwrap()) {
        assert!((a - b).abs() <= 1. / 255. + f32::EPSILON);
    }
    assert_ne!(clipped.sample([15., 15.]), outside.sample([15., 15.]));
}

fn styled_coverage(
    dst: &mut Pixmap,
    rings: &[Vec<[f32; 2]>],
    native: Option<&varos_core::scene::NativeStroke>,
    paint: &Paint,
    xf: Transform,
    mask: Option<&Mask>,
) {
    use varos_core::stroke::{StrokeCap, StrokeJoin};
    if let Some(native) = native {
        let s = &native.style;
        let stroke = Stroke {
            width: native.width,
            miter_limit: s.miter_limit,
            line_cap: match s.cap {
                StrokeCap::Butt => LineCap::Butt,
                StrokeCap::Round => LineCap::Round,
                StrokeCap::Square => LineCap::Square,
            },
            line_join: match s.join {
                StrokeJoin::Miter => LineJoin::Miter,
                StrokeJoin::Round => LineJoin::Round,
                StrokeJoin::Bevel => LineJoin::Bevel,
            },
            dash: if s.dash.is_empty() { None } else { tiny_skia::StrokeDash::new(s.dash.clone(), s.dash_phase) },
        };
        // One native path paints all subpaths once, so translucent overlapping contours are a union.
        let mut builder = PathBuilder::new();
        for pts in &native.contours {
            if let Some(first) = pts.first() {
                builder.move_to(first[0], first[1]);
                for point in &pts[1..] {
                    builder.line_to(point[0], point[1]);
                }
                if pts.first() == pts.last() {
                    builder.close();
                }
            }
        }
        if let Some(path) = builder.finish() {
            dst.stroke_path(&path, paint, &stroke, xf, mask);
        }
    } else if let Some(path) = rings_path(rings, false) {
        dst.fill_path(&path, paint, FillRule::EvenOdd, xf, mask);
    }
}

#[cfg(test)]
mod stroke_failure_tests {
    use super::*;
    #[test]
    fn rotated_dash_world_budget_refuses_pixels_png_canvas_and_page() {
        use varos_core::{
            model::{Anchor, Path, Xform},
            stroke::{StrokeCap, StrokeStyle},
        };
        let mut doc = Document::default();
        let anchors = [[0.0, 0.0], [4000.0, 0.0]]
            .into_iter()
            .enumerate()
            .map(|(i, p)| Anchor { id: 11 + i as u32, p, hin: None, hout: None, smooth: false })
            .collect();
        let mut path = Path::new(10, anchors, false, None, Some([0.0, 0.0, 0.0, 1.0]), 1.0);
        path.stroke_style = StrokeStyle { cap: StrokeCap::Butt, dash: vec![1.0, 1.0], ..Default::default() };
        let coverage = varos_core::stroke::evaluate(&path, 0.025, &|| false).unwrap();
        assert!(varos_core::stroke::evaluate::triangles(&coverage.rings).is_ok());
        doc.paths.push(path);
        doc.ids = 100;
        doc.sync_tree();
        let unit = doc.unit_of(10).unwrap();
        doc.set_node_xform(unit, Xform { rot: 0.7, piv: [0.0, 0.0] });
        let image = rasterize(Arc::new(doc.clone()), [100, 100]);
        assert!(image.errors[0].contains("limit_exceeded"));
        assert!(image.pixels.is_empty());
        assert!(image.encode_png().is_err());
        assert!(image.sample([0.0, 0.0]).is_none());
        assert!(image.into_result().is_err());
        assert!(rasterize_canvas(&doc, [100, 100], [0.0, 0.0], 1.0).into_result().is_err());
        doc.artboards.push(varos_core::model::Artboard { w: 6000.0, h: 6000.0, clip: false, ..Default::default() });
        assert!(rasterize_artboard(Arc::new(doc.clone()), 0, [100, 100]).is_none());
        assert!(rasterize_artboard_checked(Arc::new(doc), 0, [100, 100]).unwrap_err().contains("limit_exceeded"));
    }
}

// ---- Lane C ----
pub mod screens;
