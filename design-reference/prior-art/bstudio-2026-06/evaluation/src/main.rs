//! Diagnostic harness, not a substitute for the P1 acceptance suite.
use bstudio_text::*;
use std::{collections::BTreeSet, io::Write, time::Instant};
use unicode_bidi::BidiInfo;
use unicode_segmentation::UnicodeSegmentation;
mod corpus;
const PLEX: &[u8] = include_bytes!(
    "../../../../../varos/crates/varos-app/assets/fonts/IBMPlexSansArabic-Regular.ttf"
);
const INTER: &[u8] =
    include_bytes!("../../../../../varos/crates/varos-app/assets/fonts/Inter-Regular.ttf");
fn params(size: f32, width: f32) -> LayoutParams {
    LayoutParams {
        font_size: size,
        max_width: width,
        align: Align::Start,
        line_height: 1.3,
        max_kashida_per_word: 0,
    }
}
fn font(text: &str) -> &'static [u8] {
    if text.chars().any(|c| ('\u{0600}'..='\u{06ff}').contains(&c)) {
        PLEX
    } else {
        INTER
    }
}
fn unique(v: impl Iterator<Item = usize>) -> Vec<usize> {
    let mut seen = BTreeSet::new();
    v.filter(|x| seen.insert(*x)).collect()
}
// Oracle uses full-paragraph levels and each actual line's observed cluster set.
// This checks ordering, not exact source coverage (trimmed whitespace is reported separately).
fn order_ok(text: &str, start: usize, end: usize, actual: &[usize]) -> bool {
    let bi = BidiInfo::new(text, None);
    let Some(p) = bi.paragraphs.iter().find(|p| p.range.contains(&start)) else {
        return actual.is_empty();
    };
    if end > p.range.end {
        return false;
    }
    let (levels, runs) = bi.visual_runs(p, start..end);
    let have: BTreeSet<_> = actual.iter().copied().collect();
    let mut expected = Vec::new();
    for r in runs {
        let mut bs: Vec<_> = text[r.clone()]
            .char_indices()
            .map(|(b, _)| b + r.start)
            .filter(|b| have.contains(b))
            .collect();
        if levels[r.start].is_rtl() {
            bs.reverse()
        }
        expected.extend(bs)
    }
    expected == actual
}
fn cubic_check(font: &[u8], gid: u32, size: f32) -> usize {
    let o = glyph_outline(font, gid, Some(size)).unwrap();
    let mut p = [0., 0.];
    let mut begin = p;
    let mut n = 0;
    for c in o.commands {
        match c {
            OutlineCmd::MoveTo { x, y } => {
                p = [x, -y];
                begin = p
            }
            OutlineCmd::LineTo { x, y } => p = [x, -y],
            OutlineCmd::Close => p = begin,
            OutlineCmd::CurveTo {
                c1x,
                c1y,
                c2x,
                c2y,
                x,
                y,
            } => {
                assert!([c1x, c1y, c2x, c2y, x, y].iter().all(|x| x.is_finite()));
                p = [x, -y];
                n += 1
            }
            OutlineCmd::QuadTo { cx, cy, x, y } => {
                let q = [cx, -cy];
                let end = [x, -y];
                let c1 = [
                    p[0] + 2. / 3. * (q[0] - p[0]),
                    p[1] + 2. / 3. * (q[1] - p[1]),
                ];
                let c2 = [
                    end[0] + 2. / 3. * (q[0] - end[0]),
                    end[1] + 2. / 3. * (q[1] - end[1]),
                ];
                for t in [0f32, 0.25, 0.5, 0.75, 1.] {
                    for a in 0..2 {
                        let u = 1. - t;
                        let quad = u * u * p[a] + 2. * u * t * q[a] + t * t * end[a];
                        let cubic = u * u * u * p[a]
                            + 3. * u * u * t * c1[a]
                            + 3. * u * t * t * c2[a]
                            + t * t * t * end[a];
                        assert!((quad - cubic).abs() < 0.001)
                    }
                }
                p = end;
                n += 1
            }
        }
        assert!(p.iter().all(|x| x.is_finite()))
    }
    n
}
fn corpus_run() {
    let mut raw = std::fs::File::create(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("glyph-evidence.txt"),
    )
    .unwrap();
    println!("row\tconfigs\tfinite_valid\tbidi_lines_checked\tbidi_bad\tbreaks_checked\tillegal_breaks\tmissing_layout_carets\tcubic_segments\tnotdef");
    for row in &corpus::CORPUS {
        writeln!(
            raw,
            "row={} group={} requirement={}",
            row.id, row.group, row.required_evidence
        )
        .unwrap();
        for (face, f) in [("Inter", INTER), ("Plex", PLEX)] {
            let (
                mut configs,
                mut finite,
                mut lines,
                mut bidi_bad,
                mut breaks,
                mut illegal,
                mut missing,
                mut cubics,
                mut notdef,
            ) = (0, 0, 0, 0, 0, 0, 0, 0, 0);
            for &text in row.texts {
                for size in [12., 48., 200.] {
                    for width in [f32::INFINITY, 120., 600.] {
                        configs += 1;
                        let l = layout_text(f, text, params(size, width)).unwrap();
                        let l2 = layout_text(f, text, params(size, width)).unwrap();
                        writeln!(
                            raw,
                            "{} {face} text={text:?} size={size} width={width} layout={l:?}",
                            row.id
                        )
                        .unwrap();
                        if size == 48. && width == 600. {
                            for para in text.split('\n') {
                                writeln!(
                                    raw,
                                    "visual={:?}",
                                    shape_visual_line(f, para, 0).unwrap()
                                )
                                .unwrap();
                            }
                        }
                        assert_eq!(format!("{l:?}"), format!("{l2:?}"));
                        let valid = l.lines.iter().flat_map(|l| &l.glyphs).all(|g| {
                            text.is_char_boundary(g.cluster)
                                && g.cluster < text.len()
                                && [g.x, g.baseline_y, g.x_advance]
                                    .iter()
                                    .all(|x| x.is_finite())
                        });
                        if valid {
                            finite += 1
                        }
                        for line in &l.lines {
                            for g in &line.glyphs {
                                cubics += cubic_check(f, g.glyph_id, size);
                                if g.glyph_id == 0 {
                                    notdef += 1
                                }
                            }
                        }
                        for b in text
                            .grapheme_indices(true)
                            .map(|(b, _)| b)
                            .chain([text.len()])
                        {
                            if l.caret_x(b).is_none() {
                                missing += 1
                            }
                        }
                        // Multi-paragraph API loses global offsets; don't manufacture paragraph ranges for it.
                        if !text.contains(['\n', '\r']) {
                            let starts: Vec<_> = l
                                .lines
                                .iter()
                                .map(|l| l.glyphs.iter().map(|g| g.cluster).min().unwrap_or(0))
                                .collect();
                            let legal: BTreeSet<_> = unicode_linebreak::linebreaks(text)
                                .map(|(b, _)| b)
                                .collect();
                            for (i, line) in l.lines.iter().enumerate() {
                                if line.glyphs.is_empty() {
                                    continue;
                                }
                                let start = starts[i];
                                let end = starts.get(i + 1).copied().unwrap_or(text.len());
                                if end < start {
                                    continue;
                                }
                                let actual = unique(line.glyphs.iter().map(|g| g.cluster));
                                lines += 1;
                                if !order_ok(text, start, end, &actual) {
                                    bidi_bad += 1;
                                    eprintln!("BIDI face={face} row={} size={size} width={width} range={start}..{end} actual={actual:?}",row.id)
                                }
                                if i + 1 < l.lines.len() {
                                    breaks += 1;
                                    if !legal.contains(&end) {
                                        illegal += 1;
                                        eprintln!("BREAK face={face} row={} text={text:?} size={size} width={width} byte={end}",row.id)
                                    }
                                }
                            }
                        }
                    }
                }
            }
            println!("{}-{face}\t{configs}\t{finite}\t{lines}\t{bidi_bad}\t{breaks}\t{illegal}\t{missing}\t{cubics}\t{notdef}",row.id);
        }
    }
    // Keep additional checks on the separate visual/caret path separate from layout_text.
    let (mut checks, mut bad, mut roundtrip, mut rt_bad, mut inside) = (0, 0, 0, 0, 0);
    for row in &corpus::CORPUS {
        for &text in row.texts {
            for para in text.split('\n') {
                let f = font(para);
                let v = shape_visual_line(f, para, 0).unwrap();
                let actual = unique(v.glyphs.iter().map(|g| g.cluster as usize));
                checks += 1;
                if !order_ok(para, 0, para.len(), &actual) {
                    bad += 1
                }
                let gs: BTreeSet<_> = para
                    .grapheme_indices(true)
                    .map(|(b, _)| b)
                    .chain([para.len()])
                    .collect();
                for &b in &gs {
                    let x = caret_x_units(f, para, 0, b).unwrap();
                    let back = byte_at_x_units(f, para, 0, x).unwrap();
                    roundtrip += 1;
                    if caret_x_units(f, para, 0, back).unwrap() != x {
                        rt_bad += 1
                    }
                    if !gs.contains(&back) {
                        eprintln!("CARET text={para:?} requested={b} returned_inside_grapheme={back} x={x}");
                        inside += 1
                    }
                }
            }
        }
    }
    println!("VISUAL lines={checks} bidi_bad={bad} caret_x_roundtrips={roundtrip} bad_x={rt_bad} returned_inside_grapheme={inside}");
    let mut words = BTreeSet::new();
    for row in &corpus::CORPUS {
        for t in row.texts {
            for w in t.split_whitespace() {
                if w.chars().any(|c| ('\u{0600}'..='\u{06ff}').contains(&c)) {
                    words.insert(w);
                }
            }
        }
    }
    for word in words {
        let source = word.to_string();
        let base = shape_run(PLEX, &source, TextDirection::RightToLeft).unwrap();
        let result = insert_kashida(PLEX, &source, 3).unwrap();
        let working = kashida::working_text(&source, &result.inserted_at_byte_offsets);
        assert_eq!(source, word);
        for &b in &result.inserted_at_byte_offsets {
            assert!(base
                .glyphs
                .iter()
                .any(|g| g.cluster as usize == b && g.safe_to_insert_tatweel));
            let prev = word[..b].chars().last().unwrap();
            let next = word[b..].chars().next().unwrap();
            assert!(kashida::hallberg_allows(
                kashida::classify(prev),
                kashida::classify(next)
            ));
        }
        let reshaped = shape_run(PLEX, &working, TextDirection::RightToLeft).unwrap();
        assert_eq!(
            format!("{:?}", reshaped.glyphs),
            format!("{:?}", result.run.glyphs)
        );
        println!("KASHIDA word={word:?} flags={:?} inserted={:?} dropped={} source_bytes={} working_bytes={}",base.glyphs.iter().filter(|g|g.safe_to_insert_tatweel).map(|g|g.cluster).collect::<Vec<_>>(),result.inserted_at_byte_offsets,result.dropped_unsafe,word.len(),working.len());
    }
    let expanded = shape_visual_line(PLEX, "شعار", 3).unwrap();
    println!(
        "WORKING_CLUSTERS source_bytes={} clusters={:?}",
        "شعار".len(),
        expanded
            .glyphs
            .iter()
            .map(|g| g.cluster)
            .collect::<Vec<_>>()
    );
    let joined = shape_run(PLEX, "بب", TextDirection::RightToLeft).unwrap();
    let single = shape_run(PLEX, "ب", TextDirection::RightToLeft).unwrap();
    assert!(joined
        .glyphs
        .iter()
        .any(|g| g.glyph_id != single.glyphs[0].glyph_id));
    let marks = shape_run(PLEX, "السَّلَامُ عَلَيْكُمْ", TextDirection::RightToLeft).unwrap();
    println!(
        "MARK positioned_in_shaper={} layout_offset_fields=absent",
        marks
            .glyphs
            .iter()
            .filter(|g| g.x_offset != 0 || g.y_offset != 0)
            .count()
    );
    println!(
        "FALLBACK inter_arabic_notdef={} plex_probe={:?} inter_probe={:?}",
        shape_run(INTER, "شعار", TextDirection::RightToLeft)
            .unwrap()
            .glyphs
            .iter()
            .filter(|g| g.glyph_id == 0)
            .count(),
        probe_font(PLEX),
        probe_font(INTER)
    );
    println!(
        "NEWLINE second_line_clusters={:?}",
        layout_text(PLEX, "A\nشعار", params(48., 600.))
            .unwrap()
            .lines[1]
            .glyphs
            .iter()
            .map(|g| g.cluster)
            .collect::<Vec<_>>()
    );
}
fn bench(count: usize, sequential: bool) {
    let mut text: String = "Logo شعار 12 ".chars().cycle().take(count).collect();
    let start = Instant::now();
    let cold = layout_text(PLEX, &text, params(48., 600.)).unwrap();
    let cold_ms = start.elapsed().as_secs_f64() * 1000.;
    let mut times = Vec::new();
    for i in 0..if sequential { 200 } else { 41 } {
        let start = Instant::now();
        let mut edited = text.clone();
        let at = if sequential {
            match (i / 2) % 3 {
                0 => 0,
                1 => edited.char_indices().nth(count / 2).unwrap().0,
                _ => edited.len(),
            }
        } else {
            edited.char_indices().nth(count / 2).unwrap().0
        };
        if sequential && i % 2 == 1 {
            let p = if (i / 2) % 3 == 2 {
                edited.len() - 1
            } else {
                at
            };
            edited.remove(p);
        } else {
            edited.insert(at, if i % 2 == 0 { 'X' } else { 'Y' })
        }
        let layout_start = Instant::now();
        let result = layout_text(PLEX, &edited, params(48., 600.)).unwrap();
        std::hint::black_box(&result);
        let legacy_ms = layout_start.elapsed().as_secs_f64() * 1000.;
        drop(result);
        if sequential {
            text = edited;
            times.push(start.elapsed().as_secs_f64() * 1000.)
        } else {
            times.push(legacy_ms)
        }
    }
    times.sort_by(f64::total_cmp);
    println!("BENCH count={count} sequential={sequential} samples={} cold_ms={cold_ms:.3} p50={:.3} p95={:.3} max={:.3} lines={} timings={times:?}",times.len(),times[times.len()/2],times[(times.len()*95).div_ceil(100).saturating_sub(1)],times.last().unwrap(),cold.lines.len());
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("bench") {
        bench(
            args[2].parse().unwrap(),
            args.get(3).map(String::as_str) == Some("sequential"),
        )
    } else {
        corpus_run()
    }
}
