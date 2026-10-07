//! Emit/render-compare every original corpus line, with identical cropped pages.
use std::{fs, path::Path};
use varos_text_spike::{outlines::*, proof::*, *};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let out = Path::new(args.get(2).ok_or("mode directory")?);
    fs::create_dir_all(out)?;
    if args[1] == "emit" {
        let mut engine = Engine::default();
        let mut n = 0;
        let mut manifest = String::from("case\tsize\twidth\tface\tinput\tline_range\n");
        for text in CORPUS.iter().flat_map(|r| r.texts) {
            for size in SIZES_PT {
                for width in [None, Some(120.), Some(600.)] {
                    for face in [Face::Inter, Face::Plex] {
                        let mut r = Request::new(text, size, width);
                        r.face = face;
                        let l = engine.layout(&r)?;
                        for line in &l.lines {
                            let o: Vec<_> = line.glyphs.iter().map(glyph_outline).collect::<Result<_, _>>()?;
                            let b = ink_bounds(&o).unwrap_or([0., 0., 1., 1.]);
                            let w = ((b[2] - b[0]) * 2.).ceil() as u32 + 12;
                            let h = ((b[3] - b[1]) * 2.).ceil() as u32 + 12;
                            let origin = [3. - b[0], 3. - b[1]];
                            let name = format!("{n:04}");
                            fs::write(
                                out.join(format!("{name}.svg")),
                                export::svg_at(&o, w as f32 / 2., h as f32 / 2., origin),
                            )?;
                            fs::write(
                                out.join(format!("{name}.pdf")),
                                export::pdf_at(&o, w as f32 / 2., h as f32 / 2., origin),
                            )?;
                            nonzero_coverage(
                                &o,
                                w,
                                h,
                                tiny_skia::Transform::from_row(2., 0., 0., 2., origin[0] * 2., origin[1] * 2.),
                                None,
                                255,
                            )?
                            .save_png(out.join(format!("{name}-native.png")))?;
                            manifest.push_str(&format!(
                                "{name}\t{size}\t{width:?}\t{face:?}\t{text:?}\t{:?}\n",
                                line.range
                            ));
                            n += 1;
                        }
                    }
                }
            }
        }
        fs::write(out.join("cases.tsv"), manifest)?;
        println!("{n} native/PDF/SVG outline cases emitted");
    } else if args[1] == "compare" {
        let mut names: Vec<_> = fs::read_dir(out)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "pdf"))
            .collect();
        names.sort();
        let mut report = String::from("case\tbackend\tmax_alpha\tmean_alpha\tsevere\n");
        let mut failures = 0;
        let mut max = 0;
        let mut mean = 0f64;
        let count = names.len();
        for p in names {
            let stem = p.file_stem().unwrap().to_str().unwrap();
            let reference = tiny_skia::Pixmap::load_png(out.join(format!("{stem}-native.png")))?;
            for backend in ["svg", "pdf"] {
                let image = tiny_skia::Pixmap::load_png(out.join(backend).join(format!("{stem}.png")))?;
                let image = downsample_4x(&image)?;
                image.save_png(out.join(backend).join(format!("{stem}@2x.png")))?;
                let d = alpha_diff(&reference, &image);
                max = max.max(d.max_delta);
                mean = mean.max(d.mean_delta);
                failures += usize::from(d.mean_delta > 1. || d.severe > 0);
                report.push_str(&format!("{stem}\t{backend}\t{}\t{:.9}\t{}\n", d.max_delta, d.mean_delta, d.severe));
            }
        }
        fs::write(out.join("comparisons.tsv"), report)?;
        println!("{} comparisons; {failures} failures; max alpha {max}; worst mean {mean:.9}; fixed mean<=1 and zero delta>32",count*2);
        if failures > 0 {
            return Err("export pixel parity failed".into());
        }
    } else {
        return Err("expected emit or compare".into());
    }
    Ok(())
}
