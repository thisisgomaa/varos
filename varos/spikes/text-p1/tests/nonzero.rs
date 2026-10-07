use varos_text_spike::{outlines::*, proof::*, stencil::*, *};
fn rect(x: f32, reverse: bool) -> Outline {
    let mut p = [[x, 0.], [x + 20., 0.], [x + 20., 20.], [x, 20.]];
    if reverse {
        p.reverse();
    }
    Outline {
        commands: std::iter::once(Command::Move(p[0]))
            .chain(p[1..].iter().copied().map(Command::Line))
            .chain([Command::Close])
            .collect(),
    }
}
#[test]
fn independent_winding_holes_alpha_and_clip() {
    let mut outer = rect(0., false);
    let inner = Outline {
        commands: vec![
            Command::Move([5., 5.]),
            Command::Line([5., 15.]),
            Command::Line([15., 15.]),
            Command::Line([15., 5.]),
            Command::Close,
        ],
    };
    outer.commands.extend(inner.commands);
    let clip = vec![0x83; 900];
    let hole = cover(&[outer], 30, 30, &clip, 128, Rule::NonZero).unwrap();
    assert_eq!(hole.alpha[10 * 30 + 10], 0);
    assert_eq!(hole.alpha[1], 128);
    let glyphs = [rect(0., false), rect(10., true)];
    let mut clip = clip;
    clip[1] = 0x81;
    let c = cover(&glyphs, 30, 30, &clip, 128, Rule::NonZero).unwrap();
    assert_eq!(c.alpha[1], 0);
    assert_eq!(c.alpha[15], 128);
    assert_eq!(c.clip_after, clip);
    assert!(c.stencil_after.iter().all(|v| *v == 0));
    let raster = nonzero_coverage(&glyphs, 30, 30, tiny_skia::Transform::identity(), None, 128).unwrap();
    assert_eq!(raster.pixel(15, 1).unwrap().alpha(), 128);
    let combined = Outline { commands: (0..2).flat_map(|_| rect(0., false).commands).collect() };
    assert_eq!(cover(std::slice::from_ref(&combined), 30, 30, &clip, 255, Rule::EvenOdd).unwrap().alpha[15], 0);
    assert_eq!(cover(&[combined], 30, 30, &clip, 255, Rule::NonZero).unwrap().alpha[15], 255);
    let unsafe_outline = Outline { commands: (0..256).flat_map(|_| rect(0., false).commands).collect() };
    assert_eq!(cover(&[unsafe_outline], 1, 1, &[2], 255, Rule::NonZero), Err("winding overflow"));
    let empty = cover(&[], 30, 30, &clip, 128, Rule::NonZero).unwrap();
    assert!(empty.alpha.iter().all(|v| *v == 0));
}
#[test]
fn mixed_fonts_and_export_operators() {
    let l = Engine::default().layout(&Request::new("Logo شعار", 48., None)).unwrap();
    let o = layout_outlines(&l).unwrap();
    assert!(l.lines[0].glyphs.iter().any(|g| g.face == Face::Inter));
    assert!(l.lines[0].glyphs.iter().any(|g| g.face == Face::Plex));
    let svg = export::svg(&o, 600., 100.);
    assert_eq!(svg.matches("fill-rule=\"nonzero\"").count(), o.len());
    assert!(!svg.contains("evenodd"));
    let pdf = String::from_utf8(export::pdf(&o, 600., 100.)).unwrap();
    assert_eq!(pdf.lines().filter(|s| *s == "f").count(), o.len());
    assert!(!pdf.contains("f*"));
}
