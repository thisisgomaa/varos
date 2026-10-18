use vectorcraft_doc::{CharStyle, Justify, TextKind, TextObject};
use vectorcraft_geom::{Point, Rect, shapes};
use vectorcraft_text::{FontDb, layout};
fn main() {
    let db = FontDb::with_font_dirs(vec![]);
    db.set_system_fallback(false);
    db.add_font(include_bytes!("../../../assets/fonts/SourceSans3-Regular.ttf").to_vec());
    let mut text = TextObject::point(Point::ZERO, "Latin text", CharStyle { font_family: "Source Sans 3".into(), size: 20.0, ..Default::default() });
    text.kind = TextKind::Area { frame: shapes::rectangle(Rect::new(0.0, 0.0, 240.0, 100.0)) };
    text.para.justify = Justify::JustifyAll;
    let l = layout(&db, &text);
    for g in l.glyphs {
        println!("({}, {}, {}, {:?}, {:?}, {:?}),", g.gid, g.byte, g.len, g.origin.x.to_bits(), g.origin.y.to_bits(), g.advance.to_bits());
    }
}
