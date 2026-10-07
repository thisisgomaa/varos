//! Candidate AA policy, declared before measurement: 4x4 box samples per output pixel.
//! Does not replace the single-sample acceptance measurements by itself.
use varos_text_spike::{outlines::*, proof::*, *};
fn down(p: &tiny_skia::Pixmap) -> tiny_skia::Pixmap {
    let (w, h) = (p.width() / 4, p.height() / 4);
    let mut out = tiny_skia::Pixmap::new(w, h).unwrap();
    for y in 0..h {
        for x in 0..w {
            let mut sum = [0u32; 4];
            for dy in 0..4 {
                for dx in 0..4 {
                    let i = (((y * 4 + dy) * p.width() + x * 4 + dx) * 4) as usize;
                    for (s, v) in sum.iter_mut().zip(&p.data()[i..i + 4]) {
                        *s += u32::from(*v);
                    }
                }
            }
            let i = ((y * w + x) * 4) as usize;
            for (v, s) in out.data_mut()[i..i + 4].iter_mut().zip(sum) {
                *v = ((s + 8) / 16) as u8;
            }
        }
    }
    out
}
fn main() {
    let mut e = Engine::default();
    let mut n = 0;
    let mut failures = 0;
    for text in CORPUS.iter().flat_map(|r| r.texts) {
        for size in SIZES_PT {
            for width in [None, Some(120.), Some(600.)] {
                for face in [Face::Inter, Face::Plex] {
                    let mut r = Request::new(text, size, width);
                    r.face = face;
                    let l = e.layout(&r).unwrap();
                    for line in &l.lines {
                        let o: Vec<_> = line.glyphs.iter().map(glyph_outline).collect::<Result<_, _>>().unwrap();
                        let b = ink_bounds(&o).unwrap_or([0., 0., 1., 1.]);
                        let w = ((b[2] - b[0]) * 2.).ceil() as u32 + 12;
                        let h = ((b[3] - b[1]) * 2.).ceil() as u32 + 12;
                        let t = tiny_skia::Transform::from_row(8., 0., 0., 8., 24. - b[0] * 8., 24. - b[1] * 8.);
                        let a = down(&nonzero_coverage_single_sample(&o, w * 4, h * 4, t, None, 255).unwrap());
                        let z = down(&nonzero_coverage_single_sample(&o, w * 4, h * 4, t, Some(0.005), 255).unwrap());
                        let d = alpha_diff(&a, &z);
                        n += 1;
                        if d.mean_delta > 1. || d.severe > 0 {
                            failures += 1;
                            println!("FAIL {text:?} {size} {width:?} {face:?}: {d:?}");
                        }
                    }
                }
            }
        }
    }
    println!("{n} comparisons {failures} failures; 4x4 SSAA then 2x output; flatten .005pt; unchanged thresholds");
}
