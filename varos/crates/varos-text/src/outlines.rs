//! Unhinted Skrifa quadratics -> exact cubics -> Y-down points.
//! Native winding is preserved.
use crate::{FontSet, Glyph, Layout};
use skrifa::{
    instance::{LocationRef, Size},
    outline::{DrawSettings, OutlinePen},
    FontRef, GlyphId, MetadataProvider,
};
use tiny_skia::{Path, PathBuilder};

pub type Point = [f32; 2];
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Move(Point),
    Line(Point),
    Cubic(Point, Point, Point),
    Close,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outline {
    pub commands: Vec<Command>,
}
struct Pen {
    outline: Outline,
    current: Point,
    origin: Point,
}
impl Pen {
    fn point(&self, x: f32, y: f32) -> Point {
        [self.origin[0] + x, self.origin[1] - y]
    }
}
impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.point(x, y);
        self.current = p;
        self.outline.commands.push(Command::Move(p));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.point(x, y);
        self.current = p;
        self.outline.commands.push(Command::Line(p));
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let q = self.point(cx, cy);
        let end = self.point(x, y);
        let c1 = lerp(self.current, q, 2. / 3.);
        let c2 = lerp(end, q, 2. / 3.);
        self.outline.commands.push(Command::Cubic(c1, c2, end));
        self.current = end;
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let end = self.point(x, y);
        self.outline.commands.push(Command::Cubic(self.point(cx0, cy0), self.point(cx1, cy1), end));
        self.current = end;
    }
    fn close(&mut self) {
        self.outline.commands.push(Command::Close);
    }
}
pub fn glyph_outline(fonts: &FontSet, g: &Glyph) -> Result<Outline, &'static str> {
    let font = FontRef::new(&fonts.face(g.face).ok_or("invalid face id")?.bytes).map_err(|_| "invalid face")?;
    let glyph = font.outline_glyphs().get(GlyphId::new(u32::from(g.id))).ok_or("no monochrome outline")?;
    let mut pen =
        Pen { outline: Outline::default(), current: [0., 0.], origin: [g.x + g.offset[0], g.y + g.offset[1]] };
    glyph
        .draw(DrawSettings::unhinted(Size::new(g.size), LocationRef::default()), &mut pen)
        .map_err(|_| "outline draw failed")?;
    Ok(pen.outline)
}
pub fn layout_outlines(fonts: &FontSet, layout: &Layout) -> Result<Vec<Outline>, &'static str> {
    layout.lines.iter().flat_map(|l| &l.glyphs).map(|g| glyph_outline(fonts, g)).collect()
}
impl Outline {
    fn path(&self) -> Option<Path> {
        let mut p = PathBuilder::new();
        for c in &self.commands {
            match *c {
                Command::Move(a) => p.move_to(a[0], a[1]),
                Command::Line(a) => p.line_to(a[0], a[1]),
                Command::Cubic(a, b, c) => p.cubic_to(a[0], a[1], b[0], b[1], c[0], c[1]),
                Command::Close => p.close(),
            }
        }
        p.finish()
    }
    /// Conservative ink bounds from Bezier control hull, includes all marks.
    pub fn ink_bounds(&self) -> Option<[f32; 4]> {
        self.path().map(|p| {
            let b = p.bounds();
            [b.left(), b.top(), b.right(), b.bottom()]
        })
    }
}
fn lerp(a: Point, b: Point, t: f32) -> Point {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}
pub fn ink_bounds(outlines: &[Outline]) -> Option<[f32; 4]> {
    outlines
        .iter()
        .filter_map(Outline::ink_bounds)
        .reduce(|a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, FallbackPolicy, FontFace, Request};
    use tiny_skia::{FillRule, Paint, Pixmap, Transform};

    #[test]
    fn winding_preserves_glyph_counter() {
        let fonts = FontSet::new(
            vec![FontFace::new("Inter", 400, include_bytes!("../assets/fonts/Inter-Regular.ttf").as_slice().into())
                .unwrap()],
            FallbackPolicy::default(),
        )
        .unwrap();
        let mut engine = Engine::new(fonts.clone()).unwrap();
        let layout = engine.layout(&Request::new("O", 100., None)).unwrap();
        let path = glyph_outline(&fonts, &layout.lines[0].glyphs[0]).unwrap().path().unwrap();
        let bounds = path.bounds();
        let mut pixmap = Pixmap::new(160, 160).unwrap();
        let mut paint = Paint::default();
        paint.set_color_rgba8(0, 0, 0, 255);
        pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::from_translate(10. - bounds.left(), 10. - bounds.top()),
            None,
        );
        let x = (10. + bounds.width() / 2.) as u32;
        let y = (10. + bounds.height() / 2.) as u32;
        for dy in y - 2..=y + 2 {
            for dx in x - 2..=x + 2 {
                assert_eq!(pixmap.pixel(dx, dy).unwrap().alpha(), 0, "counter must remain empty");
            }
        }
        assert!((0..x).any(|dx| pixmap.pixel(dx, y).unwrap().alpha() > 200), "left stroke must be filled");
        assert!((x + 1..160).any(|dx| pixmap.pixel(dx, y).unwrap().alpha() > 200), "right stroke must be filled");
    }
}
