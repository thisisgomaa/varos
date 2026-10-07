use std::{error::Error, fmt::Write as _, fs, path::Path, time::Instant};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};
use varos_text_spike::{outlines::*, proof::*, *};

pub fn proofs(dir: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(dir)?;
    let mut engine = Engine::default();
    let mut dump = String::new();
    let mut diffs = String::from(
        "row\tinput\tface\tsize_pt\twidth_pt\tline\tpixels\tdiffering\tsevere_gt32\tmax_delta\tmean_delta\tinterior_flips\n",
    );
    let mut failures = 0;
    let mut count = 0;
    let mut worst_mean = 0f64;
    let mut worst_max = 0;
    let mut severe_total = 0;
    let mut interior_total = 0;
    let start = Instant::now();
    for row in &CORPUS {
        for (input, text) in row.texts.iter().enumerate() {
            for face in [Face::Inter, Face::Plex] {
                for size in SIZES_PT {
                    for width in [None, Some(120.), Some(600.)] {
                        let mut request = Request::new(text, size, width);
                        request.face = face;
                        let layout = engine.layout(&request)?;
                        writeln!(dump, "{} input={input} {face:?} {size}pt width={width:?}\n{layout:#?}", row.id)?;
                        for (line_i, line) in layout.lines.iter().enumerate() {
                            let outlines: Vec<_> = line.glyphs.iter().map(glyph_outline).collect::<Result<_, _>>()?;
                            let (diff, direct, converted) = nonzero_diff(&outlines, 2.)?;
                            count += 1;
                            worst_mean = worst_mean.max(diff.mean_delta);
                            worst_max = worst_max.max(diff.max_delta);
                            severe_total += diff.severe;
                            interior_total += diff.interior_flips;
                            writeln!(
                                diffs,
                                "{}\t{input}\t{face:?}\t{size}\t{width:?}\t{line_i}\t{}\t{}\t{}\t{}\t{:.6}\t{}",
                                row.id,
                                diff.pixels,
                                diff.differing,
                                diff.severe,
                                diff.max_delta,
                                diff.mean_delta,
                                diff.interior_flips
                            )?;
                            // Predeclared tolerance: <= 1 alpha level average and no >32 alpha discrepancy.
                            if diff.mean_delta > 1. || diff.severe > 0 {
                                failures += 1;
                                if failures <= 12 {
                                    direct.save_png(dir.join(format!("diff-{failures:02}-nonzero.png")))?;
                                    converted
                                        .save_png(dir.join(format!("diff-{failures:02}-flattened-nonzero.png")))?;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    fs::write(dir.join("corpus-layouts.txt"), dump)?;
    fs::write(dir.join("nonzero.tsv"), diffs)?;
    fs::write(dir.join("proof-summary.txt"),format!("@2x output; native Winding 4x4 subpixel samples, premultiplied box filter; flatten tolerance 0.005 pt; threshold mean<=1 alpha and zero pixels with delta>32\n{count} line comparisons; {failures} failures; max delta {worst_max}; max mean {worst_mean:.6}; total severe pixels {severe_total}\nInterior opaque/transparent flips: {interior_total}\nFull matrix layout/outline/diff/report wall time: {:.3} s\n",start.elapsed().as_secs_f64()))?;
    for group in ["shaping", "marks", "bidi", "wrapping", "controls", "fallback"] {
        sheet(&mut engine, dir, group)?;
    }
    new_sheets(&mut engine, dir)?;
    println!("{count} winding comparisons, {failures} outside declared tolerance. CPU proof sheets written.");
    if failures > 0 {
        return Err("winding acceptance gate failed; all proof artifacts preserved".into());
    }
    Ok(())
}
struct Block {
    caption: String,
    layout: Layout,
    outlines: Vec<Outline>,
    height: f32,
    min_y: f32,
    min_x: f32,
    right: f32,
    frame_width: f32,
}
fn sheet(engine: &mut Engine, dir: &Path, group: &str) -> Result<(), Box<dyn Error>> {
    let mut blocks = Vec::new();
    for row in CORPUS.iter().filter(|r| {
        r.group == group || (group == "legal" && ["mixed_bidi", "isolates_newlines", "wrapping"].contains(&r.id))
    }) {
        for text in row.texts {
            for face in [Face::Inter, Face::Plex] {
                let mut request =
                    Request::new(text, 48., Some(if group == "wrapping" || group == "legal" { 120. } else { 600. }));
                request.face = face;
                if row.id == "explicit_direction" {
                    request.direction = if text.starts_with("شعار") { Direction::Ltr } else { Direction::Rtl };
                }
                let layout = engine.layout(&request)?;
                let outlines = layout_outlines(&layout)?;
                let bounds = ink_bounds(&outlines).unwrap_or([0., 0., 1., 48.]);
                let bottom = layout.lines.last().map_or(48., |l| l.baseline + l.descent).max(bounds[3]);
                let min_y = bounds[1].min(0.);
                let substituted = layout.issues.iter().filter(|i| matches!(i, Issue::Substituted { .. })).count();
                let unsupported = layout.issues.iter().filter(|i| matches!(i, Issue::UnsupportedCluster(_))).count();
                let caption = format!(
                    "{} / requested {face:?} / 48 pt / {:?} / fallback {substituted} / unsupported {unsupported}",
                    row.id, request.direction
                );
                let min_x = bounds[0].min(0.);
                let right = (bounds[2] - min_x + 48.).max(700.);
                blocks.push(Block {
                    caption,
                    layout,
                    outlines,
                    height: bottom - min_y + 96.,
                    min_y,
                    min_x,
                    right,
                    frame_width: request.width.unwrap(),
                });
            }
        }
    }
    let height = 120. + blocks.iter().map(|b| b.height).sum::<f32>();
    let width = blocks.iter().map(|b| b.right).fold(700., f32::max);
    let mut sheet = Pixmap::new((width * 2.).ceil() as u32, (height * 2.).ceil() as u32).ok_or("sheet dimensions")?;
    sheet.fill(Color::from_rgba8(250, 248, 243, 255));
    label(engine, &mut sheet, &format!("TEXT P1b / {} / CPU proof @2x", group.to_uppercase()), 24., 30., 22.)?;
    label(
        engine,
        &mut sheet,
        "Layout preview / Winding / blue: carets / gray: frame / overflow shown in full",
        24.,
        65.,
        11.,
    )?;
    label(
        engine,
        &mut sheet,
        "12 / 48 / 200 pt x point / 120 / 600 pt: full numeric matrix in corpus-layouts.txt",
        24.,
        85.,
        11.,
    )?;
    let mut y = 120.;
    for b in blocks {
        label(engine, &mut sheet, &b.caption, 24., y, 11.)?;
        // Escaped source makes invisibles, LF/CRLF and isolates inspectable.
        let source = format!(
            "source: {}",
            b.layout
                .source
                .chars()
                .flat_map(|c| {
                    if c.is_control()
                        || matches!(c, '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2066}'..='\u{2069}' | '\u{a0}')
                    {
                        format!("\\u{{{:04x}}}", c as u32).chars().collect::<Vec<_>>()
                    } else {
                        vec![c]
                    }
                })
                .collect::<String>()
        );
        label(engine, &mut sheet, &source, 24., y + 19., 12.)?;
        let dy = y + 43. - b.min_y;
        // Retain overflow ink in the proof, including negative RTL positions.
        // The gray frame and carets share the same display-only translation.
        let transform = Transform::from_row(2., 0., 0., 2., (24. - b.min_x) * 2., dy * 2.);
        let mut paint = Paint::default();
        paint.set_color_rgba8(125, 125, 125, 110);
        let bottom = b.layout.lines.last().map_or(48., |l| l.baseline + l.descent);
        let frame = PathBuilder::from_rect(tiny_skia::Rect::from_xywh(0., 0., b.frame_width, bottom).unwrap());
        sheet.stroke_path(&frame, &paint, &Stroke { width: 0.4, ..Stroke::default() }, transform, None);
        paint.set_color_rgba8(25, 25, 25, 255);
        for outline in &b.outlines {
            if let Some(path) = outline.path() {
                sheet.fill_path(&path, &paint, FillRule::Winding, transform, None);
            }
        }
        let mut ticks = PathBuilder::new();
        for caret in &b.layout.carets {
            let baseline = b.layout.lines[caret.line].baseline;
            ticks.move_to(caret.x, baseline + 5.);
            ticks.line_to(caret.x, baseline + 11.);
        }
        if let Some(path) = ticks.finish() {
            paint.set_color_rgba8(12, 140, 233, 170);
            sheet.stroke_path(&path, &paint, &Stroke { width: 0.6, ..Stroke::default() }, transform, None);
        }
        y += b.height;
    }
    sheet.save_png(dir.join(format!("{group}@2x.png")))?;
    Ok(())
}
fn label(engine: &mut Engine, sheet: &mut Pixmap, text: &str, x: f32, y: f32, size: f32) -> Result<(), Box<dyn Error>> {
    let layout = engine.layout(&Request::new(text, size, Some(650.)))?;
    let mut paint = Paint::default();
    paint.set_color_rgba8(70, 72, 75, 255);
    for outline in layout_outlines(&layout)? {
        if let Some(path) = outline.path() {
            sheet.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::from_row(2., 0., 0., 2., x * 2., y * 2.),
                None,
            );
        }
    }
    Ok(())
}

fn new_sheets(engine: &mut Engine, dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut image = Pixmap::new(1400, 1100).ok_or("sheet")?;
    image.fill(Color::from_rgba8(250, 248, 243, 255));
    label(engine, &mut image, "P1b / per-run language / pinned Plex / 2x", 24., 20., 20.)?;
    label(engine, &mut image, "U+06F6: arab/URD locl lookup 6 substitutes uni0666", 24., 55., 12.)?;
    let mut all = Vec::new();
    for (i, language) in ["ar", "fa", "ur", "und"].iter().enumerate() {
        let y = 95. + i as f32 * 100.;
        label(engine, &mut image, language, 24., y, 16.)?;
        let mut r = Request::new("۶ سلام", 48., None);
        r.language = language;
        r.face = Face::Plex;
        let layout = engine.layout(&r)?;
        fs::write(dir.join(format!("language-{language}.txt")), format!("{layout:#?}"))?;
        let mut outlines = layout_outlines(&layout)?;
        for o in &mut outlines {
            for c in &mut o.commands {
                match c {
                    Command::Move(p) | Command::Line(p) => {
                        p[0] += 120.;
                        p[1] += y;
                    }
                    Command::Cubic(a, b, c) => {
                        for p in [a, b, c] {
                            p[0] += 120.;
                            p[1] += y;
                        }
                    }
                    Command::Close => {}
                }
            }
        }
        let mut paint = Paint::default();
        paint.set_color_rgba8(25, 25, 25, 255);
        for o in &outlines {
            if let Some(p) = o.path() {
                image.fill_path(&p, &paint, FillRule::Winding, Transform::from_scale(2., 2.), None);
            }
        }
        all.extend(outlines);
    }
    image.save_png(dir.join("language-forms@2x.png"))?;
    fs::write(dir.join("language-forms.svg"), export::svg(&all, 700., 550.))?;
    fs::write(dir.join("language-forms.pdf"), export::pdf(&all, 700., 550.))?;
    let layout = engine.layout(&Request::new("Logo شعار ۶ السَّلَامُ", 48., Some(600.)))?;
    let outlines = layout_outlines(&layout)?;
    let (diff, cubic, flat) = nonzero_diff(&outlines, 2.)?;
    cubic.save_png(dir.join("nonzero-fills@2x.png"))?;
    flat.save_png(dir.join("nonzero-flattened@2x.png"))?;
    fs::write(dir.join("nonzero-fill-diff.txt"), format!("{diff:#?}"))?;
    nonzero_coverage(&outlines, 1400, 300, Transform::from_scale(2., 2.), None, 255)?
        .save_png(dir.join("nonzero-export-reference@2x.png"))?;
    fs::write(dir.join("nonzero-fills.svg"), export::svg(&outlines, 700., 150.))?;
    fs::write(dir.join("nonzero-fills.pdf"), export::pdf(&outlines, 700., 150.))?;
    sheet(engine, dir, "legal")?;
    fs::copy(dir.join("legal@2x.png"), dir.join("fixed-wrapping@2x.png"))?;
    let mut edges = Pixmap::new(1400, 1500).ok_or("edge sheet")?;
    edges.fill(Color::from_rgba8(250, 248, 243, 255));
    label(engine, &mut edges, "P1b / Arabic line-edge reshaping / 2x", 24., 20., 20.)?;
    label(engine, &mut edges, "Source: beh + ZWSP + beh + ZWSP + beh (no inserted source characters)", 24., 55., 12.)?;
    let mut y = 95.;
    for width in [30., 60., 200.] {
        label(
            engine,
            &mut edges,
            &format!("width {width} pt / context clipped at actual line / whole legal unit overflow"),
            24.,
            y,
            12.,
        )?;
        let mut r = Request::new("ب\u{200b}ب\u{200b}ب", 48., Some(width));
        r.face = Face::Plex;
        r.language = "ar";
        let l = engine.layout(&r)?;
        fs::write(dir.join(format!("edge-shaping-{width}.txt")), format!("{l:#?}"))?;
        let mut paint = Paint::default();
        paint.set_color_rgba8(25, 25, 25, 255);
        for o in layout_outlines(&l)? {
            if let Some(path) = o.path() {
                edges.fill_path(
                    &path,
                    &paint,
                    FillRule::Winding,
                    Transform::from_row(2., 0., 0., 2., 100., (y + 24.) * 2.),
                    None,
                );
            }
        }
        y += 50. + l.lines.iter().map(|l| (l.ascent + l.descent).max(48. * 1.4)).sum::<f32>();
    }
    edges.save_png(dir.join("fixed-wrapping-edges@2x.png"))?;
    Ok(())
}
