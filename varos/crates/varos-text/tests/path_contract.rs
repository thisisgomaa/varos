mod common;
use common::*;
use tiny_skia::{Paint, PathBuilder, Pixmap, Transform};
use varos_text::{outlines::*, paths::*, *};
fn original_path(outline: &Outline) -> Option<tiny_skia::Path> {
    let mut p = PathBuilder::new();
    for c in &outline.commands {
        match *c {
            Command::Move(a) => p.move_to(a[0], a[1]),
            Command::Line(a) => p.line_to(a[0], a[1]),
            Command::Cubic(a, b, c) => p.cubic_to(a[0], a[1], b[0], b[1], c[0], c[1]),
            Command::Close => p.close(),
        }
    }
    p.finish()
}
fn anchor_path(contours: &[Contour]) -> Option<tiny_skia::Path> {
    let mut p = PathBuilder::new();
    for c in contours {
        let Some(first) = c.anchors.first() else { continue };
        p.move_to(first.p[0], first.p[1]);
        let count = c.anchors.len();
        for i in 0..count - 1 + usize::from(c.closed) {
            let a = &c.anchors[i];
            let b = &c.anchors[(i + 1) % count];
            if a.hout.is_some() || b.hin.is_some() {
                let x = a.hout.unwrap_or(a.p);
                let y = b.hin.unwrap_or(b.p);
                p.cubic_to(x[0], x[1], y[0], y[1], b.p[0], b.p[1]);
            } else {
                p.line_to(b.p[0], b.p[1]);
            }
        }
        if c.closed {
            p.close();
        }
    }
    p.finish()
}
fn pixels(p: Option<tiny_skia::Path>) -> Vec<u8> {
    let Some(p) = p else {
        return vec![];
    };
    let bounds = p.bounds();
    let mut image = Pixmap::new(bounds.width().ceil() as u32 + 8, bounds.height().ceil() as u32 + 8).unwrap();
    let mut paint = Paint::default();
    paint.set_color_rgba8(30, 30, 30, 180);
    image.fill_path(
        &p,
        &paint,
        tiny_skia::FillRule::Winding,
        Transform::from_translate(4. - bounds.left(), 4. - bounds.top()),
        None,
    );
    let data = image.data().to_vec();
    assert!(data.iter().any(|b| *b != 0), "nonempty glyph outline must actually render");
    data
}
#[test]
fn arabic_ligatures_marks_anchor_conversion_has_pixel_parity() {
    let mut e = engine();
    for text in ["لا لأ لإ لآ الله", "السَّلَامُ عَلَيْكُمْ", "Logo شعار 123، نعم؟"]
    {
        for size in [12., 48., 200.] {
            let l = e.layout(&Request::new(text, size, None)).unwrap();
            let out = layout_outlines(e.font_set(), &l).unwrap();
            let converted = layout_paths(e.font_set(), &l).unwrap();
            assert_eq!(out.len(), converted.len());
            for (o, p) in out.iter().zip(converted) {
                assert_eq!(p.fill_rule, FillRule::NonZero);
                assert_eq!(original_path(o).map(|p| p.bounds()), anchor_path(&p.contours).map(|p| p.bounds()));
                assert_eq!(pixels(original_path(o)), pixels(anchor_path(&p.contours)), "{text} size={size}");
            }
        }
    }
}
#[test]
fn cubic_close_keeps_incoming_handle_and_winding() {
    let o = Outline {
        commands: vec![
            Command::Move([0., 0.]),
            Command::Line([10., 0.]),
            Command::Cubic([10., 10.], [0., 10.], [0., 0.]),
            Command::Close,
        ],
    };
    let c = contours(&o).unwrap();
    assert_eq!(c[0].anchors.len(), 2);
    assert_eq!(c[0].anchors[0].hin, Some([0., 10.]));
    assert_eq!(c[0].anchors[1].hout, Some([10., 10.]));
    assert!(c[0].closed);
    assert_eq!(pixels(original_path(&o)), pixels(anchor_path(&c)));
}
#[test]
fn malformed_outline_refuses_instead_of_losing_segments() {
    assert!(contours(&Outline { commands: vec![Command::Line([0., 0.])] }).is_err());
    assert!(contours(&Outline { commands: vec![Command::Move([f32::NAN, 0.])] }).is_err());
}
#[test]
fn rtl_brackets_use_mirrored_glyphs() {
    let mut e = engine();
    let mut r = Request::new("(", 32., None);
    r.direction = Direction::Rtl;
    let rtl = e.layout(&r).unwrap();
    r.text = ")";
    r.direction = Direction::Ltr;
    let ltr = e.layout(&r).unwrap();
    assert_eq!(rtl.lines[0].glyphs[0].id, ltr.lines[0].glyphs[0].id);
    assert_eq!(rtl.lines[0].glyphs[0].face, ltr.lines[0].glyphs[0].face);
}

#[test]
fn degenerate_and_repeated_contour_closure_is_checked() {
    let point = [1., 2.];
    let single = contours(&Outline { commands: vec![Command::Move(point), Command::Close] }).unwrap();
    assert_eq!(single[0].anchors.len(), 1);
    assert!(single[0].closed);
    let duplicate =
        contours(&Outline { commands: vec![Command::Move(point), Command::Line(point), Command::Close] }).unwrap();
    assert_eq!(duplicate[0].anchors.len(), 1);
    assert!(contours(&Outline { commands: vec![Command::Close] }).is_err());
    assert!(contours(&Outline { commands: vec![Command::Move(point), Command::Close, Command::Close] }).is_err());
}
