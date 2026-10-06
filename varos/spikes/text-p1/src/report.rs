use std::{error::Error, fmt::Write as _, fs, path::Path, time::Instant};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};
use varos_text_spike::{outlines::*, proof::*, *};

pub fn proofs(dir: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(dir)?;
    let mut engine = Engine::default();
    let mut dump = String::new();
    let mut diffs = String::from(
        "row\tinput\tface\tsize_pt\twidth_pt\tline\tpixels\tdiffering\tsevere_gt32\tmax_delta\tmean_delta\tinterior_flips\tpolygon_max_delta\n",
    );
    let mut failures = 0;
    let mut count = 0;
    let mut worst_mean = 0f64;
    let mut worst_max = 0;
    let mut severe_total = 0;
    let mut interior_total = 0;
    let mut polygon_max = 0;
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
                            let (diff, direct, converted) = winding_diff(&outlines, 2.)?;
                            count += 1;
                            worst_mean = worst_mean.max(diff.mean_delta);
                            worst_max = worst_max.max(diff.max_delta);
                            severe_total += diff.severe;
                            interior_total += diff.interior_flips;
                            polygon_max = polygon_max.max(diff.polygon_max_delta);
                            writeln!(
                                diffs,
                                "{}\t{input}\t{face:?}\t{size}\t{width:?}\t{line_i}\t{}\t{}\t{}\t{}\t{:.6}\t{}\t{}",
                                row.id,
                                diff.pixels,
                                diff.differing,
                                diff.severe,
                                diff.max_delta,
                                diff.mean_delta,
                                diff.interior_flips,
                                diff.polygon_max_delta
                            )?;
                            // Predeclared tolerance: <= 1 alpha level average and no >32 alpha discrepancy.
                            if diff.mean_delta > 1. || diff.severe > 0 {
                                failures += 1;
                                if failures <= 12 {
                                    direct.save_png(dir.join(format!("diff-{failures:02}-nonzero.png")))?;
                                    converted.save_png(dir.join(format!("diff-{failures:02}-evenodd.png")))?;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    fs::write(dir.join("corpus-layouts.txt"), dump)?;
    fs::write(dir.join("winding.tsv"), diffs)?;
    fs::write(dir.join("proof-summary.txt"),format!("@2x; flatten tolerance 0.005 pt; threshold mean<=1 alpha and zero pixels with delta>32\n{count} line comparisons; {failures} failures; max delta {worst_max}; max mean {worst_mean:.6}; total severe pixels {severe_total}\nInterior opaque/transparent flips: {interior_total}; polygon-reference max alpha delta: {polygon_max}\nFull matrix layout/outline/diff/report wall time: {:.3} s\n",start.elapsed().as_secs_f64()))?;
    for group in ["shaping", "marks", "bidi", "wrapping", "controls", "fallback"] {
        sheet(&mut engine, dir, group)?;
    }
    println!("{count} winding comparisons, {failures} outside declared tolerance. Six proof sheets written.");
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
}
fn sheet(engine: &mut Engine, dir: &Path, group: &str) -> Result<(), Box<dyn Error>> {
    let mut blocks = Vec::new();
    for row in CORPUS.iter().filter(|r| r.group == group) {
        for text in row.texts {
            for face in [Face::Inter, Face::Plex] {
                let mut request = Request::new(text, 48., Some(if group == "wrapping" { 120. } else { 600. }));
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
                blocks.push(Block { caption, layout, outlines, height: bottom - min_y + 96., min_y });
            }
        }
    }
    let height = 120. + blocks.iter().map(|b| b.height).sum::<f32>();
    let mut sheet = Pixmap::new(1400, (height * 2.).ceil() as u32).ok_or("sheet dimensions")?;
    sheet.fill(Color::from_rgba8(250, 248, 243, 255));
    label(engine, &mut sheet, &format!("TEXT P1 / {} / CPU proof @2x", group.to_uppercase()), 24., 30., 22.)?;
    label(
        engine,
        &mut sheet,
        "Nonzero reference samples | converted even-odd checked in winding.tsv | blue ticks = carets",
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
                    if c.is_control() || matches!(c, '\u{200c}' | '\u{200d}' | '\u{2066}'..='\u{2069}' | '\u{a0}') {
                        format!("\\u{{{:04x}}}", c as u32).chars().collect::<Vec<_>>()
                    } else {
                        vec![c]
                    }
                })
                .collect::<String>()
        );
        label(engine, &mut sheet, &source, 24., y + 19., 12.)?;
        let dy = y + 43. - b.min_y;
        let transform = Transform::from_row(2., 0., 0., 2., 48., dy * 2.);
        let mut paint = Paint::default();
        paint.set_color_rgba8(25, 25, 25, 255);
        let all = Outline { commands: b.outlines.iter().flat_map(|o| o.commands.clone()).collect() };
        if let Some(path) = all.path() {
            sheet.fill_path(&path, &paint, FillRule::Winding, transform, None);
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
