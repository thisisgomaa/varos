//! Unhinted Skrifa quadratics -> exact cubics -> Y-down points.
//! Native winding is preserved. The old even-odd conversion remains diagnostic.
use crate::{Glyph, Layout};
use i_overlay::{core::fill_rule::FillRule, float::simplify::SimplifyShape};
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
pub fn glyph_outline(g: &Glyph) -> Result<Outline, &'static str> {
    let font = FontRef::new(g.face.bytes()).map_err(|_| "invalid face")?;
    let glyph = font.outline_glyphs().get(GlyphId::new(u32::from(g.id))).ok_or("no monochrome outline")?;
    let mut pen =
        Pen { outline: Outline::default(), current: [0., 0.], origin: [g.x + g.offset[0], g.y + g.offset[1]] };
    glyph
        .draw(DrawSettings::unhinted(Size::new(g.size), LocationRef::default()), &mut pen)
        .map_err(|_| "outline draw failed")?;
    Ok(pen.outline)
}
pub fn layout_outlines(layout: &Layout) -> Result<Vec<Outline>, &'static str> {
    layout.lines.iter().flat_map(|l| &l.glyphs).map(glyph_outline).collect()
}
impl Outline {
    pub fn path(&self) -> Option<Path> {
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
    pub fn flatten(&self, tolerance: f32) -> Vec<Vec<Point>> {
        let mut rings = Vec::new();
        let mut ring = Vec::new();
        let mut current = [0., 0.];
        for cmd in &self.commands {
            match *cmd {
                Command::Move(p) => {
                    if ring.len() > 2 {
                        rings.push(std::mem::take(&mut ring));
                    } else {
                        ring.clear();
                    }
                    ring.push(p);
                    current = p;
                }
                Command::Line(p) => {
                    ring.push(p);
                    current = p;
                }
                Command::Cubic(a, b, c) => {
                    flatten_cubic(current, a, b, c, tolerance.max(0.0001), 0, &mut ring);
                    current = c;
                }
                Command::Close => {
                    if ring.len() > 2 {
                        rings.push(std::mem::take(&mut ring));
                    } else {
                        ring.clear();
                    }
                }
            }
        }
        if ring.len() > 2 {
            rings.push(ring);
        }
        rings
    }
}
fn lerp(a: Point, b: Point, t: f32) -> Point {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}
fn flatten_cubic(a: Point, b: Point, c: Point, d: Point, tol: f32, depth: u8, out: &mut Vec<Point>) {
    // Distance from controls to the chord (including degenerate chord).
    let dist = |p: Point| {
        let dx = d[0] - a[0];
        let dy = d[1] - a[1];
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            (p[0] - a[0]).hypot(p[1] - a[1])
        } else {
            ((p[0] - a[0]) * dy - (p[1] - a[1]) * dx).abs() / len
        }
    };
    if depth >= 18 || dist(b).max(dist(c)) <= tol {
        out.push(d);
        return;
    }
    let ab = lerp(a, b, 0.5);
    let bc = lerp(b, c, 0.5);
    let cd = lerp(c, d, 0.5);
    let abc = lerp(ab, bc, 0.5);
    let bcd = lerp(bc, cd, 0.5);
    let m = lerp(abc, bcd, 0.5);
    flatten_cubic(a, ab, abc, m, tol, depth + 1, out);
    flatten_cubic(m, bcd, cd, d, tol, depth + 1, out);
}
pub fn even_odd_regions(outlines: &[Outline], tolerance: f32) -> Vec<Vec<Point>> {
    // Normalize each glyph independently, so opposite font winding conventions
    // cannot cancel each other at overlapping glyph joins.
    let rings: Vec<Vec<Point>> =
        outlines.iter().flat_map(|o| o.flatten(tolerance).simplify_shape(FillRule::NonZero)).flatten().collect();
    if rings.is_empty() {
        return vec![];
    }
    rings.simplify_shape(FillRule::NonZero).into_iter().flatten().collect()
}
pub fn rings_path(rings: &[Vec<Point>]) -> Option<Path> {
    let mut p = PathBuilder::new();
    for ring in rings {
        if let Some(first) = ring.first() {
            p.move_to(first[0], first[1]);
            for point in &ring[1..] {
                p.line_to(point[0], point[1]);
            }
            p.close();
        }
    }
    p.finish()
}
pub fn ink_bounds(outlines: &[Outline]) -> Option<[f32; 4]> {
    outlines
        .iter()
        .filter_map(Outline::ink_bounds)
        .reduce(|a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])])
}
