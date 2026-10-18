//! Real text-node proof: no conversion to path nodes.
use std::error::Error;
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{CharStyle, Document, Justify, Node, NodeKind, TextKind, TextObject};
use vectorcraft_geom::{Point, Rect, shapes};
use vectorcraft_render::Renderer;
use vectorcraft_text::{FontDb, caret_position, hit_byte, layout, selection_quads};
use vectorcraft_text_arabic::{FALLBACK_FAMILY, FALLBACK_FONT};

fn main() -> Result<(), Box<dyn Error>> {
    let source = "بسم الله الرحمن الرحيم";
    let db = FontDb::global();
    db.set_system_fallback(false);
    db.add_font(FALLBACK_FONT.to_vec());
    let mut text = TextObject::point(
        Point::ZERO,
        source,
        CharStyle { font_family: FALLBACK_FAMILY.into(), size: 48.0, fill: Paint::solid(Color::BLACK), ..Default::default() },
    );
    text.kind = TextKind::Area { frame: shapes::rectangle(Rect::new(24.0, 24.0, 624.0, 156.0)) };
    text.para.justify = Justify::JustifyAll;
    let result = layout(db, &text);
    let line = result.lines.first().ok_or("no native text line")?;
    let adapter = vectorcraft_text_arabic::justify(source, FALLBACK_FONT, FALLBACK_FAMILY, 400, 48.0, 600.0)?;
    if result.lines.len() != 1 || result.glyphs.len() != adapter.glyphs.len() || adapter.insertions == 0 || (line.x1 - line.x0 - 600.0).abs() > 0.01 {
        return Err("native text layout did not use the kashida adapter".into());
    }
    let mut doc = Document::new(648.0, 180.0);
    let layer = doc.layers.first().ok_or("no layer")?.id;
    let id = doc.alloc_id();
    doc.insert(Some(layer), usize::MAX, Node::new(id, NodeKind::Text(Box::new(text))))?;
    let image = Renderer::new().render_region(&doc, Rect::new(0.0, 0.0, 648.0, 180.0), 1.0, true);
    let ink = image.pixels.chunks_exact(4).filter(|p| p[0] < 250 || p[1] < 250 || p[2] < 250).count();
    if ink == 0 {
        return Err("text-node render has no ink".into());
    }
    image::save_buffer("arabic_textnode.png", &image.to_straight(), image.width, image.height, image::ColorType::Rgba8)?;
    println!(
        "TEXT NODE id={id:?}; lines={}; glyphs={}; kashida_insertions={}; width={:.6}; ink_pixels={ink}; PNG=arabic_textnode.png",
        result.lines.len(),
        result.glyphs.len(),
        adapter.insertions,
        line.x1 - line.x0
    );
    println!(
        "caret endpoints: {:?} {:?}; selection quads={}",
        caret_position(&result, 0).0,
        caret_position(&result, source.len()).0,
        selection_quads(&result, 0, source.len()).len()
    );
    let mut mismatches = Vec::new();
    for byte in source.char_indices().map(|(byte, _)| byte).chain(std::iter::once(source.len())) {
        let (top, bottom) = caret_position(&result, byte);
        let hit = hit_byte(&result, Point::new(top.x, (top.y + bottom.y) * 0.5));
        if hit != byte {
            mismatches.push((byte, hit));
        }
    }
    println!("caret->hit roundtrip mismatches (source byte, hit byte): {mismatches:?}");
    Ok(())
}
